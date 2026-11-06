use super::ATOMIC;
use crate::{CookContext, CookResult, NodeDefinition, node};
use materialcraft_core::Gradient;
use materialcraft_image::{Kind, luminance};

pub(super) fn definition() -> NodeDefinition {
    node("gradient_map", "Gradient Map", ATOMIC)
        .desc("Maps grayscale values to colors along a gradient.")
        .atomic()
        .gray_in("input", "Input")
        .gradient("gradient", "Gradient", Gradient::default())
        .boolean("color_mode", "Color output", true)
        .any_out()
        .cook(gradient_map)
}

fn gradient_map(context: &CookContext) -> CookResult {
    let input = context.gray_or(0, 0.0);
    let g = context.gradient("gradient");
    let lut: Vec<[f32; 4]> = (0..=512).map(|i| g.eval(i as f32 / 512.0)).collect();
    let gray = !context.b("color_mode");
    let look = |v: f32| lut.get((v.clamp(0.0, 1.0) * 512.0).round() as usize).copied().unwrap_or([0.0, 0.0, 0.0, 1.0]);
    Ok(vec![if gray { input.map(Kind::Gray, |p| [luminance(look(p[0])), 0.0, 0.0, 0.0]) } else { input.map(Kind::Color, |p| look(p[0])) }])
}
