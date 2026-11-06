use super::sbs_value::constant_xml;
use super::xml::xml_escape;
use materialcraft_core::{Node, Value};

pub(super) fn text_param(node: &Node, key: &str, default: &str) -> String {
    match node.params.get(key) {
        Some(Value::Text(s)) if !s.is_empty() => s.clone(),
        _ => default.to_string(),
    }
}
pub(super) fn param_xml(name: &str, value: &Value) -> Option<String> {
    Some(format!("<parameter><name v=\"{}\"/><relativeTo v=\"0\"/><paramValue>{}</paramValue></parameter>", xml_escape(name), constant_xml(value)?))
}
pub(super) fn special_filter_params_xml<'a>(filter: &str, value_of: &dyn Fn(&str) -> Option<&'a Value>, self_uid: u64) -> String {
    match filter {
        "bitmap" | "svg" => match value_of("resource") {
            Some(Value::Text(name)) if !name.is_empty() => param_xml(
                if filter == "svg" { "vectorresourcepath" } else { "bitmapresourcepath" },
                &Value::Text(format!("pkg:///Resources/{name}?dependency={self_uid}")),
            )
            .unwrap_or_default(),
            _ => String::new(),
        },
        "grayscaleconversion" => {
            let weight = |channel: &str| value_of(channel).and_then(Value::as_f32).unwrap_or(0.0);
            param_xml("channelsweights", &Value::Float4([weight("red"), weight("green"), weight("blue"), weight("alpha")])).unwrap_or_default()
        }
        _ => String::new(),
    }
}
