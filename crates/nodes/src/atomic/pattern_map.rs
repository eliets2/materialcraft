use super::ATOMIC;
use crate::{NodeDefinition, node};
use materialcraft_core::{PatternGraph, PatternNodeKind};

pub(super) fn definition() -> NodeDefinition {
    node("fx_map", "FX-Map", ATOMIC)
        .desc("Places patterns using Quadrant, Switch and Iterate nodes.")
        .atomic()
        .generator()
        .any_in("background", "Background")
        .any_in("pattern0", "Input image 0")
        .pattern_graph("pattern", "FX-Map graph", PatternGraph::with_root(PatternNodeKind::Quadrant))
        .float("opacity", "Opacity", 1.0, 0.0, 10.0)
        .boolean("color_mode", "Color output", false)
        .color("background", "Background color", [0.0, 0.0, 0.0, 1.0])
        .any_out()
        .cook(crate::pattern::cook)
}
