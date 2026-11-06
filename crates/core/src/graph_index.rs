use std::collections::{BTreeMap, BTreeSet};

use crate::{Connection, Graph, Node, NodeId};

/// Borrowed lookups for one graph operation, rebuilt when the graph changes
pub struct GraphIndex<'a> {
    nodes: BTreeMap<NodeId, &'a Node>,
    incoming: BTreeMap<NodeId, Vec<&'a Connection>>,
    outgoing: BTreeMap<NodeId, Vec<&'a Connection>>,
}

impl<'a> GraphIndex<'a> {
    pub fn new(graph: &'a Graph) -> Self {
        let mut nodes = BTreeMap::new();
        for node in &graph.nodes {
            nodes.entry(node.id).or_insert(node);
        }

        let mut incoming: BTreeMap<_, Vec<_>> = BTreeMap::new();
        let mut outgoing: BTreeMap<_, Vec<_>> = BTreeMap::new();
        for connection in &graph.connections {
            incoming.entry(connection.to).or_default().push(connection);
            outgoing.entry(connection.from).or_default().push(connection);
        }

        Self { nodes, incoming, outgoing }
    }

    pub fn node(&self, id: NodeId) -> Option<&'a Node> {
        self.nodes.get(&id).copied()
    }

    pub fn incoming(&self, id: NodeId) -> &[&'a Connection] {
        self.incoming.get(&id).map_or(&[], Vec::as_slice)
    }

    pub fn input_of(&self, id: NodeId, port: &str) -> Option<&'a Connection> {
        self.incoming(id).iter().copied().find(|connection| connection.to_port == port)
    }

    pub fn topo_order(&self) -> Vec<NodeId> {
        let mut degrees: BTreeMap<_, _> = self.nodes.keys().map(|id| (*id, 0usize)).collect();
        for (id, connections) in &self.incoming {
            if let Some(degree) = degrees.get_mut(id) {
                *degree = connections.iter().filter(|connection| self.nodes.contains_key(&connection.from)).count();
            }
        }

        let mut ready: Vec<_> = degrees.iter().filter(|(_, degree)| **degree == 0).map(|(id, _)| *id).collect();
        let mut ordered = Vec::with_capacity(self.nodes.len());
        while let Some(id) = ready.pop() {
            ordered.push(id);
            for connection in self.outgoing.get(&id).into_iter().flatten() {
                if let Some(degree) = degrees.get_mut(&connection.to) {
                    *degree = degree.saturating_sub(1);
                    if *degree == 0 {
                        ready.push(connection.to);
                    }
                }
            }
        }

        ordered
    }

    pub fn upstream(&self, id: NodeId) -> Vec<NodeId> {
        self.connected(id, true)
    }

    pub fn downstream(&self, id: NodeId) -> Vec<NodeId> {
        self.connected(id, false)
    }

    pub fn required_nodes(&self, roots: &[NodeId]) -> BTreeSet<NodeId> {
        let mut required: BTreeSet<_> = roots.iter().copied().collect();
        let mut pending = roots.to_vec();
        while let Some(id) = pending.pop() {
            for connection in self.incoming(id) {
                if required.insert(connection.from) {
                    pending.push(connection.from);
                }
            }
        }

        required
    }

    fn connected(&self, root: NodeId, upstream: bool) -> Vec<NodeId> {
        let adjacency = if upstream { &self.incoming } else { &self.outgoing };

        let mut visited = BTreeSet::new();
        let mut connected = Vec::new();
        let mut pending = vec![root];
        while let Some(id) = pending.pop() {
            for connection in adjacency.get(&id).into_iter().flatten() {
                let next = if upstream { connection.from } else { connection.to };

                if visited.insert(next) {
                    connected.push(next);
                    pending.push(next);
                }
            }
        }

        connected
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn traversal_preserves_document_order_and_dangling_edge_behavior() {
        let mut graph = Graph::new("Traversal");
        let first = graph.add_node("uniform", [0.0; 2]);
        let second = graph.add_node("dot", [0.0; 2]);
        let third = graph.add_node("blend", [0.0; 2]);
        let fourth = graph.add_node("dot", [0.0; 2]);
        for (from, to, port) in [(first, second, "input"), (first, third, "foreground"), (second, third, "background"), (NodeId(99), fourth, "input")] {
            graph.connections.push(Connection { from, from_port: "output".into(), to, to_port: port.into() });
        }
        let index = GraphIndex::new(&graph);
        assert_eq!(index.topo_order(), [fourth, first, second, third]);
        assert_eq!(index.upstream(third), [first, second]);
        assert_eq!(index.downstream(first), [second, third]);
        assert_eq!(index.required_nodes(&[third, fourth]), BTreeSet::from([first, second, third, fourth, NodeId(99)]));
        assert_eq!(index.input_of(third, "background").unwrap().from, second);

        graph.connections.push(Connection { from: third, from_port: "output".into(), to: first, to_port: "input".into() });
        assert_eq!(GraphIndex::new(&graph).topo_order(), [fourth]);
    }
}
