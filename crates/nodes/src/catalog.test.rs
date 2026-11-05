//! Parameter resolution tests.

use std::collections::BTreeMap;

use materialcraft_core::Value;

use crate::{catalog, clamp_params, find, resolve_params};

fn over(k: &str, v: Value) -> BTreeMap<String, Value> {
    BTreeMap::from([(k.to_string(), v)])
}

#[test]
fn resolve_params_clamps_to_hard_limits() {
    let def = find("splatter").unwrap();
    let p = resolve_params(def, &over("size", Value::Float(1e30)));
    assert_eq!(p["size"], Value::Float(17.0));
    let p = resolve_params(def, &over("size", Value::Float(-1e30)));
    assert_eq!(p["size"], Value::Float(-16.0));
    let p = resolve_params(def, &over("count", Value::Int(i32::MAX)));
    assert_eq!(p["count"], Value::Int(33984));
    let p = resolve_params(def, &over("size", Value::Float(f32::NAN)));
    assert_eq!(p["size"], def.param("size").unwrap().default);
    let shape = find("shape").unwrap();
    let p = resolve_params(shape, &over("size_xy", Value::Float2([1e9, -1e9])));
    assert_eq!(p["size_xy"], Value::Float2([34.0, -32.0]));
}

#[test]
fn clamp_params_covers_values_set_after_resolution() {
    let def = find("splatter").unwrap();
    let mut p = resolve_params(def, &BTreeMap::new());
    p.insert("size".into(), Value::Float(1e9));
    clamp_params(def, &mut p);
    assert_eq!(p["size"], Value::Float(17.0));
}

#[test]
fn defaults_lie_inside_their_hard_limits() {
    for def in catalog() {
        let p = resolve_params(def, &BTreeMap::new());
        for d in &def.params {
            assert_eq!(p[d.id], d.default, "{}.{} default is outside its range", def.id, d.id);
        }
    }
}

#[test]
fn manual_values_beyond_slider_and_multiple_angle_turns_survive() {
    let def = find("splatter").unwrap();
    assert_eq!(resolve_params(def, &over("size", Value::Float(1.25)))["size"], Value::Float(1.25));
    for def in catalog() {
        for p in &def.params {
            if p.angle && matches!(p.default, Value::Float(_)) {
                let v = p.max + (p.max - p.min);
                assert_eq!(resolve_params(def, &over(p.id, Value::Float(v)))[p.id], Value::Float(v));
            }
        }
    }
}

#[test]
fn clamp_params_bounds_enum_without_options() {
    let mut def = find("splatter").unwrap().clone();
    if let Some(p) = def.params.iter_mut().find(|p| p.id == "pattern") {
        p.options = &[];
        p.default = Value::Enum(0);
        p.min = 0.0;
        p.max = 0.0;
    }
    if let Some(p) = def.params.iter_mut().find(|p| p.id == "size") {
        p.default = Value::Float(5.0);
        p.min = 5.0;
        p.max = 5.0;
    }
    let mut p = BTreeMap::new();
    p.insert("pattern".into(), Value::Enum(999));
    p.insert("size".into(), Value::Float(1e9));
    clamp_params(&def, &mut p);
    assert_eq!(p["pattern"], Value::Enum(0));
    assert_eq!(p["size"], Value::Float(5.0));
}
