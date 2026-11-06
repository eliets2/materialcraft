use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct ExportParams {
    /// Output folder relative to the write root (created if missing; `.` = the root)
    pub dir: String,
    /// png (default), jpg, tga, tif, bmp, webp, exr, hdr
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    /// File name pattern with $(graph), $(identifier) and $(usage) (default `$(graph)_$(identifier)`)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
    /// 8, 16 or 32 bits (default: the graph's output format)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depth: Option<u32>,
    /// Pixels of the larger side, rounded down to a power of two, 16 to 8192 (default: the graph's size)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<u32>,
    /// Graph index (default: the open graph)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub graph: Option<u32>,
}
