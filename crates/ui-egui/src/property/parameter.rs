use super::{curve_editor, gradient_editor};
use crate::{MaterialCraftApp, i18n::tr, theme::Tokens};
use egui::{RichText, Ui};
use egui_phosphor::regular as icon;
use materialcraft_engine::core::{NodeId, Value};
use materialcraft_engine::nodes::ParameterDefinition;
use serde_json::{Value as JsonValue, json};

fn set(app: &mut MaterialCraftApp, node: NodeId, param: &str, value: JsonValue) {
    app.run("graph.node.param", json!({"id": node.0, "param": param, "value": value}));
}

fn typed(v: &Value) -> JsonValue {
    serde_json::to_value(v).unwrap_or(JsonValue::Null)
}

/// One parameter: its editor, plus the expose/bind menu
pub(super) fn param_row(
    app: &mut MaterialCraftApp,
    ui: &mut Ui,
    node: NodeId,
    parameter: &ParameterDefinition,
    current: Value,
    bound: Option<String>,
    exposed: &[(String, String)],
) {
    let tokens = Tokens::get(ui.ctx());
    if let Some(input) = &bound {
        ui.horizontal(|ui| {
            ui.label(RichText::new(tr(parameter.label)).color(tokens.text));
            ui.label(RichText::new(format!("{} {input}", icon::LINK)).color(tokens.accent));
            if crate::widgets::icon_button(ui, icon::LINK_BREAK, tr("Unbind"), false, 18.0).clicked() {
                app.run("graph.node.bind", json!({"id": node.0, "param": parameter.id, "input": JsonValue::Null}));
            }
        });
        return;
    }
    let show_menu = |app: &mut MaterialCraftApp, resp: &egui::Response| {
        resp.context_menu(|ui| {
            if ui.button(tr("Reset to default")).clicked() {
                app.run("graph.node.reset", json!({"id": node.0, "param": parameter.id}));
                ui.close();
            }
            let dynamic = !matches!(parameter.default, Value::Text(_) | Value::Curve(_) | Value::Gradient(_) | Value::Function(_) | Value::PatternGraph(_));
            if dynamic && ui.button(tr("Set function...")).clicked() {
                if app.run("graph.node.function", json!({"id": node.0, "param": parameter.id})).is_some() {
                    crate::graph::subgraph_navigation::open_function(app, node, crate::function::FunctionTarget::Dynamic(parameter.id.to_string()));
                }
                ui.close();
            }
            if ui.button(tr("Expose as graph input")).clicked() {
                app.run("graph.exposed.add", json!({"id": parameter.id, "label": parameter.label, "bind": {"node": node.0, "param": parameter.id}}));
                ui.close();
            }
            if !exposed.is_empty() {
                ui.menu_button(tr("Bind to input"), |ui| {
                    for (id, label) in exposed {
                        if ui.button(label).clicked() {
                            app.run("graph.node.bind", json!({"id": node.0, "param": parameter.id, "input": id}));
                            ui.close();
                        }
                    }
                });
            }
        });
    };

    match current {
        Value::Float(mut v) => {
            if parameter.angle {
                let mut degrees = v * 360.0;
                let response = crate::widgets::slider(ui, tr(parameter.label), &mut degrees, -360.0..=360.0, 1);
                if response.changed() {
                    v = degrees / 360.0;
                    set(app, node, parameter.id, json!(v));
                }
                show_menu(app, &response);
            } else {
                let (minimum, maximum) = (parameter.min.min(v), parameter.max.max(v));
                let response = crate::widgets::slider(ui, tr(parameter.label), &mut v, minimum..=maximum, if maximum - minimum > 20.0 { 1 } else { 3 });
                if response.changed() {
                    set(app, node, parameter.id, json!(v));
                }
                show_menu(app, &response);
            }
        }
        Value::Int(i) => {
            let mut v = i as f32;
            let range = parameter.min.min(v)..=parameter.max.max(v);
            let response = crate::widgets::slider(ui, tr(parameter.label), &mut v, range, 0);
            if response.changed() && v.round() as i32 != i {
                set(app, node, parameter.id, json!(v.round() as i32));
            }
            show_menu(app, &response);
        }
        Value::Float2(v) => {
            ui.label(RichText::new(tr(parameter.label)).color(tokens.text));
            let mut nv = v;
            for (k, axis) in ["X", "Y"].iter().enumerate() {
                let range = parameter.min.min(nv[k])..=parameter.max.max(nv[k]);
                let response = crate::widgets::slider(ui, axis, &mut nv[k], range, 3);
                if response.changed() {
                    set(app, node, parameter.id, json!(nv));
                }
                show_menu(app, &response);
            }
        }
        Value::Int2(v) => {
            ui.label(RichText::new(tr(parameter.label)).color(tokens.text));
            for k in 0..2 {
                let mut f = v[k] as f32;
                if crate::widgets::slider(ui, ["X", "Y"][k], &mut f, parameter.min..=parameter.max, 0).changed() {
                    let mut nv = v;
                    nv[k] = f.round() as i32;
                    set(app, node, parameter.id, json!(nv));
                }
            }
        }
        Value::Bool(b) => {
            let mut v = b;
            let response = ui.checkbox(&mut v, tr(parameter.label));
            if response.changed() {
                set(app, node, parameter.id, json!(v));
            }
            show_menu(app, &response);
        }
        Value::Float3(_) | Value::Float4(_) | Value::Int3(_) | Value::Int4(_) => {
            ui.label(tr(parameter.label));
            let mut value = current.clone();
            if crate::value_editor::show(ui, &mut value) {
                set(app, node, parameter.id, serde_json::to_value(value).unwrap_or_default());
            }
        }
        Value::Color(c) => {
            ui.horizontal(|ui| {
                ui.label(RichText::new(tr(parameter.label)).color(tokens.text));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let mut rgb = [c[0], c[1], c[2]];
                    if crate::widgets::color_swatch(ui, &format!("{}-{}", node.0, parameter.id), &mut rgb, 90.0) {
                        set(app, node, parameter.id, json!([rgb[0], rgb[1], rgb[2], c[3]]));
                    }
                });
            });
        }
        Value::Enum(i) => {
            let current = parameter.options.get(i as usize).copied().unwrap_or("?");
            crate::widgets::combo(ui, tr(parameter.label), tr(current), |ui| {
                for (k, o) in parameter.options.iter().enumerate() {
                    if ui.selectable_label(k as u32 == i, tr(o)).clicked() {
                        set(app, node, parameter.id, json!(k));
                    }
                }
            });
        }
        Value::Text(t) if parameter.resource => {
            let names: Vec<String> = app.session.pkg.resources.iter().map(|response| response.name.clone()).collect();
            crate::widgets::combo(ui, tr(parameter.label), if t.is_empty() { tr("(checker)").to_string() } else { t.clone() }, |ui| {
                for n in &names {
                    if ui.selectable_label(*n == t, n).clicked() {
                        set(app, node, parameter.id, json!(n));
                    }
                }
            });
            if ui.button(format!("{}  {}", icon::IMAGE, tr("Import bitmap..."))).clicked() {
                crate::menus::invoke(app, ui.ctx(), "file.importBitmap");
            }
        }
        Value::Text(t) => {
            ui.horizontal(|ui| {
                ui.label(RichText::new(tr(parameter.label)).color(tokens.text));
                let mut s = t.clone();
                let response = ui.add(egui::TextEdit::singleline(&mut s).desired_width(ui.available_width()));
                crate::widgets::name(&response, egui::Role::TextInput, parameter.label);
                if response.changed() {
                    set(app, node, parameter.id, json!(s));
                }
            });
        }
        Value::Curve(c) => {
            ui.label(RichText::new(tr(parameter.label)).color(tokens.text));
            if let Some(nc) = curve_editor(ui, &format!("curve-{}-{}", node.0, parameter.id), &c) {
                set(app, node, parameter.id, json!(nc.points.iter().map(|q| [q.x, q.y]).collect::<Vec<_>>()));
            }
        }
        Value::Gradient(g) => {
            ui.label(RichText::new(tr(parameter.label)).color(tokens.text));
            if let Some(ng) = gradient_editor(ui, &format!("grad-{}-{}", node.0, parameter.id), &g) {
                set(app, node, parameter.id, typed(&Value::Gradient(ng)));
            }
        }
        Value::PatternGraph(graph) => {
            ui.label(format!("{}: {} nodes", parameter.label, graph.nodes.len()));
        }
        Value::Function(f) => {
            ui.label(RichText::new(tr(parameter.label)).color(tokens.text));
            crate::windows::function_summary(app, ui, node, parameter.id, &f);
        }
    }
}

/// A parameter driven by a function: its label, Edit and Remove
pub(super) fn function_row(app: &mut MaterialCraftApp, ui: &mut Ui, node: NodeId, param: &str, label: &str) {
    let tokens = Tokens::get(ui.ctx());
    ui.horizontal(|ui| {
        ui.label(RichText::new(tr(label)).color(tokens.text));
        ui.label(RichText::new(format!("{} {}", icon::FUNCTION, tr("function"))).color(tokens.accent));
        if ui.small_button(tr("Edit")).clicked() {
            crate::graph::subgraph_navigation::open_function(app, node, crate::function::FunctionTarget::Dynamic(param.to_string()));
        }
        if crate::widgets::icon_button(ui, icon::X, tr("Remove function"), false, 18.0).clicked() {
            app.run("graph.node.function", json!({"id": node.0, "param": param, "remove": true}));
        }
    });
}
