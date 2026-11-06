#[derive(Debug, thiserror::Error)]
pub enum FormatError {
    #[error("not a MaterialCraft package: {0}")]
    BadPackage(String),
    #[error("image: {0}")]
    Image(String),
    #[error("I/O: {0}")]
    Io(String),
}
