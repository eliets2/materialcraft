use super::GRAPH_ITEMS;
use crate::{CookContext, CookResult, NodeDefinition, node};
use materialcraft_core::ComponentUsage;
use materialcraft_image::{Image, Kind};

pub(super) fn definition() -> NodeDefinition {
    node("input_color", "Input Color", GRAPH_ITEMS)
        .desc("A color image input of the graph.")
        .atomic()
        .generator()
        .text("identifier", "Identifier", "input")
        .component_usage(ComponentUsage::ANY_INDEX)
        .color("default", "Default color", [0.5, 0.5, 0.5, 1.0])
        .color_out()
        .cook(input_color_default)
}

fn input_color_default(context: &CookContext) -> CookResult {
    Ok(vec![Image::filled(context.w, context.h, Kind::Color, context.v4("default"))])
}
