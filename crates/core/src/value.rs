use crate::{Curve, CurvePoint, Gradient, GradientKey, parse_color};
use serde::{Deserialize, Serialize};

/// A typed parameter value
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "t", content = "v", rename_all = "camelCase")]
pub enum Value {
    Float(f32),
    Float2([f32; 2]),
    Float3([f32; 3]),
    Float4([f32; 4]),
    Int(i32),
    Int2([i32; 2]),
    Int3([i32; 3]),
    Int4([i32; 4]),
    Bool(bool),
    /// Linear RGBA, 0..1
    Color([f32; 4]),
    /// Index into the parameter's options
    Enum(u32),
    Text(String),
    Curve(Curve),
    Gradient(Gradient),
    /// A function graph (pixel/value processors, dynamic parameters)
    Function(Box<crate::FunctionGraph>),
    PatternGraph(Box<crate::PatternGraph>),
}

impl Value {
    pub fn as_f32(&self) -> Option<f32> {
        match self {
            Value::Float(v) => Some(*v),
            Value::Int(v) => Some(*v as f32),
            Value::Enum(v) => Some(*v as f32),
            Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
            Value::Float2(v) => Some(v[0]),
            Value::Float3(v) => Some(v[0]),
            Value::Float4(v) | Value::Color(v) => Some(v[0]),
            _ => None,
        }
    }

    pub fn as_i32(&self) -> Option<i32> {
        match self {
            Value::Int(v) => Some(*v),
            Value::Enum(v) => i32::try_from(*v).ok(),
            Value::Float(v) if v.is_finite() => Some(v.round() as i32),
            Value::Bool(b) => Some(i32::from(*b)),
            Value::Int2(v) => Some(v[0]),
            Value::Int3(v) => Some(v[0]),
            Value::Int4(v) => Some(v[0]),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            Value::Int(v) => Some(*v != 0),
            Value::Float(v) => Some(*v != 0.0),
            _ => None,
        }
    }

    /// Up to four floats (missing ones are 0; a color keeps its alpha)
    pub fn as_vec4(&self) -> Option<[f32; 4]> {
        match self {
            Value::Float(v) => Some([*v, *v, *v, 1.0]),
            Value::Float2(v) => Some([v[0], v[1], 0.0, 0.0]),
            Value::Float3(v) => Some([v[0], v[1], v[2], 0.0]),
            Value::Float4(v) | Value::Color(v) => Some(*v),
            Value::Int(v) => Some([*v as f32; 4]),
            Value::Int2(v) => Some([v[0] as f32, v[1] as f32, 0.0, 0.0]),
            Value::Int3(v) => Some([v[0] as f32, v[1] as f32, v[2] as f32, 0.0]),
            Value::Int4(v) => Some(v.map(|x| x as f32)),
            _ => None,
        }
    }

    /// Short type name, for errors and inspect output
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Float(_) => "float",
            Value::Float2(_) => "float2",
            Value::Float3(_) => "float3",
            Value::Float4(_) => "float4",
            Value::Int(_) => "int",
            Value::Int2(_) => "int2",
            Value::Int3(_) => "int3",
            Value::Int4(_) => "int4",
            Value::Bool(_) => "bool",
            Value::Color(_) => "color",
            Value::Enum(_) => "enum",
            Value::Text(_) => "text",
            Value::Curve(_) => "curve",
            Value::Gradient(_) => "gradient",
            Value::Function(_) => "function",
            Value::PatternGraph(_) => "patternGraph",
        }
    }

    /// Coerce a JSON value (from a command or MCP call) into the type of `self`
    pub fn coerce_json(&self, j: &serde_json::Value) -> Option<Value> {
        use serde_json::Value as J;
        let f = |v: &J| v.as_f64().map(|x| x as f32).filter(|x| x.is_finite());
        let arr = |n: usize| -> Option<Vec<f32>> {
            match j {
                J::Array(a) if a.len() >= n => a.iter().take(n).map(f).collect(),
                J::Number(_) => f(j).map(|v| vec![v; n]),
                _ => None,
            }
        };
        // A typed value as the API writes it ({"t": .., "v": ..})
        if let Ok(v) = serde_json::from_value::<Value>(j.clone()) {
            if v.type_name() == self.type_name() {
                return Some(v);
            }
            // Another type (a float input bound to an int parameter): coerce its payload
            return j.get("v").and_then(|p| self.coerce_json(p));
        }
        Some(match self {
            Value::Float(_) => Value::Float(f(j)?),
            Value::Float2(_) => {
                let a = arr(2)?;
                Value::Float2([a[0], a[1]])
            }
            Value::Float3(_) => {
                let a = arr(3)?;
                Value::Float3([a[0], a[1], a[2]])
            }
            Value::Float4(_) => {
                let a = arr(4)?;
                Value::Float4([a[0], a[1], a[2], a[3]])
            }
            Value::Color(_) => Value::Color(parse_color(j)?),
            Value::Int(_) => Value::Int(match j.as_i64() {
                Some(value) => i32::try_from(value).ok()?,
                None => {
                    let value = j.as_f64()?.round();
                    if !value.is_finite() || value < i32::MIN as f64 || value > i32::MAX as f64 {
                        return None;
                    }
                    value as i32
                }
            }),
            Value::Int2(_) => Value::Int2(parse_integers(j)?),
            Value::Int3(_) => Value::Int3(parse_integers(j)?),
            Value::Int4(_) => Value::Int4(parse_integers(j)?),
            Value::Bool(_) => Value::Bool(j.as_bool().or_else(|| f(j).map(|v| v != 0.0))?),
            Value::Enum(_) => Value::Enum(u32::try_from(j.as_u64()?).ok()?),
            Value::Text(_) => Value::Text(j.as_str()?.to_string()),
            Value::Curve(_) => {
                let pts: Vec<[f32; 2]> = serde_json::from_value(j.clone()).ok()?;
                let mut points: Vec<CurvePoint> =
                    pts.into_iter().filter(|p| p[0].is_finite() && p[1].is_finite()).map(|p| CurvePoint::new(p[0], p[1])).collect();
                points.sort_by(|a, b| a.x.total_cmp(&b.x));
                Value::Curve(Curve { points })
            }
            Value::Gradient(_) => {
                let keys: Vec<(f32, serde_json::Value)> = serde_json::from_value(j.clone()).ok()?;
                let mut g = Gradient { keys: keys.iter().filter_map(|(p, c)| Some(GradientKey::new(*p, parse_color(c)?))).collect() };
                g.sort();
                Value::Gradient(g)
            }
            Value::Function(_) | Value::PatternGraph(_) => return None,
        })
    }
}

fn parse_integers<const COUNT: usize>(value: &serde_json::Value) -> Option<[i32; COUNT]> {
    let components = value.as_array()?;
    if components.len() != COUNT {
        return None;
    }

    let mut result = [0; COUNT];
    for (target, component) in result.iter_mut().zip(components) {
        *target = i32::try_from(component.as_i64()?).ok()?;
    }

    Some(result)
}
