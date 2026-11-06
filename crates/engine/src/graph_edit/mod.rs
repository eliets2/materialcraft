//! Editing rules shared by Function Graphs and FX-Map graphs

mod editable_graph;
mod function_graph;
mod pattern_graph;
mod selection;

pub use editable_graph::EditableGraph;
pub use selection::{enabled, read, remove, run};
