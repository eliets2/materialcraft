//! Background cooking: when the open graph changes, a worker thread cooks a snapshot of the
//! package through the shared cache while the UI keeps drawing the previous results

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::mpsc::{Receiver, TryRecvError};

use materialcraft_engine::PreviewCook;
use materialcraft_engine::eval::{Cooked, NodeResult};

use crate::MaterialCraftApp;

/// Run `f`, turning a panic into an error naming `what`, so one bad node or command can't take
/// the app (and unsaved work) down
pub fn guard<T>(what: &str, f: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
    catch_unwind(AssertUnwindSafe(f)).unwrap_or_else(|p| {
        let msg = p.downcast_ref::<&str>().map(|s| s.to_string()).or_else(|| p.downcast_ref::<String>().cloned()).unwrap_or_default();
        Err(format!("internal error in {what}: {msg}"))
    })
}

/// Run an engine command; a panic inside it becomes an error reply (the session is kept)
pub fn execute_guarded(session: &mut materialcraft_engine::Session, id: &str, params: &serde_json::Value) -> Result<serde_json::Value, String> {
    guard(&format!("command `{id}`"), || materialcraft_engine::execute(session, id, params).map_err(|e| e.to_string()))
}

/// A cook that crashed or died: every node of the open graph shows the error instead of stale
/// previews, and the user sees a toast
fn report_failure(app: &mut MaterialCraftApp, msg: String) {
    let ids: Vec<_> = app.session.graph().map(|g| g.nodes.iter().map(|n| n.id).collect()).unwrap_or_default();
    let mut cooked = Cooked::default();
    for id in ids {
        cooked.nodes.insert(id, NodeResult { outputs: Default::default(), error: Some(msg.clone()), ms: 0.0, key: 0, cached: false });
    }
    app.session.accept_preview(PreviewCook { graph: app.session.graph, cooked, ..Default::default() });
    app.toast(msg, true);
}

#[derive(Default)]
pub struct Cooker {
    rx: Option<Receiver<Result<PreviewCook, String>>>,
    /// Stops the running cook between nodes once its result is stale
    #[cfg(not(target_arch = "wasm32"))]
    cancel: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    /// When the running cook started (app seconds)
    pub started: Option<f64>,
    /// `Session::cook_epoch` when the running cook started
    epoch: u64,
}

impl Cooker {
    pub fn busy(&self) -> bool {
        self.rx.is_some()
    }
}

/// Collect a finished cook and start the next one if the graph changed
pub fn drive(app: &mut MaterialCraftApp, ctx: &egui::Context) {
    if let Some(rx) = &app.cooker.rx {
        match rx.try_recv() {
            Ok(result) => {
                app.cooker.rx = None;
                app.cooker.started = None;
                #[cfg(not(target_arch = "wasm32"))]
                {
                    app.cooker.cancel = None;
                }
                match result {
                    // A stopped cook is partial: keep the previous results and cook again
                    Ok(c) if c.cancelled() => {
                        if app.cooker.epoch == app.session.cook_epoch {
                            app.session.dirty = true;
                        }
                    }
                    Ok(c) => {
                        if !app.session.dirty && app.cooker.epoch == app.session.cook_epoch {
                            app.session.accept_preview(c);
                        }
                    }
                    Err(msg) if !app.session.dirty && app.cooker.epoch == app.session.cook_epoch => report_failure(app, msg),
                    Err(_) => {}
                }
                ctx.request_repaint();
            }
            Err(TryRecvError::Empty) if app.session.dirty => {
                // Stop between nodes, but wait for this worker to finish before starting
                // another. A slow node must not accumulate workers during rapid edits
                #[cfg(not(target_arch = "wasm32"))]
                if let Some(flag) = &app.cooker.cancel {
                    flag.store(true, std::sync::atomic::Ordering::Relaxed);
                }
                ctx.request_repaint_after(std::time::Duration::from_millis(16));
                return;
            }
            Err(TryRecvError::Empty) => {
                ctx.request_repaint_after(std::time::Duration::from_millis(16));
                return;
            }
            Err(TryRecvError::Disconnected) => {
                app.cooker.rx = None;
                app.cooker.started = None;
                if !app.session.dirty && app.cooker.epoch == app.session.cook_epoch {
                    report_failure(app, "the cook worker stopped unexpectedly".into());
                }
                #[cfg(not(target_arch = "wasm32"))]
                {
                    app.cooker.cancel = None;
                }
            }
        }
    }
    if app.session.dirty {
        start(app, ctx.input(|i| i.time));
        ctx.request_repaint();
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn start(app: &mut MaterialCraftApp, now: f64) {
    let s = &mut app.session;
    s.dirty = false;
    let pkg = s.pkg.clone();
    let (gi, max) = (s.graph, s.preview_size);
    let materials = s.material_preview.graphs();
    let cache = s.cache.clone();
    let epoch = s.cook_epoch;
    let dec = s.resource_decoder();
    let (tx, rx) = std::sync::mpsc::channel();
    let flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let thread_flag = flag.clone();
    use materialcraft_engine::eval::Options;
    let spawned = std::thread::Builder::new().name("materialcraft-cook".into()).spawn(move || {
        let mut opts = Options::new(&dec);
        opts.max_size = max;
        opts.cancel = Some(&thread_flag);
        let cooked = guard("cook", || Ok(PreviewCook::run(&pkg, gi, &materials, &opts, &cache)));
        let _ = tx.send(cooked);
    });
    match spawned {
        Ok(_) => {
            app.cooker.rx = Some(rx);
            app.cooker.epoch = epoch;
            app.cooker.cancel = Some(flag);
            app.cooker.started = Some(now);
        }
        Err(e) => {
            log::warn!("could not start a cook thread ({e}); cooking here");
            cook_here(app);
        }
    }
}

fn cook_here(app: &mut MaterialCraftApp) {
    if let Err(msg) = guard("cook", || {
        app.session.cook();
        Ok(())
    }) {
        app.session.dirty = false;
        report_failure(app, msg);
    }
}

#[cfg(target_arch = "wasm32")]
fn start(app: &mut MaterialCraftApp, _now: f64) {
    // shortcut: no threads on the web; the cook runs on the UI thread (the page pauses)
    cook_here(app);
}

#[cfg(test)]
#[path = "cook.test.rs"]
mod tests;

#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "cook_cancellation.test.rs"]
mod cancellation_tests;
