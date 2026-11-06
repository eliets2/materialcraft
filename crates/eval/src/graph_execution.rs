use crate::{Cache, Cooked, Options, now_ms};
use crate::{node_execution::cook_node, prepared_graph::PreparedGraph};
use materialcraft_core::{Package, Value};
use materialcraft_image::Image;
use std::{
    collections::BTreeMap,
    sync::{Mutex, atomic::Ordering},
};

pub fn cook(package: &Package, graph_index: usize, options: &Options, cache: &Mutex<Cache>) -> Cooked {
    let started_ms = now_ms();
    let mut cooked = Cooked::default();
    let Some(graph) = package.graphs.get(graph_index) else { return cooked };

    // Input nodes show their bound preview images; the caller's images win
    let bound;
    let options = if !graph.input_images.is_empty() {
        let mut inputs: BTreeMap<String, Image> = graph
            .input_images
            .iter()
            .filter_map(|(id, resource)| {
                Some((id.clone(), (options.resource)(&materialcraft_image::resource_key(resource, graph.input_images_linear.contains(id)))?))
            })
            .collect();
        inputs.extend(options.inputs.iter().map(|(name, value)| (name.clone(), value.clone())));
        bound = Options { only: options.only.clone(), inputs, overrides: options.overrides.clone(), graph_stack: options.graph_stack.clone(), ..*options };
        &bound
    } else {
        options
    };

    let prepared = PreparedGraph::new(package, graph, options.only.as_deref());

    // Exposed parameter values: the graph's own, then the caller's overrides
    let mut variables: BTreeMap<String, Value> =
        graph.exposed.iter().filter_map(|exposed| graph.input_value(&exposed.id).map(|value| (exposed.id.clone(), value.clone()))).collect();
    for (name, value) in &options.overrides {
        variables.insert(name.clone(), value.clone());
    }
    for &id in &prepared.order {
        if options.cancel.is_some_and(|cancel| cancel.load(Ordering::Relaxed)) {
            cooked.cancelled = true;
            break;
        }
        let Some(node) = prepared.index.node(id) else { continue };

        let result = cook_node(package, graph_index, graph, &prepared, node, &cooked, &variables, options, cache);
        cooked.nodes.insert(id, result);
    }
    cooked.ms = (now_ms() - started_ms) as f32;
    cooked
}
