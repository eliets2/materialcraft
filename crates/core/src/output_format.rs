use serde::{Deserialize, Serialize};

/// The output bit depth ("Output format")
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OutputFormat {
    /// The main input's format; 16 bits for generators
    #[default]
    Inherit,
    U8,
    U16,
    F16,
    F32,
}

impl OutputFormat {
    pub const ALL: [OutputFormat; 5] = [OutputFormat::Inherit, OutputFormat::U8, OutputFormat::U16, OutputFormat::F16, OutputFormat::F32];

    pub fn label(self) -> &'static str {
        match self {
            OutputFormat::Inherit => "Relative to input",
            OutputFormat::U8 => "8 bits per channel",
            OutputFormat::U16 => "16 bits per channel",
            OutputFormat::F16 => "16 bits per channel float (HDR)",
            OutputFormat::F32 => "32 bits per channel float (HDR)",
        }
    }

    /// The suffix of a node's size label: `8`, `16`, `16F`, `32F`
    pub fn bits(self) -> &'static str {
        match self {
            OutputFormat::Inherit | OutputFormat::U16 => "16",
            OutputFormat::U8 => "8",
            OutputFormat::F16 => "16F",
            OutputFormat::F32 => "32F",
        }
    }
}
