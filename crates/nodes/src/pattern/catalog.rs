use materialcraft_core::PatternNodeKind;
use std::sync::OnceLock;

use crate::{NodeDefinition, node};

pub fn definitions() -> &'static [NodeDefinition] {
    static DEFINITIONS: OnceLock<Vec<NodeDefinition>> = OnceLock::new();
    DEFINITIONS.get_or_init(|| {
        vec![
            node("passthrough", "Dot", "Pattern nodes").cook(crate::node_definition_builder::no_cook),
            node("paramset", "Quadrant", "Pattern nodes")
                .int("patterntype", "Pattern", 0, 0, 255 + crate::MAX_IMAGE_INPUTS as i32)
                .float4("opacity", "Color / luminosity", [1.0; 4], 0.0, 1.0)
                .float2("branchoffset", "Branch offset", [0.0; 2], -1.0, 1.0)
                .float2("frameoffset", "Pattern offset", [0.0; 2], -1.0, 1.0)
                .float2("patternsize", "Pattern size", [1.0; 2], 0.0, 2.0)
                .angle("patternrotation", "Pattern rotation", 0.0)
                .float("patternsuppl", "Pattern variation", 0.0, 0.0, 1.0)
                .int("blendingmode", "Blending", 0, 0, 2)
                .int("imagefiltering", "Image filtering", 1, 0, 1)
                .boolean("imagepremul", "Premultiplied image", false)
                .int("imageindex", "Image input", 1, 1, crate::MAX_IMAGE_INPUTS as i32)
                .int("randomseed", "Random seed", 0, 0, 10000)
                .boolean("randominherited", "Inherit random seed", true)
                .cook(crate::node_definition_builder::no_cook),
            node("markov2", "Switch", "Pattern nodes")
                .boolean("switch", "Right branch", false)
                .int("randomseed", "Random seed", 0, 0, 10000)
                .boolean("randominherited", "Inherit random seed", true)
                .cook(crate::node_definition_builder::no_cook),
            node("addnode", "Iterate", "Pattern nodes")
                .int("numberadded", "Iterations", 1, 0, 10000)
                .int("randomseed", "Random seed", 0, 0, 10000)
                .boolean("randominherited", "Inherit random seed", true)
                .cook(crate::node_definition_builder::no_cook),
        ]
    })
}

pub fn definition(kind: PatternNodeKind) -> Option<&'static NodeDefinition> {
    definitions().iter().find(|definition| definition.id == kind.native_id())
}
