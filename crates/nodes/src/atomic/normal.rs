use super::ATOMIC;
use crate::ops;
use crate::{CookContext, CookResult, NodeDefinition, node};
use materialcraft_image::{Image, Kind};

pub(super) fn definition() -> NodeDefinition {
    node("normal", "Normal", ATOMIC)
        .desc("Converts a height map to a tangent-space normal map.")
        .atomic()
        .gray_in("input", "Input")
        .float("intensity", "Intensity", 1.0, 0.0, 10.0)
        .boolean("opengl", "OpenGL normals", true)
        .boolean("height_alpha", "Height in alpha", false)
        .color_out()
        .cook(normal)
}

fn normal(context: &CookContext) -> CookResult {
    let h = context.gray_or(0, 0.0);
    let k = context.f("intensity") * context.w as f32 / 32.0;
    Ok(vec![height_to_normal(&h, k, !context.b("opengl"), context.tile, context.b("height_alpha"))])
}

pub fn height_to_normal(h: &Image, k: f32, directx: bool, tile: bool, alpha_height: bool) -> Image {
    Image::generate(h.w, h.h, Kind::Color, |x, y| {
        let (dx, dy) = ops::slope(h, x as i64, y as i64, tile);
        let (nx, ny, nz) = (-dx * k, dy * k, 1.0);
        let l = (nx * nx + ny * ny + nz * nz).sqrt();
        let ny = if directx { -ny } else { ny };
        let a = if alpha_height { h.get_gray(x as i64, y as i64, tile) } else { 1.0 };
        [nx / l * 0.5 + 0.5, ny / l * 0.5 + 0.5, nz / l * 0.5 + 0.5, a]
    })
}
