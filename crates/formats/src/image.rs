use crate::{Depth, FormatError, Result, guard};
use materialcraft_image::{Image, Kind, MAX_SIDE, linear_to_srgb, srgb_to_linear};
use std::io::Cursor;
use std::sync::Arc;

/// Decode image file bytes into a linear float image (8-bit color is sRGB-decoded; grayscale files stay grayscale)
pub fn decode(bytes: &[u8], linear: bool) -> Result<Image> {
    if std::str::from_utf8(bytes.get(..bytes.len().min(4096)).unwrap_or_default()).is_ok_and(|text| text.contains("<svg")) {
        return guard(|| crate::svg::decode(bytes), || FormatError::Image("SVG decoder failed".into()));
    }

    let open = || -> Result<image::ImageReader<Cursor<&[u8]>>> {
        let mut reader = image::ImageReader::new(Cursor::new(bytes)).with_guessed_format().map_err(|e| FormatError::Image(e.to_string()))?;
        if reader.format().is_none() {
            reader.set_format(image::ImageFormat::Tga);
        }
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(MAX_SIDE);
        limits.max_image_height = Some(MAX_SIDE);
        limits.max_alloc = Some(1 << 30);
        reader.limits(limits);
        Ok(reader)
    };
    let failed = || FormatError::Image("the decoder failed on this file".into());
    let image_error = |e| match e {
        image::ImageError::Limits(_) => FormatError::Image(format!("image is larger than {MAX_SIDE} pixels")),
        e => FormatError::Image(e.to_string()),
    };
    let (w, h) = guard(|| open()?.into_dimensions().map_err(image_error), failed)?;
    if w == 0 || h == 0 || w > MAX_SIDE || h > MAX_SIDE {
        return Err(FormatError::Image(format!("{w}x{h} is outside 1..={MAX_SIDE} pixels")));
    }
    let img = guard(|| open()?.decode().map_err(image_error), failed)?;
    let gray = matches!(img.color(), image::ColorType::L8 | image::ColorType::L16 | image::ColorType::La8 | image::ColorType::La16);
    let float = matches!(img.color(), image::ColorType::Rgb32F | image::ColorType::Rgba32F);
    if gray {
        let l = img.into_luma16();
        let data: Vec<f32> = l.pixels().map(|p| p.0[0] as f32 / 65535.0).collect();
        return Ok(Image { w, h, kind: Kind::Gray, data: Arc::new(data) });
    }
    let mut data = img.into_rgba32f().into_raw();
    if !(float || linear) {
        for px in data.chunks_exact_mut(4) {
            for c in px.iter_mut().take(3) {
                *c = srgb_to_linear(*c);
            }
        }
    }
    Ok(Image { w, h, kind: Kind::Color, data: Arc::new(data) })
}

/// Encode `img` as `ext` (see [`EXPORT_FORMATS`]). `srgb`: color values are written through the sRGB curve (base color); data maps (normal, roughness…) are written linear
pub fn encode(img: &Image, ext: &str, depth: Depth, srgb: bool) -> Result<Vec<u8>> {
    let ext = ext.trim_start_matches('.').to_ascii_lowercase();
    let enc = |v: f32| if srgb && img.kind == Kind::Color { linear_to_srgb(v) } else { v };
    let (w, h) = (img.w, img.h);
    let color = img.kind == Kind::Color;
    let px = |i: usize| -> [f32; 4] {
        if color {
            let s = img.data.get(i * 4..i * 4 + 4).unwrap_or(&[0.0, 0.0, 0.0, 1.0]);
            [enc(s[0]), enc(s[1]), enc(s[2]), s[3]]
        } else {
            let v = img.data.get(i).copied().unwrap_or(0.0);
            [v, v, v, 1.0]
        }
    };

    let n = (w * h) as usize;
    let fmt = match ext.as_str() {
        "png" => image::ImageFormat::Png,
        "jpg" | "jpeg" => image::ImageFormat::Jpeg,
        "tga" => image::ImageFormat::Tga,
        "tif" | "tiff" => image::ImageFormat::Tiff,
        "bmp" => image::ImageFormat::Bmp,
        "webp" => image::ImageFormat::WebP,
        "exr" => image::ImageFormat::OpenExr,
        "hdr" => image::ImageFormat::Hdr,
        other => return Err(FormatError::Image(format!("unsupported export format `{other}`"))),
    };

    let float_only = matches!(fmt, image::ImageFormat::OpenExr | image::ImageFormat::Hdr);
    let sixteen_ok = matches!(fmt, image::ImageFormat::Png | image::ImageFormat::Tiff);
    let q8 = |v: f32| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
    let q16 = |v: f32| (v.clamp(0.0, 1.0) * 65535.0 + 0.5) as u16;
    let dynimg: image::DynamicImage = if float_only {
        let data: Vec<f32> = (0..n)
            .flat_map(|i| {
                let p = px(i);
                [p[0], p[1], p[2]]
            })
            .collect();
        let buf = image::Rgb32FImage::from_raw(w, h, data).ok_or_else(|| FormatError::Image("bad buffer".into()))?;
        buf.into()
    } else if depth != Depth::U8 && sixteen_ok {
        if color {
            let data: Vec<u16> = (0..n).flat_map(|i| px(i).map(q16)).collect();
            image::ImageBuffer::<image::Rgba<u16>, _>::from_raw(w, h, data).ok_or_else(|| FormatError::Image("bad buffer".into()))?.into()
        } else {
            let data: Vec<u16> = (0..n).map(|i| q16(px(i)[0])).collect();
            image::ImageBuffer::<image::Luma<u16>, _>::from_raw(w, h, data).ok_or_else(|| FormatError::Image("bad buffer".into()))?.into()
        }
    } else if color && !matches!(fmt, image::ImageFormat::Jpeg) {
        let data: Vec<u8> = (0..n).flat_map(|i| px(i).map(q8)).collect();
        image::RgbaImage::from_raw(w, h, data).ok_or_else(|| FormatError::Image("bad buffer".into()))?.into()
    } else if color {
        let data: Vec<u8> = (0..n)
            .flat_map(|i| {
                let p = px(i);
                [q8(p[0]), q8(p[1]), q8(p[2])]
            })
            .collect();
        image::RgbImage::from_raw(w, h, data).ok_or_else(|| FormatError::Image("bad buffer".into()))?.into()
    } else {
        let data: Vec<u8> = (0..n).map(|i| q8(px(i)[0])).collect();
        image::GrayImage::from_raw(w, h, data).ok_or_else(|| FormatError::Image("bad buffer".into()))?.into()
    };

    let mut out = Cursor::new(Vec::new());
    dynimg.write_to(&mut out, fmt).map_err(|e| FormatError::Image(e.to_string()))?;
    Ok(out.into_inner())
}
