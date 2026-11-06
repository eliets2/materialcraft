use super::ATOMIC;
use crate::ops;
use crate::{CookContext, CookResult, NodeDefinition, node};
use materialcraft_image::Image;

pub(super) fn definition() -> NodeDefinition {
    node("distance", "Distance", ATOMIC)
        .desc("Grows white areas of a mask into a distance gradient; with a source, spreads its colors outward.")
        .atomic()
        .gray_in("mask", "Mask input")
        .any_in("source", "Source input")
        .float("distance", "Maximum distance", 10.0, 0.0, 256.0)
        .boolean("combine", "Combine distance with source", false)
        .any_out()
        .cook(distance)
}

fn distance(context: &CookContext) -> CookResult {
    let mask = context.gray_or(0, 0.0);
    let maxd = (context.f("distance").max(0.0) * context.w as f32 / 256.0).max(1e-3);
    let data = mask.data.clone();
    let inside = |i| data.get(i).copied().unwrap_or(0.0) > 0.5;
    let Some(src) = context.sized(1) else {
        let d2 = ops::distance_sq(context.w, context.h, inside, context.tile);
        return Ok(vec![ops::gray_from(context.w, context.h, d2.iter().map(|d| (1.0 - d.sqrt() / maxd).clamp(0.0, 1.0)).collect())]);
    };
    // Propagate the exact nearest site without gaps from gradient-walk stalls
    let (d2, sites) = ops::distance_sq_with_sites(context.w, context.h, inside, context.tile);
    let combine = context.b("combine");
    Ok(vec![Image::generate(context.w, context.h, src.kind, |x, y| {
        let i = y as usize * context.w as usize + x as usize;
        let site = sites.get(i).copied().unwrap_or(usize::MAX);
        let mut c = if site == usize::MAX {
            [0.0, 0.0, 0.0, if src.is_gray() { 1.0 } else { 0.0 }]
        } else {
            src.get((site % context.w as usize) as i64, (site / context.w as usize) as i64, false)
        };
        let d = d2.get(i).copied().unwrap_or(f32::INFINITY).sqrt();
        if d > maxd {
            c = [0.0, 0.0, 0.0, if src.is_gray() { 1.0 } else { 0.0 }];
        } else if combine {
            let f = (1.0 - d / maxd).clamp(0.0, 1.0);
            c = [c[0] * f, c[1] * f, c[2] * f, c[3]];
        }
        c
    })])
}
