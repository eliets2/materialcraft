use crate::{MaterialCraftApp, i18n::tr};
use egui::Ui;
use serde_json::json;

pub(super) fn show(app: &mut MaterialCraftApp, ui: &mut Ui, id: u64) {
    let Some(mut frame) = app.session.graph().ok().and_then(|graph| graph.frames.iter().find(|frame| frame.id == id)).cloned() else {
        return;
    };

    crate::properties::section(ui, "frame-parameters", tr("Parameters"), |ui| {
        if ui.checkbox(&mut frame.title_visible, tr("Title Visible")).changed() {
            app.run("graph.frame.edit", json!({"id": id, "titleVisible": frame.title_visible}));
        }

        ui.label(tr("Title"));
        let title = ui.text_edit_singleline(&mut frame.title);
        crate::widgets::name(&title, egui::Role::TextInput, "Frame title");
        if title.changed() {
            app.run("graph.frame.edit", json!({"id": id, "title": frame.title}));
        }

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.label(tr("Frame Color"));
            if ui.color_edit_button_rgba_unmultiplied(&mut frame.color).changed() {
                app.run("graph.frame.edit", json!({"id": id, "color": frame.color}));
            }
        });

        ui.add_space(8.0);
        ui.label(tr("Description"));
        let description = ui.add(egui::TextEdit::multiline(&mut frame.description).desired_rows(8).desired_width(f32::INFINITY));
        crate::widgets::name(&description, egui::Role::TextInput, "Frame description");
        if description.changed() {
            app.run("graph.frame.edit", json!({"id": id, "description": frame.description}));
        }
    });
}
