#[derive(Clone, Copy)]
pub(super) struct NodeIdentifiers {
    pub(super) input_entry: Option<u64>,
    pub(super) graph_output: Option<u64>,
    pub(super) self_uid: u64,
}
