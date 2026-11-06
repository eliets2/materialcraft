use crate::{Cook, ParameterDefinition, PortDefinition};

/// A node in the library
#[derive(Clone, Debug)]
pub struct NodeDefinition {
    pub id: &'static str,
    pub label: &'static str,
    /// Library path, e.g. `Filters/Blurs`, `Texture generators/Noises`, `Atomic nodes`
    pub category: &'static str,
    pub description: &'static str,
    pub inputs: Vec<PortDefinition>,
    pub outputs: Vec<PortDefinition>,
    pub params: Vec<ParameterDefinition>,
    /// A generator takes its size from the graph, not from an input
    pub generator: bool,
    /// One of the engine's built-in ("atomic") nodes (shown with a grey header)
    pub atomic: bool,
    pub cook: Cook,
}

impl NodeDefinition {
    pub fn param(&self, id: &str) -> Option<&ParameterDefinition> {
        self.params.iter().find(|p| p.id == id)
    }

    pub fn input_index(&self, id: &str) -> Option<usize> {
        self.inputs.iter().position(|p| p.id == id)
    }

    pub fn output_index(&self, id: &str) -> Option<usize> {
        self.outputs.iter().position(|p| p.id == id)
    }

    /// The last path element of the category ("Blurs")
    pub fn group(&self) -> &'static str {
        self.category.rsplit('/').next().unwrap_or(self.category)
    }
}
