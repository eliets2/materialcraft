use materialcraft_engine::{
    MaterialPreview, MaterialSource, Session,
    core::{Graph, MaterialChannel, NodeId, Package, Value},
    execute, new_session,
};
use serde_json::json;

fn add(session: &mut Session, definition: &str, params: serde_json::Value) -> NodeId {
    NodeId(execute(session, "graph.node.add", &json!({"def": definition, "params": params})).unwrap()["id"].as_u64().unwrap())
}

fn source(session: &mut Session, color: [f32; 4]) -> NodeId {
    add(session, "uniform", json!({"color_mode": true, "color": color}))
}

fn output(session: &mut Session, source: NodeId, usage: u32) -> NodeId {
    let output = add(session, "output", json!({"usage": usage}));
    execute(session, "graph.connect", &json!({"from": source, "to": output})).unwrap();
    output
}

fn instance(session: &mut Session, graph: &str) -> NodeId {
    let node = session.graph_mut().unwrap().add_node("graph_instance", [0.0; 2]);
    session.graph_mut().unwrap().node_mut(node).unwrap().params.insert("graph".into(), Value::Text(graph.into()));
    node
}

fn color(session: &mut Session, channel: &str) -> [f32; 4] {
    session.cook();
    session.material_maps().into_iter().find(|(usage, _)| *usage == channel).unwrap().1.get(0, 0, false)
}

#[test]
fn explicit_assignment_shows_unclassified_output_and_tracks_edits() {
    let mut session = new_session();
    session.preview_size = Some(16);
    let red = source(&mut session, [0.8, 0.1, 0.1, 1.0]);
    let output = output(&mut session, red, 9);
    session.cook();
    assert!(session.material_maps().is_empty());
    let history = session.history();
    let package = session.pkg.clone();
    execute(&mut session, "view.3d.assign", &json!({"node": output, "channel": "baseColor"})).unwrap();
    assert_eq!(session.history(), history);
    assert_eq!(session.pkg, package);
    assert_eq!(color(&mut session, "baseColor"), [0.8, 0.1, 0.1, 1.0]);
    let key = session.material_key();
    execute(&mut session, "graph.node.param", &json!({"id": red, "param": "color", "value": [0.1, 0.8, 0.2, 1.0]})).unwrap();
    assert_eq!(color(&mut session, "baseColor"), [0.1, 0.8, 0.2, 1.0]);
    assert_ne!(session.material_key(), key);
    let inspection = execute(&mut session, "session.inspect", &json!({})).unwrap();
    assert_eq!(inspection["material3d"]["channels"][0]["size"], json!([16, 16]));
}

#[test]
fn automatic_outputs_follow_usage_and_do_not_follow_selection() {
    let mut session = new_session();
    session.preview_size = Some(16);
    let red = source(&mut session, [0.8, 0.1, 0.1, 1.0]);
    let blue = source(&mut session, [0.1, 0.1, 0.8, 1.0]);
    let output = output(&mut session, red, 0);
    assert_eq!(color(&mut session, "baseColor"), [0.8, 0.1, 0.1, 1.0]);
    execute(&mut session, "graph.select", &json!({"ids": [blue]})).unwrap();
    execute(&mut session, "view.2d", &json!({"node": blue})).unwrap();
    assert_eq!(color(&mut session, "baseColor"), [0.8, 0.1, 0.1, 1.0]);
    execute(&mut session, "graph.node.param", &json!({"id": output, "param": "usage", "value": 4})).unwrap();
    assert_eq!(color(&mut session, "height"), [0.8, 0.1, 0.1, 1.0]);
    assert!(session.material_maps().iter().all(|(channel, _)| *channel != "baseColor"));
}

#[test]
fn graph_application_and_material_reset_preserve_camera_and_scene() {
    let mut session = new_session();
    session.preview_size = Some(16);
    let red = source(&mut session, [0.8, 0.1, 0.1, 1.0]);
    output(&mut session, red, 0);
    let blue = source(&mut session, [0.1, 0.1, 0.8, 1.0]);
    execute(&mut session, "view.3d.assign", &json!({"node": blue, "channel": "baseColor"})).unwrap();
    execute(&mut session, "view.3d.assign", &json!({"node": blue, "channel": "roughness"})).unwrap();
    execute(&mut session, "view.3d", &json!({"scene": "sphere", "yaw": 70, "distance": 4, "environment": "sunset"})).unwrap();
    let view = session.view3d.clone();
    execute(&mut session, "view.3d.outputs", &json!({})).unwrap();
    assert_eq!(color(&mut session, "baseColor"), [0.8, 0.1, 0.1, 1.0]);
    assert!(session.material_maps().iter().any(|(channel, _)| *channel == "roughness"));
    execute(&mut session, "view.3d.outputs", &json!({"reset": true})).unwrap();
    session.cook();
    assert_eq!(session.material_maps().len(), 1);
    assert_eq!(session.view3d, view);
    execute(&mut session, "view.3d.clear", &json!({})).unwrap();
    assert!(session.material_maps().is_empty());
    assert_eq!(session.view3d, view);
}

#[test]
fn assignments_survive_function_editing_and_undo_without_retargeting_node_ids() {
    let mut session = new_session();
    session.preview_size = Some(16);
    let red = source(&mut session, [0.8, 0.1, 0.1, 1.0]);
    execute(&mut session, "view.3d.assign", &json!({"node": red, "channel": "baseColor"})).unwrap();
    session.cook();
    execute(&mut session, "file.new", &json!({"kind": "function", "name": "Function"})).unwrap();
    assert_eq!(session.material_maps()[0].1.get(0, 0, false), [0.8, 0.1, 0.1, 1.0], "keep the previous material while the worker cooks");
    assert_eq!(color(&mut session, "baseColor"), [0.8, 0.1, 0.1, 1.0]);
    execute(&mut session, "edit.undo", &json!({})).unwrap();
    assert_eq!(color(&mut session, "baseColor"), [0.8, 0.1, 0.1, 1.0]);
    execute(&mut session, "file.new", &json!({"template": "empty", "name": "Blue"})).unwrap();
    let blue = source(&mut session, [0.1, 0.1, 0.8, 1.0]);
    assert_eq!(red, blue);
    execute(&mut session, "view.3d.assign", &json!({"graph": 0, "node": red, "channel": "baseColor"})).unwrap();
    execute(&mut session, "graph.node.param", &json!({"id": blue, "param": "color", "value": [0.2, 0.2, 0.9, 1.0]})).unwrap();
    assert_eq!(color(&mut session, "baseColor"), [0.8, 0.1, 0.1, 1.0]);
    execute(&mut session, "graph.rename", &json!({"graph": 0, "name": "Renamed"})).unwrap();
    assert_eq!(color(&mut session, "baseColor"), [0.8, 0.1, 0.1, 1.0]);
    execute(&mut session, "graph.delete", &json!({"graph": 0})).unwrap();
    session.cook();
    assert!(session.material_maps().is_empty());
    execute(&mut session, "edit.undo", &json!({})).unwrap();
    assert_eq!(color(&mut session, "baseColor"), [0.8, 0.1, 0.1, 1.0]);
    execute(&mut session, "file.newPackage", &json!({})).unwrap();
    assert!(session.material_preview.channels.is_empty());
}

#[test]
fn opening_material_graph_automatically_applies_its_outputs() {
    let mut session = new_session();
    session.preview_size = Some(16);
    let red = source(&mut session, [0.8, 0.1, 0.1, 1.0]);
    output(&mut session, red, 0);
    execute(&mut session, "file.new", &json!({"template": "empty", "name": "Blue"})).unwrap();
    let blue = source(&mut session, [0.1, 0.1, 0.8, 1.0]);
    output(&mut session, blue, 0);
    assert_eq!(color(&mut session, "baseColor"), [0.1, 0.1, 0.8, 1.0]);
    execute(&mut session, "graph.open", &json!({"graph": 0})).unwrap();
    assert!(session.material_maps().is_empty(), "stale results must not cross graph IDs");
    assert_eq!(color(&mut session, "baseColor"), [0.8, 0.1, 0.1, 1.0]);
}

#[test]
fn invalid_assignments_are_atomic_including_scalar_instance_outputs() {
    let mut session = new_session();
    session.preview_size = Some(16);
    let image = source(&mut session, [0.8, 0.1, 0.1, 1.0]);
    execute(&mut session, "view.3d.assign", &json!({"node": image, "channel": "baseColor"})).unwrap();
    let value = add(&mut session, "input_value", json!({}));
    let value_output = output(&mut session, value, 0);
    let mut graph = Graph::new("Values");
    let input = graph.add_node("input_value", [0.0; 2]);
    let out = graph.add_node("output", [100.0, 0.0]);
    graph.node_mut(out).unwrap().params.insert("identifier".into(), Value::Text("value".into()));
    graph.connect(input, "output", out, "input").unwrap();
    session.pkg.graphs.push(graph);
    let instance = instance(&mut session, "Values");
    let before = session.material_preview.clone();
    let history = session.history();
    let package = session.pkg.clone();
    for parameters in [
        json!({"node": image, "channel": "unknown", "reset": true}),
        json!({"node": image, "channel": "baseColor", "port": 999, "reset": true}),
        json!({"node": image, "channel": "baseColor", "graph": 999, "reset": true}),
        json!({"node": value, "channel": "baseColor", "reset": true}),
        json!({"node": value_output, "channel": "baseColor", "reset": true}),
        json!({"node": instance, "port": "value", "channel": "baseColor", "reset": true}),
    ] {
        assert!(execute(&mut session, "view.3d.assign", &parameters).is_err(), "{parameters}");
        assert_eq!(session.material_preview, before);
        assert_eq!(session.history(), history);
        assert_eq!(session.pkg, package);
    }
    assert!(execute(&mut session, "view.3d.outputs", &json!({"graph": "Values", "reset": true})).is_err());
    assert_eq!(session.material_preview, before);
}

#[test]
fn multiple_graph_channels_and_multi_output_instances_use_the_correct_ports() {
    let mut package = Package::new("Test");
    let mut graph = Graph::new("Colors");
    graph.size = [4, 4];
    for (name, color) in [("red", [0.8, 0.1, 0.1, 1.0]), ("blue", [0.1, 0.1, 0.8, 1.0])] {
        let image = graph.add_node("uniform", [0.0; 2]);
        graph.node_mut(image).unwrap().params.insert("color".into(), Value::Color(color));
        let output = graph.add_node("output", [100.0, 0.0]);
        graph.node_mut(output).unwrap().params.insert("identifier".into(), Value::Text(name.into()));
        graph.connect(image, "output", output, "input").unwrap();
    }
    package.graphs.push(graph);
    package.graphs.push(Graph::new("Instances"));
    let mut session = Session::new(package);
    execute(&mut session, "graph.open", &json!({"graph": 1})).unwrap();
    let instance = instance(&mut session, "Colors");
    execute(&mut session, "view.3d.assign", &json!({"node": instance, "port": "blue", "channel": "baseColor"})).unwrap();
    assert_eq!(color(&mut session, "baseColor"), [0.1, 0.1, 0.8, 1.0]);
    let key = session.material_key();
    execute(&mut session, "view.3d.assign", &json!({"node": instance, "port": 0, "channel": "baseColor"})).unwrap();
    assert_eq!(color(&mut session, "baseColor"), [0.8, 0.1, 0.1, 1.0]);
    assert_ne!(session.material_key(), key);
    execute(&mut session, "view.3d.assign", &json!({"graph": "Colors", "node": 1, "channel": "emissive"})).unwrap();
    assert_eq!(color(&mut session, "emissive"), [0.8, 0.1, 0.1, 1.0]);
    assert_eq!(session.graph, 1);
}

#[test]
fn removing_an_earlier_graph_adjusts_sources_and_undo_restores_them() {
    let mut session = new_session();
    execute(&mut session, "file.new", &json!({"template": "empty"})).unwrap();
    let red = source(&mut session, [0.8, 0.1, 0.1, 1.0]);
    execute(&mut session, "view.3d.assign", &json!({"node": red, "channel": "baseColor"})).unwrap();
    execute(&mut session, "graph.delete", &json!({"graph": 0})).unwrap();
    assert_eq!(session.material_preview.channels[&MaterialChannel::BaseColor].graph, 0);
    assert_eq!(color(&mut session, "baseColor"), [0.8, 0.1, 0.1, 1.0]);
    execute(&mut session, "edit.undo", &json!({})).unwrap();
    assert_eq!(session.material_preview.channels[&MaterialChannel::BaseColor].graph, 1);
    assert_eq!(color(&mut session, "baseColor"), [0.8, 0.1, 0.1, 1.0]);
    let before = session.material_key();
    execute(&mut session, "graph.select", &json!({"ids": [red]})).unwrap();
    execute(&mut session, "edit.delete", &json!({})).unwrap();
    session.cook();
    assert!(session.material_maps().is_empty());
    assert_ne!(session.material_key(), before);
    execute(&mut session, "edit.undo", &json!({})).unwrap();
    assert_eq!(color(&mut session, "baseColor"), [0.8, 0.1, 0.1, 1.0]);
}

#[test]
fn source_cycles_are_rejected_without_recursion() {
    let mut package = Package::new("Cycle");
    let mut graph = Graph::new("Cycle");
    let instance = graph.add_node("graph_instance", [0.0; 2]);
    graph.node_mut(instance).unwrap().params.insert("graph".into(), Value::Text("Cycle".into()));
    let output = graph.add_node("output", [100.0, 0.0]);
    graph.node_mut(output).unwrap().params.insert("identifier".into(), Value::Text("loop".into()));
    graph.connect(instance, "loop", output, "input").unwrap();
    package.graphs.push(graph);
    let source = MaterialSource { graph: 0, node: instance, port: "loop".into() };
    assert!(!source.is_image(&package));
    assert!(MaterialPreview::for_graph(&package, 0).sources(&package).is_empty());
}

#[test]
fn diffuse_usage_previews_as_base_color_and_unknown_usages_stay_unassigned() {
    use materialcraft_engine::core::ComponentUsage;

    let mut session = new_session();
    session.preview_size = Some(16);
    let red = source(&mut session, [0.8, 0.1, 0.1, 1.0]);
    let output = output(&mut session, red, ComponentUsage::index("diffuse"));
    assert_eq!(color(&mut session, "baseColor"), [0.8, 0.1, 0.1, 1.0]);
    for usage in [ComponentUsage::ANY_INDEX, u32::MAX, ComponentUsage::index("coatRoughness")] {
        session.graph_mut().unwrap().node_mut(output).unwrap().params.insert("usage".into(), Value::Enum(usage));
        assert!(session.material_maps().is_empty(), "usage {usage} must not become a different channel");
    }
}

#[test]
fn extracted_graph_outputs_remain_unclassified() {
    let mut session = new_session();
    let uniform = source(&mut session, [0.8, 0.1, 0.1, 1.0]);
    output(&mut session, uniform, 0);
    execute(&mut session, "graph.select", &json!({"ids": [uniform]})).unwrap();
    execute(&mut session, "graph.createFromSelection", &json!({"name": "Extracted"})).unwrap();
    let (_, graph) = session.pkg.graph_by_name("Extracted").unwrap();
    let output = graph.nodes.iter().find(|node| node.def == "output").unwrap();
    assert_eq!(materialcraft_engine::eval::output_usage(output), "any");
}
