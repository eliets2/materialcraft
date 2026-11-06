#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod atomic;
pub mod catalog;
pub mod function;
pub mod ops;
pub mod pattern;
mod raster;

mod cook_context;
mod image_input;
mod node_definition;
mod node_definition_builder;
mod output_value_type;
mod parameter;
mod parameter_definition;
mod port_definition;
mod port_kind;
mod random;

pub use catalog::{catalog, categories, find};
pub use cook_context::CookContext;
pub use image_input::{MAX_IMAGE_INPUTS, image_input_prefix};
pub use node_definition::NodeDefinition;
pub(crate) use node_definition_builder::node;
pub use output_value_type::output_value_type;
pub use parameter::{clamp_params, resolve_params};
pub use parameter_definition::ParameterDefinition;
pub use port_definition::PortDefinition;
pub use port_kind::PortKind;

pub type CookResult = Result<Vec<materialcraft_image::Image>, String>;
pub type Cook = fn(&CookContext<'_>) -> CookResult;

#[cfg(test)]
mod tests;
