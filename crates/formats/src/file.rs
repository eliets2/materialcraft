#[cfg(not(target_arch = "wasm32"))]
use crate::{FormatError, Result};

pub fn safe_file_name(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
    let cleaned: String = base.chars().map(|c| if c.is_alphanumeric() || "._- ".contains(c) { c } else { '_' }).collect();
    let cleaned = cleaned.trim_start_matches('.').to_string();
    if cleaned.is_empty() { "resource".into() } else { cleaned }
}

/// The file stem of an exported output: `pattern` with `$(graph)`, `$(identifier)` and `$(usage)` replaced, sanitized for file systems
pub fn output_name(pattern: &str, graph: &str, identifier: &str, usage: &str) -> String {
    let s = pattern.replace("$(graph)", graph).replace("$(identifier)", identifier).replace("$(usage)", usage);
    safe_file_name(&s)
}

/// Write `bytes` to `path` through a temporary file and a rename, so a failed write never leaves a truncated file behind
#[cfg(not(target_arch = "wasm32"))]
pub fn write_atomic(path: &std::path::Path, bytes: &[u8]) -> Result<()> {
    let tmp = path.with_extension(format!("{}.tmp", path.extension().and_then(|e| e.to_str()).unwrap_or("out")));
    std::fs::write(&tmp, bytes).map_err(|e| FormatError::Io(format!("{}: {e}", tmp.display())))?;
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        FormatError::Io(format!("{}: {e}", path.display()))
    })
}
