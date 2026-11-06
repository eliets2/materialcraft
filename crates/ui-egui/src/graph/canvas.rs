use super::node_style::{PORT_RADIUS, header_color, node_label, node_rect, port_color};
use super::{GraphCamera, GraphDrag, PortHit, WireRef};
use super::{
    navigation::frame,
    port_hit::nearest as nearest_port,
    wire_geometry::{bezier, bezier_points, wire_at},
};
use super::{node_card, node_card::port as port_pos, node_layout::NodeLayout};
use crate::i18n::tr;
use crate::{MaterialCraftApp, theme::Tokens};
use egui::{Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, Ui, Vec2, pos2, vec2};
use egui_phosphor::regular as icon;
use materialcraft_engine::core::NodeId;

/// The graph panel: toolbar row and canvas
pub fn show(app: &mut MaterialCraftApp, ui: &mut Ui) {
    app.reconcile_editors();
    if !crate::pattern::active(app) {
        app.pattern_editor = None;
        if app.session.graph().is_ok_and(|graph| graph.function.is_some()) {
            let target = crate::function::FunctionTarget::Graph;
            if app.function_editor.as_ref().is_none_or(|editor| editor.target != target) {
                app.function_editor = Some(crate::function::FunctionEditor::standalone(app.session.graph));
            }
        } else if app.function_editor.as_ref().is_some_and(|editor| matches!(editor.target, crate::function::FunctionTarget::Graph)) {
            app.function_editor = None;
        }
    }

    super::toolbar::show(app, ui);
    if app.function_editor.is_some() {
        crate::function::show_graph(app, ui);
        return;
    }
    if crate::pattern::active(app) {
        crate::pattern::show(app, ui);
        return;
    }

    canvas(app, ui);
}

fn canvas(app: &mut MaterialCraftApp, ui: &mut Ui) {
    let theme = Tokens::get(ui.ctx());
    let ctx = ui.ctx().clone();
    let (rect, resp) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
    crate::widgets::name(&resp, egui::Role::Canvas, "Node graph");
    app.graph.view = rect;
    if app.ui.frame_all || app.graph.framed_graph != Some(app.session.graph) {
        app.ui.frame_all = false;
        app.graph.framed_graph = Some(app.session.graph);
        app.graph.selected_wires.clear();
        frame(app, rect, false);
    }
    let painter = ui.painter_at(rect);
    // Background and grid
    painter.rect_filled(rect, 0.0, Color32::from_rgb(0x2b, 0x2d, 0x31));
    let mut camera = GraphCamera { origin: rect.min, pan: Vec2::from(app.ui.graph_camera.pan), zoom: app.ui.graph_camera.zoom };
    camera.zoom_from_input(ui, rect);
    app.ui.graph_camera = camera.state();
    super::grid::paint(&painter, &camera, rect);
    let pointer = ctx.input(|input| input.pointer.hover_pos());

    let show_thumbnails = app.ui.prefs.thumbnails;
    let Ok(graph) = app.session.graph() else { return };

    let index = materialcraft_engine::core::GraphIndex::new(graph);
    let selection: std::collections::BTreeSet<_> = app.session.selection.iter().copied().collect();
    let node_delta = match &app.graph.drag {
        Some(GraphDrag::Nodes { delta }) => *delta,
        _ => Vec2::ZERO,
    };

    // Layout every node and its ports

    let mut layouts: Vec<NodeLayout> = Vec::with_capacity(graph.nodes.len());
    for node in &graph.nodes {
        let (mut ins, outs) = materialcraft_engine::eval::ports_with_connections(&app.session.pkg, node, index.incoming(node.id).iter().copied());
        let visible =
            super::input_visibility::count(ins.iter().map(|port| index.input_of(node.id, &port.id).is_some()), if node.def == "fx_map" { 2 } else { 1 });
        ins.truncate(visible);
        let mut r = node_rect(node, ins.len(), outs.len(), show_thumbnails);
        if selection.contains(&node.id) && node_delta.length() > 0.5 {
            let position = materialcraft_engine::graph_grid::translate(node.pos, node_delta.into(), app.ui.prefs.snap_to_grid);
            r = r.translate(vec2(position[0] - node.pos[0], position[1] - node.pos[1]));
        }
        r = r.translate(super::annotation_canvas::offset(app, |contents| contents.nodes.contains(&node.id)));
        let dot = node.def == "dot";
        let port_position = |i: usize, output: bool, total: usize| port_pos(r, i, total, output, dot);
        layouts.push(NodeLayout {
            id: node.id,
            rect: r,
            ins: ins.iter().enumerate().map(|(i, p)| (p.id.clone(), p.label.clone(), p.kind, port_position(i, false, ins.len()))).collect(),
            outs: outs.iter().enumerate().map(|(i, p)| (p.id.clone(), p.label.clone(), p.kind, port_position(i, true, outs.len()))).collect(),
        });
    }
    app.graph.rects =
        layouts.iter().map(|layout| (layout.id, Rect::from_min_max(camera.to_screen(layout.rect.min), camera.to_screen(layout.rect.max)))).collect();
    app.graph.ports = layouts
        .iter()
        .flat_map(|layout| {
            let ins = layout.ins.iter().map(move |(id, label, kind, p)| PortHit {
                node: layout.id,
                output: false,
                port: id.clone(),
                label: label.clone(),
                pos: *p,
                kind: *kind,
            });
            let outs = layout.outs.iter().map(move |(id, label, kind, p)| PortHit {
                node: layout.id,
                output: true,
                port: id.clone(),
                label: label.clone(),
                pos: *p,
                kind: *kind,
            });
            ins.chain(outs)
        })
        .map(|mut h| {
            h.pos = camera.to_screen(h.pos);
            h
        })
        .collect();

    let frame_hits = super::annotation_canvas::frames(app, ui, &graph.frames, &camera);
    app.graph.frames = frame_hits.iter().map(|frame| (frame.id, frame.rect)).collect();

    let mut layout_indices = std::collections::BTreeMap::new();
    for (index, layout) in layouts.iter().enumerate() {
        layout_indices.entry(layout.id).or_insert(index);
    }

    // Wires
    let cooked = &app.session.cooked;
    let detached = match &app.graph.drag {
        Some(GraphDrag::Wire { detached: Some(d), .. }) => Some(d.clone()),
        _ => None,
    };

    let mut wires: Vec<(WireRef, [Pos2; 4])> = Vec::with_capacity(graph.connections.len());
    for c in &graph.connections {
        if detached.as_ref().is_some_and(|(node, p)| *node == c.to && *p == c.to_port) {
            continue;
        }
        let a = layout_indices.get(&c.from).and_then(|index| layouts.get(*index)).and_then(|layout| layout.outs.iter().find(|p| p.0 == c.from_port));
        let b = layout_indices.get(&c.to).and_then(|index| layouts.get(*index)).and_then(|layout| layout.ins.iter().find(|p| p.0 == c.to_port));
        if let (Some(a), Some(b)) = (a, b) {
            wires.push(((c.to, c.to_port.clone()), bezier_points(camera.to_screen(a.3), camera.to_screen(b.3))));
        }
    }
    // Hover: not where a node card or a port sits on top of the wire
    let over_card = |p: Pos2| app.graph.rects.iter().any(|(_, r)| r.contains(p)) || nearest_port(&app.graph.ports, p, 9.0).is_some();
    let wire_hover = match &app.graph.drag {
        None => pointer.filter(|p| rect.contains(*p) && !over_card(*p)).and_then(|p| wire_at(&wires, p)).cloned(),
        Some(GraphDrag::WirePress { wire, .. }) => Some(wire.clone()),
        Some(_) => None,
    };

    for (w, ctrl) in &wires {
        let Some(c) = index.input_of(w.0, &w.1) else { continue };

        let hot = selection.contains(&c.from) || selection.contains(&c.to);
        let picked = app.graph.selected_wires.contains(w);
        let hovered = wire_hover.as_ref() == Some(w);
        let base = (2.0 * camera.zoom).clamp(1.0, 3.0);
        let stroke = if picked {
            Stroke::new(base + 2.0, theme.accent)
        } else if hovered {
            Stroke::new(base + 1.0, Color32::WHITE)
        } else {
            Stroke::new(base, if hot { theme.accent } else { Color32::from_gray(0xb4) })
        };

        painter.add(egui::epaint::CubicBezierShape::from_points_stroke(*ctrl, false, Color32::TRANSPARENT, stroke));
    }
    app.graph.wires = wires;

    // Nodes
    let font = FontId::proportional((10.5 * camera.zoom).clamp(6.0, 16.0));
    let small = FontId::proportional((9.0 * camera.zoom).clamp(5.5, 13.0));
    let mut node_responses: Vec<(NodeId, egui::Response)> = Vec::new();
    for layout in &layouts {
        let Some(node) = index.node(layout.id) else { continue };

        let screen_rect = Rect::from_min_max(camera.to_screen(layout.rect.min), camera.to_screen(layout.rect.max));
        if !screen_rect.intersects(rect) {
            continue;
        }
        let selected = selection.contains(&node.id);
        let result = cooked.nodes.get(&node.id);
        let error = result.and_then(|r| r.error.as_ref());
        let resp = ui.interact(screen_rect, ui.scope_id().with(("node", node.id.0)), Sense::click_and_drag());
        crate::widgets::name(&resp, egui::Role::Button, &node_label(node));
        if node.def == "dot" {
            node_card::paint_dot(&painter, screen_rect, if selected { theme.accent } else { Color32::from_gray(0xb4) });
        } else {
            node_card::paint(&painter, screen_rect, camera.zoom, &node_label(node), header_color(&node.def));
            let header = node_card::header(screen_rect, camera.zoom);
            let body = node_card::body(screen_rect, camera.zoom);
            if let Some(r0) = result
                && let Some(img) = r0.outputs.first().and_then(materialcraft_engine::eval::NodeOutput::as_image)
                && show_thumbnails
                && !app.graph.thumbnails_cleared
            {
                let side = body.width().min(body.height()) - 6.0 * camera.zoom;
                let tr_ = Rect::from_center_size(body.center(), vec2(side, side));
                let tex = app.thumbs.node(&ctx, r0.key, 0, img);
                painter.image(tex, tr_, Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0)), if node.disabled { Color32::from_gray(90) } else { Color32::WHITE });
            } else if let Some(value) = result.and_then(|result| result.outputs.first()).and_then(materialcraft_engine::eval::NodeOutput::as_value) {
                let text = match value {
                    materialcraft_engine::core::Value::Float(value) => value.to_string(),
                    _ => serde_json::to_value(value).ok().and_then(|value| value.get("v").map(ToString::to_string)).unwrap_or_default(),
                };

                let text = crate::widgets::elided(&painter, text, font.size, theme.text, body.width() - 8.0);
                painter.galley(body.center() - text.size() * 0.5, text, theme.text);
            } else if result.is_none() {
                painter.text(body.center(), Align2::CENTER_CENTER, icon::HOURGLASS_MEDIUM, small.clone(), theme.text_dim);
            }
            // Footer: size and format
            let footer = Rect::from_min_max(pos2(screen_rect.left(), body.bottom()), screen_rect.max);
            if let Some(img) = result.and_then(|r| r.outputs.first().and_then(materialcraft_engine::eval::NodeOutput::as_image)) {
                let bits = if node.base.format == materialcraft_engine::core::OutputFormat::Inherit { graph.format.bits() } else { node.base.format.bits() };

                let format = format!("{}{bits}", if img.is_gray() { "L" } else { "C" });
                let txt = if let Some(limit) = app.session.preview_size {
                    let [width, height] = graph.size_px();
                    resp.clone().on_hover_text(format!(
                        "{} x {} ({bits}bpc)\n{}: {width} x {height}\n{}: {limit} px\n{}",
                        img.w,
                        img.h,
                        tr("Parent size"),
                        tr("Preview limit"),
                        tr("Change Preview size while editing in Edit > Preferences. Exports use each node's configured output size.")
                    ));
                    format!("{}x{} {}", img.w, img.h, tr("Preview"))
                } else {
                    format!("{}x{} - {format}", img.w, img.h)
                };
                painter.text(footer.center(), Align2::CENTER_CENTER, txt, small.clone(), theme.text_dim);
            }
            if node.def == "output" {
                painter.text(header.right_center() - vec2(4.0, 0.0), Align2::RIGHT_CENTER, icon::SIGN_OUT, small.clone(), Color32::WHITE);
            }
            let outline = if error.is_some() {
                Stroke::new(2.0, Color32::from_rgb(0xff, 0x30, 0xd0))
            } else if selected {
                Stroke::new(2.0, theme.accent)
            } else if resp.hovered() {
                Stroke::new(1.0, Color32::from_gray(0x90))
            } else {
                Stroke::new(1.0, Color32::from_gray(0x10))
            };

            node_card::outline(&painter, screen_rect, camera.zoom, outline);
            if node.disabled {
                painter.line_segment([screen_rect.left_top(), screen_rect.right_bottom()], Stroke::new(1.5, theme.danger));
            }
            if let Some(e) = error {
                resp.clone().on_hover_text(e);
            }
        }
        node_responses.push((node.id, resp));
    }

    let comment_hits = super::annotation_canvas::comments(app, ui, &graph.comments, &camera);
    app.graph.comments = comment_hits.clone();
    super::annotation_canvas::pins(app, ui, &graph.pins, &camera);

    // Ports (drawn last, interactable over the node cards)
    let port_hover = pointer.and_then(|p| nearest_port(&app.graph.ports, p, 9.0)).cloned();
    for h in &app.graph.ports {
        if !rect.contains(h.pos) {
            continue;
        }
        let hot = port_hover.as_ref().is_some_and(|x| x.node == h.node && x.port == h.port && x.output == h.output);
        let r = (PORT_RADIUS * camera.zoom).clamp(2.5, 7.0) * if hot { 1.4 } else { 1.0 };

        painter.circle(h.pos, r, port_color(h.kind), Stroke::new(1.0, Color32::from_gray(0x18)));
    }
    let hovered_node = app.graph.rects.iter().rev().find(|(_, bounds)| super::port_label::visible(ui, *bounds)).map(|(id, _)| *id);
    for port in &app.graph.ports {
        if Some(port.node) != hovered_node || !rect.contains(port.pos) {
            continue;
        }

        let anchor = if port.output { Align2::LEFT_CENTER } else { Align2::RIGHT_CENTER };

        let offset = if port.output { vec2(10.0, 0.0) } else { vec2(-10.0, 0.0) };

        super::port_label::paint(&painter, port.pos + offset, anchor, &port.label, camera.zoom, theme.text_strong);
    }

    // A wire under a comment is not hit
    let wire_hover = wire_hover.filter(|_| !pointer.is_some_and(|p| comment_hits.iter().any(|(_, r)| r.contains(p))));
    if wire_hover.is_some() {
        ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    app.graph.inspected_node = super::node_selection::inspected(&app.session.selection, app.graph.inspected_node);
    super::input::handle(app, ui, &resp, rect, &camera, &node_responses, &frame_hits, &comment_hits, port_hover, wire_hover);
    app.graph.inspected_node = super::node_selection::inspected(&app.session.selection, app.graph.inspected_node);

    // GraphDrag previews
    match &app.graph.drag {
        Some(GraphDrag::Box { start }) => {
            if let Some(p) = pointer {
                super::node_selection::paint_box(&painter, *start, p, theme.accent);
            }
        }
        Some(GraphDrag::Wire { from, .. }) => {
            if let (Some(p), Some(src)) = (pointer, app.graph.ports.iter().find(|h| h.output && h.node == from.0 && h.port == from.1)) {
                bezier(&painter, src.pos, p, Stroke::new(2.0, theme.accent));
            }
        }
        _ => {}
    }

    super::search::show(app, ui, &camera);
    let (failed, placeholders) = app.session.cook_failures();
    let mut at = rect.right_bottom() - vec2(8.0, 8.0);
    if !failed.is_empty() {
        let text = format!("{} {} node(s) failed", icon::WARNING, failed.len());
        at.y = painter.text(at, Align2::RIGHT_BOTTOM, text, FontId::proportional(11.0), theme.danger).top() - 2.0;
    }
    // Placeholders for missing SBS nodes are a soft warning, not a failure
    if placeholders > 0 {
        let text = format!("{} {placeholders} missing node(s), see the Console", icon::WARNING);
        painter.text(at, Align2::RIGHT_BOTTOM, text, FontId::proportional(11.0), theme.warning);
    }
}
