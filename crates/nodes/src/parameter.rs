use crate::NodeDefinition;
use materialcraft_core::Value;
use std::collections::BTreeMap;

/// How many slider spans the hard limit extends past each end of a parameter's range
const HARD_SPAN: f32 = 16.0;

/// Clamp `v` to `[lo, hi]`; a non-finite `v` becomes `fallback`
fn clamp_finite(v: f32, lo: f32, hi: f32, fallback: f32) -> f32 {
    if v.is_finite() { v.clamp(lo, hi) } else { fallback }
}

pub fn clamp_params(def: &NodeDefinition, params: &mut BTreeMap<String, Value>) {
    for p in &def.params {
        let Some(v) = params.get_mut(p.id) else { continue };
        if let Value::Enum(i) = v {
            // (an enum without options has nothing to choose from: its default stands)
            *i = match (p.options.len().checked_sub(1), &p.default) {
                (Some(last), _) => (*i).min(u32::try_from(last).unwrap_or(u32::MAX)),
                (None, Value::Enum(d)) => *d,
                (None, _) => 0,
            };
            continue;
        }
        if let Value::Color(c) = v {
            for x in c {
                *x = clamp_finite(*x, -HARD_SPAN, 1.0 + HARD_SPAN, 0.0);
            }
            continue;
        }
        // No range declared (an Input Value's default takes any number): only a value that isn't
        // a number goes
        let ranged = p.min < p.max;
        let span = p.max - p.min;
        let (lo, hi) = if ranged { (p.min - HARD_SPAN * span, p.max + HARD_SPAN * span) } else { (f32::MIN, f32::MAX) };
        // The default's component `i`, or 0 when the default is a different variant
        let fallback = |i: usize| match &p.default {
            Value::Float(d) => [*d].get(i).copied(),
            Value::Float2(d) => d.get(i).copied(),
            Value::Float3(d) => d.get(i).copied(),
            Value::Float4(d) => d.get(i).copied(),
            _ => None,
        };

        let clamp_at = |x: &mut f32, i: usize| *x = clamp_finite(*x, lo, hi, fallback(i).unwrap_or(0.0));
        let clamp_int = |x: &mut i32| *x = (*x as f32).clamp(lo, hi).round() as i32;
        match v {
            Value::Float(x) => clamp_at(x, 0),
            Value::Float2(a) => a.iter_mut().enumerate().for_each(|(i, x)| clamp_at(x, i)),
            Value::Float3(a) => a.iter_mut().enumerate().for_each(|(i, x)| clamp_at(x, i)),
            Value::Float4(a) => a.iter_mut().enumerate().for_each(|(i, x)| clamp_at(x, i)),
            Value::Int(x) if ranged => clamp_int(x),
            Value::Int2(a) if ranged => a.iter_mut().for_each(clamp_int),
            _ => {}
        }
    }
}

/// Parameter values for a node: catalog defaults overlaid with `overrides`
pub fn resolve_params(def: &NodeDefinition, overrides: &BTreeMap<String, Value>) -> BTreeMap<String, Value> {
    let mut m: BTreeMap<String, Value> = def.params.iter().map(|p| (p.id.to_string(), p.default.clone())).collect();
    for (k, v) in overrides {
        // Keep only known parameters whose type matches (a hand-edited file may have anything)
        if let Some(d) = m.get(k)
            && (d.type_name() == v.type_name() || (def.id == "input_value" && k == "default" && materialcraft_core::ValueType::of(v).is_some()))
        {
            m.insert(k.clone(), v.clone());
        }
    }
    clamp_params(def, &mut m);
    m
}

#[cfg(test)]
#[path = "parameter.test.rs"]
mod tests;
