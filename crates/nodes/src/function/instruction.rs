use super::FunctionDefinition;
use materialcraft_core::Value;

pub(crate) struct Instruction {
    pub id: u32,
    pub definition: &'static FunctionDefinition,
    pub inputs: Vec<usize>,
    pub value: Option<Value>,
    pub type_code: u32,
    pub pure: bool,
}
