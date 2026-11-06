use super::GRAPH_ITEMS;
use crate::{CookContext, CookResult, NodeDefinition, node};
use materialcraft_core::ComponentUsage;
use materialcraft_image::{Image, Kind};

pub(super) fn definition() -> NodeDefinition {
    node("input_grayscale", "Input Grayscale", GRAPH_ITEMS)
        .desc("A grayscale image input of the graph.")
        .atomic()
        .generator()
        .text("identifier", "Identifier", "input")
        .component_usage(ComponentUsage::ANY_INDEX)
        .color("default", "Default value", [0.5, 0.5, 0.5, 1.0])
        .gray_out()
        .cook(input_gray_default)
}

fn input_gray_default(context: &CookContext) -> CookResult {
    Ok(vec![Image::filled(context.w, context.h, Kind::Gray, [context.v4("default")[0]; 4])])
}
