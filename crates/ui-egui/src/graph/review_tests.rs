use crate::{
    MaterialCraftApp, UiState,
    function::{FunctionEditor, FunctionTarget},
    menus,
};
use materialcraft_engine::{
    core::{FunctionGraph, NodeId, PatternGraph, PatternNodeKind, Value},
    new_session,
};
use serde_json::json;

#[test]
fn embedded_function_commands_do_not_edit_the_parent_selection() {
    let mut app = MaterialCraftApp::headless(new_session(), UiState::default());
    let owner = NodeId(app.run("graph.node.add", json!({"def": "pixel_processor"})).unwrap()["id"].as_u64().unwrap());
    app.function_editor = Some(FunctionEditor::new(app.session.graph, owner, FunctionTarget::Parameter("function".into())));
    app.session.selection = vec![owner];
    let context = egui::Context::default();
    menus::invoke(&mut app, &context, "edit.selectAll");
    menus::invoke(&mut app, &context, "edit.delete");
    let node = app.session.graph().unwrap().node(owner).expect("Delete must stay in the Function editor");
    let Value::Function(function) = &node.params["function"] else { panic!("missing function") };
    assert!(function.nodes.is_empty());
}

#[test]
fn nested_function_commands_take_priority_over_fx_map_selection() {
    let mut app = MaterialCraftApp::headless(new_session(), UiState::default());
    let owner = NodeId(app.run("graph.node.add", json!({"def": "fx_map"})).unwrap()["id"].as_u64().unwrap());
    let mut graph = PatternGraph::with_root(PatternNodeKind::Quadrant);
    let node = graph.nodes[0].id;
    graph.nodes[0].parameters.insert("patternrotation".into(), Value::Function(Box::new(FunctionGraph::constant(Value::Float(0.0)))));
    app.run("graph.pattern.set", json!({"id": owner.0, "pattern": graph})).unwrap();
    app.pattern_editor = Some(crate::pattern::PatternEditor::new(app.session.graph, owner));
    let context = egui::Context::default();
    menus::invoke(&mut app, &context, "edit.selectAll");
    app.function_editor = Some(FunctionEditor::new(app.session.graph, owner, FunctionTarget::Pattern { node, parameter: "patternrotation".into() }));
    menus::invoke(&mut app, &context, "edit.selectAll");
    let inspected = crate::control::handle(&mut app, &context, "ui.inspect", &json!({}));
    assert_eq!(inspected["result"]["subgraph"]["kind"], "function");
    menus::invoke(&mut app, &context, "edit.delete");
    let Value::PatternGraph(graph) = &app.session.graph().unwrap().node(owner).unwrap().params["pattern"] else { panic!("missing pattern") };
    assert_eq!(graph.nodes.len(), 1);
    let Value::Function(function) = &graph.nodes[0].parameters["patternrotation"] else { panic!("missing function") };
    assert!(function.nodes.is_empty());
    menus::invoke(&mut app, &context, "ui.graph.parent");
    assert!(app.function_editor.is_none());
    assert_eq!(app.pattern_editor.as_ref().unwrap().selection, vec![node]);
    menus::invoke(&mut app, &context, "ui.graph.parent");
    assert!(app.pattern_editor.is_none());
    assert_eq!(app.session.selection, vec![owner]);
}

#[test]
fn graph_gestures_ignore_other_button_releases_and_escape_cancels() {
    use super::frames::Frames;
    use egui::{Event, Key, Modifiers, PointerButton, vec2};
    for kind in ["material", "function", "pattern"] {
        let mut frames = Frames::new(new_session());
        frames.app.ui.prefs.snap_to_grid = false;
        match kind {
            "function" => {
                frames.app.run("file.new", json!({"kind": "function"})).unwrap();
                let mut graph = FunctionGraph::default();
                graph.add("mul", &[], None);
                frames.app.run("graph.function.set", json!({"function": graph})).unwrap();
            }
            "pattern" => {
                let owner = NodeId(frames.app.run("graph.node.add", json!({"def": "fx_map"})).unwrap()["id"].as_u64().unwrap());
                frames.app.run("graph.pattern.set", json!({"id": owner.0, "pattern": PatternGraph::with_root(PatternNodeKind::Quadrant)})).unwrap();
                frames.app.pattern_editor = Some(crate::pattern::PatternEditor::new(frames.app.session.graph, owner));
            }
            _ => {
                frames.app.run("graph.node.add", json!({"def": "uniform"})).unwrap();
            }
        }
        frames.app.ui.frame_all = true;
        frames.run(Vec::new(), Modifiers::NONE);
        for cancel in [false, true] {
            frames.run(Vec::new(), Modifiers::NONE);
            let original = frames.app.session.pkg.clone();
            let from = frames.app.graph.rects[0].1.center();
            let middle = from + vec2(20.0, 0.0);
            let to = from + vec2(60.0, 0.0);
            let button = |pos, button, pressed| Event::PointerButton { pos, button, pressed, modifiers: Modifiers::NONE };
            frames.run(vec![Event::PointerMoved(from)], Modifiers::NONE);
            frames.run(vec![button(from, PointerButton::Primary, true)], Modifiers::NONE);
            frames.run(vec![Event::PointerMoved(middle)], Modifiers::NONE);
            for pressed in [true, false] {
                frames.run(vec![button(middle, PointerButton::Middle, pressed)], Modifiers::NONE);
            }
            assert_eq!(frames.app.session.pkg, original, "{kind}: unrelated release committed movement");
            frames.run(vec![Event::PointerMoved(to)], Modifiers::NONE);
            if cancel {
                frames
                    .run(vec![Event::Key { key: Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }], Modifiers::NONE);
            }
            frames.run(vec![button(to, PointerButton::Primary, false)], Modifiers::NONE);
            frames.run(Vec::new(), Modifiers::NONE);
            if cancel {
                assert_eq!(frames.app.session.pkg, original, "{kind}: cancelled movement committed");
            } else {
                assert!((frames.app.graph.rects[0].1.center().x - from.x - 60.0).abs() < 0.1, "{kind}: gesture ended early");
            }
        }
    }
}

#[test]
fn processors_edit_inline_and_return_to_the_parent_without_document_changes() {
    use super::{editor_target::EditorTarget, frames::Frames};
    for definition in ["pixel_processor", "value_processor"] {
        let mut frames = Frames::new(new_session());
        let owner = NodeId(frames.app.run("graph.node.add", json!({"def": definition})).unwrap()["id"].as_u64().unwrap());
        frames.run(Vec::new(), egui::Modifiers::NONE);
        let parent_camera = frames.app.ui.graph_camera;
        let original = frames.app.session.pkg.clone();
        let history = frames.app.session.history();
        menus::invoke(&mut frames.app, &frames.ctx, "graph.node.edit");
        for _ in 0..3 {
            frames.run(Vec::new(), egui::Modifiers::NONE);
        }
        assert_eq!(EditorTarget::resolve(&frames.app), EditorTarget::Function);
        let inspected = crate::control::handle(&mut frames.app, &frames.ctx, "ui.inspect", &json!({}));
        assert_eq!(inspected["result"]["graphView"][0], json!(frames.app.graph.view.min.x));
        assert_eq!(inspected["result"]["subgraph"]["kind"], "function");
        let point = frames.app.graph.view.left_bottom() + egui::vec2(20.0, -20.0);
        frames.click(point, egui::Modifiers::NONE);
        assert_eq!(EditorTarget::resolve(&frames.app), EditorTarget::Function);
        menus::invoke(&mut frames.app, &frames.ctx, "ui.graph.parent");
        frames.run(Vec::new(), egui::Modifiers::NONE);
        assert_eq!(EditorTarget::resolve(&frames.app), EditorTarget::Material);
        assert_eq!(frames.app.ui.graph_camera, parent_camera);
        assert_eq!(frames.app.session.selection, vec![owner]);
        assert_eq!(frames.app.session.pkg, original);
        assert_eq!(frames.app.session.history(), history);
    }
}

#[test]
fn instance_navigation_preserves_parent_view_and_material_across_renames() {
    use materialcraft_engine::core::Graph;
    let mut app = MaterialCraftApp::headless(new_session(), UiState::default());
    let owner = app.session.graph_mut().unwrap().add_node("graph_instance", [0.0; 2]);
    app.session.graph_mut().unwrap().node_mut(owner).unwrap().params.insert("graph".into(), Value::Text("Child".into()));
    app.session.pkg.graphs.push(Graph::new("Child"));
    app.session.selection = vec![owner];
    app.ui.graph_camera = crate::state::GraphCamera { pan: [123.0, -456.0], zoom: 1.5 };
    let camera = app.ui.graph_camera;
    let material = app.session.material_preview.clone();
    let context = egui::Context::default();
    menus::invoke(&mut app, &context, "graph.node.edit");
    assert_eq!(app.session.graph, 1);
    assert_eq!(app.session.material_preview, material);
    assert!(menus::enabled(&app, "ui.graph.parent"));
    app.run("graph.rename", json!({"graph": 0, "name": "Renamed parent"})).unwrap();
    assert!(super::subgraph_navigation::breadcrumb(&app).starts_with("Renamed parent / Child"));
    menus::invoke(&mut app, &context, "ui.graph.parent");
    assert_eq!(app.session.graph, 0);
    assert_eq!(app.session.selection, vec![owner]);
    assert_eq!(app.ui.graph_camera, camera);
    assert_eq!(app.session.material_preview, material);
    assert!(!menus::enabled(&app, "ui.graph.parent"));
    menus::invoke(&mut app, &context, "graph.node.edit");
    app.run("graph.open", json!({"graph": 0})).unwrap();
    assert!(!menus::enabled(&app, "ui.graph.parent"));
}

#[test]
fn dynamic_parameter_functions_return_to_the_owning_node() {
    let mut app = MaterialCraftApp::headless(new_session(), UiState::default());
    let owner = NodeId(app.run("graph.node.add", json!({"def": "blur"})).unwrap()["id"].as_u64().unwrap());
    app.run("graph.node.function", json!({"id": owner, "param": "intensity"})).unwrap();
    super::subgraph_navigation::open_function(&mut app, owner, FunctionTarget::Dynamic("intensity".into()));
    assert!(super::subgraph_navigation::breadcrumb(&app).ends_with("Blur / intensity"));
    let context = egui::Context::default();
    menus::invoke(&mut app, &context, "edit.selectAll");
    menus::invoke(&mut app, &context, "edit.delete");
    assert!(app.session.graph().unwrap().node(owner).unwrap().functions["intensity"].nodes.is_empty());
    menus::invoke(&mut app, &context, "ui.graph.parent");
    assert_eq!(app.session.selection, vec![owner]);
    assert!(app.function_editor.is_none());
}

#[test]
fn idle_ui_frames_preserve_explicit_automation_undo_groups() {
    use super::frames::Frames;
    use egui::{Event, Modifiers, PointerButton};
    let mut frames = Frames::new(new_session());
    frames.app.run("file.new", json!({"kind": "function"})).unwrap();
    let added = frames.app.run("graph.function.edit", json!({"edit": {"action": "add", "operation": "const_float1", "at": [0, 0]}})).unwrap();
    let id = added["id"].as_u64().unwrap();
    frames.run(Vec::new(), Modifiers::NONE);
    frames.app.session.reset_history();
    for value in [1.0, 2.0, 3.0] {
        frames.app.run("graph.function.edit", json!({"merge": true, "edit": {"action": "value", "id": id, "value": Value::Float(value)}})).unwrap();
        frames.run(Vec::new(), Modifiers::NONE);
    }
    assert_eq!(frames.app.session.history().len(), 1);
    frames.app.run("edit.finish", json!({})).unwrap();
    let position = frames.app.graph.view.left_bottom() + egui::vec2(20.0, -20.0);
    frames.run(
        vec![Event::PointerMoved(position), Event::PointerButton { pos: position, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE }],
        Modifiers::NONE,
    );
    frames.app.run("graph.function.edit", json!({"merge": true, "edit": {"action": "value", "id": id, "value": Value::Float(4.0)}})).unwrap();
    frames.run(vec![Event::PointerButton { pos: position, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE }], Modifiers::NONE);
    frames.app.run("graph.function.edit", json!({"merge": true, "edit": {"action": "value", "id": id, "value": Value::Float(5.0)}})).unwrap();
    assert_eq!(frames.app.session.history().len(), 3);
}

#[test]
#[ignore = "release canvas timing, run with --nocapture"]
fn canvas_frame_cost() {
    for count in [128, 4096] {
        for kind in ["function", "pattern"] {
            let mut app = MaterialCraftApp::headless(new_session(), UiState::default());
            if kind == "function" {
                app.run("file.new", json!({"kind": "function"})).unwrap();
                let mut graph = FunctionGraph::default();
                let mut previous = None;
                for index in 0..count {
                    let id = graph.add("passthrough", &previous.into_iter().collect::<Vec<_>>(), None);
                    graph.nodes.last_mut().unwrap().pos = [(index % 64) as f32 * 200.0, (index / 64) as f32 * 180.0];
                    previous = Some(id);
                }
                app.session.graph_mut().unwrap().function = Some(graph);
            } else {
                let owner = NodeId(app.run("graph.node.add", json!({"def": "fx_map"})).unwrap()["id"].as_u64().unwrap());
                let mut graph = PatternGraph::default();
                for index in 0..count {
                    graph.add(PatternNodeKind::Quadrant).unwrap();
                    let node = graph.nodes.last_mut().unwrap();
                    node.position = [(index % 64) as f32 * 200.0, (index / 64) as f32 * 180.0];
                    node.parameters.insert("rotation".into(), Value::Function(Box::new(FunctionGraph::constant(Value::Float(0.0)))));
                }
                app.session.graph_mut().unwrap().node_mut(owner).unwrap().params.insert("pattern".into(), Value::PatternGraph(Box::new(graph)));
                app.pattern_editor = Some(crate::pattern::PatternEditor::new(app.session.graph, owner));
            }
            let context = egui::Context::default();
            crate::theme::install(&context);
            let mut frame = || {
                let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 800.0))), ..Default::default() };
                let mut output = context.run_ui(input, |ui| super::show(&mut app, ui));
                output.textures_delta.clear();
                std::hint::black_box(output);
            };
            for _ in 0..3 {
                frame();
            }
            let mut samples = Vec::new();
            for _ in 0..3 {
                let start = std::time::Instant::now();
                for _ in 0..8 {
                    frame();
                }
                samples.push(start.elapsed().as_secs_f64() * 1000.0 / 8.0);
            }
            samples.sort_by(f64::total_cmp);
            println!("{kind} {count} nodes: {:.3} ms/frame", samples[1]);
        }
    }
}

#[test]
fn undo_closes_an_embedded_editor_when_its_owning_graph_changes() {
    let mut app = MaterialCraftApp::headless(new_session(), UiState::default());
    let owner = NodeId(app.run("graph.node.add", json!({"def": "pixel_processor"})).unwrap()["id"].as_u64().unwrap());
    app.run("graph.duplicate", json!({})).unwrap();
    app.session.reset_history();
    app.run("graph.open", json!({"graph": 0})).unwrap();
    app.run("graph.size", json!({"log2": [8, 8]})).unwrap();
    app.run("graph.open", json!({"graph": 1})).unwrap();
    app.function_editor = Some(FunctionEditor::new(app.session.graph, owner, FunctionTarget::Parameter("function".into())));
    let context = egui::Context::default();
    menus::invoke(&mut app, &context, "edit.selectAll");
    menus::invoke(&mut app, &context, "edit.undo");
    assert_eq!(app.session.graph, 0);
    assert!(app.function_editor.is_none());
    for graph in &app.session.pkg.graphs {
        let node = graph.node(owner).unwrap();
        assert!(!matches!(node.params.get("function"), Some(Value::Function(graph)) if graph.nodes.is_empty()));
    }
}

#[test]
fn rejected_duplicate_keeps_the_function_editor_selection() {
    let mut app = MaterialCraftApp::headless(new_session(), UiState::default());
    app.run("file.new", json!({"kind": "function"})).unwrap();
    let mut graph = FunctionGraph::default();
    for _ in 0..2049 {
        graph.add("passthrough", &[], None);
    }
    app.run("graph.function.set", json!({"function": graph})).unwrap();
    app.function_editor = Some(FunctionEditor::standalone(app.session.graph));
    let context = egui::Context::default();
    menus::invoke(&mut app, &context, "edit.selectAll");
    let before = app.function_editor.as_ref().unwrap().selection.clone();
    menus::invoke(&mut app, &context, "edit.duplicate");
    assert_eq!(app.function_editor.as_ref().unwrap().selection, before);
    assert_eq!(app.session.graph().unwrap().function.as_ref().unwrap().nodes.len(), 2049);
}

#[test]
fn properties_and_library_follow_the_active_embedded_function() {
    use super::{editor_target::EditorTarget, frames::Frames};
    let mut frames = Frames::new(new_session());
    let owner = NodeId(frames.app.run("graph.node.add", json!({"def": "pixel_processor"})).unwrap()["id"].as_u64().unwrap());
    let target = materialcraft_engine::FunctionGraphTarget::Parameter { node: owner, parameter: "function".into() };
    frames.app.run("graph.function.set", json!({"target": target, "function": FunctionGraph::constant(Value::Float(0.5))})).unwrap();
    frames.app.function_editor = Some(FunctionEditor::new(frames.app.session.graph, owner, FunctionTarget::Parameter("function".into())));
    menus::invoke(&mut frames.app, &frames.ctx, "edit.selectAll");
    assert_eq!(EditorTarget::resolve(&frames.app), EditorTarget::Function);
    let label = materialcraft_engine::nodes::function::definition("const_float1").unwrap().label;
    assert!(crate::properties::title(&frames.app).starts_with(label));
    frames.app.ui.library_search.clear();
    let mut output = frames
        .ctx
        .run_ui(egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 800.0))), ..Default::default() }, |ui| {
            crate::library::show(&mut frames.app, ui)
        });
    output.textures_delta.clear();
    assert!(output.shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.job.text.contains("Search functions"))));
    menus::invoke(&mut frames.app, &frames.ctx, "ui.graph.parent");
    assert_eq!(EditorTarget::resolve(&frames.app), EditorTarget::Material);
    assert!(crate::properties::title(&frames.app).starts_with("Pixel Processor"));
}

#[test]
fn standalone_function_creation_keeps_selection_after_the_next_frame() {
    use super::frames::Frames;
    let mut frames = Frames::new(new_session());
    frames.app.run("file.new", json!({"kind": "function"})).unwrap();
    frames.run(Vec::new(), egui::Modifiers::NONE);
    let mut editor = frames.app.function_editor.take().unwrap();
    editor.edit(&mut frames.app, materialcraft_engine::FunctionEdit::Add { operation: "const_float1".into(), at: [0.0; 2], snap: false }, false);
    let selected = editor.selection.clone();
    frames.app.function_editor = Some(editor);
    frames.run(Vec::new(), egui::Modifiers::NONE);
    assert!(!selected.is_empty());
    assert_eq!(frames.app.function_editor.as_ref().unwrap().selection, selected);
    menus::invoke(&mut frames.app, &frames.ctx, "edit.delete");
    menus::invoke(&mut frames.app, &frames.ctx, "edit.undo");
    frames.run(Vec::new(), egui::Modifiers::NONE);
    assert_eq!(frames.app.function_editor.as_ref().unwrap().selection, selected);
}
