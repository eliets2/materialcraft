use materialcraft_engine::core::{FunctionGraph, Value};
use materialcraft_engine::{Session, execute, formats::sbs, new_session};
use serde_json::json;

fn math_function() -> FunctionGraph {
    let mut graph = FunctionGraph::default();
    let power = graph.add("get_float1", &[], Some(Value::Text("power".into())));
    let radius = graph.add("pow2", &[power], None);
    let two = graph.add_constant(Value::Float(2.0));
    let angle = graph.add("log2", &[two], None);
    let position = graph.add("cartesian", &[radius, angle], None);
    let tail = graph.add_constant(Value::Float2([0.0, 1.0]));
    graph.output = Some(graph.add("vector4", &[position, tail], None));
    graph
}

fn assert_outputs(session: &mut Session, radius: f32) {
    session.cook();
    for node in &session.graph().unwrap().nodes {
        let result = &session.cooked.nodes[&node.id];
        assert!(result.error.is_none(), "{:?}", result.error);
        let color = if node.def == "value_processor" {
            let Some(Value::Float4(value)) = session.cooked.value(node.id, 0) else { panic!("expected typed Float4 output") };
            *value
        } else {
            session.cooked.image(node.id, 0).unwrap().mean()
        };
        assert!((color[0] - radius * 1.0f32.cos()).abs() < 1e-5, "{color:?}");
        assert!((color[1] - radius * 1.0f32.sin()).abs() < 1e-5, "{color:?}");
        assert!(color[2].abs() < 1e-5 && (color[3] - 1.0).abs() < 1e-5);
    }
}

#[test]
fn math_primitives_cook_in_processors_and_parameter_functions_and_round_trip() {
    let mut session = new_session();
    session.preview_size = Some(32);
    execute(&mut session, "graph.exposed.add", &json!({"id": "power", "default": {"t": "float", "v": -2.0}})).unwrap();
    let function = math_function();
    for definition in ["value_processor", "pixel_processor", "uniform"] {
        let id = execute(&mut session, "graph.node.add", &json!({"def": definition})).unwrap()["id"].as_u64().unwrap();
        if definition == "uniform" {
            execute(&mut session, "graph.node.function", &json!({"id": id, "param": "color", "function": function})).unwrap();
        } else {
            execute(&mut session, "graph.node.param", &json!({"id": id, "param": "function", "value": Value::Function(Box::new(function.clone()))})).unwrap();
        }
    }
    assert_outputs(&mut session, 0.25);
    execute(&mut session, "graph.exposed.set", &json!({"id": "power", "value": -1.0})).unwrap();
    assert_outputs(&mut session, 0.5);
    execute(&mut session, "edit.undo", &json!({})).unwrap();
    assert_outputs(&mut session, 0.25);
    execute(&mut session, "edit.redo", &json!({})).unwrap();
    assert_outputs(&mut session, 0.5);
    let report = sbs::read_sbs(&sbs::write_sbs(&session.pkg).unwrap(), "math.sbs", &|_| None).unwrap();
    assert!(report.warnings.is_empty(), "{:?}", report.warnings);
    let mut reopened = Session::new(report.pkg);
    reopened.preview_size = Some(32);
    assert_outputs(&mut reopened, 0.5);
}
