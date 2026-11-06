use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct BitmapParams {
    /// Image file relative to the read root (png, jpg, tga, tif, bmp, webp, exr, hdr)
    pub path: String,
    /// Resource name (default: the file name)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}
