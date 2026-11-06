use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::{CoreError, Result, Value};

use crate::{Attributes, Comment, Connection, ExposedParameter, Frame, GraphKind, Node, NodeId, OutputFormat, Pin, Preset};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Graph {
    pub name: String,
    #[serde(default)]
    pub function: Option<crate::FunctionGraph>,
    pub nodes: Vec<Node>,
    pub connections: Vec<Connection>,
    #[serde(default)]
    pub frames: Vec<Frame>,
    #[serde(default)]
    pub comments: Vec<Comment>,
    #[serde(default)]
    pub pins: Vec<Pin>,
    /// log2 of the graph ("parent") size per axis
    pub size: [u32; 2],
    #[serde(default)]
    pub format: OutputFormat,
    #[serde(default)]
    pub seed: i32,
    #[serde(default)]
    pub exposed: Vec<ExposedParameter>,
    /// Current values of the exposed inputs (missing = default)
    #[serde(default)]
    pub inputs: BTreeMap<String, Value>,
    /// Named sets of input values
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub presets: Vec<Preset>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub input_images: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub input_images_linear: BTreeSet<String>,
    #[serde(default)]
    pub attributes: Attributes,
    /// The id the next node, frame, comment or pin gets
    pub next_id: u64,
}

/// Smallest and largest graph size, as log2 (16 px .. 8192 px)
pub const MIN_SIZE_LOG2: u32 = 4;
pub const MAX_SIZE_LOG2: u32 = 13;

impl Graph {
    pub fn new(name: &str) -> Graph {
        Graph {
            name: name.into(),
            function: None,
            nodes: Vec::new(),
            connections: Vec::new(),
            frames: Vec::new(),
            comments: Vec::new(),
            pins: Vec::new(),
            size: [10, 10],
            format: OutputFormat::U16,
            seed: 0,
            exposed: Vec::new(),
            inputs: BTreeMap::new(),
            presets: Vec::new(),
            input_images: BTreeMap::new(),
            input_images_linear: BTreeSet::new(),
            attributes: Attributes::default(),
            next_id: 1,
        }
    }

    pub fn kind(&self) -> GraphKind {
        if self.function.is_some() { GraphKind::Function } else { GraphKind::Material }
    }

    pub fn alloc_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        id
    }

    pub fn node(&self, id: NodeId) -> Result<&Node> {
        self.nodes.iter().find(|n| n.id == id).ok_or(CoreError::NoNode(id.0))
    }

    pub fn node_mut(&mut self, id: NodeId) -> Result<&mut Node> {
        self.nodes.iter_mut().find(|n| n.id == id).ok_or(CoreError::NoNode(id.0))
    }

    /// Add a node of definition `def` at `pos`; returns its id
    pub fn add_node(&mut self, def: &str, pos: [f32; 2]) -> NodeId {
        let id = NodeId(self.alloc_id());
        self.nodes.push(Node::new(id, def, pos));
        id
    }

    /// Remove nodes and every connection touching them
    pub fn remove_nodes(&mut self, ids: &[NodeId]) {
        self.nodes.retain(|n| !ids.contains(&n.id));
        self.connections.retain(|c| !ids.contains(&c.from) && !ids.contains(&c.to));
    }

    pub fn connect(&mut self, from: NodeId, from_port: &str, to: NodeId, to_port: &str) -> Result<()> {
        self.node(from)?;
        self.node(to)?;
        if from == to || self.reaches(to, from) {
            return Err(CoreError::Cycle);
        }
        self.connections.retain(|c| !(c.to == to && c.to_port == to_port));
        self.connections.push(Connection { from, from_port: from_port.into(), to, to_port: to_port.into() });
        Ok(())
    }

    pub fn disconnect(&mut self, to: NodeId, to_port: &str) -> bool {
        let before = self.connections.len();
        self.connections.retain(|c| !(c.to == to && c.to_port == to_port));
        before != self.connections.len()
    }

    /// The connection feeding `to.to_port`
    pub fn input_of(&self, to: NodeId, to_port: &str) -> Option<&Connection> {
        self.connections.iter().find(|c| c.to == to && c.to_port == to_port)
    }

    /// Whether `b` is downstream of (or equal to) `a`
    pub fn reaches(&self, a: NodeId, b: NodeId) -> bool {
        let mut stack = vec![a];
        let mut seen = std::collections::BTreeSet::new();
        while let Some(n) = stack.pop() {
            if n == b {
                return true;
            }
            if !seen.insert(n) {
                continue;
            }
            stack.extend(self.connections.iter().filter(|c| c.from == n).map(|c| c.to));
        }
        false
    }

    pub fn topo_order(&self) -> Vec<NodeId> {
        crate::GraphIndex::new(self).topo_order()
    }

    /// Every node upstream of `id` (not including it)
    pub fn upstream(&self, id: NodeId) -> Vec<NodeId> {
        crate::GraphIndex::new(self).upstream(id)
    }

    /// Every node downstream of `id` (not including it)
    pub fn downstream(&self, id: NodeId) -> Vec<NodeId> {
        crate::GraphIndex::new(self).downstream(id)
    }

    /// The value of exposed input `id` (its default when unset)
    pub fn input_value(&self, id: &str) -> Option<&Value> {
        self.inputs.get(id).or_else(|| self.exposed.iter().find(|e| e.id == id).map(|e| &e.default))
    }

    /// Graph size in pixels
    pub fn size_px(&self) -> [u32; 2] {
        [1 << self.size[0].clamp(MIN_SIZE_LOG2, MAX_SIZE_LOG2), 1 << self.size[1].clamp(MIN_SIZE_LOG2, MAX_SIZE_LOG2)]
    }
}

#[cfg(test)]
mod tests {
    use crate::*;

    #[test]
    fn connect_refuses_cycles_and_replaces_inputs() {
        let mut g = Graph::new("g");
        let a = g.add_node("uniform", [0.0, 0.0]);
        let b = g.add_node("blur", [100.0, 0.0]);
        let c = g.add_node("blur", [200.0, 0.0]);
        g.connect(a, "output", b, "input").unwrap();
        g.connect(b, "output", c, "input").unwrap();
        assert!(matches!(g.connect(c, "output", a, "input"), Err(CoreError::Cycle)));
        assert!(matches!(g.connect(c, "output", c, "input"), Err(CoreError::Cycle)));
        g.connect(a, "output", c, "input").unwrap();
        assert_eq!(g.connections.len(), 2);
        assert_eq!(g.topo_order().first(), Some(&a));
        assert_eq!(g.upstream(c), vec![a]);
        g.remove_nodes(&[a]);
        assert_eq!(g.connections.len(), 0);
    }
}
