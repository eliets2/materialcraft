use super::ATOMIC;
use crate::ops;
use crate::{CookContext, CookResult, NodeDefinition, node};
use materialcraft_image::{Image, Kind};

pub(super) fn definition() -> NodeDefinition {
    node("warp", "Warp", ATOMIC)
        .desc("Distorts the input along the slopes of a gradient map.")
        .atomic()
        .any_in("input", "Input")
        .gray_in("gradient", "Gradient input")
        .float("intensity", "Intensity", 1.0, 0.0, 32.0)
        .any_out()
        .cook(warp)
}

fn warp(context: &CookContext) -> CookResult {
    let img = context.any_or(0, Kind::Gray, [0.0; 4]);
    let grad = context.gray_or(1, 0.0);
    let k = context.f("intensity");
    Ok(vec![Image::generate_uv(context.w, context.h, img.kind, |u, v| {
        let (x, y) = ((u * context.w as f32) as i64, (v * context.h as f32) as i64);
        let (dx, dy) = ops::slope(&grad, x, y, context.tile);
        img.sample(u + dx * k, v + dy * k, context.tile)
    })])
}
