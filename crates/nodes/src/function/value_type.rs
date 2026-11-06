use materialcraft_core::Value;

pub fn type_code(value: &Value) -> u32 {
    materialcraft_core::ValueType::of(value).map_or(0, |value_type| value_type as u32)
}

pub fn dimensions(value: &Value) -> usize {
    match value {
        Value::Float2(_) | Value::Int2(_) => 2,
        Value::Float3(_) | Value::Int3(_) => 3,
        Value::Float4(_) | Value::Int4(_) | Value::Color(_) => 4,
        _ => 1,
    }
}

pub fn floats(value: &Value) -> Result<[f32; 4], String> {
    Ok(match value {
        Value::Float(v) => [*v; 4],
        Value::Float2(v) => [v[0], v[1], 0.0, 0.0],
        Value::Float3(v) => [v[0], v[1], v[2], 0.0],
        Value::Float4(v) | Value::Color(v) => *v,
        _ => return Err(format!("expected float, got {}", value.type_name())),
    })
}

pub fn integers(value: &Value) -> Result<[i32; 4], String> {
    Ok(match value {
        Value::Int(v) => [*v; 4],
        Value::Int2(v) => [v[0], v[1], 0, 0],
        Value::Int3(v) => [v[0], v[1], v[2], 0],
        Value::Int4(v) => *v,
        _ => return Err(format!("expected integer, got {}", value.type_name())),
    })
}

pub fn float_value(values: &[f32]) -> Result<Value, String> {
    if values.iter().any(|v| !v.is_finite()) {
        return Err("operation produced a non-finite number".into());
    }

    Ok(match values {
        [a] => Value::Float(*a),
        [a, b] => Value::Float2([*a, *b]),
        [a, b, c] => Value::Float3([*a, *b, *c]),
        [a, b, c, d] => Value::Float4([*a, *b, *c, *d]),
        _ => return Err("expected one to four components".into()),
    })
}

pub fn integer_value(values: &[i32]) -> Result<Value, String> {
    Ok(match values {
        [a] => Value::Int(*a),
        [a, b] => Value::Int2([*a, *b]),
        [a, b, c] => Value::Int3([*a, *b, *c]),
        [a, b, c, d] => Value::Int4([*a, *b, *c, *d]),
        _ => return Err("expected one to four components".into()),
    })
}

pub fn boolean(value: &Value) -> Result<bool, String> {
    match value {
        Value::Bool(value) => Ok(*value),
        _ => Err(format!("expected boolean, got {}", value.type_name())),
    }
}
