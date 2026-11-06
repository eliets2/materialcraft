//! The 2D view: one node output at full preview resolution, with pan and zoom, channel isolation,
//! a 3×3 tiling check, a histogram, a pixel readout and the size/format line.

use egui::{Color32, ColorImage, Rect, Sense, TextureHandle, TextureOptions, Ui, pos2, vec2};
use egui_phosphor::regular as icon;
use materialcraft_engine::image::{Image, Kind};
use serde_json::json;

use crate::MaterialCraftApp;
use crate::i18n::tr;
use crate::theme::Tokens;

#[derive(Default)]
pub struct View2dState {
    tex: Option<(u64, TextureHandle)>,
    /// The last shown image (RGBA8, for `view.screenshot`).
    pub shown: Option<(u32, u32, Vec<u8>)>,
    /// Per-channel histograms of `shown` (R, G, B; one channel for gray), keyed like `tex`.
    histogram: Option<(u64, Vec<[u32; BINS]>)>,
}

const BINS: usize = 64;

/// Histograms of the displayed 8-bit values: one for gray or isolated channels, else R, G and B.
fn histogram(px: &[u8], channels: usize) -> Vec<[u32; BINS]> {
    let mut h = vec![[0u32; BINS]; channels];
    for p in px.chunks_exact(4) {
        for (c, bins) in h.iter_mut().enumerate() {
            bins[p[c] as usize * BINS / 256] += 1;
        }
    }
    h
}

pub fn title(app: &MaterialCraftApp) -> String {
    let name = app.session.view2d_image().and_then(|(id, _)| app.session.graph().ok()?.node(id).ok().map(crate::graph::node_label));
    format!("{} - {}", name.unwrap_or_default(), tr("2D View"))
}

fn rgba8(img: &Image, channel: &str) -> Vec<u8> {
    let base = img.to_rgba8_display();
    let pick = match channel {
        "r" => Some(0),
        "g" => Some(1),
        "b" => Some(2),
        "a" => Some(3),
        _ => None,
    };
    match pick {
        Some(k) if img.kind == Kind::Color => base.chunks(4).flat_map(|p| [p[k], p[k], p[k], 255]).collect(),
        _ => base,
    }
}

pub fn show(app: &mut MaterialCraftApp, ui: &mut Ui) {
    let tk = Tokens::get(ui.ctx());
    let ctx = ui.ctx().clone();
    // Toolbar.
    egui::Frame::NONE.fill(tk.panel).inner_margin(egui::Margin::symmetric(6, 3)).show(ui, |ui| {
        ui.horizontal(|ui| {
            crate::preview_resolution::show(app, ui);
            for (c, label) in [("rgba", "RGBA"), ("r", "R"), ("g", "G"), ("b", "B"), ("a", "A")] {
                if crate::widgets::chip(ui, label, app.session.view2d.channel == c).clicked() {
                    app.run("view.2d", json!({"channel": c}));
                }
            }
            ui.separator();
            if crate::widgets::icon_button(ui, icon::GRID_FOUR, tr("Tile 3x3"), app.session.view2d.tiled, 20.0).clicked() {
                let t = !app.session.view2d.tiled;
                app.run("view.2d", json!({"tiled": t}));
            }
            if crate::widgets::icon_button(ui, icon::CHART_BAR, tr("Histogram"), app.session.view2d.histogram, 20.0).clicked() {
                let h = !app.session.view2d.histogram;
                app.run("view.2d", json!({"histogram": h}));
            }
            if crate::widgets::icon_button(ui, icon::CORNERS_OUT, tr("Fit"), app.ui.image_camera.fit, 20.0).clicked() {
                app.ui.image_camera = crate::state::ImageCamera::default();
            }
            if ui.small_button("1:1").on_hover_text(tr("Actual pixels")).clicked() {
                app.ui.image_camera = crate::state::ImageCamera { zoom: 1.0, offset: [0.0, 0.0], fit: false };
            }
            #[cfg(not(target_arch = "wasm32"))]
            if crate::widgets::icon_button(ui, icon::DOWNLOAD_SIMPLE, tr("Save image..."), false, 20.0).clicked() {
                save_image(app);
            }
        });
    });
    let Some((id, img)) = app.session.view2d_image().map(|(id, i)| (id, i.clone())) else {
        let (r, resp) = ui.allocate_exact_size(ui.available_size(), Sense::hover());
        crate::widgets::name(&resp, egui::Role::Image, "2D view");
        ui.painter().rect_filled(r, 0.0, tk.viewport);
        ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, tr("Click a node to show it here"), egui::FontId::proportional(12.0), tk.viewport_text);
        return;
    };
    let texture_limit = ctx.input(|i| i.max_texture_side).max(1);
    let key = app.session.cooked.nodes.get(&id).map_or(0, |r| r.key)
        ^ (texture_limit as u64).rotate_left(32)
        ^ (img.w as u64).rotate_left(16)
        ^ img.h as u64
        ^ (app.session.view2d.port as u64).wrapping_mul(0x9e37)
        ^ (app.session.view2d.channel.len() as u64 * 31)
        ^ app.session.view2d.channel.bytes().next().unwrap_or(0) as u64;
    if app.view2d.tex.as_ref().is_none_or(|(k, _)| *k != key) {
        let px = rgba8(&img, &app.session.view2d.channel);
        // Keep source pixels for readout/screenshots, but respect the renderer's upload limit.
        let ci = if img.w.max(img.h) as usize > texture_limit {
            let scale = texture_limit as f64 / f64::from(img.w.max(img.h));
            let display = img.resize((f64::from(img.w) * scale).floor().max(1.0) as u32, (f64::from(img.h) * scale).floor().max(1.0) as u32, false);
            ColorImage::from_rgba_unmultiplied([display.w as usize, display.h as usize], &rgba8(&display, &app.session.view2d.channel))
        } else {
            ColorImage::from_rgba_unmultiplied([img.w as usize, img.h as usize], &px)
        };
        app.view2d.tex = Some((key, ctx.load_texture("view2d", ci, TextureOptions::NEAREST)));
        app.view2d.shown = Some((img.w, img.h, px));
    }
    let avail = ui.available_size();
    let (rect, resp) = ui.allocate_exact_size(vec2(avail.x, (avail.y - 20.0).max(40.0)), Sense::click_and_drag());
    crate::widgets::name(&resp, egui::Role::Image, "2D view");
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 0.0, tk.viewport);
    // Camera: fit, wheel zoom around the pointer, drag to pan.
    app.ui.image_camera = app.ui.image_camera.sanitized();
    let cam = &mut app.ui.image_camera;
    let tiles = if app.session.view2d.tiled { 3.0 } else { 1.0 };
    let fit_zoom = ((rect.width() - 16.0) / (img.w as f32 * tiles)).min((rect.height() - 16.0) / (img.h as f32 * tiles)).max(0.01);
    if cam.fit {
        cam.zoom = fit_zoom;
        cam.offset = [0.0, 0.0];
    }
    if resp.hovered() {
        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        if scroll.abs() > 0.0 {
            cam.fit = false;
            let old = cam.zoom;
            cam.zoom = (cam.zoom * (scroll * 0.003).exp()).clamp(0.02, 64.0);
            if let Some(ptr) = resp.hover_pos() {
                let c = rect.center() + vec2(cam.offset[0], cam.offset[1]);
                let d = ptr - c;
                let k = cam.zoom / old;
                cam.offset = [cam.offset[0] - d.x * (k - 1.0), cam.offset[1] - d.y * (k - 1.0)];
            }
        }
    }
    if resp.dragged() {
        cam.fit = false;
        let d = resp.drag_delta();
        cam.offset = [cam.offset[0] + d.x, cam.offset[1] + d.y];
    }
    let size = vec2(img.w as f32, img.h as f32) * cam.zoom;
    let center = rect.center() + vec2(cam.offset[0], cam.offset[1]);
    let full = Rect::from_center_size(center, size * tiles);
    // A checker behind transparent images.
    if img.kind == Kind::Color && app.session.view2d.channel == "rgba" {
        let cell = 8.0;
        let vis = full.intersect(rect);
        let mut y = vis.top();
        let mut row = 0;
        while y < vis.bottom() {
            let mut x = vis.left() + if row % 2 == 0 { 0.0 } else { cell };
            while x < vis.right() {
                p.rect_filled(Rect::from_min_size(pos2(x, y), vec2(cell, cell)).intersect(vis), 0.0, Color32::from_gray(0x55));
                x += cell * 2.0;
            }
            y += cell;
            row += 1;
        }
    }
    if let Some((_, tex)) = &app.view2d.tex {
        let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(tiles, tiles));
        let mut mesh = egui::Mesh::with_texture(tex.id());
        mesh.add_rect_with_uv(full, uv, Color32::WHITE);
        p.add(mesh);
        if tiles > 1.0 {
            p.rect_stroke(Rect::from_center_size(center, size), 0.0, egui::Stroke::new(1.0, Color32::from_white_alpha(60)), egui::StrokeKind::Outside);
        }
    }
    if app.session.view2d.histogram {
        let single = img.is_gray() || app.session.view2d.channel != "rgba";
        draw_histogram(&mut app.view2d, &p, rect, key, single);
    }
    // Info line: size, format, zoom and the pixel under the pointer.
    let info = Rect::from_min_size(pos2(rect.left(), rect.bottom()), vec2(rect.width(), 20.0));
    let bits = app
        .session
        .graph()
        .ok()
        .and_then(|g| {
            g.node(id).ok().map(|n| if n.base.format == materialcraft_engine::core::OutputFormat::Inherit { g.format.bits() } else { n.base.format.bits() })
        })
        .unwrap_or("16");
    let mut text = format!("{} x {} ({}, {}bpc)   {:.0}%", img.w, img.h, if img.is_gray() { "L" } else { "RGBA" }, bits, cam.zoom * 100.0);
    if let Some(ptr) = resp.hover_pos() {
        let local = (ptr - (center - size * 0.5)) / cam.zoom;
        let (x, y) = (local.x.floor() as i64, local.y.floor() as i64);
        let v = img.get(x, y, true);
        if img.is_gray() {
            text += &format!("   ({}, {})  {:.3}", x.rem_euclid(img.w as i64), y.rem_euclid(img.h as i64), v[0]);
        } else {
            text += &format!("   ({}, {})  {:.3} {:.3} {:.3} {:.3}", x.rem_euclid(img.w as i64), y.rem_euclid(img.h as i64), v[0], v[1], v[2], v[3]);
        }
    }
    ui.painter().rect_filled(info, 0.0, tk.panel);
    ui.painter().text(info.left_center() + vec2(6.0, 0.0), egui::Align2::LEFT_CENTER, text, egui::FontId::proportional(11.0), tk.text_dim);
}

/// The histogram card in the view's bottom-right corner, computed once per shown image.
fn draw_histogram(st: &mut View2dState, p: &egui::Painter, rect: Rect, key: u64, single: bool) {
    if st.histogram.as_ref().is_none_or(|(k, _)| *k != key) {
        let Some((_, _, px)) = &st.shown else { return };
        st.histogram = Some((key, histogram(px, if single { 1 } else { 3 })));
    }
    let Some((_, hist)) = &st.histogram else { return };
    let card = Rect::from_min_size(rect.right_bottom() - vec2(BINS as f32 * 3.0 + 16.0, 96.0), vec2(BINS as f32 * 3.0 + 8.0, 88.0));
    p.rect_filled(card, 4.0, Color32::from_black_alpha(170));
    let plot = card.shrink(6.0);
    let peak = hist.iter().flat_map(|b| b.iter()).copied().max().unwrap_or(1).max(1) as f32;
    let colors: &[Color32] = if single {
        &[Color32::from_gray(210)]
    } else {
        &[
            Color32::from_rgba_unmultiplied(235, 80, 80, 150),
            Color32::from_rgba_unmultiplied(80, 210, 90, 150),
            Color32::from_rgba_unmultiplied(90, 130, 255, 150),
        ]
    };
    let w = plot.width() / BINS as f32;
    for (bins, color) in hist.iter().zip(colors) {
        for (i, n) in bins.iter().enumerate() {
            let h = plot.height() * (*n as f32 / peak).sqrt();
            let x = plot.left() + i as f32 * w;
            p.rect_filled(Rect::from_min_max(pos2(x, plot.bottom() - h), pos2(x + w, plot.bottom())), 0.0, *color);
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn save_image(app: &mut MaterialCraftApp) {
    let Some((id, img)) = app.session.view2d_image().map(|(id, i)| (id, i.clone())) else { return };
    let name = app.session.graph().ok().and_then(|g| g.node(id).ok().map(crate::graph::node_label)).unwrap_or_else(|| "image".into());
    let Some(path) = rfd::FileDialog::new()
        .set_file_name(format!("{name}.png"))
        .add_filter("PNG", &["png"])
        .add_filter("OpenEXR", &["exr"])
        .add_filter("TGA", &["tga"])
        .save_file()
    else {
        return;
    };
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("png").to_ascii_lowercase();
    let depth = if ext == "exr" { materialcraft_engine::formats::Depth::F32 } else { materialcraft_engine::formats::Depth::U16 };
    let result = materialcraft_engine::formats::encode(&img, &ext, depth, !img.is_gray()).and_then(|b| materialcraft_engine::formats::write_atomic(&path, &b));
    match result {
        Ok(()) => app.toast(format!("{} {}", tr("Saved"), path.display()), false),
        Err(e) => app.toast(e.to_string(), true),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn histogram_counts_every_pixel_once_per_channel() {
        let px = [0, 128, 255, 255, 255, 128, 0, 255];
        let h = super::histogram(&px, 3);
        assert_eq!(h.len(), 3);
        assert!(h.iter().all(|b| b.iter().sum::<u32>() == 2));
        assert_eq!(h[0][0], 1);
        assert_eq!(h[0][super::BINS - 1], 1);
        assert_eq!(h[1][32], 2);
    }
}
