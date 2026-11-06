use crate::AuthorizedWorkspace;
/// Where file paths given to [`crate::Headless`] may point
pub enum Filesystem {
    /// No file access (an MCP server started without roots)
    Denied,
    /// Host paths as given: the local CLI user typed them
    TrustedLocal,
    /// Relative paths under the authorized roots (remote or agent callers)
    Workspace(AuthorizedWorkspace),
}
