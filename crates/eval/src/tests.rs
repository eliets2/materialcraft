use crate::{cache_key::graph_tree_hash, graph_instance::instances_reach};
use materialcraft_core::{NodeId, Package, Value};
use materialcraft_image::{Image, Kind};
use std::sync::Mutex;
use std::sync::atomic::Ordering;

use super::*;
use materialcraft_core::Graph;

fn no_res(_: &str) -> Option<Image> {
    None
}

fn sample() -> (Package, NodeId, NodeId, NodeId) {
    let mut g = Graph::new("g");
    g.size = [6, 6];
    let noise = g.add_node("uniform", [0.0, 0.0]);
    let blur = g.add_node("blur", [150.0, 0.0]);
    let out = g.add_node("output", [300.0, 0.0]);
    g.connect(noise, "output", blur, "input").unwrap();
    g.connect(blur, "output", out, "input").unwrap();
    let mut p = Package::new("p");
    p.graphs.push(g);
    (p, noise, blur, out)
}

#[test]
fn cooks_and_caches() {
    let (mut p, noise, blur, out) = sample();
    let cache = Mutex::new(Cache::default());
    let opts = Options::new(&no_res);
    let a = cook(&p, 0, &opts, &cache);
    assert!(a.nodes.values().all(|r| r.error.is_none() && !r.cached));
    assert_eq!(a.image(out, 0).map(|i| (i.w, i.h)), Some((64, 64)));
    // Same graph: everything from the cache
    let b = cook(&p, 0, &opts, &cache);
    assert!(b.nodes.values().all(|r| r.cached));
    // Editing the blur re-cooks the blur and the output, not the noise
    p.graphs[0].node_mut(blur).unwrap().params.insert("intensity".into(), Value::Float(5.0));
    let c = cook(&p, 0, &opts, &cache);
    assert!(c.nodes[&noise].cached && !c.nodes[&blur].cached && !c.nodes[&out].cached);
    assert_eq!(outputs(&p.graphs[0], &c).len(), 1);
}

#[test]
fn hostile_param_is_clamped_before_cooking() {
    let (mut p, noise, _, _) = sample();
    let blur = p.graphs[0].add_node("directional_blur", [150.0, 100.0]);
    p.graphs[0].connect(noise, "output", blur, "input").unwrap();
    p.graphs[0].node_mut(blur).unwrap().params.insert("intensity".into(), Value::Float(1e30));
    let cache = Mutex::new(Cache::default());
    let c = cook(&p, 0, &Options::new(&no_res), &cache);
    assert!(c.nodes[&blur].error.is_none(), "{:?}", c.nodes[&blur].error);
    assert_eq!(c.image(blur, 0).map(|i| (i.w, i.h)), Some((64, 64)));
}

#[test]
fn preview_cap_and_unknown_nodes() {
    let (mut p, _, _, out) = sample();
    p.graphs[0].size = [10, 10];
    let bad = p.graphs[0].add_node("no_such_node", [0.0, 0.0]);
    let cache = Mutex::new(Cache::default());
    let mut opts = Options::new(&no_res);
    opts.max_size = Some(128);
    let c = cook(&p, 0, &opts, &cache);
    assert_eq!(c.image(out, 0).map(|i| i.w), Some(128));
    assert!(c.nodes[&bad].error.is_some());
}

#[test]
fn graph_instances_cook_their_subgraph() {
    let (mut p, ..) = sample();
    // A second graph: input → invert → output, instanced from the first
    let mut sub = Graph::new("inv");
    sub.size = [6, 6];
    let i = sub.add_node("input_grayscale", [0.0, 0.0]);
    let inv = sub.add_node("levels", [100.0, 0.0]);
    sub.node_mut(inv).unwrap().params.insert("out_low".into(), Value::Float4([1.0; 4]));
    sub.node_mut(inv).unwrap().params.insert("out_high".into(), Value::Float4([0.0; 4]));
    let o = sub.add_node("output", [200.0, 0.0]);
    sub.connect(i, "output", inv, "input").unwrap();
    sub.connect(inv, "output", o, "input").unwrap();
    p.graphs.push(sub);
    let g = &mut p.graphs[0];
    let u = g.add_node("uniform", [0.0, 100.0]);
    g.node_mut(u).unwrap().params.insert("color_mode".into(), Value::Bool(false));
    g.node_mut(u).unwrap().params.insert("color".into(), Value::Color([0.25, 0.25, 0.25, 1.0]));
    let inst = g.add_node("graph_instance", [100.0, 100.0]);
    g.node_mut(inst).unwrap().params.insert("graph".into(), Value::Text("inv".into()));
    g.connect(u, "output", inst, "input").unwrap();
    // A graph instancing itself is refused, not a stack overflow
    let selfi = g.add_node("graph_instance", [0.0, 200.0]);
    g.node_mut(selfi).unwrap().params.insert("graph".into(), Value::Text("g".into()));
    let cache = Mutex::new(Cache::default());
    let c = cook(&p, 0, &Options::new(&no_res), &cache);
    let img = c.image(inst, 0).unwrap();
    assert!((img.data[0] - (1.0 - 0.0722 * 0.25 - 0.2126 * 0.25 - 0.7152 * 0.25)).abs() < 1e-4, "{}", img.data[0]);
    assert!(c.nodes[&selfi].error.as_deref().is_some_and(|e| e.contains("uses itself")), "{:?}", c.nodes[&selfi].error);
}

/// A graph `name` holding one instance of `target` wired to an output
fn instancing(name: &str, target: &str) -> (Graph, NodeId) {
    let mut g = Graph::new(name);
    g.size = [4, 4];
    let inst = g.add_node("graph_instance", [0.0, 0.0]);
    g.node_mut(inst).unwrap().params.insert("graph".into(), Value::Text(target.into()));
    let o = g.add_node("output", [100.0, 0.0]);
    g.connect(inst, "output", o, "input").unwrap();
    (g, inst)
}

#[test]
fn editing_a_nested_graph_recooks_the_top() {
    let mut c = Graph::new("c");
    c.size = [4, 4];
    let u = c.add_node("uniform", [0.0, 0.0]);
    c.node_mut(u).unwrap().params.insert("color_mode".into(), Value::Bool(false));
    c.node_mut(u).unwrap().params.insert("color".into(), Value::Color([0.2, 0.2, 0.2, 1.0]));
    let o = c.add_node("output", [100.0, 0.0]);
    c.connect(u, "output", o, "input").unwrap();
    let (b, _) = instancing("b", "c");
    let (a, top) = instancing("a", "b");
    let mut p = Package::new("p");
    p.graphs = vec![a, b, c];
    let cache = Mutex::new(Cache::default());
    let opts = Options::new(&no_res);
    let first = cook(&p, 0, &opts, &cache);
    let out_a = p.graphs[0].nodes.iter().find(|n| n.def == "output").unwrap().id;
    assert!((first.image(out_a, 0).unwrap().mean()[0] - 0.2).abs() < 0.01);
    p.graphs[2].node_mut(u).unwrap().params.insert("color".into(), Value::Color([0.8, 0.8, 0.8, 1.0]));
    let second = cook(&p, 0, &opts, &cache);
    assert!(!second.nodes[&top].cached);
    assert!((second.image(out_a, 0).unwrap().mean()[0] - 0.8).abs() < 0.01);
}

#[test]
fn deep_shared_instance_walks_stay_bounded() {
    std::thread::Builder::new()
        .stack_size(128 * 1024)
        .spawn(|| {
            let mut pkg = Package::new("deep");
            for index in 0..2000 {
                let mut graph = Graph::new(&format!("g{index}"));
                // Repeated references must visit the next graph only once
                for _ in 0..4 {
                    let id = graph.add_node("graph_instance", [0.0; 2]);
                    graph.node_mut(id).unwrap().params.insert("graph".into(), Value::Text(format!("g{}", index + 1)));
                }
                pkg.graphs.push(graph);
            }
            assert!(instances_reach(&pkg, 0, 1999));
            assert!(!instances_reach(&pkg, 0, 2000));
            let before = graph_tree_hash(&pkg, 0);
            pkg.graphs[1999].seed = 42;
            assert_ne!(before, graph_tree_hash(&pkg, 0));
            // A back edge must terminate both traversals too
            pkg.graphs[1999].nodes[0].params.insert("graph".into(), Value::Text("g0".into()));
            assert!(instances_reach(&pkg, 1, 0));
            assert!(!instances_reach(&pkg, 0, 2000));
            assert_ne!(before, graph_tree_hash(&pkg, 0));
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn mutual_instances_are_an_error() {
    let (x, xi) = instancing("x", "y");
    let (y, _) = instancing("y", "x");
    let mut p = Package::new("p");
    p.graphs = vec![x, y];
    let cache = Mutex::new(Cache::default());
    let c = cook(&p, 0, &Options::new(&no_res), &cache);
    assert!(c.nodes[&xi].error.as_deref().is_some_and(|e| e.contains("uses itself")), "{:?}", c.nodes[&xi].error);
}

#[test]
fn same_size_resources_key_differently() {
    use materialcraft_core::{Resource, ResourceBytes};
    use std::sync::atomic::AtomicU8;
    let mut g = Graph::new("g");
    g.size = [4, 4];
    let bm = g.add_node("bitmap", [0.0, 0.0]);
    g.node_mut(bm).unwrap().params.insert("resource".into(), Value::Text("tex".into()));
    let mut p = Package::new("p");
    p.graphs.push(g);
    p.resources.push(Resource::new("tex", "tex.bin", ResourceBytes::new(vec![1; 100])));
    // Stands in for decoding: the first byte picks the shade
    let shade = AtomicU8::new(1);
    let res = |_: &str| Some(Image::filled(4, 4, Kind::Gray, [f32::from(shade.load(Ordering::Relaxed)) / 10.0; 4]));
    let cache = Mutex::new(Cache::default());
    let opts = Options::new(&res);
    let first = cook(&p, 0, &opts, &cache);
    let mean = first.image(bm, 0).unwrap().mean()[0];
    p.resources[0].bytes = ResourceBytes::new(vec![2; 100]);
    shade.store(2, Ordering::Relaxed);
    let second = cook(&p, 0, &opts, &cache);
    assert!(!second.nodes[&bm].cached);
    assert!((second.image(bm, 0).unwrap().mean()[0] - mean).abs() > 0.05);
}

#[test]
fn input_nodes_read_bound_preview_images() {
    let mut p = Package::new("p");
    let mut g = Graph::new("g");
    g.size = [4, 4];
    let i = g.add_node("input_grayscale", [0.0, 0.0]);
    g.node_mut(i).unwrap().params.insert("identifier".into(), Value::Text("curvature".into()));
    let o = g.add_node("output", [100.0, 0.0]);
    g.connect(i, "output", o, "input").unwrap();
    g.input_images.insert("curvature".into(), "curv".into());
    p.graphs.push(g);
    let res = |n: &str| (n == "curv").then(|| Image::filled(4, 4, Kind::Color, [0.8, 0.8, 0.8, 1.0]));
    let cache = Mutex::new(Cache::default());
    let c = cook(&p, 0, &Options::new(&res), &cache);
    let img = c.image(o, 0).unwrap();
    assert!(img.is_gray() && (img.data[0] - 0.8).abs() < 1e-4, "bound image, as gray");
    // The caller's image wins over the bound one
    let mut opts = Options::new(&res);
    opts.inputs.insert("curvature".into(), Image::filled(4, 4, Kind::Gray, [0.1; 4]));
    let c = cook(&p, 0, &opts, &cache);
    assert!((c.image(o, 0).unwrap().data[0] - 0.1).abs() < 1e-4);
}

#[test]
fn input_change_recooks_only_readers() {
    use materialcraft_core::{FunctionGraph, FunctionNode};
    let (mut p, noise, blur, out) = sample();
    let reader = p.graphs[0].add_node("pixel_processor", [0.0, 100.0]);
    let get_a = FunctionNode { id: 1, type_code: 0, op: "get_float1".into(), inputs: vec![], value: Some(Value::Text("a".into())), pos: [0.0; 2] };
    p.graphs[0].node_mut(reader).unwrap().params.insert("function".into(), Value::Function(Box::new(FunctionGraph { nodes: vec![get_a], output: Some(1) })));
    let cache = &Mutex::new(Cache::default());
    let mut opts = Options::new(&no_res);
    opts.overrides.insert("a".into(), Value::Float(0.25));
    let first = cook(&p, 0, &opts, cache);
    assert!(first.nodes.values().all(|r| r.error.is_none() && !r.cached));
    opts.overrides.insert("a".into(), Value::Float(0.75));
    let second = cook(&p, 0, &opts, cache);
    for id in [noise, blur, out] {
        assert!(second.nodes[&id].cached, "node {} does not read `a`", id.0);
    }
    assert!(!second.nodes[&reader].cached, "the reader must re-cook");
}

#[test]
fn partial_cooks_keep_shared_dependencies_and_first_duplicate_node() {
    let mut graph = Graph::new("Partial cook");
    graph.size = [4, 4];
    let source = graph.add_node("uniform", [0.0; 2]);
    let first = graph.add_node("dot", [0.0; 2]);
    let second = graph.add_node("dot", [0.0; 2]);
    let unrelated = graph.add_node("uniform", [0.0; 2]);
    graph.connect(source, "output", first, "input").unwrap();
    graph.connect(source, "output", second, "input").unwrap();

    let mut duplicate = graph.node(source).unwrap().clone();
    duplicate.def = "input_value".into();
    graph.nodes.push(duplicate);
    let mut package = Package::new("Partial cook");
    package.graphs.push(graph);
    let cache = Mutex::new(Cache::default());
    let mut options = Options::new(&no_res);
    options.only = Some(vec![first, second]);

    let cooked = cook(&package, 0, &options, &cache);
    assert_eq!(cooked.nodes.len(), 3);
    assert!(!cooked.nodes.contains_key(&unrelated));
    for id in [source, first, second] {
        assert!(cooked.nodes[&id].error.is_none());
        assert!(cooked.image(id, 0).is_some());
    }
}

use crate::cache_key::image_key;
use std::sync::Arc;
#[test]
fn cached_input_fingerprint_tracks_copy_on_write_and_shape() {
    let cache = Mutex::new(Cache::default());
    let mut image = Image::filled(128, 128, Kind::Gray, [0.0; 4]);
    let first = image_key(&image, &cache);
    assert_eq!(image_key(&image, &cache), first);
    Arc::make_mut(&mut image.data)[1] = 1.0;
    let changed = image_key(&image, &cache);
    assert_ne!(changed, first);
    image.w = 64;
    image.h = 256;
    let reshaped = image_key(&image, &cache);
    assert_ne!(reshaped, changed);

    cache.lock().unwrap().clear();
    assert_eq!(image_key(&image, &cache), reshaped);
}
#[test]
fn preview_cap_does_not_compound_downscaling() {
    let (mut p, noise, blur, out) = sample();
    p.graphs[0].size = [10, 10];
    p.graphs[0].node_mut(blur).unwrap().base.size = [-3, -3];
    let cache = Mutex::new(Cache::default());
    let mut opts = Options::new(&no_res);
    opts.max_size = Some(512);
    let c = cook(&p, 0, &opts, &cache);
    assert_eq!(c.image(noise, 0).map(|i| (i.w, i.h)), Some((512, 512)));
    assert_eq!(c.image(blur, 0).map(|i| (i.w, i.h)), Some((128, 128)));
    assert_eq!(c.image(out, 0).map(|i| (i.w, i.h)), Some((128, 128)));
}
#[test]
fn preview_cap_uses_nominal_dimensions_through_bypassed_nodes() {
    let (mut p, noise, blur, out) = sample();
    p.graphs[0].size = [13, 12];
    p.graphs[0].node_mut(blur).unwrap().disabled = true;
    p.graphs[0].node_mut(out).unwrap().base.size = [1, 0];
    let cache = Mutex::new(Cache::default());
    let mut opts = Options::new(&no_res);
    opts.max_size = Some(128);
    let c = cook(&p, 0, &opts, &cache);
    assert_eq!(c.image(noise, 0).map(|i| (i.w, i.h)), Some((128, 64)));
    assert_eq!(c.image(blur, 0).map(|i| (i.w, i.h)), Some((128, 64)));
    assert_eq!(c.image(out, 0).map(|i| (i.w, i.h)), Some((128, 64)));
}
#[test]
fn changed_unsampled_input_recooks() {
    let mut g = Graph::new("g");
    g.size = [7, 7];
    let input = g.add_node("input_grayscale", [0.0, 0.0]);
    let mut p = Package::new("p");
    p.graphs.push(g);
    let mut opts = Options::new(&no_res);
    let image = Image::filled(128, 128, Kind::Gray, [0.0; 4]);
    opts.inputs.insert("input".into(), image.clone());
    let cache = Mutex::new(Cache::default());
    let first = cook(&p, 0, &opts, &cache);
    assert!(!first.nodes[&input].cached);
    let mut changed = image;
    Arc::make_mut(&mut changed.data)[1] = 1.0;
    opts.inputs.insert("input".into(), changed);
    let second = cook(&p, 0, &opts, &cache);
    assert!(!second.nodes[&input].cached);
    assert_eq!(second.image(input, 0).unwrap().data[1], 1.0);
}
#[test]
fn linear_input_images_and_bitmap_nodes_request_data_reads() {
    // The decoder returns 0.5 for a linear request and 0.2 for an sRGB one.
    let res = |k: &str| {
        let (name, linear) = materialcraft_image::split_resource_key(k);
        (name == "m").then(|| Image::filled(4, 4, Kind::Color, [if linear { 0.5 } else { 0.2 }; 4]))
    };
    let mut p = Package::new("p");
    let mut g = Graph::new("g");
    g.size = [4, 4];
    let i = g.add_node("input_color", [0.0, 0.0]);
    g.node_mut(i).unwrap().params.insert("identifier".into(), Value::Text("n".into()));
    let b = g.add_node("bitmap", [0.0, 100.0]);
    g.node_mut(b).unwrap().params.insert("resource".into(), Value::Text("m".into()));
    g.input_images.insert("n".into(), "m".into());
    p.graphs.push(g);
    let first = |p: &Package| {
        let cache = Mutex::new(Cache::default());
        let c = cook(p, 0, &Options::new(&res), &cache);
        (c.image(i, 0).unwrap().data[0], c.image(b, 0).unwrap().data[0])
    };
    assert_eq!(first(&p), (0.2, 0.2), "default is sRGB, as before");
    p.graphs[0].input_images_linear.insert("n".into());
    p.graphs[0].node_mut(b).unwrap().params.insert("colour_space".into(), Value::Enum(1));
    assert_eq!(first(&p), (0.5, 0.5));
}

#[test]
fn bound_input_image_resource_change_recooks_instance() {
    use materialcraft_core::{Resource, ResourceBytes};
    use std::sync::atomic::AtomicU8;
    let mut sub = Graph::new("sub");
    sub.size = [4, 4];
    let i = sub.add_node("input_grayscale", [0.0, 0.0]);
    sub.node_mut(i).unwrap().params.insert("identifier".into(), Value::Text("in".into()));
    let o = sub.add_node("output", [100.0, 0.0]);
    sub.connect(i, "output", o, "input").unwrap();
    sub.input_images.insert("in".into(), "curv".into());
    let (top, inst) = instancing("top", "sub");
    let mut p = Package::new("p");
    p.graphs = vec![top, sub];
    p.resources.push(Resource::new("curv", "curv.bin", ResourceBytes::new(vec![1; 100])));
    // Stands in for decoding: the first byte picks the shade.
    let shade = AtomicU8::new(1);
    let res = |_: &str| Some(Image::filled(4, 4, Kind::Gray, [f32::from(shade.load(Ordering::Relaxed)) / 10.0; 4]));
    let cache = Mutex::new(Cache::default());
    let opts = Options::new(&res);
    let first = cook(&p, 0, &opts, &cache);
    assert!(first.nodes[&inst].error.is_none(), "{:?}", first.nodes[&inst].error);
    assert!(!first.nodes[&inst].cached);
    let unchanged = cook(&p, 0, &opts, &cache);
    assert!(unchanged.nodes[&inst].cached, "an unchanged instance should be served from the cache");
    p.resources[0].bytes = ResourceBytes::new(vec![2; 100]);
    shade.store(2, Ordering::Relaxed);
    let second = cook(&p, 0, &opts, &cache);
    assert!(!second.nodes[&inst].cached, "changing the bound input image resource must recook the instance");
}

#[test]
fn bound_input_image_linear_flag_change_recooks_instance() {
    use materialcraft_core::{Resource, ResourceBytes};
    let mut sub = Graph::new("sub");
    sub.size = [4, 4];
    let i = sub.add_node("input_grayscale", [0.0, 0.0]);
    sub.node_mut(i).unwrap().params.insert("identifier".into(), Value::Text("in".into()));
    let o = sub.add_node("output", [100.0, 0.0]);
    sub.connect(i, "output", o, "input").unwrap();
    sub.input_images.insert("in".into(), "curv".into());
    let (top, inst) = instancing("top", "sub");
    let mut p = Package::new("p");
    p.graphs = vec![top, sub];
    p.resources.push(Resource::new("curv", "curv.bin", ResourceBytes::new(vec![1; 100])));
    let res = |k: &str| {
        let (name, linear) = materialcraft_image::split_resource_key(k);
        (name == "curv").then(|| Image::filled(4, 4, Kind::Gray, [if linear { 0.5 } else { 0.2 }; 4]))
    };
    let cache = Mutex::new(Cache::default());
    let opts = Options::new(&res);
    let first = cook(&p, 0, &opts, &cache);
    assert!(first.nodes[&inst].error.is_none(), "{:?}", first.nodes[&inst].error);
    assert!(!first.nodes[&inst].cached);
    let first_value = first.image(inst, 0).unwrap().data[0];
    assert!((first_value - 0.2).abs() < 1e-4, "sRGB bound image should reach the instance, got {first_value}");
    let second = cook(&p, 0, &opts, &cache);
    assert!(second.nodes[&inst].cached, "an unchanged instance should be served from the cache");
    p.graphs[1].input_images_linear.insert("in".into());
    let third = cook(&p, 0, &opts, &cache);
    assert!(!third.nodes[&inst].cached, "toggling input_images_linear must recook the instance");
    let third_value = third.image(inst, 0).unwrap().data[0];
    assert!((third_value - 0.5).abs() < 1e-4, "linear bound image should reach the instance, got {third_value}");
}
