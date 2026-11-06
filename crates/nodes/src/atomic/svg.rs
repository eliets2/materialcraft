use super::ATOMIC;
use crate::{CookContext, CookResult, NodeDefinition, node, raster::composite};

pub(super) fn definition() -> NodeDefinition {
    node("svg", "SVG", ATOMIC)
        .desc("Renders an SVG resource from the package.")
        .atomic()
        .generator()
        .any_in("background", "Background")
        .resource("resource", "SVG resource")
        .boolean("color_mode", "Color output", true)
        .color("background", "Background color", [0.0; 4])
        .any_out()
        .cook(svg)
}

fn svg(context: &CookContext) -> CookResult {
    let name = context.text("resource");
    let foreground = if name.is_empty() {
        None
    } else {
        Some((context.resource)(name).ok_or_else(|| format!("SVG resource `{name}` could not be decoded"))?.resize(context.w, context.h, false))
    };

    Ok(vec![composite(context, |x, y| foreground.as_ref().map_or([0.0; 4], |image| image.get(i64::from(x), i64::from(y), false)))])
}
