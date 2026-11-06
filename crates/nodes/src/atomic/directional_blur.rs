use super::{ATOMIC, direction};
use crate::{CookContext, CookResult, NodeDefinition, node};
use materialcraft_image::{Image, Kind};

pub(super) fn definition() -> NodeDefinition {
    node("directional_blur", "Directional Blur", ATOMIC)
        .desc("Blurs along one direction.")
        .atomic()
        .any_in("input", "Input")
        .float("intensity", "Intensity", 10.0, 0.0, 64.0)
        .angle("angle", "Angle", 0.0)
        .any_out()
        .cook(directional_blur)
}

fn directional_blur(context: &CookContext) -> CookResult {
    blur(context, context.f("angle"))
}

pub(super) fn blur(context: &CookContext, angle: f32) -> CookResult {
    let img = context.any_or(0, Kind::Gray, [0.0; 4]);
    let len = context.f("intensity").max(0.0) * context.w as f32 / 256.0;
    if len < 0.5 {
        return Ok(vec![img]);
    }
    let (dx, dy) = direction(angle);
    let n = (len.ceil() as i32).clamp(1, 512);
    let (iw, ih) = (1.0 / context.w as f32, 1.0 / context.h as f32);
    Ok(vec![Image::generate_uv(context.w, context.h, img.kind, |u, v| {
        let mut acc = [0.0; 4];
        for i in -n..=n {
            let t = i as f32 / n as f32 * len * 0.5;
            let p = img.sample(u + dx * t * iw, v + dy * t * ih, context.tile);
            for k in 0..4 {
                acc[k] += p[k];
            }
        }
        acc.map(|a| a / (2 * n + 1) as f32)
    })])
}
