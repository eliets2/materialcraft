use super::function_port::FunctionPort;
use crate::graph::{GraphCamera, canvas_geometry::CanvasGeometry, frame_hit::FrameHit};
use egui::{Rect, Response};
use materialcraft_engine::core::Value;

pub(super) struct FunctionCanvas {
    pub response: Response,
    pub camera: GraphCamera,
    pub geometry: CanvasGeometry,
    pub ports: Vec<FunctionPort>,
    pub nodes: Vec<(u32, Response)>,
    pub values: Vec<(u32, Value)>,
    pub frames: Vec<FrameHit>,
    pub comments: Vec<(u64, Rect)>,
    pub clear_annotation: bool,
}
