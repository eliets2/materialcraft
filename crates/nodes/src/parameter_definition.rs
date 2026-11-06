use materialcraft_core::Value;

/// The editor widget and range of a parameter
#[derive(Clone, Debug)]
pub struct ParameterDefinition {
    pub id: &'static str,
    pub label: &'static str,
    pub default: Value,
    pub min: f32,
    pub max: f32,
    /// Labels of an enum parameter
    pub options: &'static [&'static str],
    /// Section heading in the Properties panel ("" = none)
    pub group: &'static str,
    /// An angle in turns (shown in degrees)
    pub angle: bool,
    /// The name of a package resource (bitmap / SVG)
    pub resource: bool,
}
