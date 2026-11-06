use crate::{CookContext, find, resolve_params};
use materialcraft_core::{FunctionGraph, PatternGraph, PatternNodeKind, Value};
use std::collections::BTreeMap;

fn cook(graph: PatternGraph) -> crate::CookResult {
    let definition = find("fx_map").unwrap();
    let parameters = resolve_params(definition, &BTreeMap::from([("pattern".into(), Value::PatternGraph(Box::new(graph)))]));
    let variables = BTreeMap::new();
    let context = CookContext { w: 16, h: 16, seed: 7, tile: true, inputs: &[], params: &parameters, resource: &|_| None, vars: &variables };
    (definition.cook)(&context)
}

#[test]
fn iterate_sets_number_and_switch_selects_one_child() {
    let mut graph = PatternGraph::default();
    let iterate = graph.add(PatternNodeKind::Iterate).unwrap();
    let switch = graph.add(PatternNodeKind::Switch).unwrap();
    let quadrant = graph.add(PatternNodeKind::Quadrant).unwrap();
    graph.root = Some(iterate);
    graph.nodes[0].children = vec![None, Some(switch)];
    graph.nodes[0].parameters.insert("numberadded".into(), Value::Int(3));
    graph.nodes[1].children = vec![Some(quadrant), None];
    graph.nodes[2].parameters.insert("patterntype".into(), Value::Int(2));
    let mut function = FunctionGraph::default();
    function.output = Some(function.add("get_float1", &[], Some(Value::Text("$number".into()))));
    graph.nodes[2].parameters.insert("opacity".into(), Value::Function(Box::new(function)));
    let image = cook(graph.clone()).unwrap().remove(0);
    assert!((image.mean()[0] - 3.0).abs() < 1e-6);
    let dot = graph.add(PatternNodeKind::Dot).unwrap();
    graph.nodes[1].children[0] = Some(dot);
    graph.nodes[3].children[0] = Some(quadrant);
    assert_eq!(cook(graph.clone()).unwrap()[0].data, image.data);
    graph.nodes[1].parameters.insert("switch".into(), Value::Bool(true));
    assert_eq!(cook(graph).unwrap()[0].mean()[0], 0.0);
}

#[test]
fn patterns_reject_cycles_and_bound_exponential_traversal() {
    let mut graph = PatternGraph::default();
    let root = graph.add(PatternNodeKind::Quadrant).unwrap();
    graph.nodes[0].children[0] = Some(root);
    assert!(cook(graph).unwrap_err().contains("cycle"));

    let mut graph = PatternGraph::default();
    for _ in 0..12 {
        graph.add(PatternNodeKind::Quadrant).unwrap();
    }
    for index in 0..11 {
        graph.nodes[index].children = vec![Some(graph.nodes[index + 1].id); 4];
    }
    assert!(cook(graph).unwrap_err().contains("budget"));
}

#[test]
fn image_indices_outside_the_supported_range_return_an_error() {
    let mut graph = PatternGraph::with_root(PatternNodeKind::Quadrant);
    graph.nodes[0].parameters.insert("patterntype".into(), Value::Int(1));
    graph.nodes[0].parameters.insert("imageindex".into(), Value::Int(i32::MAX));
    assert!(cook(graph).unwrap_err().contains("unsupported pattern"));
}
