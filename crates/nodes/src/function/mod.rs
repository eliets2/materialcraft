//! Native function primitives, compilation and bounded evaluation

mod arithmetic;
mod catalog;
mod compile;
mod evaluation_context;
mod evaluation_state;
mod evaluation_step;
mod function_definition;
mod inferred_type;
mod instruction;
mod node_types;
mod operation;
mod processor;
mod program;
pub mod value_type;
mod vector;

pub use catalog::{FUNCTIONS, default_value, definition};
pub use compile::compile;
pub use evaluation_context::EvaluationContext;
pub use evaluation_state::EvaluationState;
pub use function_definition::FunctionDefinition;
pub use node_types::node_types;
pub use processor::default_pixel_function;
pub(crate) use processor::pixel_processor;
pub use program::Program;

#[cfg(test)]
mod tests;
