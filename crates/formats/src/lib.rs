#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod material;
pub mod mesh;
pub mod sbs;
mod svg;

mod depth;
mod file;
mod format_error;
mod guard;
mod image;

pub use depth::Depth;
#[cfg(not(target_arch = "wasm32"))]
pub use file::write_atomic;
pub use file::{output_name, safe_file_name};
pub use format_error::FormatError;
pub(crate) use guard::guard;
pub use image::encode;

/// Decode image bytes after checking header dimensions.
pub fn decode(bytes: &[u8], linear: bool) -> Result<materialcraft_image::Image> {
    check_header_dimensions(bytes)?;
    image::decode(bytes, linear)
}

/// Reject dimensions declared in an image header before full decoding.
fn check_header_dimensions(bytes: &[u8]) -> Result<()> {
    use materialcraft_image::MAX_SIDE;
    let be32 = |off: usize| -> Option<u32> {
        let b = bytes.get(off..off + 4)?;
        Some(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    };
    let le32 = |off: usize| -> Option<u32> {
        let b = bytes.get(off..off + 4)?;
        Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    };
    let le16 = |off: usize| -> Option<u32> {
        let b = bytes.get(off..off + 2)?;
        Some(u16::from_le_bytes([b[0], b[1]]) as u32)
    };
    let dims = if bytes.starts_with(b"\x89PNG\r\n\x1a\n") && be32(12) == Some(0x4948_4452) {
        match (be32(16), be32(20)) {
            (Some(w), Some(h)) => Some((w, h)),
            _ => None,
        }
    } else if bytes.starts_with(b"GIF8") {
        match (le16(6), le16(8)) {
            (Some(w), Some(h)) => Some((w, h)),
            _ => None,
        }
    } else if bytes.starts_with(b"BM") {
        match (le32(18), le32(22)) {
            (Some(w), Some(h)) => Some((w, (h as i32).unsigned_abs())),
            _ => None,
        }
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        let mut off = 2usize;
        let mut found = None;
        while let (Some(marker), Some(len)) = (bytes.get(off), le16(off + 2)) {
            if *marker != 0xFF {
                break;
            }
            if matches!(marker, 0xC0 | 0xC1 | 0xC2) {
                found = match (le16(off + 5), le16(off + 7)) {
                    (Some(h), Some(w)) => Some((w, h)),
                    _ => None,
                };
                break;
            }
            if *marker == 0xDA || len < 2 {
                break;
            }
            off += 2 + len as usize;
        }
        found
    } else {
        None
    };
    if let Some((w, h)) = dims {
        if w == 0 || h == 0 || w > MAX_SIDE || h > MAX_SIDE {
            return Err(FormatError::Image(format!("{w}x{h} is outside 1..={MAX_SIDE} pixels")));
        }
    }
    Ok(())
}

pub type Result<T> = std::result::Result<T, FormatError>;
pub(crate) const MAX_JSON: u64 = 64 << 20;
pub(crate) const MAX_RESOURCE: u64 = 256 << 20;
pub const EXPORT_FORMATS: &[(&str, &str)] =
    &[("png", "PNG"), ("jpg", "JPEG"), ("tga", "TGA"), ("tif", "TIFF"), ("bmp", "BMP"), ("webp", "WEBP"), ("exr", "OpenEXR"), ("hdr", "Radiance HDR")];
pub const IMAGE_EXTENSIONS: &[&str] = &["svg", "png", "jpg", "jpeg", "tga", "tif", "tiff", "bmp", "webp", "exr", "hdr"];

#[cfg(test)]
mod tests;

#[cfg(test)]
mod header_dimension_tests {
    use super::decode;
    use std::io::Cursor;

    #[test]
    fn decode_rejects_oversized_header_dimensions() {
        let mut png = Cursor::new(Vec::new());
        ::image::GrayImage::new(1, 1).write_to(&mut png, ::image::ImageFormat::Png).unwrap();
        let mut bytes = png.into_inner();
        assert!(bytes.len() >= 24, "PNG has no IHDR");
        bytes[16..20].copy_from_slice(&8192u32.to_be_bytes());
        bytes[20..24].copy_from_slice(&8192u32.to_be_bytes());
        bytes.truncate(33);
        let err = decode(&bytes, false).unwrap_err().to_string();
        assert!(err.contains("8192"), "{err}");
    }
}
