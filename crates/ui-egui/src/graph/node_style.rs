use super::node_card;
use crate::i18n::tr;
use egui::{Color32, Rect};
use materialcraft_engine::{
    core::{Node, Value},
    nodes::PortKind,
};
pub const NODE_W: f32 = node_card::WIDTH;
pub(super) const PORT_RADIUS: f32 = 4.5;

/// Header strip color of a node by its library category
pub fn header_color(def: &str) -> Color32 {
    let cat = materialcraft_engine::nodes::find(def).map_or("", |d| d.category);
    match def {
        "output" => Color32::from_rgb(0x2c, 0x8f, 0x5e),
        "input_color" | "input_grayscale" | "input_value" => Color32::from_rgb(0x6d, 0x52, 0xb5),
        "graph_instance" => Color32::from_rgb(0x1e, 0x8a, 0xa0),
        _ if cat.starts_with("Atomic") => Color32::from_rgb(0x5c, 0x62, 0x6e),
        _ if cat.contains("Noises") => Color32::from_rgb(0xa8, 0x3c, 0x34),
        _ if cat.contains("Patterns") => Color32::from_rgb(0xb3, 0x5a, 0x2c),
        _ if cat.starts_with("Material") => Color32::from_rgb(0x3a, 0x66, 0xb0),
        _ if cat.starts_with("Filters") => Color32::from_rgb(0x3b, 0x7f, 0x4c),
        _ => Color32::from_rgb(0x5c, 0x62, 0x6e),
    }
}

pub fn port_color(kind: PortKind) -> Color32 {
    match kind {
        PortKind::Gray => Color32::from_rgb(0xc8, 0xc8, 0xc8),
        PortKind::Color => Color32::from_rgb(0xe8, 0xa9, 0x3c),
        PortKind::Value => Color32::from_rgb(0x8b, 0xc6, 0x74),
        PortKind::Any => Color32::from_rgb(0x8c, 0xb4, 0xe0),
    }
}

/// A node's display label
pub fn node_label(n: &Node) -> String {
    if !n.label.is_empty() {
        return n.label.clone();
    }
    let label = match n.def.as_str() {
        "graph_instance" => match n.params.get("graph") {
            Some(Value::Text(g)) => g.clone(),
            _ => tr("Graph instance").into(),
        },
        "output" | "input_color" | "input_grayscale" | "input_value" => materialcraft_engine::eval::identifier(n),
        d => materialcraft_engine::nodes::find(d).map_or(d, |d| d.label).to_string(),
    };
    label.split(['_', ' ']).filter(|word| !word.is_empty()).fold(String::with_capacity(label.len()), |mut display, word| {
        if !display.is_empty() {
            display.push(' ');
        }
        let mut letters = word.chars();
        if let Some(first) = letters.next() {
            display.extend(first.to_uppercase());
            display.extend(letters);
        }
        display
    })
}

/// Graph-space rectangle of a node card
pub fn node_rect(node: &Node, inputs: usize, outputs: usize, previews: bool) -> Rect {
    node_card::bounds(node.pos, inputs, outputs, previews, node.def == "dot")
}

#[cfg(test)]
mod tests {
    use super::*;
    use materialcraft_engine::core::NodeId;

    #[test]
    fn default_labels_are_capitalized_without_changing_identifiers_or_custom_labels() {
        for definition in ["input_color", "input_grayscale", "input_value", "output"] {
            let mut node = Node::new(NodeId(1), definition, [0.0; 2]);
            node.params.insert("identifier".into(), Value::Text("échantillon_value".into()));
            assert_eq!(node_label(&node), "Échantillon Value");
            assert_eq!(materialcraft_engine::eval::identifier(&node), "échantillon_value");
            node.label = "my exact_label".into();
            assert_eq!(node_label(&node), "my exact_label");
        }
        let node = Node::new(NodeId(2), "unknown_primitive", [0.0; 2]);
        assert_eq!(node_label(&node), "Unknown Primitive");
    }
}
