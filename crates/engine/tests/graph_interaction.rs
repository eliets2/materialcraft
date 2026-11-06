use materialcraft_engine::{
    Session,
    core::{NodeId, Value},
    execute,
    formats::sbs::{read_sbs, write_sbs},
    new_session,
};
use serde_json::json;

fn add(session: &mut Session, definition: &str) -> NodeId {
    NodeId(execute(session, "graph.node.add", &json!({"def": definition})).unwrap()["id"].as_u64().unwrap())
}

#[test]
fn fresh_value_processors_accept_one_source_and_round_trip() {
    let mut session = new_session();
    let input = add(&mut session, "input_value");
    execute(&mut session, "graph.node.param", &json!({"id": input.0, "param": "default", "value": 0.35})).unwrap();
    let first =
        NodeId(execute(&mut session, "graph.node.add", &json!({"def": "value_processor", "connectFrom": {"node": input.0}})).unwrap()["id"].as_u64().unwrap());
    let second = add(&mut session, "value_processor");
    execute(&mut session, "graph.connect", &json!({"from": input.0, "to": second.0, "toPort": "value1"})).unwrap();
    assert_eq!(session.graph().unwrap().connections.len(), 2);

    assert!(session.undo().is_some());
    assert_eq!(session.graph().unwrap().connections.len(), 1);
    assert_eq!(session.graph().unwrap().connections[0].to, first);
    assert!(session.redo().is_some());
    session.cook();
    for node in [first, second] {
        assert_eq!(session.cooked.value(node, 0), Some(&Value::Float(0.35)));
    }

    let bytes = write_sbs(&session.pkg).unwrap();
    let loaded = read_sbs(&bytes, "fan_out.sbs", &|_| None).unwrap();
    assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);
    let mut loaded = Session::new(loaded.pkg);
    loaded.cook();
    assert_eq!(loaded.graph().unwrap().connections.len(), 2);
    for node in loaded.graph().unwrap().nodes.iter().filter(|node| node.def == "value_processor") {
        assert_eq!(loaded.cooked.value(node.id, 0), Some(&Value::Float(0.35)));
        assert!(loaded.cooked.nodes[&node.id].error.is_none());
    }
}

#[test]
fn reconnection_is_atomic_and_preserves_other_branches() {
    let mut session = new_session();
    let input = add(&mut session, "input_value");
    let first = add(&mut session, "value_processor");
    let second = add(&mut session, "value_processor");
    let third = add(&mut session, "value_processor");
    for target in [first, second] {
        execute(&mut session, "graph.connect", &json!({"from": input.0, "to": target.0})).unwrap();
    }
    let original = session.pkg.clone();

    let invalid = json!({"from": input.0, "to": third.0, "toPort": "input0", "detach": {"to": first.0, "toPort": "value1"}});
    assert!(execute(&mut session, "graph.connect", &invalid).is_err());
    assert_eq!(session.pkg, original);

    execute(&mut session, "graph.connect", &json!({"from": input.0, "to": third.0, "detach": {"to": first.0, "toPort": "value1"}})).unwrap();
    let graph = session.graph().unwrap();
    assert!(graph.input_of(first, "value1").is_none());
    assert_eq!(graph.input_of(second, "value1").unwrap().from, input);
    assert_eq!(graph.input_of(third, "value1").unwrap().from, input);
    assert!(session.undo().is_some());
    assert_eq!(session.pkg, original);

    execute(&mut session, "graph.connect", &json!({"from": first.0, "to": third.0})).unwrap();
    let original = session.pkg.clone();
    let cycle = json!({"from": first.0, "to": first.0, "detach": {"to": third.0, "toPort": "value1"}});
    assert!(execute(&mut session, "graph.connect", &cycle).is_err());
    assert_eq!(session.pkg, original);
}

#[test]
fn clipboard_variants_keep_internal_wires_and_optionally_keep_incoming_links() {
    let mut session = new_session();
    let input = add(&mut session, "input_value");
    let first = add(&mut session, "value_processor");
    let second = add(&mut session, "value_processor");
    execute(&mut session, "graph.connect", &json!({"from": input.0, "to": first.0})).unwrap();
    execute(&mut session, "graph.connect", &json!({"from": first.0, "to": second.0})).unwrap();
    execute(&mut session, "edit.copy", &json!({"ids": [first.0, second.0]})).unwrap();

    for (command, incoming) in [("edit.paste", true), ("edit.pasteWithoutLinks", false), ("edit.duplicate", true), ("edit.duplicateWithoutLinks", false)] {
        session.selection = vec![first, second];
        execute(&mut session, command, &json!({"snapToGrid": true})).unwrap();
        let selected = &session.selection;
        let graph = session.graph().unwrap();
        assert_eq!(graph.connections.iter().filter(|wire| selected.contains(&wire.from) && selected.contains(&wire.to)).count(), 1);
        assert_eq!(graph.connections.iter().filter(|wire| wire.from == input && selected.contains(&wire.to)).count(), usize::from(incoming));
        assert!(graph.nodes.iter().filter(|node| selected.contains(&node.id)).all(|node| node.pos.iter().all(|coordinate| coordinate % 32.0 == 0.0)));
    }

    execute(&mut session, "file.new", &json!({"template": "empty"})).unwrap();
    execute(&mut session, "edit.paste", &json!({})).unwrap();
    assert_eq!(session.graph().unwrap().connections.len(), 1);
}

#[test]
fn snapping_is_explicit_in_engine_commands_and_preserves_saved_positions() {
    let mut session = new_session();
    let result = execute(&mut session, "graph.node.add", &json!({"def": "uniform", "at": [19, -19], "snapToGrid": true})).unwrap();
    let node = NodeId(result["id"].as_u64().unwrap());
    assert_eq!(session.graph().unwrap().node(node).unwrap().pos, [32.0, -32.0]);
    execute(&mut session, "graph.node.move", &json!({"by": [20, 20], "snapToGrid": true})).unwrap();
    assert_eq!(session.graph().unwrap().node(node).unwrap().pos, [64.0, 0.0]);
    execute(&mut session, "graph.node.move", &json!({"to": [19, -19], "snapToGrid": false})).unwrap();
    assert_eq!(session.graph().unwrap().node(node).unwrap().pos, [19.0, -19.0]);

    let loaded = read_sbs(&write_sbs(&session.pkg).unwrap(), "positions.sbs", &|_| None).unwrap();
    assert_eq!(loaded.pkg.graphs[0].nodes[0].pos, [19.0, -19.0]);
}

#[test]
fn fx_map_starts_with_a_quadrant_and_grows_image_inputs_as_connected() {
    use materialcraft_engine::core::PatternNodeKind;

    let mut session = new_session();
    session.preview_size = Some(16);
    let source = add(&mut session, "uniform");
    execute(&mut session, "graph.node.param", &json!({"id": source.0, "param": "color_mode", "value": false})).unwrap();
    let owner = add(&mut session, "fx_map");
    let Value::PatternGraph(pattern) = &session.graph().unwrap().node(owner).unwrap().params["pattern"] else { panic!("missing default pattern") };
    assert_eq!(pattern.nodes.len(), 1);
    assert_eq!(pattern.nodes[0].kind, PatternNodeKind::Quadrant);
    assert_eq!(pattern.nodes[0].position, [0.0, 0.0]);
    assert_eq!(pattern.root, Some(pattern.nodes[0].id));
    assert_eq!(session.input_ports(owner), vec!["background", "pattern0"]);

    let fresh = read_sbs(&write_sbs(&session.pkg).unwrap(), "fresh_pattern.sbs", &|_| None).unwrap();
    assert!(fresh.warnings.is_empty(), "{:?}", fresh.warnings);
    let loaded_pattern = fresh.pkg.graphs[0].nodes.iter().find(|node| node.def == "fx_map").unwrap();
    let Value::PatternGraph(loaded_pattern) = &loaded_pattern.params["pattern"] else { panic!("missing saved root") };
    assert_eq!(loaded_pattern.nodes.len(), 1);
    assert_eq!(loaded_pattern.nodes[0].kind, PatternNodeKind::Quadrant);
    assert_eq!(loaded_pattern.root, Some(loaded_pattern.nodes[0].id));

    for index in 0..6 {
        execute(&mut session, "graph.connect", &json!({"from": source.0, "to": owner.0, "toPort": format!("pattern{index}")})).unwrap();
        assert_eq!(session.input_ports(owner).last(), Some(&format!("pattern{}", index + 1)));
    }
    let Value::PatternGraph(pattern) = &session.graph().unwrap().node(owner).unwrap().params["pattern"] else { panic!("missing pattern") };

    let mut pattern = (**pattern).clone();
    pattern.nodes[0].parameters.insert("patterntype".into(), Value::Int(1));
    pattern.nodes[0].parameters.insert("imageindex".into(), Value::Int(6));
    execute(&mut session, "graph.pattern.set", &json!({"id": owner.0, "pattern": pattern})).unwrap();
    session.cook();
    assert!(session.cooked.nodes[&owner].error.is_none(), "{:?}", session.cooked.nodes[&owner].error);
    assert!((session.cooked.image(owner, 0).unwrap().mean()[0] - 0.5).abs() < 1e-5);

    let bytes = write_sbs(&session.pkg).unwrap();
    assert!(std::str::from_utf8(&bytes).unwrap().contains("<identifier v=\"input:5\"/>"));
    let read = read_sbs(&bytes, "image_inputs.sbs", &|_| None).unwrap();
    assert!(read.warnings.is_empty(), "{:?}", read.warnings);
    let mut loaded = Session::new(read.pkg);
    loaded.preview_size = Some(16);
    let loaded_owner = loaded.graph().unwrap().nodes.iter().find(|node| node.def == "fx_map").unwrap().id;
    assert_eq!(loaded.input_ports(loaded_owner), session.input_ports(owner));
    loaded.cook();
    assert!((loaded.cooked.image(loaded_owner, 0).unwrap().mean()[0] - 0.5).abs() < 1e-5);

    execute(&mut session, "graph.disconnect", &json!({"to": owner.0, "toPort": "pattern5"})).unwrap();
    assert_eq!(session.input_ports(owner).last().map(String::as_str), Some("pattern5"));
    assert!(session.undo().is_some());
    assert_eq!(session.input_ports(owner).last().map(String::as_str), Some("pattern6"));
    let before = session.pkg.clone();
    assert!(execute(&mut session, "graph.connect", &json!({"from": source.0, "to": owner.0, "toPort": "pattern999999999"})).is_err());
    assert_eq!(session.pkg, before);
}

#[test]
fn terminal_outputs_cook_and_round_trip_without_source_pins() {
    let mut session = new_session();
    session.preview_size = Some(16);
    let source = add(&mut session, "uniform");
    let output = add(&mut session, "output");
    execute(&mut session, "graph.connect", &json!({"from": source.0, "to": output.0})).unwrap();
    assert_eq!(session.input_ports(output), vec!["input"]);
    assert!(session.output_ports(output).is_empty());
    let before = session.pkg.clone();
    assert!(execute(&mut session, "graph.connect", &json!({"from": output.0, "to": source.0})).is_err());
    assert_eq!(session.pkg, before);

    let read = read_sbs(&write_sbs(&session.pkg).unwrap(), "terminal_output.sbs", &|_| None).unwrap();
    assert!(read.warnings.is_empty(), "{:?}", read.warnings);
    let mut loaded = Session::new(read.pkg);
    loaded.preview_size = Some(16);
    loaded.cook();
    let output = loaded.graph().unwrap().nodes.iter().find(|node| node.def == "output").unwrap().id;
    assert!(loaded.output_ports(output).is_empty());
    assert!(loaded.cooked.nodes[&output].error.is_none());
    assert!((loaded.cooked.image(output, 0).unwrap().mean()[0] - 0.5).abs() < 1e-5);
    execute(&mut loaded, "view.2d", &json!({"node": output.0})).unwrap();
    assert_eq!(loaded.view2d_image().unwrap().0, output);
}

#[test]
fn processors_sample_dynamic_image_inputs_after_sbs_round_trip() {
    use materialcraft_engine::core::FunctionGraph;

    for definition in ["pixel_processor", "value_processor"] {
        let mut session = new_session();
        session.preview_size = Some(16);
        let source = add(&mut session, "uniform");
        let processor = add(&mut session, definition);
        for index in 0..6 {
            execute(&mut session, "graph.connect", &json!({"from": source.0, "to": processor.0, "toPort": format!("input{index}")})).unwrap();
            assert_eq!(session.input_ports(processor).last(), Some(&format!("input{}", index + 1)));
        }

        let mut function = FunctionGraph::default();
        let position = function.add_constant(Value::Float2([0.5, 0.5]));
        function.output = Some(function.add("samplelum", &[position], Some(Value::Int2([5, 1]))));
        execute(&mut session, "graph.node.param", &json!({"id": processor.0, "param": "function", "value": Value::Function(Box::new(function))})).unwrap();
        let bytes = write_sbs(&session.pkg).unwrap();
        assert!(std::str::from_utf8(&bytes).unwrap().contains("<identifier v=\"input:5\"/>"));
        let read = read_sbs(&bytes, "processor_images.sbs", &|_| None).unwrap();
        assert!(read.warnings.is_empty(), "{:?}", read.warnings);
        let mut loaded = Session::new(read.pkg);
        loaded.preview_size = Some(16);
        loaded.cook();
        let processor = loaded.graph().unwrap().nodes.iter().find(|node| node.def == definition).unwrap().id;
        assert_eq!(loaded.input_ports(processor).last().map(String::as_str), Some("input6"));
        assert!(loaded.cooked.nodes[&processor].error.is_none(), "{:?}", loaded.cooked.nodes[&processor].error);
        if definition == "value_processor" {
            assert_eq!(loaded.cooked.value(processor, 0), Some(&Value::Float(0.5)));
        } else {
            assert!((loaded.cooked.image(processor, 0).unwrap().mean()[0] - 0.5).abs() < 1e-5);
        }
    }
}

#[test]
fn background_pins_preserve_connected_images_through_sbs() {
    let mut session = new_session();
    session.preview_size = Some(16);
    let source = add(&mut session, "uniform");
    for definition in ["text", "svg"] {
        let target = add(&mut session, definition);
        if definition == "text" {
            execute(&mut session, "graph.node.param", &json!({"id": target.0, "param": "text", "value": ""})).unwrap();
        }
        execute(&mut session, "graph.connect", &json!({"from": source.0, "to": target.0, "toPort": "background"})).unwrap();
    }

    let read = read_sbs(&write_sbs(&session.pkg).unwrap(), "backgrounds.sbs", &|_| None).unwrap();
    assert!(read.warnings.is_empty(), "{:?}", read.warnings);
    let mut loaded = Session::new(read.pkg);
    loaded.preview_size = Some(16);
    loaded.cook();
    for node in loaded.graph().unwrap().nodes.iter().filter(|node| matches!(node.def.as_str(), "text" | "svg")) {
        assert_eq!(loaded.input_ports(node.id), vec!["background"]);
        let image = loaded.cooked.image(node.id, 0).unwrap();
        assert_eq!(image.is_gray(), node.def == "text");
        assert!((image.mean()[0] - 0.5).abs() < 1e-5);
        assert!(loaded.cooked.nodes[&node.id].error.is_none());
    }
}
