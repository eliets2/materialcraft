//! Cook benchmarks (run with `--ignored --nocapture` in release)
use std::time::Instant;

use materialcraft_engine::{Session, execute, sample_session};
use serde_json::{Value, json};

fn run(s: &mut Session, id: &str, p: Value) -> Value {
    execute(s, id, &p).unwrap_or_else(|e| panic!("{id}: {e}"))
}

/// The `atomics` template plus an exposed input `level` read by one unconnected `uniform`
fn heavy(size: u64) -> (Session, u64) {
    let mut s = sample_session();
    run(&mut s, "file.new", json!({"template": "atomics", "size": size}));
    s.preview_size = None;
    let u = run(&mut s, "graph.node.add", json!({"def": "uniform", "params": {"color_mode": false}}))["id"].as_u64().unwrap();
    run(&mut s, "graph.exposed.add", json!({"id": "level", "default": {"t": "float", "v": 0.25}}));
    let f = json!({"nodes": [{"id": 1, "op": "get_float1", "value": {"t": "text", "v": "level"}}], "output": 1});
    run(&mut s, "graph.node.function", json!({"id": u, "param": "color", "function": f}));
    (s, u)
}

fn timed(s: &mut Session) -> (f64, usize, usize) {
    let t = Instant::now();
    s.cook();
    let ms = t.elapsed().as_secs_f64() * 1000.0;
    (ms, s.cooked.nodes.values().filter(|r| !r.cached).count(), s.cooked.nodes.len())
}

#[test]
#[ignore = "benchmark"]
fn bench_input_change() {
    println!("size, cold_ms, warm_ms, input_change_ms, recooked_nodes, total_nodes");
    for size in [1024, 2048] {
        let (mut s, _) = heavy(size);
        let (cold, ..) = timed(&mut s);
        let (warm, ..) = timed(&mut s);
        run(&mut s, "graph.exposed.set", json!({"id": "level", "value": 0.75}));
        let (change, recooked, total) = timed(&mut s);
        println!("{size}, {cold:.1}, {warm:.1}, {change:.1}, {recooked}, {total}");
    }
}

#[test]
#[ignore = "benchmark"]
fn bench_param_edit() {
    println!("size, cold_ms, param_edit_ms, recooked_nodes, total_nodes");
    for size in [1024, 2048] {
        let (mut s, u) = heavy(size);
        let (cold, ..) = timed(&mut s);
        run(&mut s, "graph.node.param", json!({"id": u, "param": "color_mode", "value": true}));
        let (edit, recooked, total) = timed(&mut s);
        println!("{size}, {cold:.1}, {edit:.1}, {recooked}, {total}");
    }
}
