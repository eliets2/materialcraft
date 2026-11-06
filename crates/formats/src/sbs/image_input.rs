use materialcraft_nodes::{MAX_IMAGE_INPUTS, image_input_prefix};

pub(super) fn read(definition: &str, identifier: &str) -> Option<String> {
    let prefix = image_input_prefix(definition)?;

    let index = if identifier == "input" { 0 } else { identifier.strip_prefix("input:")?.parse::<usize>().ok()? };
    (index < MAX_IMAGE_INPUTS).then(|| format!("{prefix}{index}"))
}

pub(super) fn write(definition: &str, identifier: &str) -> Option<String> {
    let prefix = image_input_prefix(definition)?;

    let index = identifier.strip_prefix(prefix)?.parse::<usize>().ok()?;
    (index < MAX_IMAGE_INPUTS).then(|| if index == 0 { "input".into() } else { format!("input:{index}") })
}
