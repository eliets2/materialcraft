use super::{ATOMIC, direction};
use crate::{CookContext, CookResult, NodeDefinition, node};
use materialcraft_image::{Image, Kind};

pub(super) fn definition() -> NodeDefinition {
    node("directional_warp", "Directional Warp", ATOMIC)
        .desc("Offsets the input along a direction, by an intensity map.")
        .atomic()
        .any_in("input", "Input")
        .gray_in("intensity_input", "Intensity input")
        .float("intensity", "Intensity", 10.0, 0.0, 64.0)
        .angle("angle", "Warp angle", 0.0)
        .any_out()
        .cook(directional_warp)
}

fn directional_warp(context: &CookContext) -> CookResult {
    let img = context.any_or(0, Kind::Gray, [0.0; 4]);
    let map = context.gray_or(1, 0.0);
    let (dx, dy) = direction(context.f("angle"));
    let k = context.f("intensity") / 256.0;
    Ok(vec![Image::generate_uv(context.w, context.h, img.kind, |u, v| {
        let m = map.sample_gray(u, v, context.tile) * k;
        img.sample(u + dx * m, v + dy * m, context.tile)
    })])
}
