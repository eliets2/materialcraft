use super::ATOMIC;
use crate::ops;
use crate::{CookContext, CookResult, NodeDefinition, node};
use materialcraft_image::Kind;

pub(super) fn definition() -> NodeDefinition {
    node("hsl", "HSL", ATOMIC)
        .desc("Hue, saturation and lightness adjustments (0.5 = unchanged).")
        .atomic()
        .color_in("input", "Input")
        .float("hue", "Hue", 0.5, 0.0, 1.0)
        .float("saturation", "Saturation", 0.5, 0.0, 1.0)
        .float("lightness", "Lightness", 0.5, 0.0, 1.0)
        .color_out()
        .cook(hsl)
}

fn hsl(context: &CookContext) -> CookResult {
    let img = context.any_or(0, Kind::Color, [0.5, 0.5, 0.5, 1.0]).to_color();
    let (dh, sat, lig) = (context.f("hue") - 0.5, context.f("saturation"), context.f("lightness"));
    Ok(vec![img.map(Kind::Color, |p| {
        let [h, s, l] = ops::rgb_to_hsl(p);
        let s = if sat <= 0.5 { s * sat * 2.0 } else { s + (1.0 - s) * (sat - 0.5) * 2.0 };
        let l = if lig <= 0.5 { l * lig * 2.0 } else { l + (1.0 - l) * (lig - 0.5) * 2.0 };
        let c = ops::hsl_to_rgb(h + dh, s.clamp(0.0, 1.0), l.clamp(0.0, 1.0));
        [c[0], c[1], c[2], p[3]]
    })])
}
