use materialcraft_core::{NodeId, Package};
use serde::{Deserialize, Serialize};

/// An image assigned to a material channel, independent of editor selection
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MaterialSource {
    pub graph: usize,
    pub node: NodeId,
    pub port: String,
}

impl MaterialSource {
    pub fn is_image(&self, package: &Package) -> bool {
        let mut graph_index = self.graph;
        let mut node_id = self.node;
        let mut port = self.port.clone();
        let mut visited = std::collections::BTreeSet::new();
        loop {
            if !visited.insert((graph_index, node_id, port.clone())) {
                return false;
            }
            let Some(graph) = package.graphs.get(graph_index) else { return false };
            let Ok(node) = graph.node(node_id) else { return false };

            match node.def.as_str() {
                "output" | "dot" => {
                    let Some(connection) = graph.input_of(node_id, "input") else { return true };

                    node_id = connection.from;
                    port = connection.from_port.clone();
                }
                "graph_instance" => {
                    let Some((index, graph)) = materialcraft_eval::instance_graph(package, node) else { return false };
                    let Some(output) = graph.nodes.iter().find(|node| node.def == "output" && materialcraft_eval::identifier(node) == port) else {
                        return false;
                    };

                    graph_index = index;
                    node_id = output.id;
                    port = "output".into();
                }
                _ => {
                    return materialcraft_eval::ports(package, graph, node)
                        .1
                        .iter()
                        .any(|output| output.id == port && output.kind != materialcraft_nodes::PortKind::Value)
                        && materialcraft_nodes::output_value_type(graph, node_id).is_ok_and(|value| value.is_none());
                }
            }
        }
    }

    pub fn port_index(&self, package: &Package) -> Option<usize> {
        let graph = package.graphs.get(self.graph)?;
        let node = graph.node(self.node).ok()?;
        if node.def == "output" {
            return (self.port == "output").then_some(0);
        }

        materialcraft_eval::ports(package, graph, node).1.iter().position(|port| port.id == self.port)
    }
}
