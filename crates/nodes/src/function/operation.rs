use super::{
    EvaluationContext,
    instruction::Instruction,
    value_type::{self, boolean, dimensions, floats, integers},
};
use materialcraft_core::Value;
use std::collections::BTreeMap;

pub(crate) fn evaluate(
    instruction: &Instruction,
    arguments: &[Value],
    variables: &mut BTreeMap<String, Value>,
    random: &mut u32,
    context: &EvaluationContext<'_>,
) -> Result<Value, String> {
    let op = instruction.definition.id;
    let first = || arguments.first().ok_or("missing first operand");
    let second = || arguments.get(1).ok_or("missing second operand");
    let name = || match &instruction.value {
        Some(Value::Text(name)) => Ok(name.as_str()),
        _ => Err("variable name must be a string".to_string()),
    };

    if op.starts_with("const_") {
        let value = instruction.value.clone().ok_or("constant has no value")?;
        if value_type::type_code(&value) != instruction.type_code {
            return Err("constant has the wrong type".into());
        }

        return Ok(value);
    }

    if op.starts_with("get_") {
        let name = name()?;
        let value = match name {
            "$pos" => Value::Float2(context.position),
            "$size" => Value::Float2(context.size),
            "$sizelog2" => Value::Float2(context.size.map(f32::log2)),
            _ => variables.get(name).or_else(|| context.variables.get(name)).cloned().ok_or_else(|| format!("undefined variable `{name}`"))?,
        };

        if value_type::type_code(&value) != instruction.type_code {
            return Err(format!("variable `{name}` has the wrong type for {op}"));
        }

        return Ok(value);
    }

    if op.starts_with("to") || op.contains("vector") || op.contains("swizzle") {
        return super::vector::evaluate(op, arguments, instruction.value.as_ref());
    }

    Ok(match op {
        "passthrough" => first()?.clone(),
        "set" => {
            let name = name()?;
            if name.is_empty() || name.starts_with('$') {
                return Err("use a nonempty user variable name".into());
            }

            let value = first()?.clone();
            variables.insert(name.to_string(), value.clone());
            value
        }
        "isdefined" => {
            let name = name()?;
            Value::Bool(variables.contains_key(name) || context.variables.contains_key(name) || matches!(name, "$pos" | "$size" | "$sizelog2"))
        }
        "and" => Value::Bool(boolean(first()?)? && boolean(second()?)?),
        "or" => Value::Bool(boolean(first()?)? || boolean(second()?)?),
        "not" => Value::Bool(!boolean(first()?)?),
        "eq" | "noteq" => {
            let (left, right) = (first()?, second()?);
            if value_type::type_code(left) != value_type::type_code(right) {
                return Err("comparison inputs must have the same type".into());
            }
            Value::Bool((left == right) == (op == "eq"))
        }
        "gt" | "gteq" | "lr" | "lreq" => {
            let (left, right) = (first()?, second()?);
            if dimensions(left) != 1 || value_type::type_code(left) != value_type::type_code(right) {
                return Err("comparison requires matching scalars".into());
            }
            let ordering = match (left, right) {
                (Value::Int(a), Value::Int(b)) => a.partial_cmp(b),
                (Value::Float(a), Value::Float(b)) => a.partial_cmp(b),
                _ => return Err("comparison requires numeric inputs".into()),
            }
            .ok_or("cannot compare non-finite values")?;
            Value::Bool(match op {
                "gt" => ordering.is_gt(),
                "gteq" => ordering.is_ge(),
                "lr" => ordering.is_lt(),
                _ => ordering.is_le(),
            })
        }
        "samplecol" | "samplelum" => {
            let position = floats(first()?)?;
            if dimensions(first()?) != 2 {
                return Err("sample position must be float2".into());
            }
            let setting = instruction.value.as_ref().map(integers).transpose()?.unwrap_or([0, 1, 0, 0]);
            let index = usize::try_from(setting[0]).map_err(|_| "negative image index")?;
            let pixel = match context.images.get(index).and_then(Option::as_ref) {
                Some(image) if setting[1] == 0 => {
                    image.get((position[0] * image.w as f32).floor() as i64, (position[1] * image.h as f32).floor() as i64, context.tile)
                }
                Some(image) => image.sample(position[0], position[1], context.tile),
                None => [0.0; 4],
            };

            if op == "samplelum" { Value::Float(pixel[0]) } else { Value::Float4(pixel) }
        }
        _ => return super::arithmetic::evaluate(op, arguments, random),
    })
}
