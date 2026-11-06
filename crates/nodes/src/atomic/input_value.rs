use super::GRAPH_ITEMS;
use crate::{NodeDefinition, PortKind, node};
use materialcraft_core::ComponentUsage;

pub(super) fn definition() -> NodeDefinition {
    node("input_value", "Input Value", GRAPH_ITEMS)
        .desc("A typed value input of the graph.")
        .atomic()
        .text("identifier", "Identifier", "value")
        .component_usage(ComponentUsage::ANY_INDEX)
        .float("default", "Default", 0.0, 0.0, 0.0)
        .output("output", "Value", PortKind::Value)
        .cook(crate::node_definition_builder::no_cook)
}
