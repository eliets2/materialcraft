use super::{ATOMIC, directional_blur};
use crate::{CookContext, CookResult, NodeDefinition, node};

pub(super) fn definition() -> NodeDefinition {
    node("motion_blur", "Motion Blur", ATOMIC)
        .desc("Blurs the input along a motion direction.")
        .atomic()
        .any_in("input", "Input")
        .float("intensity", "Intensity", 10.0, 0.0, 64.0)
        .angle("shiftangle", "Angle", 0.0)
        .any_out()
        .cook(cook)
}

fn cook(context: &CookContext) -> CookResult {
    directional_blur::blur(context, context.f("shiftangle"))
}
