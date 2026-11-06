use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Deserialize, Serialize, JsonSchema)]
pub struct RenderParams {
    /// `3d` (default: the material on the 3D view's scene) or `2d` (a node's output)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view: Option<String>,
    /// Pixels, 16..2048
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
    /// Camera yaw/pitch in degrees for this render only
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub yaw: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pitch: Option<f32>,
    /// 3D scene: rounded_cube, cube, sphere, cylinder, plane, torus, material_ball
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scene: Option<String>,
    /// 2D: the node to show (default: the 2D view's node, else the first output)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node: Option<u64>,
    /// 2D: the output port index
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<u32>,
    /// Cook at the full graph size instead of the preview size
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub full: Option<bool>,
    /// 3D: UV repeats of the material on the scene for this render only
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tiling: Option<f32>,
    /// 3D beauty render: real displacement from the height output, ambient occlusion and a shadowed key light (slower)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hq: Option<bool>,
    /// With `hq`: displacement depth in scene radii (default 0.04)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub displacement: Option<f32>,
}
