use super::ATOMIC;
use crate::ops;
use crate::ops::BLEND_MODES;
use crate::{CookContext, CookResult, NodeDefinition, node};
use materialcraft_image::{Image, Kind};

pub(super) fn definition() -> NodeDefinition {
    node("blend", "Blend", ATOMIC)
        .desc("Blends a foreground over a background with a blending mode, weighted by opacity and an optional mask.")
        .atomic()
        .any_in("foreground", "Foreground")
        .any_in("background", "Background")
        .gray_in("mask", "Opacity")
        .float("opacity", "Opacity", 1.0, 0.0, 1.0)
        .choice("mode", "Blending mode", BLEND_MODES, 0)
        .boolean("alpha", "Use source alpha", true)
        .any_out()
        .cook(blend)
}

fn blend(context: &CookContext) -> CookResult {
    let (fg, bg) = (context.sized(0), context.sized(1));
    let kind = if [&fg, &bg].iter().any(|i| i.as_ref().is_some_and(|i| !i.is_gray())) { Kind::Color } else { Kind::Gray };
    let fg = fg.unwrap_or_else(|| Image::filled(context.w, context.h, kind, [0.0, 0.0, 0.0, 0.0]));
    let bg = bg.unwrap_or_else(|| Image::filled(context.w, context.h, kind, [0.0, 0.0, 0.0, 1.0]));
    let mask = context.sized(2).map(|m| m.to_gray());
    let (mode, op) = (context.i("mode"), context.f("opacity"));
    let use_alpha = context.b("alpha") && !fg.is_gray();
    Ok(vec![Image::generate(context.w, context.h, kind, |x, y| {
        let (x, y) = (x as i64, y as i64);
        let m = mask.as_ref().map_or(1.0, |m| m.get_gray(x, y, false));
        ops::blend_px(mode, bg.get(x, y, false), fg.get(x, y, false), op * m, use_alpha, true)
    })])
}
