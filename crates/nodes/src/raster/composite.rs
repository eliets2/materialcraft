use materialcraft_image::{Image, Kind};

use crate::CookContext;

pub(crate) fn composite(context: &CookContext, foreground: impl Fn(u32, u32) -> [f32; 4] + Sync) -> Image {
    let background = context.any_or(0, Kind::Color, context.v4("background"));
    let image = Image::generate(context.w, context.h, Kind::Color, |x, y| {
        let foreground = foreground(x, y);
        let background = background.get(i64::from(x), i64::from(y), false);
        let foreground_alpha = foreground[3].clamp(0.0, 1.0);
        let background_alpha = background[3].clamp(0.0, 1.0) * (1.0 - foreground_alpha);
        let alpha = foreground_alpha + background_alpha;
        if alpha <= 0.0 {
            return [0.0; 4];
        }

        let channel = |index: usize| (foreground[index] * foreground_alpha + background[index] * background_alpha) / alpha;
        [channel(0), channel(1), channel(2), alpha]
    });

    if context.b("color_mode") { image } else { image.to_gray() }
}
