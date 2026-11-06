use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Deserialize, Serialize, JsonSchema)]
pub struct ScreenshotParams {
    /// `3d` (default) or `2d`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view: Option<String>,
}
