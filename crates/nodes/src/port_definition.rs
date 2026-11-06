use crate::PortKind;

#[derive(Clone, Debug)]
pub struct PortDefinition {
    pub id: &'static str,
    pub label: &'static str,
    pub kind: PortKind,
}
