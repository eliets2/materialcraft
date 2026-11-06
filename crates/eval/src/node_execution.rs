use crate::{Cache, Cooked, NodeOutput, NodeResult, Options, identifier, instance_graph, node_size, now_ms};
use crate::{
    cache_key::{graph_tree_hash, hash_of, image_key, variable_key},
    graph_instance::cook_instance,
    parameter::resolve_parameters,
    prepared_graph::PreparedGraph,
    value_execution::evaluate_value,
};
use materialcraft_core::{Graph, Node, Package, Value};
use materialcraft_image::{Image, Kind};
use materialcraft_nodes::{CookContext, find};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

/// The magenta image a failed node shows
pub fn error_image(w: u32, h: u32) -> Image {
    Image::filled(w, h, Kind::Color, [1.0, 0.0, 1.0, 1.0])
}

/// Lock the cache, recovering from a poisoned lock (a cache entry is never half-written)
fn lock(cache: &Mutex<Cache>) -> std::sync::MutexGuard<'_, Cache> {
    cache.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

pub(crate) fn cook_node(
    pkg: &Package,
    gi: usize,
    graph: &Graph,
    prepared: &PreparedGraph<'_>,
    node: &Node,
    done: &Cooked,
    vars: &BTreeMap<String, Value>,
    opts: &Options,
    cache: &Mutex<Cache>,
) -> NodeResult {
    let (in_ports, out_ports) = prepared.ports(node.id);
    let def = find(&node.def);
    // Gather inputs by port order, with their keys
    let mut inputs = Vec::new();
    let mut incoming = Vec::new();
    let mut input_keys = Vec::new();
    let mut variables = std::borrow::Cow::Borrowed(vars);
    let mut input_error = None;
    for port in in_ports {
        let source = prepared.index.input_of(node.id, &port.id).and_then(|connection| {
            let result = done.nodes.get(&connection.from)?;
            let index = prepared.ports(connection.from).1.iter().position(|output| output.id == connection.from_port)?;
            Some((result.outputs.get(index)?, result.key ^ hash_of(&index)))
        });
        input_keys.push(source.as_ref().map_or(0, |source| source.1));
        let output = source.map(|source| source.0);

        if let Some(expected) = node.input_values.get(&port.id) {
            let value = match output.as_ref() {
                Some(NodeOutput::Value(value)) if materialcraft_core::ValueType::of(value) == Some(*expected) => value.clone(),
                Some(_) => {
                    input_error = Some(format!("input `{}` requires {}", port.id, expected.label()));
                    expected.default_value()
                }
                None => expected.default_value(),
            };

            variables.to_mut().insert(port.id.clone(), value);
        } else {
            if output.as_ref().is_some_and(|output| output.as_value().is_some()) && !matches!(node.def.as_str(), "output" | "dot") {
                input_error = Some(format!("input `{}` requires an image", port.id));
            }
            inputs.push(output.and_then(NodeOutput::as_image).cloned());
        }

        incoming.push(output);
    }
    let vars = &variables;
    let first = prepared.first_input_size(node.id);
    let (w, h) = node_size(graph, def, node, first, opts.max_size);
    let fail = |msg: String, key: u64| NodeResult {
        outputs: Arc::new(vec![NodeOutput::Image(error_image(w, h)); out_ports.len().max(1)]),
        error: Some(msg),
        ms: 0.0,
        key,
        cached: false,
    };

    if let Some(error) = input_error {
        return fail(error, 0);
    }

    let seed = (graph.seed as u32).wrapping_mul(0x9e37_79b9).wrapping_add(node.base.seed as u32);
    let params = match resolve_parameters(node, def, vars, seed, [w, h]) {
        Ok(parameters) => parameters,
        Err(error) => return fail(error, 0),
    };

    let tile = node.base.tiling[0] && node.base.tiling[1];
    // Input nodes read the caller's image for their identifier
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
        &input_keys,
        res_hash,
        sub_hash,
        input_img.map(|image| image_key(image, cache)),
        variable_key(node, &params, vars),
    ));
    if let Some(v) = lock(cache).get(key) {
        return NodeResult { outputs: v, error: None, ms: 0.0, key, cached: true };
    }
    let t0 = now_ms();
    let result: Result<Vec<NodeOutput>, String> = if node.disabled {
        let output = incoming.iter().find_map(|output| *output).cloned().unwrap_or_else(|| NodeOutput::Image(Image::filled(w, h, Kind::Gray, [0.0; 4])));
        Ok(vec![output; out_ports.len().max(1)])
    } else if node.def == "input_value" {
        let value = vars.get(&identifier(node)).or_else(|| params.get("default")).cloned().unwrap_or(Value::Float(0.0));
        Ok(vec![NodeOutput::Value(value)])
    } else if node.def == "value_processor" {
        evaluate_value(&params, vars, seed, [w, h], &inputs, tile).map(|value| vec![NodeOutput::Value(value)])
    } else if matches!(node.def.as_str(), "output" | "dot") && incoming.first().and_then(Option::as_ref).is_some_and(|output| output.as_value().is_some()) {
        incoming.first().copied().flatten().cloned().map(|output| vec![output]).ok_or("missing value input".into())
    } else if let Some(image) = input_img {
        let image = image.resize(w, h, tile);
        let image = if node.def == "input_grayscale" { image.to_gray() } else { image };

        Ok(vec![NodeOutput::Image(image)])
    } else if node.def == materialcraft_core::MISSING_NODE {
        let name = if node.label.is_empty() { "this node" } else { node.label.as_str() };

        Err(format!("unsupported node `{name}`"))
    } else if node.def == "graph_instance" {
        cook_instance(pkg, gi, node, in_ports, out_ports, &inputs, &params, (w, h), opts, cache)
            .map(|images| images.into_iter().map(NodeOutput::Image).collect())
    } else if let Some(definition) = def {
        let context = CookContext { w, h, seed, tile, inputs: &inputs, params: &params, resource: opts.resource, vars };
        (definition.cook)(&context).map(|images| images.into_iter().map(NodeOutput::Image).collect())
    } else {
        Err(format!("unknown node `{}`", node.def))
    };

    match result {
        Ok(imgs) if imgs.len() == out_ports.len().max(1) || out_ports.is_empty() => {
            let v = Arc::new(imgs);
            lock(cache).put(key, v.clone());
            NodeResult { outputs: v, error: None, ms: (now_ms() - t0) as f32, key, cached: false }
        }
        Ok(imgs) => fail(format!("`{}` made {} images for {} outputs", node.def, imgs.len(), out_ports.len()), key),
        Err(e) => fail(e, key),
    }
}
