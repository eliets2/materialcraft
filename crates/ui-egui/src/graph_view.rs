//! The graph editor: a pannable, zoomable canvas of node cards (colored header, live thumbnail,
//! size label), bezier wires between typed ports, frames, comments and navigation pins.
//!
//! Gestures: left-drag a node to move it (the selection moves together), drag from an output to wire
//! it (dropping on empty canvas opens the node search, wired), drag from an unconnected input to an
//! output to wire it the other way, drag a connected input to unplug and re-route it, drag the empty
//! canvas to box-select (Ctrl adds), middle-drag or Alt+drag to pan, wheel to zoom.
//! Click a node to show it in the 2D view (double-click a graph instance to open its graph).
//! Space, Tab or double-click the canvas for the node search, right-click for menus, drop library
//! items to add them. Click a wire to select it (Shift or Ctrl+click adds more; Delete removes them,
//! X swaps where they come from), drag a wire to pull a reroute dot out of it, double-click a wire
//! to drop a dot there, right-click a wire for its menu. Every edit is an engine command; the
//! gestures only draw previews until they commit.

use egui::{Align2, Color32, CornerRadius, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2, pos2, vec2};
use egui_phosphor::regular as icon;
use materialcraft_engine::core::{Node, NodeId, Value};
use materialcraft_engine::nodes::PortKind;
use serde_json::json;

use crate::MaterialCraftApp;
use crate::i18n::tr;
use crate::theme::Tokens;

/// Node card metrics in graph units.
pub const NODE_W: f32 = 100.0;
const HEADER_H: f32 = 18.0;
const FOOTER_H: f32 = 14.0;
const PORT_GAP: f32 = 18.0;
const PORT_R: f32 = 4.5;
const DOT_R: f32 = 7.0;

/// A library item being dragged onto the canvas.
#[derive(Clone, Debug)]
pub struct LibraryDrag(pub String);

#[derive(Clone, Debug)]
enum Drag {
    Nodes {
        delta: Vec2,
    },
    Box {
        start: Pos2,
    },
    Wire {
        from: (NodeId, String),
        detached: Option<(NodeId, String)>,
    },
    /// Pressed on a wire: a click selects it, a drag pulls a reroute dot out of it.
    WirePress {
        wire: WireRef,
        start: Pos2,
    },
    WireFromInput {
        to: (NodeId, String),
    },

    Pan,
    Frame {
        id: u64,
        delta: Vec2,
        resize: bool,
        nodes: Vec<NodeId>,
    },
    Comment {
        id: u64,
        delta: Vec2,
    },
}

/// A wire, named by the input it feeds: (node, input port).
pub type WireRef = (NodeId, String);

/// Pointer distance (points) within which a wire is hit.
const WIRE_HIT: f32 = 5.0;

/// The node search popup (Space or Tab).
#[derive(Clone, Debug)]
struct Search {
    at: Pos2,
    text: String,
    sel: usize,
    connect: Option<(NodeId, String)>,
    focus: bool,
}

/// One port as drawn this frame.
#[derive(Clone, Debug)]
pub struct PortHit {
    pub node: NodeId,
    pub output: bool,
    pub port: String,
    pub label: String,
    pub pos: Pos2,
    pub kind: PortKind,
}

pub struct GraphView {
    pub(crate) keyboard_was_focused: bool,
    drag: Option<Drag>,
    search: Option<Search>,
    /// Comment being edited: id and text.
    editing: Option<(u64, String)>,
    /// Node rectangles on screen last frame (for `ui.inspect` and scripted input).
    pub rects: Vec<(NodeId, Rect)>,
    pub ports: Vec<PortHit>,
    /// The canvas rectangle last frame.
    pub view: Rect,
    /// Graph point of the last right-click (context menu actions add things there).
    menu_at: Pos2,
    /// The graph the camera last framed.
    framed_graph: Option<usize>,
    /// The selected wires, in the order they were picked (Delete removes them, X swaps them).
    pub selected_wires: Vec<WireRef>,
    /// The wire the context menu is for (right-clicked on a wire).
    menu_wire: Option<WireRef>,
    /// Wires on screen last frame: the wire and its bezier control points.
    pub wires: Vec<(WireRef, [Pos2; 4])>,
}

impl Default for GraphView {
    fn default() -> Self {
        GraphView {
            keyboard_was_focused: false,
            drag: None,
            search: None,
            editing: None,
            rects: Vec::new(),
            ports: Vec::new(),
            view: Rect::NOTHING,
            menu_at: Pos2::ZERO,
            framed_graph: None,
            selected_wires: Vec::new(),
            menu_wire: None,
            wires: Vec::new(),
        }
    }
}

struct Cam {
    origin: Pos2,
    pan: Vec2,
    zoom: f32,
}

impl Cam {
    fn to_screen(&self, p: Pos2) -> Pos2 {
        self.origin + (p.to_vec2() - self.pan) * self.zoom
    }
    fn to_graph(&self, s: Pos2) -> Pos2 {
        ((s - self.origin) / self.zoom + self.pan).to_pos2()
    }
}

/// Header strip color of a node by its library category.
pub fn header_color(def: &str) -> Color32 {
    let cat = materialcraft_engine::nodes::find(def).map_or("", |d| d.category);
    match def {
        "output" => Color32::from_rgb(0x2c, 0x8f, 0x5e),
        "input_color" | "input_grayscale" => Color32::from_rgb(0x6d, 0x52, 0xb5),
        "graph_instance" => Color32::from_rgb(0x1e, 0x8a, 0xa0),
        _ if cat.starts_with("Atomic") => Color32::from_rgb(0x5c, 0x62, 0x6e),
        _ if cat.contains("Noises") => Color32::from_rgb(0xa8, 0x3c, 0x34),
        _ if cat.contains("Patterns") => Color32::from_rgb(0xb3, 0x5a, 0x2c),
        _ if cat.starts_with("Material") => Color32::from_rgb(0x3a, 0x66, 0xb0),
        _ if cat.starts_with("Filters") => Color32::from_rgb(0x3b, 0x7f, 0x4c),
        _ => Color32::from_rgb(0x5c, 0x62, 0x6e),
    }
}

pub fn port_color(kind: PortKind) -> Color32 {
    match kind {
        PortKind::Gray => Color32::from_rgb(0xc8, 0xc8, 0xc8),
        PortKind::Color => Color32::from_rgb(0xe8, 0xa9, 0x3c),
        PortKind::Any => Color32::from_rgb(0x8c, 0xb4, 0xe0),
    }
}

/// A node's display label.
pub fn node_label(n: &Node) -> String {
    if !n.label.is_empty() {
        return n.label.clone();
    }
    match n.def.as_str() {
        "graph_instance" => match n.params.get("graph") {
            Some(Value::Text(g)) => g.clone(),
            _ => tr("Graph instance").into(),
        },
        "output" | "input_color" | "input_grayscale" => materialcraft_engine::eval::identifier(n),
        d => materialcraft_engine::nodes::find(d).map_or(d, |d| d.label).to_string(),
    }
}

/// Graph-space rectangle of a node card.
pub fn node_rect(n: &Node, n_in: usize, n_out: usize, thumbs: bool) -> Rect {
    if n.def == "dot" {
        return Rect::from_center_size(pos2(n.pos[0], n.pos[1]), vec2(DOT_R * 2.0, DOT_R * 2.0));
    }
    let ports = n_in.max(n_out) as f32 * PORT_GAP + 8.0;
    let body = if thumbs { NODE_W.max(ports) } else { ports.max(26.0) };
    Rect::from_min_size(pos2(n.pos[0], n.pos[1]), vec2(NODE_W, HEADER_H + body + FOOTER_H))
}

/// Graph-space port positions along a card side.
fn port_pos(r: Rect, i: usize, n: usize, output: bool, dot: bool) -> Pos2 {
    if dot {
        return pos2(if output { r.right() } else { r.left() }, r.center().y);
    }
    let top = r.top() + HEADER_H;
    let body = r.height() - HEADER_H - FOOTER_H;
    let start = top + (body - n as f32 * PORT_GAP) * 0.5 + PORT_GAP * 0.5;
    pos2(if output { r.right() } else { r.left() }, start + i as f32 * PORT_GAP)
}

/// Control points of the wire from output `a` to input `b` (screen space).
fn bezier_points(a: Pos2, b: Pos2) -> [Pos2; 4] {
    let dx = ((b.x - a.x).abs() * 0.5).max(30.0);
    [a, a + vec2(dx, 0.0), b - vec2(dx, 0.0), b]
}

fn bezier(p: &egui::Painter, a: Pos2, b: Pos2, stroke: Stroke) {
    p.add(egui::epaint::CubicBezierShape::from_points_stroke(bezier_points(a, b), false, Color32::TRANSPARENT, stroke));
}

/// The point at `t` (0..1) along a cubic bezier.
fn bezier_at(c: &[Pos2; 4], t: f32) -> Pos2 {
    let u = 1.0 - t;
    let w = [u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t];
    pos2(c.iter().zip(w).map(|(p, w)| p.x * w).sum(), c.iter().zip(w).map(|(p, w)| p.y * w).sum())
}

/// Distance from `p` to a cubic bezier (sampled as a polyline).
fn bezier_distance(c: &[Pos2; 4], p: Pos2) -> f32 {
    const N: usize = 32;
    let mut best = f32::INFINITY;
    let mut prev = c[0];
    for i in 1..=N {
        let q = bezier_at(c, i as f32 / N as f32);
        let (seg, rel) = (q - prev, p - prev);
        let t = if seg.length_sq() > 0.0 { (rel.dot(seg) / seg.length_sq()).clamp(0.0, 1.0) } else { 0.0 };
        best = best.min((prev + seg * t).distance(p));
        prev = q;
    }
    best
}

/// The wire nearest `p` within [`WIRE_HIT`] points.
fn wire_at(wires: &[(WireRef, [Pos2; 4])], p: Pos2) -> Option<&WireRef> {
    wires
        .iter()
        .filter(|(_, c)| Rect::from_points(c).expand(WIRE_HIT).contains(p))
        .map(|(w, c)| (w, bezier_distance(c, p)))
        .filter(|(_, d)| *d <= WIRE_HIT)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(w, _)| w)
}

/// The graph panel: toolbar row and canvas.
pub fn show(app: &mut MaterialCraftApp, ui: &mut Ui) {
    toolbar(app, ui);
    canvas(app, ui);
}

fn toolbar(app: &mut MaterialCraftApp, ui: &mut Ui) {
    let tk = Tokens::get(ui.ctx());
    egui::Frame::NONE.fill(tk.panel).inner_margin(egui::Margin::symmetric(6, 3)).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            let b = |ui: &mut Ui, ic: &str, tip: &str| crate::widgets::icon_button(ui, ic, tip, false, 22.0).clicked();
            if b(ui, icon::CORNERS_OUT, tr("Frame all (F)")) {
                app.ui.frame_all = true;
            }
            for (ic, mode, tip) in [
                (icon::ALIGN_LEFT, "left", "Align left"),
                (icon::ALIGN_TOP, "top", "Align top"),
                (icon::ALIGN_CENTER_HORIZONTAL, "hcenter", "Align centers horizontally"),
                (icon::ALIGN_CENTER_VERTICAL, "vcenter", "Align centers vertically"),
                (icon::COLUMNS, "hdistribute", "Distribute horizontally"),
                (icon::ROWS, "vdistribute", "Distribute vertically"),
            ] {
                if b(ui, ic, tr(tip)) {
                    app.run("graph.align", json!({"mode": mode}));
                }
            }
            ui.separator();
            if b(ui, icon::SELECTION, tr("Add frame around the selection")) {
                app.run("graph.frame.add", json!({}));
            }
            if b(ui, icon::NOTE, tr("Add comment")) {
                app.run("graph.comment.add", json!({}));
            }
            if b(ui, icon::MAP_PIN, tr("Add navigation pin")) {
                let c = view_center(app);
                app.run("graph.pin.add", json!({"at": [c.x, c.y]}));
            }
            ui.separator();
            // Quick insert of the atomic nodes.
            for (ic, def) in [
                (icon::IMAGE, "bitmap"),
                (icon::PALETTE, "uniform"),
                (icon::STACK, "blend"),
                (icon::DROP, "blur"),
                (icon::SHUFFLE, "channels_shuffle"),
                (icon::CHART_LINE, "curve"),
                (icon::WIND, "directional_blur"),
                (icon::ARROWS_OUT_LINE_HORIZONTAL, "directional_warp"),
                (icon::RULER, "distance"),
                (icon::MOUNTAINS, "emboss"),
                (icon::GRADIENT, "gradient_map"),
                (icon::CIRCLE_HALF, "grayscale_conversion"),
                (icon::SWATCHES, "hsl"),
                (icon::SLIDERS_HORIZONTAL, "levels"),
                (icon::CUBE, "normal"),
                (icon::CODE, "pixel_processor"),
                (icon::DIAMOND, "sharpen"),
                (icon::ARROWS_CLOCKWISE, "transformation"),
                (icon::WAVES, "warp"),
                (icon::SIGN_IN, "input_grayscale"),
                (icon::SIGN_OUT, "output"),
            ] {
                let label = materialcraft_engine::nodes::find(def).map_or(def, |d| d.label);
                if b(ui, ic, label) {
                    add_node_near_selection(app, def);
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let thumbs = app.ui.prefs.thumbnails;
                if crate::widgets::icon_button(ui, icon::IMAGE, tr("Show node thumbnails"), thumbs, 22.0).clicked() {
                    app.ui.prefs.thumbnails = !thumbs;
                }
                parent_size(app, ui);
            });
        });
    });
}

/// "Parent size: [1024 ▾] × [1024 ▾]".
fn parent_size(app: &mut MaterialCraftApp, ui: &mut Ui) {
    let Ok(g) = app.session.graph() else { return };
    let [w, h] = g.size;
    let mut nw = w;
    let mut nh = h;
    let combo = |ui: &mut Ui, id: &str, v: &mut u32| {
        let r = egui::ComboBox::from_id_salt(id).width(64.0).selected_text(format!("{}", 1u32 << *v)).show_ui(ui, |ui| {
            for l in 4..=13u32 {
                ui.selectable_value(v, l, format!("{}", 1u32 << l));
            }
        });
        crate::widgets::name(&r.response, egui::Role::ComboBox, if id == "parent-w" { "Parent width" } else { "Parent height" });
    };
    combo(ui, "parent-h", &mut nh);
    ui.label("x");
    combo(ui, "parent-w", &mut nw);
    ui.label(egui::RichText::new(tr("Parent size:")).color(Tokens::get(ui.ctx()).text_dim));
    if (nw, nh) != (w, h) {
        app.run("graph.size", json!({"log2": [nw, nh]}));
    }
}

fn view_center(app: &MaterialCraftApp) -> Pos2 {
    let c = &app.ui.graph_camera.sanitized();
    let v = app.graph.view;
    pos2(c.pan[0] + v.width() * 0.5 / c.zoom, c.pan[1] + v.height() * 0.5 / c.zoom)
}

/// Add `def` right of the last selected node (wired from it when it fits), else mid-view.
pub fn add_node_near_selection(app: &mut MaterialCraftApp, def: &str) {
    let sel = app.session.selection.last().copied();
    let has_inputs = materialcraft_engine::nodes::find(def).is_some_and(|d| !d.inputs.is_empty());
    let params = match sel {
        Some(id) if has_inputs && !app.session.output_ports(id).is_empty() => json!({"def": def, "connectFrom": {"node": id.0}}),
        _ => {
            let c = view_center(app);
            json!({"def": def, "at": [c.x - NODE_W * 0.5, c.y - 60.0]})
        }
    };
    app.run("graph.node.add", params);
}

/// Add a Bitmap node (wired to the selection when that makes sense) showing `resource`.
pub fn add_bitmap_node(app: &mut MaterialCraftApp, resource: &str) {
    add_node_near_selection(app, "bitmap");
    if let Some(id) = app.session.selection.last().copied() {
        app.run("graph.node.param", json!({"id": id.0, "param": "resource", "value": resource}));
    }
}

/// File ▸ Import bitmap from a path: import, add the node and bind it, as one undo entry.
#[cfg(not(target_arch = "wasm32"))]
pub fn import_bitmap_file(app: &mut MaterialCraftApp, path: &str) {
    app.session.begin_group("Import bitmap");
    if let Some(r) = app.run("file.importBitmap", json!({"path": path})) {
        add_bitmap_node(app, r["name"].as_str().unwrap_or_default());
    }
    app.session.end_group();
}

fn run_gesture(app: &mut MaterialCraftApp, label: &str, commands: &[(&str, serde_json::Value)]) {
    let result = app.session.group(label, |session| {
        for (id, params) in commands {
            crate::cook::execute_guarded(session, id, params).map_err(materialcraft_engine::EngineError::Other)?;
        }
        Ok(())
    });
    if let Err(error) = result {
        app.toast(error.to_string(), true);
    }
}

/// Finish dragging a wire onto input `to_port` of `to`: connect it, first removing the link it
/// was pulled off (`detached`). The gesture is one undo entry.
fn rewire(app: &mut MaterialCraftApp, from: &(NodeId, String), detached: Option<&(NodeId, String)>, to: NodeId, to_port: &str) {
    let mut commands = Vec::new();
    if let Some((n, p)) = detached
        && (*n != to || p != to_port)
    {
        commands.push(("graph.disconnect", json!({"to": n.0, "toPort": p})));
    }
    commands.push(("graph.connect", json!({"from": from.0.0, "fromPort": from.1, "to": to.0, "toPort": to_port})));
    run_gesture(app, "Connect", &commands);
}

/// Finish dragging a frame: set its rectangle and move the nodes it carries, as one undo entry.
fn move_frame(app: &mut MaterialCraftApp, id: u64, rect: [f32; 4], nodes: &[NodeId], delta: Vec2) {
    let mut commands = vec![("graph.frame.edit", json!({"id": id, "rect": rect}))];
    if !nodes.is_empty() {
        commands.push(("graph.node.move", json!({"ids": nodes.iter().map(|n| n.0).collect::<Vec<_>>(), "by": [delta.x, delta.y]})));
    }
    run_gesture(app, "Move frame", &commands);
}

/// Frame the graph (or the selection) in the view.
fn frame(app: &mut MaterialCraftApp, view: Rect, only_selection: bool) {
    let thumbs = app.ui.prefs.thumbnails;
    let Ok(g) = app.session.graph() else { return };
    let mut r = Rect::NOTHING;
    for n in &g.nodes {
        if only_selection && !app.session.selection.contains(&n.id) {
            continue;
        }
        let (i, o) = materialcraft_engine::eval::ports(&app.session.pkg, n);
        r = r.union(node_rect(n, i.len(), o.len(), thumbs));
    }
    for f in &g.frames {
        if !only_selection {
            r = r.union(Rect::from_min_size(pos2(f.rect[0], f.rect[1]), vec2(f.rect[2], f.rect[3])));
        }
    }
    if !r.is_finite() || r.width() <= 0.0 {
        app.ui.graph_camera = crate::state::GraphCamera::default();
        return;
    }
    let r = r.expand(40.0);
    let zoom = (view.width() / r.width()).min(view.height() / r.height()).clamp(0.1, 1.5);
    let pan = r.center().to_vec2() - vec2(view.width(), view.height()) * 0.5 / zoom;
    app.ui.graph_camera = crate::state::GraphCamera { pan: [pan.x, pan.y], zoom };
}

fn canvas(app: &mut MaterialCraftApp, ui: &mut Ui) {
    let tk = Tokens::get(ui.ctx());
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
    // Background and grid.
    painter.rect_filled(rect, 0.0, Color32::from_rgb(0x2b, 0x2d, 0x31));
    app.ui.graph_camera = app.ui.graph_camera.sanitized();
    let cam = Cam { origin: rect.min, pan: Vec2::from(app.ui.graph_camera.pan), zoom: app.ui.graph_camera.zoom };
    grid(&painter, &cam, rect);

    let pointer = ctx.input(|i| i.pointer.hover_pos());
    let hovered_canvas = pointer.is_some_and(|p| rect.contains(p)) && resp.hovered();

    // Zoom around the pointer.
    if hovered_canvas || ui.rect_contains_pointer(rect) {
        let scroll = ctx.input(|i| i.smooth_scroll_delta.y + i.zoom_delta().ln() * 200.0);
        if scroll.abs() > 0.0
            && let Some(p) = pointer
            && rect.contains(p)
        {
            let before = cam.to_graph(p);
            let zoom = (cam.zoom * (scroll * 0.0025).exp()).clamp(0.08, 4.0);
            let pan = before.to_vec2() - (p - rect.min) / zoom;
            app.ui.graph_camera = crate::state::GraphCamera { pan: [pan.x, pan.y], zoom };
        }
    }
    let cam = Cam { origin: rect.min, pan: Vec2::from(app.ui.graph_camera.pan), zoom: app.ui.graph_camera.zoom.clamp(0.08, 4.0) };

    let thumbs_on = app.ui.prefs.thumbnails;
    let Ok(g) = app.session.graph() else { return };
    let g = g.clone();
    let selection = app.session.selection.clone();
    let node_delta = match &app.graph.drag {
        Some(Drag::Nodes { delta }) => *delta,
        _ => Vec2::ZERO,
    };
    let frame_move = match &app.graph.drag {
        Some(Drag::Frame { id, delta, resize: false, nodes }) => Some((*id, *delta, nodes.clone())),
        _ => None,
    };

    // Layout every node and its ports.
    struct Laid {
        id: NodeId,
        rect: Rect,
        ins: Vec<(String, String, PortKind, Pos2)>,
        outs: Vec<(String, String, PortKind, Pos2)>,
    }
    let mut laid: Vec<Laid> = Vec::with_capacity(g.nodes.len());
    for n in &g.nodes {
        let (ins, outs) = materialcraft_engine::eval::ports(&app.session.pkg, n);
        let mut r = node_rect(n, ins.len(), outs.len(), thumbs_on);
        if selection.contains(&n.id) {
            r = r.translate(node_delta);
        }
        if let Some((_, d, nodes)) = &frame_move
            && nodes.contains(&n.id)
        {
            r = r.translate(*d);
        }
        let dot = n.def == "dot";
        let pi = |i: usize, output: bool, total: usize| port_pos(r, i, total, output, dot);
        laid.push(Laid {
            id: n.id,
            rect: r,
            ins: ins.iter().enumerate().map(|(i, p)| (p.id.clone(), p.label.clone(), p.kind, pi(i, false, ins.len()))).collect(),
            outs: outs.iter().enumerate().map(|(i, p)| (p.id.clone(), p.label.clone(), p.kind, pi(i, true, outs.len()))).collect(),
        });
    }
    app.graph.rects = laid.iter().map(|l| (l.id, Rect::from_min_max(cam.to_screen(l.rect.min), cam.to_screen(l.rect.max)))).collect();
    app.graph.ports = laid
        .iter()
        .flat_map(|l| {
            let ins = l.ins.iter().map(move |(id, label, kind, p)| PortHit {
                node: l.id,
                output: false,
                port: id.clone(),
                label: label.clone(),
                pos: *p,
                kind: *kind,
            });
            let outs = l.outs.iter().map(move |(id, label, kind, p)| PortHit {
                node: l.id,
                output: true,
                port: id.clone(),
                label: label.clone(),
                pos: *p,
                kind: *kind,
            });
            ins.chain(outs)
        })
        .map(|mut h| {
            h.pos = cam.to_screen(h.pos);
            h
        })
        .collect();

    // Frames (behind everything).
    let mut frame_hits: Vec<(u64, Rect, Rect)> = Vec::new();
    for f in &g.frames {
        let mut r = Rect::from_min_size(pos2(f.rect[0], f.rect[1]), vec2(f.rect[2], f.rect[3]));
        match &app.graph.drag {
            Some(Drag::Frame { id, delta, resize: false, .. }) if *id == f.id => r = r.translate(*delta),
            Some(Drag::Frame { id, delta, resize: true, .. }) if *id == f.id => r.max += *delta,
            _ => {}
        }
        let sr = Rect::from_min_max(cam.to_screen(r.min), cam.to_screen(r.max));
        let c = f.color;
        let fill = Color32::from_rgba_unmultiplied((c[0] * 255.0) as u8, (c[1] * 255.0) as u8, (c[2] * 255.0) as u8, (c[3].clamp(0.05, 0.9) * 255.0) as u8);
        painter.rect_filled(sr, 4.0, fill);
        let title = Rect::from_min_size(sr.min, vec2(sr.width(), (20.0 * cam.zoom).max(12.0)));
        painter.rect_filled(title, CornerRadius { nw: 4, ne: 4, sw: 0, se: 0 }, fill.gamma_multiply(1.6));
        painter.text(
            title.left_center() + vec2(6.0, 0.0),
            Align2::LEFT_CENTER,
            &f.title,
            FontId::proportional((12.0 * cam.zoom).clamp(8.0, 18.0)),
            tk.text_strong,
        );
        let grip = Rect::from_min_size(sr.max - vec2(12.0, 12.0), vec2(12.0, 12.0));
        painter.line_segment([grip.left_bottom(), grip.right_top()], Stroke::new(1.0, tk.text_dim));
        frame_hits.push((f.id, title, grip));
    }

    // Wires.
    let cooked = &app.session.cooked;
    let detached = match &app.graph.drag {
        Some(Drag::Wire { detached: Some(d), .. }) => Some(d.clone()),
        _ => None,
    };
    let mut wires: Vec<(WireRef, [Pos2; 4])> = Vec::with_capacity(g.connections.len());
    for c in &g.connections {
        if detached.as_ref().is_some_and(|(n, p)| *n == c.to && *p == c.to_port) {
            continue;
        }
        let a = laid.iter().find(|l| l.id == c.from).and_then(|l| l.outs.iter().find(|p| p.0 == c.from_port));
        let b = laid.iter().find(|l| l.id == c.to).and_then(|l| l.ins.iter().find(|p| p.0 == c.to_port));
        if let (Some(a), Some(b)) = (a, b) {
            wires.push(((c.to, c.to_port.clone()), bezier_points(cam.to_screen(a.3), cam.to_screen(b.3))));
        }
    }
    // Hover: not where a node card or a port sits on top of the wire.
    let over_card = |p: Pos2| app.graph.rects.iter().any(|(_, r)| r.contains(p)) || nearest_port(&app.graph.ports, p, 9.0).is_some();
    let wire_hover = match &app.graph.drag {
        None => pointer.filter(|p| rect.contains(*p) && !over_card(*p)).and_then(|p| wire_at(&wires, p)).cloned(),
        Some(Drag::WirePress { wire, .. }) => Some(wire.clone()),
        Some(_) => None,
    };
    for (w, ctrl) in &wires {
        let Some(c) = g.input_of(w.0, &w.1) else { continue };
        let hot = selection.contains(&c.from) || selection.contains(&c.to);
        let picked = app.graph.selected_wires.contains(w);
        let hovered = wire_hover.as_ref() == Some(w);
        let base = (2.0 * cam.zoom).clamp(1.0, 3.0);
        let stroke = if picked {
            Stroke::new(base + 2.0, tk.accent)
        } else if hovered {
            Stroke::new(base + 1.0, Color32::WHITE)
        } else {
            Stroke::new(base, if hot { tk.accent } else { Color32::from_gray(0xb4) })
        };
        painter.add(egui::epaint::CubicBezierShape::from_points_stroke(*ctrl, false, Color32::TRANSPARENT, stroke));
    }
    app.graph.wires = wires;

    // Nodes.
    let font = FontId::proportional((10.5 * cam.zoom).clamp(6.0, 16.0));
    let small = FontId::proportional((9.0 * cam.zoom).clamp(5.5, 13.0));
    let mut node_resps: Vec<(NodeId, egui::Response)> = Vec::new();
    for l in &laid {
        let Ok(n) = g.node(l.id) else { continue };
        let sr = Rect::from_min_max(cam.to_screen(l.rect.min), cam.to_screen(l.rect.max));
        if !sr.intersects(rect) {
            continue;
        }
        let selected = selection.contains(&n.id);
        let result = cooked.nodes.get(&n.id);
        let error = result.and_then(|r| r.error.clone());
        let resp = ui.interact(sr, ui.scope_id().with(("node", n.id.0)), Sense::click_and_drag());
        crate::widgets::name(&resp, egui::Role::Button, &node_label(n));
        if n.def == "dot" {
            painter.circle(
                sr.center(),
                DOT_R * cam.zoom,
                Color32::from_gray(0x50),
                Stroke::new(1.5, if selected { tk.accent } else { Color32::from_gray(0xb4) }),
            );
        } else {
            let header = Rect::from_min_size(sr.min, vec2(sr.width(), HEADER_H * cam.zoom));
            let r = (5.0 * cam.zoom).clamp(1.0, 6.0) as u8;
            painter.rect_filled(sr, r, Color32::from_rgb(0x1e, 0x20, 0x24));
            painter.rect_filled(header, CornerRadius { nw: r, ne: r, sw: 0, se: 0 }, header_color(&n.def));
            let title = crate::widgets::elided(&painter, node_label(n), font.size, Color32::WHITE, header.width() - 8.0);
            painter.galley(header.left_center() + vec2(4.0, -title.size().y * 0.5), title, Color32::WHITE);
            // Thumbnail or compact body.
            let body = Rect::from_min_max(pos2(sr.left(), header.bottom()), pos2(sr.right(), sr.bottom() - FOOTER_H * cam.zoom));
            if let Some(r0) = result
                && let Some(img) = r0.outputs.first()
                && thumbs_on
            {
                let side = body.width().min(body.height()) - 6.0 * cam.zoom;
                let tr_ = Rect::from_center_size(body.center(), vec2(side, side));
                let tex = app.thumbs.node(&ctx, r0.key, 0, img);
                painter.image(tex, tr_, Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0)), if n.disabled { Color32::from_gray(90) } else { Color32::WHITE });
            } else if result.is_none() {
                painter.text(body.center(), Align2::CENTER_CENTER, icon::HOURGLASS_MEDIUM, small.clone(), tk.text_dim);
            }
            // Footer: size and format.
            let footer = Rect::from_min_max(pos2(sr.left(), body.bottom()), sr.max);
            if let Some(img) = result.and_then(|r| r.outputs.first()) {
                let bits = if n.base.format == materialcraft_engine::core::OutputFormat::Inherit { g.format.bits() } else { n.base.format.bits() };
                let format = format!("{}{}", if img.is_gray() { "L" } else { "C" }, bits);
                let txt = if let Some(limit) = app.session.preview_size {
                    let [pw, ph] = g.size_px();
                    let tooltip = format!(
                        "{} x {} ({format}bpc)\n{}: {pw} x {ph}\n{}: {limit} px\n{}",
                        img.w,
                        img.h,
                        tr("Parent size"),
                        tr("Preview limit"),
                        tr("Change Preview size while editing in Edit > Preferences. Exports use each node's configured output size.")
                    );
                    let tooltip = match &error {
                        Some(e) => format!("{tooltip}\n{e}"),
                        None => tooltip,
                    };
                    resp.clone().on_hover_text(tooltip);
                    format!("{}x{} {}", img.w, img.h, tr("Preview"))
                } else {
                    format!("{}x{} - {format}", img.w, img.h)
                };
                painter.text(footer.center(), Align2::CENTER_CENTER, txt, small.clone(), tk.text_dim);
            }
            if n.def == "output" {
                painter.text(header.right_center() - vec2(4.0, 0.0), Align2::RIGHT_CENTER, icon::SIGN_OUT, small.clone(), Color32::WHITE);
            }
            let outline = if error.is_some() {
                Stroke::new(2.0, Color32::from_rgb(0xff, 0x30, 0xd0))
            } else if selected {
                Stroke::new(2.0, tk.accent)
            } else if resp.hovered() {
                Stroke::new(1.0, Color32::from_gray(0x90))
            } else {
                Stroke::new(1.0, Color32::from_gray(0x10))
            };
            painter.rect_stroke(sr, r, outline, StrokeKind::Outside);
            if n.disabled {
                painter.line_segment([sr.left_top(), sr.right_bottom()], Stroke::new(1.5, tk.danger));
            }
            let has_preview_tooltip = app.session.preview_size.is_some() && result.and_then(|r| r.outputs.first()).is_some();
            if !has_preview_tooltip {
                if let Some(e) = &error {
                    resp.clone().on_hover_text(e);
                }
            }
        }
        node_resps.push((n.id, resp));
    }

    // Comments.
    let mut comment_hits: Vec<(u64, Rect)> = Vec::new();
    for c in &g.comments {
        let mut p = pos2(c.pos[0], c.pos[1]);
        if let Some(Drag::Comment { id, delta }) = &app.graph.drag
            && *id == c.id
        {
            p += *delta;
        }
        let r = Rect::from_min_size(cam.to_screen(p), vec2(c.size[0], c.size[1]) * cam.zoom);
        painter.rect_filled(r, 4.0, Color32::from_rgba_unmultiplied(0x5a, 0x54, 0x2e, 230));
        painter.rect_stroke(r, 4.0, Stroke::new(1.0, Color32::from_rgb(0x9a, 0x8c, 0x40)), StrokeKind::Inside);
        if app.graph.editing.as_ref().is_some_and(|(id, _)| *id == c.id) {
            comment_hits.push((c.id, r));
            continue;
        }
        let galley =
            painter.layout(c.text.clone(), FontId::proportional((11.0 * cam.zoom).clamp(6.0, 18.0)), Color32::from_rgb(0xf3, 0xec, 0xc8), r.width() - 10.0);
        painter.galley(r.min + vec2(5.0, 4.0), galley, Color32::WHITE);
        comment_hits.push((c.id, r));
    }

    // Pins.
    for pin in &g.pins {
        let p = cam.to_screen(pos2(pin.pos[0], pin.pos[1]));
        painter.text(p, Align2::CENTER_BOTTOM, icon::MAP_PIN, FontId::proportional(18.0), tk.accent);
        painter.text(p + vec2(10.0, -10.0), Align2::LEFT_CENTER, &pin.name, FontId::proportional(11.0), tk.text);
    }

    // Ports (drawn last, interactable over the node cards).
    let port_hover = pointer.and_then(|p| nearest_port(&app.graph.ports, p, 9.0)).cloned();
    for h in &app.graph.ports {
        if !rect.contains(h.pos) {
            continue;
        }
        let hot = port_hover.as_ref().is_some_and(|x| x.node == h.node && x.port == h.port && x.output == h.output);
        let r = (PORT_R * cam.zoom).clamp(2.5, 7.0) * if hot { 1.4 } else { 1.0 };
        painter.circle(h.pos, r, port_color(h.kind), Stroke::new(1.0, Color32::from_gray(0x18)));
    }
    if let Some(h) = &port_hover {
        let anchor = if h.output { Align2::LEFT_CENTER } else { Align2::RIGHT_CENTER };
        let off = if h.output { vec2(10.0, 0.0) } else { vec2(-10.0, 0.0) };
        let g_ = painter.layout_no_wrap(h.label.clone(), FontId::proportional(11.0), tk.text_strong);
        let bg = anchor.anchor_size(h.pos + off, g_.size()).expand(3.0);
        painter.rect_filled(bg, 3.0, Color32::from_black_alpha(200));
        painter.galley(bg.min + vec2(3.0, 3.0), g_, tk.text_strong);
    }

    // A wire under a comment is not hit.
    let wire_hover = wire_hover.filter(|_| !pointer.is_some_and(|p| comment_hits.iter().any(|(_, r)| r.contains(p))));
    if wire_hover.is_some() {
        ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    handle_input(
        app,
        ui,
        &resp,
        rect,
        &cam,
        &node_resps,
        &frame_hits,
        &comment_hits,
        port_hover,
        wire_hover,
        &laid.iter().map(|l| (l.id, l.rect)).collect::<Vec<_>>(),
    );

    // Drag previews.
    match &app.graph.drag {
        Some(Drag::Box { start }) => {
            if let Some(p) = pointer {
                let r = Rect::from_two_pos(*start, p);
                painter.rect(r, 0.0, tk.accent.gamma_multiply(0.12), Stroke::new(1.0, tk.accent), StrokeKind::Inside);
            }
        }
        Some(Drag::Wire { from, .. }) => {
            if let (Some(p), Some(src)) = (pointer, app.graph.ports.iter().find(|h| h.output && h.node == from.0 && h.port == from.1)) {
                bezier(&painter, src.pos, p, Stroke::new(2.0, tk.accent));
            }
        }
        Some(Drag::WireFromInput { to }) => {
            if let (Some(p), Some(dst)) = (pointer, app.graph.ports.iter().find(|h| !h.output && h.node == to.0 && h.port == to.1)) {
                bezier(&painter, p, dst.pos, Stroke::new(2.0, tk.accent));
            }
        }
        _ => {}
    }

    search_popup(app, ui, &cam);
    comment_editor(app, ui);
    let (failed, placeholders) = app.session.cook_failures();
    let mut at = rect.right_bottom() - vec2(8.0, 8.0);
    if !failed.is_empty() {
        let text = format!("{} {} node(s) failed", icon::WARNING, failed.len());
        at.y = painter.text(at, Align2::RIGHT_BOTTOM, text, FontId::proportional(11.0), tk.danger).top() - 2.0;
    }
    // Placeholders for missing SBS nodes are a soft warning, not a failure
    if placeholders > 0 {
        let text = format!("{} {placeholders} missing node(s), see the Console", icon::WARNING);
        painter.text(at, Align2::RIGHT_BOTTOM, text, FontId::proportional(11.0), tk.warning);
    }
}

fn grid(p: &egui::Painter, cam: &Cam, rect: Rect) {
    for (step, alpha) in [(32.0f32, 10u8), (256.0, 22)] {
        let s = step * cam.zoom;
        if s < 8.0 {
            continue;
        }
        let col = Color32::from_white_alpha(alpha);
        let x0 = cam.pan.x - (cam.pan.x.rem_euclid(step));
        let mut x = x0;
        while cam.to_screen(pos2(x, 0.0)).x < rect.right() {
            let sx = cam.to_screen(pos2(x, 0.0)).x;
            p.line_segment([pos2(sx, rect.top()), pos2(sx, rect.bottom())], Stroke::new(1.0, col));
            x += step;
        }
        let y0 = cam.pan.y - (cam.pan.y.rem_euclid(step));
        let mut y = y0;
        while cam.to_screen(pos2(0.0, y)).y < rect.bottom() {
            let sy = cam.to_screen(pos2(0.0, y)).y;
            p.line_segment([pos2(rect.left(), sy), pos2(rect.right(), sy)], Stroke::new(1.0, col));
            y += step;
        }
    }
}

fn nearest_port(ports: &[PortHit], p: Pos2, radius: f32) -> Option<&PortHit> {
    ports.iter().filter(|h| h.pos.distance(p) <= radius).min_by(|a, b| a.pos.distance(p).total_cmp(&b.pos.distance(p)))
}

#[allow(clippy::too_many_arguments)]
fn handle_input(
    app: &mut MaterialCraftApp,
    ui: &mut Ui,
    canvas: &egui::Response,
    rect: Rect,
    cam: &Cam,
    nodes: &[(NodeId, egui::Response)],
    frames: &[(u64, Rect, Rect)],
    comments: &[(u64, Rect)],
    port_hover: Option<PortHit>,
    wire_hover: Option<WireRef>,
    graph_rects: &[(NodeId, Rect)],
) {
    let ctx = ui.ctx().clone();
    let (pointer, mods, primary_pressed, primary_down, middle_down, released) = ctx.input(|i| {
        (i.pointer.interact_pos(), i.modifiers, i.pointer.primary_pressed(), i.pointer.primary_down(), i.pointer.middle_down(), i.pointer.any_released())
    });
    let zoom = cam.zoom;
    let motion = ctx.input(|i| i.pointer.delta()) / zoom;

    // Starting gestures (only on a press inside the canvas, with no gesture running).
    if app.graph.drag.is_none()
        && let Some(p) = pointer
        && rect.contains(p)
        && app.graph.search.is_none()
    {
        if middle_down || (primary_pressed && mods.alt) {
            app.graph.drag = Some(Drag::Pan);
        } else if primary_pressed {
            let on_card = nodes.iter().any(|(_, r)| r.rect.contains(p)) || comments.iter().any(|(_, r)| r.contains(p));
            let wire = wire_hover.as_ref().filter(|_| port_hover.is_none() && !on_card);
            // Any press other than on a wire drops the wire selection.
            if wire.is_none() {
                app.graph.selected_wires.clear();
            }
            if let Some(w) = wire {
                app.graph.drag = Some(Drag::WirePress { wire: w.clone(), start: p });
            } else if let Some(h) = &port_hover {
                if h.output {
                    app.graph.drag = Some(Drag::Wire { from: (h.node, h.port.clone()), detached: None });
                } else if let Some(c) = app.session.graph().ok().and_then(|g| g.input_of(h.node, &h.port).cloned()) {
                    app.graph.drag = Some(Drag::Wire { from: (c.from, c.from_port.clone()), detached: Some((h.node, h.port.clone())) });
                } else {
                    app.graph.drag = Some(Drag::WireFromInput { to: (h.node, h.port.clone()) });
                }
            } else if let Some((id, title, grip)) = frames.iter().find(|(_, t, g)| t.contains(p) || g.contains(p)) {
                let resize = grip.contains(p) && !title.contains(p);
                // Moving a frame carries the nodes inside it.
                let inside: Vec<NodeId> = app
                    .session
                    .graph()
                    .ok()
                    .and_then(|g| g.frames.iter().find(|f| f.id == *id))
                    .map(|f| {
                        let fr = Rect::from_min_size(pos2(f.rect[0], f.rect[1]), vec2(f.rect[2], f.rect[3]));
                        graph_rects.iter().filter(|(_, r)| fr.contains_rect(*r)).map(|(n, _)| *n).collect()
                    })
                    .unwrap_or_default();
                app.graph.drag = Some(Drag::Frame { id: *id, delta: Vec2::ZERO, resize, nodes: if resize { Vec::new() } else { inside } });
            } else if let Some((id, _)) = comments.iter().rev().find(|(_, r)| r.contains(p)) {
                app.graph.drag = Some(Drag::Comment { id: *id, delta: Vec2::ZERO });
            } else if let Some((id, _)) = nodes.iter().rev().find(|(_, r)| r.rect.contains(p)) {
                if !app.session.selection.contains(id) {
                    if mods.ctrl || mods.shift {
                        app.session.selection.push(*id);
                    } else {
                        app.session.selection = vec![*id];
                    }
                } else if mods.ctrl {
                    app.session.selection.retain(|s| s != id);
                }
                app.graph.drag = Some(Drag::Nodes { delta: Vec2::ZERO });
            } else {
                app.graph.drag = Some(Drag::Box { start: p });
            }
        }
    }

    // Continuing gestures.
    match &mut app.graph.drag {
        Some(Drag::Pan) => {
            let d = ctx.input(|i| i.pointer.delta()) / zoom;
            app.ui.graph_camera.pan = [app.ui.graph_camera.pan[0] - d.x, app.ui.graph_camera.pan[1] - d.y];
        }
        Some(Drag::Nodes { delta }) | Some(Drag::Frame { delta, .. }) | Some(Drag::Comment { delta, .. }) => *delta += motion,
        Some(Drag::WirePress { wire, start }) => {
            // Dragged far enough: a reroute dot where the press was, carried by the pointer.
            if let Some(p) = pointer
                && p.distance(*start) > 4.0
            {
                let (wire, start) = (wire.clone(), *start);
                let at = cam.to_graph(start);
                app.graph.drag = None;
                if app.run("graph.wire.insertDot", json!({"to": wire.0.0, "toPort": wire.1, "at": [at.x, at.y]})).is_some() {
                    app.graph.drag = Some(Drag::Nodes { delta: (p - start) / zoom });
                }
            }
        }
        _ => {}
    }

    // Ending gestures.
    let ended = released && !primary_down && !middle_down;
    if ended && let Some(drag) = app.graph.drag.take() {
        match drag {
            Drag::Pan => {}
            Drag::WirePress { wire, .. } => {
                app.session.selection.clear();
                let sel = &mut app.graph.selected_wires;
                if mods.ctrl && sel.contains(&wire) {
                    // Ctrl+click toggles a wire out of the selection.
                    sel.retain(|w| *w != wire);
                } else if mods.ctrl || mods.shift {
                    if !sel.contains(&wire) {
                        sel.push(wire);
                    }
                } else {
                    *sel = vec![wire];
                }
            }
            Drag::Nodes { delta } => {
                if delta.length() > 0.5 {
                    app.run("graph.node.move", json!({"by": [delta.x, delta.y]}));
                }
            }
            Drag::Box { start } => {
                if let Some(p) = pointer {
                    let r = Rect::from_two_pos(start, p);
                    if r.width() > 3.0 || r.height() > 3.0 {
                        let hit: Vec<NodeId> = app.graph.rects.iter().filter(|(_, nr)| r.intersects(*nr)).map(|(id, _)| *id).collect();
                        if mods.ctrl || mods.shift {
                            for id in hit {
                                if !app.session.selection.contains(&id) {
                                    app.session.selection.push(id);
                                }
                            }
                        } else {
                            app.session.selection = hit;
                        }
                    } else if !mods.ctrl && !mods.shift {
                        app.session.selection.clear();
                    }
                }
            }
            Drag::Wire { from, detached } => {
                let target = pointer.and_then(|p| nearest_port(&app.graph.ports, p, 12.0)).filter(|h| !h.output).cloned();
                match target {
                    Some(t) => rewire(app, &from, detached.as_ref(), t.node, &t.port),
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
            Drag::WireFromInput { to } => {
                let source = pointer.and_then(|p| nearest_port(&app.graph.ports, p, 12.0)).filter(|h| h.output).cloned();
                if let Some(s) = source {
                    app.run("graph.connect", json!({"from": s.node.0, "fromPort": s.port, "to": to.0.0, "toPort": to.1}));
                }
            }
            Drag::Frame { id, delta, resize, nodes } => {
                if delta.length() > 0.5
                    && let Some(f) = app.session.graph().ok().and_then(|g| g.frames.iter().find(|f| f.id == id)).cloned()
                {
                    let r = if resize {
                        [f.rect[0], f.rect[1], f.rect[2] + delta.x, f.rect[3] + delta.y]
                    } else {
                        [f.rect[0] + delta.x, f.rect[1] + delta.y, f.rect[2], f.rect[3]]
                    };
                    move_frame(app, id, r, &nodes, delta);
                }
            }
            Drag::Comment { id, delta } => {
                if delta.length() > 0.5
                    && let Some(c) = app.session.graph().ok().and_then(|g| g.comments.iter().find(|c| c.id == id)).cloned()
                {
                    app.run("graph.comment.edit", json!({"id": id, "at": [c.pos[0] + delta.x, c.pos[1] + delta.y]}));
                }
            }
        }
    }

    // Clicks.
    for (id, r) in nodes {
        if r.double_clicked() {
            let def = app.session.graph().ok().and_then(|g| g.node(*id).ok()).map(|n| (n.def.clone(), n.params.get("graph").cloned()));
            match def {
                Some((d, Some(Value::Text(name)))) if d == "graph_instance" => {
                    app.run("graph.open", json!({"graph": name}));
                }
                Some((d, _)) if d == "pixel_processor" || d == "value_processor" => {
                    app.run("view.2d", json!({"node": id.0, "port": 0}));
                    app.fn_editor = Some(crate::function_editor::FnEditor::new(*id, "function"));
                }
                _ => {
                    app.run("view.2d", json!({"node": id.0, "port": 0}));
                }
            }
        } else if r.clicked() && app.session.selection.contains(id) && app.session.view2d.node != Some(*id) {
            // A click that leaves the node selected shows it, replacing a node shown earlier (a
            // Ctrl+click that deselects it does not; the shown node keeps its chosen port).
            app.run("view.2d", json!({"node": id.0}));
        }
        r.context_menu(|ui| node_menu(app, ui, *id));
    }
    if let Some(h) = &port_hover
        && h.output
        && ctx.input(|i| i.pointer.primary_clicked())
    {
        let idx = app.session.output_ports(h.node).iter().position(|p| *p == h.port).unwrap_or(0);
        app.run("view.2d", json!({"node": h.node.0, "port": idx}));
    }
    for (id, r) in comments {
        if canvas.double_clicked() && pointer.is_some_and(|p| r.contains(p)) {
            let text = app.session.graph().ok().and_then(|g| g.comments.iter().find(|c| c.id == *id)).map(|c| c.text.clone()).unwrap_or_default();
            app.graph.editing = Some((*id, text));
        }
    }
    let on_node = pointer.is_some_and(|p| nodes.iter().any(|(_, r)| r.rect.contains(p)) || comments.iter().any(|(_, r)| r.contains(p)));
    if canvas.double_clicked()
        && !on_node
        && let Some(p) = pointer
    {
        match &wire_hover {
            // Double-click a wire: a reroute dot there.
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
        app.graph.menu_wire = wire_hover.clone().filter(|_| !on_node);
        // Right-clicking a selected wire keeps the selection (to swap or delete them all).
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
        None => canvas_menu(app, ui, at),
    });

    // Library drops.
    if let Some(payload) = canvas.dnd_release_payload::<LibraryDrag>()
        && let Some(p) = pointer
    {
        let gp = cam.to_graph(p);
        let at = [gp.x - NODE_W * 0.5, gp.y - 20.0];
        match payload.0.strip_prefix('@') {
            Some(graph) => app.run("graph.node.add", json!({"def": "graph_instance", "at": at, "params": {"graph": graph}})),
            None => app.run("graph.node.add", json!({"def": payload.0, "at": at})),
        };
    }

    // Keys over the canvas.
    let hovered = pointer.is_some_and(|p| rect.contains(p));
    if hovered && app.graph.search.is_none() {
        let space = !ctx.egui_wants_keyboard_input() && ctx.input(|i| i.key_pressed(egui::Key::Space));
        let tab = !app.graph.keyboard_was_focused && !ctx.text_edit_focused() && ctx.input(|i| i.key_pressed(egui::Key::Tab));
        if (space || tab)
            && let Some(p) = pointer
        {
            open_search(app, cam.to_graph(p), None);
        }
        if !ctx.egui_wants_keyboard_input() && ctx.input(|i| i.key_pressed(egui::Key::F) && i.modifiers.is_none()) {
            let sel = !app.session.selection.is_empty();
            frame(app, rect, sel);
        }
    }
}

fn open_search(app: &mut MaterialCraftApp, at: Pos2, connect: Option<(NodeId, String)>) {
    app.graph.search = Some(Search { at, text: String::new(), sel: 0, connect, focus: true });
}

/// Catalog entries matching `text` (label or id contains every word).
fn matches(text: &str) -> Vec<&'static materialcraft_engine::nodes::NodeDef> {
    let words: Vec<String> = text.split_whitespace().map(str::to_ascii_lowercase).collect();
    let mut v: Vec<_> = materialcraft_engine::nodes::catalog()
        .iter()
        .filter(|d| d.id != "graph_instance")
        .filter(|d| {
            let hay = format!("{} {} {}", d.label.to_ascii_lowercase(), d.id, d.category.to_ascii_lowercase());
            words.iter().all(|w| hay.contains(w.as_str()))
        })
        .collect();
    if let Some(w) = words.first() {
        v.sort_by_key(|d| !d.label.to_ascii_lowercase().starts_with(w.as_str()));
    }
    v
}

fn search_popup(app: &mut MaterialCraftApp, ui: &mut Ui, cam: &Cam) {
    let Some(s) = app.graph.search.as_mut() else { return };
    let tk = Tokens::get(ui.ctx());
    let screen = cam.to_screen(s.at);
    let mut close = false;
    let mut pick: Option<&'static str> = None;
    egui::Area::new(egui::Id::unique("node-search")).fixed_pos(screen).order(egui::Order::Foreground).show(ui.ctx(), |ui| {
        egui::Frame::popup(ui.style()).show(ui, |ui| {
            ui.set_width(260.0);
            let mut reveal_selection = s.focus;
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(icon::MAGNIFYING_GLASS).color(tk.text_dim));
                let r = ui.add(egui::TextEdit::singleline(&mut s.text).hint_text(tr("Search nodes...")).desired_width(220.0));
                crate::widgets::name(&r, egui::Role::SearchInput, "Search nodes");
                if s.focus {
                    r.request_focus();
                    s.focus = false;
                }
                if r.changed() {
                    s.sel = 0;
                    reveal_selection = true;
                }
            });
            let list = matches(&s.text);
            let (up, down, enter, esc) = ui.input(|i| {
                (i.key_pressed(egui::Key::ArrowUp), i.key_pressed(egui::Key::ArrowDown), i.key_pressed(egui::Key::Enter), i.key_pressed(egui::Key::Escape))
            });
            if down {
                s.sel = (s.sel + 1).min(list.len().saturating_sub(1));
            }
            if up {
                s.sel = s.sel.saturating_sub(1);
            }
            if esc {
                close = true;
            }
            if enter && let Some(d) = list.get(s.sel) {
                pick = Some(d.id);
            }
            egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                for (i, d) in list.iter().enumerate() {
                    let r = ui.add(
                        egui::Button::selectable(i == s.sel, format!("{}   ", d.label))
                            .min_size(vec2(ui.available_width(), 0.0))
                            .right_text(egui::RichText::new(d.group()).size(10.0).color(tk.text_dim)),
                    );
                    if i == s.sel && (up || down || reveal_selection) {
                        r.scroll_to_me(None);
                    }
                    if r.clicked() {
                        pick = Some(d.id);
                    }
                }
                if list.is_empty() {
                    ui.label(egui::RichText::new(tr("No matching node")).color(tk.text_dim));
                }
            });
        });
    });
    let clicked_out = ui.input(|i| i.pointer.any_pressed())
        && ui.ctx().pointer_interact_pos().is_some_and(|p| !ui.ctx().layer_id_at(p).is_some_and(|l| l.id == egui::Id::unique("node-search")));
    if let Some(def) = pick {
        let s = app.graph.search.take();
        if let Some(s) = s {
            let mut p = json!({"def": def, "at": [s.at.x, s.at.y - 20.0]});
            if let Some((n, port)) = s.connect {
                p["connectFrom"] = json!({"node": n.0, "port": port});
            }
            app.run("graph.node.add", p);
        }
    } else if close || clicked_out {
        app.graph.search = None;
    }
}

fn comment_editor(app: &mut MaterialCraftApp, ui: &mut Ui) {
    let Some((id, text)) = app.graph.editing.as_mut() else { return };
    let Some(r) = app.session.graph().ok().and_then(|g| g.comments.iter().find(|c| c.id == *id)).map(|c| (c.pos, c.size)) else {
        app.graph.editing = None;
        return;
    };
    let cam = &app.ui.graph_camera;
    let origin = app.graph.view.min;
    let at = origin + (Vec2::from(r.0) - Vec2::from(cam.pan)) * cam.zoom;
    let mut done = false;
    egui::Area::new(egui::Id::unique("comment-edit")).fixed_pos(at).order(egui::Order::Foreground).show(ui.ctx(), |ui| {
        let resp = ui.add(egui::TextEdit::multiline(text).desired_width(r.1[0] * cam.zoom).desired_rows(3));
        resp.request_focus();
        done = resp.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Escape));
    });
    if done && let Some((id, text)) = app.graph.editing.take() {
        app.run("graph.comment.edit", json!({"id": id, "text": text}));
    }
}

fn node_menu(app: &mut MaterialCraftApp, ui: &mut Ui, id: NodeId) {
    if !app.session.selection.contains(&id) {
        app.session.selection = vec![id];
    }
    let mut item = |ui: &mut Ui, label: &str, cmd: &str, params: serde_json::Value| {
        if ui.button(tr(label)).clicked() {
            app.run(cmd, params);
            ui.close();
        }
    };
    item(ui, "View in 2D view", "view.2d", json!({"node": id.0}));
    ui.separator();
    item(ui, "Copy selection", "edit.copy", json!({}));
    item(ui, "Duplicate selection", "edit.duplicate", json!({}));
    item(ui, "Duplicate selection without link", "edit.duplicate", json!({"withoutLinks": true}));
    item(ui, "Delete selection", "edit.delete", json!({}));
    item(ui, "Delete and relink", "edit.deleteRelink", json!({}));
    ui.separator();
    item(ui, "Select upstream nodes", "graph.selectUpstream", json!({}));
    item(ui, "Select downstream nodes", "graph.selectDownstream", json!({}));
    item(ui, "Disable selection", "graph.node.disable", json!({}));
    item(ui, "Reset parameters", "graph.node.reset", json!({"id": id.0}));
    ui.separator();
    item(ui, "Add frame around selection", "graph.frame.add", json!({}));
    item(ui, "Create graph from selection", "graph.createFromSelection", json!({}));
}

/// `wires` as the `{wires: [{to, toPort}, ...]}` parameter of the wire commands.
fn wires_json(wires: &[WireRef]) -> serde_json::Value {
    json!({"wires": wires.iter().map(|(to, port)| json!({"to": to.0, "toPort": port})).collect::<Vec<_>>()})
}

/// Delete the selected wires in one undo step (the Delete and Backspace keys); `false` when none
/// is selected.
pub fn delete_selected_wires(app: &mut MaterialCraftApp) -> bool {
    let wires = std::mem::take(&mut app.graph.selected_wires);
    if wires.is_empty() {
        return false;
    }
    app.run("graph.wire.delete", wires_json(&wires));
    true
}

/// Swap where the selected wires come from (the X key): two wires trade sources, more reverse
/// them in input order. The wires stay selected, so X again swaps back. `false` when no wire is
/// selected.
pub fn swap_selected_wires(app: &mut MaterialCraftApp) -> bool {
    if app.graph.selected_wires.is_empty() {
        return false;
    }
    let params = wires_json(&app.graph.selected_wires);
    app.run("graph.wire.swap", params);
    true
}

fn wire_menu(app: &mut MaterialCraftApp, ui: &mut Ui, (to, port): WireRef, at: Pos2) {
    let n = app.graph.selected_wires.len();
    if ui.button(format!("{}  {}", icon::DOT_OUTLINE, tr("Insert reroute dot"))).clicked() {
        app.run("graph.wire.insertDot", json!({"to": to.0, "toPort": port, "at": [at.x, at.y]}));
        app.graph.selected_wires.clear();
        ui.close();
    }
    if n >= 2 && ui.button(format!("{}  {}  (X)", icon::ARROWS_LEFT_RIGHT, tr("Swap selected links"))).clicked() {
        swap_selected_wires(app);
        ui.close();
    }
    let label = if n >= 2 { tr("Delete selected links") } else { tr("Delete link") };
    if ui.button(format!("{}  {label}", icon::TRASH)).clicked() {
        if n < 2 {
            app.graph.selected_wires = vec![(to, port)];
        }
        delete_selected_wires(app);
        ui.close();
    }
}

fn canvas_menu(app: &mut MaterialCraftApp, ui: &mut Ui, at: Pos2) {
    ui.menu_button(tr("Add node"), |ui| {
        for (cat, ids) in materialcraft_engine::nodes::categories() {
            ui.menu_button(cat, |ui| {
                egui::ScrollArea::vertical().max_height(420.0).show(ui, |ui| {
                    for id in ids {
                        let label = materialcraft_engine::nodes::find(id).map_or(id, |d| d.label);
                        if ui.button(label).clicked() {
                            app.run("graph.node.add", json!({"def": id, "at": [at.x, at.y]}));
                            ui.close();
                        }
                    }
                });
            });
        }
        let others: Vec<String> = app.session.pkg.graphs.iter().enumerate().filter(|(i, _)| *i != app.session.graph).map(|(_, g)| g.name.clone()).collect();
        if !others.is_empty() {
            ui.menu_button(tr("Graphs in this package"), |ui| {
                for name in others {
                    if ui.button(&name).clicked() {
                        app.run("graph.node.add", json!({"def": "graph_instance", "at": [at.x, at.y], "params": {"graph": name}}));
                        ui.close();
                    }
                }
            });
        }
    });
    let mut item = |ui: &mut Ui, label: &str, cmd: &str, params: serde_json::Value| {
        if ui.button(tr(label)).clicked() {
            app.run(cmd, params);
            ui.close();
        }
    };
    item(ui, "Add Comment", "graph.comment.add", json!({"at": [at.x, at.y]}));
    item(ui, "Add Frame", "graph.frame.add", json!({"rect": [at.x, at.y, 400.0, 260.0]}));
    item(ui, "Add Navigation pin", "graph.pin.add", json!({"at": [at.x, at.y]}));
    item(ui, "Add Dot node", "graph.node.add", json!({"def": "dot", "at": [at.x, at.y]}));
    ui.separator();
    item(ui, "Compute graph", "graph.cook", json!({}));
    item(ui, "Paste", "edit.paste", json!({"at": [at.x, at.y]}));
    item(ui, "Select all", "edit.selectAll", json!({}));
    ui.separator();
    let pins: Vec<(String, [f32; 2])> = app.session.graph().map(|g| g.pins.iter().map(|p| (p.name.clone(), p.pos)).collect()).unwrap_or_default();
    if !pins.is_empty() {
        ui.menu_button(tr("Go to pin"), |ui| {
            for (name, pos) in pins {
                if ui.button(&name).clicked() {
                    let v = app.graph.view;
                    let z = app.ui.graph_camera.zoom;
                    app.ui.graph_camera.pan = [pos[0] - v.width() * 0.5 / z, pos[1] - v.height() * 0.5 / z];
                    ui.close();
                }
            }
        });
    }
    if ui.button(tr("Frame all")).clicked() {
        app.ui.frame_all = true;
        ui.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_failed_rewire_keeps_the_original_connection_and_history() {
        let mut app = empty_app();
        let source = app.run("graph.node.add", json!({"def": "uniform"})).unwrap()["id"].as_u64().unwrap();
        let target = app.run("graph.node.add", json!({"def": "levels"})).unwrap()["id"].as_u64().unwrap();
        app.run("graph.connect", json!({"from": source, "to": target, "toPort": "input"})).unwrap();
        let before = app.session.graph().unwrap().connections.clone();
        let history = app.session.history();
        rewire(&mut app, &(NodeId(source), "output".into()), Some(&(NodeId(target), "input".into())), NodeId(target), "missing");
        assert_eq!(app.session.graph().unwrap().connections, before);
        assert_eq!(app.session.history(), history);
        assert!(app.toasts.iter().any(|t| t.error));
    }

    fn empty_app() -> MaterialCraftApp {
        let mut app = MaterialCraftApp::headless(materialcraft_engine::sample_session(), crate::UiState::default());
        app.run("file.new", json!({"template": "empty"}));
        app
    }

    fn add(app: &mut MaterialCraftApp, def: &str) -> NodeId {
        NodeId(app.run("graph.node.add", json!({"def": def})).and_then(|v| v["id"].as_u64()).unwrap())
    }

    #[test]
    fn rewiring_a_dragged_link_is_one_undo_entry() {
        let mut app = empty_app();
        let (a, b, c) = (add(&mut app, "clouds_1"), add(&mut app, "levels"), add(&mut app, "levels"));
        app.run("graph.connect", json!({"from": a.0, "fromPort": "output", "to": b.0, "toPort": "input"}));
        let base = app.session.history().len();
        rewire(&mut app, &(a, "output".to_string()), Some(&(b, "input".to_string())), c, "input");
        assert_eq!(app.session.history().len(), base + 1, "{:?}", app.session.history());
        let conns = &app.session.graph().unwrap().connections;
        assert_eq!(conns.len(), 1);
        assert_eq!(conns[0].to, c);
        app.session.undo();
        assert_eq!(app.session.graph().unwrap().connections[0].to, b, "one undo restores the old link");
    }

    #[test]
    fn dragging_a_frame_with_its_nodes_is_one_undo_entry() {
        let mut app = empty_app();
        let n = add(&mut app, "levels");
        let id = app.run("graph.frame.add", json!({"rect": [0, 0, 300, 200]})).and_then(|v| v["id"].as_u64()).unwrap();
        let base = app.session.history().len();
        move_frame(&mut app, id, [10.0, 10.0, 300.0, 200.0], &[n], vec2(10.0, 10.0));
        assert_eq!(app.session.history().len(), base + 1, "{:?}", app.session.history());
        app.session.undo();
        assert_eq!(app.session.graph().unwrap().frames[0].rect, [0.0, 0.0, 300.0, 200.0]);
    }

    /// The whole app in an offscreen egui context, driven one frame at a time.
    struct Frames {
        ctx: egui::Context,
        app: MaterialCraftApp,
        time: f64,
    }

    impl Frames {
        /// `session` after a few frames, so the canvas is laid out.
        fn new(session: materialcraft_engine::Session) -> Frames {
            let mut f = Frames { ctx: egui::Context::default(), app: MaterialCraftApp::headless(session, crate::UiState::default()), time: 0.0 };
            for _ in 0..3 {
                f.run(Vec::new(), egui::Modifiers::NONE);
            }
            f
        }

        /// One frame with `events` while `modifiers` are held, in a desktop-sized window (egui's
        /// default screen is so large that the 3D view's frame would exceed the texture limit).
        fn run(&mut self, events: Vec<egui::Event>, modifiers: egui::Modifiers) {
            self.time += 0.05;
            let events = std::iter::once(egui::Event::ModifiersChanged(modifiers)).chain(events).collect();
            let screen_rect = Some(Rect::from_min_size(Pos2::ZERO, vec2(1600.0, 960.0)));
            let input = egui::RawInput { events, screen_rect, time: Some(self.time), ..Default::default() };
            let app = &mut self.app;
            self.ctx
                .run_ui(input, |ui| {
                    app.step(&ui.ctx().clone());
                    app.draw(ui);
                })
                .textures_delta
                .clear();
        }

        /// Press and release the primary button at `pos`, a second after any earlier click so
        /// the two never pair into a double-click.
        fn click(&mut self, pos: Pos2, modifiers: egui::Modifiers) {
            self.time += 1.0;
            self.run(vec![egui::Event::PointerMoved(pos)], modifiers);
            for pressed in [true, false] {
                self.run(vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers }], modifiers);
            }
        }

        /// The center of node `id` on screen last frame.
        fn center(&self, id: NodeId) -> Pos2 {
            self.app.graph.rects.iter().find(|(n, _)| *n == id).map(|(_, r)| r.center()).unwrap_or_else(|| panic!("node {id:?} is not on screen"))
        }

        /// The node the 2D view shows once the graph has cooked.
        fn shown(&mut self) -> Option<NodeId> {
            self.app.session.cook();
            self.app.session.view2d_image().map(|(id, _)| id)
        }
    }

    /// A graph with a Perlin noise, a checker and an RGBA split side by side, framed on screen.
    fn three_nodes() -> (Frames, [NodeId; 3]) {
        let mut s = materialcraft_engine::sample_session();
        materialcraft_engine::execute(&mut s, "file.new", &json!({"template": "empty", "size": 256})).unwrap();
        let ids = ["perlin_noise", "checker", "rgba_split"].iter().enumerate().map(|(i, def)| {
            let added = materialcraft_engine::execute(&mut s, "graph.node.add", &json!({"def": def, "at": [i as f32 * 300.0, 0.0]})).unwrap();
            NodeId(added["id"].as_u64().unwrap())
        });
        let ids: [NodeId; 3] = ids.collect::<Vec<_>>().try_into().unwrap();
        s.selection.clear();
        (Frames::new(s), ids)
    }

    #[test]
    fn clicking_a_node_shows_it_in_the_2d_view() {
        let (mut f, [noise, checker, _]) = three_nodes();
        // Shown explicitly first (a double-click, an output port or "View in 2D view").
        f.app.run("view.2d", json!({"node": noise.0}));
        assert_eq!(f.shown(), Some(noise));
        let at = f.center(checker);
        f.click(at, egui::Modifiers::NONE);
        assert_eq!(f.app.session.selection, vec![checker]);
        assert_eq!(f.app.session.view2d.node, Some(checker));
        assert_eq!(f.shown(), Some(checker));
        // And back.
        let at = f.center(noise);
        f.click(at, egui::Modifiers::NONE);
        assert_eq!(f.app.session.view2d.node, Some(noise));
        assert_eq!(f.shown(), Some(noise));
        // Ctrl+click adds a node to the selection and shows it too.
        let at = f.center(checker);
        f.click(at, egui::Modifiers::CTRL);
        assert_eq!(f.app.session.selection, vec![noise, checker]);
        assert_eq!(f.shown(), Some(checker));
    }

    #[test]
    fn ctrl_click_that_deselects_a_node_does_not_show_it() {
        let (mut f, [noise, checker, _]) = three_nodes();
        f.app.session.selection = vec![noise, checker];
        f.app.run("view.2d", json!({"node": checker.0}));
        let at = f.center(noise);
        f.click(at, egui::Modifiers::CTRL);
        assert_eq!(f.app.session.selection, vec![checker]);
        assert_eq!(f.shown(), Some(checker));
    }

    #[test]
    fn dragging_a_node_keeps_the_2d_view() {
        let (mut f, [noise, checker, _]) = three_nodes();
        f.app.run("view.2d", json!({"node": noise.0}));
        let before = f.app.session.graph().unwrap().node(checker).unwrap().pos;
        let from = f.center(checker);
        let none = egui::Modifiers::NONE;
        f.run(vec![egui::Event::PointerMoved(from)], none);
        f.run(vec![egui::Event::PointerButton { pos: from, button: egui::PointerButton::Primary, pressed: true, modifiers: none }], none);
        for step in 1..=8 {
            f.run(vec![egui::Event::PointerMoved(from + vec2(5.0 * step as f32, 0.0))], none);
        }
        let to = from + vec2(40.0, 0.0);
        f.run(vec![egui::Event::PointerButton { pos: to, button: egui::PointerButton::Primary, pressed: false, modifiers: none }], none);
        let after = f.app.session.graph().unwrap().node(checker).unwrap().pos;
        let moved = 40.0 / f.app.ui.graph_camera.zoom;
        assert!((after[0] - before[0] - moved).abs() < 1e-3 && after[1] == before[1], "the checker moved {before:?} -> {after:?}, not {moved} right");
        assert_eq!(f.app.session.selection, vec![checker]);
        assert_eq!(f.shown(), Some(noise));
    }

    #[test]
    fn clicking_the_shown_node_keeps_its_port() {
        let (mut f, [_, _, split]) = three_nodes();
        f.app.run("view.2d", json!({"node": split.0, "port": 2}));
        let at = f.center(split);
        f.click(at, egui::Modifiers::NONE);
        assert_eq!(f.app.session.selection, vec![split]);
        assert_eq!((f.app.session.view2d.node, f.app.session.view2d.port), (Some(split), 2));
    }

    #[test]
    fn tab_opens_search_in_the_full_workspace() {
        let mut f = Frames::new(materialcraft_engine::sample_session());
        let pointer = f.app.graph.view.center();
        let tab = egui::Event::Key { key: egui::Key::Tab, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE };
        f.run(vec![egui::Event::PointerMoved(pointer), tab], egui::Modifiers::NONE);
        assert!(f.app.graph.search.is_some());
    }

    #[test]
    fn clicked_wires_swap_with_x_and_delete_together() {
        let mut app = MaterialCraftApp::headless(materialcraft_engine::sample_session(), crate::UiState::default());
        app.run("file.new", json!({"template": "empty"}));
        let id = |v: Option<serde_json::Value>| v.and_then(|v| v["id"].as_u64()).unwrap();
        let a = id(app.run("graph.node.add", json!({"def": "clouds_1", "at": [0, -150]})));
        let b = id(app.run("graph.node.add", json!({"def": "checker", "at": [0, 150]})));
        let blend = id(app.run("graph.node.add", json!({"def": "blend", "at": [450, 0]})));
        app.run("graph.connect", json!({"from": a, "to": blend, "toPort": "foreground"}));
        app.run("graph.connect", json!({"from": b, "to": blend, "toPort": "background"}));
        let ctx = egui::Context::default();
        let mut frame = |app: &mut MaterialCraftApp, mut events: Vec<egui::Event>, modifiers: egui::Modifiers| {
            events.insert(0, egui::Event::ModifiersChanged(modifiers));
            let input = egui::RawInput { events, ..Default::default() };
            ctx.run_ui(input, |ui| {
                app.step(&ui.ctx().clone());
                app.draw(ui);
            })
            .textures_delta
            .clear();
        };
        for _ in 0..3 {
            frame(&mut app, Vec::new(), egui::Modifiers::NONE);
        }
        let mid =
            |app: &MaterialCraftApp, port: &str| app.graph.wires.iter().find(|(w, _)| w.0.0 == blend && w.1 == port).map(|(_, c)| bezier_at(c, 0.5)).unwrap();
        let click =
            |app: &mut MaterialCraftApp, frame: &mut dyn FnMut(&mut MaterialCraftApp, Vec<egui::Event>, egui::Modifiers), port: &str, m: egui::Modifiers| {
                let p = mid(app, port);
                let button = |pressed| egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed, modifiers: m };
                frame(app, vec![egui::Event::PointerMoved(p)], m);
                frame(app, vec![button(true)], m);
                frame(app, vec![button(false)], m);
            };
        click(&mut app, &mut frame, "foreground", egui::Modifiers::NONE);
        click(&mut app, &mut frame, "background", egui::Modifiers::SHIFT);
        assert_eq!(app.graph.selected_wires.len(), 2, "{:?}", app.graph.selected_wires);
        let key = |k| egui::Event::Key { key: k, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE };
        let src = |app: &MaterialCraftApp, port: &str| app.session.graph().unwrap().input_of(NodeId(blend), port).unwrap().from.0;
        frame(&mut app, vec![key(egui::Key::X)], egui::Modifiers::NONE);
        assert_eq!((src(&app, "foreground"), src(&app, "background")), (b, a));
        // Still selected: X again swaps back.
        frame(&mut app, vec![key(egui::Key::X)], egui::Modifiers::NONE);
        assert_eq!((src(&app, "foreground"), src(&app, "background")), (a, b));
        frame(&mut app, vec![key(egui::Key::Delete)], egui::Modifiers::NONE);
        assert!(app.session.graph().unwrap().connections.is_empty());
        assert!(app.graph.selected_wires.is_empty());
    }

    #[test]
    fn wires_connect_when_dragged_from_either_end() {
        let mut app = MaterialCraftApp::headless(materialcraft_engine::sample_session(), crate::UiState::default());
        app.run("file.new", json!({"template": "empty"}));
        let id = |v: Option<serde_json::Value>| v.and_then(|v| v["id"].as_u64()).unwrap();
        let noise = id(app.run("graph.node.add", json!({"def": "perlin_noise", "at": [0, 0]})));
        let blend = id(app.run("graph.node.add", json!({"def": "blend", "at": [450, 0]})));
        let ctx = egui::Context::default();
        let frame = |app: &mut MaterialCraftApp, events: Vec<egui::Event>| {
            ctx.run_ui(egui::RawInput { events, screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1600.0, 960.0))), ..Default::default() }, |ui| {
                app.step(&ui.ctx().clone());
                app.draw(ui);
            })
            .textures_delta
            .clear();
        };
        for _ in 0..3 {
            frame(&mut app, Vec::new());
        }
        let port = |app: &MaterialCraftApp, node: u64, output: bool, name: &str| {
            app.graph.ports.iter().find(|h| h.node.0 == node && h.output == output && h.port == name).map(|h| h.pos).unwrap()
        };
        let output = port(&app, noise, true, "output");
        let foreground = port(&app, blend, false, "foreground");
        let background = port(&app, blend, false, "background");
        let empty = app.graph.view.left_top() + vec2(5.0, 5.0);
        let drag = |app: &mut MaterialCraftApp, from: Pos2, to: Pos2| {
            let button = |pos, pressed| egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE };
            frame(app, vec![egui::Event::PointerMoved(from)]);
            frame(app, vec![button(from, true)]);
            frame(app, vec![egui::Event::PointerMoved(to)]);
            frame(app, vec![button(to, false)]);
        };
        let source = |app: &MaterialCraftApp| app.session.graph().unwrap().input_of(NodeId(blend), "foreground").map(|c| (c.from.0, c.from_port.clone()));

        // From the input: only an output finishes the wire, and a miss changes nothing.
        drag(&mut app, foreground, empty);
        assert_eq!(source(&app), None);
        assert!(app.graph.search.is_none());
        drag(&mut app, foreground, background);
        assert_eq!(source(&app), None);
        drag(&mut app, foreground, output);
        assert_eq!(source(&app), Some((noise, "output".to_string())));

        // From the output, as before.
        app.run("graph.disconnect", json!({"to": blend, "toPort": "foreground"}));
        drag(&mut app, output, foreground);
        assert_eq!(source(&app), Some((noise, "output".to_string())));
    }

    #[test]
    fn node_search_shortcuts_respect_hover_and_keyboard_focus() {
        for key in [egui::Key::Space, egui::Key::Tab] {
            for (hovered, editing) in [(true, false), (false, false), (true, true)] {
                let ctx = egui::Context::default();
                let mut app = MaterialCraftApp::headless(materialcraft_engine::sample_session(), crate::UiState::default());
                let mut text = String::new();
                let mut draw = |ui: &mut Ui| {
                    if editing {
                        ui.text_edit_singleline(&mut text).request_focus();
                    }
                    app.graph.keyboard_was_focused = ui.ctx().egui_wants_keyboard_input();
                    show(&mut app, ui);
                };
                ctx.run_ui(egui::RawInput::default(), &mut draw).textures_delta.clear();
                let pointer = if hovered { pos2(200.0, 200.0) } else { pos2(-10.0, -10.0) };
                let input = egui::RawInput {
                    events: vec![
                        egui::Event::PointerMoved(pointer),
                        egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE },
                    ],
                    ..Default::default()
                };
                ctx.run_ui(input, &mut draw).textures_delta.clear();
                assert_eq!(app.graph.search.is_some(), hovered && !editing, "{key:?}: hovered={hovered}, editing={editing}");
                if let Some(search) = &app.graph.search {
                    let cam = Cam { origin: app.graph.view.min, pan: Vec2::from(app.ui.graph_camera.pan), zoom: app.ui.graph_camera.zoom };
                    assert_eq!(search.at, cam.to_graph(pointer));
                    assert!(search.text.is_empty());
                }
            }
        }
    }

    #[test]
    fn search_finds_by_label_and_category() {
        assert!(matches("perlin").iter().any(|d| d.id == "perlin_noise"));
        assert!(matches("blur").first().is_some_and(|d| d.label.to_ascii_lowercase().starts_with("blur")));
        assert!(matches("noises cells").iter().all(|d| d.category.contains("Noises")));
        assert!(matches("zzzz").is_empty());
    }

    #[test]
    fn wires_are_hit_near_their_curve_only() {
        let a = (NodeId(1), "input".to_string());
        let b = (NodeId(2), "input".to_string());
        let wires = vec![(a.clone(), bezier_points(pos2(0.0, 0.0), pos2(200.0, 100.0))), (b.clone(), bezier_points(pos2(0.0, 300.0), pos2(200.0, 300.0)))];
        // The midpoint of a symmetric wire lies on the line between its ends.
        let mid = bezier_at(&wires[0].1, 0.5);
        assert!((mid - pos2(100.0, 50.0)).length() < 1e-3, "{mid:?}");
        assert_eq!(wire_at(&wires, mid + vec2(0.0, 3.0)), Some(&a));
        assert_eq!(wire_at(&wires, pos2(100.0, 302.0)), Some(&b));
        assert_eq!(wire_at(&wires, pos2(100.0, 200.0)), None);
        assert_eq!(wire_at(&wires, mid + vec2(0.0, WIRE_HIT * 3.0)), None);
        // The ends are on the wire.
        assert!(bezier_distance(&wires[0].1, pos2(0.0, 0.0)) < 1e-3);
        assert!(bezier_distance(&wires[0].1, pos2(200.0, 100.0)) < 1e-3);
    }

    #[test]
    fn port_positions_stay_on_the_card() {
        let n = Node::new(NodeId(1), "blend", [0.0, 0.0]);
        let r = node_rect(&n, 3, 1, true);
        for i in 0..3 {
            let p = port_pos(r, i, 3, false, false);
            assert!(r.contains(p) || (p.x - r.left()).abs() < 1e-3);
            assert!(p.y > r.top() + HEADER_H && p.y < r.bottom() - FOOTER_H);
        }
    }
    fn search_harness(render: bool) -> egui_kittest::Harness<'static, MaterialCraftApp> {
        let app = MaterialCraftApp::headless(materialcraft_engine::new_session(), crate::UiState::default());
        let builder = egui_kittest::Harness::builder().with_size(vec2(1600.0, 960.0)).with_step_dt(1.0 / 60.0);
        let builder = if render { builder.wgpu() } else { builder };
        let mut h = builder.build_ui_state(
            |ui, app: &mut MaterialCraftApp| {
                app.step(&ui.ctx().clone());
                app.draw(ui);
            },
            app,
        );
        h.run_steps(10);
        let pointer = h.state().graph.view.center();
        h.event(egui::Event::PointerMoved(pointer));
        h.step();
        h.key_press(egui::Key::Tab);
        h.step();
        assert!(h.state().graph.search.is_some());
        h
    }

    fn select_result_after_cutoff(h: &mut egui_kittest::Harness<'static, MaterialCraftApp>) -> &'static str {
        for _ in 0..60 {
            h.key_press(egui::Key::ArrowDown);
            h.step();
        }
        h.run_steps(3); // Let scroll_to_me bring the selected row into view.
        let selected = h.state().graph.search.as_ref().unwrap().sel;
        assert_eq!(selected, 60);
        let def = matches("")[selected];
        let popup = h.ctx.memory(|m| m.area_rect(egui::Id::unique("node-search"))).unwrap();
        fn visible(shape: &egui::epaint::Shape, clip: Rect, popup: Rect, label: &str) -> bool {
            match shape {
                egui::epaint::Shape::Text(t) => popup.contains(t.pos) && clip.contains(t.pos) && t.galley.text().trim_start().starts_with(label),
                egui::epaint::Shape::Vec(v) => v.iter().any(|s| visible(s, clip, popup, label)),
                _ => false,
            }
        }
        assert!(
            h.output().shapes.iter().any(|s| visible(&s.shape, s.clip_rect, popup, def.label)),
            "keyboard-selected result {selected} ({}) is not visible in the popup",
            def.label
        );
        def.id
    }

    #[test]
    fn keyboard_search_selection_after_sixty_results_is_visible_and_inserts() {
        let mut h = search_harness(false);
        let expected = select_result_after_cutoff(&mut h);
        h.key_press(egui::Key::Enter);
        h.step();
        assert!(h.state().graph.search.is_none());
        let graph = h.state().session.graph().unwrap();
        assert_eq!(graph.nodes.len(), 1);
        assert_eq!(graph.nodes[0].def, expected);
    }

    #[test]
    #[ignore = "requires an offscreen GPU"]
    fn search_result_after_sixty_screenshot() {
        let mut h = search_harness(true);
        select_result_after_cutoff(&mut h);
        let dir = std::path::PathBuf::from(std::env::var_os("MC_SNAPSHOT_DIR").expect("set MC_SNAPSHOT_DIR"));
        std::fs::create_dir_all(&dir).unwrap();
        h.render().unwrap().save(dir.join("search-result-61.png")).unwrap();
    }

    #[test]
    fn error_node_still_shows_preview_resolution_tooltip() {
        let mut app = MaterialCraftApp::headless(materialcraft_engine::sample_session(), crate::UiState::default());
        app.run("file.new", json!({"template": "empty"}));
        let id = NodeId(app.run("graph.node.add", json!({"def": "uniform"})).and_then(|v| v["id"].as_u64()).unwrap());
        app.session.cook();
        assert!(app.session.preview_size.is_some());
        let result = app.session.cooked.nodes.get_mut(&id).unwrap();
        result.error = Some("cook failed".to_string());
        assert!(result.outputs.first().is_some());
        let builder = egui_kittest::Harness::builder().with_size(vec2(1600.0, 960.0)).with_step_dt(1.0 / 60.0);
        let mut h = builder.build_ui_state(
            |ui, app: &mut MaterialCraftApp| {
                app.step(&ui.ctx().clone());
                app.draw(ui);
            },
            app,
        );
        h.run_steps(10);
        let rect = h.state().graph.rects.iter().find(|(n, _)| *n == id).unwrap().1;
        h.event(egui::Event::PointerMoved(rect.center()));
        h.run_steps(90);
        fn texts(shape: &egui::epaint::Shape, out: &mut Vec<String>) {
            match shape {
                egui::epaint::Shape::Text(t) => out.push(t.galley.text().to_owned()),
                egui::epaint::Shape::Vec(shapes) => {
                    for s in shapes {
                        texts(s, out);
                    }
                }
                _ => {}
            }
        }
        let mut painted = Vec::new();
        for s in &h.output().shapes {
            texts(&s.shape, &mut painted);
        }
        let text = painted.join("\n");
        assert!(text.contains("Parent size:"), "missing parent size in tooltip: {text}");
        assert!(text.contains("Preview limit: 512 px"), "missing preview limit in tooltip: {text}");
        assert!(text.contains("Edit > Preferences"), "missing preview-size hint in tooltip: {text}");
    }
}
