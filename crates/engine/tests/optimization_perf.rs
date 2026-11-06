//! Focused optimization benchmarks: release, ignored, serial, with --nocapture
use std::{collections::BTreeMap, hint::black_box, sync::Mutex, time::Instant};

use materialcraft_core::{Connection, FunctionGraph, Graph, Package, PatternGraph, PatternNodeKind, Value};
use materialcraft_eval::{Cache, Options, cook};
use materialcraft_nodes::{CookContext, find, resolve_params};

fn median_ms(mut run: impl FnMut()) -> f64 {
    let mut samples = [0.0; 3];
    for sample in &mut samples {
        let start = Instant::now();
        run();
        *sample = start.elapsed().as_secs_f64() * 1000.0;
    }
    samples.sort_by(f64::total_cmp);
    samples[1]
}

#[test]
#[ignore = "optimization benchmark"]
fn optimization_baseline() {
    for count in [128, 1024, 4096] {
        let ms = median_ms(|| {
            let mut graph = PatternGraph::default();
            for _ in 0..count {
                graph.add(PatternNodeKind::Quadrant).unwrap();
            }
            black_box(graph);
        });
        println!("pattern_add,{count},{ms:.3}");
    }

    let mut function = FunctionGraph::default();
    let position = function.add("get_float2", &[], Some(Value::Text("$pos".into())));
    let horizontal = function.add("swizzle1", &[position], Some(Value::Int(0)));
    let factor = function.add_constant(Value::Float(0.5));
    function.output = Some(function.add("mul", &[horizontal, factor], None));
    let definition = find("pixel_processor").unwrap();
    let mut parameters = resolve_params(definition, &BTreeMap::new());
    parameters.insert("function".into(), Value::Function(Box::new(function)));
    parameters.insert("color_mode".into(), Value::Bool(false));
    for size in [512, 2048] {
        for count in [0, 16] {
            let variables = (0..count).map(|index| (format!("input_{index}"), Value::Float(index as f32))).collect();
            let context = CookContext { w: size, h: size, seed: 7, tile: true, inputs: &[], params: &parameters, resource: &|_| None, vars: &variables };
            let ms = median_ms(|| {
                black_box((definition.cook)(&context).unwrap());
            });
            println!("pixel_processor,{size},{count},{ms:.3}");
        }
    }

    for count in [128, 1024, 4096] {
        for branching in [false, true] {
            let mut graph = Graph::new("Benchmark");
            graph.size = [4, 4];
            let source = graph.add_node("uniform", [0.0; 2]);
            let mut previous = source;
            for _ in 1..count {
                let node = graph.add_node("dot", [0.0; 2]);
                graph.connections.push(Connection {
                    from: if branching { source } else { previous },
                    from_port: "output".into(),
                    to: node,
                    to_port: "input".into(),
                });
                previous = node;
            }
            let mut package = Package::new("Benchmark");
            package.graphs.push(graph);
            let options = Options::new(&|_| None);
            let cache = Mutex::new(Cache::default());
            let initial = cook(&package, 0, &options, &cache);
            assert_eq!(initial.nodes.len(), count);
            assert!(initial.nodes.values().all(|result| result.error.is_none()));
            let ms = median_ms(|| {
                let cooked = cook(&package, 0, &options, &cache);
                assert!(cooked.nodes.values().all(|result| result.cached));
                black_box(cooked);
            });
            println!("warm_cook,{count},{branching},{ms:.3}");
        }
    }
}
