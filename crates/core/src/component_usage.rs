/// Input/output semantics shared by node properties, SBS and texture exports
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ComponentUsage {
    pub id: &'static str,
    pub label: &'static str,
}

impl ComponentUsage {
    pub const ANY: Self = Self { id: "any", label: "Any" };
    pub const ANY_INDEX: u32 = 9;

    // Existing indices are saved in documents; append new entries without reordering
    pub const ALL: &'static [Self] = &[
        Self { id: "baseColor", label: "Base color" },
        Self { id: "normal", label: "Normal" },
        Self { id: "roughness", label: "Roughness" },
        Self { id: "metallic", label: "Metallic" },
        Self { id: "height", label: "Height" },
        Self { id: "ambientOcclusion", label: "Ambient occlusion" },
        Self { id: "emissive", label: "Emissive" },
        Self { id: "opacity", label: "Opacity" },
        Self { id: "specular", label: "Specular" },
        Self::ANY,
        Self { id: "diffuse", label: "Diffuse" },
        Self { id: "glossiness", label: "Glossiness" },
        Self { id: "specularLevel", label: "Specular level" },
        Self { id: "specularEdgeColor", label: "Specular edge color" },
        Self { id: "anisotropyLevel", label: "Anisotropy level" },
        Self { id: "anisotropyAngle", label: "Anisotropy angle" },
        Self { id: "IOR", label: "Index of refraction" },
        Self { id: "scattering", label: "Scattering" },
        Self { id: "scatteringColor", label: "Scattering color" },
        Self { id: "scatteringDistance", label: "Scattering distance" },
        Self { id: "scatteringDistanceScale", label: "Scattering distance scale" },
        Self { id: "transmissive", label: "Transmissive" },
        Self { id: "translucency", label: "Translucency" },
        Self { id: "absorptionColor", label: "Absorption color" },
        Self { id: "absorptionDistance", label: "Absorption distance" },
        Self { id: "coatOpacity", label: "Coat opacity" },
        Self { id: "coatColor", label: "Coat color" },
        Self { id: "coatRoughness", label: "Coat roughness" },
        Self { id: "coatSpecularLevel", label: "Coat specular level" },
        Self { id: "coatNormal", label: "Coat normal" },
        Self { id: "coatIOR", label: "Coat index of refraction" },
        Self { id: "sheenOpacity", label: "Sheen opacity" },
        Self { id: "sheenColor", label: "Sheen color" },
        Self { id: "sheenRoughness", label: "Sheen roughness" },
        Self { id: "reflection", label: "Reflection" },
        Self { id: "displacement", label: "Displacement" },
        Self { id: "ambient", label: "Ambient" },
        Self { id: "mask", label: "Mask" },
        Self { id: "blendingmask", label: "Blending mask" },
        Self { id: "bump", label: "Bump" },
        Self { id: "user0", label: "User 0" },
        Self { id: "user1", label: "User 1" },
        Self { id: "user2", label: "User 2" },
        Self { id: "user3", label: "User 3" },
        Self { id: "user4", label: "User 4" },
        Self { id: "user5", label: "User 5" },
        Self { id: "user6", label: "User 6" },
        Self { id: "user7", label: "User 7" },
    ];

    pub const LABELS: &'static [&'static str] = &{
        let mut labels = [""; Self::ALL.len()];
        let mut index = 0;
        while index < labels.len() {
            labels[index] = Self::ALL[index].label;
            index += 1;
        }
        labels
    };

    pub fn from_index(index: u32) -> Self {
        Self::ALL.get(index as usize).copied().unwrap_or(Self::ANY)
    }

    pub fn index(id: &str) -> u32 {
        Self::ALL.iter().position(|usage| usage.id.eq_ignore_ascii_case(id.trim())).map_or(Self::ANY_INDEX, |index| index as u32)
    }
}
