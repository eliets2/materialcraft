use std::collections::BTreeMap;

use materialcraft_core::Value;
use materialcraft_image::{Image, Kind};

use super::*;

fn cook(def: &NodeDefinition, inputs: &[Option<Image>], size: u32) -> CookResult {
    let params = resolve_params(def, &BTreeMap::new());
    let no_res = |_: &str| None;
    let vars = BTreeMap::new();
    let ctx = CookContext { w: size, h: size, seed: 7, tile: true, inputs, params: &params, resource: &no_res, vars: &vars };
    (def.cook)(&ctx)
}

/// Every node cooks with no inputs and with hostile-sized inputs, returns as many images as it has outputs, at the requested size, with finite values
#[test]
fn every_node_cooks() {
    let ids: Vec<&str> = catalog().iter().map(|d| d.id).collect();
    let mut seen = std::collections::BTreeSet::new();
    for id in &ids {
        assert!(seen.insert(*id), "duplicate node id {id}");
    }
    let odd = Image::generate(5, 3, Kind::Color, |x, y| [x as f32 / 4.0, y as f32 / 2.0, 0.5, 1.0]);
    for def in catalog() {
        if matches!(def.id, "graph_instance" | "value_processor" | "input_value") {
            continue;
        }
        for inputs in [vec![None; def.inputs.len()], vec![Some(odd.clone()); def.inputs.len()]] {
            let out = cook(def, &inputs, 32).unwrap_or_else(|e| panic!("{}: {e}", def.id));
            assert_eq!(out.len(), def.outputs.len().max(1), "{}", def.id);
            for img in &out {
                assert_eq!((img.w, img.h), (32, 32), "{}", def.id);
                assert!(img.data.iter().all(|v| v.is_finite()), "{} produced a non-finite value", def.id);
            }
        }
    }
    assert_eq!(catalog().len(), 30);
}

/// Every node cooks with each numeric parameter at a hostile value (huge, negative huge, NaN): the hard limits keep the cook bounded, finite and the right size
#[test]
fn every_node_survives_hostile_params() {
    use std::time::{Duration, Instant};

    let mut slow: Vec<(String, Duration)> = Vec::new();
    let total = Instant::now();
    for def in catalog() {
        if matches!(def.id, "graph_instance" | "value_processor" | "input_value") {
            continue;
        }
        for p in &def.params {
            for h in [1e30_f32, -1e30, f32::NAN] {
                let v = match p.default {
                    Value::Float(_) => Value::Float(h),
                    Value::Int(_) => Value::Int(h as i32),
                    Value::Float2(_) => Value::Float2([h, h]),
                    Value::Enum(_) => Value::Enum(h as u32),
                    _ => continue,
                };

                let mut params = resolve_params(def, &BTreeMap::from([(p.id.to_string(), v)]));
                clamp_params(def, &mut params);
                let inputs = vec![None; def.inputs.len()];
                let no_res = |_: &str| None;
                let vars = BTreeMap::new();
                let ctx = CookContext { w: 16, h: 16, seed: 7, tile: true, inputs: &inputs, params: &params, resource: &no_res, vars: &vars };
                let t = Instant::now();
                let out = (def.cook)(&ctx).unwrap_or_else(|e| panic!("{}.{}={h}: {e}", def.id, p.id));
                let dt = t.elapsed();
                assert!(dt < Duration::from_secs(2), "{}.{}={h} took {dt:?}", def.id, p.id);
                if dt > Duration::from_millis(250) {
                    slow.push((format!("{}.{}={h}", def.id, p.id), dt));
                }
                for img in &out {
                    assert_eq!((img.w, img.h), (16, 16), "{}.{}={h}", def.id, p.id);
                    assert!(img.data.iter().all(|x| x.is_finite()), "{}.{}={h} produced a non-finite value", def.id, p.id);
                }
            }
        }
    }
    eprintln!("hostile params: total {:?}; cooks over 250 ms: {slow:?}", total.elapsed());
}

#[test]
fn blend_and_levels_math() {
    let fg = Image::filled(4, 4, Kind::Gray, [0.25; 4]);
    let bg = Image::filled(4, 4, Kind::Gray, [0.5; 4]);
    let def = find("blend").unwrap();
    let mut params = resolve_params(def, &BTreeMap::new());
    params.insert("mode".into(), materialcraft_core::Value::Enum(3));
    let no_res = |_: &str| None;
    let vars = BTreeMap::new();
    let inputs = [Some(fg), Some(bg), None];
    let ctx = CookContext { w: 4, h: 4, seed: 0, tile: true, inputs: &inputs, params: &params, resource: &no_res, vars: &vars };
    let out = (def.cook)(&ctx).unwrap();
    assert!((out[0].data[0] - 0.125).abs() < 1e-6);
    assert!((atomic::levels_fn(0.5, 0.0, 0.5, 1.0, 0.0, 1.0) - 0.5).abs() < 1e-5);
    assert_eq!(atomic::levels_fn(0.2, 0.4, 0.5, 0.8, 0.0, 1.0), 0.0);
}

#[test]
fn dynamic_gradient_preserves_the_gradient_image_type() {
    let definition = find("gradient_dynamic").unwrap();
    for kind in [Kind::Gray, Kind::Color] {
        let source = Image::filled(4, 4, Kind::Gray, [0.5; 4]);
        let gradient = Image::filled(4, 4, kind, [0.25, 0.5, 0.75, 1.0]);
        let result = cook(definition, &[Some(source), Some(gradient.clone())], 16).unwrap();
        assert_eq!(result[0].kind, kind);
        assert_eq!(result[0].get(0, 0, false), gradient.get(0, 0, false));
    }
}
