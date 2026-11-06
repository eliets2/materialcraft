use std::collections::BTreeMap;

use materialcraft_core::Value;
use materialcraft_image::{Image, Kind};
use materialcraft_nodes::{CookContext, find, resolve_params};

fn cook(mask: Image, source: Option<Image>, tile: bool, combine: bool, distance: f32) -> Image {
    let def = find("distance").unwrap();
    let params = resolve_params(def, &BTreeMap::from([("distance".into(), Value::Float(distance)), ("combine".into(), Value::Bool(combine))]));
    let (w, h) = (mask.w, mask.h);
    let inputs = [Some(mask), source];
    let vars = BTreeMap::new();
    let resource = |_: &str| None;
    let ctx = CookContext { w, h, seed: 0, tile, inputs: &inputs, params: &params, resource: &resource, vars: &vars };
    (def.cook)(&ctx).unwrap().remove(0)
}

#[test]
fn tiled_distance_spreads_source_across_stationary_gradient() {
    let mask = Image::gray(32, 32, |x, y| if x == 0 && y == 0 { 1.0 } else { 0.0 });
    let output = cook(mask.clone(), Some(mask), true, false, 256.0);
    // This is within range of the only mask pixel, but the symmetric distance field has zero
    // slope here. A gradient walk stopped on the black source background instead of the seed.
    assert_eq!(output.get_gray(16, 16, false), 1.0);
    assert!(output.data.iter().all(|&v| v == 1.0));
}

#[test]
fn distance_spreads_source_beyond_the_old_walk_limit() {
    let mask = Image::gray(1024, 1, |x, _| if x == 0 { 1.0 } else { 0.0 });
    let output = cook(mask.clone(), Some(mask), false, false, 256.0);
    assert_eq!(output.get_gray(1023, 0, false), 1.0);
}

#[test]
fn distance_preserves_source_alpha_range_and_combine() {
    for tile in [false, true] {
        let mask = Image::gray(9, 3, |x, y| if x == 0 && y == 1 { 1.0 } else { 0.0 });
        let source = Image::generate(9, 3, Kind::Color, |x, y| if x == 0 && y == 1 { [0.8, 0.4, 0.2, 0.6] } else { [0.1, 0.1, 0.1, 1.0] });
        for combine in [false, true] {
            let output = cook(mask.clone(), Some(source.clone()), tile, combine, 512.0 / 9.0); // two pixels
            assert_eq!(output.get(0, 1, false), [0.8, 0.4, 0.2, 0.6]);
            let expected = if combine { [0.4, 0.2, 0.1, 0.6] } else { [0.8, 0.4, 0.2, 0.6] };
            assert_eq!(output.get(1, 1, false), expected);
            assert_eq!(output.get(4, 1, false), [0.0; 4]);
            assert_eq!(output.get(8, 1, false), if tile { expected } else { [0.0; 4] });
        }
    }
}

#[test]
fn distance_handles_empty_full_masks_and_no_source() {
    for tile in [false, true] {
        let empty = Image::filled(7, 5, Kind::Gray, [0.0; 4]);
        let full = Image::filled(7, 5, Kind::Gray, [1.0; 4]);
        let source = Image::generate(7, 5, Kind::Color, |x, y| [x as f32 / 7.0, y as f32 / 5.0, 0.4, 0.7]);
        assert!(cook(empty.clone(), Some(source.clone()), tile, false, 256.0).data.iter().all(|&v| v == 0.0));
        assert_eq!(cook(full, Some(source), tile, true, 256.0).get(3, 2, false), [3.0 / 7.0, 2.0 / 5.0, 0.4, 0.7]);
        assert!(cook(empty, None, tile, true, 256.0).data.iter().all(|&v| v == 0.0));
        let mask = Image::gray(7, 5, |x, y| if x == 0 && y == 0 { 1.0 } else { 0.0 });
        let ramp = cook(mask, None, tile, false, 512.0 / 7.0);
        assert_eq!(ramp.get_gray(0, 0, false), 1.0);
        assert_eq!(ramp.get_gray(1, 0, false), 0.5);
        assert_eq!(ramp.get_gray(3, 0, false), 0.0);
    }
}
