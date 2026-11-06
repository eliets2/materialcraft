use super::{GraphCamera, GraphDrag, LibraryDrag, NODE_W, PortHit, WireRef};
use super::{navigation::frame, port_hit::nearest as nearest_port, search::open_search, wire_actions::wire_menu};
use crate::MaterialCraftApp;
use egui::{Rect, Ui, Vec2};
use materialcraft_engine::core::NodeId;
use serde_json::json;

#[allow(clippy::too_many_arguments)]
pub(super) fn handle(
    app: &mut MaterialCraftApp,
    ui: &mut Ui,
    canvas: &egui::Response,
    rect: Rect,
    cam: &GraphCamera,
    nodes: &[(NodeId, egui::Response)],
    frames: &[super::frame_hit::FrameHit],
    comments: &[(u64, Rect)],
    port_hover: Option<PortHit>,
    wire_hover: Option<WireRef>,
) {
    let ctx = ui.ctx().clone();
    if super::gesture::cancelled(ui) {
        app.graph.drag = None;
        return;
    }
    let (pointer, mods, primary_pressed) = ctx.input(|input| (input.pointer.interact_pos(), input.modifiers, input.pointer.primary_pressed()));
    let zoom = cam.zoom;
    let motion = ctx.input(|i| i.pointer.delta()) / zoom;

    // Starting gestures (only on a press inside the canvas, with no gesture running)
    if app.graph.drag.is_none()
        && let Some(p) = pointer
        && rect.contains(p)
        && ui.rect_contains_pointer(rect)
        && app.graph.search.is_none()
    {
        if GraphCamera::pan_started(ui, rect) {
            app.graph.drag = Some(GraphDrag::Pan(super::gesture::pan_button(ui)));
        } else if primary_pressed {
            if app.session.annotation_selection.is_some() {
                app.run("graph.annotation.select", json!({}));
            }

            let on_card = nodes.iter().any(|(_, r)| r.rect.contains(p)) || comments.iter().any(|(_, r)| r.contains(p));
            let wire = wire_hover.as_ref().filter(|_| port_hover.is_none() && !on_card);
            // Any press other than on a wire drops the wire selection
            if wire.is_none() {
                app.graph.selected_wires.clear();
            }
            if let Some(w) = wire {
                app.graph.drag = Some(GraphDrag::WirePress { wire: w.clone(), start: p });
            } else if let Some(h) = &port_hover {
                if h.output {
                    app.graph.drag = Some(GraphDrag::Wire { from: (h.node, h.port.clone()), detached: None });
                } else if let Some(c) = app.session.graph().ok().and_then(|g| g.input_of(h.node, &h.port).cloned()) {
                    app.graph.drag = Some(GraphDrag::Wire { from: (c.from, c.from_port.clone()), detached: Some((h.node, h.port.clone())) });
                }
            } else if let Some((id, _)) = comments.iter().rev().find(|(_, r)| r.contains(p)) {
                super::annotation_input::begin_comment(app, *id);
            } else if let Some((id, _)) = nodes.iter().rev().find(|(_, r)| r.rect.contains(p)) {
                super::node_selection::select(&mut app.session.selection, Some(*id), mods);
                app.graph.drag = Some(GraphDrag::Nodes { delta: Vec2::ZERO });
            } else if let Some(hit) = frames.iter().rev().find(|frame| frame.contains(p)) {
                super::annotation_input::begin_frame(app, hit.id, hit.grip.contains(p));
            } else {
                app.graph.drag = Some(GraphDrag::Box { start: p });
            }
        }
    }

    // Continuing gestures
    match &mut app.graph.drag {
        Some(GraphDrag::Pan(_)) => {
            let d = ctx.input(|i| i.pointer.delta()) / zoom;
            app.ui.graph_camera.pan = [app.ui.graph_camera.pan[0] - d.x, app.ui.graph_camera.pan[1] - d.y];
        }
        Some(GraphDrag::Nodes { delta }) | Some(GraphDrag::Frame { delta, .. }) | Some(GraphDrag::Comment { delta, .. }) => *delta += motion,
        Some(GraphDrag::WirePress { wire, start }) => {
            // Dragged far enough: a reroute dot where the press was, carried by the pointer
            if let Some(p) = pointer
                && p.distance(*start) > 4.0
            {
                let (wire, start) = (wire.clone(), *start);
                let at = cam.to_graph(start);
                app.graph.drag = None;
                if app.run("graph.wire.insertDot", json!({"to": wire.0.0, "toPort": wire.1, "at": [at.x, at.y]})).is_some() {
                    app.graph.drag = Some(GraphDrag::Nodes { delta: (p - start) / zoom });
                }
            }
        }
        _ => {}
    }

    // Ending gestures
    let owner = match app.graph.drag {
        Some(GraphDrag::Pan(button)) => button,
        _ => egui::PointerButton::Primary,
    };
    let ended = super::gesture::finished(ui, owner);
    if ended && let Some(drag) = app.graph.drag.take() {
        match drag {
            GraphDrag::Pan(_) => {}
            GraphDrag::WirePress { wire, .. } => {
                app.session.selection.clear();
                let sel = &mut app.graph.selected_wires;
                if mods.ctrl && sel.contains(&wire) {
                    // Ctrl+click toggles a wire out of the selection
                    sel.retain(|w| *w != wire);
                } else if mods.ctrl || mods.shift {
                    if !sel.contains(&wire) {
                        sel.push(wire);
                    }
                } else {
                    *sel = vec![wire];
                }
            }
            GraphDrag::Nodes { delta } => {
                if delta.length() > 0.5 {
                    app.run("graph.node.move", json!({"by": [delta.x, delta.y]}));
                }
            }
            GraphDrag::Box { start } => {
                if let Some(p) = pointer {
                    super::node_selection::select_box(&mut app.session.selection, app.graph.rects.iter().copied(), start, p, mods);
                }
            }
            GraphDrag::Wire { from, detached } => {
                let target = pointer.and_then(|p| nearest_port(&app.graph.ports, p, 12.0)).filter(|h| !h.output).cloned();
                match target {
                    Some(t) => {
                        let mut parameters = json!({"from": from.0.0, "fromPort": from.1, "to": t.node.0, "toPort": t.port});
                        if let Some((node, port)) = detached {
                            parameters["detach"] = json!({"to": node.0, "toPort": port});
                        }

                        app.run("graph.connect", parameters);
                    }
                    None => {
                        if let Some((n, p)) = &detached {
                            app.run("graph.disconnect", json!({"to": n.0, "toPort": p}));
                        } else if let Some(p) = pointer
                            && rect.contains(p)
                        {
                            open_search(app, cam.to_graph(p), Some(from));
                        }
                    }
                }
            }
            GraphDrag::Frame { .. } | GraphDrag::Comment { .. } => super::annotation_input::finish(app, drag),
        }
    }

    // Clicks
    for (id, r) in nodes {
        if r.double_clicked() {
            app.run("view.2d", json!({"node": id.0, "port": 0}));
            if crate::menus::enabled(app, "graph.node.edit") {
                crate::menus::invoke(app, ui.ctx(), "graph.node.edit");
            }
        } else if r.clicked() && app.session.selection.contains(id) && app.session.view2d.node != Some(*id) {
            // Keep the current output when deselecting or clicking the node already shown
            app.run("view.2d", json!({"node": id.0}));
        }
        if r.secondary_clicked()
            && let Some(pointer) = pointer
        {
            app.graph.menu_at = cam.to_graph(pointer);
        }

        let at = app.graph.menu_at;
        r.context_menu(|ui| super::context_menu::show(app, ui, at, Some(*id)));
    }
    if let Some(h) = &port_hover
        && h.output
        && ui.rect_contains_pointer(rect)
        && ctx.input(|i| i.pointer.primary_clicked())
    {
        let idx = app.session.output_ports(h.node).iter().position(|p| *p == h.port).unwrap_or(0);
        app.run("view.2d", json!({"node": h.node.0, "port": idx}));
    }
    let on_node = pointer.is_some_and(|p| {
        nodes.iter().any(|(_, r)| r.rect.contains(p)) || comments.iter().any(|(_, r)| r.contains(p)) || frames.iter().any(|frame| frame.contains(p))
    });
    if canvas.double_clicked()
        && !on_node
        && let Some(p) = pointer
    {
        match &wire_hover {
            // Double-click a wire: a reroute dot there
            Some(w) => {
                let at = cam.to_graph(p);
                app.run("graph.wire.insertDot", json!({"to": w.0.0, "toPort": w.1, "at": [at.x, at.y]}));
                app.graph.selected_wires.clear();
            }
            None => open_search(app, cam.to_graph(p), None),
        }
    }
    if canvas.secondary_clicked()
        && let Some(p) = pointer
    {
        app.graph.menu_at = cam.to_graph(p);
        if let Some((id, _)) = comments.iter().rev().find(|(_, rect)| rect.contains(p)) {
            app.run("graph.annotation.select", json!({"kind": "comment", "id": id}));
        } else if let Some(hit) = frames.iter().rev().find(|frame| frame.contains(p)) {
            app.run("graph.annotation.select", json!({"kind": "frame", "id": hit.id}));
        }
        app.graph.menu_wire = wire_hover.clone().filter(|_| !on_node);
        // Right-clicking a selected wire keeps the selection (to swap or delete them all)
        if let Some(w) = &app.graph.menu_wire
            && !app.graph.selected_wires.contains(w)
        {
            app.session.selection.clear();
            app.graph.selected_wires = vec![w.clone()];
        }
    }
    let at = app.graph.menu_at;
    canvas.context_menu(|ui| match app.graph.menu_wire.clone() {
        Some(w) => wire_menu(app, ui, w, at),
        None => super::context_menu::show(app, ui, at, None),
    });

    // Library drops
    if let Some(payload) = crate::widgets::dnd_release::<LibraryDrag>(canvas)
        && let Some(p) = pointer
    {
        let gp = cam.to_graph(p);
        let at = [gp.x - NODE_W * 0.5, gp.y - 20.0];
        match payload.0.strip_prefix('@') {
            Some(graph) => app.run("graph.node.add", json!({"def": "graph_instance", "at": at, "params": {"graph": graph}})),
            None => app.run("graph.node.add", json!({"def": payload.0, "at": at})),
        };
    }

    // Keys over the canvas
    let hovered = pointer.is_some_and(|p| rect.contains(p));
    if hovered && app.graph.search.is_none() {
        let space = !ctx.egui_wants_keyboard_input() && ctx.input(|i| i.key_pressed(egui::Key::Space));
        let tab = !app.graph.keyboard_was_focused && !ctx.text_edit_focused() && ctx.input(|i| i.key_pressed(egui::Key::Tab));
        if (space || tab)
            && let Some(p) = pointer
        {
            open_search(app, cam.to_graph(p), None);
        }
        if GraphCamera::frame_key(ui, rect) {
            let sel = !app.session.selection.is_empty();
            frame(app, rect, sel);
        }
    }
}
