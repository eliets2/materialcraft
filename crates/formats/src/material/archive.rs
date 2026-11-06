use std::io::{Cursor, Read, Write};

use super::Manifest;
use materialcraft_core::Value;

use crate::{FormatError, MAX_JSON, MAX_RESOURCE, Result};

/// The portable material extension
pub const MATERIAL_EXT: &str = "mcmat";
/// `format` of every manifest
pub const FORMAT: &str = "craft-material";
/// Newest manifest version this build reads and writes
pub const VERSION: u32 = 2;
const MANIFEST: &str = "material.json";
const SOURCE: &str = "source.sbs";

/// Whether maps of `usage` hold colors (stored sRGB-encoded) rather than data (linear)
pub fn srgb_usage(usage: &str) -> bool {
    matches!(
        usage,
        "baseColor"
            | "diffuse"
            | "emissive"
            | "specular"
            | "specularEdgeColor"
            | "scatteringColor"
            | "absorptionColor"
            | "coatColor"
            | "sheenColor"
            | "reflection"
            | "ambient"
    )
}

/// Whether other apps can set a value of this type from the manifest
pub fn portable(v: &Value) -> bool {
    matches!(
        v,
        Value::Float(_)
            | Value::Float2(_)
            | Value::Float3(_)
            | Value::Float4(_)
            | Value::Int(_)
            | Value::Int2(_)
            | Value::Bool(_)
            | Value::Color(_)
            | Value::Enum(_)
    )
}

/// Write a portable material: `manifest` (its `source` is set here), the encoded maps named by `manifest.maps[].file`, and the source package bytes
pub fn write_material(manifest: &Manifest, maps: &[(String, Vec<u8>)], source: &[u8]) -> Result<Vec<u8>> {
    let io = |e: zip::result::ZipError| FormatError::Io(e.to_string());
    let manifest = Manifest { source: SOURCE.into(), ..manifest.clone() };
    let json = serde_json::to_vec_pretty(&manifest).map_err(|e| FormatError::Io(e.to_string()))?;
    let mut buf = Cursor::new(Vec::new());
    {
        let mut z = zip::ZipWriter::new(&mut buf);
        let deflate = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        // Maps and the package are already compressed
        let stored = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
        z.start_file(MANIFEST, deflate).map_err(io)?;
        z.write_all(&json).map_err(|e| FormatError::Io(e.to_string()))?;
        for (file, bytes) in maps {
            z.start_file(file.as_str(), stored).map_err(io)?;
            z.write_all(bytes).map_err(|e| FormatError::Io(e.to_string()))?;
        }
        z.start_file(SOURCE, stored).map_err(io)?;
        z.write_all(source).map_err(|e| FormatError::Io(e.to_string()))?;
        z.finish().map_err(io)?;
    }
    Ok(buf.into_inner())
}

/// Read a portable material's manifest and source package bytes
pub fn read_material(bytes: &[u8]) -> Result<(Manifest, Vec<u8>)> {
    let bad = |m: String| FormatError::BadPackage(format!("portable material: {m}"));
    let mut z = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| bad(e.to_string()))?;
    let mut read = |name: &str, cap: u64| -> Result<Vec<u8>> {
        let entry = z.by_name(name).map_err(|_| bad(format!("{name} is missing")))?;
        if entry.size() > cap {
            return Err(bad(format!("{name} exceeds {cap} bytes")));
        }
        let mut data = Vec::new();
        entry.take(cap + 1).read_to_end(&mut data).map_err(|e| FormatError::Io(e.to_string()))?;
        if data.len() as u64 > cap {
            return Err(bad(format!("{name} exceeds {cap} bytes")));
        }
        Ok(data)
    };

    let manifest: Manifest = serde_json::from_slice(&read(MANIFEST, MAX_JSON)?).map_err(|e| bad(e.to_string()))?;
    if manifest.format != FORMAT || manifest.version != VERSION {
        return Err(bad(format!("unsupported {} version {}", manifest.format, manifest.version)));
    }
    let source = read(&manifest.source, MAX_RESOURCE)?;
    Ok((manifest, source))
}

/// Whether `bytes` look like a portable material (a zip with `material.json`)
pub fn is_material(bytes: &[u8]) -> bool {
    zip::ZipArchive::new(Cursor::new(bytes)).is_ok_and(|mut z| z.by_name(MANIFEST).is_ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::material::{ImageInput, MapEntry, Parameter};
    use materialcraft_core::ExposedParameter;

    fn manifest() -> Manifest {
        let param = ExposedParameter {
            id: "rough".into(),
            label: "Roughness".into(),
            group: String::new(),
            default: Value::Float(0.5),
            min: 0.0,
            max: 1.0,
            options: Vec::new(),
        };
        Manifest {
            format: FORMAT.into(),
            version: VERSION,
            name: "Bricks".into(),
            graph: "Bricks".into(),
            size: [64, 64],
            maps: vec![MapEntry { usage: "baseColor".into(), file: "maps/baseColor.png".into(), color_space: "srgb".into() }],
            parameters: vec![Parameter { param, value: Value::Float(0.25) }],
            inputs: vec![ImageInput { identifier: "curvature".into(), kind: "gray".into() }],
            presets: Vec::new(),
            source: String::new(),
        }
    }

    #[test]
    fn round_trips_and_rejects_newer_versions() {
        let m = manifest();
        let bytes = write_material(&m, &[("maps/baseColor.png".into(), vec![1, 2, 3])], b"pkg").unwrap();
        assert!(is_material(&bytes));
        let (back, source) = read_material(&bytes).unwrap();
        assert_eq!(back, Manifest { source: SOURCE.into(), ..m.clone() }, "the writer names the source entry");
        assert_eq!(source, b"pkg");
        // Parameters are flat objects with the value in the API's typed form
        let json = serde_json::to_value(&m).unwrap();
        assert_eq!(json["parameters"][0]["id"], "rough");
        assert_eq!(json["parameters"][0]["value"], serde_json::json!({"t": "float", "v": 0.25}));

        let newer = Manifest { version: VERSION + 1, ..m };
        assert!(read_material(&write_material(&newer, &[], b"").unwrap()).is_err());
        assert!(!is_material(b"not a zip"));
        assert!(read_material(b"garbage").is_err());
    }
}
