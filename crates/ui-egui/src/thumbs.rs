use std::collections::{BTreeMap, HashMap};
use std::sync::mpsc::Receiver;

use egui::{ColorImage, TextureHandle, TextureOptions};
use materialcraft_engine::image::{Image, Kind};
use materialcraft_engine::nodes::{CookContext, catalog, resolve_params};

/// Rendered previews by id: (id, RGBA8 pixels)
pub type Previews = Vec<(String, Vec<u8>)>;

/// Side of node thumbnails and library previews, in pixels
pub const THUMB: u32 = 96;
const LIBRARY_PREVIEW_SIZE: u32 = 72;

#[derive(Default)]
pub struct Thumbs {
    /// (cook key, port) → texture and the frame it was last used
    nodes: HashMap<(u64, usize), (TextureHandle, u64)>,
    library: HashMap<String, TextureHandle>,
    library_rx: Option<Receiver<Previews>>,
    library_started: bool,
    frame: u64,
}

/// An image as an egui texture image (sRGB display, `side` px)
pub fn color_image(img: &Image, side: u32) -> ColorImage {
    let small = img.resize(side, side, true);
    ColorImage::from_rgba_unmultiplied([side as usize, side as usize], &small.to_rgba8_display())
}

impl Thumbs {
    pub fn clear_nodes(&mut self) {
        self.nodes.clear();
    }

    /// The thumbnail of a node output, uploaded on first use
    pub fn node(&mut self, ctx: &egui::Context, key: u64, port: usize, img: &Image) -> egui::TextureId {
        let frame = self.frame;
        if let Some((t, f)) = self.nodes.get_mut(&(key, port)) {
            *f = frame;
            return t.id();
        }
        let tex = ctx.load_texture(format!("node-{key}-{port}"), color_image(img, THUMB), TextureOptions::LINEAR);
        let id = tex.id();
        self.nodes.insert((key, port), (tex, frame));
        id
    }

    /// Once per frame: forget thumbnails unused for a few seconds; collect library previews
    pub fn tick(&mut self, ctx: &egui::Context) {
        self.frame += 1;
        let f = self.frame;
        if self.nodes.len() > 256 {
            self.nodes.retain(|_, (_, used)| f - *used < 120);
        }
        if let Some(rx) = &self.library_rx
            && let Ok(list) = rx.try_recv()
        {
            for (id, rgba) in list {
                let img = ColorImage::from_rgba_unmultiplied([LIBRARY_PREVIEW_SIZE as usize, LIBRARY_PREVIEW_SIZE as usize], &rgba);
                self.library.insert(id.clone(), ctx.load_texture(format!("library-{id}"), img, TextureOptions::LINEAR));
            }
            self.library_rx = None;
        }
    }

    /// The library preview of node `def` (generated in the background on first use)
    pub fn library(&mut self, ctx: &egui::Context, def: &str) -> Option<egui::TextureId> {
        if !self.library_started {
            self.library_started = true;
            self.library_rx = Some(spawn_library_previews(ctx.clone()));
        }
        self.library.get(def).map(TextureHandle::id)
    }
}

/// Sample inputs for previews: a wave height, a colored version and its normal map
fn sample_inputs() -> (Image, Image, Image) {
    let gray = Image::generate_uv(LIBRARY_PREVIEW_SIZE, LIBRARY_PREVIEW_SIZE, Kind::Gray, |u, v| {
        [0.5 + 0.25 * (u * std::f32::consts::TAU).sin() + 0.25 * (v * std::f32::consts::TAU).cos(), 0.0, 0.0, 0.0]
    });
    let color = gray.map(Kind::Color, |p| [0.15 + 0.7 * p[0], 0.35 + 0.3 * p[0], 0.8 - 0.5 * p[0], 1.0]);
    let normal = materialcraft_engine::nodes::atomic::height_to_normal(&gray, 4.0, false, true, false);
    (gray, color, normal)
}

fn previews() -> Previews {
    let (gray, color, normal) = sample_inputs();
    let no_res = |_: &str| None;
    let vars = BTreeMap::new();
    let mut out = Vec::new();
    for def in catalog() {
        if def.id == "graph_instance" {
            continue;
        }
        let inputs: Vec<Option<Image>> = def
            .inputs
            .iter()
            .enumerate()
            .map(|(i, p)| {
                // Masks and secondary inputs stay empty so the main effect shows
                if i > 0 && !matches!(def.id, "blend" | "height_blend" | "normal_blend" | "rgba_merge") {
                    return None;
                }
                Some(if p.id.contains("normal") || def.category.contains("Normal") && p.kind == materialcraft_engine::nodes::PortKind::Color {
                    normal.clone()
                } else if p.kind == materialcraft_engine::nodes::PortKind::Color {
                    color.clone()
                } else if i == 1 {
                    gray.map(Kind::Gray, |v| [1.0 - v[0], 0.0, 0.0, 0.0])
                } else {
                    gray.clone()
                })
            })
            .collect();
        let params = resolve_params(def, &BTreeMap::new());
        let ctx = CookContext {
            w: LIBRARY_PREVIEW_SIZE,
            h: LIBRARY_PREVIEW_SIZE,
            seed: 3,
            tile: true,
            inputs: &inputs,
            params: &params,
            resource: &no_res,
            vars: &vars,
        };
        if let Ok(imgs) = (def.cook)(&ctx)
            && let Some(img) = imgs.first()
        {
            out.push((def.id.to_string(), img.resize(LIBRARY_PREVIEW_SIZE, LIBRARY_PREVIEW_SIZE, true).to_rgba8_display()));
        }
    }
    out
}

#[cfg(not(target_arch = "wasm32"))]
fn spawn_library_previews(ctx: egui::Context) -> Receiver<Previews> {
    let (tx, rx) = std::sync::mpsc::channel();
    let _ = std::thread::Builder::new().name("materialcraft-library".into()).spawn(move || {
        let _ = tx.send(previews());
        ctx.request_repaint();
    });
    rx
}

#[cfg(target_arch = "wasm32")]
fn spawn_library_previews(_ctx: egui::Context) -> Receiver<Previews> {
    let (tx, rx) = std::sync::mpsc::channel();
    let _ = tx.send(previews());
    rx
}
