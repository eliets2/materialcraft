#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod cache;
mod cache_key;
mod cooked;
mod graph_execution;
mod graph_instance;
mod graph_output;
mod input_fingerprint;
mod node_execution;
mod node_output;
mod node_result;
mod node_size;
mod options;
mod parameter;
mod port;
mod ports;
mod prepared_graph;
mod time;
mod value_execution;
mod value_input_label;

use crate::graph_execution::cook as cook_inner;
pub use cache::Cache;
pub use cooked::Cooked;
pub use graph_output::{output_usage, outputs};
pub use node_execution::error_image;
pub use node_output::NodeOutput;
pub use node_result::NodeResult;
pub use node_size::node_size;
pub use options::Options;
pub use port::Port;
pub use ports::{identifier, instance_graph, ports, ports_with_connections};
pub use time::now_ms;
pub use value_input_label::value_input_label;

use std::collections::{BTreeSet, HashMap};
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex};

pub const MAX_DEPTH: u32 = 8;

/// One cache's most recent fingerprint, keyed by package allocation address.
/// Stored per cache rather than in a global map keyed by content identity, so
/// reverting or interleaving two packages on one cache cannot hit a stale
/// entry: every identity or fingerprint mismatch clears the cache, and there
/// is only the most recent (package identity, fingerprint) entry per cache.
struct CacheState {
    latest: Option<(usize, u64)>,
}

static CACHE_STATES: std::sync::LazyLock<Mutex<HashMap<usize, Arc<Mutex<CacheState>>>>> = std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

fn cache_state(cache: &Mutex<Cache>) -> Arc<Mutex<CacheState>> {
    let addr = std::ptr::from_ref(cache) as usize;
    let mut states = CACHE_STATES.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    states.entry(addr).or_insert_with(|| Arc::new(Mutex::new(CacheState { latest: None }))).clone()
}

/// Fixed-key 64-bit FNV-1a hasher: stable across instances, threads and runs,
/// unlike `DefaultHasher`, whose keys are per-instance.
struct StableHasher(u64);

impl Hasher for StableHasher {
    fn write(&mut self, bytes: &[u8]) {
        let mut hash = self.0;
        for &byte in bytes {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0100_0000_01b3);
        }
        self.0 = hash;
    }

    fn finish(&self) -> u64 {
        self.0
    }
}

impl Default for StableHasher {
    fn default() -> StableHasher {
        StableHasher(0xcbf2_9ce4_8422_2325)
    }
}

/// Stored digest of the resource bytes reachable from the last cooked graph,
/// kept per cache (see `CacheState`) so a fingerprint for one cache can
/// never validate contents cooked into another.
///
/// Digest the contents of every graph reachable from `gi`, including the
/// decoded bytes of every bitmap resource reachable from those graphs. Edits to
/// nested instanced graph contents (node defs, params, connections, constants)
/// must change this digest, so the outermost cook drops cached outputs instead
/// of reusing stale nested results. Package and resource closure bytes are
/// likewise hashed per graph, as closures may stand in for edited bytes.
fn resource_fingerprint(pkg: &materialcraft_core::Package, gi: usize, opts: &Options) -> u64 {
    let mut names = BTreeSet::new();
    let mut pending = vec![gi];
    let mut seen = BTreeSet::new();
    let mut h = StableHasher::default();
    while let Some(index) = pending.pop() {
        if !seen.insert(index) {
            continue;
        }
        let Some(graph) = pkg.graphs.get(index) else { continue };
        index.hash(&mut h);
        serde_json::to_string(graph).unwrap_or_default().hash(&mut h);
        for (id, res) in &graph.input_images {
            res.hash(&mut h);
            names.insert(res.clone());
            pkg.resource(res).map(|r| r.bytes.content_hash()).hash(&mut h);
            id.hash(&mut h);
            graph.input_images_linear.contains(id).hash(&mut h);
        }
        for node in &graph.nodes {
            if let Some(materialcraft_core::Value::Text(name)) = node.params.get("resource") {
                names.insert(name.clone());
                pkg.resource(name).map(|r| r.bytes.content_hash()).hash(&mut h);
            }
            if node.def == "graph_instance"
                && let Some((next, _)) = instance_graph(pkg, node)
            {
                pending.push(next);
            }
        }
    }
    for name in &names {
        name.hash(&mut h);
        match (opts.resource)(name) {
            None => 0u8.hash(&mut h),
            Some(image) => {
                (image.w, image.h, image.kind.channels(), image.data.len()).hash(&mut h);
                for value in image.data.iter() {
                    value.to_bits().hash(&mut h);
                }
            }
        }
    }
    h.finish()
}

/// Cook a graph, dropping cached outputs when the resource bytes reachable from
/// it changed since the last cook through this cache (or when no digest is
/// stored, e.g. the cache was populated without a fingerprint). Nested instance
/// cooks (options.depth > 0) run through the inner entry directly, so they
/// never clear mid-cook; this also works across threads, unlike a thread-local
/// depth flag. The per-cache lock is held across the whole outermost cook
/// and the fingerprint is stored only after it returns, so a failed cook
/// cannot validate a partial cache and concurrent outermost cooks cannot
/// interleave clear/store.
pub fn cook(package: &materialcraft_core::Package, graph_index: usize, options: &Options, cache: &Mutex<Cache>) -> Cooked {
    if options.depth > 0 {
        return cook_inner(package, graph_index, options, cache);
    }
    let state = cache_state(cache);
    let identity = std::ptr::from_ref(package) as usize;
    let fingerprint = resource_fingerprint(package, graph_index, options);
    let mut guard_state = state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let stored = guard_state.latest;
    if stored != Some((identity, fingerprint)) {
        cache.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clear();
    }
    let cooked = cook_inner(package, graph_index, options, cache);
    // A failed cook cannot validate a partial cache: clear it and store no
    // fingerprint, so the next cook re-derives every result.
    if cooked.cancelled || cooked.nodes.values().any(|result| result.error.is_some()) {
        cache.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clear();
    } else {
        guard_state.latest = Some((identity, fingerprint));
    }
    cooked
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod nested_instance_edit_invalidates_cached_cook_tests {
    use super::*;
    use materialcraft_core::{Graph, NodeId, Package, Value};
    use materialcraft_image::{Image, Kind};
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicU8, Ordering};

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
    fn nested_instance_edit_invalidates_cached_cook() {
        // The deepest graph reads a bitmap resource; its bytes are the
        // decoded image the resource closure returns for "tex".
        let mut sub = Graph::new("sub");
        sub.size = [4, 4];
        let bm = sub.add_node("bitmap", [0.0, 0.0]);
        sub.node_mut(bm).unwrap().params.insert("resource".into(), Value::Text("tex".into()));
        let o = sub.add_node("output", [100.0, 0.0]);
        sub.connect(bm, "output", o, "input").unwrap();
        let (mid, _) = instancing("mid", "sub");
        let (top, inst) = instancing("top", "mid");
        let mut p = Package::new("p");
        p.graphs = vec![top, mid, sub];
        // Stands in for the resource bytes: the shade picks the decoded image
        let shade = AtomicU8::new(1);
        let res = |_: &str| Some(Image::filled(4, 4, Kind::Gray, [f32::from(shade.load(Ordering::Relaxed)) / 10.0; 4]));
        let cache = Mutex::new(Cache::default());
        let opts = Options::new(&res);
        let first = cook(&p, 0, &opts, &cache);
        assert!(first.nodes[&inst].error.is_none(), "{:?}", first.nodes[&inst].error);
        assert!(!first.nodes[&inst].cached);
        let out = p.graphs[0].nodes.iter().find(|n| n.def == "output").unwrap().id;
        let first_mean = first.image(out, 0).unwrap().mean()[0];
        // Edit the resource bytes used inside the nested instance
        shade.store(8, Ordering::Relaxed);
        let second = cook(&p, 0, &opts, &cache);
        assert!(!second.nodes[&inst].cached, "editing resource bytes inside a nested instance must recook the instance");
        assert!((second.image(out, 0).unwrap().mean()[0] - first_mean).abs() > 0.05);
    }

    #[test]
    fn nested_instance_param_edit_invalidates_cached_cook() {
        let mut sub = Graph::new("sub");
        sub.size = [4, 4];
        let u = sub.add_node("uniform", [0.0, 0.0]);
        sub.node_mut(u).unwrap().params.insert("color".into(), Value::Color([0.2, 0.2, 0.2, 1.0]));
        let o = sub.add_node("output", [100.0, 0.0]);
        sub.connect(u, "output", o, "input").unwrap();
        let (mid, _) = instancing("mid", "sub");
        let (top, inst) = instancing("top", "mid");
        let mut p = Package::new("p");
        p.graphs = vec![top, mid, sub];
        let cache = Mutex::new(Cache::default());
        let opts = Options::new(&|_: &str| None);
        let first = cook(&p, 0, &opts, &cache);
        assert!(!first.nodes[&inst].cached);
        let out = p.graphs[0].nodes.iter().find(|n| n.def == "output").unwrap().id;
        let first_mean = first.image(out, 0).unwrap().mean()[0];
        p.graphs[2].node_mut(u).unwrap().params.insert("color".into(), Value::Color([0.8, 0.8, 0.8, 1.0]));
        let second = cook(&p, 0, &opts, &cache);
        assert!(!second.nodes[&inst].cached, "editing a nested instance param must recook the instance");
        assert!((second.image(out, 0).unwrap().mean()[0] - first_mean).abs() > 0.05);
    }
}
