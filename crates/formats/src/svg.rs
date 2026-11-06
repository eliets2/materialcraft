use crate::{FormatError, Result};
use materialcraft_image::{Image, Kind, MAX_SIDE, srgb_to_linear};
use resvg::{tiny_skia, usvg};

pub(crate) fn decode(bytes: &[u8]) -> Result<Image> {
    if bytes.len() > 16 << 20 {
        return Err(FormatError::Image("SVG exceeds the 16 MiB limit".into()));
    }

    let mut options = usvg::Options::default();
    options.image_href_resolver.resolve_string = Box::new(|_, _| None);
    let tree = usvg::Tree::from_data(bytes, &options).map_err(|error| FormatError::Image(error.to_string()))?;
    let size = tree.size().to_int_size();
    let (width, height) = (size.width(), size.height());
    if width > MAX_SIDE || height > MAX_SIDE {
        return Err(FormatError::Image(format!("SVG exceeds {MAX_SIDE} pixels")));
    }

    let mut pixels = tiny_skia::Pixmap::new(width, height).ok_or_else(|| FormatError::Image("could not allocate SVG image".into()))?;
    resvg::render(&tree, tiny_skia::Transform::identity(), &mut pixels.as_mut());
    Ok(Image::generate(width, height, Kind::Color, |x, y| {
        let pixel = pixels.pixel(x, y).map(|pixel| pixel.demultiply());
        pixel.map_or([0.0; 4], |pixel| {
            [
                srgb_to_linear(f32::from(pixel.red()) / 255.0),
                srgb_to_linear(f32::from(pixel.green()) / 255.0),
                srgb_to_linear(f32::from(pixel.blue()) / 255.0),
                f32::from(pixel.alpha()) / 255.0,
            ]
        })
    }))
}
