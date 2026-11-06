#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("unknown command `{0}`")]
    UnknownCommand(String),
    #[error("`{cmd}` is not available: {why}")]
    Disabled { cmd: String, why: String },
    #[error("`{cmd}`: {msg}")]
    BadParams { cmd: String, msg: String },
    #[error(transparent)]
    Core(#[from] materialcraft_core::CoreError),
    #[error(transparent)]
    Format(#[from] materialcraft_formats::FormatError),
    #[error("{0}")]
    Other(String),
}
