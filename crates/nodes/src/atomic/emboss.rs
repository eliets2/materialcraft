use super::{ATOMIC, direction};
use crate::ops;
use crate::{CookContext, CookResult, NodeDefinition, node};
use materialcraft_image::{Image, Kind};

pub(super) fn definition() -> NodeDefinition {
    node("emboss", "Emboss", ATOMIC)
        .desc("Lights the input with the relief of a height map.")
        .atomic()
        .any_in("input", "Input")
        .gray_in("height", "Intensity input")
        .float("intensity", "Intensity", 5.0, 0.0, 32.0)
        .angle("light_angle", "Light angle", 0.375)
        .color("highlight", "Highlight color", [1.0, 1.0, 1.0, 1.0])
        .color("shadow", "Shadow color", [0.0, 0.0, 0.0, 1.0])
        .any_out()
        .cook(emboss)
}

fn emboss(context: &CookContext) -> CookResult {
    let img = context.any_or(0, Kind::Color, [0.5, 0.5, 0.5, 1.0]);
    let hmap = context.gray_or(1, 0.0);
    let (lx, ly) = direction(context.f("light_angle"));
    let k = context.f("intensity") * context.w as f32 / 64.0;
    let (hi, sh) = (context.v4("highlight"), context.v4("shadow"));
    Ok(vec![Image::generate(context.w, context.h, img.kind, |x, y| {
        let (gx, gy) = ops::slope(&hmap, x as i64, y as i64, context.tile);
        let l = (-(gx * lx + gy * ly) * k).clamp(-1.0, 1.0);
        let p = img.get(x as i64, y as i64, false);
        let tint = if l >= 0.0 { hi } else { sh };
        let t = l.abs();
        [p[0] + (tint[0] - p[0]) * t, p[1] + (tint[1] - p[1]) * t, p[2] + (tint[2] - p[2]) * t, p[3]]
    })])
}
