use super::EditableGraph;
use materialcraft_core::{FunctionGraph, FunctionNode};

impl EditableGraph for FunctionGraph {
    type Node = FunctionNode;

    const LINKS_ARE_INPUTS: bool = true;
    const RELINK: bool = true;

    fn nodes(&self) -> &[FunctionNode] {
        &self.nodes
    }
    fn nodes_mut(&mut self) -> &mut Vec<FunctionNode> {
        &mut self.nodes
    }
    fn id(node: &FunctionNode) -> u32 {
        node.id
    }
    fn position(node: &mut FunctionNode) -> &mut [f32; 2] {
        &mut node.pos
    }
    fn links(node: &FunctionNode) -> &[Option<u32>] {
        &node.inputs
    }
    fn links_mut(node: &mut FunctionNode) -> &mut [Option<u32>] {
        &mut node.inputs
    }
    fn result(&mut self) -> &mut Option<u32> {
        &mut self.output
    }

    fn check_capacity(&self, additional: usize) -> Result<(), String> {
        if self.nodes.len().saturating_add(additional) > 4096 {
            return Err("function exceeds the 4096-node limit".into());
        }
        Ok(())
    }

    fn insert(&mut self, mut node: FunctionNode) -> Result<u32, String> {
        let id = self.next_id();
        node.id = id;
        self.nodes.push(node);
        Ok(id)
    }
}
