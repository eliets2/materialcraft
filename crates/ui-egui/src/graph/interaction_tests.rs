use egui::{Event, Modifiers, PointerButton, Pos2, vec2};
use materialcraft_engine::{core::NodeId, new_session};
use serde_json::json;

use super::frames::Frames;
use crate::{MaterialCraftApp, UiState, menus};

fn drag(frames: &mut Frames, from: Pos2, to: Pos2) {
    frames.run(vec![Event::PointerMoved(from)], Modifiers::NONE);
    frames.run(vec![Event::PointerButton { pos: from, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE }], Modifiers::NONE);
    for step in 1..=8 {
        frames.run(vec![Event::PointerMoved(from.lerp(to, step as f32 / 8.0))], Modifiers::NONE);
    }
    frames.run(vec![Event::PointerButton { pos: to, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE }], Modifiers::NONE);
    frames.run(Vec::new(), Modifiers::NONE);
}

#[test]
fn output_port_drags_branch_to_two_fresh_processors() {
    let mut frames = Frames::new(new_session());
    let mut ids = Vec::new();
    for (definition, position) in [("input_value", [0, 64]), ("value_processor", [320, 0]), ("value_processor", [320, 256])] {
        ids.push(NodeId(frames.app.run("graph.node.add", json!({"def": definition, "at": position})).unwrap()["id"].as_u64().unwrap()));
    }
    frames.app.ui.frame_all = true;
    frames.run(Vec::new(), Modifiers::NONE);
    for target in &ids[1..] {
        let source = frames.app.graph.ports.iter().find(|port| port.node == ids[0] && port.output).unwrap().pos;
        let destination = frames.app.graph.ports.iter().find(|port| port.node == *target && port.port == "value1").unwrap().pos;
        drag(&mut frames, source, destination);
    }
    let graph = frames.app.session.graph().unwrap();
    assert_eq!(graph.connections.len(), 2);
    assert!(graph.connections.iter().all(|wire| wire.from == ids[0]));
}

#[test]
fn placement_and_dragging_snap_by_default_and_can_be_disabled() {
    let mut frames = Frames::new(new_session());
    assert!(frames.app.ui.prefs.snap_to_grid);
    let node = NodeId(frames.app.run("graph.node.add", json!({"def": "uniform", "at": [19, -19]})).unwrap()["id"].as_u64().unwrap());
    assert_eq!(frames.app.session.graph().unwrap().node(node).unwrap().pos, [32.0, -32.0]);
    frames.app.ui.frame_all = true;
    frames.run(Vec::new(), Modifiers::NONE);

    for snapping in [true, false] {
        let before = frames.app.session.graph().unwrap().node(node).unwrap().pos;
        let from = frames.center(node);
        let zoom = frames.app.ui.graph_camera.zoom;
        drag(&mut frames, from, from + vec2(23.0, 7.0));
        let expected = materialcraft_engine::graph_grid::snap_position([before[0] + 23.0 / zoom, before[1] + 7.0 / zoom], snapping);
        let after = frames.app.session.graph().unwrap().node(node).unwrap().pos;
        assert!((after[0] - expected[0]).abs() < 0.001 && (after[1] - expected[1]).abs() < 0.001, "{after:?} != {expected:?}");
        menus::invoke(&mut frames.app, &frames.ctx, "ui.graph.snapToGrid");
    }
}

#[test]
fn pin_navigation_wraps_and_thumbnail_commands_keep_the_document() {
    let mut app = MaterialCraftApp::headless(new_session(), UiState::default());
    app.graph.view = egui::Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0));
    let context = egui::Context::default();
    for position in [[100, 200], [700, 400]] {
        app.run("graph.pin.add", json!({"at": position}));
    }
    let original = app.session.pkg.clone();
    menus::invoke(&mut app, &context, "ui.graph.nextPin");
    let first = app.ui.graph_camera.pan;
    menus::invoke(&mut app, &context, "ui.graph.nextPin");
    assert_ne!(first, app.ui.graph_camera.pan);
    menus::invoke(&mut app, &context, "ui.graph.nextPin");
    assert_eq!(first, app.ui.graph_camera.pan);
    menus::invoke(&mut app, &context, "ui.graph.previousPin");
    assert_ne!(first, app.ui.graph_camera.pan);

    menus::invoke(&mut app, &context, "ui.graph.clearThumbnails");
    assert!(app.graph.thumbnails_cleared);
    menus::invoke(&mut app, &context, "ui.graph.computeThumbnails");
    assert!(!app.graph.thumbnails_cleared);
    assert!(app.session.dirty);
    assert_eq!(original, app.session.pkg);
}

#[test]
fn frame_drag_selects_the_border_and_moves_comments_and_nodes_together() {
    let mut frames = Frames::new(new_session());
    let node = NodeId(frames.app.run("graph.node.add", json!({"def": "uniform", "at": [64, 64]})).unwrap()["id"].as_u64().unwrap());
    frames.app.run("graph.comment.add", json!({"at": [64, 220], "text": "Follow the frame"}));
    let id =
        frames.app.run("graph.frame.add", json!({"rect": [0, 0, 400, 320], "description": "Follow the frame description"})).unwrap()["id"].as_u64().unwrap();
    frames.app.ui.frame_all = true;
    frames.run(Vec::new(), Modifiers::NONE);
    assert!(frames.canvas_text.iter().any(|text| text == "Follow the frame description"));
    let from = frames.app.graph.frames.iter().find(|(candidate, _)| *candidate == id).unwrap().1.left_bottom() + vec2(12.0, -12.0);
    let original = frames.app.session.pkg.clone();
    let zoom = frames.app.ui.graph_camera.zoom;
    drag(&mut frames, from, from + vec2(64.0, 32.0) * zoom);
    let graph = frames.app.session.graph().unwrap();
    assert_eq!(graph.node(node).unwrap().pos, [128.0, 96.0]);
    assert_eq!(graph.comments[0].pos, [128.0, 252.0]);
    assert_eq!(frames.app.session.annotation_selection, Some(materialcraft_engine::AnnotationSelection::Frame(id)));
    frames.app.session.undo().unwrap();
    assert_eq!(frames.app.session.pkg, original);

    frames.run(Vec::new(), Modifiers::NONE);
    let comment = frames.app.graph.comments[0].1.center();
    frames.click(comment, Modifiers::NONE);
    assert!(matches!(frames.app.session.annotation_selection, Some(materialcraft_engine::AnnotationSelection::Comment(_))));
    assert_eq!(crate::properties::title(&frames.app), "Comment - Properties");
}

#[test]
fn standalone_function_canvas_wires_constants_and_marks_an_output() {
    let mut session = new_session();
    materialcraft_engine::execute(&mut session, "file.new", &json!({"kind": "function"})).unwrap();
    let mut function = materialcraft_engine::core::FunctionGraph::default();
    let first = function.add_constant(materialcraft_engine::core::Value::Float(2.0));
    let second = function.add_constant(materialcraft_engine::core::Value::Float(3.0));
    let multiply = function.add("mul", &[], None);
    function.nodes[1].pos = [0.0, 128.0];
    function.nodes[2].pos = [256.0, 64.0];
    function.output = Some(multiply);
    materialcraft_engine::execute(&mut session, "graph.function.set", &json!({"function": function})).unwrap();
    let mut frames = Frames::new(session);

    for (source, slot) in [(first, "0"), (second, "1")] {
        let from = frames.app.graph.ports.iter().find(|port| port.node.0 == u64::from(source) && port.output).unwrap().pos;
        let to = frames.app.graph.ports.iter().find(|port| port.node.0 == u64::from(multiply) && port.port == slot).unwrap().pos;
        drag(&mut frames, from, to);
    }
    let function = frames.app.session.graph().unwrap().function.as_ref().unwrap();
    assert_eq!(function.node(multiply).unwrap().inputs, vec![Some(first), Some(second)]);
    assert_eq!(function.output, Some(multiply));
}

#[test]
fn selected_pattern_subgraph_opens_with_control_e() {
    let mut frames = Frames::new(new_session());
    let node = NodeId(frames.app.run("graph.node.add", json!({"def": "fx_map"})).unwrap()["id"].as_u64().unwrap());
    frames.app.run("graph.select", json!({"ids": [node.0]}));
    frames.run(Vec::new(), Modifiers::NONE);
    let original = frames.app.session.pkg.clone();
    let history = frames.app.session.history();

    let control = Modifiers::CTRL | Modifiers::COMMAND;
    frames.run(vec![Event::Key { key: egui::Key::E, physical_key: Some(egui::Key::E), pressed: true, repeat: false, modifiers: control }], control);
    frames.run(vec![Event::Key { key: egui::Key::E, physical_key: Some(egui::Key::E), pressed: false, repeat: false, modifiers: control }], control);
    assert_eq!(frames.app.pattern_editor.as_ref().map(|editor| editor.node), Some(node));
    assert_eq!(frames.app.session.pkg, original);
    assert_eq!(frames.app.session.history(), history);
    assert_eq!(frames.app.graph.rects.len(), 1);
    assert_eq!(frames.app.graph.ports.len(), 5);
    assert!(frames.canvas_text.iter().any(|label| label == "Quadrant"));

    menus::invoke(&mut frames.app, &frames.ctx, "ui.graph.parent");
    assert!(frames.app.pattern_editor.is_none());
    let response = crate::control::handle(&mut frames.app, &frames.ctx, "ui.menu.invoke", &json!({"id": "graph.node.edit"}));
    assert_eq!(response["ok"], true);
    assert_eq!(frames.app.pattern_editor.as_ref().map(|editor| editor.node), Some(node));

    frames.app.pattern_editor = None;
    frames.app.run("graph.node.add", json!({"def": "uniform"}));
    assert!(!menus::enabled(&frames.app, "graph.node.edit"));
    menus::invoke(&mut frames.app, &frames.ctx, "graph.node.edit");
    assert!(frames.app.pattern_editor.is_none());
}

#[test]
fn pattern_canvas_wires_moves_and_deletes_only_embedded_nodes() {
    use materialcraft_engine::core::{PatternGraph, PatternNodeKind, Value};

    let mut frames = Frames::new(new_session());
    let owner = NodeId(frames.app.run("graph.node.add", json!({"def": "fx_map"})).unwrap()["id"].as_u64().unwrap());
    let mut pattern = PatternGraph::default();
    let root = pattern.add(PatternNodeKind::Quadrant).unwrap();
    let child = pattern.add(PatternNodeKind::Switch).unwrap();
    let leaf = pattern.add(PatternNodeKind::Iterate).unwrap();
    pattern.nodes[0].position = [0.0, 0.0];
    pattern.nodes[1].position = [0.0, 192.0];
    pattern.nodes[2].position = [192.0, 384.0];
    frames.app.run("graph.pattern.set", json!({"id": owner.0, "pattern": pattern})).unwrap();
    let parent_camera = frames.app.ui.graph_camera;
    menus::invoke(&mut frames.app, &frames.ctx, "graph.node.edit");
    frames.run(Vec::new(), Modifiers::NONE);
    assert_eq!(frames.app.graph.rects.len(), 3);
    assert_eq!(frames.app.graph.ports.len(), 11);
    assert!(!menus::enabled(&frames.app, "graph.node.edit"));

    for (source, slot, target) in [(root, "output0", child), (root, "output1", leaf), (child, "output0", leaf)] {
        let from = frames.app.graph.ports.iter().find(|port| port.node.0 == u64::from(source) && port.port == slot).unwrap().pos;
        let to = frames.app.graph.ports.iter().find(|port| port.node.0 == u64::from(target) && !port.output).unwrap().pos;
        drag(&mut frames, from, to);
    }
    let current = |frames: &Frames| {
        let Value::PatternGraph(graph) = &frames.app.session.graph().unwrap().node(owner).unwrap().params["pattern"] else { panic!("missing pattern") };
        (**graph).clone()
    };
    assert_eq!(current(&frames).nodes[0].children, vec![Some(child), Some(leaf), None, None]);
    assert_eq!(current(&frames).nodes[1].children, vec![Some(leaf), None]);
    let before_cycle = frames.app.session.pkg.clone();
    let history = frames.app.session.history();
    let from = frames.app.graph.ports.iter().find(|port| port.node.0 == u64::from(leaf) && port.output).unwrap().pos;
    let to = frames.app.graph.ports.iter().find(|port| port.node.0 == u64::from(root) && !port.output).unwrap().pos;
    drag(&mut frames, from, to);
    assert_eq!(frames.app.session.pkg, before_cycle);
    assert_eq!(frames.app.session.history(), history);
    assert!(frames.app.status.contains("cycle"));

    let from = frames.center(NodeId(u64::from(child)));
    drag(&mut frames, from, from + vec2(35.0, 12.0));
    let moved = current(&frames).nodes[1].position;
    assert_ne!(moved, [0.0, 192.0]);
    assert!(moved.iter().all(|coordinate| coordinate % 32.0 == 0.0));
    assert_eq!(crate::properties::title(&frames.app), "Switch - Properties");
    frames.app.session.undo().unwrap();
    assert_eq!(frames.app.session.pkg, before_cycle);

    frames.app.graph.selected_wires.push((owner, "parent-wire".into()));
    frames.run(vec![Event::Key { key: egui::Key::Delete, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }], Modifiers::NONE);
    frames.run(vec![Event::Key { key: egui::Key::Delete, physical_key: None, pressed: false, repeat: false, modifiers: Modifiers::NONE }], Modifiers::NONE);
    assert_eq!(current(&frames).nodes.len(), 2);
    assert_eq!(current(&frames).nodes[0].children[0], None);
    assert_eq!(frames.app.session.graph().unwrap().nodes.len(), 1);
    frames.app.session.undo().unwrap();
    assert_eq!(frames.app.session.pkg, before_cycle);
    for command in ["edit.selectAll", "edit.copy", "edit.paste"] {
        menus::invoke(&mut frames.app, &frames.ctx, command);
    }
    let pasted = current(&frames);
    assert_eq!(pasted.nodes.len(), 6);
    assert_eq!(pasted.nodes[3].children, vec![Some(pasted.nodes[4].id), Some(pasted.nodes[5].id), None, None]);
    assert_eq!(pasted.nodes[4].children, vec![Some(pasted.nodes[5].id), None]);
    frames.app.session.undo().unwrap();
    assert_eq!(frames.app.session.pkg, before_cycle);
    menus::invoke(&mut frames.app, &frames.ctx, "ui.graph.parent");
    frames.run(Vec::new(), Modifiers::NONE);
    assert!(frames.app.pattern_editor.is_none());
    assert_eq!(frames.app.ui.graph_camera, parent_camera);
    assert_eq!(frames.app.graph.rects.len(), 1);
}

#[test]
fn replacing_the_document_closes_the_embedded_graph() {
    let mut frames = Frames::new(new_session());
    frames.app.run("graph.node.add", json!({"def": "fx_map"})).unwrap();
    menus::invoke(&mut frames.app, &frames.ctx, "graph.node.edit");
    assert!(frames.app.pattern_editor.is_some());
    let response = crate::control::handle(&mut frames.app, &frames.ctx, "command.run", &json!({"id": "file.new", "params": {}}));
    assert_eq!(response["ok"], true);
    assert!(frames.app.pattern_editor.is_none());
}

#[test]
fn pattern_dot_has_vertical_ports_and_can_be_wired_and_moved() {
    use materialcraft_engine::core::{PatternGraph, PatternNodeKind, Value};

    let mut frames = Frames::new(new_session());
    let owner = NodeId(frames.app.run("graph.node.add", json!({"def": "fx_map"})).unwrap()["id"].as_u64().unwrap());
    let mut pattern = PatternGraph::default();
    let dot = pattern.add(PatternNodeKind::Dot).unwrap();
    let quadrant = pattern.add(PatternNodeKind::Quadrant).unwrap();
    pattern.nodes[0].position = [48.0, 0.0];
    pattern.nodes[1].position = [0.0, 192.0];
    frames.app.run("graph.pattern.set", json!({"id": owner.0, "pattern": pattern})).unwrap();
    menus::invoke(&mut frames.app, &frames.ctx, "graph.node.edit");
    frames.run(Vec::new(), Modifiers::NONE);

    let input = frames.app.graph.ports.iter().find(|port| port.node.0 == u64::from(dot) && !port.output).unwrap().pos;
    let output = frames.app.graph.ports.iter().find(|port| port.node.0 == u64::from(dot) && port.output).unwrap().pos;
    assert_eq!(input.x, output.x);
    assert!(input.y < output.y);
    let destination = frames.app.graph.ports.iter().find(|port| port.node.0 == u64::from(quadrant) && !port.output).unwrap().pos;
    drag(&mut frames, output, destination);
    let from = frames.center(NodeId(u64::from(dot)));
    drag(&mut frames, from, from + vec2(32.0, 16.0));
    let Value::PatternGraph(graph) = &frames.app.session.graph().unwrap().node(owner).unwrap().params["pattern"] else { panic!("missing pattern") };
    assert_eq!(graph.nodes[0].children, vec![Some(quadrant)]);
    assert_ne!(graph.nodes[0].position, [48.0, 0.0]);
    assert_eq!(crate::properties::title(&frames.app), "Dot - Properties");
}

#[test]
fn pin_labels_follow_node_hover_while_pins_stay_visible() {
    use materialcraft_engine::core::FunctionGraph;

    let mut frames = Frames::new(new_session());
    let owner = NodeId(frames.app.run("graph.node.add", json!({"def": "fx_map"})).unwrap()["id"].as_u64().unwrap());
    frames.app.ui.frame_all = true;
    frames.run(vec![Event::PointerGone], Modifiers::NONE);
    let labels = ["Background", "Input image 0", "Output"];
    assert!(labels.iter().all(|label| !frames.canvas_text.iter().any(|text| text == label)), "{:?}", frames.canvas_text);
    assert_eq!(frames.app.graph.ports.len(), 3);

    let position = frames.center(owner);
    frames.run(vec![Event::PointerMoved(position)], Modifiers::NONE);
    assert!(labels.iter().all(|label| frames.canvas_text.iter().any(|text| text == label)), "{:?}", frames.canvas_text);
    frames.run(vec![Event::PointerGone], Modifiers::NONE);
    assert!(labels.iter().all(|label| !frames.canvas_text.iter().any(|text| text == label)), "{:?}", frames.canvas_text);
    assert_eq!(frames.app.graph.ports.len(), 3);

    menus::invoke(&mut frames.app, &frames.ctx, "graph.node.edit");
    frames.run(vec![Event::PointerGone], Modifiers::NONE);
    let labels = ["Input", "Top left", "Top right", "Bottom left", "Bottom right"];
    assert!(labels.iter().all(|label| !frames.canvas_text.iter().any(|text| text == label)), "{:?}", frames.canvas_text);
    assert_eq!(frames.app.graph.ports.len(), 5);

    let position = frames.app.graph.rects[0].1.center();
    frames.run(vec![Event::PointerMoved(position)], Modifiers::NONE);
    assert!(labels.iter().all(|label| !frames.canvas_text.iter().any(|text| text == label)), "{:?}", frames.canvas_text);
    frames.run(vec![Event::PointerGone], Modifiers::NONE);
    assert!(labels.iter().all(|label| !frames.canvas_text.iter().any(|text| text == label)), "{:?}", frames.canvas_text);
    assert_eq!(frames.app.graph.ports.len(), 5);

    frames.app.run("file.new", json!({"kind": "function"}));
    let mut function = FunctionGraph::default();
    let multiply = function.add("mul", &[], None);
    frames.app.run("graph.function.set", json!({"function": function}));
    frames.run(vec![Event::PointerGone], Modifiers::NONE);
    let labels = ["A", "Output"];
    assert!(labels.iter().all(|label| !frames.canvas_text.iter().any(|text| text == label)), "{:?}", frames.canvas_text);
    assert_eq!(frames.app.graph.ports.len(), 2);

    let position = frames.center(NodeId(u64::from(multiply)));
    frames.run(vec![Event::PointerMoved(position)], Modifiers::NONE);
    assert!(labels.iter().all(|label| frames.canvas_text.iter().any(|text| text == label)), "{:?}", frames.canvas_text);
    frames.run(vec![Event::PointerGone], Modifiers::NONE);
    assert!(labels.iter().all(|label| !frames.canvas_text.iter().any(|text| text == label)), "{:?}", frames.canvas_text);
    assert_eq!(frames.app.graph.ports.len(), 2);
}

#[test]
fn function_dots_wire_move_and_save_with_regular_card_geometry() {
    use materialcraft_engine::core::{FunctionGraph, Value};
    use materialcraft_engine::formats::sbs::{read_sbs, write_sbs};

    let mut frames = Frames::new(new_session());
    frames.app.run("file.new", json!({"kind": "function", "name": "Dot_sample"}));
    let mut function = FunctionGraph::default();
    let source = function.add_constant(Value::Float(3.0));
    let dot = function.add("passthrough", &[], None);
    let multiply = function.add("mul", &[], None);
    function.nodes[1].pos = [224.0, 64.0];
    function.nodes[2].pos = [416.0, 0.0];
    function.output = Some(multiply);
    frames.app.run("graph.function.set", json!({"function": function}));
    frames.run(Vec::new(), Modifiers::NONE);
    let zoom = frames.app.ui.graph_camera.zoom;
    let dot_bounds = frames.app.graph.rects.iter().find(|(id, _)| id.0 == u64::from(dot)).unwrap().1;
    let card = frames.app.graph.rects.iter().find(|(id, _)| id.0 == u64::from(multiply)).unwrap().1;
    assert!((dot_bounds.width() / zoom - 14.0).abs() < 0.01);
    assert!((card.width() / zoom - 100.0).abs() < 0.01);
    assert!((card.height() / zoom - 132.0).abs() < 0.01);
    assert!(!frames.canvas_text.iter().any(|text| text.contains("Reroute")));
    let dot_ports: Vec<_> = frames.app.graph.ports.iter().filter(|port| port.node.0 == u64::from(dot)).collect();
    assert_eq!(dot_ports.len(), 2);
    assert!((dot_ports[0].pos.y - dot_ports[1].pos.y).abs() < 0.01);
    assert!(dot_ports[0].pos.x < dot_ports[1].pos.x);
    assert!(!frames.app.graph.ports.iter().any(|port| port.node.0 == u64::from(multiply) && port.port == "1"));

    for (from, to, input) in [(source, dot, "0"), (dot, multiply, "0"), (dot, multiply, "1")] {
        let start = frames.app.graph.ports.iter().find(|port| port.node.0 == u64::from(from) && port.output).unwrap().pos;
        let end = frames.app.graph.ports.iter().find(|port| port.node.0 == u64::from(to) && port.port == input).unwrap().pos;
        drag(&mut frames, start, end);
    }
    let graph = frames.app.session.graph().unwrap().function.as_ref().unwrap();
    assert_eq!(graph.node(dot).unwrap().inputs, vec![Some(source)]);
    assert_eq!(graph.node(multiply).unwrap().inputs, vec![Some(dot), Some(dot)]);
    let before_move = frames.app.session.pkg.clone();
    let center = frames.center(NodeId(u64::from(dot)));
    drag(&mut frames, center, center + vec2(64.0, 32.0) * zoom);
    let graph = frames.app.session.graph().unwrap().function.as_ref().unwrap();
    assert_eq!(graph.node(dot).unwrap().pos, [288.0, 96.0]);
    assert_eq!(graph.node(multiply).unwrap().inputs, vec![Some(dot), Some(dot)]);

    let bytes = write_sbs(&frames.app.session.pkg).unwrap();
    assert!(std::str::from_utf8(&bytes).unwrap().contains("<function v=\"passthrough\"/>"));
    let read = read_sbs(&bytes, "dot_sample.sbs", &|_| None).unwrap();
    assert!(read.warnings.is_empty(), "{:?}", read.warnings);
    let saved = read.pkg.graphs.iter().find(|graph| graph.name == "Dot_sample").unwrap().function.as_ref().unwrap();
    let saved_dot = saved.nodes.iter().find(|node| node.op == "passthrough").unwrap();
    assert_eq!(saved_dot.pos, [288.0, 96.0]);
    let saved_multiply = saved.nodes.iter().find(|node| node.op == "mul").unwrap();
    assert_eq!(saved_multiply.inputs, vec![Some(saved_dot.id), Some(saved_dot.id)]);
    frames.app.session.undo().unwrap();
    assert_eq!(frames.app.session.pkg, before_move);
}

#[test]
fn regular_inputs_expand_on_connection_and_keep_saved_later_connections_visible() {
    let mut frames = Frames::new(new_session());
    let source = frames.app.run("graph.node.add", json!({"def": "uniform", "at": [0, 0]})).unwrap()["id"].as_u64().unwrap();
    let target = frames.app.run("graph.node.add", json!({"def": "blend", "at": [320, 0]})).unwrap()["id"].as_u64().unwrap();
    frames.app.ui.frame_all = true;
    frames.run(Vec::new(), Modifiers::NONE);
    let visible =
        |frames: &Frames| frames.app.graph.ports.iter().filter(|port| port.node.0 == target && !port.output).map(|port| port.port.clone()).collect::<Vec<_>>();
    let inputs = frames.app.session.input_ports(NodeId(target));
    assert_eq!(visible(&frames), inputs[..1]);
    for input in &inputs {
        frames.app.run("graph.connect", json!({"from": source, "to": target, "toPort": input})).unwrap();
        frames.run(Vec::new(), Modifiers::NONE);
    }
    assert_eq!(visible(&frames), inputs);
    frames.app.run("graph.disconnect", json!({"to": target, "toPort": inputs[0]})).unwrap();
    frames.run(Vec::new(), Modifiers::NONE);
    assert_eq!(visible(&frames), inputs);
    for input in &inputs[1..] {
        frames.app.run("graph.disconnect", json!({"to": target, "toPort": input})).unwrap();
    }
    frames.run(Vec::new(), Modifiers::NONE);
    assert_eq!(visible(&frames), inputs[..1]);
}

#[test]
fn every_canvas_keeps_properties_after_deselection() {
    use materialcraft_engine::core::{FunctionGraph, Value};

    for kind in ["graph", "function", "pattern"] {
        let mut frames = Frames::new(new_session());
        let (id, title) = match kind {
            "function" => {
                frames.app.run("file.new", json!({"kind": "function"})).unwrap();
                let mut graph = FunctionGraph::default();
                let source = graph.add_constant(Value::Float(2.0));
                let id = graph.add("mul", &[source, source], None);
                graph.nodes[1].pos = [256.0, 0.0];
                graph.output = Some(id);
                frames.app.run("graph.function.set", json!({"function": graph})).unwrap();
                (NodeId(u64::from(id)), "Multiply - Properties")
            }
            "pattern" => {
                let owner = NodeId(frames.app.run("graph.node.add", json!({"def": "fx_map"})).unwrap()["id"].as_u64().unwrap());
                frames.app.pattern_editor = Some(crate::pattern::PatternEditor::new(frames.app.session.graph, owner));
                let Value::PatternGraph(graph) = &frames.app.session.graph().unwrap().node(owner).unwrap().params["pattern"] else {
                    panic!("missing pattern graph");
                };

                (NodeId(u64::from(graph.root.unwrap())), "Quadrant - Properties")
            }
            _ => {
                let id = NodeId(frames.app.run("graph.node.add", json!({"def": "uniform"})).unwrap()["id"].as_u64().unwrap());
                (id, "Uniform Color - Properties")
            }
        };
        frames.app.ui.frame_all = true;
        frames.run(Vec::new(), Modifiers::NONE);
        frames.run(Vec::new(), Modifiers::NONE);
        let original = frames.app.session.pkg.clone();
        let center = frames.center(id);
        frames.click(center, Modifiers::NONE);
        assert_eq!(crate::properties::title(&frames.app), title, "{kind}");

        let blank = frames.app.graph.view.left_bottom() + vec2(12.0, -12.0);
        for deselect in [blank, center] {
            frames.click(center, Modifiers::NONE);
            let modifiers = if deselect == center { Modifiers::CTRL | Modifiers::COMMAND } else { Modifiers::NONE };
            frames.click(deselect, modifiers);
            frames.run(Vec::new(), Modifiers::NONE);

            let empty = match kind {
                "function" => frames.app.function_editor.as_ref().unwrap().selection.is_empty(),
                "pattern" => frames.app.pattern_editor.as_ref().unwrap().selection.is_empty(),
                _ => frames.app.session.selection.is_empty(),
            };
            assert!(empty, "{kind} remained selected");
            assert_eq!(crate::properties::title(&frames.app), title, "{kind}");
            assert_eq!(frames.app.session.pkg, original);
        }
    }
}

#[test]
fn box_selection_and_alt_panning_are_shared_by_every_canvas() {
    use materialcraft_engine::core::{FunctionGraph, PatternGraph, PatternNodeKind, Value};

    for kind in ["material", "function", "pattern"] {
        let mut frames = Frames::new(new_session());
        match kind {
            "function" => {
                frames.app.run("file.new", json!({"kind": "function"})).unwrap();
                let mut graph = FunctionGraph::default();
                for x in [0.0, 320.0] {
                    graph.add("get_float1", &[], Some(Value::Text("value".into())));
                    graph.nodes.last_mut().unwrap().pos = [x, 0.0];
                }
                frames.app.run("graph.function.set", json!({"function": graph})).unwrap();
            }
            "pattern" => {
                let owner = NodeId(frames.app.run("graph.node.add", json!({"def": "fx_map"})).unwrap()["id"].as_u64().unwrap());
                let mut graph = PatternGraph::default();
                for x in [0.0, 320.0] {
                    graph.add(PatternNodeKind::Quadrant).unwrap();
                    graph.nodes.last_mut().unwrap().position = [x, 0.0];
                }
                frames.app.run("graph.pattern.set", json!({"id": owner.0, "pattern": graph})).unwrap();
                frames.app.pattern_editor = Some(crate::pattern::PatternEditor::new(frames.app.session.graph, owner));
            }
            _ => {
                for x in [0, 320] {
                    frames.app.run("graph.node.add", json!({"def": "uniform", "at": [x, 0]})).unwrap();
                }
            }
        }
        if kind == "function" {
            frames.app.run("graph.frame.add", json!({"rect": [-256, 256, 240, 100]})).unwrap();
        }
        frames.app.ui.frame_all = true;
        frames.run(Vec::new(), Modifiers::NONE);
        frames.run(Vec::new(), Modifiers::NONE);
        let original = frames.app.session.pkg.clone();
        let bounds = frames.app.graph.rects.iter().fold(egui::Rect::NOTHING, |bounds, (_, rect)| bounds.union(*rect)).expand(12.0);
        drag(&mut frames, bounds.min, bounds.max);
        let selection_count = match kind {
            "function" => frames.app.function_editor.as_ref().unwrap().selection.len(),
            "pattern" => frames.app.pattern_editor.as_ref().unwrap().selection.len(),
            _ => frames.app.session.selection.len(),
        };
        assert_eq!(selection_count, 2, "{kind} box selection");

        let before = frames.app.graph.rects[0].1.center();
        let from = frames.app.graph.view.left_bottom() + vec2(20.0, -20.0);
        let to = from + vec2(30.0, -20.0);
        frames.run(vec![Event::PointerMoved(from)], Modifiers::ALT);
        frames.run(vec![Event::PointerButton { pos: from, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::ALT }], Modifiers::ALT);
        frames.run(vec![Event::PointerMoved(to)], Modifiers::ALT);
        frames.run(vec![Event::PointerButton { pos: to, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::ALT }], Modifiers::ALT);
        frames.run(Vec::new(), Modifiers::NONE);
        let after = frames.app.graph.rects[0].1.center();
        assert!((after - before - (to - from)).length() < 0.01, "{kind} pan: {before:?} -> {after:?}");
        assert_eq!(frames.app.session.pkg, original, "{kind} navigation edited the document");
    }
}
