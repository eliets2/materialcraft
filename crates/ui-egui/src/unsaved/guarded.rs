use serde::{Deserialize, Serialize};

/// An action that would throw the open package's unsaved work away
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Guarded {
    /// A menu item: `ui.closeAll` or `file.newPackage`
    Menu { id: String },
    /// Open the package at `path` (desktop)
    OpenPath { path: String },
    /// Open the package whose bytes wait in `MaterialCraftApp::pending_load` (browser)
    LoadBytes { name: String },
    /// Close the window
    Quit,
}
