use super::type_code;
use super::xml::{format_float, parse_floats, parse_ints, xml_escape};
use materialcraft_core::{Value, linear_to_srgb, srgb_to_linear};
use roxmltree::Node as XmlNode;

#[derive(Clone, Debug, PartialEq)]
pub(super) enum SbsValue {
    Floats(Vec<f32>),
    Ints(Vec<i32>),
    Bool(bool),
    Text(String),
}

impl SbsValue {
    pub(super) fn float(&self, i: usize) -> f32 {
        match self {
            SbsValue::Floats(v) => v.get(i).or(v.first()).copied().unwrap_or(0.0),
            SbsValue::Ints(v) => v.get(i).or(v.first()).copied().unwrap_or(0) as f32,
            SbsValue::Bool(b) => f32::from(u8::from(*b)),
            SbsValue::Text(_) => 0.0,
        }
    }
    pub(super) fn int(&self, i: usize) -> i32 {
        match self {
            SbsValue::Ints(v) => v.get(i).or(v.first()).copied().unwrap_or(0),
            SbsValue::Floats(v) => v.get(i).or(v.first()).copied().unwrap_or(0.0).round() as i32,
            SbsValue::Bool(b) => i32::from(*b),
            SbsValue::Text(_) => 0,
        }
    }

    pub(super) fn truthy(&self) -> bool {
        match self {
            SbsValue::Bool(b) => *b,
            SbsValue::Text(s) => s == "1" || s.eq_ignore_ascii_case("true"),
            _ => self.float(0) != 0.0,
        }
    }

    pub(super) fn component_count(&self) -> usize {
        match self {
            SbsValue::Floats(v) => v.len(),
            SbsValue::Ints(v) => v.len(),
            _ => 1,
        }
    }
}
pub(super) fn sbs_constant(holder: XmlNode) -> Option<SbsValue> {
    sbs_constant_element(holder.children().find(|child| child.is_element())?)
}

pub(super) fn sbs_constant_element(element: XmlNode) -> Option<SbsValue> {
    let text = element.attribute("v").unwrap_or("");
    let tag = element.tag_name().name();

    if tag == "constantValueBool" {
        Some(SbsValue::Bool(parse_ints(text).first().is_some_and(|b| *b != 0) || text.trim().eq_ignore_ascii_case("true")))
    } else if tag == "constantValueString" {
        Some(SbsValue::Text(text.to_string()))
    } else if tag.starts_with("constantValueFloat") {
        Some(SbsValue::Floats(parse_floats(text)))
    } else if tag.starts_with("constantValueInt") {
        Some(SbsValue::Ints(parse_ints(text)))
    } else {
        None
    }
}
pub(super) fn to_param_value(like: &Value, value: &SbsValue, options: usize) -> Option<Value> {
    let f = |i| value.float(i);

    Some(match like {
        Value::Float(_) => Value::Float(f(0)),
        Value::Float2(_) => Value::Float2([f(0), f(1)]),
        Value::Float3(_) => Value::Float3([f(0), f(1), f(2)]),
        Value::Float4(_) => Value::Float4([f(0), f(1), f(2), f(3)]),
        // SBS colors are stored as displayed (sRGB); MaterialCraft's are linear
        Value::Color(_) if value.component_count() >= 3 => {
            let alpha = if value.component_count() >= 4 { f(3) } else { 1.0 };

            Value::Color([srgb_to_linear(f(0)), srgb_to_linear(f(1)), srgb_to_linear(f(2)), alpha])
        }
        Value::Color(_) => {
            let gray = srgb_to_linear(f(0));
            Value::Color([gray, gray, gray, 1.0])
        }
        Value::Int(_) => Value::Int(value.int(0)),
        Value::Int2(_) => Value::Int2([value.int(0), value.int(1)]),
        Value::Int3(_) => Value::Int3([value.int(0), value.int(1), value.int(2)]),
        Value::Int4(_) => Value::Int4([value.int(0), value.int(1), value.int(2), value.int(3)]),
        Value::Bool(_) => Value::Bool(value.truthy()),
        Value::Enum(_) => {
            let last = options.saturating_sub(1).min(i32::MAX as usize) as i32;
            Value::Enum(value.int(0).clamp(0, last) as u32)
        }
        Value::Text(_) => match value {
            SbsValue::Text(s) => Value::Text(s.clone()),
            _ => return None,
        },
        Value::Curve(_) | Value::Gradient(_) | Value::Function(_) | Value::PatternGraph(_) => return None,
    })
}
pub(super) fn to_natural_value(value: &SbsValue) -> Value {
    let f = |i| value.float(i);

    match value {
        SbsValue::Bool(b) => Value::Bool(*b),
        SbsValue::Text(s) => Value::Text(s.clone()),
        SbsValue::Ints(v) if v.len() <= 1 => Value::Int(value.int(0)),
        SbsValue::Ints(v) if v.len() == 2 => Value::Int2([value.int(0), value.int(1)]),
        SbsValue::Ints(v) if v.len() == 3 => Value::Int3([value.int(0), value.int(1), value.int(2)]),
        SbsValue::Ints(_) => Value::Int4([value.int(0), value.int(1), value.int(2), value.int(3)]),
        SbsValue::Floats(v) => match v.len() {
            0 | 1 => Value::Float(f(0)),
            2 => Value::Float2([f(0), f(1)]),
            3 => Value::Float3([f(0), f(1), f(2)]),
            _ => Value::Float4([f(0), f(1), f(2), f(3)]),
        },
    }
}
pub(super) fn exposed_default(code: i64, value: Option<&SbsValue>) -> Value {
    use type_code::*;

    let value = value.cloned().unwrap_or(SbsValue::Floats(vec![0.0]));
    let f = |i| value.float(i);
    match code {
        BOOL => Value::Bool(value.truthy()),
        INT1 => Value::Int(value.int(0)),
        INT2 => Value::Int2([value.int(0), value.int(1)]),
        INT3 => Value::Int3([value.int(0), value.int(1), value.int(2)]),
        INT4 => Value::Int4([value.int(0), value.int(1), value.int(2), value.int(3)]),
        FLOAT1 => Value::Float(f(0)),
        FLOAT2 => Value::Float2([f(0), f(1)]),
        FLOAT3 => Value::Float3([f(0), f(1), f(2)]),
        FLOAT4 => Value::Float4([f(0), f(1), f(2), f(3)]),
        STRING => match &value {
            SbsValue::Text(s) => Value::Text(s.clone()),
            _ => Value::Text(String::new()),
        },
        _ => to_natural_value(&value),
    }
}

pub(super) fn exposed_type_code(value: &Value) -> i64 {
    materialcraft_core::ValueType::of(value).map_or(type_code::FLOAT1, |kind| i64::from(kind as u32))
}

pub(super) fn constant_xml(value: &Value) -> Option<String> {
    let join = |xs: &[f32]| xs.iter().map(|x| format_float(*x)).collect::<Vec<_>>().join(" ");
    let (tag, text) = match value {
        Value::Float(x) => ("constantValueFloat1", format_float(*x)),
        Value::Float2(x) => ("constantValueFloat2", join(x)),
        Value::Float3(x) => ("constantValueFloat3", join(x)),
        Value::Float4(x) => ("constantValueFloat4", join(x)),
        Value::Color(c) => ("constantValueFloat4", join(&[linear_to_srgb(c[0]), linear_to_srgb(c[1]), linear_to_srgb(c[2]), c[3]])),
        Value::Int(i) => ("constantValueInt32", i.to_string()),
        Value::Int2(i) => ("constantValueInt2", format!("{} {}", i[0], i[1])),
        Value::Int3(i) => ("constantValueInt3", format!("{} {} {}", i[0], i[1], i[2])),
        Value::Int4(i) => ("constantValueInt4", format!("{} {} {} {}", i[0], i[1], i[2], i[3])),
        Value::Enum(e) => ("constantValueInt32", e.to_string()),
        Value::Bool(b) => ("constantValueBool", u8::from(*b).to_string()),
        Value::Text(s) => ("constantValueString", s.clone()),
        Value::Curve(_) | Value::Gradient(_) | Value::Function(_) | Value::PatternGraph(_) => return None,
    };

    Some(format!("<{tag} v=\"{}\"/>", xml_escape(&text)))
}
