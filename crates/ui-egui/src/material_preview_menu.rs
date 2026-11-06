use egui::Ui;
use materialcraft_engine::{
    MaterialSource, Session,
    core::{MaterialChannel, NodeId},
};
use serde_json::{Value, json};

use crate::i18n::tr;

/// Shared node/output assignment menu for the canvas and Explorer
pub(crate) fn show(ui: &mut Ui, session: &Session, graph_index: usize, node_id: NodeId) -> Option<Value> {
    let graph = session.pkg.graphs.get(graph_index)?;
    let node = graph.node(node_id).ok()?;
    let ports = if node.def == "output" {
        vec![("output".to_string(), materialcraft_engine::eval::identifier(node))]
    } else {
        materialcraft_engine::eval::ports(&session.pkg, graph, node).1.into_iter().map(|port| (port.id, port.label)).collect()
    };
    let ports: Vec<_> =
        ports.into_iter().filter(|(port, _)| MaterialSource { graph: graph_index, node: node_id, port: port.clone() }.is_image(&session.pkg)).collect();
    let mut action = None;
    ui.add_enabled_ui(!ports.is_empty(), |ui| {
        for (label, reset) in [("View in 3D view", false), ("Reset and view in 3D view", true)] {
            ui.menu_button(tr(label), |ui| {
                for (port, label) in &ports {
                    let mut channels = |ui: &mut Ui| {
                        for channel in MaterialChannel::ALL {
                            if ui.button(tr(channel.label())).clicked() {
                                action = Some(json!({"graph": graph_index, "node": node_id.0, "port": port, "channel": channel.id(), "reset": reset}));
                                ui.close();
                            }
                        }
                    };
                    if ports.len() == 1 {
                        channels(ui);
                    } else {
                        ui.menu_button(label, channels);
                    }
                }
            });
        }
    });
    action
}
