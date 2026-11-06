use crate::{MaterialCraftApp, i18n::tr};
use egui::Ui;
use serde_json::json;

pub(super) fn show(app: &mut MaterialCraftApp, ui: &mut Ui, id: u64) {
    let Some(mut text) = app.session.graph().ok().and_then(|graph| graph.comments.iter().find(|comment| comment.id == id)).map(|comment| comment.text.clone())
    else {
        return;
    };

    crate::properties::section(ui, "comment-parameters", tr("Parameters"), |ui| {
        ui.label(tr("Description"));
        let description = ui.add(egui::TextEdit::multiline(&mut text).desired_rows(8).desired_width(f32::INFINITY));
        crate::widgets::name(&description, egui::Role::TextInput, "Comment description");
        if description.changed() {
            app.run("graph.comment.edit", json!({"id": id, "text": text}));
        }
    });
}
