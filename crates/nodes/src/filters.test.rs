#[cfg(test)]
mod normal_blend_tests {
    use std::collections::BTreeMap;

    use materialcraft_image::{Image, Kind};

    use crate::catalog::Ctx;
    use crate::catalog::{find, resolve_params};

    /// A base normal pointing straight away from the viewer (blue = 0) used to divide by zero.
    #[test]
    fn normal_blend_with_a_black_base_stays_finite() {
        let def = find("normal_blend").expect("normal_blend exists");
        let params = resolve_params(def, &BTreeMap::new());
        let no_res = |_: &str| None;
        let vars = BTreeMap::new();
        let black = Image::filled(8, 8, Kind::Color, [0.0, 0.0, 0.0, 1.0]);
        let bump = Image::filled(8, 8, Kind::Color, [0.7, 0.4, 0.6, 1.0]);
        let inputs = vec![Some(black), Some(bump), None];
        let ctx = Ctx { w: 8, h: 8, seed: 1, tile: true, inputs: &inputs, params: &params, resource: &no_res, vars: &vars };
        let out = (def.cook)(&ctx).expect("cooks");
        assert!(out[0].data.iter().all(|v| v.is_finite()), "NaN or Inf in the blended normal");
    }

    /// The blended normal keeps the base normal's alpha.
    #[test]
    fn normal_blend_preserves_base_alpha() {
        let def = find("normal_blend").expect("normal_blend exists");
        let params = resolve_params(def, &BTreeMap::new());
        let no_res = |_: &str| None;
        let vars = BTreeMap::new();
        let alpha = 0.42;
        // A regular base normal and one pointing away from the viewer (the guard path).
        for base in [[0.5, 0.5, 1.0, alpha], [0.0, 0.0, 0.0, alpha]] {
            let base = Image::filled(8, 8, Kind::Color, base);
            let detail = Image::filled(8, 8, Kind::Color, [0.7, 0.4, 0.6, 0.9]);
            let inputs = vec![Some(base), Some(detail), None];
            let ctx = Ctx { w: 8, h: 8, seed: 1, tile: true, inputs: &inputs, params: &params, resource: &no_res, vars: &vars };
            let out = (def.cook)(&ctx).expect("cooks");
            let alphas: Vec<f32> = out[0].data.chunks(4).map(|p| p[3]).collect();
            assert!(alphas.iter().all(|a| (a - alpha).abs() < 1e-6), "base alpha {alpha}, got {:?}", alphas[0]);
        }
    }
}

// Tests for the blur helpers.

use materialcraft_image::{Image, Kind};

use super::blur_xy;

fn noise(n: u32) -> Image {
    Image::generate(n, n, Kind::Gray, |x, y| [((x * 7 + y * 13) % 17) as f32 / 16.0, 0.0, 0.0, 1.0])
}

#[test]
fn blur_xy_huge_sigma_is_bounded_and_finite() {
    let t = std::time::Instant::now();
    let out = blur_xy(&noise(64), 1e14, 1.0, true);
    assert!(out.data.iter().all(|v| v.is_finite()));
    assert!(t.elapsed().as_secs() < 20, "took {:?}", t.elapsed());
}

#[test]
fn blur_xy_horizontal_blur_is_stronger_than_vertical() {
    // Vertical stripes survive a vertical-only blur but are smoothed by a horizontal one.
    let img = Image::generate(64, 64, Kind::Gray, |x, _| [(x % 2) as f32, 0.0, 0.0, 1.0]);
    let out = blur_xy(&img, 6.0, 0.0, true);
    let range = out.data.chunks(1).map(|v| v[0]).fold((f32::MAX, f32::MIN), |a, v| (a.0.min(v), a.1.max(v)));
    assert!(range.1 - range.0 < 0.1, "stripes should be blurred away: {range:?}");
    let mean: f32 = out.data.iter().sum::<f32>() / out.data.len() as f32;
    assert!((mean - 0.5).abs() < 0.02);
}

mod height_roundtrip {
    //! Round-trip test for Height to Normal -> Normal to Height on a smooth analytic bump.

    use std::collections::BTreeMap;

    use materialcraft_core::Value;
    use materialcraft_image::{Image, Kind};

    use crate::{Ctx, find, resolve_params};

    fn run(id: &str, input: &Image, size: u32, over: &[(&str, Value)]) -> Image {
        let def = find(id).unwrap();
        let over: BTreeMap<String, Value> = over.iter().map(|(k, v)| (k.to_string(), v.clone())).collect();
        let params = resolve_params(def, &over);
        let no_res = |_: &str| None;
        let vars = BTreeMap::new();
        let inputs = [Some(input.clone())];
        let ctx = Ctx { w: size, h: size, seed: 1, tile: true, inputs: &inputs, params: &params, resource: &no_res, vars: &vars };
        (def.cook)(&ctx).unwrap().remove(0)
    }

    fn pearson(a: &Image, b: &Image) -> f32 {
        let n = (a.w * a.h) as usize;
        let (ma, mb) = (a.data.iter().take(n).sum::<f32>() / n as f32, b.data.iter().take(n).sum::<f32>() / n as f32);
        let (mut sab, mut saa, mut sbb) = (0.0f32, 0.0f32, 0.0f32);
        for i in 0..n {
            let (da, db) = (a.data[i] - ma, b.data[i] - mb);
            sab += da * db;
            saa += da * da;
            sbb += db * db;
        }
        sab / (saa * sbb).sqrt().max(1e-9)
    }

    #[test]
    fn normal_to_height_recovers_the_broad_shape_of_a_bump() {
        for size in [256u32, 1024] {
            // A wide wrap-around Gaussian bump: nearly all of its energy is at coarse scales.
            let bump = Image::generate_uv(size, size, Kind::Gray, |u, v| {
                let (dx, dy) = (u - 0.5, v - 0.5);
                [(-(dx * dx + dy * dy) / (2.0 * 0.12 * 0.12)).exp(), 0.0, 0.0, 0.0]
            });
            let normal = run("height_to_normal", &bump, size, &[("intensity", Value::Float(2.0))]);
            let back = run("normal_to_height", &normal, size, &[]);
            let c = pearson(&bump, &back);
            assert!(c > 0.99, "size {size}: correlation {c}");
        }
    }
}
