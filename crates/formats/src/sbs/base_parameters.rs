use super::sbs_value::sbs_constant;
use super::xml::{child_element, child_value, children_named};
use materialcraft_core::{BaseParameters, MAX_SIZE_LOG2, OutputFormat, SizeMode};
use roxmltree::Node as XmlNode;
use std::fmt::Write as _;

pub(super) fn read_base_params(params: Option<XmlNode>) -> BaseParameters {
    let mut base = BaseParameters::default();
    for param in params.into_iter().flat_map(|p| children_named(p, "parameter")) {
        let relative_to = child_value(param, "relativeTo").and_then(|r| r.trim().parse::<i32>().ok()).unwrap_or(1);
        let Some(value) = child_element(param, "paramValue").and_then(sbs_constant) else { continue };

        match child_value(param, "name") {
            Some("outputsize") => {
                base.size_mode = match relative_to {
                    0 => SizeMode::Absolute,
                    2 => SizeMode::RelativeToParent,
                    _ => SizeMode::RelativeToInput,
                };

                let limit = |log2: i32| log2.clamp(-(MAX_SIZE_LOG2 as i32), MAX_SIZE_LOG2 as i32);
                base.size = [limit(value.int(0)), limit(value.int(1))];
            }
            Some("format") if relative_to == 0 => {
                base.format = match value.int(0) {
                    0 => OutputFormat::U8,
                    1 => OutputFormat::U16,
                    2 => OutputFormat::F16,
                    3 => OutputFormat::F32,
                    _ => OutputFormat::Inherit,
                }
            }
            Some("randomseed") if relative_to == 0 => base.seed = value.int(0),
            Some("tiling") if relative_to == 0 => base.tiling = [value.int(0) & 1 != 0, value.int(0) & 2 != 0],
            _ => {}
        }
    }
    base
}

pub(super) fn base_params_xml(base: &BaseParameters) -> String {
    let mut out = String::new();

    if base.size_mode != SizeMode::RelativeToInput || base.size != [0, 0] {
        let relative_to = match base.size_mode {
            SizeMode::Absolute => 0,
            SizeMode::RelativeToInput => 1,
            SizeMode::RelativeToParent => 2,
        };

        let [width, height] = base.size;
        let _ = write!(
            out,
            "<parameter><name v=\"outputsize\"/><relativeTo v=\"{relative_to}\"/><paramValue><constantValueInt2 v=\"{width} {height}\"/></paramValue></parameter>"
        );
    }

    let format = match base.format {
        OutputFormat::Inherit => None,
        OutputFormat::U8 => Some(0),
        OutputFormat::U16 => Some(1),
        OutputFormat::F16 => Some(2),
        OutputFormat::F32 => Some(3),
    };

    if let Some(format) = format {
        let _ = write!(out, "<parameter><name v=\"format\"/><relativeTo v=\"0\"/><paramValue><constantValueInt32 v=\"{format}\"/></paramValue></parameter>");
    }

    if base.seed != 0 {
        let _ = write!(
            out,
            "<parameter><name v=\"randomseed\"/><relativeTo v=\"0\"/><paramValue><constantValueInt32 v=\"{}\"/></paramValue></parameter>",
            base.seed
        );
    }
    if base.tiling != [true, true] {
        let code = i32::from(base.tiling[0]) | (i32::from(base.tiling[1]) << 1);
        out.push_str(&super::node_parameter::param_xml("tiling", &materialcraft_core::Value::Int(code)).unwrap_or_default());
    }
    out
}
