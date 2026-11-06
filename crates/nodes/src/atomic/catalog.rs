use super::*;
use crate::NodeDefinition;

pub(crate) fn defs() -> Vec<NodeDefinition> {
    vec![
        bitmap::definition(),
        svg::definition(),
        text::definition(),
        pattern_map::definition(),
        blend::definition(),
        blur::definition(),
        channels_shuffle::definition(),
        curve::definition(),
        directional_blur::definition(),
        directional_warp::definition(),
        distance::definition(),
        emboss::definition(),
        gradient_dynamic::definition(),
        gradient_map::definition(),
        grayscale_conversion::definition(),
        hsl::definition(),
        levels::definition(),
        normal::definition(),
        motion_blur::definition(),
        pixel_processor::definition(),
        value_processor::definition(),
        sharpen::definition(),
        transformation::definition(),
        uniform::definition(),
        warp::definition(),
        input_color::definition(),
        input_value::definition(),
        input_grayscale::definition(),
        output::definition(),
        dot::definition(),
    ]
}
