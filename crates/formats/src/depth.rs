use materialcraft_core::OutputFormat;

/// Bit depth for export
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Depth {
    U8,
    U16,
    F32,
}

impl Depth {
    pub fn from_format(f: OutputFormat) -> Depth {
        match f {
            OutputFormat::U8 => Depth::U8,
            OutputFormat::F16 | OutputFormat::F32 => Depth::F32,
            _ => Depth::U16,
        }
    }
}
