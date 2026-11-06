use serde::{Deserialize, Serialize};

/// Texture channels supported by the material preview
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MaterialChannel {
    BaseColor,
    Normal,
    Roughness,
    Metallic,
    Height,
    AmbientOcclusion,
    Emissive,
    Opacity,
}

impl MaterialChannel {
    pub const ALL: [Self; 8] =
        [Self::BaseColor, Self::Normal, Self::Roughness, Self::Metallic, Self::Height, Self::AmbientOcclusion, Self::Emissive, Self::Opacity];

    pub fn id(self) -> &'static str {
        match self {
            Self::BaseColor => "baseColor",
            Self::Normal => "normal",
            Self::Roughness => "roughness",
            Self::Metallic => "metallic",
            Self::Height => "height",
            Self::AmbientOcclusion => "ambientOcclusion",
            Self::Emissive => "emissive",
            Self::Opacity => "opacity",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::BaseColor => "Base color",
            Self::Normal => "Normal",
            Self::Roughness => "Roughness",
            Self::Metallic => "Metallic",
            Self::Height => "Height",
            Self::AmbientOcclusion => "Ambient occlusion",
            Self::Emissive => "Emissive",
            Self::Opacity => "Opacity",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|channel| channel.id().eq_ignore_ascii_case(id)).or_else(|| id.eq_ignore_ascii_case("diffuse").then_some(Self::BaseColor))
    }
}
