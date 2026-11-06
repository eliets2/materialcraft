/// What a port carries
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PortKind {
    Gray,
    Color,
    /// Either; outputs of this kind follow the main input
    Any,
    Value,
}
