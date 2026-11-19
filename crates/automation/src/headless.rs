//! The headless backend: a [`Session`] plus file I/O and windowless renders. Synchronous and UI-free; the MCP server, the RPC server and the CLI all drive it

use std::fs::OpenOptions;
use std::io::Read;
use std::path::Path;

use materialcraft_engine::core::NodeId;
use materialcraft_engine::formats::material::MATERIAL_EXT;
use materialcraft_engine::{Session, commands, file_cmds, inspect, material_cmds};
use materialcraft_render::{Lighting, Maps, Scene, Settings};
use serde_json::{Value, json};

use crate::budgets::{HQ_TRIANGLES, MAX_INPUT_ENCODED_BYTES, MAX_LOCAL_RENDER_SIDE, MAX_PNG_SIDE, check_png, encode_png};
use crate::workspace::{MAX_READ_BYTES, authorize_command};
use crate::{AutomationError, bad};
pub use crate::{Filesystem, RenderOptions, Rendered};

const MAX_EXPORTED_MATERIAL_BYTES: u64 = 256 << 20;

pub struct Headless {
    pub session: Session,
    fs: Filesystem,
    scene: Option<(String, Scene)>,
    light: Option<Lighting>,
}

/// Read a file under `fs`'s access rules
fn read_file(fs: &Filesystem, path: &str) -> Result<Vec<u8>, AutomationError> {
    match fs {
        Filesystem::Denied => Err(bad("file access is not granted: start the server with --read-root <dir>")),
        Filesystem::TrustedLocal => read_trusted(path),
        Filesystem::Workspace(w) => w.read(path),
    }
}

/// Read a local file under the same bound as capability workspaces: trusted access widens where a
/// path may point, not how much memory one read may take.
fn read_trusted(path: &str) -> Result<Vec<u8>, AutomationError> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;

        // Reject named pipes without waiting for a writer, even if the path changes during open
        options.custom_flags(rustix::fs::OFlags::NONBLOCK.bits() as i32);
    }
    let file = options.open(path).map_err(|error| AutomationError::Io(format!("cannot read {path}: {error}")))?;
    let metadata = file.metadata().map_err(|e| AutomationError::Io(format!("cannot read {path}: {e}")))?;
    if !metadata.is_file() {
        return Err(bad(format!("not a regular file: `{path}`")));
    }

    if metadata.len() > MAX_READ_BYTES {
        return Err(bad(format!("file too large: `{path}` is {} bytes (limit {MAX_READ_BYTES})", metadata.len())));
    }

    let mut bytes = Vec::new();
    file.take(MAX_READ_BYTES + 1).read_to_end(&mut bytes).map_err(|e| AutomationError::Io(format!("cannot read {path}: {e}")))?;
    if bytes.len() as u64 > MAX_READ_BYTES {
        return Err(bad(format!("file too large: `{path}` exceeds the {MAX_READ_BYTES}-byte limit")));
    }

    Ok(bytes)
}

/// Read a local image input file for material export, enforcing the 16 MiB encoded-input cap.
fn read_trusted_input(path: &str) -> Result<Vec<u8>, AutomationError> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;

        options.custom_flags(rustix::fs::OFlags::NONBLOCK.bits() as i32);
    }
    let file = options.open(path).map_err(|e| AutomationError::Io(format!("cannot read {path}: {e}")))?;
    let metadata = file.metadata().map_err(|e| AutomationError::Io(format!("cannot read {path}: {e}")))?;
    if !metadata.is_file() {
        return Err(bad(format!("not a regular file: `{path}`")));
    }
    if metadata.len() > MAX_INPUT_ENCODED_BYTES as u64 {
        return Err(bad(format!("input file `{path}` is {} bytes, over the {MAX_INPUT_ENCODED_BYTES}-byte limit", metadata.len())));
    }
    let mut bytes = Vec::new();
    file.take(MAX_INPUT_ENCODED_BYTES as u64 + 1).read_to_end(&mut bytes).map_err(|e| AutomationError::Io(format!("cannot read {path}: {e}")))?;
    if bytes.len() as u64 > MAX_INPUT_ENCODED_BYTES as u64 {
        return Err(bad(format!("input file `{path}` exceeds the {MAX_INPUT_ENCODED_BYTES}-byte limit")));
    }
    Ok(bytes)
}

impl Headless {
    pub fn new(session: Session, fs: Filesystem) -> Headless {
        Headless { session, fs, scene: None, light: None }
    }

    /// The session a fresh server starts with: the atomic sample
    pub fn default_session() -> Session {
        materialcraft_engine::sample_session()
    }

    fn trusted(&self) -> bool {
        matches!(self.fs, Filesystem::TrustedLocal)
    }

    fn read(&self, path: &str) -> Result<Vec<u8>, AutomationError> {
        read_file(&self.fs, path)
    }

    fn write(&self, path: &str, bytes: &[u8]) -> Result<(), AutomationError> {
        match &self.fs {
            Filesystem::Denied => Err(bad("file access is not granted: start the server with --write-root <dir>")),
            Filesystem::TrustedLocal => materialcraft_engine::formats::write_atomic(Path::new(path), bytes).map_err(|e| AutomationError::Io(e.to_string())),
            Filesystem::Workspace(w) => w.write(path, bytes),
        }
    }

    fn create_dir_all(&self, dir: &str) -> Result<(), AutomationError> {
        match &self.fs {
            _ if dir == "." => Ok(()),
            Filesystem::Denied => Err(bad("file access is not granted: start the server with --write-root <dir>")),
            Filesystem::TrustedLocal => std::fs::create_dir_all(dir).map_err(|e| AutomationError::Io(format!("cannot create {dir}: {e}"))),
            Filesystem::Workspace(w) => w.create_dir_all(dir),
        }
    }

    pub fn inspect(&mut self) -> Value {
        self.session.cook_if_dirty();
        inspect::session(&self.session, None)
    }

    pub fn command_list(&self, filter: Option<&str>) -> Value {
        commands::list(&self.session, filter)
    }

    /// Run an engine command. Untrusted callers can't reach host paths through it
    pub fn command_run(&mut self, id: &str, params: Value) -> Result<Value, AutomationError> {
        if id == "material.cook" {
            return Ok(materialcraft_engine::material_bake::cook(&self.session, &params, |path| {
                self.read(path).map_err(|e| materialcraft_engine::EngineError::Other(e.to_string()))
            })?);
        }
        if !self.trusted() {
            authorize_command(id, &params)?;
        }
        Ok(materialcraft_engine::commands::execute_request(&mut self.session, id, &params)?)
    }

    /// Run `[{id, params?}]` in order, stopping at the first error (reported with its index)
    pub fn batch(&mut self, steps: &[Value]) -> Value {
        let mut results = Vec::new();
        for (i, step) in steps.iter().enumerate() {
            let id = step.get("id").and_then(Value::as_str).unwrap_or_default();
            match self.command_run(id, step.get("params").cloned().unwrap_or(Value::Null)) {
                Ok(v) => results.push(v),
                Err(e) => return batch_reply(results, Some((i, id, e.to_string()))),
            }
        }
        batch_reply(results, None)
    }

    /// Open an `.sbs` or `.mcraft` package (or a `.mcmat` material's source), using the granted file access
    pub fn open(&mut self, path: &str) -> Result<Value, AutomationError> {
        self.load(path)?;
        Ok(self.inspect())
    }

    /// Open the document without generating preview images when a caller only needs its interface.
    pub fn load(&mut self, path: &str) -> Result<(), AutomationError> {
        let bytes = self.read(path)?;

        // Workspace save paths stay relative to their granted roots
        let document = self.session.open_packages().into_iter().find(|entry| entry.2 == Some(path)).map(|entry| entry.0);
        if let Some(document) = document {
            self.session.activate_document(document)?;
            return Ok(());
        }

        match &self.fs {
            Filesystem::TrustedLocal => file_cmds::open_bytes_linked(&mut self.session, &bytes, Some(path.into()), &file_cmds::linked_files(path))?,
            Filesystem::Workspace(workspace) => {
                let document = workspace.document_path(path)?;
                file_cmds::open_bytes_scoped(&mut self.session, &bytes, &document.to_string_lossy(), &|file| workspace.read_dependency(file))?;
            }
            Filesystem::Denied => return Err(bad("file access is not granted")),
        }
        if self.session.path.is_some() {
            self.session.path = Some(path.to_string());
        }

        Ok(())
    }

    /// Save SBS with adjacent resources, or an independent MaterialCraft archive.
    pub fn save(&mut self, path: Option<&str>) -> Result<Value, AutomationError> {
        let path = match path {
            Some(p) => file_cmds::package_path(p),
            None => self.session.path.clone().ok_or_else(|| bad("the package has no file yet (new, or opened from another program): give a `path`"))?,
        };

        let (destination, workspace_write) = match &self.fs {
            Filesystem::Workspace(workspace) => {
                let (destination, writer, save_path) = workspace.save_destination(&path)?;
                (destination.to_string_lossy().into_owned(), Some((writer, save_path)))
            }
            Filesystem::TrustedLocal => (path.clone(), None),
            Filesystem::Denied => return Err(bad("file access is not granted")),
        };
        let files = file_cmds::save_files_at(&self.session, &destination)?;
        let bytes = if file_cmds::is_mcraft(&path) {
            let bytes = files.zip().map_err(|error| bad(error.to_string()))?;
            if let Some((writer, save_path)) = &workspace_write {
                writer.write(save_path, &bytes)?;
            } else {
                self.write(&path, &bytes)?;
            }
            bytes.len()
        } else {
            match &workspace_write {
                None => materialcraft_engine::formats::project::write_files(Path::new(&path), &files).map_err(|error| bad(error.to_string()))?,
                Some((writer, save_path)) => {
                    let parent = save_path.rsplit_once('/').map_or(String::new(), |(parent, _)| format!("{parent}/"));
                    for name in files.files.keys().filter(|name| *name != &files.entry) {
                        writer.check_write(&format!("{parent}{name}"))?;
                    }
                    writer.create_dir_all(&format!("{parent}resources"))?;
                    for (name, bytes) in &files.files {
                        if name == &files.entry {
                            continue;
                        }

                        let target = format!("{parent}{name}");
                        if let Some((directory, _)) = target.rsplit_once('/') {
                            writer.create_dir_all(directory)?;
                        }
                        writer.write_resource(&target, bytes.as_slice())?;
                    }
                    writer.write(save_path, files.xml().map_err(|error| bad(error.to_string()))?)?;
                }
            }
            files.xml().map_err(|error| bad(error.to_string()))?.len()
        };
        self.session.path = Some(path.clone());
        self.session.modified = false;
        Ok(json!({"path": path, "bytes": bytes}))
    }

    /// Embed an image file as a bitmap resource
    pub fn import_bitmap(&mut self, path: &str, name: Option<&str>) -> Result<Value, AutomationError> {
        let bytes = self.read(path)?;
        let file = Path::new(path).file_name().and_then(|f| f.to_str()).unwrap_or("bitmap.png").to_string();
        let name = file_cmds::add_resource(&mut self.session, &file, bytes, name)?;
        Ok(json!({"name": name}))
    }

    /// Export every output of a graph into folder `dir`
    pub fn export(&mut self, p: &Value) -> Result<Value, AutomationError> {
        let dir = p.get("dir").and_then(Value::as_str).ok_or_else(|| bad("`dir` is required"))?;
        let (files, mut report) = materialcraft_engine::material_bake::export(&self.session, p, |path| {
            self.read(path).map_err(|e| materialcraft_engine::EngineError::Other(e.to_string()))
        })?;
        self.create_dir_all(dir)?;

        let mut written = Vec::new();
        for exported in files {
            let path = if dir == "." { exported.file.clone() } else { format!("{}/{}", dir.trim_end_matches('/'), exported.file) };
            self.write(&path, &exported.bytes)?;
            written.push(path);
        }
        report["files"] = json!(written);
        Ok(report)
    }

    /// Export a portable material below the granted write root
    pub fn export_material(&mut self, p: &Value) -> Result<Value, AutomationError> {
        let path = p.get("path").and_then(Value::as_str).ok_or_else(|| bad("`path` is required"))?;
        let path = if path.to_ascii_lowercase().ends_with(&format!(".{MATERIAL_EXT}")) { path.to_string() } else { format!("{path}.{MATERIAL_EXT}") };
        match &self.fs {
            Filesystem::Denied => return Err(bad("file access is not granted: start the server with --write-root <dir>")),
            Filesystem::Workspace(w) => w.check_write(&path)?,
            Filesystem::TrustedLocal => {}
        }

        let (gi, mut options) = material_cmds::material_request(&self.session, "material.export", p)?;
        material_cmds::load_request_inputs("material.export", |f| self.read_input_image(f).map_err(|e| e.to_string()), &self.session, gi, p, &mut options)?;
        let bytes = material_cmds::material_bytes_with_options(&mut self.session, gi, &options)?;
        if !self.trusted() && bytes.len() as u64 > MAX_EXPORTED_MATERIAL_BYTES {
            return Err(bad(format!("the material is {} bytes (limit {MAX_EXPORTED_MATERIAL_BYTES}); reduce source asset sizes", bytes.len())));
        }
        if bytes.len() as u64 > MAX_EXPORTED_MATERIAL_BYTES {
            return Err(bad(format!("the material is {} bytes (limit {MAX_EXPORTED_MATERIAL_BYTES}); reduce source asset sizes", bytes.len())));
        }

        self.write(&path, &bytes)?;
        Ok(json!({"path": path, "bytes": bytes.len()}))
    }

    /// Read an image input file for material export, enforcing the 16 MiB encoded-input cap.
    fn read_input_image(&self, path: &str) -> Result<Vec<u8>, AutomationError> {
        match &self.fs {
            Filesystem::Denied => Err(bad("file access is not granted: start the server with --read-root <dir>")),
            Filesystem::TrustedLocal => read_trusted_input(path),
            Filesystem::Workspace(w) => w.read_input_image(path),
        }
    }

    /// Render the assigned material or a node output as PNG
    pub fn render_png(&mut self, o: &RenderOptions) -> Result<Rendered, AutomationError> {
        let max = if self.trusted() { MAX_LOCAL_RENDER_SIDE } else { MAX_PNG_SIDE };
        let saved = self.session.preview_size;
        if o.full {
            self.session.preview_size = None;
            self.session.dirty = true;
        }
        self.session.cook_if_dirty();
        let out = match o.view.as_deref().unwrap_or("3d") {
            "2d" => self.render_2d(o, max),
            "3d" => Ok(self.render_3d(o, max)),
            other => Err(bad(format!("unknown view `{other}` (3d or 2d)"))),
        };

        if o.full {
            self.session.preview_size = saved;
            self.session.dirty = true;
        }
        let (w, h, rgba) = out?;
        let png = encode_png(w, h, rgba)?;
        check_png(png.len())?;
        Ok(Rendered { width: w, height: h, png })
    }

    fn render_2d(&mut self, options: &RenderOptions, maximum_side: u32) -> Result<(u32, u32, Vec<u8>), AutomationError> {
        let image = match options.node {
            Some(n) => self.session.cooked.image(NodeId(n), options.port.unwrap_or(0)).ok_or_else(|| bad(format!("node {n} has no cooked output")))?,
            None => self.session.view2d_image().map(|(_, i)| i).ok_or_else(|| bad("nothing to show: the graph has no outputs; pass `node`"))?,
        };

        let (w, h) = (options.width.unwrap_or(image.width()).clamp(16, maximum_side), options.height.unwrap_or(image.height()).clamp(16, maximum_side));
        let image = image.resize(w, h, true);
        Ok((w, h, image.to_rgba8_display()))
    }

    fn render_3d(&mut self, o: &RenderOptions, max: u32) -> (u32, u32, Vec<u8>) {
        let v = &self.session.rendered_view3d();
        let scene_id = o.scene.clone().unwrap_or_else(|| v.scene.clone());
        let custom = self.session.custom_mesh.clone().filter(|_| scene_id == "custom");
        let key = custom.as_ref().map_or_else(|| scene_id.clone(), |(name, _)| format!("custom:{name}:{}", self.session.custom_mesh_generation));

        if self.scene.as_ref().is_none_or(|(id, _)| *id != key) {
            let scene = match custom {
                Some((_, mesh)) => Scene::new((*mesh).clone()),
                None => Scene::builtin(&scene_id),
            };

            self.scene = Some((key, scene));
        }

        if self.light.as_ref().is_none_or(|l| l.name != v.environment) {
            self.light = Some(Lighting::new(&v.environment));
        }

        let tiling = o.tiling.unwrap_or(v.tiling).clamp(0.1, 64.0);
        let depth = o.displacement.unwrap_or(v.height_scale).clamp(0.0, 0.5);
        let view = materialcraft_engine::View3d { yaw: o.yaw.unwrap_or(v.yaw), pitch: o.pitch.unwrap_or(v.pitch), tiling, height_scale: depth, ..v.clone() };
        let mut settings = Settings { supersample: view.samples.max(2), ..Settings::for_view(&view) };

        let (w, h) = (o.width.unwrap_or(1024).clamp(16, max), o.height.unwrap_or(768).clamp(16, max));
        let maps = Maps::from_usages(self.session.material_maps());
        let (Some((_, scene)), Some(light)) = (&self.scene, &self.light) else { return (w, h, vec![0; (w * h * 4) as usize]) };
        let triangles = if o.hq { HQ_TRIANGLES } else { materialcraft_render::relief_triangles(&view) };
        let displaced = materialcraft_render::raised(scene, &maps, &view, triangles);

        if o.hq {
            settings.supersample = 3;
            settings.ao_rays = 8;
            settings.key_light = 2.5;
        }
        (w, h, materialcraft_render::render(displaced.as_ref().unwrap_or(scene), &maps, light, &settings, w, h))
    }
}

pub fn batch_reply(results: Vec<Value>, failed: Option<(usize, &str, String)>) -> Value {
    json!({
        "completed": results.len(),
        "results": results,
        "failed": failed.map(|(index, id, error)| json!({ "index": index, "id": id, "error": error })),
    })
}

#[cfg(test)]
#[path = "headless.test.rs"]
mod tests;
