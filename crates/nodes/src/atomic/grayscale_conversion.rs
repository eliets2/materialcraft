use super::ATOMIC;
use crate::{CookContext, CookResult, NodeDefinition, node};
use materialcraft_image::Kind;

pub(super) fn definition() -> NodeDefinition {
    node("grayscale_conversion", "Grayscale Conversion", ATOMIC)
        .desc("Weighted sum of the RGBA channels.")
        .atomic()
        .color_in("input", "Input")
        .float("red", "Red weight", 0.333, 0.0, 1.0)
        .float("green", "Green weight", 0.333, 0.0, 1.0)
        .float("blue", "Blue weight", 0.333, 0.0, 1.0)
        .float("alpha", "Alpha weight", 0.0, 0.0, 1.0)
        .boolean("flatten_alpha", "Flatten alpha", false)
        .float("matte", "Matte level", 0.0, 0.0, 1.0)
        .gray_out()
        .cook(grayscale_conversion)
}

fn grayscale_conversion(context: &CookContext) -> CookResult {
    let img = context.any_or(0, Kind::Color, [0.0, 0.0, 0.0, 1.0]).to_color();
    let w = [context.f("red"), context.f("green"), context.f("blue"), context.f("alpha")];
    let flatten = context.b("flatten_alpha");
    let matte = context.f("matte");
    Ok(vec![img.map(Kind::Gray, |p| {
        let mut v = p[0] * w[0] + p[1] * w[1] + p[2] * w[2] + p[3] * w[3];
        if flatten {
            v = v * p[3] + matte * (1.0 - p[3]);
        }
        [v, 0.0, 0.0, 0.0]
    })])
}
