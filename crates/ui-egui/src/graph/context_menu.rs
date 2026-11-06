use egui::{Pos2, Ui};
use materialcraft_engine::core::NodeId;
use serde_json::{Value, json};

use crate::{MaterialCraftApp, i18n::tr, menus};

pub(super) fn show(app: &mut MaterialCraftApp, ui: &mut Ui, at: Pos2, node: Option<NodeId>) {
    if let Some(node) = node
        && app.session.selection.last() != Some(&node)
    {
        let mut selection = app.session.selection.clone();
        if selection.contains(&node) {
            selection.retain(|selected| *selected != node);
        } else {
            selection.clear();
        }
        selection.push(node);
        app.run("graph.select", json!({"ids": selection}));
    }

    ui.set_min_width(290.0);
    if node.is_some() && menus::enabled(app, "graph.node.edit") {
        action(app, ui, "Edit graph", "graph.node.edit", Value::Null);
        ui.separator();
    }

    ui.menu_button(tr("Add node"), |ui| add_nodes(app, ui, at));
    action(app, ui, "Add Comment", "graph.comment.add", json!({"at": [at.x, at.y]}));
    action(app, ui, "Add Frame", "graph.frame.add", json!({"rect": [at.x, at.y, 400.0, 260.0]}));
    action(app, ui, "Add Navigation pin", "graph.pin.add", json!({"at": [at.x, at.y]}));
    action(app, ui, "Add Dot node", "graph.node.add", json!({"def": "dot", "at": [at.x, at.y]}));
    ui.separator();

    if let Some(node) = node {
        if let Some(parameters) = crate::material_preview_menu::show(ui, &app.session, app.session.graph, node) {
            app.run("view.3d.assign", parameters);
        }
    } else {
        action(app, ui, "View outputs in 3D view", "ui.graph.viewOutputs", Value::Null);
        action(app, ui, "Reset and view outputs in 3D view", "ui.graph.resetViewOutputs", Value::Null);
    }
    outputs(app, ui, node);
    action(app, ui, "Compute nodes thumbnails", "ui.graph.computeThumbnails", Value::Null);
    action(app, ui, "Clear nodes thumbnails", "ui.graph.clearThumbnails", Value::Null);
    action(app, ui, "Save package", "file.save", Value::Null);
    if let Some(command) = super::edit_menu::selection(ui, |command| menus::enabled(app, command)) {
        let parameters = if matches!(command, "edit.paste" | "edit.pasteWithoutLinks") { json!({"at": [at.x, at.y]}) } else { Value::Null };
        app.run(command, parameters);
    }
    action(app, ui, "Previous pin", "ui.graph.previousPin", Value::Null);
    action(app, ui, "Next pin", "ui.graph.nextPin", Value::Null);
}

fn add_nodes(app: &mut MaterialCraftApp, ui: &mut Ui, at: Pos2) {
    let definitions = materialcraft_engine::nodes::catalog().iter().filter(|definition| definition.id != "dot");
    let entries = definitions.map(|definition| (definition.id, definition.label, definition.category));
    if let Some(definition) = super::add_node_menu::show(ui, entries) {
        app.run("graph.node.add", json!({"def": definition, "at": [at.x, at.y], "snapToGrid": app.ui.prefs.snap_to_grid}));
    }
}

fn outputs(app: &mut MaterialCraftApp, ui: &mut Ui, node: Option<NodeId>) {
    let targets: Vec<_> = app
        .session
        .graph()
        .into_iter()
        .flat_map(|graph| graph.nodes.iter().map(move |node| (graph, node)))
        .filter(|(_, candidate)| node == Some(candidate.id) || (node.is_none() && candidate.def == "output"))
        .flat_map(|(graph, node)| {
            if node.def == "output" {
                return vec![(node.id, 0, super::node_label(node))];
            }

            materialcraft_engine::eval::ports(&app.session.pkg, graph, node)
                .1
                .into_iter()
                .enumerate()
                .map(move |(index, port)| (node.id, index, format!("{} / {}", super::node_label(node), port.label)))
                .collect()
        })
        .collect();

    ui.add_enabled_ui(!targets.is_empty(), |ui| {
        ui.menu_button(tr("View output in 2D view"), |ui| {
            for (node, port, label) in targets {
                if ui.button(label).clicked() {
                    app.run("view.2d", json!({"node": node.0, "port": port}));
                    app.ui.dock.focus(crate::docking::Panel::View2d);
                    ui.close();
                }
            }
        });
    });
}

fn action(app: &mut MaterialCraftApp, ui: &mut Ui, label: &str, command: &str, parameters: Value) {
    if super::edit_menu::button(ui, label, command, menus::enabled(app, command)) {
        if command.starts_with("ui.") || matches!(command, "file.save" | "graph.node.edit") {
            menus::invoke(app, ui.ctx(), command);
        } else {
            app.run(command, parameters);
        }

        ui.close();
    }
}
