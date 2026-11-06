use materialcraft_nodes::PortKind;

#[derive(Clone, Debug, PartialEq)]
pub struct Port {
    pub id: String,
    pub label: String,
    pub kind: PortKind,
}
