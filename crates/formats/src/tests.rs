use super::*;
use materialcraft_image::{Image, Kind, srgb_to_linear};
use std::io::Cursor;

#[test]
fn export_every_format() {
    let img = Image::generate(8, 8, Kind::Color, |x, y| [x as f32 / 7.0, y as f32 / 7.0, 0.5, 1.0]);
    let gray = img.to_gray();
    for (ext, _) in EXPORT_FORMATS {
        for depth in [Depth::U8, Depth::U16, Depth::F32] {
            for i in [&img, &gray] {
                let b = encode(i, ext, depth, true).unwrap_or_else(|e| panic!("{ext}: {e}"));
                let back = decode(&b, false).unwrap_or_else(|e| panic!("{ext} decode: {e}"));
                assert_eq!((back.w, back.h), (8, 8));
            }
        }
    }
    let png16 = encode(&gray, "png", Depth::U16, false).unwrap();
    assert!((decode(&png16, false).unwrap().get(7, 0, false)[0] - gray.get(7, 0, false)[0]).abs() < 1e-3);
}

#[test]
fn names_are_safe() {
    assert_eq!(output_name("$(graph)_$(usage)", "Bricks", "out", "baseColor"), "Bricks_baseColor");
    assert_eq!(safe_file_name("../../etc/passwd"), "passwd");
    assert_eq!(safe_file_name(".."), "resource");
}

#[test]
fn oversized_images_are_refused_before_decoding() {
    let mut png = Cursor::new(Vec::new());
    ::image::GrayImage::new(8193, 1).write_to(&mut png, ::image::ImageFormat::Png).unwrap();
    let err = decode(png.get_ref(), false).unwrap_err().to_string();
    assert!(err.contains("8192"), "{err}");
}

#[test]
fn tga_still_decodes() {
    let mut tga = Cursor::new(Vec::new());
    ::image::RgbaImage::new(2, 2).write_to(&mut tga, ::image::ImageFormat::Tga).unwrap();
    let img = decode(tga.get_ref(), false).unwrap();
    assert_eq!((img.w, img.h, img.kind), (2, 2, Kind::Color));
}

#[test]
fn decode_matches_previous_values() {
    let mut png = Cursor::new(Vec::new());
    ::image::RgbImage::from_pixel(2, 2, ::image::Rgb([128, 128, 128])).write_to(&mut png, ::image::ImageFormat::Png).unwrap();
    let img = decode(png.get_ref(), false).unwrap();
    let want = srgb_to_linear(128.0 / 255.0);
    assert!((img.get(1, 1, false)[0] - want).abs() < 1e-4);
}
