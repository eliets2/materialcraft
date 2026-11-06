#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("no node {0}")]
    NoNode(u64),
    #[error("no graph {0}")]
    NoGraph(usize),
    #[error("that connection would make a cycle")]
    Cycle,
}
