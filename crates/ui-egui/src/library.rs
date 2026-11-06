//! The library: a category tree on the left, the matching nodes as preview tiles (or a list) on
//! the right, with search. Drag a tile into the graph, or double-click it to add it next to the
//! selection.

use egui::{Color32, Rect, RichText, Sense, Ui, vec2};
use egui_phosphor::regular as icon;

use crate::MaterialCraftApp;
use crate::graph::{LibraryDrag, header_color};
use crate::i18n::tr;
use crate::theme::Tokens;

/// The tree: (top level, its sub-categories as full category paths).
fn tree() -> Vec<(&'static str, Vec<&'static str>)> {
    let mut out: Vec<(&'static str, Vec<&'static str>)> = Vec::new();
    for (cat, _) in materialcraft_engine::nodes::categories() {
        let top = cat.split('/').next().unwrap_or(cat);
        match out.iter_mut().find(|(t, _)| *t == top) {
            Some((_, subs)) => {
                if cat != top {
                    subs.push(cat);
                }
            }
            None => out.push((top, if cat != top { vec![cat] } else { Vec::new() })),
        }
    }
    out
}

fn category_icon(top: &str) -> &'static str {
    match top {
        "Atomic nodes" => icon::CUBE,
        "Graph items" => icon::FLOW_ARROW,
        "Texture generators" => icon::DICE_FIVE,
        "Filters" => icon::FADERS,
        "Material filters" => icon::SPHERE,
        _ => icon::FOLDER_OPEN,
    }
}

pub fn show(app: &mut MaterialCraftApp, ui: &mut Ui) {
    match crate::graph::editor_target::EditorTarget::resolve(app) {
        crate::graph::editor_target::EditorTarget::Function => {
            crate::function::library::show(app, ui);
            return;
        }
        crate::graph::editor_target::EditorTarget::Pattern => {
            crate::pattern::library::show(app, ui);
            return;
        }
        crate::graph::editor_target::EditorTarget::Material => {}
    }

    let tk = Tokens::get(ui.ctx());
    ui.horizontal(|ui| {
        ui.label(RichText::new(icon::MAGNIFYING_GLASS).color(tk.text_dim));
        let r = ui.add(egui::TextEdit::singleline(&mut app.ui.library_search).hint_text(tr("Search")).desired_width((ui.available_width() - 56.0).max(40.0)));
        crate::widgets::name(&r, egui::Role::SearchInput, "Search the library");
        if crate::widgets::icon_button(ui, icon::SQUARES_FOUR, tr("Icons"), !app.ui.prefs.library_list, 20.0).clicked() {
            app.ui.prefs.library_list = false;
        }
        if crate::widgets::icon_button(ui, icon::LIST, tr("List"), app.ui.prefs.library_list, 20.0).clicked() {
            app.ui.prefs.library_list = true;
        }
    });
    ui.add_space(2.0);
    let avail = ui.available_size();
    let tree_w = (avail.x * 0.36).clamp(90.0, 170.0);
    let top_down = egui::Layout::top_down(egui::Align::Min);
    ui.horizontal_top(|ui| {
        ui.allocate_ui_with_layout(vec2(tree_w, avail.y), top_down, |ui| {
            egui::ScrollArea::vertical().id_salt("lib-tree").auto_shrink([false, false]).show(ui, |ui| category_tree(app, ui));
        });
        ui.separator();
        ui.allocate_ui_with_layout(vec2(ui.available_width(), avail.y), top_down, |ui| {
            egui::ScrollArea::vertical().id_salt("lib-items").auto_shrink([false, false]).show(ui, |ui| items(app, ui));
        });
    });
}

fn category_tree(app: &mut MaterialCraftApp, ui: &mut Ui) {
    let tk = Tokens::get(ui.ctx());
    let mut row = |ui: &mut Ui, text: String, path: &str, indent: f32| {
        let on = app.ui.library_category == path;
        let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 20.0), Sense::click());
        crate::widgets::name(&resp, egui::Role::TreeItem, &text);
        if on {
            ui.painter().rect_filled(r, 4.0, tk.row_selected);
        } else if resp.hovered() {
            ui.painter().rect_filled(r, 4.0, tk.row_hover);
        }
        let g = crate::widgets::elided(ui.painter(), text, 11.5, if on { tk.text_strong } else { tk.text }, r.width() - indent - 4.0);
        ui.painter().galley(r.left_center() + vec2(indent, -g.size().y * 0.5), g, tk.text);
        if resp.clicked() {
            app.ui.library_category = path.to_string();
        }
    };
    row(ui, format!("{}  {}", icon::SQUARES_FOUR, tr("All nodes")), "", 4.0);
    if app.session.pkg.graphs.len() > 1 {
        row(ui, format!("{}  {}", icon::TREE_STRUCTURE, tr("This package")), "@package", 4.0);
    }
    for (top, subs) in tree() {
        row(ui, format!("{}  {}", category_icon(top), tr(top)), top, 4.0);
        for sub in subs {
            let name = sub.rsplit('/').next().unwrap_or(sub);
            row(ui, tr(name).to_string(), sub, 22.0);
        }
    }
}

/// Library entries for the current filter: (id, label, category, is a package graph).
fn entries(app: &MaterialCraftApp) -> Vec<(String, String, &'static str, bool)> {
    let search = app.ui.library_search.to_ascii_lowercase();
    let cat = &app.ui.library_category;
    let mut out: Vec<(String, String, &'static str, bool)> = Vec::new();
    if cat == "@package" || !search.is_empty() {
        for (i, g) in app.session.pkg.graphs.iter().enumerate() {
            if i != app.session.graph && (search.is_empty() || g.name.to_ascii_lowercase().contains(&search)) {
                out.push((g.name.clone(), g.name.clone(), "This package", true));
            }
        }
        if cat == "@package" {
            return out;
        }
    }
    for d in materialcraft_engine::nodes::catalog() {
        if d.id == "graph_instance" {
            continue;
        }
        if !(cat.is_empty() || d.category == cat || d.category.starts_with(&format!("{cat}/"))) {
            continue;
        }
        if !(search.is_empty() || d.label.to_ascii_lowercase().contains(&search) || d.id.contains(&search)) {
            continue;
        }
        out.push((d.id.to_string(), d.label.to_string(), d.category, false));
    }
    out
}

fn items(app: &mut MaterialCraftApp, ui: &mut Ui) {
    let tk = Tokens::get(ui.ctx());
    let ctx = ui.ctx().clone();
    let list = entries(app);
    if list.is_empty() {
        ui.label(RichText::new(tr("Nothing matches")).color(tk.text_dim));
        return;
    }
    let as_list = app.ui.prefs.library_list;
    let tile = vec2(78.0, 98.0);
    let cols = if as_list { 1 } else { ((ui.available_width() + 4.0) / (tile.x + 4.0)).floor().max(1.0) as usize };
    let mut add: Option<(String, bool)> = None;
    for chunk in list.chunks(cols) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
            for (id, label, cat, is_graph) in chunk {
                let size = if as_list { vec2(ui.available_width(), 26.0) } else { tile };
                let (r, resp) = ui.allocate_exact_size(size, Sense::click_and_drag());
                crate::widgets::name(&resp, egui::Role::ListItem, label);
                let def_id = if *is_graph { "graph_instance" } else { id.as_str() };
                if resp.hovered() {
                    ui.painter().rect_filled(r, 6.0, tk.row_hover);
                }
                let tex = if *is_graph { None } else { app.thumbs.library(&ctx, id) };
                let img_rect = if as_list {
                    Rect::from_min_size(r.min + vec2(2.0, 2.0), vec2(22.0, 22.0))
                } else {
                    Rect::from_min_size(r.min + vec2(5.0, 4.0), vec2(68.0, 68.0))
                };
                match tex {
                    Some(t) => {
                        ui.painter().image(t, img_rect, Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)), Color32::WHITE);
                    }
                    None => {
                        ui.painter().rect_filled(img_rect, 4.0, header_color(def_id).gamma_multiply(0.7));
                        ui.painter().text(
                            img_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            if *is_graph { icon::TREE_STRUCTURE } else { icon::CUBE },
                            egui::FontId::proportional(img_rect.height() * 0.4),
                            Color32::WHITE,
                        );
                    }
                }
                // The category color as a strip, like the node headers.
                let strip = Rect::from_min_size(img_rect.left_bottom() - vec2(0.0, 3.0), vec2(img_rect.width(), 3.0));
                ui.painter().rect_filled(strip, 0.0, header_color(def_id));
                if as_list {
                    let g = crate::widgets::elided(ui.painter(), label.clone(), 11.5, tk.text, r.width() - 34.0);
                    ui.painter().galley(r.left_center() + vec2(30.0, -g.size().y * 0.5), g, tk.text);
                } else {
                    let galley = ui.painter().layout(label.clone(), egui::FontId::proportional(10.5), tk.text, tile.x - 4.0);
                    let pos = egui::pos2(r.center().x - galley.size().x * 0.5, img_rect.bottom() + 3.0);
                    ui.painter().galley(pos, galley, tk.text);
                }
                let resp = resp.on_hover_text(format!("{label}\n{cat}"));
                if resp.drag_started() {
                    let payload = if *is_graph { format!("@{id}") } else { id.clone() };
                    egui::DragAndDrop::set_payload(&ctx, LibraryDrag(payload));
                }
                if resp.double_clicked() {
                    add = Some((id.clone(), *is_graph));
                }
            }
        });
    }
    if let Some((id, is_graph)) = add {
        if is_graph {
            app.run("graph.node.add", serde_json::json!({"def": "graph_instance", "params": {"graph": id}}));
        } else {
            crate::graph::add_node_near_selection(app, &id);
        }
    }
}
