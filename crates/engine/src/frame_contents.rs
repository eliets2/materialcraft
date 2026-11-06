use materialcraft_core::{Frame, Graph, NodeId};

#[derive(Clone, Debug, Default)]
pub struct FrameContents {
    pub nodes: Vec<NodeId>,
    pub function_nodes: Vec<u32>,
    pub comments: Vec<u64>,
    pub frames: Vec<u64>,
    pub pins: Vec<u64>,
}

impl FrameContents {
    pub fn capture(graph: &Graph, frame: &Frame) -> Self {
        Self {
            function_nodes: graph
                .function
                .iter()
                .flat_map(|function| &function.nodes)
                .filter(|node| frame.contains_position(node.pos))
                .map(|node| node.id)
                .collect(),
            nodes: graph.nodes.iter().filter(|node| frame.contains_position(node.pos)).map(|node| node.id).collect(),
            comments: graph.comments.iter().filter(|comment| frame.contains_position(comment.pos)).map(|comment| comment.id).collect(),
            frames: graph.frames.iter().filter(|candidate| candidate.id == frame.id || frame.contains_rect(candidate.rect)).map(|frame| frame.id).collect(),
            pins: graph.pins.iter().filter(|pin| frame.contains_position(pin.pos)).map(|pin| pin.id).collect(),
        }
    }

    pub fn translate(&self, graph: &mut Graph, offset: [f32; 2]) {
        let translate = |position: [f32; 2]| crate::graph_grid::snap_position([position[0] + offset[0], position[1] + offset[1]], false);
        if let Some(function) = &mut graph.function {
            for node in function.nodes.iter_mut().filter(|node| self.function_nodes.contains(&node.id)) {
                node.pos = translate(node.pos);
            }
        }
        for node in graph.nodes.iter_mut().filter(|node| self.nodes.contains(&node.id)) {
            node.pos = translate(node.pos);
        }
        for comment in graph.comments.iter_mut().filter(|comment| self.comments.contains(&comment.id)) {
            comment.pos = translate(comment.pos);
        }
        for frame in graph.frames.iter_mut().filter(|frame| self.frames.contains(&frame.id)) {
            let position = translate([frame.rect[0], frame.rect[1]]);
            frame.rect[0] = position[0];
            frame.rect[1] = position[1];
        }
        for pin in graph.pins.iter_mut().filter(|pin| self.pins.contains(&pin.id)) {
            pin.pos = translate(pin.pos);
        }
    }
}
