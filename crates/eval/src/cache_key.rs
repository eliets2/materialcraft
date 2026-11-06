use crate::{Cache, identifier, instance_graph};
use materialcraft_core::{Node, Package, Value};
use materialcraft_image::Image;
use std::collections::{BTreeMap, HashSet};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::{Arc, Mutex};

pub(crate) fn image_key(i: &Image, cache: &Mutex<Cache>) -> u64 {
    let ptr = Arc::as_ptr(&i.data) as usize;
    let shape = (i.w, i.h, i.kind.channels());
    if let Some(key) = cache
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .input_hashes
        .get(&ptr)
        .and_then(|saved| saved.buffer.upgrade().filter(|data| Arc::ptr_eq(data, &i.data) && saved.shape == shape).map(|_| saved.key))
    {
        return key;
    }
    let mut h = DefaultHasher::new();
    (shape, i.data.len()).hash(&mut h);
    for value in i.data.iter() {
        value.to_bits().hash(&mut h);
    }
    let key = h.finish();
    let mut cache = cache.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if cache.input_hashes.len() >= 128 {
        cache.input_hashes.retain(|_, saved| saved.buffer.strong_count() > 0);
        if cache.input_hashes.len() >= 128 {
            cache.input_hashes.clear();
        }
    }
    cache.input_hashes.insert(ptr, crate::input_fingerprint::InputFingerprint { buffer: Arc::downgrade(&i.data), shape, key });
    key
}

pub(crate) fn hash_of(v: &impl Hash) -> u64 {
    let mut h = DefaultHasher::new();
    v.hash(&mut h);
    h.finish()
}

pub(crate) fn graph_tree_hash(pkg: &Package, gi: usize) -> u64 {
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
        for (id, res) in &graph.input_images {
            pkg.resource(res).map(|r| r.bytes.content_hash()).hash(&mut h);
            id.hash(&mut h);
            graph.input_images_linear.contains(id).hash(&mut h);
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

pub(crate) fn variable_key(node: &Node, parameters: &BTreeMap<String, Value>, variables: &BTreeMap<String, Value>) -> u64 {
    let mut names = std::collections::BTreeSet::new();
    let mut pending: Vec<&Value> = parameters.values().collect();
    while let Some(value) = pending.pop() {
        match value {
            Value::Function(graph) => {
                for function in &graph.nodes {
                    if (function.op.starts_with("get_") || function.op == "isdefined")
                        && let Some(Value::Text(name)) = &function.value
                    {
                        names.insert(name.as_str());
                    }
                }
            }
            Value::PatternGraph(graph) => {
                pending.extend(graph.nodes.iter().flat_map(|node| node.parameters.values()));
            }
            _ => {}
        }
    }

    let input_identifier = identifier(node);
    if node.def == "input_value" {
        names.insert(&input_identifier);
    }

    let mut hasher = DefaultHasher::new();
    for name in names {
        name.hash(&mut hasher);
        variables.get(name).map(|value| serde_json::to_string(value).unwrap_or_default()).hash(&mut hasher);
    }
    hasher.finish()
}
