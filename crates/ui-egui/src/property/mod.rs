mod curve;
mod gradient;
mod graph;
mod input;
mod node;
mod parameter;
mod section;

pub use curve::curve_editor;
pub use gradient::gradient_editor;
pub(crate) use section::section;

use crate::{MaterialCraftApp, graph::editor_target::EditorTarget, i18n::tr};
use egui::Ui;

/// The panel title: "<node> - Properties" or "<graph> - Properties"
pub fn title(app: &MaterialCraftApp) -> String {
    match EditorTarget::resolve(app) {
        EditorTarget::Pattern => return crate::pattern::properties::title(app),
        EditorTarget::Function if !EditorTarget::annotation_active(app) => {
            return crate::function::properties::title(app).unwrap_or_else(|| format!("{} - {}", tr("Function Graph"), tr("Properties")));
        }
        _ => {}
    }

    if let Some(selection) = app.session.annotation_selection {
        let name = match selection {
            materialcraft_engine::AnnotationSelection::Frame(_) => tr("Frame"),
            materialcraft_engine::AnnotationSelection::Comment(_) => tr("Comment"),
        };
        return format!("{name} - {}", tr("Properties"));
    }

    let sel = crate::graph::node_selection::inspected(&app.session.selection, app.graph.inspected_node)
        .and_then(|id| app.session.graph().ok()?.node(id).ok().map(crate::graph::node_label));
    let name = sel.or_else(|| app.session.graph().ok().map(|g| g.name.clone())).unwrap_or_default();
    format!("{name} - {}", tr("Properties"))
}

pub fn show(app: &mut MaterialCraftApp, ui: &mut Ui) {
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.set_width(ui.available_width());
        match EditorTarget::resolve(app) {
            EditorTarget::Pattern => {
                crate::pattern::properties::show(app, ui);
                return;
            }
            EditorTarget::Function if !EditorTarget::annotation_active(app) => {
                if !crate::function::properties::show(app, ui) {
                    ui.label(tr("Select a node"));
                }
                return;
            }
            _ => {}
        }

        if let Some(selection) = app.session.annotation_selection {
            ui.push_id((app.session.graph, format!("{selection:?}")), |ui| crate::annotation::show(app, ui, selection));
            return;
        }

        let node = crate::graph::node_selection::inspected(&app.session.selection, app.graph.inspected_node)
            .and_then(|id| app.session.graph().ok()?.node(id).ok().cloned());
        ui.push_id((app.session.graph, node.as_ref().map(|n| n.id)), |ui| match node {
            Some(n) => node::show(app, ui, &n),
            None => graph::show(app, ui),
        });
    });
}
