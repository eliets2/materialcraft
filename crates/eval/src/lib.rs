//! # materialcraft-eval
//!
//! Graph cooking, L2. [`cook`] walks a graph in dependency order, works out each node's output
//! size (relative to its input or the graph, or absolute, under an optional preview cap), resolves
//! parameters (defaults, overrides, bindings to exposed inputs) and runs the node's cook. Results
//! live in a content-addressed [`Cache`]: a node's key hashes its definition, parameters, size,
//! seed, tiling and its inputs' keys, so editing one node re-cooks only what lies downstream, and
//! undo/redo finds earlier results again. Graph instances cook their sub-graph recursively.
#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use std::collections::{BTreeMap, HashMap, HashSet};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Weak};

use materialcraft_core::{Graph, Node, NodeId, Package, SizeMode, Value};
use materialcraft_image::{Image, Kind};
use materialcraft_nodes::{Ctx, NodeDef, PortKind, clamp_params, find, resolve_params};

/// Sub-graphs may nest this deep (a graph instancing itself would recurse forever).
pub const MAX_DEPTH: u32 = 8;

/// A port as the editor and the evaluator see it (catalog ports, or a sub-graph's inputs and
/// outputs for a graph instance).
#[derive(Clone, Debug, PartialEq)]
pub struct Port {
    pub id: String,
    pub label: String,
    pub kind: PortKind,
}

/// The input and output ports of `node` in `pkg`.
pub fn ports(pkg: &Package, node: &Node) -> (Vec<Port>, Vec<Port>) {
    if node.def == "graph_instance" {
        let Some((_, sub)) = instance_graph(pkg, node) else { return (Vec::new(), Vec::new()) };
        let io = |def: &str| -> Vec<Port> {
            sub.nodes
                .iter()
                .filter(|n| n.def == def || (def == "input" && n.def.starts_with("input_")))
                .map(|n| {
                    let id = identifier(n);
                    let kind = if n.def == "input_grayscale" {
                        PortKind::Gray
                    } else if n.def == "input_color" {
                        PortKind::Color
                    } else {
                        PortKind::Any
                    };
                    Port { label: id.clone(), id, kind }
                })
                .collect()
        };
        return (io("input"), io("output"));
    }
    if node.def == materialcraft_core::MISSING_NODE {
        let (ins, outs) = materialcraft_core::missing_ports(node);
        let ins = ins.into_iter().map(|id| Port { label: id.clone(), id, kind: PortKind::Any }).collect();
        let outs = outs.into_iter().map(|(id, gray)| Port { label: id.clone(), id, kind: if gray { PortKind::Gray } else { PortKind::Color } }).collect();
        return (ins, outs);
    }
    let Some(def) = find(&node.def) else { return (Vec::new(), Vec::new()) };
    let conv = |p: &materialcraft_nodes::PortDef| Port { id: p.id.into(), label: p.label.into(), kind: p.kind };
    (def.inputs.iter().map(conv).collect(), def.outputs.iter().map(conv).collect())
}

/// The graph a graph-instance node refers to.
pub fn instance_graph<'a>(pkg: &'a Package, node: &Node) -> Option<(usize, &'a Graph)> {
    match node.params.get("graph") {
        Some(Value::Text(name)) => pkg.graph_by_name(name),
        _ => None,
    }
}

/// An input or output node's identifier (its `identifier` parameter, default "input"/"output").
pub fn identifier(n: &Node) -> String {
    match n.params.get("identifier") {
        Some(Value::Text(s)) if !s.is_empty() => s.clone(),
        _ => if n.def == "output" { "output" } else { "input" }.into(),
    }
}

/// One node's cooked result.
#[derive(Clone, Debug)]
pub struct NodeResult {
    /// One image per output port (empty when the cook failed).
    pub outputs: Arc<Vec<Image>>,
    pub error: Option<String>,
    /// Cook time in milliseconds (0 when served from the cache).
    pub ms: f32,
    pub key: u64,
    pub cached: bool,
}

/// Results of a cook, by node.
#[derive(Clone, Debug, Default)]
pub struct Cooked {
    pub nodes: BTreeMap<NodeId, NodeResult>,
    /// Total wall time of the cook in milliseconds.
    pub ms: f32,
    pub cancelled: bool,
}

impl Cooked {
    /// Output `port` of node `id`.
    pub fn image(&self, id: NodeId, port: usize) -> Option<&Image> {
        self.nodes.get(&id).and_then(|r| r.outputs.get(port))
    }
}

type InputFingerprint = (Weak<Vec<f32>>, (u32, u32, usize), u64);

/// LRU cache of node outputs, bounded by bytes.
pub struct Cache {
    entries: HashMap<u64, (Arc<Vec<Image>>, usize, u64)>,
    bytes: usize,
    cap: usize,
    tick: u64,
    input_hashes: HashMap<usize, InputFingerprint>,
}

impl Default for Cache {
    fn default() -> Self {
        Cache::new(1 << 30)
    }
}

impl Cache {
    pub fn new(cap_bytes: usize) -> Cache {
        Cache { entries: HashMap::new(), bytes: 0, cap: cap_bytes, tick: 0, input_hashes: HashMap::new() }
    }

    pub fn bytes(&self) -> usize {
        self.bytes
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.input_hashes.clear();
        self.bytes = 0;
    }

    fn get(&mut self, key: u64) -> Option<Arc<Vec<Image>>> {
        self.tick += 1;
        let t = self.tick;
        self.entries.get_mut(&key).map(|e| {
            e.2 = t;
            e.0.clone()
        })
    }

    fn put(&mut self, key: u64, v: Arc<Vec<Image>>) {
        let size: usize = v.iter().map(|i| i.data.len() * 4).sum();
        self.tick += 1;
        if let Some(old) = self.entries.insert(key, (v, size, self.tick)) {
            self.bytes = self.bytes.saturating_sub(old.1);
        }
        self.bytes += size;
        while self.bytes > self.cap && self.entries.len() > 1 {
            let Some((&k, _)) = self.entries.iter().min_by_key(|(_, e)| e.2) else { break };
            if let Some(e) = self.entries.remove(&k) {
                self.bytes = self.bytes.saturating_sub(e.1);
            }
        }
    }
}

/// What to cook and how.
pub struct Options<'a> {
    /// Cap on the larger side of every node's output (previews); `None` = full size.
    pub max_size: Option<u32>,
    /// Cook only these nodes and what they depend on (`None` = the whole graph).
    pub only: Option<Vec<NodeId>>,
    /// Images for the graph's input nodes, by identifier (graph instances).
    pub inputs: BTreeMap<String, Image>,
    /// Values for the graph's exposed parameters (graph instances), over the graph's own.
    pub overrides: BTreeMap<String, Value>,
    /// Decodes a package resource (bitmap) by name.
    pub resource: &'a (dyn Fn(&str) -> Option<Image> + Sync),
    /// Set from another thread to stop between nodes.
    pub cancel: Option<&'a AtomicBool>,
    pub depth: u32,
    /// Graphs being cooked around this one, outermost first (cycle detection).
    pub graph_stack: Vec<usize>,
}

impl<'a> Options<'a> {
    pub fn new(resource: &'a (dyn Fn(&str) -> Option<Image> + Sync)) -> Options<'a> {
        Options { max_size: None, only: None, inputs: BTreeMap::new(), overrides: BTreeMap::new(), resource, cancel: None, depth: 0, graph_stack: Vec::new() }
    }
}

/// The magenta image a failed node shows.
pub fn error_image(w: u32, h: u32) -> Image {
    Image::filled(w, h, Kind::Color, [1.0, 0.0, 1.0, 1.0])
}

/// Fingerprint all input values. Reuse the fingerprint while the immutable backing buffer
/// remains alive; Weak guards against allocator address reuse and Arc copy-on-write mutations.
fn image_key(i: &Image, cache: &Mutex<Cache>) -> u64 {
    let ptr = Arc::as_ptr(&i.data) as usize;
    let shape = (i.w, i.h, i.kind.channels());
    if let Some(key) = lock(cache)
        .input_hashes
        .get(&ptr)
        .and_then(|(weak, saved_shape, key)| weak.upgrade().filter(|data| Arc::ptr_eq(data, &i.data) && *saved_shape == shape).map(|_| *key))
    {
        return key;
    }
    let mut h = DefaultHasher::new();
    (shape, i.data.len()).hash(&mut h);
    for value in i.data.iter() {
        value.to_bits().hash(&mut h);
    }
    let key = h.finish();
    let mut cache = lock(cache);
    if cache.input_hashes.len() >= 128 {
        cache.input_hashes.retain(|_, (weak, _, _)| weak.strong_count() > 0);
        if cache.input_hashes.len() >= 128 {
            cache.input_hashes.clear();
        }
    }
    cache.input_hashes.insert(ptr, (Arc::downgrade(&i.data), shape, key));
    key
}

fn hash_of(v: &impl Hash) -> u64 {
    let mut h = DefaultHasher::new();
    v.hash(&mut h);
    h.finish()
}

/// Output size of `node` given its first connected input's size.
pub fn node_size(graph: &Graph, def: Option<&NodeDef>, node: &Node, input: Option<(u32, u32)>, max: Option<u32>) -> (u32, u32) {
    let parent = graph.size_px();
    let (bw, bh) = match node.base.size_mode {
        SizeMode::Absolute => {
            let s = |l: i32| 1u32 << l.clamp(4, 13);
            (s(node.base.size[0]), s(node.base.size[1]))
        }
        mode => {
            let base = match (mode, input, def.is_some_and(|d| d.generator)) {
                (SizeMode::RelativeToInput, Some(i), false) => i,
                _ => (parent[0], parent[1]),
            };
            let sh = |v: u32, o: i32| {
                let v = v as f64 * 2f64.powi(o.clamp(-12, 12));
                v.round().clamp(16.0, 8192.0) as u32
            };
            (sh(base.0, node.base.size[0]), sh(base.1, node.base.size[1]))
        }
    };
    match max {
        Some(m) if bw.max(bh) > m => {
            let k = m as f64 / bw.max(bh) as f64;
            (((bw as f64 * k).round() as u32).max(1), ((bh as f64 * k).round() as u32).max(1))
        }
        _ => (bw, bh),
    }
}

/// A function result (4 lanes) as a value of `like`'s type.
pub fn function_value(like: &Value, v: [f32; 4]) -> Option<Value> {
    let f = |x: f32| if x.is_finite() { x } else { 0.0 };
    Some(match like {
        Value::Float(_) => Value::Float(f(v[0])),
        Value::Float2(_) => Value::Float2([f(v[0]), f(v[1])]),
        Value::Float3(_) => Value::Float3([f(v[0]), f(v[1]), f(v[2])]),
        Value::Float4(_) => Value::Float4(v.map(f)),
        Value::Color(_) => Value::Color(v.map(f)),
        Value::Int(_) => Value::Int(f(v[0]).round().clamp(i32::MIN as f32, i32::MAX as f32) as i32),
        Value::Int2(_) => Value::Int2([f(v[0]).round() as i32, f(v[1]).round() as i32]),
        Value::Bool(_) => Value::Bool(f(v[0]) != 0.0),
        Value::Enum(_) => Value::Enum(f(v[0]).round().max(0.0) as u32),
        _ => return None,
    })
}

/// Lock the cache, recovering from a poisoned lock (a cache entry is never half-written).
fn lock(cache: &Mutex<Cache>) -> std::sync::MutexGuard<'_, Cache> {
    cache.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Cook graph `gi` of `pkg`. The cache is locked only around lookups and inserts, never while a
/// node cooks, so other users of the cache are not blocked for the length of a cook.
pub fn cook(pkg: &Package, gi: usize, opts: &Options, cache: &Mutex<Cache>) -> Cooked {
    let t0 = now_ms();
    let mut out = Cooked::default();
    let Some(graph) = pkg.graphs.get(gi) else { return out };
    // The top-level graph's input nodes show their preview images; the caller's images win.
    let bound;
    let opts = if opts.depth == 0 && !graph.input_images.is_empty() {
        let mut inputs: BTreeMap<String, Image> = graph
            .input_images
            .iter()
            .filter_map(|(id, res)| Some((id.clone(), (opts.resource)(&materialcraft_image::resource_key(res, graph.input_images_linear.contains(id)))?)))
            .collect();
        inputs.extend(opts.inputs.iter().map(|(k, v)| (k.clone(), v.clone())));
        bound = Options { only: opts.only.clone(), inputs, overrides: opts.overrides.clone(), graph_stack: opts.graph_stack.clone(), ..*opts };
        &bound
    } else {
        opts
    };
    let order = graph.topo_order();
    let needed: Option<std::collections::BTreeSet<NodeId>> = opts.only.as_ref().map(|ids| {
        let mut s: std::collections::BTreeSet<NodeId> = ids.iter().copied().collect();
        for id in ids {
            s.extend(graph.upstream(*id));
        }
        s
    });
    // Exposed parameter values: the graph's own, then the caller's overrides.
    let mut vars: BTreeMap<String, Value> = graph.exposed.iter().filter_map(|e| graph.input_value(&e.id).map(|v| (e.id.clone(), v.clone()))).collect();
    for (k, v) in &opts.overrides {
        vars.insert(k.clone(), v.clone());
    }
    let mut nominal_sizes = BTreeMap::new();
    for id in order {
        if needed.as_ref().is_some_and(|s| !s.contains(&id)) {
            continue;
        }
        if opts.cancel.is_some_and(|c| c.load(Ordering::Relaxed)) {
            out.cancelled = true;
            break;
        }
        let Ok(node) = graph.node(id) else { continue };
        let (r, nominal) = cook_node(pkg, gi, graph, node, &out, &nominal_sizes, &vars, opts, cache);
        nominal_sizes.insert(id, nominal);
        out.nodes.insert(id, r);
    }
    out.ms = (now_ms() - t0) as f32;
    out
}

/// Hash each reachable graph and its resource content once. An iterative walk keeps
/// deep chains and graphs with many shared instances bounded by the package size.
fn graph_tree_hash(pkg: &Package, gi: usize) -> u64 {
    let mut h = DefaultHasher::new();
    let mut pending = vec![gi];
    let mut seen = HashSet::new();
    while let Some(index) = pending.pop() {
        if !seen.insert(index) {
            continue;
        }
        let Some(graph) = pkg.graphs.get(index) else { continue };
        index.hash(&mut h);
        serde_json::to_string(graph).unwrap_or_default().hash(&mut h);
        for res in graph.input_images.values() {
            pkg.resource(res).map(|r| r.bytes.content_hash()).hash(&mut h);
        }
        for n in &graph.nodes {
            if let Some(Value::Text(name)) = n.params.get("resource") {
                pkg.resource(name).map(|r| r.bytes.content_hash()).hash(&mut h);
            }
            if n.def == "graph_instance"
                && let Some((si, _)) = instance_graph(pkg, n)
            {
                pending.push(si);
            }
        }
    }
    h.finish()
}

/// The graph inputs a node's cook can read: those named by `get_*` ops of its function-valued
/// parameters. Bindings and node functions are already folded into `params` before the key is
/// computed, so these are the only remaining readers of `vars` (`Ctx::vars` in the function nodes).
/// Matching every `get_*` op, not only `get_var`, over-approximates on purpose.
fn vars_read<'v>(params: &BTreeMap<String, Value>, vars: &'v BTreeMap<String, Value>) -> Vec<(&'v str, &'v Value)> {
    let mut read: Vec<(&str, &Value)> = params
        .values()
        .filter_map(|v| if let Value::Function(g) = v { Some(g) } else { None })
        .flat_map(|g| g.nodes.iter())
        .filter(|n| n.op.starts_with("get_"))
        .filter_map(|n| if let Some(Value::Text(name)) = &n.value { vars.get_key_value(name.as_str()) } else { None })
        .map(|(k, v)| (k.as_str(), v))
        .collect();
    read.sort_by_key(|(k, _)| *k);
    read.dedup_by_key(|(k, _)| *k);
    read
}

fn cook_node(
    pkg: &Package,
    gi: usize,
    graph: &Graph,
    node: &Node,
    done: &Cooked,
    nominal_sizes: &BTreeMap<NodeId, (u32, u32)>,
    vars: &BTreeMap<String, Value>,
    opts: &Options,
    cache: &Mutex<Cache>,
) -> (NodeResult, (u32, u32)) {
    let (in_ports, out_ports) = ports(pkg, node);
    let def = find(&node.def);
    // Gather inputs by port order, with their keys.
    let mut inputs: Vec<Option<Image>> = Vec::with_capacity(in_ports.len());
    let mut in_keys: Vec<u64> = Vec::with_capacity(in_ports.len());
    let mut first_nominal = None;
    for p in &in_ports {
        let src = graph.input_of(node.id, &p.id).and_then(|c| {
            let r = done.nodes.get(&c.from)?;
            let from = graph.node(c.from).ok()?;
            let idx = ports(pkg, from).1.iter().position(|o| o.id == c.from_port)?;
            let image = r.outputs.get(idx)?.clone();
            let nominal = nominal_sizes.get(&c.from).copied().unwrap_or((image.w, image.h));
            Some((image, r.key ^ hash_of(&idx), nominal))
        });
        in_keys.push(src.as_ref().map_or(0, |s| s.1));
        if first_nominal.is_none() {
            first_nominal = src.as_ref().map(|s| s.2);
        }
        inputs.push(src.map(|s| s.0));
    }
    let nominal =
        if node.disabled { first_nominal.unwrap_or_else(|| node_size(graph, def, node, None, None)) } else { node_size(graph, def, node, first_nominal, None) };
    let (w, h) = node_size(graph, def, node, first_nominal, opts.max_size);
    let fail = |msg: String, key: u64| NodeResult {
        outputs: Arc::new(vec![error_image(w, h); out_ports.len().max(1)]),
        error: Some(msg),
        ms: 0.0,
        key,
        cached: false,
    };
    // Parameters: defaults, overrides, then bindings to exposed inputs.
    let mut params = match def {
        Some(d) if node.def != "graph_instance" => resolve_params(d, &node.params),
        _ => node.params.clone(),
    };
    for (param, exposed) in &node.bindings {
        if let (Some(v), Some(cur)) = (vars.get(exposed), params.get(param))
            && let Some(c) = cur.coerce_json(&serde_json::to_value(v).unwrap_or_default())
        {
            params.insert(param.clone(), c);
        }
    }
    let seed = (graph.seed as u32).wrapping_mul(0x9e37_79b9).wrapping_add(node.base.seed as u32);
    // Dynamic parameters: functions of the graph's inputs, evaluated once per cook.
    for (param, f) in &node.functions {
        let Some(cur) = params.get(param) else { continue };
        match materialcraft_nodes::function::compile(f, vars, seed) {
            Ok(prog) => {
                let v = prog.eval([0.5, 0.5], [w as f32, h as f32], &[], true, &mut Vec::new());
                if let Some(c) = function_value(cur, v) {
                    params.insert(param.clone(), c);
                }
            }
            Err(e) => return (fail(format!("function of `{param}`: {e}"), 0), nominal),
        }
    }
    // Hard limits also apply after bindings and dynamic functions.
    if let Some(d) = def
        && node.def != "graph_instance"
    {
        clamp_params(d, &mut params);
    }
    let tile = node.base.tiling[0] && node.base.tiling[1];
    // Input nodes read the caller's image for their identifier.
    let input_img = node.def.starts_with("input_").then(|| opts.inputs.get(&identifier(node))).flatten();
    let res_hash = match params.get("resource") {
        Some(Value::Text(name)) => pkg.resource(name).map_or(0, |r| r.bytes.content_hash()),
        _ => 0,
    };
    let sub_hash = if node.def == "graph_instance" { instance_graph(pkg, node).map_or(0, |(si, _)| graph_tree_hash(pkg, si)) } else { 0 };
    let key = hash_of(&(
        &node.def,
        serde_json::to_string(&params).unwrap_or_default(),
        (w, h, seed, tile, node.disabled),
        &in_keys,
        res_hash,
        sub_hash,
        input_img.map(|image| image_key(image, cache)),
        serde_json::to_string(&vars_read(&params, vars)).unwrap_or_default(),
    ));
    if let Some(v) = lock(cache).get(key) {
        return (NodeResult { outputs: v, error: None, ms: 0.0, key, cached: true }, nominal);
    }
    let t0 = now_ms();
    let result: Result<Vec<Image>, String> = if node.disabled {
        Ok(vec![inputs.iter().flatten().next().cloned().unwrap_or_else(|| Image::filled(w, h, Kind::Gray, [0.0; 4])); out_ports.len().max(1)])
    } else if let Some(img) = input_img {
        let img = img.resize(w, h, tile);
        Ok(vec![if node.def == "input_grayscale" { img.to_gray() } else { img }])
    } else if node.def == materialcraft_core::MISSING_NODE {
        let name = if node.label.is_empty() { "this node" } else { node.label.as_str() };
        Err(format!("`{name}` is not in MaterialCraft's library yet: kept as a placeholder from the SBS file"))
    } else if node.def == "graph_instance" {
        cook_instance(pkg, gi, node, &in_ports, &out_ports, &inputs, &params, (w, h), opts, cache)
    } else if let Some(d) = def {
        let ctx = Ctx { w, h, seed, tile, inputs: &inputs, params: &params, resource: opts.resource, vars };
        (d.cook)(&ctx)
    } else {
        Err(format!("unknown node `{}`", node.def))
    };
    let result = match result {
        Ok(imgs) if imgs.len() == out_ports.len().max(1) || out_ports.is_empty() => {
            let v = Arc::new(imgs);
            lock(cache).put(key, v.clone());
            NodeResult { outputs: v, error: None, ms: (now_ms() - t0) as f32, key, cached: false }
        }
        Ok(imgs) => fail(format!("`{}` made {} images for {} outputs", node.def, imgs.len(), out_ports.len()), key),
        Err(e) => fail(e, key),
    };
    (result, nominal)
}

/// Whether graph `from` reaches `target`, without recursive stack growth.
fn instances_reach(pkg: &Package, from: usize, target: usize) -> bool {
    let mut pending = vec![from];
    let mut seen = HashSet::new();
    while let Some(index) = pending.pop() {
        if !seen.insert(index) {
            continue;
        }
        let Some(graph) = pkg.graphs.get(index) else { continue };
        for (si, _) in graph.nodes.iter().filter(|n| n.def == "graph_instance").filter_map(|n| instance_graph(pkg, n)) {
            if si == target {
                return true;
            }
            pending.push(si);
        }
    }
    false
}

fn cook_instance(
    pkg: &Package,
    gi: usize,
    node: &Node,
    in_ports: &[Port],
    out_ports: &[Port],
    inputs: &[Option<Image>],
    params: &BTreeMap<String, Value>,
    size: (u32, u32),
    opts: &Options,
    cache: &Mutex<Cache>,
) -> Result<Vec<Image>, String> {
    if opts.depth >= MAX_DEPTH {
        return Err("graph instances nest too deep (does a graph use itself?)".into());
    }
    let (sub_gi, sub) = instance_graph(pkg, node).ok_or("the instanced graph does not exist")?;
    if sub_gi == gi || opts.graph_stack.contains(&sub_gi) || instances_reach(pkg, sub_gi, gi) {
        return Err(format!("graph `{}` uses itself through a graph instance", sub.name));
    }
    let mut o = Options::new(opts.resource);
    o.max_size = opts.max_size;
    o.cancel = opts.cancel;
    o.depth = opts.depth + 1;
    o.graph_stack = opts.graph_stack.clone();
    o.graph_stack.push(gi);
    o.inputs = in_ports.iter().zip(inputs).filter_map(|(p, i)| Some((p.id.clone(), i.clone()?))).collect();
    o.overrides = params.iter().filter(|(k, _)| sub.exposed.iter().any(|e| &e.id == *k)).map(|(k, v)| (k.clone(), v.clone())).collect();
    let cooked = cook(pkg, sub_gi, &o, cache);
    if cooked.cancelled {
        return Err("cancelled".into());
    }
    Ok(out_ports
        .iter()
        .map(|p| {
            sub.nodes
                .iter()
                .find(|n| n.def == "output" && identifier(n) == p.id)
                .and_then(|n| cooked.image(n.id, 0))
                .map(|i| i.resize(size.0, size.1, true))
                .unwrap_or_else(|| Image::filled(size.0, size.1, Kind::Gray, [0.0; 4]))
        })
        .collect())
}

/// The graph's output nodes with their usage id and image, in node order.
pub fn outputs<'c>(graph: &Graph, cooked: &'c Cooked) -> Vec<(NodeId, &'static str, String, Option<&'c Image>)> {
    graph
        .nodes
        .iter()
        .filter(|n| n.def == "output")
        .map(|n| {
            let usage = n.params.get("usage").and_then(Value::as_i32).unwrap_or(0).clamp(0, materialcraft_nodes::atomic::USAGE_IDS.len() as i32 - 1) as usize;
            (n.id, materialcraft_nodes::atomic::USAGE_IDS.get(usage).copied().unwrap_or("any"), identifier(n), cooked.image(n.id, 0))
        })
        .collect()
}

/// Milliseconds since the first call (0 on the web, where `std::time::Instant` is unavailable).
pub fn now_ms() -> f64 {
    #[cfg(not(target_arch = "wasm32"))]
    {
        static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
        START.get_or_init(std::time::Instant::now).elapsed().as_secs_f64() * 1000.0
    }
    #[cfg(target_arch = "wasm32")]
    0.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use materialcraft_core::Graph;

    fn no_res(_: &str) -> Option<Image> {
        None
    }

    fn sample() -> (Package, NodeId, NodeId, NodeId) {
        let mut g = Graph::new("g");
        g.size = [6, 6];
        let noise = g.add_node("perlin_noise", [0.0, 0.0]);
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
        // Same graph: everything from the cache.
        let b = cook(&p, 0, &opts, &cache);
        assert!(b.nodes.values().all(|r| r.cached));
        // Editing the blur re-cooks the blur and the output, not the noise.
        p.graphs[0].node_mut(blur).unwrap().params.insert("intensity".into(), Value::Float(5.0));
        let c = cook(&p, 0, &opts, &cache);
        assert!(c.nodes[&noise].cached && !c.nodes[&blur].cached && !c.nodes[&out].cached);
        assert_eq!(outputs(&p.graphs[0], &c).len(), 1);
    }

    #[test]
    fn hostile_param_is_clamped_before_cooking() {
        let (mut p, noise, _, _) = sample();
        let blur = p.graphs[0].add_node("anisotropic_blur", [150.0, 100.0]);
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
    fn input_image_cache_key_covers_every_pixel() {
        let a = Image::filled(128, 128, Kind::Gray, [0.0; 4]);
        let mut b = a.clone();
        Arc::make_mut(&mut b.data)[1] = 1.0;
        let cache = Mutex::new(Cache::default());
        assert_ne!(image_key(&a, &cache), image_key(&b, &cache));
    }

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
        assert_ne!(image_key(&image, &cache), changed);
        lock(&cache).clear();
        assert_eq!(image_key(&image, &cache), image_key(&image, &cache));
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
    fn graph_instances_cook_their_subgraph() {
        let (mut p, ..) = sample();
        // A second graph: input → invert → output, instanced from the first.
        let mut sub = Graph::new("inv");
        sub.size = [6, 6];
        let i = sub.add_node("input_grayscale", [0.0, 0.0]);
        let inv = sub.add_node("invert", [100.0, 0.0]);
        let o = sub.add_node("output", [200.0, 0.0]);
        sub.connect(i, "output", inv, "input").unwrap();
        sub.connect(inv, "output", o, "input").unwrap();
        p.graphs.push(sub);
        let g = &mut p.graphs[0];
        let u = g.add_node("uniform", [0.0, 100.0]);
        g.node_mut(u).unwrap().params.insert("mode".into(), Value::Enum(1));
        g.node_mut(u).unwrap().params.insert("color".into(), Value::Color([0.25, 0.25, 0.25, 1.0]));
        let inst = g.add_node("graph_instance", [100.0, 100.0]);
        g.node_mut(inst).unwrap().params.insert("graph".into(), Value::Text("inv".into()));
        g.connect(u, "output", inst, "input").unwrap();
        // A graph instancing itself is refused, not a stack overflow.
        let selfi = g.add_node("graph_instance", [0.0, 200.0]);
        g.node_mut(selfi).unwrap().params.insert("graph".into(), Value::Text("g".into()));
        let cache = Mutex::new(Cache::default());
        let c = cook(&p, 0, &Options::new(&no_res), &cache);
        let img = c.image(inst, 0).unwrap();
        assert!((img.data[0] - (1.0 - 0.0722 * 0.25 - 0.2126 * 0.25 - 0.7152 * 0.25)).abs() < 1e-4, "{}", img.data[0]);
        assert!(c.nodes[&selfi].error.as_deref().is_some_and(|e| e.contains("uses itself")), "{:?}", c.nodes[&selfi].error);
    }

    /// A graph `name` holding one instance of `target` wired to an output.
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
        c.node_mut(u).unwrap().params.insert("mode".into(), Value::Enum(1));
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
                    // Repeated references must visit the next graph only once.
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
                // A back edge must terminate both traversals too.
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
        // Stands in for decoding: the first byte picks the shade.
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

    fn uniform_graph(name: &str, v: f32) -> Graph {
        let mut g = Graph::new(name);
        g.size = [4, 4];
        let u = g.add_node("uniform", [0.0, 0.0]);
        g.node_mut(u).unwrap().params.insert("mode".into(), Value::Enum(1));
        g.node_mut(u).unwrap().params.insert("color".into(), Value::Color([v, v, v, 1.0]));
        let o = g.add_node("output", [100.0, 0.0]);
        g.connect(u, "output", o, "input").unwrap();
        g
    }

    fn instance_chain(from: &str, to: &str) -> Graph {
        let mut g = Graph::new(from);
        g.size = [4, 4];
        let i = g.add_node("graph_instance", [0.0, 0.0]);
        g.node_mut(i).unwrap().params.insert("graph".into(), Value::Text(to.into()));
        let o = g.add_node("output", [100.0, 0.0]);
        g.connect(i, "output", o, "input").unwrap();
        g
    }

    /// Editing a graph that is instanced two levels down must change the top graph's result.
    #[test]
    fn nested_instances_see_edits_to_the_innermost_graph() {
        let mut p = Package::new("p");
        p.graphs.push(instance_chain("top", "mid"));
        p.graphs.push(instance_chain("mid", "leaf"));
        p.graphs.push(uniform_graph("leaf", 0.2));
        let top_out = p.graphs[0].nodes.iter().find(|n| n.def == "output").unwrap().id;
        let cache = Mutex::new(Cache::default());
        let before = cook(&p, 0, &Options::new(&no_res), &cache).image(top_out, 0).unwrap().data[0];
        assert!((before - 0.2).abs() < 1e-3, "{before}");
        let leaf = &mut p.graphs[2];
        let u = leaf.nodes.iter().find(|n| n.def == "uniform").unwrap().id;
        leaf.node_mut(u).unwrap().params.insert("color".into(), Value::Color([0.7, 0.7, 0.7, 1.0]));
        let after = cook(&p, 0, &Options::new(&no_res), &cache).image(top_out, 0).unwrap().data[0];
        assert!((after - 0.7).abs() < 1e-3, "stale cached result: {after}");
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
        // The caller's image wins over the bound one.
        let mut opts = Options::new(&res);
        opts.inputs.insert("curvature".into(), Image::filled(4, 4, Kind::Gray, [0.1; 4]));
        let c = cook(&p, 0, &opts, &cache);
        assert!((c.image(o, 0).unwrap().data[0] - 0.1).abs() < 1e-4);
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
    fn input_change_recooks_only_readers() {
        use materialcraft_core::function::{FnGraph, FnNode};
        let (mut p, noise, blur, out) = sample();
        let reader = p.graphs[0].add_node("pixel_processor", [0.0, 100.0]);
        let get_a = FnNode { id: 1, op: "get_var".into(), inputs: vec![], value: Some(Value::Text("a".into())), pos: [0.0; 2] };
        p.graphs[0].node_mut(reader).unwrap().params.insert("function".into(), Value::Function(Box::new(FnGraph { nodes: vec![get_a], output: Some(1) })));
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

    /// Bound exposed values obey hard safety limits while manual values beyond the slider remain valid.
    #[test]
    fn bound_values_are_clamped_to_hard_limits() {
        let mut g = Graph::new("g");
        g.size = [4, 4];
        let white = g.add_node("uniform", [0.0, 0.0]);
        g.node_mut(white).unwrap().params.insert("mode".into(), Value::Enum(1));
        g.node_mut(white).unwrap().params.insert("color".into(), Value::Color([1.0, 1.0, 1.0, 1.0]));
        let black = g.add_node("uniform", [0.0, 100.0]);
        g.node_mut(black).unwrap().params.insert("mode".into(), Value::Enum(1));
        g.node_mut(black).unwrap().params.insert("color".into(), Value::Color([0.0, 0.0, 0.0, 1.0]));
        let blend = g.add_node("blend", [100.0, 0.0]);
        g.connect(white, "output", blend, "foreground").unwrap();
        g.connect(black, "output", blend, "background").unwrap();
        g.node_mut(blend).unwrap().bindings.insert("opacity".into(), "o".into());
        let mut p = Package::new("p");
        p.graphs.push(g);
        let mut opts = Options::new(&no_res);
        opts.overrides.insert("o".into(), Value::Float(1e30));
        let c = cook(&p, 0, &opts, &Mutex::new(Cache::default()));
        let v = c.image(blend, 0).unwrap().data[0];
        assert!((v - 17.0).abs() < 1e-4, "blend value {v}");
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
        p.resources[0].bytes = ResourceBytes::new(vec![2; 100]);
        shade.store(2, Ordering::Relaxed);
        let second = cook(&p, 0, &opts, &cache);
        assert!(!second.nodes[&inst].cached, "changing the bound input image resource must recook the instance");
    }
}
