mod comment_properties;
mod frame_properties;

use crate::MaterialCraftApp;
use egui::Ui;
use materialcraft_engine::AnnotationSelection;

pub(crate) fn show(app: &mut MaterialCraftApp, ui: &mut Ui, selection: AnnotationSelection) {
    match selection {
        AnnotationSelection::Frame(id) => frame_properties::show(app, ui, id),
        AnnotationSelection::Comment(id) => comment_properties::show(app, ui, id),
    }
}
