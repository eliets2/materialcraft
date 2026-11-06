use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MapEntry {
    /// Usage id: `baseColor`, `normal`, `roughness`, `metallic`, `height`, `ambientOcclusion`, `emissive`, `opacity`, `specular`
    pub usage: String,
    pub file: String,
    /// `srgb` or `linear`
    pub color_space: String,
}
