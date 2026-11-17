//! Capability-based file access for untrusted automation paths (MCP, `serve`)

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use cap_std::ambient_authority;
use cap_std::fs::{Dir, OpenOptions};
use serde_json::Value;

use crate::budgets::MAX_INPUT_ENCODED_BYTES;
use crate::{AutomationError, bad};

/// Upper bound for one automation read, so a hostile path cannot force a multi-gigabyte allocation.
pub const MAX_READ_BYTES: u64 = 2 << 30;

/// Separate read and write directory capabilities. A missing authority fails closed
#[derive(Clone, Default)]
pub struct AuthorizedWorkspace {
    read: Option<Arc<Dir>>,
    read_path: Option<PathBuf>,
    write: Option<Arc<Dir>>,
    write_path: Option<PathBuf>,
}

impl AuthorizedWorkspace {
    pub fn new(read_root: Option<&Path>, write_root: Option<&Path>) -> Result<Self, AutomationError> {
        let read = open_root(read_root, "read")?;
        let read_path = read_root.map(std::fs::canonicalize).transpose().map_err(|e| bad(e.to_string()))?;
        let write = open_root(write_root, "write")?;
        let write_path = write_root.map(std::fs::canonicalize).transpose().map_err(|e| bad(e.to_string()))?;
        Ok(Self { read, read_path, write, write_path })
    }

    /// Engine identities are absolute. Actual dependency reads still go through the directory capability.
    pub(crate) fn document_path(&self, path: &str) -> Result<PathBuf, AutomationError> {
        Ok(self.read_path.as_ref().ok_or_else(|| denied("read"))?.join(relative_path(path)?))
    }

    /// Resolve a save once and keep its destination capability through serialization and writes.
    pub(crate) fn save_destination(&self, path: &str) -> Result<(PathBuf, Self, String), AutomationError> {
        let relative = relative_path(path)?;
        let root = self.write.as_ref().ok_or_else(|| denied("write"))?;
        let parent = relative.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));

        // Resolve directory symlinks inside the capability so rebased links use the correct depth
        let mut ancestor = parent;
        let mut suffix = PathBuf::new();
        let resolved_ancestor = loop {
            match root.canonicalize(ancestor) {
                Ok(resolved) => break resolved,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    // Defer directory creation until after serialization
                    // Reject dangling symlinks
                    match root.symlink_metadata(ancestor) {
                        Err(missing) if missing.kind() == std::io::ErrorKind::NotFound => {}
                        _ => return Err(file_error("resolve destination", path, error)),
                    }
                    let leaf = ancestor.file_name().ok_or_else(|| file_error("resolve destination", path, error))?;
                    suffix = Path::new(leaf).join(suffix);
                    ancestor = ancestor.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
                }
                Err(error) => return Err(file_error("resolve destination", path, error)),
            }
        };

        let leaf = relative.file_name().ok_or_else(|| bad("missing destination filename"))?;
        let relative = suffix.join(leaf);
        let save_path = relative.to_str().ok_or_else(|| bad("the save path must be UTF-8"))?.replace('\\', "/");
        let write_path = self.write_path.as_ref().ok_or_else(|| denied("write"))?.join(&resolved_ancestor);

        // Writes through this handle no longer traverse the original directory alias
        let dir = root.open_dir(&resolved_ancestor).map_err(|error| file_error("resolve destination", path, error))?;
        let destination = write_path.join(relative);
        let workspace = Self { write: Some(Arc::new(dir)), write_path: Some(write_path), ..Self::default() };
        Ok((destination, workspace, save_path))
    }

    pub(crate) fn read_dependency(&self, path: &str) -> Option<Vec<u8>> {
        if path.starts_with("sbs://") {
            return materialcraft_engine::formats::sbs::library::read(path).ok();
        }

        let relative = Path::new(path).strip_prefix(self.read_path.as_ref()?).ok()?;
        self.read(&relative.to_str()?.replace('\\', "/")).ok()
    }

    /// Read one regular file below the read root (general read, up to 2 GiB)
    pub fn read(&self, path: &str) -> Result<Vec<u8>, AutomationError> {
        let rel = relative_path(path)?;
        let root = self.read.as_ref().ok_or_else(|| denied("read"))?;
        let file = root.open_with(&rel, &regular_read_options()).map_err(|e| file_error("read", path, e))?;
        let metadata = file.metadata().map_err(|e| file_error("read", path, e))?;
        if !metadata.is_file() {
            return Err(bad(format!("not a regular file: `{path}`")));
        }
        if metadata.len() > MAX_READ_BYTES {
            return Err(bad(format!("file too large: `{path}` is {} bytes (limit {MAX_READ_BYTES})", metadata.len())));
        }
        let mut bytes = Vec::new();
        file.take(MAX_READ_BYTES + 1).read_to_end(&mut bytes).map_err(|e| file_error("read", path, e))?;
        if bytes.len() as u64 > MAX_READ_BYTES {
            return Err(bad(format!("file too large: `{path}` exceeds the {MAX_READ_BYTES}-byte limit")));
        }
        Ok(bytes)
    }

    /// Read an image input file for material export, enforcing the 16 MiB encoded-input cap.
    /// This limits the stream to the encoded cap plus one byte and checks the returned length.
    pub fn read_input_image(&self, path: &str) -> Result<Vec<u8>, AutomationError> {
        let rel = relative_path(path)?;
        let root = self.read.as_ref().ok_or_else(|| denied("read"))?;
        let file = root.open(&rel).map_err(|e| file_error("read", path, e))?;
        let metadata = file.metadata().map_err(|e| file_error("read", path, e))?;
        if !metadata.is_file() {
            return Err(bad(format!("not a regular file: `{path}`")));
        }
        if metadata.len() > MAX_INPUT_ENCODED_BYTES as u64 {
            return Err(bad(format!("input file `{path}` is {} bytes, over the {MAX_INPUT_ENCODED_BYTES}-byte limit", metadata.len())));
        }
        if metadata.len() > MAX_READ_BYTES {
            return Err(bad(format!("file too large: `{path}` is {} bytes (limit {MAX_READ_BYTES})", metadata.len())));
        }
        let mut bytes = Vec::new();
        file.take(MAX_INPUT_ENCODED_BYTES as u64 + 1).read_to_end(&mut bytes).map_err(|e| file_error("read", path, e))?;
        if bytes.len() as u64 > MAX_INPUT_ENCODED_BYTES as u64 {
            return Err(bad(format!("input file `{path}` exceeds the {MAX_INPUT_ENCODED_BYTES}-byte limit")));
        }

        Ok(bytes)
    }

    /// Create or replace one file below the write root: a temporary file beside the target, synced and renamed over it. The parent directory must exist
    pub fn write(&self, path: &str, bytes: &[u8]) -> Result<(), AutomationError> {
        let rel = relative_path(path)?;
        let root = self.write.as_ref().ok_or_else(|| denied("write"))?;
        let leaf = rel.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let tmp = rel.with_file_name(format!(".{leaf}.{}.tmp", std::process::id()));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);

        // Only the writer that acquired the staging file may remove it
        let mut file = root.open_with(&tmp, &options).map_err(|error| file_error("write", path, error))?;
        let written = file.write_all(bytes).and_then(|()| file.sync_all());
        drop(file);

        if let Err(error) = written.and_then(|()| root.rename(&tmp, root, &rel)) {
            let _ = root.remove_file(&tmp);
            return Err(file_error("write", path, error));
        }

        Ok(())
    }

    /// Validate the write capability and relative path before cooking
    pub fn check_write(&self, path: &str) -> Result<(), AutomationError> {
        relative_path(path)?;
        self.write.as_ref().map(|_| ()).ok_or_else(|| denied("write"))
    }

    /// Create a directory and its parents below the write root
    pub fn create_dir_all(&self, path: &str) -> Result<(), AutomationError> {
        let rel = relative_path(path)?;
        let root = self.write.as_ref().ok_or_else(|| denied("write"))?;
        root.create_dir_all(&rel).map_err(|e| file_error("create", path, e))
    }

    /// Stage an immutable project asset without overwriting another package's resources.
    pub fn write_resource(&self, path: &str, bytes: &[u8]) -> Result<(), AutomationError> {
        let rel = relative_path(path)?;
        let root = self.write.as_ref().ok_or_else(|| denied("write"))?;
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        match root.open_with(&rel, &options) {
            Ok(mut file) => {
                if let Err(error) = file.write_all(bytes).and_then(|_| file.sync_all()) {
                    let _ = root.remove_file(&rel);
                    return Err(file_error("write", path, error));
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                let file = root.open_with(&rel, &regular_read_options()).map_err(|error| file_error("read", path, error))?;
                let metadata = file.metadata().map_err(|error| file_error("read", path, error))?;
                if !metadata.is_file() || metadata.len() != bytes.len() as u64 {
                    return Err(bad(format!("Resource filename collision: `{path}`")));
                }

                if !resource_matches(file, bytes).map_err(|error| file_error("read", path, error))? {
                    return Err(bad(format!("Resource filename collision: `{path}`")));
                }
            }
            Err(error) => return Err(file_error("write", path, error)),
        }
        Ok(())
    }
}

// A repeated save compares immutable assets without buffering a second copy
fn resource_matches(mut file: cap_std::fs::File, mut expected: &[u8]) -> std::io::Result<bool> {
    const COMPARISON_BUFFER_BYTES: usize = 8 * 1024;
    let mut buffer = [0; COMPARISON_BUFFER_BYTES];
    loop {
        let count = match file.read(&mut buffer) {
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            result => result?,
        };
        if count == 0 {
            return Ok(expected.is_empty());
        }

        let Some(rest) = expected.strip_prefix(&buffer[..count]) else {
            return Ok(false);
        };

        expected = rest;
    }
}

// Keep capability-based resolution while preventing FIFO opens from waiting for a writer
fn regular_read_options() -> OpenOptions {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use cap_std::fs::OpenOptionsExt;
        options.custom_flags(rustix::fs::OFlags::NONBLOCK.bits() as i32);
    }
    options
}

fn open_root(path: Option<&Path>, authority: &str) -> Result<Option<Arc<Dir>>, AutomationError> {
    let Some(path) = path else { return Ok(None) };
    if path.as_os_str().is_empty() {
        return Err(bad(format!("the automation {authority} root is empty")));
    }

    let dir =
        Dir::open_ambient_dir(path, ambient_authority()).map_err(|e| AutomationError::Io(format!("cannot open {authority} root `{}`: {e}", path.display())))?;
    Ok(Some(Arc::new(dir)))
}

/// Validate an untrusted path before any I/O (cap-std then confines resolution to the root)
pub fn relative_path(raw: &str) -> Result<PathBuf, AutomationError> {
    let reject = |why: &str| bad(format!("path rejected ({why}): `{raw}`; use a forward-slash path relative to the workspace root"));
    if raw.is_empty() {
        return Err(reject("empty"));
    }

    if raw.contains('\\') {
        return Err(reject("backslash"));
    }

    if raw.starts_with('/') {
        return Err(reject("absolute"));
    }

    if raw.contains(':') {
        return Err(reject("drive, scheme or stream prefix"));
    }

    for component in raw.split('/') {
        if component.is_empty() || component == "." || component == ".." {
            return Err(reject("empty, `.` or `..` component"));
        }

        if component.ends_with(['.', ' ']) || component.chars().any(char::is_control) {
            return Err(reject("component ends in a dot or space, or has control characters"));
        }

        let stem = component.split('.').next().unwrap_or(component).to_ascii_uppercase();
        let device = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$")
            || (stem.len() == 4 && (stem.starts_with("COM") || stem.starts_with("LPT")) && stem.ends_with(|ch: char| ch.is_ascii_digit()));
        if device {
            return Err(reject("reserved device name"));
        }
    }

    Ok(PathBuf::from(raw))
}

fn denied(authority: &str) -> AutomationError {
    bad(format!("file access is not granted: no {authority} root (start the server with --{authority}-root <dir>)"))
}

fn file_error(op: &str, path: &str, e: std::io::Error) -> AutomationError {
    AutomationError::Io(format!("{op} `{path}` failed: {e}"))
}

/// Route file access through the workspace boundary
pub fn authorize_command(id: &str, params: &Value) -> Result<(), AutomationError> {
    let has_path = ["path", "dir"].iter().any(|k| params.get(k).is_some_and(|v| !v.is_null()));
    let ok = match id {
        "material.cook" => !has_path && params.get("inputs").is_none_or(|v| v.is_null() || v.as_object().is_some_and(|o| o.is_empty())),
        "file.new" | "file.newPackage" | "file.newAtomicSample" => !has_path,
        _ => !id.starts_with("file.") && !has_path,
    };

    if ok {
        Ok(())
    } else {
        Err(bad(format!("`{id}` with host paths is disabled for automation; use package_open, package_save, bitmap_import or outputs_export")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn roots(base: &Path) -> (PathBuf, PathBuf, AuthorizedWorkspace) {
        let (inside, outside) = (base.join("inside"), base.join("outside"));
        std::fs::create_dir_all(&inside).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        let ws = AuthorizedWorkspace::new(Some(&inside), Some(&inside)).unwrap();
        (inside, outside, ws)
    }

    #[cfg(unix)]
    #[test]
    fn capability_reads_and_resource_collisions_reject_fifos_without_a_writer() {
        use std::os::unix::fs::OpenOptionsExt;
        use std::time::Duration;
        for collision in [false, true] {
            let directory = tempfile::tempdir().unwrap();
            let (inside, _, ws) = roots(directory.path());
            let path = inside.join("pipe");
            assert!(std::process::Command::new("mkfifo").arg(&path).status().unwrap().success());
            let (sender, receiver) = std::sync::mpsc::channel();
            let worker = std::thread::spawn(move || {
                let result = if collision { ws.write_resource("pipe", b"x") } else { ws.read("pipe").map(|_| ()) };
                sender.send(result).unwrap();
            });
            let result = receiver.recv_timeout(Duration::from_secs(2));
            if result.is_err() {
                // Release the regression's blocked read before failing the test
                let writer = std::fs::OpenOptions::new().write(true).custom_flags(rustix::fs::OFlags::NONBLOCK.bits() as i32).open(&path).unwrap();
                drop(writer);
            }

            worker.join().unwrap();
            assert!(result.expect("FIFO open must not wait for a writer").is_err());
        }
    }

    #[test]
    fn reads_and_writes_stay_in_the_root() {
        let directory = tempfile::tempdir().unwrap();
        let (inside, _, ws) = roots(directory.path());
        std::fs::write(inside.join("a.txt"), b"canary").unwrap();
        assert_eq!(ws.read("a.txt").unwrap(), b"canary");
        ws.create_dir_all("out/deep").unwrap();
        ws.write("out/deep/b.txt", b"one").unwrap();
        ws.write("out/deep/b.txt", b"two").unwrap();
        assert_eq!(std::fs::read(inside.join("out/deep/b.txt")).unwrap(), b"two");
        let leftovers = std::fs::read_dir(inside.join("out/deep")).unwrap().count();
        assert_eq!(leftovers, 1, "no temporary files left behind");
        assert!(ws.write("missing/c.txt", b"x").is_err(), "parent must exist");
    }

    #[test]
    fn immutable_resources_compare_the_last_byte_of_a_partial_buffer() {
        let directory = tempfile::tempdir().unwrap();
        let (inside, _, workspace) = roots(directory.path());
        let mut bytes = vec![17; 32 * 1024 + 1];
        workspace.write_resource("asset.bin", &bytes).unwrap();
        workspace.write_resource("asset.bin", &bytes).unwrap();

        *bytes.last_mut().unwrap() = 42;
        let collision = workspace.write_resource("asset.bin", &bytes).unwrap_err();

        assert!(collision.to_string().contains("collision"));
        assert_eq!(std::fs::read(inside.join("asset.bin")).unwrap().last(), Some(&17));
    }

    #[test]
    fn a_busy_save_preserves_the_existing_staging_file() {
        let directory = tempfile::tempdir().unwrap();
        let (inside, _, workspace) = roots(directory.path());
        let staging = inside.join(format!(".scene.sbs.{}.tmp", std::process::id()));
        std::fs::write(&staging, b"pending save").unwrap();

        let result = workspace.write("scene.sbs", b"second save");

        assert!(result.is_err());
        assert_eq!(std::fs::read(staging).unwrap(), b"pending save");
        assert!(!inside.join("scene.sbs").exists());
    }

    #[test]
    fn escapes_and_malformed_paths_are_rejected() {
        let directory = tempfile::tempdir().unwrap();
        let (_, outside, ws) = roots(directory.path());
        let abs = outside.join("x.txt").display().to_string();
        for p in ["", "/abs", "../x", "a/../../x", "a\\b", "C:/x", "a//b", "./a", "a:b", "NUL", "con.txt", "x/COM1.log", "dot.", abs.as_str()] {
            assert!(ws.read(p).is_err(), "read accepted {p:?}");
            assert!(ws.write(p, b"x").is_err(), "write accepted {p:?}");
            assert!(ws.create_dir_all(p).is_err(), "mkdir accepted {p:?}");
        }
    }

    #[test]
    fn authorities_are_separate_and_fail_closed() {
        let directory = tempfile::tempdir().unwrap();
        let (inside, _, _) = roots(directory.path());
        std::fs::write(inside.join("r.txt"), b"r").unwrap();
        let ro = AuthorizedWorkspace::new(Some(&inside), None).unwrap();
        assert!(ro.read("r.txt").is_ok());
        assert!(ro.write("w.txt", b"x").is_err());
        let none = AuthorizedWorkspace::default();
        assert!(none.read("r.txt").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn oversized_reads_are_rejected_without_allocating() {
        let directory = tempfile::tempdir().unwrap();
        let (inside, _, ws) = roots(directory.path());
        let file = std::fs::File::create(inside.join("huge.bin")).unwrap();
        file.set_len(MAX_READ_BYTES + 1).unwrap();
        let err = ws.read("huge.bin").unwrap_err().to_string();
        assert!(err.contains("too large"), "{err}");
    }

    #[cfg(unix)]
    #[test]
    fn symlink_escape_is_rejected() {
        let directory = tempfile::tempdir().unwrap();
        let (inside, outside, ws) = roots(directory.path());
        std::fs::write(outside.join("secret.txt"), b"s").unwrap();
        std::os::unix::fs::symlink(&outside, inside.join("link")).unwrap();
        assert!(ws.read("link/secret.txt").is_err());
        assert!(ws.write("link/new.txt", b"x").is_err());
        assert!(!outside.join("new.txt").exists());
    }

    #[cfg(unix)]
    #[test]
    fn save_keeps_the_resolved_destination_when_an_alias_is_retargeted() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        let (inside, _, workspace) = roots(directory.path());
        std::fs::create_dir(inside.join("original")).unwrap();
        std::fs::create_dir_all(inside.join("alternate/deep")).unwrap();
        let alias = inside.join("alias");
        symlink("original", &alias).unwrap();
        let (destination, writer, save_path) = workspace.save_destination("alias/scene.sbs").unwrap();

        std::fs::remove_file(&alias).unwrap();
        symlink("alternate/deep", &alias).unwrap();
        let parent = save_path.rsplit_once('/').map_or(String::new(), |(parent, _)| format!("{parent}/"));
        writer.create_dir_all(&format!("{parent}resources")).unwrap();
        writer.write_resource(&format!("{parent}resources/asset.bin"), b"asset").unwrap();
        writer.write(&save_path, b"source").unwrap();

        assert_eq!(destination, inside.join("original/scene.sbs"));
        assert_eq!(std::fs::read(destination).unwrap(), b"source");
        assert_eq!(std::fs::read(inside.join("original/resources/asset.bin")).unwrap(), b"asset");
        assert_eq!(std::fs::read_dir(inside.join("alternate/deep")).unwrap().count(), 0);
    }

    #[cfg(windows)]
    #[test]
    fn junction_escape_is_rejected() {
        let directory = tempfile::tempdir().unwrap();
        let (inside, outside, ws) = roots(directory.path());
        std::fs::write(outside.join("secret.txt"), b"s").unwrap();
        let link = inside.join("junction");
        let made = std::process::Command::new("cmd.exe").args(["/D", "/C", "mklink", "/J"]).arg(&link).arg(&outside).output();
        if !made.is_ok_and(|o| o.status.success()) {
            eprintln!("skipping: cannot create a junction");
            return;
        }

        assert!(ws.read("junction/secret.txt").is_err());
        assert!(ws.write("junction/new.txt", b"x").is_err());
        assert!(!outside.join("new.txt").exists());
        let _ = std::fs::remove_dir(link);
    }

    #[test]
    fn path_commands_are_denied() {
        for id in ["file.open", "file.save", "file.saveAs", "file.export", "file.importBitmap"] {
            assert!(authorize_command(id, &json!({})).is_err(), "{id}");
        }
        for id in ["file.new", "graph.node.add", "anything.new"] {
            assert!(authorize_command(id, &json!({"path": "/etc/x"})).is_err(), "{id}");
            assert!(authorize_command(id, &json!({"dir": "/etc"})).is_err(), "{id}");
        }
        assert!(authorize_command("file.new", &json!({"template": "atomics"})).is_ok());
        assert!(authorize_command("graph.node.add", &json!({"def": "blur", "path": null})).is_ok());
    }

    /// Every registry command documenting a host path must be classified (fails when a new path-taking command is added without an automation policy)
    #[test]
    fn registry_path_params_are_classified() {
        let probe = json!({"path": "/outside/probe", "dir": "/outside", "mesh": "/outside/m.obj"});
        let open: Vec<&str> = materialcraft_engine::command_specs()
            .iter()
            .filter(|c| c.params.contains("path") || c.params.contains("{dir"))
            .filter(|c| authorize_command(c.id, &probe).is_ok())
            .map(|c| c.id)
            .collect();
        assert!(open.is_empty(), "path-taking commands need an automation policy: {open:?}");
    }
}
