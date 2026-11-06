use super::EditableGraph;
use materialcraft_core::{PatternGraph, PatternNode};

impl EditableGraph for PatternGraph {
    type Node = PatternNode;

    const LINKS_ARE_INPUTS: bool = false;
    const RELINK: bool = false;

    fn nodes(&self) -> &[PatternNode] {
        &self.nodes
    }
    fn nodes_mut(&mut self) -> &mut Vec<PatternNode> {
        &mut self.nodes
    }
    fn id(node: &PatternNode) -> u32 {
        node.id
    }
    fn position(node: &mut PatternNode) -> &mut [f32; 2] {
        &mut node.position
    }
    fn links(node: &PatternNode) -> &[Option<u32>] {
        &node.children
    }
    fn links_mut(node: &mut PatternNode) -> &mut [Option<u32>] {
        &mut node.children
    }
    fn result(&mut self) -> &mut Option<u32> {
        &mut self.root
    }

    fn check_capacity(&self, additional: usize) -> Result<(), String> {
        if self.nodes.len().saturating_add(additional) > 4096 {
            return Err("pattern graph exceeds the 4096-node limit".into());
        }
        Ok(())
    }

    fn insert(&mut self, mut node: PatternNode) -> Result<u32, String> {
        let id = self.add(node.kind)?;
        node.id = id;
        if let Some(destination) = self.nodes.last_mut() {
            *destination = node;
        }
        Ok(id)
    }
}
