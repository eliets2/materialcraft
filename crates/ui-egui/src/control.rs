//! The JSON control channel: agents drive the running app with the same commands as the menus,
//! inspect session and UI state, invoke menu items, synthesize mouse and keyboard input and take
//! screenshots.
//!
//! Methods (params as JSON objects):
//! - `command.run {id, params?}`: run an engine command (programmatic: never opens a dialog).
//! - `command.list {filter?}`: every command with params docs and enabled state.
//! - `session.inspect`: package, graph, nodes, wires, parameters, cook results, views, history.
//! - `ui.inspect`: the UI state (panels, windows, graph camera, node rectangles and ports on screen).
//! - `ui.menu.list` / `ui.menu.invoke {id}`: the menu catalog; behave like a menu click.
//! - `ui.set {...}`: merge fields into the UI state (e.g. `{"prefs": {"thumbnails": false}}`).
//! - `ui.input {events}`: synthetic pointer, wheel, key and text input (see [`parse_events`]).
//! - `view.screenshot {view?: "3d"|"2d"}`: PNG of a view, base64.
//! - `ui.screenshot`: PNG of the whole window (replies after the next frame).
//! - `ui.resize {width, height}`: the window's inner size in points.

use std::sync::mpsc::{Receiver, Sender, channel};

use base64::Engine as _;
use egui::{Event, Key, Modifiers, PointerButton, Pos2, pos2, vec2};
use serde_json::{Value, json};

use crate::MaterialCraftApp;

pub struct ControlRequest {
    pub method: String,
    pub params: Value,
    pub reply: Sender<Value>,
}

impl ControlRequest {
    pub fn new(method: String, params: Value) -> (ControlRequest, Receiver<Value>) {
        let (tx, rx) = channel();
        (ControlRequest { method, params, reply: tx }, rx)
    }
}

/// One frame's worth of synthetic input (modifiers ride on each event).
#[derive(Clone, Debug, Default)]
pub struct Scripted {
    pub events: Vec<Event>,
}

fn ok(v: Value) -> Value {
    json!({ "ok": true, "result": v })
}

fn err(e: impl std::fmt::Display) -> Value {
    json!({ "ok": false, "error": e.to_string() })
}

pub fn poll(app: &mut MaterialCraftApp, ctx: &egui::Context) {
    let Some(rx) = app.control.as_ref() else { return };
    let reqs: Vec<ControlRequest> = rx.try_iter().collect();
    for r in reqs {
        if r.method == "ui.screenshot" {
            // Captured from the UI pass (`request_screenshots`), where viewport commands land.
            app.pending_screenshots.push(r.reply);
            ctx.request_repaint();
            continue;
        }
        let reply = handle(app, ctx, &r.method, &r.params);
        let _ = r.reply.send(reply);
        ctx.request_repaint();
    }
}

/// Ask for one window capture answering every pending `ui.screenshot` (call from the UI pass).
pub fn request_screenshots(app: &mut MaterialCraftApp, ctx: &egui::Context) {
    if app.pending_screenshots.is_empty() {
        return;
    }
    let replies = std::mem::take(&mut app.pending_screenshots);
    let repaint = ctx.clone();
    let callback = egui::ScreenshotCallback::new(move |img| {
        let answer = encode_png(img.size[0] as u32, img.size[1] as u32, img.pixels.iter().flat_map(|c| c.to_array()).collect())
            .map(|(w, h, png)| ok(json!({ "width": w, "height": h, "png": base64::engine::general_purpose::STANDARD.encode(png) })))
            .unwrap_or_else(|| err("could not encode the screenshot"));
        for r in &replies {
            let _ = r.send(answer.clone());
        }
        repaint.request_repaint();
    });
    ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(callback));
    ctx.send_viewport_cmd(egui::ViewportCommand::RequestPaintWhileHidden);
    ctx.request_repaint();
}

pub(crate) fn encode_png(w: u32, h: u32, rgba: Vec<u8>) -> Option<(u32, u32, Vec<u8>)> {
    let img = image::RgbaImage::from_raw(w, h, rgba)?;
    let mut png = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).ok()?;
    Some((w, h, png))
}

/// Feed the next frame of scripted input to egui (from `eframe::App::raw_input_hook`).
pub fn feed_input(app: &mut MaterialCraftApp, raw: &mut egui::RawInput) {
    if let Some(step) = app.input_queue.pop_front() {
        raw.events.extend(step.events);
    }
}

fn point(v: &Value, key: &str) -> Option<Pos2> {
    let a = v.get(key)?.as_array()?;
    let (x, y) = (a.first()?.as_f64()? as f32, a.get(1)?.as_f64()? as f32);
    (x.is_finite() && y.is_finite()).then(|| pos2(x, y))
}

fn button(v: &Value) -> PointerButton {
    match v.get("button").and_then(Value::as_str) {
        Some("right" | "secondary") => PointerButton::Secondary,
        Some("middle") => PointerButton::Middle,
        _ => PointerButton::Primary,
    }
}

fn modifiers(v: &Value) -> Modifiers {
    let mut m = Modifiers::NONE;
    for k in v.get("modifiers").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str) {
        match k.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => {
                m.ctrl = true;
                m.command = true;
            }
            "shift" => m.shift = true,
            "alt" | "option" => m.alt = true,
            "cmd" | "command" | "meta" => {
                m.mac_cmd = cfg!(target_os = "macos");
                m.command = true;
            }
            _ => {}
        }
    }
    m
}

/// Frames of input for a list of event descriptions. Each press, release and move lands on its
/// own frame, so egui sees real clicks and drags.
pub fn parse_events(list: &[Value]) -> Result<Vec<Scripted>, String> {
    let mut frames = Vec::new();
    let frame = |events: Vec<Event>, _modifiers: Modifiers| Scripted { events };
    for (i, e) in list.iter().enumerate() {
        let ty = e.get("type").and_then(Value::as_str).ok_or(format!("event {i}: `type` is required"))?;
        let mods = modifiers(e);
        let need = |k: &str| point(e, k).ok_or(format!("event {i} ({ty}): `{k}` [x, y] is required"));
        match ty {
            "move" => frames.push(frame(vec![Event::PointerMoved(need("pos")?)], mods)),
            "down" | "up" => {
                let pos = need("pos")?;
                frames
                    .push(frame(vec![Event::PointerMoved(pos), Event::PointerButton { pos, button: button(e), pressed: ty == "down", modifiers: mods }], mods));
            }
            "click" | "doubleClick" => {
                let pos = need("pos")?;
                let b = button(e);
                let times = if ty == "click" { 1 } else { 2 };
                frames.push(frame(vec![Event::PointerMoved(pos)], mods));
                for _ in 0..times {
                    frames.push(frame(vec![Event::PointerButton { pos, button: b, pressed: true, modifiers: mods }], mods));
                    frames.push(frame(vec![Event::PointerButton { pos, button: b, pressed: false, modifiers: mods }], mods));
                }
            }
            "drag" => {
                let (from, to) = (need("from")?, need("to")?);
                let b = button(e);
                let steps = e.get("steps").and_then(Value::as_u64).unwrap_or(8).clamp(1, 120) as usize;
                frames.push(frame(vec![Event::PointerMoved(from)], mods));
                frames.push(frame(vec![Event::PointerButton { pos: from, button: b, pressed: true, modifiers: mods }], mods));
                for s in 1..=steps {
                    let t = s as f32 / steps as f32;
                    frames.push(frame(vec![Event::PointerMoved(from + (to - from) * t)], mods));
                }
                frames.push(frame(vec![Event::PointerButton { pos: to, button: b, pressed: false, modifiers: mods }], mods));
            }
            "scroll" => {
                let pos = need("pos")?;
                let d = point(e, "delta").unwrap_or(pos2(0.0, -120.0));
                frames.push(frame(
                    vec![
                        Event::PointerMoved(pos),
                        Event::MouseWheel {
                            unit: egui::MouseWheelUnit::Point,
                            delta: vec2(d.x, d.y),
                            phase: egui::TouchPhase::Move,
                            source: egui::MouseWheelSource::Unknown,
                            modifiers: mods,
                        },
                    ],
                    mods,
                ));
            }
            "key" => {
                let name = e.get("key").and_then(Value::as_str).ok_or(format!("event {i} (key): `key` is required"))?;
                let key = Key::from_name(name).ok_or(format!("event {i}: unknown key `{name}`"))?;
                frames.push(frame(vec![Event::Key { key, physical_key: Some(key), pressed: true, repeat: false, modifiers: mods }], mods));
                frames.push(frame(vec![Event::Key { key, physical_key: Some(key), pressed: false, repeat: false, modifiers: mods }], mods));
            }
            "text" => {
                let t = e.get("text").and_then(Value::as_str).ok_or(format!("event {i} (text): `text` is required"))?;
                frames.push(frame(vec![Event::Text(t.to_string())], mods));
            }
            other => return Err(format!("event {i}: unknown type `{other}`")),
        }
        if frames.len() > 10_000 {
            return Err("too many input events".into());
        }
    }
    Ok(frames)
}

/// Recursive JSON merge: objects merge key by key, anything else in `patch` replaces `dst`.
fn merge_json(dst: &mut Value, patch: &Value) {
    match (dst, patch) {
        (Value::Object(d), Value::Object(p)) => {
            for (k, v) in p {
                match d.get_mut(k) {
                    Some(slot) => merge_json(slot, v),
                    None => {
                        d.insert(k.clone(), v.clone());
                    }
                }
            }
        }
        (slot, v) => *slot = v.clone(),
    }
}

pub fn handle(app: &mut MaterialCraftApp, ctx: &egui::Context, method: &str, p: &Value) -> Value {
    match method {
        "ping" => ok(json!("pong")),
        "ui.resize" => {
            let size = |k: &str| p.get(k).and_then(Value::as_f64).filter(|v| v.is_finite()).map(|v| v.clamp(640.0, 7680.0) as f32);
            match (size("width"), size("height")) {
                (Some(w), Some(h)) => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(false));
                    ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(vec2(w, h)));
                    ok(Value::Null)
                }
                _ => err("`width` and `height` (points, 640..7680) are required"),
            }
        }
        "command.run" => {
            let Some(id) = p.get("id").and_then(Value::as_str) else { return err("`id` is required") };
            let params = p.get("params").cloned().unwrap_or(Value::Null);
            match crate::cook::guard("command request", || {
                materialcraft_engine::commands::execute_request(&mut app.session, id, &params).map_err(|error| error.to_string())
            }) {
                Ok(v) => {
                    if id == "view.previewSize" {
                        app.ui.prefs.preview_size = app.session.preview_size.unwrap_or(0);
                    }
                    app.close_subgraph_after(id);
                    ok(v)
                }
                Err(e) => err(e),
            }
        }
        "command.list" => ok(materialcraft_engine::commands::list(&app.session, p.get("filter").and_then(Value::as_str))),
        "session.inspect" => {
            app.session.cook_if_dirty();
            ok(materialcraft_engine::inspect::session(&app.session, None))
        }
        "ui.inspect" => {
            let rect = |r: egui::Rect| json!([r.min.x, r.min.y, r.width(), r.height()]);
            let target = crate::graph::editor_target::EditorTarget::resolve(app);
            let (view, nodes, ports) = (app.graph.view, &app.graph.rects, &app.graph.ports);
            ok(json!({
                "state": app.ui,
                "graphView": rect(view),
                "selection": app.session.selection,
                "inspectedNode": app.graph.inspected_node,
                "subgraph": match target {
                    crate::graph::editor_target::EditorTarget::Function => app.function_editor.as_ref()
                        .map(|editor| json!({"kind": "function", "graph": app.session.graph, "node": editor.node, "target": editor.command_target(), "selection": editor.selection, "inspectedNode": editor.inspected_node})),
                    crate::graph::editor_target::EditorTarget::Pattern => app.pattern_editor.as_ref()
                        .map(|editor| json!({"kind": "pattern", "graph": editor.graph, "node": editor.node.0, "selection": editor.selection, "inspectedNode": editor.inspected_node})),
                    crate::graph::editor_target::EditorTarget::Material => None,
                },
                "annotationSelection": app.session.annotation_selection,
                "frames": app.graph.frames.iter().map(|(id, bounds)| json!({"id": id, "rect": rect(*bounds)})).collect::<Vec<_>>(),
                "comments": app.graph.comments.iter().map(|(id, bounds)| json!({"id": id, "rect": rect(*bounds)})).collect::<Vec<_>>(),
                "nodes": nodes.iter().map(|(id, r)| json!({"id": id.0, "rect": rect(*r)})).collect::<Vec<_>>(),
                "ports": ports.iter().map(|port| json!({
                    "node": port.node.0,
                    "port": port.port,
                    "label": port.label,
                    "output": port.output,
                    "pos": [port.pos.x, port.pos.y],
                })).collect::<Vec<_>>(),
                "fps": app.fps,
                "uiMs": app.ui_ms,
                "cooking": app.cooker.busy(),
                "render3dMs": app.view3d.render_ms,
                "toasts": app.toasts.iter().map(|t| json!({"text": t.text, "error": t.error})).collect::<Vec<_>>(),
            }))
        }
        "ui.menu.list" => ok(crate::menus::list(app)),
        "ui.menu.invoke" => {
            let Some(id) = p.get("id").and_then(Value::as_str) else { return err("`id` is required") };
            if !crate::menus::implemented(id) {
                return err(format!("menu item `{id}` is not implemented"));
            }
            crate::menus::invoke(app, ctx, id);
            ok(Value::Null)
        }
        "ui.set" => {
            let mut cur = serde_json::to_value(&app.ui).unwrap_or(Value::Null);
            if p.is_object() {
                merge_json(&mut cur, p);
            }
            match serde_json::from_value::<crate::state::UiState>(cur) {
                Ok(mut ui) => {
                    ui.graph_camera = ui.graph_camera.sanitized();
                    ui.image_camera = ui.image_camera.sanitized();
                    ui.windows.unsaved = app.ui.windows.unsaved.clone();
                    app.ui = ui;
                    ok(Value::Null)
                }
                Err(e) => err(format!("invalid UI state: {e}")),
            }
        }
        "ui.input" => {
            let Some(list) = p.get("events").and_then(Value::as_array) else { return err("`events` (an array) is required") };
            match parse_events(list) {
                Ok(frames) => {
                    let n = frames.len();
                    app.input_queue.extend(frames);
                    ctx.request_repaint();
                    ok(json!({"frames": n}))
                }
                Err(e) => err(e),
            }
        }
        "view.screenshot" => {
            let shown = if p.get("view").and_then(Value::as_str) == Some("2d") { &app.view2d.shown } else { &app.view3d.shown };
            match shown.as_ref().and_then(|(w, h, px)| encode_png(*w, *h, px.clone())) {
                Some((w, h, png)) => ok(json!({ "width": w, "height": h, "png": base64::engine::general_purpose::STANDARD.encode(png) })),
                None => err("that view has not drawn anything yet"),
            }
        }
        other => err(format!("unknown method `{other}`")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn input_events_become_frames() {
        let list = vec![
            json!({"type": "click", "pos": [10, 20]}),
            json!({"type": "drag", "from": [0, 0], "to": [100, 0], "steps": 4}),
            json!({"type": "key", "key": "Space"}),
            json!({"type": "text", "text": "blur"}),
            json!({"type": "scroll", "pos": [5, 5], "delta": [0, -60]}),
        ];
        let f = parse_events(&list).unwrap();
        // click: move, press, release; drag: move, press, 4 moves, release; key: 2; text 1; scroll 1.
        assert_eq!(f.len(), 3 + 7 + 2 + 1 + 1);
        assert!(parse_events(&[json!({"type": "key", "key": "NoSuchKey"})]).is_err());
        assert!(parse_events(&[json!({"type": "click"})]).is_err());
        assert!(parse_events(&[json!({"pos": [1, 2]})]).is_err());
    }

    fn app() -> MaterialCraftApp {
        MaterialCraftApp::headless(materialcraft_engine::sample_session(), crate::state::UiState::default())
    }

    #[test]
    fn ui_set_merges_nested_objects_instead_of_resetting_siblings() {
        let mut app = app();
        let ctx = egui::Context::default();
        app.ui.prefs.ui_scale = 1.5;
        app.ui.prefs.autosave_minutes = 9;
        let r = handle(&mut app, &ctx, "ui.set", &json!({"prefs": {"thumbnails": false}}));
        assert_eq!(r["ok"], json!(true), "{r}");
        assert!(!app.ui.prefs.thumbnails);
        assert_eq!(app.ui.prefs.ui_scale, 1.5);
        assert_eq!(app.ui.prefs.autosave_minutes, 9);
    }

    #[test]
    fn ui_set_clamps_camera_zoom() {
        let mut app = app();
        let ctx = egui::Context::default();
        let r = handle(&mut app, &ctx, "ui.set", &json!({"graphCamera": {"zoom": 0.0}, "imageCamera": {"zoom": 0.0, "fit": false}}));
        assert_eq!(r["ok"], json!(true), "{r}");
        assert!(app.ui.graph_camera.zoom > 0.0);
        assert!(app.ui.image_camera.zoom > 0.0);
        let c = &app.ui.graph_camera;
        assert!((c.pan[0] + 100.0 / c.zoom).is_finite());
    }
}
