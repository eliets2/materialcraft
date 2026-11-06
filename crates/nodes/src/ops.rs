//! Image operations shared by many nodes: Gaussian blur, Euclidean distance transform, slopes,
//! color space helpers and blend modes.

use std::sync::Arc;

use materialcraft_image::{Image, Kind};
use rayon::prelude::*;

/// Gaussian blur with standard deviation `sigma` (pixels). Small sigmas use a sampled kernel;
/// larger ones three box passes (a close Gaussian approximation in O(1) per pixel).
pub fn gaussian(img: &Image, sigma: f32, tile: bool) -> Image {
    if sigma.is_nan() || sigma <= 0.05 || !sigma.is_finite() {
        return img.clone();
    }
    let sigma = sigma.min(img.w.max(img.h) as f32);
    // Bound each axis by its own dimension: a shared radius based on the
    // longer side can far exceed the shorter side and let edge samples dominate.
    let sigma_x = sigma.min(img.w as f32);
    let sigma_y = sigma.min(img.h as f32);
    if sigma < 3.0 {
        let kernel = |s: f32| {
            let r = (s * 3.0).ceil() as i64;
            let k: Vec<f32> = (-r..=r).map(|i| (-(i * i) as f32 / (2.0 * s * s)).exp()).collect();
            let sum: f32 = k.iter().sum();
            (k.iter().map(|v| v / sum).collect::<Vec<f32>>(), r)
        };
        let (kx, rx) = kernel(sigma_x);
        let (ky, ry) = kernel(sigma_y);
        let mid = convolve_1d(img, &kx, rx, true, tile);
        return convolve_1d(&mid, &ky, ry, false, tile);
    }
    // Box widths for three passes (Kovesi, "Fast almost-Gaussian filtering").
    let radii = |s: f32| {
        let n = 3.0f32;
        let wl = ((12.0 * s * s / n + 1.0).sqrt()).floor() as i64;
        let wl = if wl % 2 == 0 { wl - 1 } else { wl };
        let wu = wl + 2;
        let m = ((12.0 * s * s - n * (wl * wl) as f32 - 4.0 * n * wl as f32 - 3.0 * n) / (-4.0 * wl as f32 - 4.0)).round() as i64;
        [0, 1, 2].map(|i| ((if i < m { wl } else { wu }) - 1) / 2)
    };
    let radii_x = radii(sigma_x);
    let radii_y = radii(sigma_y);
    let mut out = img.clone();
    for i in 0..3 {
        out = box_1d(&out, radii_x[i].max(0), true, tile);
        out = box_1d(&out, radii_y[i].max(0), false, tile);
    }
    out
}

/// Border handling: out-of-range taps are skipped and the accumulator is divided by the sum
/// of in-range kernel weights (`wsum`), so edge pixels are averaged over in-range taps only.
/// This changes small-sigma (sigma < 3) gaussian border output compared with a clamped-tap
/// sum; constant images stay constant at the border.
fn convolve_1d(src: &Image, k: &[f32], r: i64, horizontal: bool, tile: bool) -> Image {
    let c = src.channels();
    Image::generate(src.w, src.h, src.kind, |x, y| {
        let mut acc = [0.0f32; 4];
        let mut wsum = 0.0f32;
        for (i, kv) in k.iter().enumerate() {
            let o = i as i64 - r;
            let pos = if horizontal { x as i64 + o } else { y as i64 + o };
            let n = if horizontal { src.w } else { src.h } as i64;
            if pos < 0 || pos >= n {
                continue;
            }
            let p = if horizontal { src.get(pos, y as i64, tile) } else { src.get(x as i64, pos, tile) };
            for ch in 0..c.clamp(1, 4) {
                acc[ch] += p[ch] * kv;
            }
            wsum += kv;
        }
        let inv = if wsum > 0.0 { 1.0 / wsum } else { 0.0 };
        for ch in 0..c.clamp(1, 4) {
            acc[ch] *= inv;
        }
        if c == 1 { [acc[0], 0.0, 0.0, 0.0] } else { acc }
    })
}

/// One running-sum box pass of radius `r`.
fn box_1d(src: &Image, r: i64, horizontal: bool, tile: bool) -> Image {
    if r <= 0 {
        return src.clone();
    }
    let (w, h) = (src.w as usize, src.h as usize);
    let c = src.channels();
    let inv = 1.0 / (2 * r + 1) as f32;
    let line_len = if horizontal { w } else { h };
    // A window spanning the whole line gives the line mean at every position.
    if 2 * r + 1 >= line_len as i64 {
        let cc = c.clamp(1, 4);
        let lines = if horizontal { h } else { w };
        let inv_line = if line_len > 0 { 1.0 / line_len as f32 } else { 0.0 };
        let mut means = vec![0.0f32; lines * cc];
        for line in 0..lines {
            let mut acc = [0.0f32; 4];
            for i in 0..line_len {
                let (px, py) = if horizontal { (i, line) } else { (line, i) };
                for ch in 0..cc {
                    acc[ch] += src.data.get((py * w + px) * c + ch).copied().unwrap_or(0.0);
                }
            }
            for ch in 0..cc {
                means[line * cc + ch] = acc[ch] * inv_line;
            }
        }
        return Image::generate(src.w, src.h, src.kind, |x, y| {
            let line = if horizontal { y as usize } else { x as usize };
            let mut acc = [0.0f32; 4];
            for ch in 0..cc {
                acc[ch] = means.get(line * cc + ch).copied().unwrap_or(0.0);
            }
            if c == 1 { [acc[0], 0.0, 0.0, 0.0] } else { acc }
        });
    }
    let mut data = vec![0.0f32; w * h * c];
    let lines = if horizontal { h } else { w };
    let get = |line: usize, i: i64, ch: usize| -> f32 {
        let n = line_len as i64;
        let i = if tile { i.rem_euclid(n) } else { i.clamp(0, n - 1) } as usize;
        let (x, y) = if horizontal { (i, line) } else { (line, i) };
        src.data.get((y * w + x) * c + ch).copied().unwrap_or(0.0)
    };
    let results: Vec<Vec<f32>> = (0..lines)
        .into_par_iter()
        .map(|line| {
            let mut out = vec![0.0f32; line_len * c];
            for ch in 0..c {
                let mut sum: f32 = (-r..=r).map(|i| get(line, i, ch)).sum();
                for i in 0..line_len {
                    out[i * c + ch] = sum * inv;
                    sum += get(line, i as i64 + r + 1, ch) - get(line, i as i64 - r, ch);
                }
            }
            out
        })
        .collect();
    for (line, vals) in results.iter().enumerate() {
        for i in 0..line_len {
            let (x, y) = if horizontal { (i, line) } else { (line, i) };
            let dst = (y * w + x) * c;
            data[dst..dst + c].copy_from_slice(&vals[i * c..i * c + c]);
        }
    }
    Image { w: src.w, h: src.h, kind: src.kind, data: Arc::new(data) }
}

/// Squared Euclidean distance (pixels) from every pixel to the nearest pixel where `inside` is
/// true (Felzenszwalb–Huttenlocher, exact, linear time). With `tile`, distances wrap.
pub fn distance_sq(w: u32, h: u32, inside: impl Fn(usize) -> bool + Sync, tile: bool) -> Vec<f32> {
    distance_transform::<false>(w, h, inside, tile).0
}

/// The same exact distance transform, also retaining the nearest mask pixel's row-major index.
/// Empty masks have no nearest pixel: their indices are `usize::MAX`.
pub fn distance_sq_with_sites(w: u32, h: u32, inside: impl Fn(usize) -> bool + Sync, tile: bool) -> (Vec<f32>, Vec<usize>) {
    distance_transform::<true>(w, h, inside, tile)
}

const DISTANCE_INF: f32 = 1e20;

fn edt_1d<const SITES: bool>(f: &[f32]) -> (Vec<f32>, Vec<usize>) {
    let n = f.len();
    if n == 0 {
        return (Vec::new(), Vec::new());
    }
    let f: Vec<f64> = f.iter().map(|v| *v as f64).collect();
    let mut d = vec![0.0f32; n];
    // Distance-only callers do not allocate or propagate site indices.
    let mut sites = if SITES { vec![usize::MAX; n] } else { Vec::new() };
    let mut v = vec![0usize; n];
    let mut z = vec![0.0f64; n + 1];
    let mut k = 0usize;
    z[0] = -(DISTANCE_INF as f64);
    z[1] = DISTANCE_INF as f64;
    let cross = |q: usize, p: usize| ((f[q] + (q * q) as f64) - (f[p] + (p * p) as f64)) / (2.0 * q as f64 - 2.0 * p as f64);
    for q in 1..n {
        let mut s = cross(q, v[k]);
        while k > 0 && s <= z[k] {
            k -= 1;
            s = cross(q, v[k]);
        }
        k += 1;
        v[k] = q;
        z[k] = s;
        z[k + 1] = DISTANCE_INF as f64;
    }
    k = 0;
    for (q, dq) in d.iter_mut().enumerate() {
        while k + 1 < n && z[k + 1] < q as f64 {
            k += 1;
        }
        let p = v[k];
        *dq = ((q as f64 - p as f64).powi(2) + f[p]) as f32;
        if SITES && f[p] < DISTANCE_INF as f64 {
            sites[q] = p;
        }
    }
    (d, sites)
}

fn distance_transform<const SITES: bool>(w: u32, h: u32, inside: impl Fn(usize) -> bool + Sync, tile: bool) -> (Vec<f32>, Vec<usize>) {
    let (w, h) = (w as usize, h as usize);
    if w == 0 || h == 0 {
        return (Vec::new(), Vec::new());
    }
    // Tiling: run on a 3x tiled line so distances cross the edges.
    let rep = if tile { 3 } else { 1 };
    // Columns. Site indices are vertical coordinates at this stage.
    let cols: Vec<_> = (0..w)
        .into_par_iter()
        .map(|x| {
            let f: Vec<f32> = (0..h * rep).map(|y| if inside((y % h) * w + x) { 0.0 } else { DISTANCE_INF }).collect();
            let (d, sites) = edt_1d::<SITES>(&f);
            let off = if tile { h } else { 0 };
            (d[off..off + h].to_vec(), if SITES { sites[off..off + h].to_vec() } else { Vec::new() })
        })
        .collect();
    // Rows. Carry the chosen column's vertical site through the horizontal transform.
    let rows: Vec<_> = (0..h)
        .into_par_iter()
        .map(|y| {
            let f: Vec<f32> = (0..w * rep).map(|x| cols[x % w].0[y]).collect();
            let (d, sites) = edt_1d::<SITES>(&f);
            let off = if tile { w } else { 0 };
            let sites = if SITES {
                sites[off..off + w]
                    .iter()
                    .map(|&x| {
                        if x == usize::MAX {
                            usize::MAX
                        } else {
                            let x = x % w;
                            let sy = cols[x].1[y];
                            if sy == usize::MAX { usize::MAX } else { (sy % h) * w + x }
                        }
                    })
                    .collect()
            } else {
                Vec::new()
            };
            (d[off..off + w].to_vec(), sites)
        })
        .collect();
    drop(cols);
    let mut distances = Vec::with_capacity(w * h);
    let mut sites = if SITES { Vec::with_capacity(w * h) } else { Vec::new() };
    for (row, row_sites) in rows {
        distances.extend(row);
        sites.extend(row_sites);
    }
    (distances, sites)
}

/// Slope (dx, dy) of a gray image by central differences, in value units per pixel.
#[inline]
pub fn slope(img: &Image, x: i64, y: i64, tile: bool) -> (f32, f32) {
    let dx = (img.get_gray(x + 1, y, tile) - img.get_gray(x - 1, y, tile)) * 0.5;
    let dy = (img.get_gray(x, y + 1, tile) - img.get_gray(x, y - 1, tile)) * 0.5;
    (dx, dy)
}

pub fn rgb_to_hsl(c: [f32; 4]) -> [f32; 3] {
    let (r, g, b) = (c[0], c[1], c[2]);
    let mx = r.max(g).max(b);
    let mn = r.min(g).min(b);
    let l = (mx + mn) * 0.5;
    if (mx - mn).abs() < 1e-7 {
        return [0.0, 0.0, l];
    }
    let d = mx - mn;
    let s = if l > 0.5 { d / (2.0 - mx - mn) } else { d / (mx + mn) };
    let h = if mx == r {
        (g - b) / d + if g < b { 6.0 } else { 0.0 }
    } else if mx == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    [h / 6.0, s, l]
}

pub fn hsl_to_rgb(h: f32, s: f32, l: f32) -> [f32; 3] {
    if s <= 0.0 {
        return [l, l, l];
    }
    let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
    let p = 2.0 * l - q;
    let f = |mut t: f32| {
        t = t.rem_euclid(1.0);
        if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 0.5 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        }
    };
    [f(h + 1.0 / 3.0), f(h), f(h - 1.0 / 3.0)]
}

pub const BLEND_MODES: &[&str] = &[
    "Copy",
    "Add (Linear dodge)",
    "Subtract",
    "Multiply",
    "Add Sub",
    "Max (Lighten)",
    "Min (Darken)",
    "Switch",
    "Divide",
    "Overlay",
    "Screen",
    "Soft light",
    "Difference",
    "Color dodge",
    "Color burn",
    "Hard light",
];

/// Blend one channel: `b` background, `f` foreground.
#[inline]
pub fn blend_channel(mode: i32, b: f32, f: f32) -> f32 {
    match mode {
        1 => b + f,
        2 => b - f,
        3 => b * f,
        4 => b + 2.0 * f - 1.0,
        5 => b.max(f),
        6 => b.min(f),
        8 => {
            if f.abs() < 1e-6 {
                if b > 0.0 { 1.0 } else { 0.0 }
            } else {
                b / f
            }
        }
        9 => {
            if b < 0.5 {
                2.0 * b * f
            } else {
                1.0 - 2.0 * (1.0 - b) * (1.0 - f)
            }
        }
        10 => 1.0 - (1.0 - b) * (1.0 - f),
        11 => {
            // W3C soft light.
            if f <= 0.5 {
                b - (1.0 - 2.0 * f) * b * (1.0 - b)
            } else {
                let d = if b <= 0.25 { ((16.0 * b - 12.0) * b + 4.0) * b } else { b.max(0.0).sqrt() };
                b + (2.0 * f - 1.0) * (d - b)
            }
        }
        12 => (b - f).abs(),
        13 => {
            // W3C color dodge: a black background stays black, even under a white foreground
            if f >= 1.0 { if b > 0.0 { 1.0 } else { 0.0 } } else { (b / (1.0 - f)).min(1.0) }
        }
        14 => {
            // W3C color burn: a white background stays white, even under a black foreground
            if f <= 0.0 { if b < 1.0 { 0.0 } else { 1.0 } } else { 1.0 - ((1.0 - b) / f).min(1.0) }
        }
        15 => {
            if f < 0.5 {
                2.0 * b * f
            } else {
                1.0 - 2.0 * (1.0 - b) * (1.0 - f)
            }
        }
        _ => f,
    }
}

/// Blend `fg` over `bg` with `mode`, weighted by `opacity` (0..1, already including any mask).
/// `use_alpha`: the foreground's alpha also weights it.
#[inline]
pub fn blend_px(mode: i32, bg: [f32; 4], fg: [f32; 4], opacity: f32, use_alpha: bool, clamp: bool) -> [f32; 4] {
    let t = opacity * if use_alpha { fg[3] } else { 1.0 };
    let mut o = [0.0; 4];
    for k in 0..3 {
        let mut v = blend_channel(mode, bg[k], fg[k]);
        if clamp {
            v = v.clamp(0.0, 1.0);
        }
        o[k] = bg[k] + (v - bg[k]) * t;
    }
    o[3] = if mode == 0 || mode == 7 { bg[3] + (fg[3] - bg[3]) * opacity } else { bg[3] };
    o
}

/// A gray image of the output size from per-pixel values.
pub fn gray_from(w: u32, h: u32, v: Vec<f32>) -> Image {
    Image { w, h, kind: Kind::Gray, data: Arc::new(v) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_dodge_and_burn_keep_black_and_white_backgrounds() {
        const DODGE: i32 = 13;
        const BURN: i32 = 14;
        // (background, foreground, expected), W3C compositing formulas
        for (b, f, expected) in [(0.0, 1.0, 0.0), (0.0, 0.5, 0.0), (0.5, 1.0, 1.0), (0.25, 0.5, 0.5), (1.0, 0.75, 1.0)] {
            assert_eq!(blend_channel(DODGE, b, f), expected, "dodge b={b} f={f}");
        }
        for (b, f, expected) in [(1.0, 0.0, 1.0), (1.0, 0.5, 1.0), (0.5, 0.0, 0.0), (0.75, 0.5, 0.5), (0.0, 0.25, 0.0)] {
            assert_eq!(blend_channel(BURN, b, f), expected, "burn b={b} f={f}");
        }
    }

    #[test]
    fn blur_keeps_mean_and_tiles() {
        let img = Image::gray(32, 32, |x, y| if (x / 4 + y / 4) % 2 == 0 { 1.0 } else { 0.0 });
        for s in [1.0, 6.0] {
            let b = gaussian(&img, s, true);
            assert!((b.mean()[0] - img.mean()[0]).abs() < 1e-3, "sigma {s}");
        }
        let flat = gaussian(&Image::filled(8, 8, Kind::Color, [0.2, 0.4, 0.6, 1.0]), 4.0, false);
        assert!((flat.get(3, 3, false)[1] - 0.4).abs() < 1e-5);
    }

    #[test]
    fn gaussian_huge_sigma_keeps_row_mean_on_narrow_image() {
        let img = Image::gray(8, 32, |x, _y| if x == 0 { 1.0 } else { 0.0 });
        let b = gaussian(&img, 1000.0, false);
        for y in 0..32i64 {
            for x in 0..8i64 {
                assert!((b.get(x, y, false)[0] - 0.125).abs() < 0.05, "pixel {x},{y}");
            }
        }
    }

    #[test]
    fn edt_exact() {
        let d = distance_sq(9, 1, |i| i == 4, false);
        assert_eq!(d, vec![16.0, 9.0, 4.0, 1.0, 0.0, 1.0, 4.0, 9.0, 16.0]);
        let t = distance_sq(9, 1, |i| i == 0, true);
        assert_eq!(t[8], 1.0);
    }

    #[test]
    fn edt_sites_match_brute_force() {
        // Exhaust all small masks, including ties and empty/full masks. Larger rectangles use
        // deterministic pseudo-random masks; the oracle measures every site independently.
        for (w, h) in [(1usize, 1usize), (1, 7), (7, 1), (2, 3), (3, 3), (5, 7), (8, 6)] {
            let cases = if w * h <= 9 { 1usize << (w * h) } else { 128 };
            for case in 0..cases {
                let mut state = case as u32 + 1;
                let mask: Vec<bool> = (0..w * h)
                    .map(|i| {
                        if w * h <= 9 {
                            case & (1 << i) != 0
                        } else {
                            state = state.wrapping_mul(1664525).wrapping_add(1013904223);
                            state.is_multiple_of(7)
                        }
                    })
                    .collect();
                for tile in [false, true] {
                    let (d, sites) = distance_sq_with_sites(w as u32, h as u32, |i| mask[i], tile);
                    assert_eq!(d, distance_sq(w as u32, h as u32, |i| mask[i], tile));
                    for i in 0..w * h {
                        let distance = |site: usize| {
                            let dx = (i % w).abs_diff(site % w);
                            let dy = (i / w).abs_diff(site / w);
                            let (dx, dy) = if tile { (dx.min(w - dx), dy.min(h - dy)) } else { (dx, dy) };
                            (dx * dx + dy * dy) as f32
                        };
                        let expected = (0..mask.len()).filter(|&site| mask[site]).map(distance).reduce(f32::min);
                        if let Some(expected) = expected {
                            assert_eq!(d[i], expected, "{w}x{h}, mask {case}, tile {tile}, pixel {i}");
                            assert!(sites[i] < mask.len() && mask[sites[i]]);
                            assert_eq!(distance(sites[i]), expected);
                        } else {
                            assert_eq!(sites[i], usize::MAX);
                            assert!(d[i] >= DISTANCE_INF);
                        }
                    }
                }
            }
        }
        assert!(distance_sq_with_sites(0, 3, |_| true, true).0.is_empty());
        assert!(distance_sq_with_sites(3, 0, |_| true, false).1.is_empty());
    }

    #[test]
    fn hsl_round_trip() {
        let c = [0.8, 0.3, 0.1, 1.0];
        let h = rgb_to_hsl(c);
        let r = hsl_to_rgb(h[0], h[1], h[2]);
        for k in 0..3 {
            assert!((r[k] - c[k]).abs() < 1e-5);
        }
    }

    #[test]
    fn gaussian_tiled_matches_untiled_for_oversized_radius() {
        let img = Image::gray(4, 5, |x, y| ((x * 3 + y * 5) % 7) as f32 / 6.0);
        for sigma in [2.0f32, 20.0] {
            let tiled = gaussian(&img, sigma, true);
            let untiled = gaussian(&img, sigma, false);
            for y in 0..img.h as i64 {
                for x in 0..img.w as i64 {
                    let diff = (tiled.get(x, y, true)[0] - untiled.get(x, y, false)[0]).abs();
                    assert!(diff < 1e-4, "sigma {sigma} pixel {x},{y} diff {diff}");
                }
            }
        }
    }

    #[test]
    fn small_sigma_convolve_preserves_mean_on_narrow_image() {
        // Impulse far enough from the edges that every affected tap is in range, so the
        // renormalised convolve_1d passes must preserve the total (row/column) mean.
        let img = Image::gray(10, 12, |x, y| if x == 5 && y == 6 { 1.0 } else { 0.0 });
        let b = gaussian(&img, 0.6, false);
        assert!((b.mean()[0] - img.mean()[0]).abs() < 1e-5, "mean {} vs {}", b.mean()[0], img.mean()[0]);
        let row_sum: f32 = (0..10i64).map(|x| b.get(x, 6, false)[0]).sum();
        let col_sum: f32 = (0..12i64).map(|y| b.get(5, y, false)[0]).sum();
        assert!(row_sum > 0.0 && row_sum < 1.0, "row sum {row_sum}");
        assert!(col_sum > 0.0 && col_sum < 1.0, "col sum {col_sum}");
        // Edge renormalisation: a constant narrow image stays constant at the border.
        let flat = gaussian(&Image::filled(6, 9, Kind::Gray, [0.5, 0.0, 0.0, 0.0]), 1.0, false);
        for y in 0..9i64 {
            for x in 0..6i64 {
                assert!((flat.get(x, y, false)[0] - 0.5).abs() < 1e-5, "pixel {x},{y}");
            }
        }
    }
}
