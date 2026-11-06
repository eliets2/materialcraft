#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum FunctionTarget {
    Graph,
    Parameter(String),
    Dynamic(String),
    Pattern { node: u32, parameter: String },
}

impl FunctionTarget {
    pub fn parameter(&self) -> &str {
        match self {
            Self::Graph => "",
            Self::Parameter(parameter) | Self::Dynamic(parameter) | Self::Pattern { parameter, .. } => parameter,
        }
    }
}
