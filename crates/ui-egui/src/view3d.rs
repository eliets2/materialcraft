//! The 3D view: assigned material channels on a built-in scene, rendered by
//! `materialcraft-render` (one ray per pixel at half resolution while the camera moves, 2×2
//! supersampled when it rests). Left-drag orbits, Alt+drag or middle-drag pans, the wheel zooms,
//! Shift+drag turns the light and a double-click resets the camera.
//!
//! The full-quality frame waits until the view's size and the camera have rested for [`SETTLE`]
//! (a panel, divider or window drag changes the size on every step), then renders on a worker
//! thread so the UI never waits for it; until it lands the last frame stays up, kept in proportion.
//! A frame that is no longer wanted is cancelled. The browser has no threads and renders here.
//!
//! The height output raises the mesh for real ([`Scene::with_relief`]), built on a worker thread and cached until its inputs change.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::Duration;

use egui::{Color32, ColorImage, Rect, Sense, TextureHandle, TextureOptions, Ui, pos2, vec2};
use egui_phosphor::regular as icon;
use materialcraft_engine::view_cmds::{ENVIRONMENTS, SCENES};
use materialcraft_render::{Lighting, Maps, Relief, Scene, Settings};
use serde_json::json;

use crate::MaterialCraftApp;
use crate::i18n::tr;
use crate::theme::Tokens;

/// Seconds the view's size and the camera rest before the full-quality frame renders.
pub const SETTLE: f64 = 0.15;

/// Triangle budget of the displaced mesh; the browser builds it on the UI thread, so it gets a smaller one.
const RELIEF_TRIANGLES: usize = if cfg!(target_arch = "wasm32") { 120_000 } else { materialcraft_render::VIEW_TRIANGLES };

#[derive(Default)]
pub struct View3dState {
    scene: Option<(String, Arc<Scene>)>,
    /// The scene displaced by the height output, keyed by what it was built from (see [`relief_key`]).
    displaced: Option<(u64, Arc<Scene>)>,
    /// The displaced scene building on a worker thread.
    relief_job: Option<ReliefJob>,
    light: Option<Arc<Lighting>>,
    tex: Option<TextureHandle>,
    /// What the texture shows: (content key, full quality).
    key: Option<(u64, bool)>,
    /// The view size (points) the texture was rendered for.
    tex_size: Option<(u32, u32)>,
    /// The view size on the last frame.
    size: Option<(u32, u32)>,
    /// When the view's size or the camera last changed (app seconds).
    changed_at: Option<f64>,
    /// The full-quality frame rendering on a worker thread.
    job: Option<Job>,
    pub render_ms: f32,
    /// Frames rendered so far (previews and full quality).
    pub renders: u64,
    /// The last frame shown (RGBA8, for `view.screenshot`).
    pub shown: Option<(u32, u32, Vec<u8>)>,
    show_env: bool,
}

impl View3dState {
    /// A frame or its displaced geometry is still being built in the background.
    pub fn busy(&self) -> bool {
        self.job.is_some() || self.relief_job.is_some()
    }
}

/// A rendered frame: RGBA8 pixels and how long they took.
struct Frame {
    w: u32,
    h: u32,
    px: Vec<u8>,
    ms: f32,
}

/// A full-quality frame rendering on a worker thread. Dropping it cancels the render.
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
struct Job {
    key: u64,
    size: (u32, u32),
    cancel: Arc<AtomicBool>,
    rx: Receiver<Option<Frame>>,
}

impl Drop for Job {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

mod relief_job;
use relief_job::ReliefJob;

pub fn scene_label(app: &MaterialCraftApp) -> String {
    let id = &app.session.view3d.scene;
    if id == "custom"
        && let Some((name, _)) = &app.session.custom_mesh
    {
        return name.clone();
    }
    SCENES.iter().find(|(s, _)| s == id).map_or(id.as_str(), |(_, l)| l).to_string()
}

fn menus(app: &mut MaterialCraftApp, ui: &mut Ui) {
    let tk = Tokens::get(ui.ctx());
    egui::Frame::NONE.fill(tk.panel).inner_margin(egui::Margin::symmetric(4, 1)).show(ui, |ui| {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button(tr("Material"), |ui| {
                let sources = app.session.material_preview.sources(&app.session.pkg);
                for channel in materialcraft_engine::core::MaterialChannel::ALL {
                    let source = sources.get(&channel);
                    let label = source
                        .and_then(|source| {
                            let graph = app.session.pkg.graphs.get(source.graph)?;
                            let node = graph.node(source.node).ok()?;
                            Some(format!("{} / {}", graph.name, crate::graph::node_label(node)))
                        })
                        .unwrap_or_else(|| tr("Unassigned").to_string());
                    ui.label(format!("{}: {label}", tr(channel.label())));
                }
                ui.separator();
                if ui.button(tr("Clear material assignments")).clicked() {
                    app.run("view.3d.clear", json!({}));
                    ui.close();
                }
            });
            ui.menu_button(tr("Scene"), |ui| {
                for (id, label) in SCENES {
                    if ui.selectable_label(app.session.view3d.scene == *id, tr(label)).clicked() {
                        app.run("view.3d", json!({"scene": id}));
                        ui.close();
                    }
                }
                ui.separator();
                if ui.button(tr("Load mesh...")).clicked() {
                    crate::web_files::load_mesh(app, ui.ctx());
                    ui.close();
                }
                if app.session.custom_mesh.is_some() && ui.selectable_label(app.session.view3d.scene == "custom", scene_label_custom(app)).clicked() {
                    app.run("view.3d", json!({"scene": "custom"}));
                    ui.close();
                }
                ui.separator();
                if ui.button(tr("Reset scene")).clicked() {
                    app.run("view.3d.reset", json!({}));
                    ui.close();
                }
            });
            ui.menu_button(tr("Camera"), |ui| {
                for (label, yaw, pitch) in [("Default", 35.0, 25.0), ("Front", 0.0, 0.0), ("Top", 0.0, 89.0), ("Side", 90.0, 0.0)] {
                    if ui.button(tr(label)).clicked() {
                        app.run("view.3d", json!({"yaw": yaw, "pitch": pitch, "distance": 3.2, "target": [0.0, 0.0, 0.0]}));
                        ui.close();
                    }
                }
            });
            ui.menu_button(tr("Environment"), |ui| {
                for (id, label) in ENVIRONMENTS {
                    if ui.selectable_label(app.session.view3d.environment == *id, tr(label)).clicked() {
                        app.run("view.3d", json!({"environment": id}));
                        ui.close();
                    }
                }
                ui.separator();
                let mut rot = app.session.view3d.env_rotation;
                if crate::widgets::slider(ui, tr("Rotation"), &mut rot, 0.0..=360.0, 0).changed() {
                    app.run("view.3d", json!({"envRotation": rot}));
                }
                let mut exp = app.session.view3d.exposure;
                if crate::widgets::slider(ui, tr("Exposure"), &mut exp, -4.0..=4.0, 2).changed() {
                    app.run("view.3d", json!({"exposure": exp}));
                }
            });
            ui.menu_button(tr("Display"), |ui| {
                crate::preview_resolution::show(app, ui);
                ui.separator();
                let mut flip = app.session.view3d.normal_y_flip;
                if ui.checkbox(&mut flip, tr("Flip normal map Y (DirectX)")).changed() {
                    app.run("view.3d", json!({"normalYFlip": flip}));
                }
                ui.separator();
                ui.checkbox(&mut app.view3d.show_env, tr("Environment background"));
                ui.separator();
                let mut t = app.session.view3d.transparency;
                if ui.checkbox(&mut t, tr("Transparency (alpha)")).changed() {
                    app.run("view.3d", json!({"transparency": t}));
                }
                let mut a = app.session.view3d.alpha_view;
                if ui.checkbox(&mut a, tr("Show alpha channel")).changed() {
                    app.run("view.3d", json!({"alphaView": a}));
                }
            });
            ui.menu_button(tr("Renderer"), |ui| {
                let mut depth_percent = app.session.view3d.height_scale * 100.0;
                if crate::widgets::slider(ui, tr("Height depth (% of the mesh size)"), &mut depth_percent, 0.0..=20.0, 1).changed() {
                    app.run("view.3d", json!({"heightScale": depth_percent / 100.0}));
                }
                let mut level = app.session.view3d.height_level;
                if crate::widgets::slider(ui, tr("Height level (0 = rises out of the surface)"), &mut level, 0.0..=1.0, 2).changed() {
                    app.run("view.3d", json!({"heightLevel": level}));
                }
                let mut t = app.session.view3d.tiling;
                if crate::widgets::slider(ui, tr("Tiling"), &mut t, 0.25..=8.0, 2).changed() {
                    app.run("view.3d", json!({"tiling": t}));
                }
            });
        });
    });
}

pub fn show(app: &mut MaterialCraftApp, ui: &mut Ui) {
    let tk = Tokens::get(ui.ctx());
    menus(app, ui);
    let avail = ui.available_size();
    let (rect, resp) = ui.allocate_exact_size(vec2(avail.x, (avail.y - 22.0).max(40.0)), Sense::click_and_drag());
    crate::widgets::name(&resp, egui::Role::Image, "3D view");
    let graph_hover = resp.dnd_hover_payload::<crate::graph_drag_payload::GraphDragPayload>().is_some();
    if let Some(graph) = crate::widgets::dnd_release::<crate::graph_drag_payload::GraphDragPayload>(&resp) {
        app.run("view.3d.outputs", json!({"graph": graph.name}));
    }

    // Camera gestures
    let mut moving = false;
    if resp.dragged() {
        let d = resp.drag_delta();
        let v = &app.session.view3d;
        if ui.input(|i| i.modifiers.alt) || resp.dragged_by(egui::PointerButton::Middle) {
            if let Some((_, scene)) = &app.view3d.scene {
                let s = Settings { yaw: v.yaw, pitch: v.pitch, distance: v.distance, target: v.target, ..Settings::default() };
                let cam = app.session.camera;
                let k = cam.pan_speed / rect.height();
                app.session.view3d.target = cam.clamp_target(materialcraft_render::pan(scene, &s, d.x * k, d.y * k));
            }
        } else if ui.input(|i| i.modifiers.shift) {
            let rot = (v.env_rotation + d.x * 0.5).rem_euclid(360.0);
            app.session.view3d.env_rotation = rot;
        } else {
            let (yaw, pitch) = (v.yaw - d.x * 0.4, (v.pitch + d.y * 0.4).clamp(-89.0, 89.0));
            app.session.view3d.yaw = yaw.rem_euclid(360.0);
            app.session.view3d.pitch = pitch;
        }
        moving = true;
    }
    if resp.hovered() {
        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        if scroll.abs() > 0.0 {
            app.session.view3d.distance = (app.session.view3d.distance * (-scroll * 0.002).exp()).clamp(1.2, 20.0);
            moving = true;
        }
    }
    if resp.double_clicked() {
        app.run("view.3d", json!({"yaw": 35.0, "pitch": 25.0, "distance": 3.2, "target": [0.0, 0.0, 0.0]}));
    }
    render(app, ui.ctx(), rect, moving || resp.is_pointer_button_down_on());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 0.0, Color32::from_rgb(0x1b, 0x1c, 0x1e));
    if let Some(t) = &app.view3d.tex {
        p.image(t.id(), frame_rect(rect, app.view3d.tex_size), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
    }
    if app.session.view3d.alpha_view {
        p.text(rect.left_top() + vec2(8.0, 6.0), egui::Align2::LEFT_TOP, tr("Alpha channel"), egui::FontId::proportional(11.0), tk.viewport_text);
    }
    if app.session.material_maps().is_empty() {
        p.text(
            rect.center_top() + vec2(0.0, 18.0),
            egui::Align2::CENTER_TOP,
            tr("Assign an image with View in 3D view, or connect Output nodes with material usages"),
            egui::FontId::proportional(11.0),
            tk.viewport_text,
        );
    }
    p.text(
        rect.right_bottom() - vec2(8.0, 6.0),
        egui::Align2::RIGHT_BOTTOM,
        format!("{}: {:.0} ms", tr("Render time"), app.view3d.render_ms),
        egui::FontId::proportional(10.5),
        tk.viewport_text,
    );
    if graph_hover {
        p.rect_stroke(rect.shrink(2.0), 4.0, egui::Stroke::new(2.0, tk.accent), egui::StrokeKind::Inside);
        p.text(
            rect.center_top() + vec2(0.0, 38.0),
            egui::Align2::CENTER_TOP,
            tr("Drop to apply graph outputs"),
            egui::FontId::proportional(13.0),
            tk.viewport_text,
        );
    }

    // Bottom bar
    let bar = Rect::from_min_size(pos2(rect.left(), rect.bottom()), vec2(rect.width(), 22.0));
    ui.scope_builder(egui::UiBuilder::new().max_rect(bar), |ui| {
        ui.painter().rect_filled(bar, 0.0, tk.panel);
        ui.horizontal_centered(|ui| {
            ui.add_space(6.0);
            ui.label(egui::RichText::new(icon::SPHERE).color(tk.text_dim));
            let r = egui::ComboBox::from_id_salt("scene3d").width(110.0).selected_text(scene_label(app)).show_ui(ui, |ui| {
                for (id, label) in SCENES {
                    if ui.selectable_label(app.session.view3d.scene == *id, tr(label)).clicked() {
                        app.run("view.3d", json!({"scene": id}));
                    }
                }
            });
            crate::widgets::name(&r.response, egui::Role::ComboBox, "Scene");
            ui.label(egui::RichText::new(format!("{}  sRGB / ACES", icon::SUN)).size(11.0).color(tk.text_dim));
            let a = app.session.view3d.alpha_view;
            if crate::widgets::icon_button(ui, icon::CHECKERBOARD, tr("Show alpha channel"), a, 20.0).clicked() {
                app.run("view.3d", json!({"alphaView": !a}));
            }
        });
    });
}

fn scene_label_custom(app: &MaterialCraftApp) -> String {
    app.session.custom_mesh.as_ref().map_or_else(String::new, |(n, _)| n.clone())
}

/// The view's size in whole points: part of the content key and of `tex_size`.
fn view_size(rect: Rect) -> (u32, u32) {
    (rect.width() as u32, rect.height() as u32)
}

/// Where the last frame goes in `rect`: all of it when it was rendered for this size, otherwise
/// at the view's height and centred. The camera's field of view is vertical, so that is what a
/// new render will show (cropped, or with bars at the sides) while a resize is in progress,
/// instead of a stretched picture.
fn frame_rect(rect: Rect, rendered: Option<(u32, u32)>) -> Rect {
    match rendered {
        Some((w, h)) if h > 0 && (w, h) != view_size(rect) => Rect::from_center_size(rect.center(), vec2(rect.height() * w as f32 / h as f32, rect.height())),
        _ => rect,
    }
}

fn content_key(app: &MaterialCraftApp, w: u32, h: u32, show_env: bool, relief_shown: Option<u64>) -> u64 {
    use std::hash::{DefaultHasher, Hash, Hasher};
    let mut hs = DefaultHasher::new();
    let v = &app.session.view3d;
    (w, h, show_env, &v.scene, &v.environment, app.session.custom_mesh.as_ref().map(|(n, _)| n), v.transparency, v.alpha_view, relief_shown).hash(&mut hs);
    v.normal_y_flip.hash(&mut hs);
    for f in [v.yaw, v.pitch, v.distance, v.target[0], v.target[1], v.target[2], v.tiling, v.exposure, v.env_rotation, v.height_scale, v.height_level] {
        f.to_bits().hash(&mut hs);
    }
    app.session.material_key().hash(&mut hs);
    hs.finish()
}

/// Keep the 3D view's texture up to date: a quick frame right away while the camera moves, the
/// full-quality one once the view's size and the camera have rested for [`SETTLE`].
fn render(app: &mut MaterialCraftApp, ctx: &egui::Context, rect: Rect, moving: bool) {
    let now = ctx.input(|i| i.time);
    let size = view_size(rect);
    if moving || app.view3d.size.is_some_and(|s| s != size) {
        app.view3d.changed_at = Some(now);
    }
    app.view3d.size = Some(size);
    let relief_shown = update_relief(app, ctx);
    let key = content_key(app, size.0, size.1, app.view3d.show_env, relief_shown);
    collect(app, ctx, key);
    if app.view3d.key == Some((key, true)) || (moving && app.view3d.key == Some((key, false))) {
        return;
    }
    let ppp = ctx.pixels_per_point();
    if moving {
        // One ray per pixel at half resolution, here: the camera follows the pointer.
        if let Some(frame) = render_now(app, rect, ppp * 0.5, 1) {
            show_frame(app, ctx, frame, key, false, size);
        }
        ctx.request_repaint_after(Duration::from_secs_f64(SETTLE));
        return;
    }
    let wait = app.view3d.changed_at.map_or(0.0, |t| SETTLE - (now - t));
    if wait > 0.0 {
        // Mid-resize or the camera just stopped: the last frame stays up (see `frame_rect`).
        ctx.request_repaint_after(Duration::from_secs_f64(wait.min(SETTLE)));
        return;
    }
    if app.view3d.job.as_ref().is_some_and(|j| j.key == key) {
        return;
    }
    start_full(app, ctx, rect, key, size);
}

/// Show a finished full-quality frame if it is still the one wanted; drop (and so cancel) one
/// that is not.
fn collect(app: &mut MaterialCraftApp, ctx: &egui::Context, key: u64) {
    let Some(job) = app.view3d.job.take() else { return };
    if job.key != key {
        return;
    }
    match job.rx.try_recv() {
        Ok(Some(frame)) => show_frame(app, ctx, frame, key, true, job.size),
        Ok(None) | Err(TryRecvError::Disconnected) => {}
        Err(TryRecvError::Empty) => app.view3d.job = Some(job),
    }
}

/// The plain scene of the current view and its cache key, building it when the scene changed.
fn base_scene(app: &mut MaterialCraftApp) -> (String, Arc<Scene>) {
    let v = &app.session.view3d;
    let scene_key = match (&*v.scene, &app.session.custom_mesh) {
        ("custom", Some((name, _))) => format!("custom:{name}"),
        _ => v.scene.clone(),
    };

    match &app.view3d.scene {
        Some((key, scene)) if *key == scene_key => (scene_key, scene.clone()),
        _ => {
            let scene = match (&*v.scene, &app.session.custom_mesh) {
                ("custom", Some((_, mesh))) => Scene::new((**mesh).clone()),
                _ => Scene::builtin(&v.scene),
            };
            let scene = Arc::new(scene);
            app.view3d.scene = Some((scene_key.clone(), scene.clone()));
            (scene_key, scene)
        }
    }
}

/// What the displaced scene depends on (scene, height output cook, relief settings, normal map); `None` when nothing is raised.
fn relief_key(app: &MaterialCraftApp, scene_key: &str, maps: &Maps) -> Option<u64> {
    use std::hash::{DefaultHasher, Hash, Hasher};

    let v = &app.session.view3d;
    if maps.height.is_none() || v.height_scale <= 0.0 {
        return None;
    }

    let sources = app.session.material_preview.sources(&app.session.pkg);
    let height_source = sources.get(&materialcraft_engine::core::MaterialChannel::Height)?;
    let height_cook = app.session.material_source_key(height_source);

    let mut hasher = DefaultHasher::new();
    (scene_key, height_cook, maps.normal.is_some()).hash(&mut hasher);
    for f in [v.height_scale, v.height_level, v.tiling] {
        f.to_bits().hash(&mut hasher);
    }
    Some(hasher.finish())
}

/// Collect a finished relief build or start a missing one; returns the key of the displaced scene shown (`None` = plain mesh).
fn update_relief(app: &mut MaterialCraftApp, ctx: &egui::Context) -> Option<u64> {
    let (scene_key, base) = base_scene(app);
    let maps = Maps::from_usages(app.session.material_maps());
    let wanted = relief_key(app, &scene_key, &maps);

    if let Some(job) = app.view3d.relief_job.take() {
        match job.rx.try_recv() {
            Ok(Some(scene)) if Some(job.key) == wanted => app.view3d.displaced = Some((job.key, scene)),
            Ok(_) | Err(TryRecvError::Disconnected) => {}
            // Finish the active build before starting a newer one. Dropping its receiver
            // cannot cancel subdivision, so rapid edits must not accumulate workers.
            Err(TryRecvError::Empty) => app.view3d.relief_job = Some(job),
        }
    }

    let wanted = wanted?;

    if app.view3d.displaced.as_ref().is_some_and(|(key, _)| *key == wanted) {
        return Some(wanted);
    }
    if app.view3d.relief_job.is_none() {
        let v = &app.session.view3d;
        let relief = Relief { depth: v.height_scale, level: v.height_level };
        start_relief(app, ctx, wanted, base, maps, relief, v.tiling);
    }
    None
}

/// Build the displaced scene on a worker thread.
#[cfg(not(target_arch = "wasm32"))]
fn start_relief(app: &mut MaterialCraftApp, ctx: &egui::Context, key: u64, base: Arc<Scene>, maps: Maps, relief: Relief, tiling: f32) {
    let (tx, rx) = std::sync::mpsc::channel();
    let (repaint, viewport) = (ctx.clone(), ctx.viewport_id());
    let (thread_base, thread_maps) = (base.clone(), maps.clone());
    let spawned = std::thread::Builder::new().name("materialcraft-relief".into()).spawn(move || {
        let _ = tx.send(thread_base.with_relief(&thread_maps, relief, tiling, RELIEF_TRIANGLES).map(Arc::new));
        repaint.request_repaint_of(viewport);
    });

    match spawned {
        Ok(_) => app.view3d.relief_job = Some(ReliefJob { key, rx }),
        Err(e) => {
            log::warn!("could not start a 3D view relief thread ({e}); building it here");
            app.view3d.displaced = base.with_relief(&maps, relief, tiling, RELIEF_TRIANGLES).map(|scene| (key, Arc::new(scene)));
        }
    }
}

/// The browser has no threads: the displaced scene builds here (the page pauses).
#[cfg(target_arch = "wasm32")]
fn start_relief(app: &mut MaterialCraftApp, _ctx: &egui::Context, key: u64, base: Arc<Scene>, maps: Maps, relief: Relief, tiling: f32) {
    app.view3d.displaced = base.with_relief(&maps, relief, tiling, RELIEF_TRIANGLES).map(|scene| (key, Arc::new(scene)));
}

/// The scene, lighting, maps and settings of the current view: the displaced scene when it is current, else the plain one.
fn inputs(app: &mut MaterialCraftApp, supersample: u32) -> Option<(Arc<Scene>, Arc<Lighting>, Maps, Settings)> {
    let (scene_key, base) = base_scene(app);
    let v = app.session.view3d.clone();
    if app.view3d.light.as_ref().is_none_or(|l| l.name != v.environment) {
        app.view3d.light = Some(Arc::new(Lighting::new(&v.environment)));
    }

    let settings = Settings {
        yaw: v.yaw,
        pitch: v.pitch,
        distance: v.distance,
        target: v.target,
        tiling: v.tiling,
        exposure: v.exposure,
        env_rotation: v.env_rotation,
        normal_y_flip: v.normal_y_flip,
        show_env: app.view3d.show_env,
        supersample,
        transparency: v.transparency,
        alpha_view: v.alpha_view,
        ..Settings::default()
    };
    let maps = Maps::from_usages(app.session.material_maps());
    let wanted = relief_key(app, &scene_key, &maps);
    let scene = match &app.view3d.displaced {
        Some((key, displaced)) if Some(*key) == wanted => displaced.clone(),
        _ => base,
    };
    let light = app.view3d.light.clone()?;
    Some((scene, light, maps, settings))
}

/// Texture pixels for `rect` at `scale` pixels per point.
fn pixels(rect: Rect, scale: f32) -> (u32, u32) {
    (((rect.width() * scale) as u32).clamp(16, 4096), ((rect.height() * scale) as u32).clamp(16, 4096))
}

/// Render a frame on this thread.
fn render_now(app: &mut MaterialCraftApp, rect: Rect, scale: f32, supersample: u32) -> Option<Frame> {
    let (scene, light, maps, settings) = inputs(app, supersample)?;
    let (w, h) = pixels(rect, scale);
    let t0 = crate::now_ms();
    let px = materialcraft_render::render(&scene, &maps, &light, &settings, w, h);
    Some(Frame { w, h, px, ms: (crate::now_ms() - t0) as f32 })
}

/// Start the full-quality frame (2×2 supersampled, full resolution) on a worker thread.
#[cfg(not(target_arch = "wasm32"))]
fn start_full(app: &mut MaterialCraftApp, ctx: &egui::Context, rect: Rect, key: u64, size: (u32, u32)) {
    let Some((scene, light, maps, settings)) = inputs(app, 2) else { return };
    let (w, h) = pixels(rect, ctx.pixels_per_point());
    let cancel = Arc::new(AtomicBool::new(false));
    let (tx, rx) = std::sync::mpsc::channel();
    let (stop, repaint, viewport) = (cancel.clone(), ctx.clone(), ctx.viewport_id());
    let spawned = std::thread::Builder::new().name("materialcraft-view3d".into()).spawn(move || {
        let t0 = crate::now_ms();
        let px = materialcraft_render::render_cancellable(&scene, &maps, &light, &settings, w, h, &stop);
        let _ = tx.send(px.map(|px| Frame { w, h, px, ms: (crate::now_ms() - t0) as f32 }));
        repaint.request_repaint_of(viewport);
    });
    match spawned {
        Ok(_) => app.view3d.job = Some(Job { key, size, cancel, rx }),
        Err(e) => {
            log::warn!("could not start a 3D view render thread ({e}); rendering here");
            if let Some(frame) = render_now(app, rect, ctx.pixels_per_point(), 2) {
                show_frame(app, ctx, frame, key, true, size);
            }
        }
    }
}

/// The browser has no threads: the full-quality frame renders here (the page pauses).
#[cfg(target_arch = "wasm32")]
fn start_full(app: &mut MaterialCraftApp, ctx: &egui::Context, rect: Rect, key: u64, size: (u32, u32)) {
    if let Some(frame) = render_now(app, rect, ctx.pixels_per_point(), 2) {
        show_frame(app, ctx, frame, key, true, size);
    }
}

/// Upload a frame and remember what it shows.
fn show_frame(app: &mut MaterialCraftApp, ctx: &egui::Context, frame: Frame, key: u64, full: bool, size: (u32, u32)) {
    let st = &mut app.view3d;
    let img = ColorImage::from_rgba_unmultiplied([frame.w as usize, frame.h as usize], &frame.px);
    match &mut st.tex {
        Some(t) => t.set(img, TextureOptions::LINEAR),
        None => st.tex = Some(ctx.load_texture("view3d", img, TextureOptions::LINEAR)),
    }
    st.render_ms = frame.ms;
    st.renders += 1;
    st.shown = Some((frame.w, frame.h, frame.px));
    st.key = Some((key, full));
    st.tex_size = Some(size);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relief_tracks_assigned_height_while_another_graph_is_open() {
        let mut app = MaterialCraftApp::headless(materialcraft_engine::sample_session(), crate::UiState::default());
        app.session.preview_size = Some(16);
        app.session.cook();
        let (scene_key, _) = base_scene(&mut app);
        let original = relief_key(&app, &scene_key, &Maps::from_usages(app.session.material_maps())).unwrap();
        app.session.pkg.graphs.push(materialcraft_engine::core::Graph::new("Other"));
        app.session.graph = 1;
        app.session.cook();
        assert_eq!(relief_key(&app, &scene_key, &Maps::from_usages(app.session.material_maps())), Some(original));
        app.session.pkg.graphs[0].seed += 1;
        app.session.cook();
        assert_ne!(relief_key(&app, &scene_key, &Maps::from_usages(app.session.material_maps())), Some(original));
    }

    #[test]
    fn changing_relief_keeps_one_worker_and_discards_stale_geometry() {
        let mut app = MaterialCraftApp::headless(materialcraft_engine::sample_session(), crate::UiState::default());
        app.session.cook();
        let (scene_key, base) = base_scene(&mut app);
        let maps = Maps::from_usages(app.session.material_maps());
        let old_key = relief_key(&app, &scene_key, &maps).unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        app.view3d.relief_job = Some(ReliefJob { key: old_key, rx });
        let ctx = egui::Context::default();
        for index in 1..=20 {
            app.session.view3d.height_scale = index as f32 * 0.001;
            assert_eq!(update_relief(&mut app, &ctx), None);
            assert_eq!(app.view3d.relief_job.as_ref().unwrap().key, old_key, "a second subdivision worker started");
            assert!(app.view3d.busy(), "geometry work must count as a busy view");
        }
        let wanted = relief_key(&app, &scene_key, &maps).unwrap();
        assert_ne!(old_key, wanted);
        app.view3d.displaced = Some((wanted, base.clone()));
        tx.send(Some(Arc::new(Scene::builtin("sphere")))).unwrap();
        assert_eq!(update_relief(&mut app, &ctx), Some(wanted));
        assert!(app.view3d.relief_job.is_none());
        assert!(!app.view3d.busy());
        assert!(Arc::ptr_eq(&app.view3d.displaced.as_ref().unwrap().1, &base), "outdated geometry replaced the current scene");
    }

    #[test]
    fn a_frame_for_another_size_keeps_its_proportions() {
        let rect = Rect::from_min_size(pos2(10.0, 20.0), vec2(400.0, 200.0));
        assert_eq!(frame_rect(rect, None), rect);
        assert_eq!(frame_rect(rect, Some((400, 200))), rect);
        // Rendered narrower (the view was widened): full height, centred, bars at the sides.
        let r = frame_rect(rect, Some((300, 200)));
        assert_eq!((r.width(), r.height(), r.center()), (300.0, 200.0, rect.center()));
        // A degenerate size never divides by zero.
        assert_eq!(frame_rect(rect, Some((300, 0))), rect);
    }
}
