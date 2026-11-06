use crate::{MaterialCraftApp, i18n::tr};
use egui::Ui;

const SIZES: [u32; 6] = [256, 512, 1024, 2048, 4096, 0];

pub(crate) fn label(size: u32) -> String {
    if size == 0 { tr("Full size").to_string() } else { format!("{size} px") }
}

pub(crate) fn choices(ui: &mut Ui, size: &mut u32) -> bool {
    let mut changed = false;
    for option in SIZES {
        changed |= ui.selectable_value(size, option, label(option)).changed();
    }
    changed
}

pub(crate) fn show(app: &mut MaterialCraftApp, ui: &mut Ui) {
    let mut size = app.session.preview_size.unwrap_or(0);
    let response = egui::ComboBox::from_id_salt("preview_resolution")
        .selected_text(label(size))
        .width(85.0)
        .show_ui(ui, |ui| {
            if choices(ui, &mut size) {
                app.run("view.previewSize", serde_json::json!({"size": size}));
            }
        })
        .response;
    crate::widgets::name(&response, egui::Role::ComboBox, "Preview resolution");
    response.on_hover_text(tr("Preview size while editing"));
}
