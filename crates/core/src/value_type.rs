use crate::Value;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u32)]
pub enum ValueType {
    Boolean = 4,
    Integer = 16,
    Integer2 = 32,
    Integer3 = 64,
    Integer4 = 128,
    Float = 256,
    Float2 = 512,
    Float3 = 1024,
    Float4 = 2048,
    String = 16384,
}

impl ValueType {
    pub const ALL: [Self; 10] =
        [Self::Float, Self::Float2, Self::Float3, Self::Float4, Self::Integer, Self::Integer2, Self::Integer3, Self::Integer4, Self::Boolean, Self::String];

    pub fn from_code(code: u32) -> Option<Self> {
        Self::ALL.into_iter().find(|value_type| *value_type as u32 == code)
    }

    pub fn of(value: &Value) -> Option<Self> {
        Some(match value {
            Value::Bool(_) => Self::Boolean,
            Value::Int(_) | Value::Enum(_) => Self::Integer,
            Value::Int2(_) => Self::Integer2,
            Value::Int3(_) => Self::Integer3,
            Value::Int4(_) => Self::Integer4,
            Value::Float(_) => Self::Float,
            Value::Float2(_) => Self::Float2,
            Value::Float3(_) => Self::Float3,
            Value::Float4(_) | Value::Color(_) => Self::Float4,
            Value::Text(_) => Self::String,
            _ => return None,
        })
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Boolean => "Boolean",
            Self::Integer => "Integer",
            Self::Integer2 => "Integer2",
            Self::Integer3 => "Integer3",
            Self::Integer4 => "Integer4",
            Self::Float => "Float",
            Self::Float2 => "Float2",
            Self::Float3 => "Float3",
            Self::Float4 => "Float4",
            Self::String => "String",
        }
    }

    pub fn default_value(self) -> Value {
        match self {
            Self::Boolean => Value::Bool(false),
            Self::Integer => Value::Int(0),
            Self::Integer2 => Value::Int2([0; 2]),
            Self::Integer3 => Value::Int3([0; 3]),
            Self::Integer4 => Value::Int4([0; 4]),
            Self::Float => Value::Float(0.0),
            Self::Float2 => Value::Float2([0.0; 2]),
            Self::Float3 => Value::Float3([0.0; 3]),
            Self::Float4 => Value::Float4([0.0; 4]),
            Self::String => Value::Text(String::new()),
        }
    }
}
