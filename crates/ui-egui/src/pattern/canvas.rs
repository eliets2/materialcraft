use crate::graph::canvas_geometry::CanvasGeometry;
use egui::{Color32, Rect, Sense, Stroke, Ui, vec2};
use materialcraft_engine::core::NodeId;
use materialcraft_engine::nodes::PortKind;
use std::collections::BTreeMap;

use super::{PatternDrag, PatternEditor, geometry};
use crate::{
    MaterialCraftApp,
    graph::{GraphCamera, PortHit},
    i18n::tr,
    theme::Tokens,
};

const PORT_COLOR: Color32 = Color32::from_rgb(165, 157, 215);

pub(crate) fn show(app: &mut MaterialCraftApp, ui: &mut Ui) {
    let Some(mut editor) = app.pattern_editor.take() else { return };

    if let Some(graph) = editor.current(app) {
        let present: std::collections::BTreeSet<_> = graph.nodes.iter().map(|node| node.id).collect();
        editor.selection.retain(|id| present.contains(id));
    }
    let Some((geometry, response, camera)) = paint(app, &mut editor, ui) else { return };

    app.ui.frame_all = false;
    geometry.publish(&mut app.graph, response.rect);
    app.graph.frames.clear();
    app.graph.comments.clear();
    super::input::handle(app, &mut editor, ui, &response, &camera);
    if let Some(PatternDrag::Wire { node, slot }) = editor.drag
        && let Some(source) = app.graph.ports.iter().find(|port| {
            port.node.0 == u64::from(node)
                && port.output == slot.is_some()
                && slot.is_none_or(|slot| port.port == format!("output{slot}") || slot == 0 && port.port == "output")
        })
        && let Some(pointer) = ui.input(|input| input.pointer.hover_pos())
    {
        let points = if slot.is_some() { geometry::wire(source.pos, pointer) } else { geometry::wire(pointer, source.pos) };
        ui.painter_at(response.rect).add(egui::epaint::CubicBezierShape::from_points_stroke(points, false, Color32::TRANSPARENT, Stroke::new(2.0, PORT_COLOR)));
    }
    response.context_menu(|ui| super::context_menu::show(app, &mut editor, ui));
    editor.inspected_node = crate::graph::node_selection::inspected(&editor.selection, editor.inspected_node);
    app.pattern_editor = Some(editor);
}

fn paint(app: &MaterialCraftApp, editor: &mut PatternEditor, ui: &mut Ui) -> Option<(CanvasGeometry, egui::Response, GraphCamera)> {
    let graph = editor.current(app)?;
    let selected: std::collections::BTreeSet<_> = editor.selection.iter().copied().collect();
    let mut geometry = CanvasGeometry::default();
    let (bounds, response) = ui.allocate_exact_size(ui.available_size().max(vec2(100.0, 100.0)), Sense::click_and_drag());
    crate::widgets::name(&response, egui::Role::Canvas, "FX-Map graph");
    let painter = ui.painter_at(bounds);
    painter.rect_filled(bounds, 0.0, Color32::from_rgb(0x26, 0x28, 0x2c));
    let snap = app.ui.prefs.snap_to_grid;
    let frame_key = GraphCamera::frame_key(ui, bounds);
    if !editor.framed || app.ui.frame_all || frame_key {
        let selected_only = frame_key && !editor.selection.is_empty();
        let destination = graph
            .nodes
            .iter()
            .filter(|node| !selected_only || selected.contains(&node.id))
            .map(|node| geometry::bounds(node, editor, snap, selected.contains(&node.id)))
            .fold(Rect::NOTHING, |all, node| all.union(node));
        let camera = GraphCamera::fit(bounds, destination);
        editor.pan = camera.pan;
        editor.zoom = camera.zoom;
        editor.framed = true;
    }

    let mut camera = GraphCamera { origin: bounds.min, pan: editor.pan, zoom: editor.zoom };
    camera.zoom_from_input(ui, bounds);
    editor.pan = camera.pan;
    editor.zoom = camera.zoom;
    crate::graph::grid::paint(&painter, &camera, bounds);
    let rectangles: BTreeMap<_, _> = graph
        .nodes
        .iter()
        .map(|node| {
            let bounds = geometry::bounds(node, editor, snap, selected.contains(&node.id));
            (node.id, Rect::from_min_max(camera.to_screen(bounds.min), camera.to_screen(bounds.max)))
        })
        .collect();
    for node in &graph.nodes {
        let Some(bounds) = rectangles.get(&node.id) else { continue };

        for (slot, child) in node.children.iter().enumerate() {
            if let Some(child_bounds) = child.and_then(|id| rectangles.get(&id)) {
                let points = geometry::wire(geometry::port(*bounds, Some(slot), node.children.len()), child_bounds.center_top());
                painter.add(egui::epaint::CubicBezierShape::from_points_stroke(points, false, Color32::TRANSPARENT, Stroke::new(2.0, PORT_COLOR)));
                geometry.wires.push(((NodeId(u64::from(node.id)), node.kind.child_port(slot).unwrap_or_default().to_string()), points));
            }
        }
    }

    let tokens = Tokens::get(ui.ctx());
    for node in &graph.nodes {
        let Some(bounds) = rectangles.get(&node.id).copied() else { continue };

        geometry.rects.push((NodeId(u64::from(node.id)), bounds));
        let outline = if selected.contains(&node.id) { tokens.accent } else { Color32::from_gray(30) };

        super::node_canvas::paint(&painter, bounds, node.kind, editor.zoom, outline);
        if graph.root == Some(node.id) {
            painter.text(
                bounds.right_top() + vec2(-4.0, 4.0) * editor.zoom,
                egui::Align2::RIGHT_TOP,
                egui_phosphor::regular::STAR,
                egui::FontId::proportional(11.0 * editor.zoom),
                Color32::GOLD,
            );
        }
        let node_response = ui.interact(bounds, response.id.with(("node", node.id)), Sense::hover());
        crate::widgets::name(&node_response, egui::Role::Button, node.kind.label());

        for slot in std::iter::once(None).chain((0..node.children.len()).map(Some)) {
            let position = geometry::port(bounds, slot, node.children.len());
            let label = slot.map_or("Input", |slot| geometry::port_label(node.kind, slot));
            painter.circle_filled(position, 5.0 * editor.zoom, PORT_COLOR);
            painter.circle_stroke(position, 5.0 * editor.zoom, Stroke::new(1.5, Color32::from_gray(30)));
            ui.interact(Rect::from_center_size(position, vec2(16.0, 16.0)), response.id.with((node.id, slot)), Sense::hover())
                .on_hover_text(tr("Drag to connect; Alt-click to disconnect"));
            geometry.ports.push(PortHit {
                node: NodeId(u64::from(node.id)),
                output: slot.is_some(),
                port: slot.map_or_else(|| "input".into(), |slot| node.kind.child_port(slot).unwrap_or_default().to_string()),
                label: label.into(),
                pos: position,
                kind: PortKind::Any,
            });
        }
    }

    Some((geometry, response, camera))
}
