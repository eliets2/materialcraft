use super::ATOMIC;
use crate::{NodeDefinition, node};

pub(super) fn definition() -> NodeDefinition {
    node("value_processor", "Value Processor", ATOMIC)
        .desc("Computes a typed value with a function graph.")
        .atomic()
        .any_in("input0", "Input image 0")
        .function("function", "Function", materialcraft_core::FunctionGraph::constant(materialcraft_core::Value::Float(0.5)))
        .output("output", "Value", crate::PortKind::Value)
        .cook(crate::node_definition_builder::no_cook)
}
