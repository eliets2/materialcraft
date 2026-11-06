use super::ATOMIC;
use crate::{NodeDefinition, node};

pub(super) fn definition() -> NodeDefinition {
    node("pixel_processor", "Pixel Processor", ATOMIC)
        .desc("Computes every pixel with a function graph ($pos, $size; sample the inputs).")
        .atomic()
        .any_in("input0", "Input image 0")
        .function("function", "Per pixel function", crate::function::default_pixel_function())
        .boolean("color_mode", "Color output", true)
        .any_out()
        .cook(crate::function::pixel_processor)
}
