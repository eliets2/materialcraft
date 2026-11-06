use super::{
    input::input_editor,
    parameter::{function_row, param_row},
    section,
};
use crate::{MaterialCraftApp, i18n::tr, theme::Tokens};
use egui::{Rect, RichText, Ui, vec2};
use egui_phosphor::regular as icon;
use materialcraft_engine::core::{OutputFormat, SizeMode};
use serde_json::json;

pub(super) fn show(app: &mut MaterialCraftApp, ui: &mut Ui, node: &materialcraft_engine::core::Node) {
    let tokens = Tokens::get(ui.ctx());
    let definition = materialcraft_engine::nodes::find(&node.def);
    // Header: label (renamable) and description
    ui.horizontal(|ui| {
        ui.painter().rect_filled(Rect::from_min_size(ui.cursor().min + vec2(0.0, 2.0), vec2(4.0, 16.0)), 1.0, crate::graph::header_color(&node.def));
        ui.add_space(8.0);
        let mut label = if node.label.is_empty() { crate::graph::node_label(node) } else { node.label.clone() };
        let response = ui.add(egui::TextEdit::singleline(&mut label).desired_width(ui.available_width() - 4.0).font(egui::TextStyle::Heading));
        crate::widgets::name(&response, egui::Role::TextInput, "Node label");
        if response.changed() {
            app.run("graph.node.rename", json!({"id": node.id.0, "label": label}));
        }
    });
    if let Some(definition) = definition
        && !definition.description.is_empty()
    {
        ui.label(RichText::new(definition.description).size(11.0).color(tokens.text_dim));
    }
    if let Some(response) = app.session.cooked.nodes.get(&node.id)
        && let Some(entry) = &response.error
    {
        ui.label(RichText::new(format!("{}  {entry}", icon::WARNING)).color(tokens.danger));
    }
    ui.add_space(4.0);
    section(ui, "base", tr("Base parameters"), |ui| base_params(app, ui, node));
    crate::value_input_editor::show(app, ui, node);
    if node.def == "input_value" {
        crate::value_input_editor::default_value(app, ui, node);
    }
    if node.def == "graph_instance" {
        section(ui, "instance", tr("Instance parameters"), |ui| instance_params(app, ui, node));
        return;
    }
    let Some(definition) = definition else { return };

    if definition.params.is_empty() {
        return;
    }
    section(ui, "specific", tr("Specific parameters"), |ui| {
        if node.def == "levels" {
            crate::levels_editor::show(app, ui, node);
            ui.add_space(6.0);
        }
        let exposed: Vec<(String, String)> =
            app.session.graph().map(|g| g.exposed.iter().map(|entry| (entry.id.clone(), entry.label.clone())).collect()).unwrap_or_default();
        for parameter in &definition.params {
            if node.def == "input_value" && parameter.id == "default" {
                continue;
            }
            let current = node.params.get(parameter.id).cloned().unwrap_or_else(|| parameter.default.clone());
            if node.functions.contains_key(parameter.id) {
                function_row(app, ui, node.id, parameter.id, parameter.label);
                continue;
            }
            let bound = node.bindings.get(parameter.id).cloned();
            param_row(app, ui, node.id, parameter, current, bound, &exposed);
        }
    });
}

fn base_params(app: &mut MaterialCraftApp, ui: &mut Ui, node: &materialcraft_engine::core::Node) {
    let base = &node.base;
    let id = node.id.0;
    let modes = [(SizeMode::RelativeToInput, "Relative to input"), (SizeMode::RelativeToParent, "Relative to parent"), (SizeMode::Absolute, "Absolute")];
    let current = modes.iter().find(|mode| mode.0 == base.size_mode).map_or("", |mode| mode.1);
    crate::widgets::combo(ui, tr("Output size"), tr(current), |ui| {
        for (mode, label) in modes {
            if ui.selectable_label(base.size_mode == mode, tr(label)).clicked() {
                let key = match mode {
                    SizeMode::RelativeToInput => "relativeToInput",
                    SizeMode::RelativeToParent => "relativeToParent",
                    SizeMode::Absolute => "absolute",
                };

                let size = if mode == SizeMode::Absolute { json!([10, 10]) } else { json!([0, 0]) };
                app.run("graph.node.base", json!({"id": id, "sizeMode": key, "size": size}));
            }
        }
    });
    let (minimum, maximum) = if base.size_mode == SizeMode::Absolute { (4.0, 13.0) } else { (-6.0, 6.0) };
    for (axis, label) in [(0usize, "Width"), (1, "Height")] {
        let mut value = base.size[axis] as f32;
        let caption = if base.size_mode == SizeMode::Absolute {
            format!("{label}  {} px", 1u32 << base.size[axis].clamp(4, 13))
        } else {
            format!("{label}  x {}", 2f32.powi(base.size[axis]))
        };

        if crate::widgets::slider(ui, &caption, &mut value, minimum..=maximum, 0).changed() {
            let mut size = base.size;
            size[axis] = value.round() as i32;
            app.run("graph.node.base", json!({"id": id, "size": size}));
        }
    }
    crate::widgets::combo(ui, tr("Output format"), tr(base.format.label()), |ui| {
        for format in OutputFormat::ALL {
            if ui.selectable_label(base.format == format, tr(format.label())).clicked() {
                let key = match format {
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
    let mut seed = base.seed as f32;
    if crate::widgets::slider(ui, tr("Random seed"), &mut seed, 0.0..=1000.0, 0).changed() {
        app.run("graph.node.base", json!({"id": id, "seed": seed.round() as i64}));
    }
    if ui.button(format!("{}  {}", icon::SHUFFLE, tr("New random seed"))).clicked() {
        let seed = (crate::now_ms() as u64 ^ id).wrapping_mul(2654435761) % 1000;
        app.run("graph.node.base", json!({"id": id, "seed": seed}));
    }
    let mut tiling = base.tiling[0] && base.tiling[1];
    if ui.checkbox(&mut tiling, tr("Tiling (H and V)")).changed() {
        app.run("graph.node.base", json!({"id": id, "tiling": tiling}));
    }
}

fn instance_params(app: &mut MaterialCraftApp, ui: &mut Ui, node: &materialcraft_engine::core::Node) {
    let tokens = Tokens::get(ui.ctx());
    let Some((graph_index, subgraph)) = materialcraft_engine::eval::instance_graph(&app.session.pkg, node) else {
        ui.label(RichText::new(tr("The instanced graph is missing")).color(tokens.danger));
        return;
    };

    let exposed = subgraph.exposed.clone();
    if ui.button(format!("{}  {}", icon::ARROW_SQUARE_OUT, tr("Open graph"))).clicked() {
        crate::graph::subgraph_navigation::open_instance(app, graph_index);
        return;
    }
    if exposed.is_empty() {
        ui.label(RichText::new(tr("This graph exposes no inputs")).color(tokens.text_dim));
    }
    for entry in exposed {
        let current = node.params.get(&entry.id).cloned().unwrap_or_else(|| entry.default.clone());
        if let Some(value) = input_editor(ui, &entry, &current) {
            app.run("graph.node.param", json!({"id": node.id.0, "param": entry.id, "value": value}));
        }
    }
}
