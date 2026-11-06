use egui::Ui;
use materialcraft_engine::core::{Node, ValueType};
use serde_json::json;

use crate::MaterialCraftApp;

pub(crate) fn show(app: &mut MaterialCraftApp, ui: &mut Ui, node: &Node) {
    ui.collapsing("Value inputs", |ui| {
        for (identifier, value_type) in &node.input_values {
            ui.horizontal(|ui| {
                ui.label(materialcraft_engine::eval::value_input_label(identifier));
                ui.label(value_type.label());

                if ui.small_button("Remove").clicked() {
                    app.run("graph.node.value_input", json!({"id": node.id.0, "name": identifier, "remove": true}));
                }
            });
        }

        ui.menu_button("Add value input", |ui| {
            for value_type in ValueType::ALL {
                if ui.button(value_type.label()).clicked() {
                    let name = (1..=65).map(|index| format!("value{index}")).find(|name| !node.input_values.contains_key(name));

                    if let Some(name) = name {
                        app.run("graph.node.value_input", json!({"id": node.id.0, "name": name, "type": value_type.label()}));
                    }

                    ui.close();
                }
            }
        });
    });
}

pub(crate) fn default_value(app: &mut MaterialCraftApp, ui: &mut Ui, node: &Node) {
    use materialcraft_engine::core::Value;
    let mut value = node.params.get("default").cloned().unwrap_or(Value::Float(0.0));
    let mut kind = ValueType::of(&value).unwrap_or(ValueType::Float);
    let before = kind;
    egui::ComboBox::from_label("Value type").selected_text(kind.label()).show_ui(ui, |ui| {
        for candidate in ValueType::ALL {
            ui.selectable_value(&mut kind, candidate, candidate.label());
        }
    });
    let mut changed = kind != before;
    if changed {
        value = kind.default_value();
    }
    changed |= crate::value_editor::show(ui, &mut value);
    if changed {
        app.run("graph.node.param", json!({"id": node.id.0, "param": "default", "value": value}));
    }
}
