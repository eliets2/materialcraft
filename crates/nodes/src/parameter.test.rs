//! Parameter resolution tests.

use std::collections::BTreeMap;

use materialcraft_core::Value;

use crate::{catalog, clamp_params, find, resolve_params};

fn over(k: &str, v: Value) -> BTreeMap<String, Value> {
    BTreeMap::from([(k.to_string(), v)])
}

#[test]
fn resolve_params_clamps_to_hard_limits() {
    let def = find("blend").unwrap();
    let p = resolve_params(def, &over("opacity", Value::Float(1e30)));
    assert_eq!(p["opacity"], Value::Float(17.0));
    let p = resolve_params(def, &over("opacity", Value::Float(-1e30)));
    assert_eq!(p["opacity"], Value::Float(-16.0));
    let p = resolve_params(def, &over("opacity", Value::Float(f32::NAN)));
    assert_eq!(p["opacity"], def.param("opacity").unwrap().default);
    let levels = find("levels").unwrap();
    let p = resolve_params(levels, &over("in_low", Value::Float4([1e9, -1e9, 0.0, 1.0])));
    assert_eq!(p["in_low"], Value::Float4([17.0, -16.0, 0.0, 1.0]));
}

#[test]
fn clamp_params_covers_values_set_after_resolution() {
    let def = find("blend").unwrap();
    let mut p = resolve_params(def, &BTreeMap::new());
    p.insert("opacity".into(), Value::Float(1e9));
    clamp_params(def, &mut p);
    assert_eq!(p["opacity"], Value::Float(17.0));
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
    let def = find("blend").unwrap();
    assert_eq!(resolve_params(def, &over("opacity", Value::Float(1.25)))["opacity"], Value::Float(1.25));
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
    let p = def.params.iter_mut().find(|p| p.id == "pattern").expect("pattern param");
    p.options = &[];
    p.default = Value::Enum(0);
    p.min = 0.0;
    p.max = 0.0;
    let mut p = BTreeMap::new();
    p.insert("pattern".into(), Value::Enum(999));
    clamp_params(&def, &mut p);
    assert_eq!(p["pattern"], Value::Enum(0));
}

#[test]
fn clamp_params_degenerate_range_clamps_to_min() {
    let mut def = find("splatter").unwrap().clone();
    let p = def.params.iter_mut().find(|p| p.id == "size").expect("size param");
    p.default = Value::Float(5.0);
    p.min = 5.0;
    p.max = 5.0;
    let mut p = BTreeMap::new();
    p.insert("size".into(), Value::Float(1e9));
    clamp_params(&def, &mut p);
    assert_eq!(p["size"], Value::Float(5.0));
}

#[test]
fn clamp_params_inverted_range_falls_back_to_default() {
    let mut def = find("splatter").unwrap().clone();
    let p = def.params.iter_mut().find(|p| p.id == "size").expect("size param");
    p.default = Value::Float(5.0);
    p.min = 10.0;
    p.max = 0.0;
    let mut p = BTreeMap::new();
    p.insert("size".into(), Value::Float(1e9));
    clamp_params(&def, &mut p);
    assert_eq!(p["size"], Value::Float(5.0));
}

#[test]
fn clamp_params_nan_range_falls_back_to_default() {
    let mut def = find("splatter").unwrap().clone();
    let p = def.params.iter_mut().find(|p| p.id == "size").expect("size param");
    p.default = Value::Float(5.0);
    p.min = f32::NAN;
    p.max = 1.0;
    let mut p = BTreeMap::new();
    p.insert("size".into(), Value::Float(1e9));
    clamp_params(&def, &mut p);
    assert_eq!(p["size"], Value::Float(5.0));
}

#[test]
fn clamp_params_non_finite_range_is_unbounded() {
    let mut def = find("splatter").unwrap().clone();
    let p = def.params.iter_mut().find(|p| p.id == "size").expect("size param");
    p.default = Value::Float(5.0);
    p.min = f32::NEG_INFINITY;
    p.max = f32::INFINITY;
    let mut p = BTreeMap::new();
    p.insert("size".into(), Value::Float(1e9));
    clamp_params(&def, &mut p);
    assert_eq!(p["size"], Value::Float(1e9));
}
