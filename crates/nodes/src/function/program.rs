use super::{EvaluationContext, EvaluationState, evaluation_step::EvaluationStep, instruction::Instruction, operation, value_type};
use materialcraft_core::Value;

/// Validated graph with input ids resolved to instruction indices
pub struct Program {
    pub(crate) instructions: Vec<Instruction>,
    pub(crate) output: usize,
    pub(crate) seed: u32,
}

const MAX_EVALUATION_STEPS: usize = 100_000;

impl Program {
    pub fn output_type(&self) -> u32 {
        self.instructions.get(self.output).map_or(0, |instruction| instruction.type_code)
    }

    pub fn node_type(&self, id: u32) -> Option<u32> {
        self.instructions.iter().find(|instruction| instruction.id == id).map(|instruction| instruction.type_code)
    }

    fn instruction(&self, index: usize) -> Result<&Instruction, String> {
        self.instructions.get(index).ok_or_else(|| "invalid function instruction".into())
    }

    fn input(&self, index: usize, slot: usize) -> Result<usize, String> {
        self.instruction(index)?.inputs.get(slot).copied().ok_or_else(|| "missing function input".into())
    }

    pub fn evaluate(&self, context: &EvaluationContext<'_>, state: &mut EvaluationState) -> Result<Value, String> {
        state.variables.clone_from(context.variables);
        self.run(context, state)
    }

    // Pixel assignments are local; only the general evaluator exports the complete environment
    pub(super) fn evaluate_pixel(&self, context: &EvaluationContext<'_>, state: &mut EvaluationState) -> Result<Value, String> {
        state.variables.clear();
        self.run(context, state)
    }

    fn run(&self, context: &EvaluationContext<'_>, state: &mut EvaluationState) -> Result<Value, String> {
        use EvaluationStep::*;

        state.steps.clear();
        state.values.clear();
        state.registers.clear();
        state.registers.resize(self.instructions.len(), None);
        state.random = context.variables.get("$randomseed").and_then(Value::as_i32).map_or(self.seed, |seed| seed as u32);
        state.steps.push(Enter(self.output));
        let mut remaining_steps = MAX_EVALUATION_STEPS;

        while let Some(step) = state.steps.pop() {
            remaining_steps = remaining_steps.checked_sub(1).ok_or("function exceeded its execution budget; check loop termination")?;
            match step {
                Enter(index) => {
                    let instruction = self.instruction(index)?;
                    if let Some(Some(value)) = state.registers.get(index) {
                        state.values.push(value.clone());
                        continue;
                    }

                    match instruction.definition.id {
                        "ifelse" => {
                            state.steps.push(Branch(index));
                            state.steps.push(Enter(self.input(index, 0)?));
                        }
                        "sequence" => {
                            state.steps.push(Sequence(index));
                            state.steps.push(Enter(self.input(index, 0)?));
                        }
                        "while" => {
                            state.steps.push(LoopStart(index));
                            state.steps.push(Enter(self.input(index, 0)?));
                        }
                        _ => {
                            state.steps.push(Apply { instruction: index, base: state.values.len() });
                            state.steps.extend(instruction.inputs.iter().rev().map(|index| Enter(*index)));
                        }
                    }
                }
                Apply { instruction: index, base } => {
                    let instruction = self.instruction(index)?;
                    let arguments = state.values.get(base..).ok_or("invalid function operands")?;
                    let result = operation::evaluate(instruction, arguments, &mut state.variables, &mut state.random, context)
                        .map_err(|error| format!("{} ({}): {error}", instruction.definition.label, instruction.id))?;

                    state.values.truncate(base);
                    if instruction.pure
                        && let Some(register) = state.registers.get_mut(index)
                    {
                        *register = Some(result.clone());
                    }

                    state.values.push(result);
                }
                Branch(index) => {
                    let condition = state.values.pop().ok_or("missing branch condition")?;
                    let slot = if value_type::boolean(&condition)? { 1 } else { 2 };

                    state.steps.push(Enter(self.input(index, slot)?));
                }
                Sequence(index) => {
                    state.values.pop().ok_or("missing sequence result")?;
                    state.steps.push(Enter(self.input(index, 1)?));
                }
                LoopStart(index) => {
                    let last = state.values.pop().ok_or("missing loop initialization")?;
                    state.steps.push(LoopCondition { instruction: index, iteration: 0, last });
                    state.steps.push(Enter(self.input(index, 1)?));
                }
                LoopCondition { instruction: index, iteration, last } => {
                    let exit = value_type::boolean(&state.values.pop().ok_or("missing loop condition")?)?;
                    let maximum = self.instruction(index)?.value.as_ref().and_then(Value::as_i32).unwrap_or(100);
                    if exit || (maximum >= 0 && iteration >= maximum as u32) {
                        state.values.push(last);
                    } else {
                        state.steps.push(LoopBody { instruction: index, iteration: iteration.saturating_add(1) });
                        state.steps.push(Enter(self.input(index, 2)?));
                    }
                }
                LoopBody { instruction: index, iteration } => {
                    let last = state.values.pop().ok_or("missing loop body result")?;
                    state.steps.push(LoopCondition { instruction: index, iteration, last });
                    state.steps.push(Enter(self.input(index, 1)?));
                }
            }
        }

        let result = state.values.pop().ok_or("function returned no value")?;
        let expected = self.instruction(self.output)?.type_code;
        if value_type::type_code(&result) != expected {
            return Err(format!("function returned {}, expected native type {expected}", result.type_name()));
        }

        Ok(result)
    }
}
