/// One native function primitive and its ordered input connectors
#[derive(Clone, Copy, Debug)]
pub struct FunctionDefinition {
    pub id: &'static str,
    pub label: &'static str,
    pub inputs: &'static [&'static str],
    pub category: &'static str,
}
