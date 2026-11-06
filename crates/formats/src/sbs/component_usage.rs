use materialcraft_core::{ComponentUsage, Node, Value};
use roxmltree::Node as XmlNode;
use std::fmt::Write as _;

use super::xml::{child_element, child_value};

pub(super) fn read(element: XmlNode) -> u32 {
    child_element(element, "usages")
        .and_then(|usages| child_element(usages, "usage"))
        .and_then(|usage| child_value(usage, "name"))
        .map_or(ComponentUsage::ANY_INDEX, ComponentUsage::index)
}

pub(super) fn write(out: &mut String, node: &Node, default: u32) {
    let index = match node.params.get("usage") {
        None => default,
        Some(Value::Enum(index)) => *index,
        Some(value) => value.as_i32().map_or(ComponentUsage::ANY_INDEX, |index| index.max(0) as u32),
    };
    let id = ComponentUsage::from_index(index).id;
    let _ = write!(out, "<usages><usage><components v=\"RGBA\"/><name v=\"{id}\"/></usage></usages>");
}
