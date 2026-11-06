use serde::{Deserialize, Serialize};

use super::{GraphCamera, ImageCamera, Prefs, Windows};
use crate::docking::Dock;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct UiState {
    pub dock: Dock,
    pub prefs: Prefs,
    pub windows: Windows,
    pub graph_camera: GraphCamera,
    pub image_camera: ImageCamera,
    /// Library: the selected category path ("" = all) and the search text
    pub library_category: String,
    pub library_search: String,
    /// Recently opened or saved packages, newest first
    pub recent: Vec<String>,
    /// New graph dialog fields
    pub new_graph_kind: materialcraft_engine::core::GraphKind,
    pub new_graph_template: String,
    pub new_graph_name: String,
    pub new_graph_size: u32,
    /// Export dialog fields
    pub export_dir: String,
    pub export_format: String,
    pub export_depth: u32,
    /// Export size in pixels; 0 = the graph's own size
    pub export_size: u32,
    pub export_pattern: String,
    /// Frame the whole graph on the next frame
    #[serde(skip)]
    pub frame_all: bool,
}

impl Default for UiState {
    fn default() -> Self {
        UiState {
            dock: Dock::default(),
            prefs: Prefs::default(),
            windows: Windows::default(),
            graph_camera: GraphCamera::default(),
            image_camera: ImageCamera::default(),
            library_category: String::new(),
            library_search: String::new(),
            recent: Vec::new(),
            new_graph_kind: materialcraft_engine::core::GraphKind::Material,
            new_graph_template: "pbr".into(),
            new_graph_name: "New_graph".into(),
            new_graph_size: 1024,
            export_dir: String::new(),
            export_format: "png".into(),
            export_depth: 0,
            export_size: 0,
            export_pattern: "$(graph)_$(identifier)".into(),
            frame_all: true,
        }
    }
}

impl UiState {
    /// Remember `path` at the top of the recent list
    pub fn add_recent(&mut self, path: &str) {
        self.recent.retain(|p| p != path);
        self.recent.insert(0, path.to_string());
        self.recent.truncate(12);
    }
}
