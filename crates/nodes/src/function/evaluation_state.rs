use super::evaluation_step::EvaluationStep;
use materialcraft_core::Value;
use std::collections::BTreeMap;

/// Reusable execution storage for one worker
#[derive(Default)]
pub struct EvaluationState {
    pub(crate) steps: Vec<EvaluationStep>,
    pub(crate) values: Vec<Value>,
    pub(crate) registers: Vec<Option<Value>>,
    pub(crate) variables: BTreeMap<String, Value>,
    pub(crate) random: u32,
}

impl EvaluationState {
    pub fn variables(&self) -> &BTreeMap<String, Value> {
        &self.variables
    }
}
