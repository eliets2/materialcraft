use crate::{MaterialCraftApp, i18n::tr};
use egui::Ui;
use materialcraft_engine::FunctionEdit;

pub(crate) fn show(app: &mut MaterialCraftApp, ui: &mut Ui) {
    let Some(target) = app.function_editor.as_ref().and_then(|editor| editor.command_target()) else { return };

    let operations = target.available_operations(&app.session);
    ui.add(egui::TextEdit::singleline(&mut app.ui.library_search).hint_text(tr("Search functions")));
    let search = app.ui.library_search.to_ascii_lowercase();
    egui::ScrollArea::vertical().show(ui, |ui| {
        for definition in operations.filter(|definition| definition.label.to_ascii_lowercase().contains(&search)) {
            if ui.selectable_label(false, tr(definition.label)).double_clicked()
                && let Some(mut editor) = app.function_editor.take()
            {
                let at = editor.current(app).map_or([0.0; 2], |graph| [graph.nodes.len() as f32 * 140.0, 0.0]);
                editor.edit(app, FunctionEdit::Add { operation: definition.id.into(), at, snap: app.ui.prefs.snap_to_grid }, false);
                app.function_editor = Some(editor);
            }
        }
    });
}
