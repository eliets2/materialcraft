use super::xml::{child_value, xml_escape};
use materialcraft_core::Graph;
use roxmltree::Node as XmlNode;
use std::fmt::Write as _;

const IMAGE_PREFIX: &str = "materialcraft.previewImage.";
const LINEAR_PREFIX: &str = "materialcraft.previewLinear.";

pub(super) fn write(out: &mut String, graph: &Graph) {
    for (identifier, resource) in &graph.input_images {
        let _ = write!(out, "<option><name v=\"{IMAGE_PREFIX}{}\"/><value v=\"{}\"/></option>", xml_escape(identifier), xml_escape(resource));
        if graph.input_images_linear.contains(identifier) {
            let _ = write!(out, "<option><name v=\"{LINEAR_PREFIX}{}\"/><value v=\"1\"/></option>", xml_escape(identifier));
        }
    }
}

pub(super) fn read(option: XmlNode, graph: &mut Graph) {
    let Some(name) = child_value(option, "name") else { return };
    let Some(value) = child_value(option, "value") else { return };

    if let Some(identifier) = name.strip_prefix(IMAGE_PREFIX) {
        graph.input_images.insert(identifier.to_string(), value.to_string());
    } else if let Some(identifier) = name.strip_prefix(LINEAR_PREFIX)
        && value == "1"
    {
        graph.input_images_linear.insert(identifier.to_string());
    }
}
