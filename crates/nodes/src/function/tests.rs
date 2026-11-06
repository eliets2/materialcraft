use materialcraft_core::{FunctionGraph, Value};
use materialcraft_image::{Image, Kind};
use std::collections::{BTreeMap, BTreeSet};

use super::{EvaluationContext, EvaluationState, FUNCTIONS, compile, default_value};

fn evaluate(graph: &FunctionGraph, variables: &BTreeMap<String, Value>) -> Result<Value, String> {
    let image = Image::filled(3, 5, Kind::Color, [0.25, 0.5, 0.75, 1.0]);
    let context = EvaluationContext { position: [0.5; 2], size: [3.0, 5.0], images: &[Some(image)], tile: true, variables };
    compile(graph, 7)?.evaluate(&context, &mut EvaluationState::default())
}

fn operation(name: &str, arguments: &[Value], setting: Option<Value>) -> Result<Value, String> {
    let mut graph = FunctionGraph::default();
    let inputs: Vec<_> = arguments.iter().map(|value| graph.add_constant(value.clone())).collect();
    graph.output = Some(graph.add(name, &inputs, setting));
    evaluate(&graph, &BTreeMap::new())
}

#[test]
fn every_native_function_executes_with_its_declared_ports() {
    let mut covered = BTreeSet::new();
    for definition in FUNCTIONS {
        let name = definition.id;
        assert!(covered.insert(name), "duplicate {name}");
        let mut graph = FunctionGraph::default();
        let mut variables = BTreeMap::new();
        let mut setting = default_value(name);
        let mut arguments = vec![Value::Float(0.5); definition.inputs.len()];
        match name {
            "get_bool" => {
                variables.insert("variable".into(), Value::Bool(true));
            }
            "get_string" => {
                variables.insert("variable".into(), Value::Text("test".into()));
            }
            "get_float1" => {
                variables.insert("variable".into(), Value::Float(1.0));
            }
            "get_float2" => {
                variables.insert("variable".into(), Value::Float2([1.0; 2]));
            }
            "get_float3" => {
                variables.insert("variable".into(), Value::Float3([1.0; 3]));
            }
            "get_float4" => {
                variables.insert("variable".into(), Value::Float4([1.0; 4]));
            }
            "get_integer1" => {
                variables.insert("variable".into(), Value::Int(1));
            }
            "get_integer2" => {
                variables.insert("variable".into(), Value::Int2([1; 2]));
            }
            "get_integer3" => {
                variables.insert("variable".into(), Value::Int3([1; 3]));
            }
            "get_integer4" => {
                variables.insert("variable".into(), Value::Int4([1; 4]));
            }
            "and" | "or" | "not" => arguments.fill(Value::Bool(true)),
            "atan2" | "samplecol" | "samplelum" => arguments = vec![Value::Float2([0.5; 2])],
            "vector3" => arguments = vec![Value::Float2([1.0; 2]), Value::Float(2.0)],
            "vector4" => arguments = vec![Value::Float3([1.0; 3]), Value::Float(2.0)],
            "ivector2" => arguments = vec![Value::Int(1), Value::Int(2)],
            "ivector3" => arguments = vec![Value::Int2([1; 2]), Value::Int(2)],
            "ivector4" => arguments = vec![Value::Int3([1; 3]), Value::Int(2)],
            "tofloat" => arguments = vec![Value::Int(1)],
            "tofloat2" => arguments = vec![Value::Int2([1; 2])],
            "tofloat3" => arguments = vec![Value::Int3([1; 3])],
            "tofloat4" => arguments = vec![Value::Int4([1; 4])],
            "toint2" => arguments = vec![Value::Float2([1.5; 2])],
            "toint3" => arguments = vec![Value::Float3([1.5; 3])],
            "toint4" => arguments = vec![Value::Float4([1.5; 4])],
            "ifelse" => arguments = vec![Value::Bool(false), Value::Float(1.0), Value::Float(2.0)],
            "while" => arguments = vec![Value::Float(0.0), Value::Bool(true), Value::Float(1.0)],
            _ if name.starts_with("iswizzle") => arguments = vec![Value::Int4([1, 2, 3, 4])],
            _ if name.starts_with("swizzle") => arguments = vec![Value::Float4([1.0, 2.0, 3.0, 4.0])],
            _ => {}
        }
        if name.starts_with("get_") || matches!(name, "set" | "isdefined") {
            setting = Some(Value::Text("variable".into()));
        }
        let inputs: Vec<_> = arguments.into_iter().map(|value| graph.add_constant(value)).collect();
        graph.output = Some(graph.add(name, &inputs, setting));
        evaluate(&graph, &variables).unwrap_or_else(|error| panic!("{name}: {error}"));
    }
    assert_eq!(covered.len(), 85);
}

#[test]
fn integer_math_preserves_all_32_bits_and_rejects_overflow() {
    assert_eq!(operation("add", &[Value::Int(16_777_217), Value::Int(1)], None).unwrap(), Value::Int(16_777_218));
    assert_eq!(operation("mul", &[Value::Int3([2, 3, 4]), Value::Int3([3, 4, 5])], None).unwrap(), Value::Int3([6, 12, 20]));
    assert!(operation("add", &[Value::Int(i32::MAX), Value::Int(1)], None).is_err());
    assert!(operation("div", &[Value::Int(i32::MIN), Value::Int(-1)], None).is_err());
    assert!(operation("div", &[Value::Float(1.0), Value::Float(0.0)], None).is_err());
}

#[test]
fn vectors_concatenate_swizzle_and_cast_without_broadcasting() {
    let vector = operation("vector4", &[Value::Float2([1.0, 2.0]), Value::Float2([3.0, 4.0])], None).unwrap();
    assert_eq!(vector, Value::Float4([1.0, 2.0, 3.0, 4.0]));
    assert_eq!(operation("swizzle3", &[vector], Some(Value::Int3([3, 0, 3]))).unwrap(), Value::Float3([4.0, 1.0, 4.0]));
    assert_eq!(operation("toint2", &[Value::Float2([-1.9, 2.9])], None).unwrap(), Value::Int2([-1, 2]));
    assert!(operation("swizzle1", &[Value::Float2([0.0; 2])], Some(Value::Int(2))).is_err());
    assert_eq!(operation("pow2", &[Value::Float(3.0)], None).unwrap(), Value::Float(8.0));
}

#[test]
fn branches_are_lazy_and_sequence_orders_side_effects() {
    let mut graph = FunctionGraph::default();
    let condition = graph.add_constant(Value::Bool(true));
    let expected = graph.add_constant(Value::Int(17));
    let undefined = graph.add("get_integer1", &[], Some(Value::Text("missing".into())));
    graph.output = Some(graph.add("ifelse", &[condition, expected, undefined], None));
    assert_eq!(evaluate(&graph, &BTreeMap::new()).unwrap(), Value::Int(17));

    let set = graph.add("set", &[expected], Some(Value::Text("stored".into())));
    let get = graph.add("get_integer1", &[], Some(Value::Text("stored".into())));
    graph.output = Some(graph.add("sequence", &[set, get], None));
    assert_eq!(evaluate(&graph, &BTreeMap::new()).unwrap(), Value::Int(17));
}

#[test]
fn loops_recompute_state_and_have_a_budget() {
    let mut graph = FunctionGraph::default();
    let zero = graph.add_constant(Value::Int(0));
    let one = graph.add_constant(Value::Int(1));
    let three = graph.add_constant(Value::Int(3));
    let init = graph.add("set", &[zero], Some(Value::Text("counter".into())));
    let get = graph.add("get_integer1", &[], Some(Value::Text("counter".into())));
    let condition = graph.add("gteq", &[get, three], None);
    let add = graph.add("add", &[get, one], None);
    let body = graph.add("set", &[add], Some(Value::Text("counter".into())));
    graph.output = Some(graph.add("while", &[init, condition, body], Some(Value::Int(-1))));
    assert_eq!(evaluate(&graph, &BTreeMap::new()).unwrap(), Value::Int(3));

    let condition = graph.add_constant(Value::Bool(false));
    graph.output = Some(graph.add("while", &[zero, condition, one], Some(Value::Int(-1))));
    assert!(evaluate(&graph, &BTreeMap::new()).unwrap_err().contains("budget"));
}

#[test]
fn deep_graphs_shared_inputs_and_cycles_do_not_recurse() {
    let mut graph = FunctionGraph::constant(Value::Float(1.0));
    let mut previous = graph.output.unwrap();
    for _ in 0..3000 {
        previous = graph.add("passthrough", &[previous], None);
    }
    graph.output = Some(previous);
    assert_eq!(evaluate(&graph, &BTreeMap::new()).unwrap(), Value::Float(1.0));
    graph.nodes[0].op = "passthrough".into();
    graph.nodes[0].inputs = vec![Some(previous)];
    assert!(compile(&graph, 0).err().unwrap().contains("cycle"));

    let mut graph = FunctionGraph::constant(Value::Float(1.0));
    let mut previous = graph.output.unwrap();
    for _ in 0..30 {
        previous = graph.add("add", &[previous, previous], None);
    }
    graph.output = Some(previous);
    assert_eq!(evaluate(&graph, &BTreeMap::new()).unwrap(), Value::Float(1_073_741_824.0));
}

#[test]
fn pixel_assignments_are_local_and_match_exported_evaluation() {
    let mut graph = FunctionGraph::default();
    let current = graph.add("get_float1", &[], Some(Value::Text("counter".into())));
    let one = graph.add_constant(Value::Float(1.0));
    let next = graph.add("add", &[current, one], None);
    let assign = graph.add("set", &[next], Some(Value::Text("counter".into())));
    let get = graph.add("get_float1", &[], Some(Value::Text("counter".into())));
    graph.output = Some(graph.add("sequence", &[assign, get], None));
    let program = compile(&graph, 7).unwrap();
    let mut pixel_state = EvaluationState::default();
    for initial in [3.0, 10.0, -2.0] {
        let variables = BTreeMap::from([("counter".into(), Value::Float(initial)), ("untouched".into(), Value::Text("keep".into()))]);
        let context = EvaluationContext { position: [0.5; 2], size: [1.0; 2], images: &[], tile: true, variables: &variables };
        let mut exported = EvaluationState::default();
        let expected = program.evaluate(&context, &mut exported).unwrap();
        for _ in 0..2 {
            assert_eq!(program.evaluate_pixel(&context, &mut pixel_state).unwrap(), expected);
        }
        assert_eq!(expected, Value::Float(initial + 1.0));
        assert_eq!(variables["counter"], Value::Float(initial));
        assert_eq!(exported.variables()["counter"], expected);
        assert_eq!(exported.variables()["untouched"], variables["untouched"]);
    }

    let read_only = compile(&FunctionGraph::constant(Value::Float(0.5)), 7).unwrap();
    let context = EvaluationContext { position: [0.5; 2], size: [1.0; 2], images: &[], tile: true, variables: &BTreeMap::new() };
    assert_eq!(read_only.evaluate_pixel(&context, &mut pixel_state).unwrap(), Value::Float(0.5));
    assert!(pixel_state.variables().is_empty());
}

#[test]
fn pixel_random_seed_and_defined_variables_match_general_evaluation() {
    let mut graph = FunctionGraph::default();
    let maximum = graph.add_constant(Value::Float(1.0));
    let random = graph.add("rand", &[maximum], None);
    let defined = graph.add("isdefined", &[], Some(Value::Text("known".into())));
    let zero = graph.add_constant(Value::Float(0.0));
    graph.output = Some(graph.add("ifelse", &[defined, random, zero], None));
    let program = compile(&graph, 7).unwrap();
    let mut state = EvaluationState::default();
    for variables in [BTreeMap::from([("$randomseed".into(), Value::Int(42)), ("known".into(), Value::Bool(true))]), BTreeMap::new()] {
        let context = EvaluationContext { position: [0.5; 2], size: [1.0; 2], images: &[], tile: true, variables: &variables };
        let expected = program.evaluate(&context, &mut EvaluationState::default()).unwrap();
        assert_eq!(program.evaluate_pixel(&context, &mut state).unwrap(), expected);
    }
}
