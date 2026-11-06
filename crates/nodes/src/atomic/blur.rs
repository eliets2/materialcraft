use super::ATOMIC;
use crate::ops;
use crate::{CookContext, CookResult, NodeDefinition, node};
use materialcraft_image::Kind;

pub(super) fn definition() -> NodeDefinition {
    node("blur", "Blur", ATOMIC).desc("Gaussian blur.").atomic().any_in("input", "Input").float("intensity", "Intensity", 2.0, 0.0, 32.0).any_out().cook(blur)
}

fn blur(context: &CookContext) -> CookResult {
    let img = context.any_or(0, Kind::Gray, [0.0; 4]);
    let sigma = context.f("intensity").max(0.0) * context.w as f32 / 256.0 * 0.5;
    Ok(vec![ops::gaussian(&img, sigma, context.tile)])
}
