use super::GRAPH_ITEMS;
use crate::{CookContext, CookResult, NodeDefinition, node};
use materialcraft_image::{Image, Kind};

pub(super) fn definition() -> NodeDefinition {
    node("dot", "Dot", GRAPH_ITEMS).desc("A reroute point for wires.").atomic().any_in("input", "Input").any_out().cook(passthrough)
}

pub(crate) fn passthrough(context: &CookContext) -> CookResult {
    Ok(vec![context.sized(0).unwrap_or_else(|| Image::filled(context.w, context.h, Kind::Gray, [0.0; 4]))])
}
