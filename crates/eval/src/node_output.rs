use materialcraft_core::Value;
use materialcraft_image::Image;

#[derive(Clone, Debug, PartialEq)]
pub enum NodeOutput {
    Image(Image),
    Value(Value),
}

impl NodeOutput {
    pub fn as_image(&self) -> Option<&Image> {
        match self {
            Self::Image(image) => Some(image),
            Self::Value(_) => None,
        }
    }

    pub fn as_value(&self) -> Option<&Value> {
        match self {
            Self::Image(_) => None,
            Self::Value(value) => Some(value),
        }
    }

    pub(crate) fn memory_bytes(&self) -> usize {
        match self {
            Self::Image(image) => image.data.len().saturating_mul(size_of::<f32>()),
            Self::Value(Value::Text(text)) => size_of::<Value>().saturating_add(text.len()),
            Self::Value(_) => size_of::<Value>(),
        }
    }
}
