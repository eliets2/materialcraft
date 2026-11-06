use super::ATOMIC;
use crate::{CookContext, CookResult, NodeDefinition, node};
use materialcraft_image::{Image, Kind, luminance};

pub(super) fn definition() -> NodeDefinition {
    node("uniform", "Uniform Color", ATOMIC)
        .desc("A single color or gray value.")
        .atomic()
        .generator()
        .color("color", "Color", [0.5, 0.5, 0.5, 1.0])
        .boolean("color_mode", "Color output", true)
        .any_out()
        .cook(uniform)
}

fn uniform(context: &CookContext) -> CookResult {
    let c = context.v4("color");
    Ok(vec![if !context.b("color_mode") {
        Image::filled(context.w, context.h, Kind::Gray, [luminance(c); 4])
    } else {
        Image::filled(context.w, context.h, Kind::Color, c)
    }])
}
