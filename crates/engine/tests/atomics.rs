use materialcraft_engine::{
    Session, atomic_sample,
    core::{FunctionGraph, Graph, Package, Resource, ResourceBytes, Value, ValueType},
    execute,
    formats::sbs::{read_sbs, write_sbs},
};
use serde_json::json;

#[test]
fn atomic_sample_round_trips_through_native_fields_and_cooks_identically() {
    let mut original = Session::new(atomic_sample().unwrap());
    original.preview_size = Some(64);
    original.cook();
    for result in original.cooked.nodes.values() {
        assert!(result.error.is_none(), "{:?}", result.error);
    }
    let bytes = write_sbs(&original.pkg).unwrap();
    let text = std::str::from_utf8(&bytes).unwrap();
    for required in ["<paramsGraph>", "<paramNodes>", "<inputValues>", "<compInputBridge>", "<compOutputBridge>"] {
        assert!(text.contains(required), "missing {required}");
    }
    for forbidden in ["materialcraft-package", "materialcraft://", "<compInstance>"] {
        assert!(!text.contains(forbidden));
    }
    let read = read_sbs(&bytes, "atomic_sample.sbs", &|_| None).unwrap();
    assert!(read.warnings.is_empty(), "{:?}", read.warnings);
    let mut loaded = Session::new(read.pkg);
    loaded.preview_size = Some(64);
    loaded.cook();
    assert_eq!(original.graph().unwrap().connections.len(), loaded.graph().unwrap().connections.len());
    for (before, after) in original.graph().unwrap().nodes.iter().zip(&loaded.graph().unwrap().nodes) {
        assert_eq!(before.def, after.def);
        let result = &loaded.cooked.nodes[&after.id];
        assert!(result.error.is_none(), "{}: {:?}", after.def, result.error);
        match (original.cooked.image(before.id, 0), loaded.cooked.image(after.id, 0)) {
            (Some(before), Some(after)) => {
                assert_eq!((before.w, before.h, before.kind), (after.w, after.h, after.kind));
                let error = before.data.iter().zip(after.data.iter()).map(|(a, b)| (a - b).abs()).fold(0.0_f32, f32::max);
                assert!(error < 1e-5, "pixel error {error}");
            }
            (None, None) => assert_eq!(original.cooked.value(before.id, 0), loaded.cooked.value(after.id, 0)),
            _ => panic!("output kind changed for {}", before.def),
        }
    }
}

#[test]
fn every_catalog_entry_has_a_native_sbs_representation() {
    let mut package = Package::new("Primitives");
    let mut graph = Graph::new("Primitives");
    for definition in materialcraft_engine::nodes::catalog() {
        graph.add_node(definition.id, [0.0, graph.nodes.len() as f32 * 180.0]);
    }
    package.graphs.push(graph);
    let bytes = write_sbs(&package).unwrap();
    let loaded = read_sbs(&bytes, "primitives.sbs", &|_| None).unwrap();
    assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);
    assert_eq!(loaded.pkg.graphs[0].nodes.len(), 30);
    for (before, after) in package.graphs[0].nodes.iter().zip(&loaded.pkg.graphs[0].nodes) {
        assert_eq!(before.def, after.def);
    }
}

#[test]
fn native_value_types_survive_processors_and_sbs() {
    let mut package = Package::new("Values");
    let mut graph = Graph::new("Values");
    let values = [
        Value::Int(16_777_217),
        Value::Int2([i32::MIN, i32::MAX]),
        Value::Int3([1, 2, 3]),
        Value::Int4([4, 3, 2, 1]),
        Value::Float(0.25),
        Value::Float2([0.25; 2]),
        Value::Float3([0.25; 3]),
        Value::Float4([0.25; 4]),
        Value::Bool(true),
        Value::Text("sample & <value>".into()),
    ];
    for value in &values {
        let node = graph.add_node("value_processor", [0.0; 2]);
        graph.node_mut(node).unwrap().params.insert("function".into(), Value::Function(Box::new(FunctionGraph::constant(value.clone()))));
    }
    package.graphs.push(graph);
    let read = read_sbs(&write_sbs(&package).unwrap(), "values.sbs", &|_| None).unwrap();
    assert!(read.warnings.is_empty(), "{:?}", read.warnings);
    let mut session = Session::new(read.pkg);
    session.preview_size = Some(16);
    session.cook();
    for (node, expected) in session.graph().unwrap().nodes.iter().zip(values) {
        assert_eq!(session.cooked.value(node.id, 0), Some(&expected), "{:?}", session.cooked.nodes[&node.id].error);
    }
}

#[test]
fn value_wiring_checks_types_and_edits_undo() {
    let mut session = Session::new(atomic_sample().unwrap());
    let graph = session.graph().unwrap();
    let input = graph.nodes.iter().find(|node| node.def == "input_value").unwrap().id;
    let processor = graph.nodes.iter().find(|node| node.def == "value_processor").unwrap().id;
    let blur = graph.nodes.iter().find(|node| node.def == "blur").unwrap().id;
    assert!(execute(&mut session, "graph.connect", &json!({"from": input.0, "to": blur.0})).is_err());
    execute(&mut session, "graph.node.value_input", &json!({"id": processor.0, "name": "integer", "type": "Integer"})).unwrap();
    assert!(execute(&mut session, "graph.connect", &json!({"from": input.0, "to": processor.0, "toPort": "integer"})).is_err());
    assert_eq!(session.graph().unwrap().node(processor).unwrap().input_values.get("integer"), Some(&ValueType::Integer));
    execute(&mut session, "edit.undo", &json!({})).unwrap();
    assert!(!session.graph().unwrap().node(processor).unwrap().input_values.contains_key("integer"));
}

#[test]
fn svg_resource_is_embedded_and_rendered_after_reopening() {
    let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32"><circle cx="16" cy="16" r="8" fill="red"/></svg>"#;
    let mut package = Package::new("Vector");
    package.resources.push(Resource::new("circle", "circle.svg", ResourceBytes::new(svg.to_vec())));
    let mut graph = Graph::new("Vector");
    graph.size = [5, 5];
    let node = graph.add_node("svg", [0.0; 2]);
    graph.node_mut(node).unwrap().params.insert("resource".into(), Value::Text("circle".into()));
    package.graphs.push(graph);
    let read = read_sbs(&write_sbs(&package).unwrap(), "vector.sbs", &|_| None).unwrap();
    assert!(read.warnings.is_empty(), "{:?}", read.warnings);
    assert_eq!(read.pkg.resources[0].bytes.as_slice(), svg);
    let mut session = Session::new(read.pkg);
    session.cook();
    let node = session.graph().unwrap().nodes[0].id;
    assert!(session.cooked.nodes[&node].error.is_none());
    let image = session.cooked.image(node, 0).unwrap();
    assert!(image.get(16, 16, false)[0] > 0.99);
    assert_eq!(image.get(0, 0, false)[3], 0.0);
}

#[test]
fn exposed_parameter_binding_remains_live_after_saving() {
    let mut session = Session::new(atomic_sample().unwrap());
    let blur = session.graph().unwrap().nodes.iter().find(|node| node.def == "blur").unwrap().id;
    execute(&mut session, "graph.exposed.add", &json!({"id": "radius", "bind": {"node": blur.0, "param": "intensity"}})).unwrap();
    execute(&mut session, "graph.exposed.set", &json!({"id": "radius", "value": 4.0})).unwrap();
    let bytes = write_sbs(&session.pkg).unwrap();
    let loaded = read_sbs(&bytes, "exposed.sbs", &|_| None).unwrap();
    assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);
    let mut loaded = Session::new(loaded.pkg);
    loaded.preview_size = Some(32);
    loaded.cook();
    let before = loaded.cooked.image(blur, 0).unwrap().data.clone();
    execute(&mut loaded, "graph.exposed.set", &json!({"id": "radius", "value": 0.0})).unwrap();
    loaded.cook();
    assert_ne!(before, loaded.cooked.image(blur, 0).unwrap().data);
}

#[test]
fn levels_preserve_independent_rgba_parameters_in_sbs() {
    let mut session = Session::new(atomic_sample().unwrap());
    let input =
        execute(&mut session, "graph.node.add", &json!({"def": "uniform", "params": {"color": {"t": "color", "v": [0.5, 0.5, 0.5, 0.5]}}})).unwrap()["id"]
            .as_u64()
            .unwrap();
    let levels =
        execute(&mut session, "graph.node.add", &json!({"def": "levels", "connectFrom": {"node": input}, "params": {"out_high": [1.0, 0.8, 0.6, 0.4]}}))
            .unwrap()["id"]
            .as_u64()
            .unwrap();
    let loaded = read_sbs(&write_sbs(&session.pkg).unwrap(), "levels.sbs", &|_| None).unwrap();
    assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);
    let mut session = Session::new(loaded.pkg);
    session.preview_size = Some(16);
    session.cook();
    let pixel = session.cooked.image(materialcraft_engine::core::NodeId(levels), 0).unwrap().get(0, 0, false);
    for (actual, expected) in pixel.into_iter().zip([0.5, 0.4, 0.3, 0.2]) {
        assert!((actual - expected).abs() < 1e-5, "{pixel:?}");
    }
}

#[test]
fn selected_node_graph_is_resolved_without_changing_the_document() {
    let mut session = materialcraft_engine::new_session();
    assert!(execute(&mut session, "graph.node.edit", &json!({})).is_err());
    for (definition, kind, parameter) in
        [("fx_map", "pattern", "pattern"), ("pixel_processor", "function", "function"), ("value_processor", "function", "function")]
    {
        let node = execute(&mut session, "graph.node.add", &json!({"def": definition})).unwrap()["id"].as_u64().unwrap();
        let original = session.pkg.clone();
        let history = session.history();
        assert_eq!(execute(&mut session, "graph.node.edit", &json!({})).unwrap(), json!({"kind": kind, "node": node, "parameter": parameter}));
        assert_eq!(session.pkg, original);
        assert_eq!(session.history(), history);
    }
    execute(&mut session, "graph.node.add", &json!({"def": "uniform"})).unwrap();
    assert!(execute(&mut session, "graph.node.edit", &json!({})).is_err());

    let node = session.graph_mut().unwrap().add_node("graph_instance", [0.0; 2]);
    session.graph_mut().unwrap().node_mut(node).unwrap().params.insert("graph".into(), Value::Text("Child".into()));
    session.selection = vec![node];
    assert!(execute(&mut session, "graph.node.edit", &json!({})).is_err());
    session.pkg.graphs.push(materialcraft_engine::core::Graph::new("Child"));
    let original = session.pkg.clone();
    let history = session.history();
    assert_eq!(execute(&mut session, "graph.node.edit", &json!({})).unwrap(), json!({"kind": "graph", "node": node, "graph": 1}));
    assert_eq!(session.pkg, original);
    assert_eq!(session.history(), history);
}
