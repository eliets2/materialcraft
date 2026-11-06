use super::{input::input_editor, section};
use crate::{MaterialCraftApp, i18n::tr, theme::Tokens};
use egui::{RichText, Ui};
use egui_phosphor::regular as icon;
use materialcraft_engine::core::{Graph, OutputFormat, Resource, Value};
use serde_json::{Value as JsonValue, json};

pub(super) fn show(app: &mut MaterialCraftApp, ui: &mut Ui) {
    let tokens = Tokens::get(ui.ctx());
    let Ok(graph) = app.session.graph() else {
        ui.label(tr("No graph is open"));
        return;
    };

    let mut commands = Vec::new();
    let mut emit = |id: &'static str, parameters| commands.push((id, parameters));
    section(ui, "graph-base", tr("Base parameters"), |ui| {
        ui.horizontal(|ui| {
            ui.label(tr("Name"));
            let mut name = graph.name.clone();
            let response = ui.add(egui::TextEdit::singleline(&mut name).desired_width(ui.available_width()));
            crate::widgets::name(&response, egui::Role::TextInput, "Graph name");
            if response.lost_focus() && name != graph.name {
                emit("graph.rename", json!({"name": name}));
            }
        });
        if graph.function.is_some() {
            return;
        }

        for (axis, label) in [(0usize, "Width"), (1, "Height")] {
            let mut value = graph.size[axis] as f32;
            if crate::widgets::slider(ui, &format!("{}  {} px", tr(label), 1u32 << graph.size[axis]), &mut value, 4.0..=13.0, 0).changed() {
                let mut edited = graph.size;
                edited[axis] = value.round() as u32;
                emit("graph.size", json!({"log2": edited}));
            }
        }
        crate::widgets::combo(ui, tr("Output format"), tr(graph.format.label()), |ui| {
            for (format, key) in [(OutputFormat::U8, "u8"), (OutputFormat::U16, "u16"), (OutputFormat::F16, "f16"), (OutputFormat::F32, "f32")] {
                if ui.selectable_label(graph.format == format, tr(format.label())).clicked() {
                    emit("graph.format", json!({"format": key}));
                }
            }
        });
        let mut seed = graph.seed as f32;
        if crate::widgets::slider(ui, tr("Random seed"), &mut seed, 0.0..=1000.0, 0).changed() {
            emit("graph.seed", json!({"seed": seed.round() as i64}));
        }
    });
    section(ui, "graph-attr", tr("Attributes"), |ui| {
        for (key, label, current) in [
            ("category", "Category", &graph.attributes.category),
            ("description", "Description", &graph.attributes.description),
            ("author", "Author", &graph.attributes.author),
            ("tags", "Tags", &graph.attributes.tags),
        ] {
            ui.horizontal(|ui| {
                ui.label(tr(label));
                let mut edited = current.clone();
                let response = ui.add(egui::TextEdit::singleline(&mut edited).desired_width(ui.available_width()));
                crate::widgets::name(&response, egui::Role::TextInput, label);
                if response.lost_focus() && edited != *current {
                    emit("graph.attributes", json!({key: edited}));
                }
            });
        }
    });
    section(ui, "graph-inputs", tr("Input parameters"), |ui| {
        if graph.exposed.is_empty() {
            ui.label(RichText::new(tr("Right-click a node parameter > Expose as graph input")).size(11.0).color(tokens.text_dim));
        } else {
            presets_bar(ui, graph, &mut emit);
        }
        let mut groups: Vec<&str> = Vec::new();
        for exposed in &graph.exposed {
            if !groups.contains(&exposed.group.as_str()) {
                groups.push(&exposed.group);
            }
        }
        let mut last_group: Option<&str> = None;
        let mut order: Vec<&materialcraft_engine::core::ExposedParameter> = Vec::new();
        for group in &groups {
            order.extend(graph.exposed.iter().filter(|exposed| exposed.group == *group));
        }
        for exposed in order {
            if last_group != Some(exposed.group.as_str()) && !exposed.group.is_empty() {
                ui.label(RichText::new(exposed.group.to_uppercase()).size(10.5).strong().color(tokens.text_dim));
            }
            last_group = Some(exposed.group.as_str());
            let current = graph.input_value(&exposed.id).cloned().unwrap_or_else(|| exposed.default.clone());
            ui.horizontal(|ui| {
                if crate::widgets::icon_button(ui, icon::X, tr("Delete input"), false, 18.0).clicked() {
                    emit("graph.exposed.delete", json!({"id": exposed.id}));
                }
                ui.vertical(|ui| {
                    if let Some(value) = input_editor(ui, exposed, &current) {
                        emit("graph.exposed.set", json!({"id": exposed.id, "value": value}));
                    }
                });
            });
        }
    });
    input_previews(ui, graph, &app.session.pkg.resources, &mut emit);
    section(ui, "graph-outputs", tr("Outputs"), |ui| {
        for node in graph.nodes.iter().filter(|node| node.def == "output") {
            let usage = node.params.get("usage").and_then(Value::as_i32).unwrap_or(0) as usize;
            let label = format!(
                "{}  {} | {}",
                icon::SIGN_OUT,
                materialcraft_engine::core::ComponentUsage::from_index(usage as u32).label,
                materialcraft_engine::eval::identifier(node)
            );
            if ui.add(egui::Button::selectable(app.session.view2d.node == Some(node.id), label)).clicked() {
                emit("view.2d", json!({"node": node.id.0}));
            }
        }
    });

    for (id, parameters) in commands {
        app.run(id, parameters);
    }
}

/// Preview images of the graph's input nodes: a bitmap resource, or (desktop) an image file such as a mesh map baked in TextureCraft
fn input_previews(ui: &mut Ui, graph: &Graph, resources: &[Resource], emit: &mut impl FnMut(&'static str, JsonValue)) {
    let inputs = materialcraft_engine::material_cmds::image_inputs(graph);
    if inputs.is_empty() {
        return;
    }
    section(ui, "graph-input-images", tr("Input previews"), |ui| {
        for input in &inputs {
            let id = &input.identifier;
            let bound = graph.input_images.get(id);
            ui.horizontal(|ui| {
                ui.label(id);
                egui::ComboBox::from_id_salt(("input-preview", id)).width(120.0).selected_text(bound.map_or(tr("None"), String::as_str)).show_ui(ui, |ui| {
                    if ui.selectable_label(bound.is_none(), tr("None")).clicked() {
                        emit("graph.input.unbind", json!({"identifier": id}));
                    }
                    for resource in resources {
                        if ui.selectable_label(bound == Some(&resource.name), &resource.name).clicked() {
                            emit("graph.input.bind", json!({"identifier": id, "resource": resource.name}));
                        }
                    }
                });
                #[cfg(not(target_arch = "wasm32"))]
                if crate::widgets::icon_button(ui, icon::FOLDER_OPEN, tr("Pick an image file (a mesh map baked in TextureCraft)"), false, 18.0).clicked()
                    && let Some(path) = rfd::FileDialog::new().add_filter("Images", materialcraft_engine::formats::IMAGE_EXTENSIONS).pick_file()
                {
                    emit("graph.input.bind", json!({"identifier": id, "path": path.display().to_string()}));
                }
            });
        }
    });
}

/// Presets: apply one, save the current inputs under a name, delete one
fn presets_bar(ui: &mut Ui, graph: &Graph, emit: &mut impl FnMut(&'static str, JsonValue)) {
    let name_id = ui.make_persistent_id("preset-name");
    let mut name: String = ui.data(|data| data.get_temp(name_id)).unwrap_or_default();
    ui.horizontal(|ui| {
        let response = egui::ComboBox::from_id_salt("presets").width(110.0).selected_text(tr("Presets")).show_ui(ui, |ui| {
            for preset in &graph.presets {
                if ui.button(&preset.name).clicked() {
                    emit("graph.preset.apply", json!({"name": preset.name}));
                }
            }
            if graph.presets.is_empty() {
                ui.label(tr("No presets yet"));
            }
        });
        crate::widgets::name(&response.response, egui::Role::ComboBox, "Presets");
        let text_response = ui.add(egui::TextEdit::singleline(&mut name).hint_text(tr("Preset name")).desired_width(90.0));
        crate::widgets::name(&text_response, egui::Role::TextInput, "Preset name");
        if crate::widgets::icon_button(ui, icon::FLOPPY_DISK, tr("Save preset"), false, 20.0).clicked() && !name.trim().is_empty() {
            emit("graph.preset.save", json!({"name": name}));
        }
        if crate::widgets::icon_button(ui, icon::X, tr("Delete preset"), false, 20.0).clicked() && !name.trim().is_empty() {
            emit("graph.preset.delete", json!({"name": name}));
        }
    });
    ui.data_mut(|data| data.insert_temp(name_id, name));
}
