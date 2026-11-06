use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

use crate::{PatternNode, PatternNodeKind};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PatternGraph {
    pub nodes: Vec<PatternNode>,
    pub root: Option<u32>,
}

impl PatternGraph {
    pub fn with_root(kind: PatternNodeKind) -> Self {
        Self {
            root: Some(1),
            nodes: vec![PatternNode { id: 1, kind, children: vec![None; kind.child_count()], parameters: BTreeMap::new(), position: [0.0; 2] }],
        }
    }

    pub fn add(&mut self, kind: PatternNodeKind) -> Result<u32, String> {
        if self.nodes.len() >= 4096 {
            return Err("pattern graph exceeds the 4096-node limit".into());
        }

        let mut occupied = [false; 4097];
        for node in &self.nodes {
            if let Some(slot) = occupied.get_mut(node.id.saturating_sub(1) as usize)
                && node.id != 0
            {
                *slot = true;
            }
        }
        let id = occupied.iter().position(|occupied| !occupied).ok_or("no available pattern node id")? as u32 + 1;
        self.nodes.push(PatternNode {
            id,
            kind,
            children: vec![None; kind.child_count()],
            parameters: BTreeMap::new(),
            position: [self.nodes.len() as f32 * 160.0, 0.0],
        });
        self.root.get_or_insert(id);
        Ok(id)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.nodes.len() > 4096 {
            return Err("pattern graph exceeds the 4096-node limit".into());
        }

        let nodes: BTreeMap<_, _> = self.nodes.iter().map(|node| (node.id, node)).collect();
        if nodes.len() != self.nodes.len() {
            return Err("duplicate pattern node ids".into());
        }
        if self.root.is_some_and(|root| !nodes.contains_key(&root)) {
            return Err("missing pattern root".into());
        }

        let mut active = BTreeSet::new();
        let mut finished = BTreeSet::new();
        for root in &self.nodes {
            let mut pending = vec![(root.id, false)];
            while let Some((id, visited)) = pending.pop() {
                if finished.contains(&id) {
                    continue;
                }

                let node = nodes.get(&id).ok_or_else(|| format!("missing pattern node {id}"))?;
                if node.children.len() != node.kind.child_count() {
                    return Err(format!("{} requires {} child slots", node.kind.label(), node.kind.child_count()));
                }
                if visited {
                    active.remove(&id);
                    finished.insert(id);
                    continue;
                }
                if !active.insert(id) {
                    return Err(format!("pattern cycle at node {id}"));
                }

                pending.push((id, true));
                pending.extend(node.children.iter().rev().flatten().map(|child| (*child, false)));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocation_reuses_the_lowest_hole_and_ignores_out_of_range_ids() {
        let mut graph = PatternGraph::with_root(PatternNodeKind::Quadrant);
        let template = graph.nodes[0].clone();
        graph.nodes = [0, 2, 4097, u32::MAX].map(|id| PatternNode { id, ..template.clone() }).to_vec();
        assert_eq!(graph.add(PatternNodeKind::Dot).unwrap(), 1);
        assert_eq!(graph.add(PatternNodeKind::Dot).unwrap(), 3);
        graph.nodes.retain(|node| node.id != 2);
        assert_eq!(graph.add(PatternNodeKind::Dot).unwrap(), 2);
        assert_eq!(graph.root, Some(1));

        graph.nodes.resize(4096, template);
        let before = graph.clone();
        assert!(graph.add(PatternNodeKind::Dot).is_err());
        assert_eq!(graph, before);
    }
}
