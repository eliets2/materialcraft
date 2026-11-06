use serde::{Deserialize, Serialize};

use crate::theme::{Accent, Brightness};
use materialcraft_engine::CameraPrefs;

/// Preferences (Edit ▸ Preferences)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Prefs {
    /// UI language code (`i18n::LANGUAGES`)
    pub language: String,
    pub brightness: Brightness,
    pub accent: Accent,
    pub ui_scale: f32,
    /// Largest node output while editing (0 = full size)
    pub preview_size: u32,
    /// Minutes between autosaves of a modified package (0 = off)
    pub autosave_minutes: u32,
    /// Show node thumbnails in the graph
    pub thumbnails: bool,
    pub snap_to_grid: bool,
    /// Library tiles: large icons or a list
    pub library_list: bool,
    /// Open the home screen when the app starts
    pub show_home: bool,
    /// 3D view camera: pan limit and speed
    pub camera: CameraPrefs,
}

impl Default for Prefs {
    fn default() -> Self {
        Prefs {
            language: crate::i18n::os_language().into(),
            brightness: Brightness::default(),
            accent: Accent::default(),
            ui_scale: 1.0,
            preview_size: 512,
            autosave_minutes: 5,
            thumbnails: true,
            snap_to_grid: true,
            library_list: false,
            show_home: true,
            camera: CameraPrefs::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camera_prefs_persist_and_default_when_missing() {
        // Missing preferences use their defaults
        let old: Prefs = serde_json::from_value(serde_json::json!({"uiScale": 1.25})).unwrap();
        assert_eq!(old.camera, CameraPrefs::default());
        assert_eq!(old.ui_scale, 1.25);
        assert!(old.snap_to_grid);
        let p = Prefs { camera: CameraPrefs { pan_limit: 8.0, pan_speed: 0.5 }, ..Prefs::default() };
        let v = serde_json::to_value(&p).unwrap();
        assert_eq!(v["camera"], serde_json::json!({"panLimit": 8.0, "panSpeed": 0.5}));
        assert_eq!(serde_json::from_value::<Prefs>(v).unwrap(), p);
    }
}
