use materialcraft_core::{Curve, CurvePoint, Gradient, GradientKey, Node, Value};
use roxmltree::Node as XmlNode;
use std::collections::BTreeMap;
use std::fmt::Write;

use super::node_parameter::param_xml;
use super::sbs_value::{sbs_constant, to_natural_value};
use super::sbs_writer::SbsWriter;
use super::xml::{child_element, child_value, children_named};

pub(super) fn read(node: &mut Node, implementation: XmlNode<'_, '_>) {
    for array in child_element(implementation, "paramsArrays").into_iter().flat_map(|arrays| children_named(arrays, "paramsArray")) {
        let mut curve = Curve { points: Vec::new() };
        let mut gradient = Gradient { keys: Vec::new() };
        for cell in child_element(array, "paramsArrayCells").into_iter().flat_map(|cells| children_named(cells, "paramsArrayCell")).take(4096) {
            let parameters: BTreeMap<_, _> = child_element(cell, "parameters")
                .into_iter()
                .flat_map(|parameters| children_named(parameters, "parameter"))
                .filter_map(|parameter| Some((child_value(parameter, "name")?, to_natural_value(&sbs_constant(child_element(parameter, "paramValue")?)?))))
                .collect();
            let vector = |name| parameters.get(name).and_then(Value::as_vec4).map(|value| [value[0], value[1]]);
            match child_value(array, "name") {
                Some("curveluminance") => {
                    let position = vector("position").unwrap_or([0.0; 2]);
                    curve.points.push(CurvePoint { x: position[0], y: position[1], left: vector("left"), right: vector("right") });
                }
                Some("gradientrgba" | "gradientluminance") => {
                    let position = parameters.get("position").and_then(Value::as_f32).unwrap_or(0.0);
                    let color = match parameters.get("value") {
                        Some(Value::Float(gray)) => [*gray, *gray, *gray, 1.0],
                        Some(value) => value.as_vec4().unwrap_or([0.0; 4]),
                        None => [0.0; 4],
                    };

                    let midpoint = parameters.get("midpoint").and_then(Value::as_f32).unwrap_or(0.5);
                    gradient.keys.push(GradientKey { pos: position, color, midpoint });
                }
                _ => {}
            }
        }
        if !curve.points.is_empty() {
            curve.points.sort_by(|left, right| left.x.total_cmp(&right.x));
            node.params.insert("curve".into(), Value::Curve(curve));
        }
        if !gradient.keys.is_empty() {
            gradient.sort();
            node.params.insert("gradient".into(), Value::Gradient(gradient));
        }
    }
}

pub(super) fn write(value: &Value, writer: &mut SbsWriter) -> String {
    let name = match value {
        Value::Curve(_) => "curveluminance",
        Value::Gradient(_) => "gradientrgba",
        _ => return String::new(),
    };

    let mut output = format!("<paramsArrays><paramsArray><name v=\"{name}\"/><uid v=\"{}\"/><paramsArrayCells>", writer.next_uid());
    let mut cell = |parameters: Vec<(&str, Value)>| {
        let _ = write!(output, "<paramsArrayCell><uid v=\"{}\"/><parameters>", writer.next_uid());
        for (name, value) in parameters {
            output.push_str(&param_xml(name, &value).unwrap_or_default());
        }
        output.push_str("</parameters></paramsArrayCell>");
    };

    match value {
        Value::Curve(curve) => {
            for (index, point) in curve.points.iter().enumerate() {
                let position = [point.x, point.y];
                let left = index.checked_sub(1).and_then(|index| curve.segment(index)).map_or(position, |segment| segment[2]);
                let right = curve.segment(index).map_or(position, |segment| segment[1]);
                cell(vec![
                    ("position", Value::Float2(position)),
                    ("left", Value::Float2(left)),
                    ("right", Value::Float2(right)),
                    ("isLeftBroken", Value::Bool(true)),
                    ("isRightBroken", Value::Bool(true)),
                    ("isLocked", Value::Bool(false)),
                ]);
            }
        }
        Value::Gradient(gradient) => {
            for key in &gradient.keys {
                cell(vec![("position", Value::Float(key.pos)), ("midpoint", Value::Float(key.midpoint)), ("value", Value::Float4(key.color))]);
            }
        }
        _ => {}
    }
    output.push_str("</paramsArrayCells></paramsArray></paramsArrays>");
    output
}
