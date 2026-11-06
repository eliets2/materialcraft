pub const MAX_IMAGE_INPUTS: usize = 256;

pub fn image_input_prefix(definition: &str) -> Option<&'static str> {
    match definition {
        "fx_map" => Some("pattern"),
        "pixel_processor" | "value_processor" => Some("input"),
        _ => None,
    }
}
