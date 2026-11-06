use std::sync::{Mutex, PoisonError};

use crate::MaterialCraftApp;
use crate::i18n::tr;

/// What a picked file is for
#[derive(Clone, Copy, PartialEq)]
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
enum Pick {
    Package,
    Bitmap,
    Mesh,
}

/// Files picked in the browser, waiting for the next frame: (file name, bytes, purpose)
static INBOX: Mutex<Vec<(String, Vec<u8>, Pick)>> = Mutex::new(Vec::new());

/// Load files that arrived since the last frame. Call once per frame
pub fn poll(app: &mut MaterialCraftApp) {
    let picked = std::mem::take(&mut *INBOX.lock().unwrap_or_else(PoisonError::into_inner));
    for (name, bytes, pick) in picked {
        if pick == Pick::Mesh {
            mesh(app, &name, &bytes);
        } else {
            load(app, &name, bytes, pick == Pick::Bitmap);
        }
    }
}

/// Show a mesh file's bytes in the 3D view
fn mesh(app: &mut MaterialCraftApp, name: &str, bytes: &[u8]) {
    match app.session.load_mesh_bytes(name, bytes) {
        Ok(()) => app.toast(format!("{} {name}", tr("Showing")), false),
        Err(e) => app.toast(format!("{} {name}: {e}", tr("Couldn't load")), true),
    }
}

/// 3D view ▸ Scene ▸ Load mesh: a native dialog on the desktop, the file picker in the browser
pub fn load_mesh(app: &mut MaterialCraftApp, ctx: &egui::Context) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = ctx;
        if let Some(p) = rfd::FileDialog::new().add_filter("Meshes", materialcraft_engine::formats::mesh::MESH_EXTENSIONS).pick_file() {
            app.run("view.3d.loadMesh", serde_json::json!({"path": p.display().to_string()}));
        }
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = app;
        web::pick_mesh(ctx);
    }
}

/// Open a package or import an image (by extension unless `bitmap` forces an image)
pub fn load(app: &mut MaterialCraftApp, name: &str, bytes: Vec<u8>, bitmap: bool) {
    let ext = name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
    if materialcraft_engine::formats::mesh::MESH_EXTENSIONS.contains(&ext.as_str()) {
        mesh(app, name, &bytes);
        return;
    }
    if bitmap || materialcraft_engine::formats::IMAGE_EXTENSIONS.contains(&ext.as_str()) {
        crate::graph::import_bitmap_bytes(app, name, bytes);
        return;
    }
    if app.ui.windows.unsaved.is_some() {
        return;
    }
    app.pending_load = Some((name.to_string(), bytes));
    crate::unsaved::guard(app, crate::unsaved::Guarded::LoadBytes { name: name.to_string() });
}

pub(crate) fn load_now(app: &mut MaterialCraftApp, name: &str, bytes: &[u8]) {
    match materialcraft_engine::file_cmds::open_bytes(&mut app.session, bytes, Some(name.to_string())) {
        Ok(()) => {
            app.close_subgraph_after("file.open");
            app.ui.frame_all = true;
            app.toast(format!("{} {name}", tr("Opened")), false);
            crate::windows::after_open(app);
        }
        Err(e) => app.toast(format!("{} {name}: {e}", tr("Couldn't open")), true),
    }
}

/// A file dropped on the window: a package opens, an image becomes a Bitmap node (in the browser the bytes arrive on a later frame)
pub fn dropped(app: &mut MaterialCraftApp, ctx: &egui::Context, file: &egui::DroppedFileHandle) {
    let name = file.path().file_name().and_then(|n| n.to_str()).unwrap_or("dropped").to_string();
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = ctx;
        let lower = name.to_ascii_lowercase();
        if lower.ends_with(".sbs") && file.path().is_file() {
            // Open from the path so an SBS package finds the bitmaps kept next to it
            crate::windows::open_path(app, &file.path().display().to_string());
            return;
        }
        match file.bytes() {
            Ok(bytes) => load(app, &name, bytes, false),
            Err(e) => app.toast(format!("{} {name}: {e}", tr("Couldn't open")), true),
        }
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = app;
        let (file, ctx) = (file.clone(), ctx.clone());
        wasm_bindgen_futures::spawn_local(async move {
            match file.bytes_async().await {
                Ok(bytes) => INBOX.lock().unwrap_or_else(PoisonError::into_inner).push((name, bytes, Pick::Package)),
                Err(e) => log::warn!("couldn't read dropped file {name}: {e}"),
            }
            ctx.request_repaint();
        });
    }
}

/// A zip of `(name, bytes)` files (export downloads several maps at once)
pub fn zip_files(files: &[(String, Vec<u8>)]) -> Result<Vec<u8>, String> {
    use std::io::Write as _;
    let mut buf = std::io::Cursor::new(Vec::new());
    {
        let mut z = zip::ZipWriter::new(&mut buf);
        let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
        for (name, bytes) in files {
            z.start_file(name.as_str(), opts).map_err(|e| e.to_string())?;
            z.write_all(bytes).map_err(|e| e.to_string())?;
        }
        z.finish().map_err(|e| e.to_string())?;
    }
    Ok(buf.into_inner())
}

#[cfg(target_arch = "wasm32")]
pub use web::*;

#[cfg(target_arch = "wasm32")]
mod web {
    use std::sync::PoisonError;

    use wasm_bindgen::JsCast as _;

    use super::{INBOX, Pick, zip_files};
    use crate::MaterialCraftApp;
    use crate::i18n::tr;

    fn pick(ctx: &egui::Context, filter: &'static str, exts: &'static [&'static str], kind: Pick) {
        let ctx = ctx.clone();
        wasm_bindgen_futures::spawn_local(async move {
            if let Some(file) = rfd::AsyncFileDialog::new().add_filter(filter, exts).pick_file().await {
                let bytes = file.read().await;
                INBOX.lock().unwrap_or_else(PoisonError::into_inner).push((file.file_name(), bytes, kind));
            }
            ctx.request_repaint();
        });
    }

    /// File ▸ Open: a package, picked in the browser
    pub fn open(ctx: &egui::Context) {
        pick(ctx, "MaterialCraft package or material", crate::windows::OPEN_EXTS, Pick::Package);
    }

    /// 3D view ▸ Scene ▸ Load mesh
    pub fn pick_mesh(ctx: &egui::Context) {
        pick(ctx, "Meshes", materialcraft_engine::formats::mesh::MESH_EXTENSIONS, Pick::Mesh);
    }

    /// File ▸ Import bitmap
    pub fn import_bitmap(ctx: &egui::Context) {
        pick(ctx, "Images", materialcraft_engine::formats::IMAGE_EXTENSIONS, Pick::Bitmap);
    }

    /// File ▸ Save: download the package as `<name>.sbs` (bitmaps embedded)
    pub fn save(app: &mut MaterialCraftApp) {
        let name = format!("{}.sbs", materialcraft_engine::formats::safe_file_name(&app.session.pkg.name));
        match materialcraft_engine::file_cmds::save_bytes(&app.session).map_err(|e| e.to_string()).and_then(|b| download(&name, &b)) {
            Ok(()) => {
                app.session.modified = false;
                app.toast(format!("{} {name}", tr("Downloaded")), false);
            }
            Err(e) => app.toast(format!("{} {name}: {e}", tr("Couldn't save")), true),
        }
    }

    /// File ▸ Export material: download the open graph as `<graph>.mcmat`
    pub fn export_material(app: &mut MaterialCraftApp) {
        use materialcraft_engine::material_cmds::{MaterialOptions, material_bytes};
        let name = format!("{}.mcmat", materialcraft_engine::formats::safe_file_name(&app.session.graph().map(|g| g.name.clone()).unwrap_or_default()));
        match material_bytes(&app.session, app.session.graph, &MaterialOptions::default()).map_err(|e| e.to_string()).and_then(|b| download(&name, &b)) {
            Ok(()) => app.toast(format!("{} {name}", tr("Downloaded")), false),
            Err(e) => app.toast(format!("{}: {e}", tr("Couldn't export")), true),
        }
    }

    /// Export outputs: one file downloads as is, several as `<graph>_outputs.zip`
    pub fn export(app: &mut MaterialCraftApp, params: &serde_json::Value) {
        let result = materialcraft_engine::file_cmds::export_params(&app.session, params)
            .and_then(|(gi, ext, pattern, depth, size)| materialcraft_engine::file_cmds::export_outputs(&app.session, gi, &ext, &pattern, depth, size))
            .map_err(|e| e.to_string())
            .and_then(|files| {
                let files: Vec<(String, Vec<u8>)> = files.into_iter().map(|f| (f.file, f.bytes)).collect();
                match files.as_slice() {
                    [(name, bytes)] => download(name, bytes).map(|()| name.clone()),
                    _ => {
                        let name = format!("{}_outputs.zip", app.session.graph().map(|g| g.name.clone()).unwrap_or_default());
                        download(&name, &zip_files(&files)?).map(|()| name)
                    }
                }
            });
        match result {
            Ok(name) => app.toast(format!("{} {name}", tr("Downloaded")), false),
            Err(e) => app.toast(format!("{}: {e}", tr("Couldn't export")), true),
        }
    }

    /// Start a browser download of `bytes` named `name` (Blob + a temporary `<a download>`)
    pub fn download(name: &str, bytes: &[u8]) -> Result<(), String> {
        let js = |e: wasm_bindgen::JsValue| format!("{e:?}");
        let window = web_sys::window().ok_or("no window")?;
        let document = window.document().ok_or("no document")?;
        let parts = js_sys::Array::of1(&js_sys::Uint8Array::from(bytes));
        let opts = web_sys::BlobPropertyBag::new();
        opts.set_type(if name.ends_with(".zip") { "application/zip" } else { "application/octet-stream" });
        let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &opts).map_err(js)?;
        let url = web_sys::Url::create_object_url_with_blob(&blob).map_err(js)?;
        let a: web_sys::HtmlAnchorElement = document.create_element("a").map_err(js)?.dyn_into().map_err(|_| "not an anchor")?;
        a.set_href(&url);
        a.set_download(name);
        a.style().set_property("display", "none").map_err(js)?;
        document.body().ok_or("no body")?.append_child(&a).map_err(js)?;
        a.click();
        a.remove();
        // Revoke once the click has been dispatched; the download keeps its own reference
        let revoke = wasm_bindgen::closure::Closure::once_into_js(move || {
            web_sys::Url::revoke_object_url(&url).ok();
        });
        window.set_timeout_with_callback_and_timeout_and_arguments_0(revoke.unchecked_ref(), 10_000).map_err(js)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(modified: bool) -> MaterialCraftApp {
        let mut s = materialcraft_engine::sample_session();
        s.modified = modified;
        MaterialCraftApp::headless(s, crate::state::UiState::default())
    }

    #[test]
    fn a_package_picked_over_unsaved_work_is_held_not_loaded() {
        let mut app = app(true);
        let before = app.session.pkg.name.clone();
        load(&mut app, "other.sbs", b"not a package".to_vec(), false);
        assert_eq!(app.ui.windows.unsaved, Some(crate::unsaved::Guarded::LoadBytes { name: "other.sbs".into() }));
        assert_eq!(app.pending_load.as_ref().map(|(n, _)| n.as_str()), Some("other.sbs"));
        assert_eq!(app.session.pkg.name, before);
        assert!(app.toasts.is_empty());
    }

    #[test]
    fn an_image_import_is_not_a_replacement_and_never_asks() {
        let mut app = app(true);
        load(&mut app, "x.png", b"junk".to_vec(), true);
        assert!(app.ui.windows.unsaved.is_none());
    }

    #[test]
    fn zip_round_trips() {
        let files = vec![("a.png".to_string(), b"one".to_vec()), ("b.png".to_string(), vec![7u8; 4096])];
        let bytes = zip_files(&files).unwrap();
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        assert_eq!(zip.len(), 2);
        for (name, data) in &files {
            let mut out = Vec::new();
            std::io::Read::read_to_end(&mut zip.by_name(name).unwrap(), &mut out).unwrap();
            assert_eq!(&out, data);
        }
    }
}
