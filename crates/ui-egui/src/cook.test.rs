#[cfg(test)]
mod cook_tests {
    use crate::MaterialCraftApp;
    use crate::cook::{drive, start};
    use crate::state::UiState;
    use materialcraft_engine::{execute, sample_session};
    use serde_json::json;

    fn wait_and_drive(app: &mut MaterialCraftApp, ctx: &egui::Context) {
        for _ in 0..2000 {
            drive(app, ctx);
            if !app.cooker.busy() {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        panic!("background cook never finished");
    }

    /// A background cook of version A lands after a synchronous cook of version B (as in
    /// `session.inspect`): B's results must survive.
    #[test]
    fn stale_background_cook_does_not_overwrite_newer_sync_cook() {
        let mut session = sample_session();
        session.preview_size = Some(64);
        let mut app = MaterialCraftApp::headless(session, UiState::default());
        let ctx = egui::Context::default();

        start(&mut app, 0.0); // cook of version A runs in the background
        let added = execute(&mut app.session, "graph.node.add", &json!({"def": "uniform", "at": [0, 0]})).unwrap();
        let id = materialcraft_engine::core::NodeId(added["id"].as_u64().unwrap());
        assert!(app.session.dirty);
        app.session.cook_if_dirty(); // version B cooked synchronously, dirty cleared
        assert!(app.session.cooked.nodes.contains_key(&id));

        wait_and_drive(&mut app, &ctx);
        assert!(app.session.cooked.nodes.contains_key(&id), "stale cook replaced the newer results");
    }
}

#[cfg(test)]
mod worker_failure_tests {
    use crate::MaterialCraftApp;
    use crate::cook::{drive, guard};
    use crate::state::UiState;
    use materialcraft_engine::PreviewCook;
    use materialcraft_engine::sample_session;

    fn app() -> MaterialCraftApp {
        let mut session = sample_session();
        session.preview_size = Some(64);
        session.cook();
        MaterialCraftApp::headless(session, UiState::default())
    }

    #[test]
    fn guard_turns_a_panic_into_an_error() {
        let r: Result<u32, String> = guard("test", || -> Result<u32, String> { panic!("boom") });
        let e = r.unwrap_err();
        assert!(e.contains("boom"), "{e}");
        assert_eq!(guard("test", || Ok::<_, String>(7)).unwrap(), 7);
    }

    #[test]
    fn a_panicked_cook_worker_reports_errors_on_every_node() {
        let mut app = app();
        assert!(app.session.cook_failures().0.is_empty());
        let (tx, rx) = std::sync::mpsc::channel::<Result<PreviewCook, String>>();
        app.cooker.rx = Some(rx);
        app.cooker.epoch = app.session.cook_epoch;
        tx.send(Err("boom".into())).unwrap();
        app.session.dirty = false;
        drive(&mut app, &egui::Context::default());
        assert!(!app.cooker.busy());
        let (failed, _) = app.session.cook_failures();
        assert!(!failed.is_empty());
        assert!(failed.iter().all(|(_, e)| e.contains("boom")));
        assert!(app.toasts.iter().any(|t| t.error && t.text.contains("boom")));
    }

    #[test]
    fn a_dead_cook_worker_is_reported_not_silent() {
        let mut app = app();
        let (tx, rx) = std::sync::mpsc::channel::<Result<PreviewCook, String>>();
        app.cooker.rx = Some(rx);
        app.cooker.epoch = app.session.cook_epoch;
        drop(tx);
        app.session.dirty = false;
        drive(&mut app, &egui::Context::default());
        assert!(!app.cooker.busy());
        assert!(app.toasts.iter().any(|t| t.error));
        assert!(!app.session.cook_failures().0.is_empty());
    }
}

#[test]
fn stale_worker_failure_does_not_replace_a_newer_sync_cook() {
    let mut session = materialcraft_engine::sample_session();
    session.preview_size = Some(64);
    session.cook();
    let mut app = crate::MaterialCraftApp::headless(session, crate::UiState::default());
    let (tx, rx) = std::sync::mpsc::channel();
    app.cooker.rx = Some(rx);
    app.cooker.epoch = app.session.cook_epoch.wrapping_sub(1);
    tx.send(Err("stale failure".into())).unwrap();
    let count = app.session.cooked.nodes.len();
    crate::cook::drive(&mut app, &egui::Context::default());
    assert_eq!(app.session.cooked.nodes.len(), count);
    assert!(app.session.cook_failures().0.is_empty());
    assert!(app.toasts.is_empty());
}
