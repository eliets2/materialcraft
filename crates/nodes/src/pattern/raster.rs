use materialcraft_image::{Image, Kind};
use std::sync::Arc;

use super::{pattern_stamp::PatternStamp, shape};
use crate::CookContext;

const MAX_SAMPLES: u64 = 256_000_000;

pub(super) fn render(context: &CookContext, stamps: &[PatternStamp]) -> Result<Image, String> {
    let (width, height) = materialcraft_image::clamp_size(context.w, context.h);
    let kind = if context.b("color_mode") { Kind::Color } else { Kind::Gray };
    let mut image =
        context.input(0).map(|image| image.resize(width, height, context.tile)).unwrap_or_else(|| Image::filled(width, height, kind, context.v4("background")));
    image = if kind == Kind::Gray { image.to_gray() } else { image.to_color() };
    let channels = kind.channels();
    let opacity = context.f("opacity").max(0.0);
    let pixels = Arc::make_mut(&mut image.data);
    let mut budget = MAX_SAMPLES;

    for stamp in stamps {
        if !stamp.center.iter().chain(&stamp.size).chain(&stamp.color).chain([&stamp.rotation, &stamp.variation]).all(|value| value.is_finite()) {
            return Err("pattern parameters must be finite".into());
        }
        if stamp.size.iter().any(|size| size.abs() < 1e-8) {
            continue;
        }
        if !matches!(stamp.blend, 0..=2) {
            return Err(format!("unsupported pattern blending mode {}", stamp.blend));
        }
        if !(2..=18).contains(&stamp.pattern) && !(256..256 + crate::MAX_IMAGE_INPUTS as i32).contains(&stamp.pattern) {
            return Err(format!("unsupported pattern {}", stamp.pattern));
        }

        let (sine, cosine) = (stamp.rotation * std::f32::consts::TAU).sin_cos();
        let radius = [
            (stamp.size[0] * cosine).abs() * 0.5 + (stamp.size[1] * sine).abs() * 0.5,
            (stamp.size[0] * sine).abs() * 0.5 + (stamp.size[1] * cosine).abs() * 0.5,
        ];
        let center = if context.tile { stamp.center.map(|value| value.rem_euclid(1.0)) } else { stamp.center };
        let bounds = |axis: usize, side: u32| {
            let first = ((center[axis] - radius[axis]) * side as f32).floor() as i64;
            let last = ((center[axis] + radius[axis]) * side as f32).ceil() as i64;
            if context.tile { (first, last) } else { (first.max(0), last.min(i64::from(side))) }
        };

        let (left, right) = bounds(0, width);
        let (top, bottom) = bounds(1, height);
        let samples =
            (right.saturating_sub(left).max(0) as u64).checked_mul(bottom.saturating_sub(top).max(0) as u64).ok_or("pattern bounds exceed the pixel budget")?;
        budget = budget.checked_sub(samples).ok_or("pattern rendering exceeded its pixel budget")?;

        for y in top..bottom {
            for x in left..right {
                let delta = [(x as f32 + 0.5) / width as f32 - center[0], (y as f32 + 0.5) / height as f32 - center[1]];
                let local = [(delta[0] * cosine + delta[1] * sine) * 2.0 / stamp.size[0], (-delta[0] * sine + delta[1] * cosine) * 2.0 / stamp.size[1]];
                if local.iter().any(|coordinate| coordinate.abs() > 1.0) {
                    continue;
                }

                let pattern = if stamp.pattern >= 256 {
                    let index = (stamp.pattern - 256) as usize + 1;
                    match context.input(index) {
                        Some(input) => {
                            let uv = local.map(|coordinate| coordinate * 0.5 + 0.5);
                            if stamp.filtering == 0 {
                                input.get((uv[0] * input.w as f32) as i64, (uv[1] * input.h as f32) as i64, false)
                            } else {
                                input.sample(uv[0], uv[1], false)
                            }
                        }
                        None => [0.0; 4],
                    }
                } else {
                    let coverage = shape::coverage(stamp.pattern, local[0], local[1], stamp.variation)?;
                    [coverage, coverage, coverage, coverage]
                };

                let pixel = (y.rem_euclid(i64::from(height)) as usize * width as usize + x.rem_euclid(i64::from(width)) as usize) * channels;
                let target = pixels.get_mut(pixel..pixel + channels).ok_or("invalid pattern pixel")?;
                for (channel, target) in target.iter_mut().enumerate() {
                    let alpha = if stamp.pattern >= 256 && !stamp.premultiplied && channel < 3 { pattern[3] } else { 1.0 };
                    let value = pattern[channel] * stamp.color[channel] * alpha * opacity;
                    *target = match stamp.blend {
                        1 => value + *target * (1.0 - (pattern[3] * stamp.color[3] * opacity).clamp(0.0, 1.0)),
                        2 => target.max(value),
                        _ => *target + value,
                    };
                    if !target.is_finite() {
                        return Err("pattern accumulation overflowed".into());
                    }
                }
            }
        }
    }
    Ok(image)
}
