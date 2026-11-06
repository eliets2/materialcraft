use crate::NodeDefinition;
use std::sync::OnceLock;

/// Every node definition, in library order
pub fn catalog() -> &'static [NodeDefinition] {
    static CAT: OnceLock<Vec<NodeDefinition>> = OnceLock::new();
    CAT.get_or_init(crate::atomic::defs)
}

pub fn find(id: &str) -> Option<&'static NodeDefinition> {
    catalog().iter().find(|d| d.id == id)
}

/// The library's category tree, in display order: (path, ids)
pub fn categories() -> Vec<(&'static str, Vec<&'static str>)> {
    let mut out: Vec<(&'static str, Vec<&'static str>)> = Vec::new();
    for d in catalog() {
        match out.iter_mut().find(|(c, _)| *c == d.category) {
            Some((_, ids)) => ids.push(d.id),
            None => out.push((d.category, vec![d.id])),
        }
    }
    out
}
use std::collections::BTreeMap;

use crate::Value;

/// How many slider spans the hard limit extends past each end of a parameter's range.
const HARD_SPAN: f32 = 16.0;

/// Clamp `v` to `[lo, hi]`; a non-finite `v` becomes `fallback`.
fn clamp_finite(v: f32, lo: f32, hi: f32, fallback: f32) -> f32 {
    if v.is_finite() { v.clamp(lo, hi) } else { fallback }
}

/// Clamp numeric parameters to hard limits: the declared slider range widened by
/// `HARD_SPAN` spans on each side (the editor lets values past the slider through, so the
/// slider range itself is not a limit). Non-finite values become the default. Keeps cooks
/// bounded whatever a file, binding, function or command supplies.
pub fn clamp_params(def: &NodeDefinition, params: &mut BTreeMap<String, Value>) {
    for p in &def.params {
        let Some(v) = params.get_mut(p.id) else { continue };
        if let Value::Enum(i) = v {
            let fallback = match &p.default {
                Value::Enum(d) => *d,
                _ => 0,
            };
            let bounds = if !p.options.is_empty() {
                let hi = u32::try_from(p.options.len() - 1).unwrap_or(u32::MAX);
                Some((0u32, hi))
            } else if p.min.is_finite() && p.max.is_finite() && p.min <= p.max {
                if p.min < 0.0 { None } else { Some((p.min as u32, p.max as u32)) }
            } else {
                None
            };
            match bounds {
                Some((lo, hi)) if lo <= hi => *i = (*i).clamp(lo, hi),
                _ => *i = fallback,
            }
            continue;
        }
        if let Value::Color(c) = v {
            for x in c {
                *x = clamp_finite(*x, -HARD_SPAN, 1.0 + HARD_SPAN, 0.0);
            }
            continue;
        }
        if p.min.is_nan() || p.max.is_nan() || p.min > p.max {
            // No usable range: fall back to the parameter default. Infinite
            // bounds are usable: they make the range unbounded (below).
            match (&mut *v, &p.default) {
                (Value::Float(x), Value::Float(d)) => *x = *d,
                (Value::Float2(a), Value::Float2(d)) => *a = *d,
                (Value::Float3(a), Value::Float3(d)) => *a = *d,
                (Value::Float4(a), Value::Float4(d)) => *a = *d,
                (Value::Int(x), Value::Int(d)) => *x = *d,
                (Value::Int2(a), Value::Int2(d)) => *a = *d,
                _ => {}
            }
            continue;
        }
        let span = p.max - p.min;
        let (lo, hi) = if span.is_finite() {
            (p.min - HARD_SPAN * span, p.max + HARD_SPAN * span)
        } else {
            // An infinite bound leaves no finite span to widen: unbounded.
            (f32::NEG_INFINITY, f32::INFINITY)
        };
        // The default's component `i`, or 0 when the default is a different variant.
        let fallback = |i: usize| match &p.default {
            Value::Float(d) => [*d].get(i).copied(),
            Value::Float2(d) => d.get(i).copied(),
            Value::Float3(d) => d.get(i).copied(),
            Value::Float4(d) => d.get(i).copied(),
            Value::Int(d) => Some(*d as f32),
            Value::Int2(d) => d.get(i).map(|x| *x as f32),
            Value::Enum(d) => Some(*d as f32),
            _ => None,
        };
        let clamp_at = |x: &mut f32, i: usize| *x = clamp_finite(*x, lo, hi, fallback(i).unwrap_or(0.0));
        let clamp_int = |x: &mut i32| *x = (*x as f32).clamp(lo, hi).round() as i32;
        match v {
            Value::Float(x) => clamp_at(x, 0),
            Value::Float2(a) => a.iter_mut().enumerate().for_each(|(i, x)| clamp_at(x, i)),
            Value::Float3(a) => a.iter_mut().enumerate().for_each(|(i, x)| clamp_at(x, i)),
            Value::Float4(a) => a.iter_mut().enumerate().for_each(|(i, x)| clamp_at(x, i)),
            Value::Int(x) => clamp_int(x),
            Value::Int2(a) => a.iter_mut().for_each(clamp_int),
            _ => {}
        }
    }
}

/// Parameter values for a node: catalog defaults overlaid with `overrides`.
pub fn resolve_params(def: &NodeDefinition, overrides: &BTreeMap<String, Value>) -> BTreeMap<String, Value> {
    let mut m: BTreeMap<String, Value> = def.params.iter().map(|p| (p.id.to_string(), p.default.clone())).collect();
    for (k, v) in overrides {
        // Keep only known parameters whose type matches (a hand-edited file may have anything).
        if let Some(d) = m.get(k)
            && d.type_name() == v.type_name()
        {
            m.insert(k.clone(), v.clone());
        }
    }
    clamp_params(def, &mut m);
    m
}

#[cfg(test)]
#[path = "catalog.test.rs"]
mod tests;
