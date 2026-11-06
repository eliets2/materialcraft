/// The data access needed by shared function and pattern editing operations
pub trait EditableGraph {
    type Node: Clone;

    const LINKS_ARE_INPUTS: bool;
    const RELINK: bool;

    fn nodes(&self) -> &[Self::Node];
    fn nodes_mut(&mut self) -> &mut Vec<Self::Node>;
    fn id(node: &Self::Node) -> u32;
    fn position(node: &mut Self::Node) -> &mut [f32; 2];
    fn links(node: &Self::Node) -> &[Option<u32>];
    fn links_mut(node: &mut Self::Node) -> &mut [Option<u32>];
    fn result(&mut self) -> &mut Option<u32>;
    fn check_capacity(&self, additional: usize) -> Result<(), String>;
    fn insert(&mut self, node: Self::Node) -> Result<u32, String>;
}
