use crate::ResourceBytes;
use serde::{Deserialize, Serialize};

/// An embedded file: a bitmap a Bitmap node shows, or an SVG
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Resource {
    /// Unique name, referenced by nodes (`resource` parameter)
    pub name: String,
    /// File name inside the package's `resources/` folder
    pub file: String,
    /// The file's bytes (stored in the zip, not in `package.json`)
    #[serde(skip)]
    pub bytes: ResourceBytes,
}

impl Resource {
    /// A resource named `name`, stored as `file` in the package
    pub fn new(name: impl Into<String>, file: impl Into<String>, bytes: ResourceBytes) -> Resource {
        Resource { name: name.into(), file: file.into(), bytes }
    }
}
