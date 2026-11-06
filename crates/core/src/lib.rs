#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod attributes;
mod base_parameters;
mod color;
mod comment;
mod component_usage;
mod connection;
mod core_error;
mod curve;
mod curve_point;
mod exposed_parameter;
mod frame;
mod function_graph;
mod function_node;
mod gradient;
mod gradient_key;
mod graph;
mod graph_index;
mod graph_kind;
mod material_channel;
mod missing_node;
mod node;
mod node_id;
mod output_format;
mod package;
mod pattern_graph;
mod pattern_node;
mod pattern_node_kind;
mod pin;
mod preset;
mod resource;
mod resource_bytes;
mod size_mode;
mod value;
mod value_type;

pub use attributes::Attributes;
pub use base_parameters::BaseParameters;
pub use color::{linear_to_srgb, parse_color, srgb_to_linear};
pub use comment::Comment;
pub use component_usage::ComponentUsage;
pub use connection::Connection;
pub use core_error::CoreError;
pub use curve::Curve;
pub use curve_point::CurvePoint;
pub use exposed_parameter::ExposedParameter;
pub use frame::Frame;
pub use function_graph::FunctionGraph;
pub use function_node::FunctionNode;
pub use gradient::Gradient;
pub use gradient_key::GradientKey;
pub use graph::{Graph, MAX_SIZE_LOG2, MIN_SIZE_LOG2};
pub use graph_index::GraphIndex;
pub use graph_kind::GraphKind;
pub use material_channel::MaterialChannel;
pub use missing_node::{MISSING_NODE, missing_ports};
pub use node::Node;
pub use node_id::NodeId;
pub use output_format::OutputFormat;
pub use package::{PACKAGE_EXT, Package};
pub use pattern_graph::PatternGraph;
pub use pattern_node::PatternNode;
pub use pattern_node_kind::PatternNodeKind;
pub use pin::Pin;
pub use preset::Preset;
pub use resource::Resource;
pub use resource_bytes::ResourceBytes;
pub use size_mode::SizeMode;
pub use value::Value;
pub use value_type::ValueType;

pub type Result<T> = std::result::Result<T, CoreError>;

#[cfg(test)]
mod value_tests;
