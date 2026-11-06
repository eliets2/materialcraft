use egui::Ui;
use materialcraft_engine::core::{FunctionGraph, Value};

use crate::{MaterialCraftApp, function::FunctionTarget, i18n::tr};

pub(crate) fn title(app: &MaterialCraftApp) -> String {
    let kind = app.pattern_editor.as_ref().and_then(|editor| {
        let graph = editor.current(app)?;
        let inspected = crate::graph::node_selection::inspected(&editor.selection, editor.inspected_node);
        graph.nodes.iter().find(|node| Some(node.id) == inspected).map(|node| node.kind)
    });
    format!("{} - {}", kind.map_or("FX-Map", |kind| kind.label()), tr("Properties"))
}

pub(crate) fn show(app: &mut MaterialCraftApp, ui: &mut Ui) {
    let Some(mut editor) = app.pattern_editor.take() else { return };

    let Some(graph) = editor.current(app) else { return };

    let mut edits = Vec::new();
    let mut function_target = None;

    let inspected = crate::graph::node_selection::inspected(&editor.selection, editor.inspected_node);
    if let Some(node) = graph.nodes.iter().find(|node| Some(node.id) == inspected) {
        ui.heading(tr(node.kind.label()));
        if graph.root == Some(node.id) {
            ui.label(tr("Root node"));
        }
        if let Some(definition) = materialcraft_engine::nodes::pattern::definition(node.kind) {
            for parameter in &definition.params {
                ui.push_id(parameter.id, |ui| {
                    ui.label(tr(parameter.label));
                    ui.horizontal_wrapped(|ui| {
                        let value = node.parameters.get(parameter.id).unwrap_or(&parameter.default);
                        if matches!(value, Value::Function(_)) {
                            if ui.small_button(tr("Edit function")).clicked() {
                                function_target = Some(FunctionTarget::Pattern { node: node.id, parameter: parameter.id.to_string() });
                            }
                            if ui.small_button(tr("Reset")).clicked() {
                                edits.push((materialcraft_engine::PatternEdit::Value { id: node.id, parameter: parameter.id.into(), value: None }, false));
                            }
                        } else {
                            let mut value = value.clone();
                            if super::parameter_value::show(ui, parameter.id, &mut value) {
                                edits.push((
                                    materialcraft_engine::PatternEdit::Value { id: node.id, parameter: parameter.id.into(), value: Some(value.clone()) },
                                    true,
                                ));
                            }
                            if ui.small_button(tr("Function")).clicked() {
                                edits.push((
                                    materialcraft_engine::PatternEdit::Value {
                                        id: node.id,
                                        parameter: parameter.id.into(),
                                        value: Some(Value::Function(Box::new(FunctionGraph::constant(value)))),
                                    },
                                    false,
                                ));
                                function_target = Some(FunctionTarget::Pattern { node: node.id, parameter: parameter.id.to_string() });
                            }
                        }
                    });
                    ui.add_space(6.0);
                });
            }
        }
    } else {
        ui.heading(tr("FX-Map"));
        ui.label(tr("Select a node to edit its parameters"));
    }
    for (edit, merge) in edits {
        editor.edit(app, edit, merge);
    }
    if let Some(target) = function_target {
        crate::graph::subgraph_navigation::open_function(app, editor.node, target);
    }
    app.pattern_editor = Some(editor);
}
