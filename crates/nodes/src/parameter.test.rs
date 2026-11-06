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
