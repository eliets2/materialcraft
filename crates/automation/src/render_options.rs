/// What [`crate::Headless::render_png`] draws
#[derive(Clone, Debug, Default)]
pub struct RenderOptions {
    /// `3d` (default) or `2d` (a node's output; the 2D view's node by default)
    pub view: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    /// Camera overrides in degrees (the session's view is not changed)
    pub yaw: Option<f32>,
    pub pitch: Option<f32>,
    /// 3D scene override (`rounded_cube`, `sphere`, …)
    pub scene: Option<String>,
    /// 2D: the node whose output to show
    pub node: Option<u64>,
    /// 2D: the output port
    pub port: Option<usize>,
    /// Cook at full size instead of the preview size
    pub full: bool,
    /// 3D: UV repeats override (the session's view is not changed)
    pub tiling: Option<f32>,
    /// 3D: beauty render: the height output displaces a subdivided mesh, with ray-traced ambient occlusion, a shadowed key light and 3×3 supersampling. Slower; for stills
    pub hq: bool,
    /// 3D, with `hq`: displacement depth in scene radii (default 0.04)
    pub displacement: Option<f32>,
}
