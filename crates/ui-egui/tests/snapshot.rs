use std::time::Duration;

use egui::vec2;
use egui_kittest::{
    Harness,
    kittest::{By, NodeT, Queryable},
};
use materialcraft_ui_egui::{MaterialCraftApp, UiState};
use serde_json::json;

fn harness(app: MaterialCraftApp, w: f32, h: f32) -> Harness<'static, MaterialCraftApp> {
    Harness::builder().with_size(vec2(w, h)).with_pixels_per_point(1.0).wgpu().build_ui_state(
        |ui, app: &mut MaterialCraftApp| {
            app.step(&ui.ctx().clone());
            app.draw(ui);
        },
        app,
    )
}

/// Run frames until background work (cooks, previews) settles
fn settle(h: &mut Harness<'static, MaterialCraftApp>, frames: usize) {
    for _ in 0..frames {
        h.step();
        std::thread::sleep(Duration::from_millis(25));
    }
}

fn save(h: &mut Harness<'static, MaterialCraftApp>, dir: &std::path::Path, name: &str) {
    let img = h.render().expect("render");
    img.save(dir.join(format!("{name}.png"))).expect("save");
    println!("wrote {}", dir.join(format!("{name}.png")).display());
}

#[test]
#[ignore = "requires an offscreen GPU; run explicitly with --ignored"]
fn explorer_export_opens_the_clicked_graph() {
    let mut app = MaterialCraftApp::headless(materialcraft_engine::sample_session(), UiState::default());
    app.run("graph.duplicate", json!({"graph": 0}));
    app.session.pkg.graphs[1].name = "Export target".into();
    app.run("graph.open", json!({"graph": 0}));
    let mut h = harness(app, 1600.0, 960.0);
    settle(&mut h, 5);
    h.get_by_label("Export target").click_secondary();
    settle(&mut h, 3);
    h.get_all(By::new().label("Export outputs...")).last().unwrap().click();
    settle(&mut h, 3);
    assert_eq!(h.state().session.graph, 1);
    assert!(h.state().ui.windows.export);
    if let Some(dir) = std::env::var_os("MC_SNAPSHOT_DIR").map(std::path::PathBuf::from) {
        std::fs::create_dir_all(&dir).unwrap();
        save(&mut h, &dir, "explorer-export");
    }
}

/// The startup graph stays empty through real UI frames; templates are still available on demand
#[test]
#[ignore = "requires an offscreen GPU; run explicitly with --ignored"]
fn empty_startup_can_create_a_template() {
    let app = MaterialCraftApp::headless(materialcraft_engine::new_session(), UiState::default());
    let mut harness = harness(app, 1600.0, 960.0);
    settle(&mut harness, 40);
    assert_eq!(harness.state().session.pkg.graphs.len(), 1);
    assert!(harness.state().session.graph().unwrap().nodes.is_empty());
    assert!(harness.state().graph.rects.is_empty());
    if let Some(dir) = std::env::var_os("MC_SNAPSHOT_DIR").map(std::path::PathBuf::from) {
        std::fs::create_dir_all(&dir).unwrap();
        save(&mut harness, &dir, "empty-startup");
    }
    harness.get_by_label("New graph...").click();
    settle(&mut harness, 4);
    harness.get(By::new().role(egui::accesskit::Role::ComboBox).label("Template")).click();
    settle(&mut harness, 2);
    harness.get_by_label("Atomic sample").click();
    settle(&mut harness, 4);
    harness.get_by_label("Create").click();
    settle(&mut harness, 40);
    assert_eq!(harness.state().session.pkg.graphs.len(), 2);
    assert!(!harness.state().session.graph().unwrap().nodes.is_empty());
    assert!(!harness.state().graph.rects.is_empty());
    if let Some(dir) = std::env::var_os("MC_SNAPSHOT_DIR").map(std::path::PathBuf::from) {
        save(&mut harness, &dir, "chosen-template");
    }
}

#[test]
#[ignore]
fn snapshots() {
    let Some(dir) = std::env::var_os("MC_SNAPSHOT_DIR").map(std::path::PathBuf::from) else {
        eprintln!("set MC_SNAPSHOT_DIR");
        return;
    };
    std::fs::create_dir_all(&dir).unwrap();
    let app = MaterialCraftApp::headless(materialcraft_engine::sample_session(), UiState::default());
    let mut h = harness(app, 1600.0, 960.0);
    settle(&mut h, 40);
    // Select a node so the properties panel shows parameters
    let id = h.state().session.graph().unwrap().nodes[0].id;
    h.state_mut().run("graph.select", json!({"ids": [id.0]}));
    let preview = h.state().session.graph().unwrap().nodes.iter().find(|node| node.def == "blur").unwrap().id;
    h.state_mut().run("view.2d", json!({"node": preview.0, "histogram": true}));
    settle(&mut h, 30);
    save(&mut h, &dir, "workspace");
    h.state_mut().ui.windows.new_graph = true;
    settle(&mut h, 40);
    save(&mut h, &dir, "new-graph");
    h.state_mut().ui.windows.new_graph = false;

    // A cut-out material in the 3D view (an opacity output), a reroute dot pulled out of a wire and another wire selected; then the alpha channel view
    h.state_mut().run("file.new", json!({"template": "atomics", "size": 512}));
    let holes = h.state_mut().run("graph.node.add", json!({"def": "pixel_processor", "at": [0, -260]})).unwrap()["id"].as_u64().unwrap();
    let out = h.state_mut().run("graph.node.add", json!({"def": "output", "at": [420, -260], "connectFrom": {"node": holes}, "params": {"usage": 7}})).unwrap()
        ["id"]
        .as_u64()
        .unwrap();
    h.state_mut().run("graph.wire.insertDot", json!({"to": out, "toPort": "input", "at": [210, -330]}));
    let wire = h.state().session.graph().unwrap().connections.iter().find(|c| c.to.0 != out).map(|c| (c.to, c.to_port.clone()));
    h.state_mut().graph.selected_wires = wire.into_iter().collect();
    h.state_mut().run("view.3d", json!({"scene": "sphere"}));
    h.state_mut().ui.frame_all = true;
    settle(&mut h, 60);
    save(&mut h, &dir, "alpha-transparency");
    h.state_mut().run("view.3d", json!({"alphaView": true}));
    settle(&mut h, 40);
    save(&mut h, &dir, "alpha-channel");
    h.state_mut().run("view.3d", json!({"alphaView": false, "scene": "rounded_cube"}));

    // A pixel processor drawing rings, with its function graph open
    h.state_mut().run("file.new", json!({"template": "empty", "name": "Rings"}));
    let f = json!({"t": "function", "v": {"nodes": [
        {"id": 1, "op": "get_float2", "value": {"t": "text", "v": "$pos"}, "pos": [0, 0]},
        {"id": 2, "op": "const_float2", "value": {"t": "float2", "v": [0.5, 0.5]}, "pos": [0, 110]},
        {"id": 3, "op": "sub", "inputs": [1, 2], "pos": [170, 40]},
        {"id": 8, "op": "dot", "inputs": [3, 3], "pos": [340, 40]},
        {"id": 4, "op": "sqrt", "inputs": [8], "pos": [340, -80]},
        {"id": 5, "op": "const_float1", "value": {"t": "float", "v": 60.0}, "pos": [340, 130]},
        {"id": 6, "op": "mul", "inputs": [4, 5], "pos": [510, 60]},
        {"id": 7, "op": "sin", "inputs": [6], "pos": [680, 60]}
    ], "output": 7}});
    let id =
        h.state_mut().run("graph.node.add", json!({"def": "pixel_processor", "at": [0, 0], "params": {"color_mode": false}})).unwrap()["id"].as_u64().unwrap();
    h.state_mut().run("graph.node.param", json!({"id": id, "param": "function", "value": f}));
    let out =
        h.state_mut().run("graph.node.add", json!({"def": "output", "connectFrom": {"node": id}, "params": {"usage": 4}})).unwrap()["id"].as_u64().unwrap();
    let _ = out;
    h.state_mut().run("graph.select", json!({"ids": [id]}));
    h.state_mut().run("view.2d", json!({"node": id}));
    h.state_mut().function_editor = Some(materialcraft_ui_egui::function::FunctionEditor::new(
        h.state().session.graph,
        materialcraft_engine::core::NodeId(id),
        materialcraft_ui_egui::function::FunctionTarget::Parameter("function".into()),
    ));
    h.state_mut().ui.frame_all = true;
    settle(&mut h, 40);
    save(&mut h, &dir, "function-editor");
    h.state_mut().function_editor = None;

    h.state_mut().run("file.newAtomicSample", json!({})).unwrap();
    let input = h.state().session.graph().unwrap().nodes.iter().find(|node| node.def == "pixel_processor").unwrap().id;
    let id = h.state_mut().run("graph.node.add", json!({"def": "levels", "connectFrom": {"node": input.0}})).unwrap()["id"].as_u64().unwrap();
    h.state_mut().run("graph.select", json!({"ids": [id]})).unwrap();
    h.state_mut().run("graph.node.param", json!({"id": id, "param": "in_low", "value": 0.3})).unwrap();
    h.state_mut().run("graph.node.param", json!({"id": id, "param": "in_mid", "value": 0.35})).unwrap();
    h.state_mut().run("graph.node.param", json!({"id": id, "param": "in_high", "value": 0.85})).unwrap();
    h.state_mut().run("view.2d", json!({"node": id})).unwrap();
    h.state_mut().ui.frame_all = true;
    settle(&mut h, 40);
    save(&mut h, &dir, "levels-editor");

    h.state_mut().run("file.newAtomicSample", json!({}));
    h.state_mut().ui.frame_all = true;
    settle(&mut h, 60);
    save(&mut h, &dir, "atomic-sample");
    let pattern = h.state().session.graph().unwrap().nodes.iter().find(|node| node.def == "fx_map").unwrap().id;
    h.state_mut().pattern_editor = Some(materialcraft_ui_egui::pattern::PatternEditor::new(h.state().session.graph, pattern));
    settle(&mut h, 20);
    save(&mut h, &dir, "pattern-editor");
}

/// Steady-state UI build time per frame with each panel closed in turn (release): `cargo test -p materialcraft-ui-egui --release --test snapshot perf -- --ignored --nocapture`
#[test]
#[ignore]
fn perf() {
    use materialcraft_ui_egui::docking::Panel;
    let app = MaterialCraftApp::headless(materialcraft_engine::sample_session(), UiState::default());
    let mut h = harness(app, 1600.0, 960.0);
    settle(&mut h, 60);
    let measure = |h: &mut Harness<'static, MaterialCraftApp>| {
        let t = std::time::Instant::now();
        for _ in 0..30 {
            h.step();
        }
        t.elapsed().as_secs_f64() * 1000.0 / 30.0
    };
    println!("all panels: {:.2} ms/frame", measure(&mut h));
    for p in [Panel::Library, Panel::View3d, Panel::View2d, Panel::Properties, Panel::Explorer] {
        h.state_mut().ui.dock.close(p);
        settle(&mut h, 3);
        println!("without {p:?}: {:.2} ms/frame", measure(&mut h));
        h.state_mut().ui.dock.focus(p);
        settle(&mut h, 3);
    }
}

#[test]
#[ignore = "requires an offscreen GPU"]
fn graph_context_menu_has_flat_node_list() {
    let app = MaterialCraftApp::headless(materialcraft_engine::sample_session(), UiState::default());
    let mut harness = harness(app, 1600.0, 960.0);
    settle(&mut harness, 8);
    harness.get(By::new().role(egui::accesskit::Role::Button).label("Brightness")).click_secondary();
    settle(&mut harness, 3);
    let directory = std::env::var_os("MC_SNAPSHOT_DIR").map(std::path::PathBuf::from);
    if let Some(directory) = &directory {
        std::fs::create_dir_all(directory).unwrap();
        save(&mut harness, directory, "graph-context-menu");
    }

    for label in [
        "Add Comment",
        "Add Frame",
        "Add Navigation pin",
        "Add Dot node",
        "View in 3D view",
        "Reset and view in 3D view",
        "View output in 2D view",
        "Compute nodes thumbnails",
        "Clear nodes thumbnails",
        "Save package",
        "Paste without link",
        "Previous pin",
        "Next pin",
        "Delete and relink",
        "Duplicate selection without link",
        "Select upstream nodes",
        "Select downstream nodes",
        "Disable selection",
    ] {
        assert!(harness.query_all(By::new().label_contains(label)).next().is_some(), "missing {label}");
    }

    harness.get_by_label("Add node").click();
    settle(&mut harness, 3);
    harness.key_press(egui::Key::S);
    settle(&mut harness, 3);
    let button = |label| By::new().role(egui::accesskit::Role::Button).label(label);
    assert!(!harness.get_all(button("SVG")).last().unwrap().accesskit_node().is_disabled());
    assert!(!harness.get_all(button("Sharpen")).last().unwrap().accesskit_node().is_disabled());
    assert!(harness.get_all(button("Grayscale Conversion")).last().unwrap().accesskit_node().is_disabled());
    if let Some(directory) = &directory {
        save(&mut harness, directory, "flat-add-node-menu");
    }

    let before_clear = harness.state().session.pkg.clone();
    harness.key_press(egui::Key::Backspace);
    settle(&mut harness, 3);
    assert!(!harness.get_all(button("Grayscale Conversion")).last().unwrap().accesskit_node().is_disabled());
    assert_eq!(harness.state().session.pkg, before_clear);

    for _ in 0..2 {
        harness.key_press(egui::Key::Escape);
        settle(&mut harness, 2);
    }
    let graph = harness.state().session.graph().unwrap();
    let selected = graph.nodes.iter().find(|node| node.def == "fx_map").unwrap().id;
    let other = graph.nodes.iter().find(|node| node.def != "fx_map").unwrap().id;
    harness.state_mut().run("graph.select", json!({"ids": [selected.0, other.0]}));
    harness.state_mut().ui.frame_all = true;
    settle(&mut harness, 3);
    assert!(harness.query_all(By::new().label("Edit FX-Map graph")).next().is_none());
    let position = harness.state().graph.rects.iter().find(|(id, _)| *id == selected).unwrap().1.center();
    harness.event(egui::Event::PointerMoved(position));
    harness.step();
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton { pos: position, button: egui::PointerButton::Secondary, pressed, modifiers: egui::Modifiers::NONE });
        harness.step();
    }
    settle(&mut harness, 3);
    if let Some(directory) = &directory {
        save(&mut harness, directory, "pattern-node-context-menu");
    }
    assert!(
        harness.query_all(By::new().label_contains("Edit graph")).next().is_some(),
        "selection: {:?}, target: {selected:?}, position: {position:?}",
        harness.state().session.selection
    );
    assert_eq!(harness.state().session.selection.last(), Some(&selected));

    harness.get(By::new().role(egui::accesskit::Role::Button).label_contains("Edit graph")).click();
    settle(&mut harness, 3);
    assert_eq!(harness.state().pattern_editor.as_ref().map(|editor| editor.node), Some(selected));
    assert!(harness.query_all(By::new().role(egui::accesskit::Role::Window).label("FX-Map graph")).next().is_none());
    assert!(harness.query_all(By::new().role(egui::accesskit::Role::Canvas).label("FX-Map graph")).next().is_some());
    assert_eq!(harness.state().graph.rects.len(), 3);
    let mut graph = match &harness.state().session.graph().unwrap().node(selected).unwrap().params["pattern"] {
        materialcraft_engine::core::Value::PatternGraph(graph) => (**graph).clone(),
        _ => panic!("missing pattern"),
    };
    let dot = graph.add(materialcraft_engine::core::PatternNodeKind::Dot).unwrap();
    graph.nodes[0].children[1] = Some(dot);
    graph.nodes[3].position = [288.0, 240.0];
    graph.nodes[3].children[0] = Some(graph.nodes[2].id);
    let quadrant_id = graph.nodes[2].id;
    harness.state_mut().run("graph.pattern.set", json!({"id": selected.0, "pattern": graph})).unwrap();
    settle(&mut harness, 3);
    let quadrant = harness.state().graph.rects.iter().find(|(id, _)| id.0 == u64::from(quadrant_id)).unwrap().1.center();
    harness.event(egui::Event::PointerMoved(quadrant));
    harness.step();
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton { pos: quadrant, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE });
        harness.step();
    }
    settle(&mut harness, 3);
    assert!(harness.query_all(By::new().label("Pattern size")).next().is_some());
    if let Some(directory) = &directory {
        save(&mut harness, directory, "pattern-graph-editor");
    }
    harness.get(By::new().role(egui::accesskit::Role::Button).label("Return to parent graph")).click();
    settle(&mut harness, 3);
    assert!(harness.state().pattern_editor.is_none());

    harness.state_mut().run("file.new", json!({"template": "empty"})).unwrap();
    harness.state_mut().run("graph.node.add", json!({"def": "fx_map"})).unwrap();
    harness.state_mut().ui.frame_all = true;
    harness.event(egui::Event::PointerGone);
    settle(&mut harness, 3);
    assert_eq!(harness.state().graph.ports.len(), 3);
    if let Some(directory) = &directory {
        save(&mut harness, directory, "default-pattern-inputs");
    }
    let position = harness.state().graph.rects[0].1.center();
    harness.event(egui::Event::PointerMoved(position));
    settle(&mut harness, 3);
    if let Some(directory) = &directory {
        save(&mut harness, directory, "hovered-pattern-inputs");
    }

    harness.state_mut().ui.dock.focus(materialcraft_ui_egui::docking::Panel::Graph);
    harness.key_press_modifiers(egui::Modifiers::CTRL | egui::Modifiers::COMMAND, egui::Key::E);
    settle(&mut harness, 3);
    assert_eq!(harness.state().graph.rects.len(), 1);
    assert_eq!(harness.state().graph.ports.len(), 5);
    harness.event(egui::Event::PointerGone);
    settle(&mut harness, 3);
    assert!(harness.query_all(By::new().role(egui::accesskit::Role::Button).label("Quadrant")).next().is_some());
    if let Some(directory) = &directory {
        save(&mut harness, directory, "default-pattern-root");
    }
    let position = harness.state().graph.rects[0].1.center();
    harness.event(egui::Event::PointerMoved(position));
    settle(&mut harness, 3);
    if let Some(directory) = &directory {
        save(&mut harness, directory, "hovered-pattern-root");
    }

    harness.get_all(By::new().role(egui::accesskit::Role::Button).label("Quadrant")).last().unwrap().click();
    settle(&mut harness, 3);
    let blank = harness.state().graph.view.left_bottom() + vec2(12.0, -12.0);
    click_canvas(&mut harness, blank);
    if let Some(directory) = &directory {
        save(&mut harness, directory, "deselected-pattern-properties");
    }
    harness.get_all(By::new().role(egui::accesskit::Role::Button).label("Quadrant")).last().unwrap().click_secondary();
    settle(&mut harness, 3);
    harness.get_by_label("Add node").click();
    settle(&mut harness, 3);
    harness.key_press(egui::Key::S);
    settle(&mut harness, 3);
    if let Some(directory) = &directory {
        save(&mut harness, directory, "filtered-pattern-menu");
    }
    assert!(!harness.get_all(button("Switch")).last().unwrap().accesskit_node().is_disabled());
    assert!(harness.query_all(button("Quadrant")).any(|node| node.accesskit_node().is_disabled()));
    assert!(harness.query_all(button("Iterate")).any(|node| node.accesskit_node().is_disabled()));

    harness.key_press(egui::Key::Enter);
    settle(&mut harness, 3);
    let owner = harness.state().pattern_editor.as_ref().unwrap().node;
    let materialcraft_engine::core::Value::PatternGraph(pattern) = &harness.state().session.graph().unwrap().node(owner).unwrap().params["pattern"] else {
        panic!("missing pattern");
    };

    assert_eq!(pattern.nodes.len(), 2);
    assert_eq!(pattern.nodes[1].kind, materialcraft_engine::core::PatternNodeKind::Switch);
}

#[test]
#[ignore = "requires an offscreen GPU; run explicitly with --ignored"]
fn annotations_and_function_graph_creation() {
    use materialcraft_engine::core::{FunctionGraph, GraphKind, Value};

    let directory = std::path::PathBuf::from(std::env::var_os("MC_SNAPSHOT_DIR").unwrap());
    std::fs::create_dir_all(&directory).unwrap();
    let mut app = MaterialCraftApp::headless(materialcraft_engine::new_session(), UiState::default());
    app.run("graph.node.add", json!({"def": "uniform", "at": [64, 64]})).unwrap();
    let frame =
        app.run("graph.frame.add", json!({"rect": [0, -64, 500, 404], "title": "Frame", "description": "Frame descriptions appear inside the frame and wrap as the frame is resized. Nodes and comments move together with this frame."})).unwrap()["id"].as_u64().unwrap();
    let comment = app.run("graph.comment.add", json!({"at": [64, 240], "text": "Comment"})).unwrap()["id"].as_u64().unwrap();
    app.run("graph.annotation.select", json!({"kind": "frame", "id": frame}));
    let mut harness = harness(app, 1600.0, 960.0);
    settle(&mut harness, 8);
    assert!(harness.query_all(By::new().label("Frame title")).next().is_some());
    assert!(harness.query_all(By::new().label("Title Visible")).next().is_some());
    save(&mut harness, &directory, "selected-frame-properties");

    harness.state_mut().run("graph.annotation.select", json!({"kind": "comment", "id": comment}));
    settle(&mut harness, 3);
    assert!(harness.query_all(By::new().label("Comment description")).next().is_some());
    assert!(harness.query_all(By::new().label("Frame title")).next().is_none());
    save(&mut harness, &directory, "plain-comment-properties");

    let processor = harness.state_mut().run("graph.node.add", json!({"def": "value_processor", "at": [256, 64]})).unwrap()["id"].as_u64().unwrap();
    harness.state_mut().run("graph.node.value_input", json!({"id": processor, "name": "roughness_scale", "type": "Float"}));
    let value = harness.state_mut().run("graph.node.add", json!({"def": "input_value", "at": [64, 440]})).unwrap()["id"].as_u64().unwrap();
    harness.state_mut().run("graph.connect", json!({"from": value, "to": processor, "toPort": "value1"})).unwrap();
    harness.state_mut().run("graph.select", json!({"ids": [processor]}));
    settle(&mut harness, 3);
    harness.get_by_label("Value inputs").click();
    settle(&mut harness, 3);
    assert!(harness.query_all(By::new().label("Value 1")).next().is_some());
    assert!(harness.query_all(By::new().label("Roughness Scale")).next().is_some());
    save(&mut harness, &directory, "processor-port-labels");

    let inspection = materialcraft_ui_egui::control::handle(harness.state_mut(), &egui::Context::default(), "ui.inspect", &json!({}));
    let ports = inspection["result"]["ports"].as_array().unwrap();
    assert!(ports.iter().any(|port| port["node"] == processor && port["port"] == "value1" && port["label"] == "Value 1 (Float)"));
    assert!(ports.iter().any(|port| port["node"] == processor && port["port"] == "roughness_scale" && port["label"] == "Roughness Scale (Float)"));

    harness.state_mut().ui.windows.new_graph = true;
    settle(&mut harness, 3);
    harness.get_by_label("Function Graph").click();
    settle(&mut harness, 3);
    save(&mut harness, &directory, "new-function-graph");
    harness.get_by_label("Create").click();
    settle(&mut harness, 3);
    assert_eq!(harness.state().session.graph().unwrap().kind(), GraphKind::Function);

    let mut function = FunctionGraph::default();
    let first = function.add_constant(Value::Float(2.0));
    let second = function.add_constant(Value::Float(3.0));
    let output = function.add("mul", &[first, second], None);
    function.output = Some(output);
    function.nodes[1].pos = [0.0, 128.0];
    function.nodes[2].pos = [256.0, 64.0];
    let condition = function.add_constant(Value::Bool(true));
    let branch = function.add("ifelse", &[condition, output, second], None);
    function.nodes[3].pos = [256.0, -128.0];
    function.nodes[4].pos = [512.0, 64.0];
    function.output = Some(branch);
    harness.state_mut().run("graph.function.set", json!({"function": function}));
    harness.state_mut().ui.frame_all = true;
    settle(&mut harness, 5);
    save(&mut harness, &directory, "standalone-function-graph");
    let inspection = materialcraft_ui_egui::control::handle(harness.state_mut(), &egui::Context::default(), "ui.inspect", &json!({}));
    let ports = inspection["result"]["ports"].as_array().unwrap();
    assert!(ports.iter().any(|port| port["node"] == branch && port["port"] == "1" && port["label"] == "If"));
    assert!(ports.iter().any(|port| port["node"] == branch && port["port"] == "output" && port["label"] == "Output"));

    let position = harness.state().graph.view.right_bottom() - vec2(20.0, 20.0);
    harness.event(egui::Event::PointerMoved(position));
    harness.step();
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton { pos: position, button: egui::PointerButton::Secondary, pressed, modifiers: egui::Modifiers::NONE });
        harness.step();
    }
    settle(&mut harness, 3);
    save(&mut harness, &directory, "function-context-menu");
    harness.get_by_label("Add node").click();
    settle(&mut harness, 3);
    for label in ["Float", "Float2", "Multiply", "While loop"] {
        assert!(harness.query_all(By::new().label(label)).next().is_some(), "missing {label}");
    }
    save(&mut harness, &directory, "flat-function-node-menu");
}

#[test]
#[ignore = "requires an offscreen GPU; run explicitly with --ignored"]
fn function_cards_and_dot_share_the_regular_graph_style() {
    use materialcraft_engine::core::{FunctionGraph, Value};

    let mut app = MaterialCraftApp::headless(materialcraft_engine::new_session(), UiState::default());
    app.run("file.new", json!({"kind": "function", "name": "Function_cards"})).unwrap();
    let mut function = FunctionGraph::default();
    let value = function.add_constant(Value::Float4([1.0, 0.5, 0.25, 1.0]));
    let dot = function.add("passthrough", &[value], None);
    let multiply = function.add("mul", &[dot, dot], None);
    function.nodes[0].pos = [0.0, 0.0];
    function.nodes[1].pos = [224.0, 64.0];
    function.nodes[2].pos = [416.0, 0.0];
    function.output = Some(multiply);
    app.run("graph.function.set", json!({"function": function})).unwrap();
    let mut harness = harness(app, 1600.0, 960.0);
    settle(&mut harness, 5);
    assert!(harness.query_all(By::new().role(egui::accesskit::Role::Button).label("Dot")).next().is_some());
    let directory = std::path::PathBuf::from(std::env::var_os("MC_SNAPSHOT_DIR").unwrap());
    std::fs::create_dir_all(&directory).unwrap();
    save(&mut harness, &directory, "function-cards-and-dot");
    harness.get_all(By::new().role(egui::accesskit::Role::Button).label("Multiply")).last().unwrap().click();
    settle(&mut harness, 3);
    let blank = harness.state().graph.view.left_bottom() + vec2(12.0, -12.0);
    click_canvas(&mut harness, blank);
    save(&mut harness, &directory, "deselected-function-properties");

    let position = harness.state().graph.view.right_bottom() - vec2(20.0, 20.0);
    harness.event(egui::Event::PointerMoved(position));
    harness.step();
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton { pos: position, button: egui::PointerButton::Secondary, pressed, modifiers: egui::Modifiers::NONE });
        harness.step();
    }
    settle(&mut harness, 3);
    harness.get_by_label("Add node").click();
    settle(&mut harness, 3);
    save(&mut harness, &directory, "grouped-function-menu");
    harness.key_press(egui::Key::M);
    settle(&mut harness, 3);
    save(&mut harness, &directory, "filtered-function-menu");
    assert!(harness.get_all(By::new().role(egui::accesskit::Role::Button).label("Boolean")).last().unwrap().accesskit_node().is_disabled());
    harness.key_press(egui::Key::Enter);
    settle(&mut harness, 3);
    let function = harness.state().session.graph().unwrap().function.as_ref().unwrap();
    assert_eq!(function.nodes.len(), 4);
    let added = function.nodes.last().unwrap();
    assert!(materialcraft_engine::nodes::function::definition(&added.op).unwrap().label.starts_with('M'));
}

fn click_canvas(harness: &mut Harness<'static, MaterialCraftApp>, position: egui::Pos2) {
    harness.event(egui::Event::PointerMoved(position));
    harness.step();
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton { pos: position, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE });
        harness.step();
    }
    settle(harness, 3);
}

#[test]
#[ignore = "requires an offscreen GPU; run explicitly with --ignored"]
fn named_outputs_open_properties_and_preview_only_on_double_click() {
    let mut app = MaterialCraftApp::headless(materialcraft_engine::new_session(), UiState::default());
    let source = app.run("graph.node.add", json!({"def": "uniform", "at": [0, 0]})).unwrap()["id"].as_u64().unwrap();
    let output = app
        .run(
            "graph.node.add",
            json!({
                "def": "output", "at": [224, 0], "connectFrom": {"node": source}, "params": {"identifier": "base_color"}
            }),
        )
        .unwrap()["id"]
        .as_u64()
        .unwrap();
    app.run("view.2d", json!({"node": source})).unwrap();
    app.session.cook();
    let material = app.session.material_maps().into_iter().map(|(name, image)| (name, image.data.clone())).collect::<Vec<_>>();
    let view3d = serde_json::to_value(&app.session.view3d).unwrap();
    let mut harness = harness(app, 1600.0, 960.0);
    settle(&mut harness, 5);
    let query = By::new().role(egui::accesskit::Role::TreeItem).label("Base Color");
    let position = harness.get(query.clone()).rect().center();
    click_canvas(&mut harness, position);
    assert_eq!(harness.state().session.selection, vec![materialcraft_engine::core::NodeId(output)]);
    assert_eq!(harness.state().session.view2d.node, Some(materialcraft_engine::core::NodeId(source)));
    assert!(harness.query_all(By::new().label("Identifier")).next().is_some());
    assert!(!harness.state().graph.ports.iter().any(|port| port.node.0 == output && port.output));

    let directory = std::path::PathBuf::from(std::env::var_os("MC_SNAPSHOT_DIR").unwrap());
    std::fs::create_dir_all(&directory).unwrap();
    save(&mut harness, &directory, "named-output-properties");
    let start = harness.ctx.input(|input| input.time) + 1.0;
    for (index, pressed) in [true, false, true, false].into_iter().enumerate() {
        harness.input_mut().time = Some(start + index as f64 * 0.05);
        harness.event(egui::Event::PointerButton { pos: position, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE });
        harness.step();
    }
    settle(&mut harness, 3);
    assert_eq!(harness.state().session.view2d.node, Some(materialcraft_engine::core::NodeId(output)));
    assert_eq!(serde_json::to_value(&harness.state().session.view3d).unwrap(), view3d);
    let current = harness.state().session.material_maps().into_iter().map(|(name, image)| (name, image.data.clone())).collect::<Vec<_>>();
    assert_eq!(current, material);
    save(&mut harness, &directory, "named-output-preview");
}

#[test]
#[ignore = "requires an offscreen GPU; run explicitly with --ignored"]
fn shared_toolbar_targets_each_graph_and_preserves_undo() {
    use materialcraft_engine::core::{FunctionGraph, NodeId, PatternGraph, PatternNodeKind, Value};

    let directory = std::path::PathBuf::from(std::env::var_os("MC_SNAPSHOT_DIR").unwrap());
    std::fs::create_dir_all(&directory).unwrap();
    let mut common_controls = None;
    for kind in ["material", "function", "pattern"] {
        let mut app = MaterialCraftApp::headless(materialcraft_engine::new_session(), UiState::default());
        let mut owner = 0;
        match kind {
            "function" => {
                app.run("file.new", json!({"kind": "function"})).unwrap();
                let mut graph = FunctionGraph::default();
                for position in [[0.0, 0.0], [256.0, 160.0]] {
                    graph.add_constant(Value::Float(1.0));
                    graph.nodes.last_mut().unwrap().pos = position;
                }
                graph.output = Some(2);
                app.run("graph.function.set", json!({"function": graph})).unwrap();
            }
            "pattern" => {
                owner = app.run("graph.node.add", json!({"def": "fx_map", "at": [700, 800]})).unwrap()["id"].as_u64().unwrap();
                let mut graph = PatternGraph::default();
                for position in [[0.0, 0.0], [256.0, 160.0]] {
                    graph.add(PatternNodeKind::Quadrant).unwrap();
                    graph.nodes.last_mut().unwrap().position = position;
                }
                app.run("graph.pattern.set", json!({"id": owner, "pattern": graph})).unwrap();
                app.pattern_editor = Some(materialcraft_ui_egui::pattern::PatternEditor::new(app.session.graph, NodeId(owner)));
            }
            _ => {
                for position in [[0, 0], [256, 160]] {
                    app.run("graph.node.add", json!({"def": "uniform", "at": position})).unwrap();
                }
            }
        }
        let mut harness = harness(app, 1600.0, 960.0);
        settle(&mut harness, 4);
        let query = |label| By::new().role(egui::accesskit::Role::Button).label(label);
        let controls = ["Frame all (F)", "Align left", "Distribute horizontally", "Snap to grid"].map(|label| harness.get(query(label)).rect());
        if let Some(expected) = common_controls {
            assert_eq!(controls, expected, "{kind} toolbar drift");
        }
        common_controls = Some(controls);
        let context = harness.ctx.clone();
        materialcraft_ui_egui::menus::invoke(harness.state_mut(), &context, "edit.selectAll");
        settle(&mut harness, 2);
        let original = harness.state().session.pkg.clone();
        harness.get(query("Align left")).click();
        settle(&mut harness, 3);
        let graph = harness.state().session.graph().unwrap();
        let positions: Vec<_> = match kind {
            "function" => graph.function.as_ref().unwrap().nodes.iter().map(|node| node.pos).collect(),
            "pattern" => {
                let Value::PatternGraph(pattern) = &graph.node(NodeId(owner)).unwrap().params["pattern"] else { panic!("missing pattern") };
                assert_eq!(graph.node(NodeId(owner)).unwrap().pos, [704.0, 800.0]);
                pattern.nodes.iter().map(|node| node.position).collect()
            }
            _ => graph.nodes.iter().map(|node| node.pos).collect(),
        };
        assert_eq!(positions, vec![[0.0, 0.0], [0.0, 160.0]], "{kind} alignment");
        harness.state_mut().session.undo().unwrap();
        assert_eq!(harness.state().session.pkg, original);
        let snap = harness.state().ui.prefs.snap_to_grid;
        harness.get(query("Snap to grid")).click();
        settle(&mut harness, 2);
        assert_eq!(harness.state().ui.prefs.snap_to_grid, !snap);
        assert_eq!(harness.state().session.pkg, original);
        harness.get(query("Frame all (F)")).click();
        settle(&mut harness, 2);
        save(&mut harness, &directory, &format!("shared-toolbar-{kind}"));
        if kind == "pattern" {
            materialcraft_ui_egui::menus::invoke(harness.state_mut(), &context, "edit.delete");
            settle(&mut harness, 2);
            assert!(harness.query_all(By::new().label("Set a node as the root")).next().is_some());
            harness.state_mut().session.undo().unwrap();
            harness.get(query("Return to parent graph")).click();
            settle(&mut harness, 2);
            assert!(harness.state().pattern_editor.is_none());
            assert_eq!(harness.state().session.pkg, original);
        } else if kind == "function" {
            harness.get(query("Add frame around the selection")).click();
            settle(&mut harness, 2);
            assert_eq!(harness.state().session.graph().unwrap().frames.len(), 1);
            assert_eq!(harness.state().session.graph().unwrap().function, original.graphs[harness.state().session.graph].function);
            save(&mut harness, &directory, "shared-toolbar-function-frame");
        }
    }
}

#[test]
#[ignore = "requires an offscreen GPU; run explicitly with --ignored"]
fn embedded_function_toolbar_and_panels_share_the_active_target() {
    use materialcraft_engine::{
        FunctionGraphTarget,
        core::{FunctionGraph, NodeId, Value},
    };
    for definition in ["pixel_processor", "value_processor"] {
        let mut app = MaterialCraftApp::headless(materialcraft_engine::new_session(), UiState::default());
        let owner = NodeId(app.run("graph.node.add", json!({"def": definition, "at": [700, 800]})).unwrap()["id"].as_u64().unwrap());
        let target = FunctionGraphTarget::Parameter { node: owner, parameter: "function".into() };
        let mut graph = FunctionGraph::default();
        for position in [[0.0, 0.0], [256.0, 160.0]] {
            graph.add_constant(Value::Float(0.5));
            graph.nodes.last_mut().unwrap().pos = position;
        }
        app.run("graph.function.set", json!({"target": target, "function": graph})).unwrap();
        let mut harness = harness(app, 1600.0, 960.0);
        settle(&mut harness, 4);
        let context = harness.ctx.clone();
        materialcraft_ui_egui::menus::invoke(harness.state_mut(), &context, "graph.node.edit");
        materialcraft_ui_egui::menus::invoke(harness.state_mut(), &context, "edit.selectAll");
        settle(&mut harness, 2);
        assert!(harness.query_all(By::new().role(egui::accesskit::Role::Window)).next().is_none());
        assert_eq!(harness.query_all(By::new().role(egui::accesskit::Role::Canvas).label("Function Graph")).count(), 1);
        let query = |label| By::new().role(egui::accesskit::Role::Button).label(label);
        for label in ["Frame all (F)", "Align left", "Distribute horizontally", "Snap to grid", "Show node thumbnails"] {
            assert_eq!(harness.query_all(query(label)).count(), 1, "shared control {label}");
        }
        let original = harness.state().session.pkg.clone();
        harness.get(query("Align left")).click();
        settle(&mut harness, 2);
        let function = target.current(&harness.state().session).unwrap();
        assert_eq!(function.nodes.iter().map(|node| node.pos).collect::<Vec<_>>(), vec![[0.0, 0.0], [0.0, 160.0]]);
        assert_eq!(harness.state().session.graph().unwrap().node(owner).unwrap().pos, [704.0, 800.0]);
        materialcraft_ui_egui::menus::invoke(harness.state_mut(), &context, "edit.undo");
        settle(&mut harness, 2);
        assert_eq!(harness.state().session.pkg, original);
        let snap = harness.state().ui.prefs.snap_to_grid;
        harness.get(query("Snap to grid")).click();
        settle(&mut harness, 2);
        assert_eq!(harness.state().ui.prefs.snap_to_grid, !snap);
        assert!(harness.query_all(By::new().label("Get Float")).next().is_some());
        assert!(materialcraft_ui_egui::properties::title(harness.state()).starts_with("Float"));
        let directory = std::path::PathBuf::from(std::env::var_os("MC_SNAPSHOT_DIR").unwrap());
        std::fs::create_dir_all(&directory).unwrap();
        save(&mut harness, &directory, &format!("{definition}-inline-function"));
        harness.state_mut().ui.library_search = "Sample".into();
        settle(&mut harness, 2);
        for label in ["Sample grayscale", "Sample color"] {
            assert_eq!(harness.query_all(By::new().label(label)).next().is_some(), definition == "pixel_processor");
        }
        harness.get(By::new().role(egui::accesskit::Role::Canvas).label("Function Graph")).click_secondary();
        settle(&mut harness, 2);
        harness.get_by_label("Add node").click();
        settle(&mut harness, 2);
        for label in ["Sample grayscale", "Sample color"] {
            assert_eq!(harness.query_all(query(label)).next().is_some(), definition == "pixel_processor");
        }
        save(&mut harness, &directory, &format!("{definition}-node-menu"));
        harness.key_press(egui::Key::Escape);
        harness.key_press(egui::Key::Escape);
        settle(&mut harness, 2);
        harness.get(query("Return to parent graph")).click();
        settle(&mut harness, 3);
        assert!(harness.state().function_editor.is_none());
        assert_eq!(harness.state().session.selection, vec![owner]);
        assert_eq!(harness.state().session.pkg, original);
    }
}

#[test]
#[ignore = "requires an offscreen GPU; run explicitly with --ignored"]
fn material_assignment_previews_outputs_and_survives_function_editing() {
    use materialcraft_engine::core::NodeId;

    let mut app = MaterialCraftApp::headless(materialcraft_engine::new_session(), UiState::default());
    app.session.preview_size = Some(64);
    app.ui.prefs.preview_size = 64;
    let source =
        app.run("graph.node.add", json!({"def": "uniform", "at": [0, 0], "params": {"color_mode": true, "color": [0.8, 0.04, 0.02, 1.0]}})).unwrap()["id"]
            .as_u64()
            .unwrap();
    let output = app
        .run("graph.node.add", json!({"def": "output", "at": [224, 0], "connectFrom": {"node": source}, "params": {"identifier": "preview_image", "usage": 9}}))
        .unwrap()["id"]
        .as_u64()
        .unwrap();
    let mut harness = harness(app, 1600.0, 960.0);
    settle(&mut harness, 45);
    assert!(harness.state().session.material_maps().is_empty());
    let before = harness.state().view3d.shown.clone().unwrap();
    let directory = std::path::PathBuf::from(std::env::var_os("MC_SNAPSHOT_DIR").unwrap());
    std::fs::create_dir_all(&directory).unwrap();
    save(&mut harness, &directory, "material-unassigned");

    let selection = harness.state().session.selection.clone();
    harness.get(By::new().role(egui::accesskit::Role::TreeItem).label("Preview Image")).click_secondary();
    settle(&mut harness, 3);
    harness.get_by_label("View in 3D view").click();
    settle(&mut harness, 3);
    save(&mut harness, &directory, "material-channel-menu");
    harness.get(By::new().role(egui::accesskit::Role::Button).label("Base color")).click();
    settle(&mut harness, 45);
    assert_eq!(harness.state().session.selection, selection, "channel menu clicks must not reach the graph beneath them");
    assert_eq!(harness.state().session.material_maps()[0].0, "baseColor");
    let red_frame = harness.state().view3d.shown.clone().unwrap();
    assert_ne!(before.2, red_frame.2, "assignment must invalidate the rendered frame");
    let red_pixels = red_frame.2.chunks_exact(4).filter(|pixel| u16::from(pixel[0]) > u16::from(pixel[1]) + 30).count();
    assert!(red_pixels > 100, "the rendered mesh must show the assigned red image");
    save(&mut harness, &directory, "material-assigned-red");

    harness.state_mut().run("graph.node.param", json!({"id": source, "param": "color", "value": [0.02, 0.04, 0.8, 1.0]})).unwrap();
    settle(&mut harness, 45);
    let blue_frame = harness.state().view3d.shown.clone().unwrap();
    let blue_pixels = blue_frame.2.chunks_exact(4).filter(|pixel| u16::from(pixel[2]) > u16::from(pixel[0]) + 30).count();
    assert!(blue_pixels > 100, "edits must refresh the assigned material");
    assert_ne!(red_frame.2, blue_frame.2);
    save(&mut harness, &directory, "material-assigned-blue");

    harness.state_mut().run("file.new", json!({"kind": "function", "name": "Function preview"})).unwrap();
    settle(&mut harness, 45);
    let maps = harness.state().session.material_maps();
    assert_eq!(maps[0].1.get(0, 0, false), [0.02, 0.04, 0.8, 1.0]);
    assert_eq!(harness.state().session.material_preview.channels[&materialcraft_engine::core::MaterialChannel::BaseColor].node, NodeId(output));
    save(&mut harness, &directory, "material-retained-in-function");
}

#[test]
#[ignore = "requires an offscreen GPU; run explicitly with --ignored"]
fn explorer_inputs_and_graph_drop_share_labels_and_material_assignment() {
    use materialcraft_engine::core::{ComponentUsage, MaterialChannel, NodeId};

    let mut app = MaterialCraftApp::headless(materialcraft_engine::new_session(), UiState::default());
    app.session.pkg.graphs[0].name = "Atomic_sample".into();
    app.ui.prefs.preview_size = 64;
    let mut inputs = Vec::new();
    for (definition, identifier, label) in [
        ("input_color", "color_input", "Color Input"),
        ("input_grayscale", "gray_input", "Gray Input"),
        ("input_value", "brightness_value", "Brightness Value"),
    ] {
        let id = app.run("graph.node.add", json!({"def": definition, "at": [0, inputs.len() * 192], "params": {"identifier": identifier}})).unwrap()["id"]
            .as_u64()
            .unwrap();
        inputs.push((NodeId(id), label));
    }
    let source = app.run("graph.node.add", json!({"def": "uniform", "at": [224, 0], "params": {"color_mode": true, "color": [0.8, 0.04, 0.02, 1.0]}})).unwrap()
        ["id"]
        .as_u64()
        .unwrap();
    let output = app.run("graph.node.add", json!({"def": "output", "at": [448, 0], "connectFrom": {"node": source}, "params": {"identifier": "base_color", "usage": ComponentUsage::index("diffuse")}})).unwrap()["id"].as_u64().unwrap();
    let mut harness = harness(app, 1600.0, 960.0);
    settle(&mut harness, 8);
    let directory = std::path::PathBuf::from(std::env::var_os("MC_SNAPSHOT_DIR").unwrap());
    std::fs::create_dir_all(&directory).unwrap();
    let graph_row = harness.get(By::new().role(egui::accesskit::Role::TreeItem).label("Atomic_sample")).rect();
    for (id, label) in inputs {
        let row = harness.get(By::new().role(egui::accesskit::Role::TreeItem).label(label));
        assert!(row.rect().left() > graph_row.left());
        row.click();
        settle(&mut harness, 3);
        assert_eq!(harness.state().session.selection, vec![id]);
        assert_eq!(harness.get(By::new().role(egui::accesskit::Role::TextInput).label("Node label")).value().as_deref(), Some(label));
        assert!(harness.query_all(By::new().label("Identifier")).next().is_some());
        let before = harness.state().session.graph().unwrap().node(id).unwrap().clone();
        let default = &materialcraft_engine::nodes::find(&before.def).unwrap().param("usage").unwrap().default;
        assert_eq!(before.params.get("usage").unwrap_or(default), &materialcraft_engine::core::Value::Enum(ComponentUsage::ANY_INDEX));
        harness.get(By::new().role(egui::accesskit::Role::ComboBox).label("Component Usage")).click();
        settle(&mut harness, 2);
        harness.get_by_label("Base color").scroll_to_me();
        settle(&mut harness, 2);
        harness.get_by_label("Base color").click();
        settle(&mut harness, 2);
        assert_eq!(harness.state().session.graph().unwrap().node(id).unwrap().params.get("usage"), Some(&materialcraft_engine::core::Value::Enum(0)));
        save(&mut harness, &directory, &format!("component-usage-{}", before.def));
        harness.state_mut().run("edit.undo", json!({})).unwrap();
        assert_eq!(harness.state().session.graph().unwrap().node(id).unwrap(), &before);
    }
    harness.get(By::new().role(egui::accesskit::Role::TreeItem).label("Base Color")).click();
    settle(&mut harness, 3);
    harness.get(By::new().role(egui::accesskit::Role::ComboBox).label("Component Usage")).click();
    settle(&mut harness, 3);
    harness.get_by_label("Glossiness").scroll_to_me();
    settle(&mut harness, 3);
    harness.get_by_label("Glossiness").click();
    settle(&mut harness, 3);
    let node = harness.state().session.graph().unwrap().node(NodeId(output)).unwrap();
    assert_eq!(materialcraft_engine::eval::output_usage(node), "glossiness");
    harness.state_mut().run("graph.node.param", json!({"id": output, "param": "usage", "value": ComponentUsage::index("diffuse")})).unwrap();

    settle(&mut harness, 3);
    save(&mut harness, &directory, "explorer-inputs-and-usages");
    harness.state_mut().run("file.new", json!({"kind": "function", "name": "Function editing"})).unwrap();
    harness.state_mut().run("view.3d.clear", json!({})).unwrap();
    settle(&mut harness, 8);
    let graph = harness.state().session.graph;
    let camera = harness.state().session.view3d.clone();
    let package = harness.state().session.pkg.clone();
    let start = harness.get(By::new().role(egui::accesskit::Role::TreeItem).label("Atomic_sample")).rect().center();
    let end = harness.get(By::new().role(egui::accesskit::Role::Image).label("3D view")).rect().center();
    harness.event(egui::Event::PointerMoved(start));
    harness.step();
    harness.event(egui::Event::PointerButton { pos: start, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE });
    harness.step();
    for position in [start + vec2(12.0, 0.0), end] {
        harness.event(egui::Event::PointerMoved(position));
        harness.step();
    }
    save(&mut harness, &directory, "graph-drop-target");
    harness.event(egui::Event::PointerButton { pos: end, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE });
    settle(&mut harness, 45);
    assert_eq!(harness.state().session.graph, graph, "dropping a graph must not open it");
    assert_eq!(harness.state().session.view3d, camera, "dropping must not orbit the camera");
    assert_eq!(harness.state().session.pkg, package);
    let binding = &harness.state().session.material_preview.channels[&MaterialChannel::BaseColor];
    assert_eq!((binding.graph, binding.node), (0, NodeId(output)));
    let frame = &harness.state().view3d.shown.as_ref().unwrap().2;
    assert!(frame.chunks_exact(4).filter(|pixel| u16::from(pixel[0]) > u16::from(pixel[1]) + 30).count() > 100);
    save(&mut harness, &directory, "graph-dropped-on-material");
}
