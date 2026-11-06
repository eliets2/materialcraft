use crate::Node;
use crate::Value;

pub const MISSING_NODE: &str = "missing_node";

pub fn missing_ports(node: &Node) -> (Vec<String>, Vec<(String, bool)>) {
    let list = |key: &str| -> Vec<String> {
        match node.params.get(key) {
            Some(Value::Text(s)) => s.split(',').map(str::trim).filter(|p| !p.is_empty()).take(64).map(str::to_string).collect(),
            _ => Vec::new(),
        }
    };

    let outputs = list("outputs")
        .into_iter()
        .map(|o| match o.split_once(':') {
            Some((id, kind)) => (id.to_string(), kind == "gray"),
            None => (o, false),
        })
        .collect();
    (list("inputs"), outputs)
}
