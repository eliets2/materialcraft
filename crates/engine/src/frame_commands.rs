use crate::commands::{CommandSpec, bad, boolp, has_graph, point, strp, u64p};
use crate::graph_cmds::nodes_rect;
use crate::{AnnotationSelection, FrameContents, Result, Session, cmd};
use materialcraft_core::Frame;
use serde_json::{Value as J, json};

pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "graph.frame.add",
            "Add Frame",
            [],
            None,
            "{title?, titleVisible?, description?, rect? [x, y, w, h], color? [r,g,b,a]} around the selection when no rect",
            has_graph,
            frame_add
        ),
        cmd!("graph.frame.edit", "Edit frame", [], None, "{id, title?, titleVisible?, description?, rect?, color?}", has_graph, frame_edit),
        cmd!("graph.frame.delete", "Delete frame", [], None, "{id}", has_graph, frame_delete),
        cmd!("graph.frame.move", "Move frame and contents", [], None, "{id, by: [dx, dy], snapToGrid?}", has_graph, frame_move),
    ]
}

fn rect_of(parameters: &J) -> Option<[f32; 4]> {
    let values = parameters.get("rect")?.as_array()?;
    let value: Vec<f32> = values.iter().filter_map(J::as_f64).map(|value| value as f32).filter(|value| value.is_finite()).collect();
    (value.len() == 4).then(|| [value[0].clamp(-1e7, 1e7), value[1].clamp(-1e7, 1e7), value[2].clamp(10.0, 1e6), value[3].clamp(10.0, 1e6)])
}

fn color_of(parameters: &J) -> Option<[f32; 4]> {
    materialcraft_core::parse_color(parameters.get("color")?)
}

fn frame_add(session: &mut Session, parameters: &J) -> Result<J> {
    let rect = match rect_of(parameters) {
        Some(rect) => rect,
        None => nodes_rect(session, &session.selection)
            .map(|rect| [rect[0] - 24.0, rect[1] - 44.0, rect[2] + 48.0, rect[3] + 68.0])
            .unwrap_or([0.0, 0.0, 400.0, 260.0]),
    };

    let title = strp(parameters, "title").unwrap_or("Frame").to_string();
    let color = color_of(parameters).unwrap_or([materialcraft_core::srgb_to_linear(0.102), materialcraft_core::srgb_to_linear(0.553), 1.0, 0.251]);
    let title_visible = boolp(parameters, "titleVisible").unwrap_or(true);
    let description = strp(parameters, "description").unwrap_or("").to_string();

    session.edit("Add frame", |session| {
        let graph = session.graph_mut()?;
        let id = graph.alloc_id();
        graph.frames.push(Frame { id, title, title_visible, description, rect, color });
        session.selection.clear();
        session.annotation_selection = Some(AnnotationSelection::Frame(id));
        Ok(json!({"id": id}))
    })
}

fn frame_edit(session: &mut Session, parameters: &J) -> Result<J> {
    let id = u64p(parameters, "id").ok_or_else(|| bad("graph.frame.edit", "`id` is required"))?;

    session.edit_merged(&format!("frame{id}"), "Edit frame", |session| {
        let frame = session.graph_mut()?.frames.iter_mut().find(|frame| frame.id == id).ok_or_else(|| bad("graph.frame.edit", format!("no frame {id}")))?;
        if let Some(text) = strp(parameters, "title") {
            frame.title = text.into();
        }
        if let Some(visible) = boolp(parameters, "titleVisible") {
            frame.title_visible = visible;
        }
        if let Some(text) = strp(parameters, "description") {
            frame.description = text.into();
        }
        if let Some(rect) = rect_of(parameters) {
            frame.rect = rect;
        }
        if let Some(color) = color_of(parameters) {
            frame.color = color;
        }

        Ok(J::Null)
    })
}

fn frame_delete(session: &mut Session, parameters: &J) -> Result<J> {
    let id = u64p(parameters, "id").ok_or_else(|| bad("graph.frame.delete", "`id` is required"))?;

    session.edit("Delete frame", |session| {
        let graph = session.graph_mut()?;
        let previous_count = graph.frames.len();
        graph.frames.retain(|frame| frame.id != id);
        if previous_count == graph.frames.len() {
            return Err(bad("graph.frame.delete", format!("no frame {id}")));
        }

        Ok(J::Null)
    })
}

fn frame_move(session: &mut Session, parameters: &J) -> Result<J> {
    let id = u64p(parameters, "id").ok_or_else(|| bad("graph.frame.move", "`id` is required"))?;
    let offset = point(parameters, "by").ok_or_else(|| bad("graph.frame.move", "`by` is required"))?;
    let offset = crate::graph_grid::snap_position(offset, boolp(parameters, "snapToGrid").unwrap_or(false));
    let graph = session.graph()?;
    let frame = graph.frames.iter().find(|frame| frame.id == id).ok_or_else(|| bad("graph.frame.move", "frame does not exist"))?;
    let contents = FrameContents::capture(graph, frame);

    session.edit("Move frame and contents", |session| {
        contents.translate(session.graph_mut()?, offset);

        Ok(J::Null)
    })
}
