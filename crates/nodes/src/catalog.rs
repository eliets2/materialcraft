use crate::NodeDefinition;
use std::sync::OnceLock;

/// Every node definition, in library order
pub fn catalog() -> &'static [NodeDefinition] {
    static CAT: OnceLock<Vec<NodeDefinition>> = OnceLock::new();
    CAT.get_or_init(crate::atomic::defs)
}

pub fn find(id: &str) -> Option<&'static NodeDefinition> {
    catalog().iter().find(|d| d.id == id)
}

/// The library's category tree, in display order: (path, ids)
pub fn categories() -> Vec<(&'static str, Vec<&'static str>)> {
    let mut out: Vec<(&'static str, Vec<&'static str>)> = Vec::new();
    for d in catalog() {
        match out.iter_mut().find(|(c, _)| *c == d.category) {
            Some((_, ids)) => ids.push(d.id),
            None => out.push((d.category, vec![d.id])),
        }
    }
    out
}
