//! Properties: the selected node's base and specific parameters, or the graph's own settings,
//! inputs and outputs when no node is selected. Every change is an engine command (slider drags
//! merge into one undo step).

use egui::{Color32, Pos2, Rect, RichText, Sense, Stroke, Ui, pos2, vec2};
use egui_phosphor::regular as icon;
use materialcraft_engine::core::{Curve, Gradient, NodeId, OutputFormat, SizeMode, Value};
use materialcraft_engine::nodes::ParamDef;
use serde_json::{Value as J, json};

use crate::MaterialCraftApp;
use crate::i18n::tr;
use crate::theme::Tokens;

/// The panel title: "<node> - Properties" or "<graph> - Properties".
pub fn title(app: &MaterialCraftApp) -> String {
    let sel = app.session.selection.last().and_then(|id| app.session.graph().ok()?.node(*id).ok().map(crate::graph_view::node_label));
    let name = sel.or_else(|| app.session.graph().ok().map(|g| g.name.clone())).unwrap_or_default();
    format!("{name} - {}", tr("Properties"))
}

fn section(ui: &mut Ui, id: &str, title: &str, body: impl FnOnce(&mut Ui)) {
    let tk = Tokens::get(ui.ctx());
    let id = ui.make_persistent_id(id);
    let mut state = egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, true);
    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 20.0), Sense::click());
    crate::widgets::name(&resp, egui::Role::Button, title);
    ui.painter().rect_filled(r, 3.0, tk.row);
    let chev = if state.is_open() { icon::CARET_DOWN } else { icon::CARET_RIGHT };
    ui.painter().text(r.left_center() + vec2(6.0, 0.0), egui::Align2::LEFT_CENTER, chev, egui::FontId::proportional(10.0), tk.text_dim);
    ui.painter().text(r.left_center() + vec2(20.0, 0.0), egui::Align2::LEFT_CENTER, title.to_uppercase(), egui::FontId::proportional(10.5), tk.text_strong);
    if resp.clicked() {
        state.toggle(ui);
    }
    state.show_body_unindented(ui, |ui| {
        ui.add_space(3.0);
        body(ui);
        ui.add_space(6.0);
    });
}

pub fn show(app: &mut MaterialCraftApp, ui: &mut Ui) {
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.set_width(ui.available_width());
        let node = app.session.selection.last().and_then(|id| app.session.graph().ok()?.node(*id).ok().cloned());
        ui.push_id((app.session.graph, node.as_ref().map(|n| n.id)), |ui| match node {
            Some(n) => node_props(app, ui, &n),
            None => graph_props(app, ui),
        });
    });
}

fn node_props(app: &mut MaterialCraftApp, ui: &mut Ui, n: &materialcraft_engine::core::Node) {
    let tk = Tokens::get(ui.ctx());
    let def = materialcraft_engine::nodes::find(&n.def);
    // Header: label (renamable) and description.
    ui.horizontal(|ui| {
        ui.painter().rect_filled(Rect::from_min_size(ui.cursor().min + vec2(0.0, 2.0), vec2(4.0, 16.0)), 1.0, crate::graph_view::header_color(&n.def));
        ui.add_space(8.0);
        let mut label = if n.label.is_empty() { crate::graph_view::node_label(n) } else { n.label.clone() };
        let r = ui.add(egui::TextEdit::singleline(&mut label).desired_width(ui.available_width() - 4.0).font(egui::TextStyle::Heading));
        crate::widgets::name(&r, egui::Role::TextInput, "Node label");
        if r.changed() {
            app.run("graph.node.rename", json!({"id": n.id.0, "label": label}));
        }
    });
    if let Some(d) = def
        && !d.description.is_empty()
    {
        ui.label(RichText::new(d.description).size(11.0).color(tk.text_dim));
    }
    if let Some(r) = app.session.cooked.nodes.get(&n.id)
        && let Some(e) = &r.error
    {
        ui.label(RichText::new(format!("{}  {e}", icon::WARNING)).color(tk.danger));
    }
    ui.add_space(4.0);
    section(ui, "base", tr("Base parameters"), |ui| base_params(app, ui, n));
    if n.def == "graph_instance" {
        section(ui, "instance", tr("Instance parameters"), |ui| instance_params(app, ui, n));
        return;
    }
    let Some(def) = def else { return };
    if def.params.is_empty() {
        return;
    }
    section(ui, "specific", tr("Specific parameters"), |ui| {
        if n.def == "levels" {
            crate::levels_editor::show(app, ui, n);
            ui.add_space(6.0);
        }
        let exposed: Vec<(String, String)> =
            app.session.graph().map(|g| g.exposed.iter().map(|e| (e.id.clone(), e.label.clone())).collect()).unwrap_or_default();
        for p in &def.params {
            let cur = n.params.get(p.id).cloned().unwrap_or_else(|| p.default.clone());
            if n.functions.contains_key(p.id) {
                function_row(app, ui, n.id, p.id, p.label);
                continue;
            }
            let bound = n.bindings.get(p.id).cloned();
            param_row(app, ui, n.id, p, cur, bound, &exposed);
        }
    });
}

fn base_params(app: &mut MaterialCraftApp, ui: &mut Ui, n: &materialcraft_engine::core::Node) {
    let b = &n.base;
    let id = n.id.0;
    let modes = [(SizeMode::RelativeToInput, "Relative to input"), (SizeMode::RelativeToParent, "Relative to parent"), (SizeMode::Absolute, "Absolute")];
    let cur = modes.iter().find(|m| m.0 == b.size_mode).map_or("", |m| m.1);
    crate::widgets::combo(ui, tr("Output size"), tr(cur), |ui| {
        for (m, label) in modes {
            if ui.selectable_label(b.size_mode == m, tr(label)).clicked() {
                let key = match m {
                    SizeMode::RelativeToInput => "relativeToInput",
                    SizeMode::RelativeToParent => "relativeToParent",
                    SizeMode::Absolute => "absolute",
                };
                let size = if m == SizeMode::Absolute { json!([10, 10]) } else { json!([0, 0]) };
                app.run("graph.node.base", json!({"id": id, "sizeMode": key, "size": size}));
            }
        }
    });
    let (lo, hi) = if b.size_mode == SizeMode::Absolute { (4.0, 13.0) } else { (-6.0, 6.0) };
    for (axis, label) in [(0usize, "Width"), (1, "Height")] {
        let mut v = b.size[axis] as f32;
        let caption = if b.size_mode == SizeMode::Absolute {
            format!("{label}  {} px", 1u32 << b.size[axis].clamp(4, 13))
        } else {
            format!("{label}  x {}", 2f32.powi(b.size[axis]))
        };
        if crate::widgets::slider(ui, &caption, &mut v, lo..=hi, 0).changed() {
            let mut size = b.size;
            size[axis] = v.round() as i32;
            app.run("graph.node.base", json!({"id": id, "size": size}));
        }
    }
    crate::widgets::combo(ui, tr("Output format"), tr(b.format.label()), |ui| {
        for f in OutputFormat::ALL {
            if ui.selectable_label(b.format == f, tr(f.label())).clicked() {
                let key = match f {
                    OutputFormat::Inherit => "inherit",
                    OutputFormat::U8 => "u8",
                    OutputFormat::U16 => "u16",
                    OutputFormat::F16 => "f16",
                    OutputFormat::F32 => "f32",
                };
                app.run("graph.node.base", json!({"id": id, "format": key}));
            }
        }
    });
    let mut seed = b.seed as f32;
    if crate::widgets::slider(ui, tr("Random seed"), &mut seed, 0.0..=1000.0, 0).changed() {
        app.run("graph.node.base", json!({"id": id, "seed": seed.round() as i64}));
    }
    if ui.button(format!("{}  {}", icon::SHUFFLE, tr("New random seed"))).clicked() {
        let s = (crate::now_ms() as u64 ^ id).wrapping_mul(2654435761) % 1000;
        app.run("graph.node.base", json!({"id": id, "seed": s}));
    }
    let mut tiling = b.tiling[0] && b.tiling[1];
    if ui.checkbox(&mut tiling, tr("Tiling (H and V)")).changed() {
        app.run("graph.node.base", json!({"id": id, "tiling": tiling}));
    }
}

fn set(app: &mut MaterialCraftApp, node: NodeId, param: &str, value: J) {
    app.run("graph.node.param", json!({"id": node.0, "param": param, "value": value}));
}

fn typed(v: &Value) -> J {
    serde_json::to_value(v).unwrap_or(J::Null)
}

/// One parameter: its editor, plus the expose/bind menu.
fn param_row(app: &mut MaterialCraftApp, ui: &mut Ui, node: NodeId, p: &ParamDef, cur: Value, bound: Option<String>, exposed: &[(String, String)]) {
    let tk = Tokens::get(ui.ctx());
    if let Some(input) = &bound {
        ui.horizontal(|ui| {
            ui.label(RichText::new(tr(p.label)).color(tk.text));
            ui.label(RichText::new(format!("{} {input}", icon::LINK)).color(tk.accent));
            if crate::widgets::icon_button(ui, icon::LINK_BREAK, tr("Unbind"), false, 18.0).clicked() {
                app.run("graph.node.bind", json!({"id": node.0, "param": p.id, "input": J::Null}));
            }
        });
        return;
    }
    let resp_menu = |app: &mut MaterialCraftApp, resp: &egui::Response| {
        resp.context_menu(|ui| {
            if ui.button(tr("Reset to default")).clicked() {
                app.run("graph.node.reset", json!({"id": node.0, "param": p.id}));
                ui.close();
            }
            let dynamic = !matches!(p.default, Value::Text(_) | Value::Curve(_) | Value::Gradient(_) | Value::Function(_));
            if dynamic && ui.button(tr("Set function...")).clicked() {
                if app.run("graph.node.function", json!({"id": node.0, "param": p.id})).is_some() {
                    app.fn_editor = Some(crate::function_editor::FnEditor::new(node, &format!("fn:{}", p.id)));
                }
                ui.close();
            }
            if ui.button(tr("Expose as graph input")).clicked() {
                app.run("graph.exposed.add", json!({"id": p.id, "label": p.label, "bind": {"node": node.0, "param": p.id}}));
                ui.close();
            }
            if !exposed.is_empty() {
                ui.menu_button(tr("Bind to input"), |ui| {
                    for (id, label) in exposed {
                        if ui.button(label).clicked() {
                            app.run("graph.node.bind", json!({"id": node.0, "param": p.id, "input": id}));
                            ui.close();
                        }
                    }
                });
            }
        });
    };
    match cur {
        Value::Float(mut v) => {
            if p.angle {
                let mut deg = v * 360.0;
                let r = crate::widgets::slider(ui, tr(p.label), &mut deg, -360.0..=360.0, 1);
                if r.changed() {
                    v = deg / 360.0;
                    set(app, node, p.id, json!(v));
                }
                resp_menu(app, &r);
            } else {
                let (lo, hi) = (p.min.min(v), p.max.max(v));
                let r = crate::widgets::slider(ui, tr(p.label), &mut v, lo..=hi, if hi - lo > 20.0 { 1 } else { 3 });
                if r.changed() {
                    set(app, node, p.id, json!(v));
                }
                resp_menu(app, &r);
            }
        }
        Value::Int(i) => {
            let mut v = i as f32;
            let range = p.min.min(v)..=p.max.max(v);
            let r = crate::widgets::slider(ui, tr(p.label), &mut v, range, 0);
            if r.changed() && v.round() as i32 != i {
                set(app, node, p.id, json!(v.round() as i32));
            }
            resp_menu(app, &r);
        }
        Value::Float2(v) => {
            ui.label(RichText::new(tr(p.label)).color(tk.text));
            let mut nv = v;
            for (k, axis) in ["X", "Y"].iter().enumerate() {
                let range = p.min.min(nv[k])..=p.max.max(nv[k]);
                let r = crate::widgets::slider(ui, axis, &mut nv[k], range, 3);
                if r.changed() {
                    set(app, node, p.id, json!(nv));
                }
                resp_menu(app, &r);
            }
        }
        Value::Int2(v) => {
            ui.label(RichText::new(tr(p.label)).color(tk.text));
            for k in 0..2 {
                let mut f = v[k] as f32;
                if crate::widgets::slider(ui, ["X", "Y"][k], &mut f, p.min..=p.max, 0).changed() {
                    let mut nv = v;
                    nv[k] = f.round() as i32;
                    set(app, node, p.id, json!(nv));
                }
            }
        }
        Value::Bool(b) => {
            let mut v = b;
            let r = ui.checkbox(&mut v, tr(p.label));
            if r.changed() {
                set(app, node, p.id, json!(v));
            }
            resp_menu(app, &r);
        }
        Value::Color(c) | Value::Float4(c) => {
            ui.horizontal(|ui| {
                ui.label(RichText::new(tr(p.label)).color(tk.text));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let mut rgb = [c[0], c[1], c[2]];
                    if crate::widgets::color_swatch(ui, &format!("{}-{}", node.0, p.id), &mut rgb, 90.0) {
                        set(app, node, p.id, json!([rgb[0], rgb[1], rgb[2], c[3]]));
                    }
                });
            });
        }
        Value::Enum(i) => {
            let cur = p.options.get(i as usize).copied().unwrap_or("?");
            crate::widgets::combo(ui, tr(p.label), tr(cur), |ui| {
                for (k, o) in p.options.iter().enumerate() {
                    if ui.selectable_label(k as u32 == i, tr(o)).clicked() {
                        set(app, node, p.id, json!(k));
                    }
                }
            });
        }
        Value::Text(t) if p.resource => {
            let names: Vec<String> = app.session.pkg.resources.iter().map(|r| r.name.clone()).collect();
            crate::widgets::combo(ui, tr(p.label), if t.is_empty() { tr("(checker)").to_string() } else { t.clone() }, |ui| {
                for n in &names {
                    if ui.selectable_label(*n == t, n).clicked() {
                        set(app, node, p.id, json!(n));
                    }
                }
            });
            if ui.button(format!("{}  {}", icon::IMAGE, tr("Import bitmap..."))).clicked() {
                crate::menus::invoke(app, ui.ctx(), "file.importBitmap");
            }
        }
        Value::Text(t) => {
            ui.horizontal(|ui| {
                ui.label(RichText::new(tr(p.label)).color(tk.text));
                let mut s = t.clone();
                let r = ui.add(egui::TextEdit::singleline(&mut s).desired_width(ui.available_width()));
                crate::widgets::name(&r, egui::Role::TextInput, p.label);
                if r.changed() {
                    set(app, node, p.id, json!(s));
                }
            });
        }
        Value::Curve(c) => {
            ui.label(RichText::new(tr(p.label)).color(tk.text));
            if let Some(nc) = curve_editor(ui, &format!("curve-{}-{}", node.0, p.id), &c) {
                set(app, node, p.id, json!(nc.points.iter().map(|q| [q.x, q.y]).collect::<Vec<_>>()));
            }
        }
        Value::Gradient(g) => {
            ui.label(RichText::new(tr(p.label)).color(tk.text));
            if let Some(ng) = gradient_editor(ui, &format!("grad-{}-{}", node.0, p.id), &g) {
                set(app, node, p.id, typed(&Value::Gradient(ng)));
            }
        }
        Value::Function(f) => {
            ui.label(RichText::new(tr(p.label)).color(tk.text));
            crate::windows::function_summary(app, ui, node, p.id, &f);
        }
        other => {
            ui.label(format!("{}: {}", p.label, other.type_name()));
        }
    }
}

/// A parameter driven by a function: its label, Edit and Remove.
fn function_row(app: &mut MaterialCraftApp, ui: &mut Ui, node: NodeId, param: &str, label: &str) {
    let tk = Tokens::get(ui.ctx());
    ui.horizontal(|ui| {
        ui.label(RichText::new(tr(label)).color(tk.text));
        ui.label(RichText::new(format!("{} {}", icon::FUNCTION, tr("function"))).color(tk.accent));
        if ui.small_button(tr("Edit")).clicked() {
            app.fn_editor = Some(crate::function_editor::FnEditor::new(node, &format!("fn:{param}")));
        }
        if crate::widgets::icon_button(ui, icon::X, tr("Remove function"), false, 18.0).clicked() {
            app.run("graph.node.function", json!({"id": node.0, "param": param, "remove": true}));
        }
    });
}

fn instance_params(app: &mut MaterialCraftApp, ui: &mut Ui, n: &materialcraft_engine::core::Node) {
    let tk = Tokens::get(ui.ctx());
    let Some((gi, sub)) = materialcraft_engine::eval::instance_graph(&app.session.pkg, n) else {
        ui.label(RichText::new(tr("The instanced graph is missing")).color(tk.danger));
        return;
    };
    let exposed = sub.exposed.clone();
    if ui.button(format!("{}  {}", icon::ARROW_SQUARE_OUT, tr("Open graph"))).clicked() {
        app.run("graph.open", json!({"graph": gi}));
        return;
    }
    if exposed.is_empty() {
        ui.label(RichText::new(tr("This graph exposes no inputs")).color(tk.text_dim));
    }
    for e in exposed {
        let cur = n.params.get(&e.id).cloned().unwrap_or_else(|| e.default.clone());
        if let Some(v) = input_editor(ui, &e, &cur) {
            app.run("graph.node.param", json!({"id": n.id.0, "param": e.id, "value": v}));
        }
    }
}

/// The editor for one exposed input's value; returns the new value as JSON when it changed.
fn input_editor(ui: &mut Ui, e: &materialcraft_engine::core::ExposedParam, cur: &Value) -> Option<J> {
    match *cur {
        Value::Float(mut f) => {
            let v0 = f;
            crate::widgets::slider(ui, &e.label, &mut f, e.min.min(v0)..=e.max.max(v0), 3).changed().then(|| json!(f))
        }
        Value::Int(i) => {
            let mut f = i as f32;
            let v0 = f;
            crate::widgets::slider(ui, &e.label, &mut f, e.min.min(v0)..=e.max.max(v0), 0).changed().then(|| json!(f.round()))
        }
        Value::Bool(mut b) => ui.checkbox(&mut b, &e.label).changed().then(|| json!(b)),
        Value::Enum(i) => {
            let mut out = None;
            crate::widgets::combo(ui, &e.label, e.options.get(i as usize).cloned().unwrap_or_default(), |ui| {
                for (k, o) in e.options.iter().enumerate() {
                    if ui.selectable_label(k as u32 == i, o).clicked() {
                        out = Some(json!(k));
                    }
                }
            });
            out
        }
        Value::Color(c) => {
            let mut rgb = [c[0], c[1], c[2]];
            let mut out = None;
            ui.horizontal(|ui| {
                ui.label(&e.label);
                if crate::widgets::color_swatch(ui, &format!("exp-{}", e.id), &mut rgb, 80.0) {
                    out = Some(json!([rgb[0], rgb[1], rgb[2], c[3]]));
                }
            });
            out
        }
        ref other => {
            ui.label(format!("{}: {}", e.label, other.type_name()));
            None
        }
    }
}

fn graph_props(app: &mut MaterialCraftApp, ui: &mut Ui) {
    let tk = Tokens::get(ui.ctx());
    let Ok(g) = app.session.graph() else {
        ui.label(tr("No graph is open"));
        return;
    };
    let g = g.clone();
    section(ui, "graph-base", tr("Base parameters"), |ui| {
        ui.horizontal(|ui| {
            ui.label(tr("Name"));
            let name_id = ui.make_persistent_id(format!("graph-name-{}", app.session.graph));
            let mut name: String = match ui.data(|d| d.get_temp::<(usize, String, String)>(name_id)) {
                Some((idx, seen, text)) if idx == app.session.graph && seen == g.name => text,
                _ => g.name.clone(),
            };
            let r = ui.add(egui::TextEdit::singleline(&mut name).desired_width(ui.available_width()));
            crate::widgets::name(&r, egui::Role::TextInput, "Graph name");
            if r.lost_focus() {
                let committed = name.clone();
                if committed != g.name {
                    app.run("graph.rename", json!({"name": committed}));
                }
                ui.data_mut(|d| d.remove_temp::<(usize, String, String)>(name_id));
            } else if r.has_focus() || name != g.name {
                ui.data_mut(|d| d.insert_temp(name_id, (app.session.graph, g.name.clone(), name.clone())));
            } else {
                ui.data_mut(|d| d.remove_temp::<(usize, String, String)>(name_id));
            }
            if g.nodes.is_empty() && g.name == "Empty" && !r.has_focus() && !app.session.pkg.graphs.iter().any(|x| x.name == "New_graph") {
                let renamed = app.run("graph.rename", json!({"name": "New_graph"}));
                if renamed.is_some() {
                    ui.data_mut(|d| d.remove_temp::<(usize, String, String)>(name_id));
                }
            }
        });
        for (axis, label) in [(0usize, "Width"), (1, "Height")] {
            let mut v = g.size[axis] as f32;
            if crate::widgets::slider(ui, &format!("{}  {} px", tr(label), 1u32 << g.size[axis]), &mut v, 4.0..=13.0, 0).changed() {
                let mut s = g.size;
                s[axis] = v.round() as u32;
                app.run("graph.size", json!({"log2": s}));
            }
        }
        crate::widgets::combo(ui, tr("Output format"), tr(g.format.label()), |ui| {
            for (f, key) in [(OutputFormat::U8, "u8"), (OutputFormat::U16, "u16"), (OutputFormat::F16, "f16"), (OutputFormat::F32, "f32")] {
                if ui.selectable_label(g.format == f, tr(f.label())).clicked() {
                    app.run("graph.format", json!({"format": key}));
                }
            }
        });
        let mut seed = g.seed as f32;
        if crate::widgets::slider(ui, tr("Random seed"), &mut seed, 0.0..=1000.0, 0).changed() {
            app.run("graph.seed", json!({"seed": seed.round() as i64}));
        }
    });
    section(ui, "graph-attr", tr("Attributes"), |ui| {
        for (key, label, cur) in [
            ("category", "Category", &g.attributes.category),
            ("description", "Description", &g.attributes.description),
            ("author", "Author", &g.attributes.author),
            ("tags", "Tags", &g.attributes.tags),
        ] {
            ui.horizontal(|ui| {
                ui.label(tr(label));
                let s_id = ui.make_persistent_id(format!("graph-attr-{}-{}", app.session.graph, key));
                let mut s: String = match ui.data(|d| d.get_temp::<(usize, String, String)>(s_id)) {
                    Some((idx, seen, text)) if idx == app.session.graph && seen == *cur => text,
                    _ => cur.clone(),
                };
                let r = ui.add(egui::TextEdit::singleline(&mut s).desired_width(ui.available_width()));
                crate::widgets::name(&r, egui::Role::TextInput, label);
                if r.lost_focus() {
                    if s != *cur {
                        app.run("graph.attributes", json!({key: s}));
                    }
                    ui.data_mut(|d| d.remove_temp::<(usize, String, String)>(s_id));
                } else if r.has_focus() || s != *cur {
                    ui.data_mut(|d| d.insert_temp(s_id, (app.session.graph, cur.clone(), s.clone())));
                } else {
                    ui.data_mut(|d| d.remove_temp::<(usize, String, String)>(s_id));
                }
            });
        }
    });
    section(ui, "graph-inputs", tr("Input parameters"), |ui| {
        if g.exposed.is_empty() {
            ui.label(RichText::new(tr("Right-click a node parameter > Expose as graph input")).size(11.0).color(tk.text_dim));
        } else {
            presets_bar(app, ui, &g);
        }
        let mut groups: Vec<&str> = Vec::new();
        for e in &g.exposed {
            if !groups.contains(&e.group.as_str()) {
                groups.push(&e.group);
            }
        }
        let mut last_group: Option<&str> = None;
        let mut order: Vec<&materialcraft_engine::core::ExposedParam> = Vec::new();
        for grp in &groups {
            order.extend(g.exposed.iter().filter(|e| e.group == *grp));
        }
        for e in order {
            if last_group != Some(e.group.as_str()) && !e.group.is_empty() {
                ui.label(RichText::new(e.group.to_uppercase()).size(10.5).strong().color(tk.text_dim));
            }
            last_group = Some(e.group.as_str());
            let cur = g.input_value(&e.id).cloned().unwrap_or_else(|| e.default.clone());
            ui.horizontal(|ui| {
                if crate::widgets::icon_button(ui, icon::X, tr("Delete input"), false, 18.0).clicked() {
                    app.run("graph.exposed.delete", json!({"id": e.id}));
                }
                ui.vertical(|ui| {
                    if let Some(v) = input_editor(ui, e, &cur) {
                        app.run("graph.exposed.set", json!({"id": e.id, "value": v}));
                    }
                });
            });
        }
    });
    input_previews(app, ui, &g);
    section(ui, "graph-outputs", tr("Outputs"), |ui| {
        for n in g.nodes.iter().filter(|n| n.def == "output") {
            let usage = n.params.get("usage").and_then(Value::as_i32).unwrap_or(0) as usize;
            let label = format!(
                "{}  {} | {}",
                icon::SIGN_OUT,
                materialcraft_engine::nodes::atomic::USAGES.get(usage).copied().unwrap_or("Any"),
                materialcraft_engine::eval::identifier(n)
            );
            if ui.add(egui::Button::selectable(app.session.view2d.node == Some(n.id), label)).clicked() {
                app.run("view.2d", json!({"node": n.id.0}));
            }
        }
    });
}

/// Preview images of the graph's input nodes: a bitmap resource, or (desktop) an image file such
/// as a mesh map baked in TextureCraft.
fn input_previews(app: &mut MaterialCraftApp, ui: &mut Ui, g: &materialcraft_engine::core::Graph) {
    let inputs = materialcraft_engine::material_cmds::image_inputs(g);
    if inputs.is_empty() {
        return;
    }
    let resources: Vec<String> = app.session.pkg.resources.iter().map(|r| r.name.clone()).collect();
    section(ui, "graph-input-images", tr("Input previews"), |ui| {
        for i in &inputs {
            let id = &i.identifier;
            let bound = g.input_images.get(id);
            ui.horizontal(|ui| {
                ui.label(id);
                egui::ComboBox::from_id_salt(("input-preview", id)).width(120.0).selected_text(bound.map_or(tr("None"), String::as_str)).show_ui(ui, |ui| {
                    if ui.selectable_label(bound.is_none(), tr("None")).clicked() {
                        app.run("graph.input.unbind", json!({"identifier": id}));
                    }
                    for r in &resources {
                        if ui.selectable_label(bound == Some(r), r).clicked() {
                            app.run("graph.input.bind", json!({"identifier": id, "resource": r}));
                        }
                    }
                });
                #[cfg(not(target_arch = "wasm32"))]
                if crate::widgets::icon_button(ui, icon::FOLDER_OPEN, tr("Pick an image file (a mesh map baked in TextureCraft)"), false, 18.0).clicked()
                    && let Some(p) = rfd::FileDialog::new().add_filter("Images", materialcraft_engine::formats::IMAGE_EXTENSIONS).pick_file()
                {
                    app.run("graph.input.bind", json!({"identifier": id, "path": p.display().to_string()}));
                }
            });
        }
    });
}

/// Presets: apply one, save the current inputs under a name, delete one.
fn presets_bar(app: &mut MaterialCraftApp, ui: &mut Ui, g: &materialcraft_engine::core::Graph) {
    let name_id = ui.make_persistent_id("preset-name");
    let mut name: String = ui.data(|d| d.get_temp(name_id)).unwrap_or_default();
    ui.horizontal(|ui| {
        let names: Vec<String> = g.presets.iter().map(|p| p.name.clone()).collect();
        let r = egui::ComboBox::from_id_salt("presets").width(110.0).selected_text(tr("Presets")).show_ui(ui, |ui| {
            for n in &names {
                if ui.button(n).clicked() {
                    app.run("graph.preset.apply", json!({"name": n}));
                }
            }
            if names.is_empty() {
                ui.label(tr("No presets yet"));
            }
        });
        crate::widgets::name(&r.response, egui::Role::ComboBox, "Presets");
        let t = ui.add(egui::TextEdit::singleline(&mut name).hint_text(tr("Preset name")).desired_width(90.0));
        crate::widgets::name(&t, egui::Role::TextInput, "Preset name");
        if crate::widgets::icon_button(ui, icon::FLOPPY_DISK, tr("Save preset"), false, 20.0).clicked() && !name.trim().is_empty() {
            app.run("graph.preset.save", json!({"name": name}));
        }
        if crate::widgets::icon_button(ui, icon::X, tr("Delete preset"), false, 20.0).clicked() && !name.trim().is_empty() {
            app.run("graph.preset.delete", json!({"name": name}));
        }
    });
    ui.data_mut(|d| d.insert_temp(name_id, name));
}

/// A tone curve editor: drag points, double-click to add one, right-click a point to remove it.
/// Returns the new curve when it changed.
pub fn curve_editor(ui: &mut Ui, id: &str, c: &Curve) -> Option<Curve> {
    let tk = Tokens::get(ui.ctx());
    let w = ui.available_width().min(240.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(w, w * 0.75), Sense::click_and_drag());
    crate::widgets::name(&resp, egui::Role::Canvas, "Curve editor");
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 4.0, tk.field);
    for i in 1..4 {
        let t = i as f32 / 4.0;
        p.line_segment([pos2(rect.left() + rect.width() * t, rect.top()), pos2(rect.left() + rect.width() * t, rect.bottom())], Stroke::new(1.0, tk.separator));
        p.line_segment([pos2(rect.left(), rect.top() + rect.height() * t), pos2(rect.right(), rect.top() + rect.height() * t)], Stroke::new(1.0, tk.separator));
    }
    let to_s = |x: f32, y: f32| pos2(rect.left() + x * rect.width(), rect.bottom() - y * rect.height());
    let from_s = |s: Pos2| (((s.x - rect.left()) / rect.width()).clamp(0.0, 1.0), ((rect.bottom() - s.y) / rect.height()).clamp(0.0, 1.0));
    let pts: Vec<Pos2> = (0..=64).map(|i| i as f32 / 64.0).map(|x| to_s(x, c.eval(x).clamp(0.0, 1.0))).collect();
    p.add(egui::Shape::line(pts, Stroke::new(2.0, tk.accent)));
    let drag_id = ui.make_persistent_id(id).with("drag");
    let mut dragging: Option<usize> = ui.data(|d| d.get_temp(drag_id));
    let pointer = resp.interact_pointer_pos().or(resp.hover_pos());
    let nearest = pointer.and_then(|q| {
        c.points
            .iter()
            .enumerate()
            .map(|(i, pt)| (i, to_s(pt.x, pt.y).distance(q)))
            .filter(|(_, d)| *d < 10.0)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i)
    });
    for (i, pt) in c.points.iter().enumerate() {
        let hot = Some(i) == nearest || Some(i) == dragging;
        p.circle(to_s(pt.x, pt.y), if hot { 5.0 } else { 4.0 }, tk.panel, Stroke::new(1.5, if hot { tk.text_strong } else { tk.text }));
    }
    let mut out: Option<Curve> = None;
    if resp.drag_started() {
        dragging = nearest;
    }
    if resp.dragged()
        && let (Some(i), Some(q)) = (dragging, pointer)
    {
        let (x, y) = from_s(q);
        let mut nc = c.clone();
        let last = nc.points.len().saturating_sub(1);
        if let Some(pt) = nc.points.get_mut(i) {
            // End points keep their x.
            if i != 0 && i != last {
                pt.x = x;
            }
            pt.y = y;
        }
        nc.points.sort_by(|a, b| a.x.total_cmp(&b.x));
        out = Some(nc);
    }
    if resp.drag_stopped() {
        dragging = None;
    }
    ui.data_mut(|d| d.insert_temp(drag_id, dragging));
    if resp.double_clicked()
        && let Some(q) = pointer
    {
        let (x, _) = from_s(q);
        let mut nc = c.clone();
        nc.points.push(materialcraft_engine::core::CurvePoint { x, y: c.eval(x) });
        nc.points.sort_by(|a, b| a.x.total_cmp(&b.x));
        out = Some(nc);
    }
    if resp.secondary_clicked()
        && let Some(i) = nearest
        && i != 0
        && i + 1 != c.points.len()
    {
        let mut nc = c.clone();
        nc.points.remove(i);
        out = Some(nc);
    }
    out
}

/// A gradient editor: the ramp with its keys below; drag keys, click the ramp to add one, pick a
/// key's color, right-click a key to remove it.
pub fn gradient_editor(ui: &mut Ui, id: &str, g: &Gradient) -> Option<Gradient> {
    let tk = Tokens::get(ui.ctx());
    let w = ui.available_width().min(260.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 30.0), Sense::click_and_drag());
    crate::widgets::name(&resp, egui::Role::Canvas, "Gradient editor");
    let p = ui.painter_at(rect.expand(6.0));
    let bar = Rect::from_min_size(rect.min, vec2(w, 18.0));
    let n = 64;
    for i in 0..n {
        let t0 = i as f32 / n as f32;
        let c = g.eval(t0 + 0.5 / n as f32);
        let q = |v: f32| (materialcraft_engine::image::linear_to_srgb(v.clamp(0.0, 1.0)) * 255.0) as u8;
        let r = Rect::from_min_max(pos2(bar.left() + bar.width() * t0, bar.top()), pos2(bar.left() + bar.width() * (t0 + 1.0 / n as f32) + 0.5, bar.bottom()));
        p.rect_filled(r, 0.0, Color32::from_rgb(q(c[0]), q(c[1]), q(c[2])));
    }
    p.rect_stroke(bar, 2.0, Stroke::new(1.0, tk.field_border), egui::StrokeKind::Inside);
    let sel_id = ui.make_persistent_id(id).with("sel");
    let mut sel: usize = ui.data(|d| d.get_temp(sel_id)).unwrap_or(0);
    let key_x = |pos: f32| bar.left() + pos * bar.width();
    let pointer = resp.interact_pointer_pos().or(resp.hover_pos());
    let nearest = pointer.and_then(|q| {
        g.keys.iter().enumerate().map(|(i, k)| (i, (key_x(k.pos) - q.x).abs())).filter(|(_, d)| *d < 7.0).min_by(|a, b| a.1.total_cmp(&b.1)).map(|(i, _)| i)
    });
    for (i, k) in g.keys.iter().enumerate() {
        let x = key_x(k.pos);
        let tri = vec![pos2(x, bar.bottom() + 1.0), pos2(x - 5.0, bar.bottom() + 10.0), pos2(x + 5.0, bar.bottom() + 10.0)];
        p.add(egui::Shape::convex_polygon(tri, if i == sel { tk.accent } else { tk.text }, Stroke::NONE));
    }
    let mut out: Option<Gradient> = None;
    if resp.drag_started()
        && let Some(i) = nearest
    {
        sel = i;
    }
    if resp.dragged()
        && let Some(q) = pointer
        && nearest.is_some()
    {
        let mut ng = g.clone();
        if let Some(k) = ng.keys.get_mut(sel) {
            k.pos = ((q.x - bar.left()) / bar.width()).clamp(0.0, 1.0);
        }
        let moved = ng.keys.get(sel).copied();
        ng.sort();
        if let Some(m) = moved {
            sel = ng.keys.iter().position(|k| k.pos == m.pos && k.color == m.color).unwrap_or(sel);
        }
        out = Some(ng);
    }
    if resp.clicked()
        && let Some(q) = pointer
    {
        match nearest {
            Some(i) => sel = i,
            None => {
                let t = ((q.x - bar.left()) / bar.width()).clamp(0.0, 1.0);
                let mut ng = g.clone();
                ng.keys.push(materialcraft_engine::core::GradientKey { pos: t, color: g.eval(t) });
                ng.sort();
                sel = ng.keys.iter().position(|k| k.pos == t).unwrap_or(0);
                out = Some(ng);
            }
        }
    }
    if resp.secondary_clicked()
        && let Some(i) = nearest
        && g.keys.len() > 1
    {
        let mut ng = g.clone();
        ng.keys.remove(i);
        sel = 0;
        out = Some(ng);
    }
    ui.data_mut(|d| d.insert_temp(sel_id, sel));
    if let Some(k) = g.keys.get(sel) {
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("{} {:.2}", tr("Key at"), k.pos)).size(11.0).color(tk.text_dim));
            let mut rgb = [k.color[0], k.color[1], k.color[2]];
            if crate::widgets::color_swatch(ui, &format!("{id}-key"), &mut rgb, 60.0) {
                let mut ng = out.clone().unwrap_or_else(|| g.clone());
                if let Some(kk) = ng.keys.get_mut(sel) {
                    kk.color = [rgb[0], rgb[1], rgb[2], kk.color[3]];
                }
                out = Some(ng);
            }
        });
    }
    out
}
