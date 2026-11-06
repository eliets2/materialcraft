use materialcraft_core::Value;

pub(crate) enum EvaluationStep {
    Enter(usize),
    Apply { instruction: usize, base: usize },
    Branch(usize),
    Sequence(usize),
    LoopStart(usize),
    LoopCondition { instruction: usize, iteration: u32, last: Value },
    LoopBody { instruction: usize, iteration: u32 },
}
