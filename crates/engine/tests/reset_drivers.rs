use materialcraft_engine::{
    Session,
    core::{FunctionGraph, NodeId, Value},
    execute, formats, new_session,
};
use serde_json::{Value as Json, json};

fn run(s: &mut Session, id: &str, params: Json) -> Json {
    execute(s, id, &params).unwrap()
}

fn setup() -> (Session, NodeId) {
    let mut s = new_session();
    s.preview_size = Some(32);
    let source = run(&mut s, "graph.node.add", json!({"def": "uniform"}))["id"].as_u64().unwrap();
    let levels = NodeId(run(&mut s, "graph.node.add", json!({"def": "levels", "connectFrom": {"node": source}}))["id"].as_u64().unwrap());
    (s, levels)
}

fn drive(s: &mut Session, id: NodeId, param: &str, value: f32, function: bool) {
    if function {
        run(s, "graph.node.function", json!({"id": id.0, "param": param, "function": FunctionGraph::constant(Value::Float4([value; 4]))}));
    } else {
        run(s, "graph.exposed.add", json!({"id": param, "default": Value::Float4([value; 4]), "bind": {"node": id.0, "param": param}}));
    }
}

fn pixel(s: &mut Session, id: NodeId) -> f32 {
    let cooked = s.cook();
    assert!(cooked.nodes[&id].error.is_none());
    cooked.image(id, 0).unwrap().get_gray(0, 0, false)
}

fn check_single_reset(function: bool) {
    let (mut s, id) = setup();
    drive(&mut s, id, "in_low", 0.4, function);
    drive(&mut s, id, "out_high", 0.8, !function);
    let before = s.pkg.graphs.clone();
    let driven = pixel(&mut s, id);
    assert!(driven < 0.2);
    run(&mut s, "graph.node.reset", json!({"id": id.0, "param": "in_low"}));
    assert!((pixel(&mut s, id) - 0.4).abs() < 1e-5, "reset left the in_low driver active (function={function})");
    let n = s.graph().unwrap().node(id).unwrap();
    assert!(!n.bindings.contains_key("in_low") && !n.functions.contains_key("in_low"));
    assert!(n.bindings.contains_key("out_high") || n.functions.contains_key("out_high"));
    run(&mut s, "edit.undo", json!({}));
    assert_eq!(s.pkg.graphs, before);
    assert!((pixel(&mut s, id) - driven).abs() < 1e-5);
    run(&mut s, "edit.redo", json!({}));
    assert!((pixel(&mut s, id) - 0.4).abs() < 1e-5);
}

#[test]
fn resetting_one_bound_parameter_preserves_other_drivers() {
    check_single_reset(false);
}

#[test]
fn resetting_one_function_parameter_preserves_other_drivers() {
    check_single_reset(true);
}

#[test]
fn resetting_all_parameters_removes_drivers_and_restores_default_output() {
    let (mut s, id) = setup();
    drive(&mut s, id, "in_low", 0.4, false);
    drive(&mut s, id, "out_high", 0.8, true);
    run(&mut s, "graph.node.param", json!({"id": id.0, "param": "out_low", "value": [0.1, 0.1, 0.1, 0.1]}));
    let before = s.pkg.graphs.clone();
    let driven = pixel(&mut s, id);
    assert!((driven - 0.5).abs() > 0.1);
    let history = s.history().len();
    run(&mut s, "graph.node.reset", json!({"id": id.0}));
    assert!((pixel(&mut s, id) - 0.5).abs() < 1e-5, "reset left dynamic parameters active");
    let n = s.graph().unwrap().node(id).unwrap();
    assert!(n.params.is_empty() && n.bindings.is_empty() && n.functions.is_empty());
    assert_eq!(s.history().len(), history + 1);
    assert_eq!(s.graph().unwrap().exposed, before[0].exposed);
    run(&mut s, "edit.undo", json!({}));
    assert_eq!(s.pkg.graphs, before);
    assert!((pixel(&mut s, id) - driven).abs() < 1e-5);
    run(&mut s, "edit.redo", json!({}));
    assert!((pixel(&mut s, id) - 0.5).abs() < 1e-5);
    let mut reopened = Session::new(formats::sbs::read_sbs(&formats::sbs::write_sbs(&s.pkg).unwrap(), "test.sbs", &|_| None).unwrap().pkg);
    reopened.preview_size = Some(32);
    let reopened_id = reopened.graph().unwrap().nodes.iter().find(|node| node.def == "levels").unwrap().id;
    assert!((pixel(&mut reopened, reopened_id) - 0.5).abs() < 1e-5);
}
