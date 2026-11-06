use super::GraphCamera;
use super::canvas::*;
use super::wire_geometry::{WIRE_HIT, bezier_at, bezier_distance, bezier_points, wire_at};
use crate::MaterialCraftApp;
use egui::{Ui, Vec2, pos2, vec2};
use materialcraft_engine::core::NodeId;
use serde_json::json;

use super::frames::Frames;

/// Three primitive nodes framed on screen
fn three_nodes() -> (Frames, [NodeId; 3]) {
    let mut s = materialcraft_engine::sample_session();
    materialcraft_engine::execute(&mut s, "file.new", &json!({"template": "empty", "size": 256})).unwrap();
    let ids = ["uniform", "uniform", "channels_shuffle"].iter().enumerate().map(|(i, def)| {
        let added = materialcraft_engine::execute(&mut s, "graph.node.add", &json!({"def": def, "at": [i as f32 * 300.0, 0.0]})).unwrap();
        NodeId(added["id"].as_u64().unwrap())
    });
    let ids: [NodeId; 3] = ids.collect::<Vec<_>>().try_into().unwrap();
    s.selection.clear();
    (Frames::new(s), ids)
}

#[test]
fn clicking_a_node_shows_it_in_the_2d_view() {
    let (mut f, [noise, checker, _]) = three_nodes();
    // Shown explicitly first (a double-click, an output port or "View in 2D view")
    f.app.run("view.2d", json!({"node": noise.0}));
    assert_eq!(f.shown(), Some(noise));
    let at = f.center(checker);
    f.click(at, egui::Modifiers::NONE);
    assert_eq!(f.app.session.selection, vec![checker]);
    assert_eq!(f.app.session.view2d.node, Some(checker));
    assert_eq!(f.shown(), Some(checker));
    // And back
    let at = f.center(noise);
    f.click(at, egui::Modifiers::NONE);
    assert_eq!(f.app.session.view2d.node, Some(noise));
    assert_eq!(f.shown(), Some(noise));
    // Ctrl+click adds a node to the selection and shows it too
    let at = f.center(checker);
    f.click(at, egui::Modifiers::CTRL);
    assert_eq!(f.app.session.selection, vec![noise, checker]);
    assert_eq!(f.shown(), Some(checker));
}

#[test]
fn ctrl_click_that_deselects_a_node_does_not_show_it() {
    let (mut f, [noise, checker, _]) = three_nodes();
    f.app.session.selection = vec![noise, checker];
    f.app.run("view.2d", json!({"node": checker.0}));
    let at = f.center(noise);
    f.click(at, egui::Modifiers::CTRL);
    assert_eq!(f.app.session.selection, vec![checker]);
    assert_eq!(f.shown(), Some(checker));
}

#[test]
fn dragging_a_node_keeps_the_2d_view() {
    let (mut f, [noise, checker, _]) = three_nodes();
    f.app.run("view.2d", json!({"node": noise.0}));
    let before = f.app.session.graph().unwrap().node(checker).unwrap().pos;
    let from = f.center(checker);
    let none = egui::Modifiers::NONE;
    f.run(vec![egui::Event::PointerMoved(from)], none);
    f.run(vec![egui::Event::PointerButton { pos: from, button: egui::PointerButton::Primary, pressed: true, modifiers: none }], none);
    for step in 1..=8 {
        f.run(vec![egui::Event::PointerMoved(from + vec2(5.0 * step as f32, 0.0))], none);
    }
    let to = from + vec2(40.0, 0.0);
    f.run(vec![egui::Event::PointerButton { pos: to, button: egui::PointerButton::Primary, pressed: false, modifiers: none }], none);
    let after = f.app.session.graph().unwrap().node(checker).unwrap().pos;
    let moved = 40.0 / f.app.ui.graph_camera.zoom;
    assert_eq!(after, materialcraft_engine::graph_grid::snap_position([before[0] + moved, before[1]], true));
    assert_eq!(f.app.session.selection, vec![checker]);
    assert_eq!(f.shown(), Some(noise));
}

#[test]
fn clicking_the_shown_node_keeps_its_port() {
    let (mut f, [_, _, split]) = three_nodes();
    f.app.run("view.2d", json!({"node": split.0, "port": 2}));
    let at = f.center(split);
    f.click(at, egui::Modifiers::NONE);
    assert_eq!(f.app.session.selection, vec![split]);
    assert_eq!((f.app.session.view2d.node, f.app.session.view2d.port), (Some(split), 2));
}

#[test]
fn clicked_wires_swap_with_x_and_delete_together() {
    let mut app = MaterialCraftApp::headless(materialcraft_engine::sample_session(), crate::UiState::default());
    app.run("file.new", json!({"template": "empty"}));
    let id = |v: Option<serde_json::Value>| v.and_then(|v| v["id"].as_u64()).unwrap();
    let a = id(app.run("graph.node.add", json!({"def": "uniform", "at": [0, -150]})));
    let b = id(app.run("graph.node.add", json!({"def": "uniform", "at": [0, 150]})));
    let blend = id(app.run("graph.node.add", json!({"def": "blend", "at": [450, 0]})));
    app.run("graph.connect", json!({"from": a, "to": blend, "toPort": "foreground"}));
    app.run("graph.connect", json!({"from": b, "to": blend, "toPort": "background"}));
    let ctx = egui::Context::default();
    let mut frame = |app: &mut MaterialCraftApp, mut events: Vec<egui::Event>, modifiers: egui::Modifiers| {
        events.insert(0, egui::Event::ModifiersChanged(modifiers));
        let input = egui::RawInput { events, ..Default::default() };
        ctx.run_ui(input, |ui| {
            app.step(&ui.ctx().clone());
            app.draw(ui);
        })
        .textures_delta
        .clear();
    };

    for _ in 0..3 {
        frame(&mut app, Vec::new(), egui::Modifiers::NONE);
    }
    let mid = |app: &MaterialCraftApp, port: &str| app.graph.wires.iter().find(|(w, _)| w.0.0 == blend && w.1 == port).map(|(_, c)| bezier_at(c, 0.5)).unwrap();
    let click =
        |app: &mut MaterialCraftApp, frame: &mut dyn FnMut(&mut MaterialCraftApp, Vec<egui::Event>, egui::Modifiers), port: &str, m: egui::Modifiers| {
            let p = mid(app, port);
            let button = |pressed| egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed, modifiers: m };
            frame(app, vec![egui::Event::PointerMoved(p)], m);
            frame(app, vec![button(true)], m);
            frame(app, vec![button(false)], m);
        };

    click(&mut app, &mut frame, "foreground", egui::Modifiers::NONE);
    click(&mut app, &mut frame, "background", egui::Modifiers::SHIFT);
    assert_eq!(app.graph.selected_wires.len(), 2, "{:?}", app.graph.selected_wires);
    let key = |k| egui::Event::Key { key: k, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE };
    let src = |app: &MaterialCraftApp, port: &str| app.session.graph().unwrap().input_of(NodeId(blend), port).unwrap().from.0;
    frame(&mut app, vec![key(egui::Key::X)], egui::Modifiers::NONE);
    assert_eq!((src(&app, "foreground"), src(&app, "background")), (b, a));
    // Still selected: X again swaps back
    frame(&mut app, vec![key(egui::Key::X)], egui::Modifiers::NONE);
    assert_eq!((src(&app, "foreground"), src(&app, "background")), (a, b));
    frame(&mut app, vec![key(egui::Key::Delete)], egui::Modifiers::NONE);
    assert!(app.session.graph().unwrap().connections.is_empty());
    assert!(app.graph.selected_wires.is_empty());
}

#[test]
fn node_search_shortcuts_respect_hover_and_keyboard_focus() {
    for key in [egui::Key::Space, egui::Key::Tab] {
        for (hovered, editing) in [(true, false), (false, false), (true, true)] {
            let ctx = egui::Context::default();
            let mut app = MaterialCraftApp::headless(materialcraft_engine::sample_session(), crate::UiState::default());
            let mut text = String::new();
            let mut draw = |ui: &mut Ui| {
                if editing {
                    ui.text_edit_singleline(&mut text).request_focus();
                }
                app.graph.keyboard_was_focused = ui.ctx().egui_wants_keyboard_input();
                show(&mut app, ui);
            };

            ctx.run_ui(egui::RawInput::default(), &mut draw).textures_delta.clear();
            let pointer = if hovered { pos2(200.0, 200.0) } else { pos2(-10.0, -10.0) };

            let input = egui::RawInput {
                events: vec![
                    egui::Event::PointerMoved(pointer),
                    egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE },
                ],
                ..Default::default()
            };

            ctx.run_ui(input, &mut draw).textures_delta.clear();
            assert_eq!(app.graph.search.is_some(), hovered && !editing, "{key:?}: hovered={hovered}, editing={editing}");
            if let Some(search) = &app.graph.search {
                let cam = GraphCamera { origin: app.graph.view.min, pan: Vec2::from(app.ui.graph_camera.pan), zoom: app.ui.graph_camera.zoom };
                assert_eq!(search.at, cam.to_graph(pointer));
                assert!(search.text.is_empty());
            }
        }
    }
}

#[test]
fn wires_are_hit_near_their_curve_only() {
    let a = (NodeId(1), "input".to_string());
    let b = (NodeId(2), "input".to_string());
    let wires = vec![(a.clone(), bezier_points(pos2(0.0, 0.0), pos2(200.0, 100.0))), (b.clone(), bezier_points(pos2(0.0, 300.0), pos2(200.0, 300.0)))];
    // The midpoint of a symmetric wire lies on the line between its ends
    let mid = bezier_at(&wires[0].1, 0.5);
    assert!((mid - pos2(100.0, 50.0)).length() < 1e-3, "{mid:?}");
    assert_eq!(wire_at(&wires, mid + vec2(0.0, 3.0)), Some(&a));
    assert_eq!(wire_at(&wires, pos2(100.0, 302.0)), Some(&b));
    assert_eq!(wire_at(&wires, pos2(100.0, 200.0)), None);
    assert_eq!(wire_at(&wires, mid + vec2(0.0, WIRE_HIT * 3.0)), None);
    // The ends are on the wire
    assert!(bezier_distance(&wires[0].1, pos2(0.0, 0.0)) < 1e-3);
    assert!(bezier_distance(&wires[0].1, pos2(200.0, 100.0)) < 1e-3);
}
