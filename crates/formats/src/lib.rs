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
pub use image::{decode, encode};

pub type Result<T> = std::result::Result<T, FormatError>;
pub(crate) const MAX_JSON: u64 = 64 << 20;
pub(crate) const MAX_RESOURCE: u64 = 256 << 20;
pub const EXPORT_FORMATS: &[(&str, &str)] =
    &[("png", "PNG"), ("jpg", "JPEG"), ("tga", "TGA"), ("tif", "TIFF"), ("bmp", "BMP"), ("webp", "WEBP"), ("exr", "OpenEXR"), ("hdr", "Radiance HDR")];
pub const IMAGE_EXTENSIONS: &[&str] = &["svg", "png", "jpg", "jpeg", "tga", "tif", "tiff", "bmp", "webp", "exr", "hdr"];

#[cfg(test)]
mod tests;
