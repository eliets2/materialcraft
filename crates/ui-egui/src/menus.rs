//! Menu actions and shortcuts share stable command ids

use egui::{Key, KeyboardShortcut, Modifiers};
use serde_json::Value;

use crate::MaterialCraftApp;
use crate::i18n::tr;
use crate::theme::Tokens;

pub use crate::menu_item::MenuItem;

const fn it(menu: &'static str, label: &'static str, id: &'static str, shortcut: Option<&'static str>) -> MenuItem {
    MenuItem { menu, sub: None, label, id, shortcut }
}

const fn sub(menu: &'static str, sub: &'static str, label: &'static str, id: &'static str, shortcut: Option<&'static str>) -> MenuItem {
    MenuItem { menu, sub: Some(sub), label, id, shortcut }
}

const fn sep(menu: &'static str) -> MenuItem {
    MenuItem { menu, sub: None, label: "-", id: "", shortcut: None }
}

pub const TOP: [&str; 5] = ["File", "Edit", "Tools", "Windows", "Help"];

/// The menu catalog (the target menu tree with our ids)
pub const CATALOG: &[MenuItem] = &[
    sub("File", "New", "Graph...", "ui.newGraph", Some("Ctrl+N")),
    sub("File", "New", "Function Graph...", "ui.newFunctionGraph", None),
    sub("File", "New", "Empty package", "file.newPackage", None),
    it("File", "Open...", "file.open", Some("Ctrl+O")),
    sep("File"),
    it("File", "Save", "file.save", Some("Ctrl+S")),
    it("File", "Save as...", "file.saveAs", Some("Ctrl+Shift+S")),
    it("File", "Close all", "ui.closeAll", None),
    sep("File"),
    it("File", "Import bitmap...", "file.importBitmap", None),
    it("File", "Export outputs...", "ui.export", Some("Ctrl+Shift+E")),
    it("File", "Export material...", "file.exportMaterial", None),
    it("File", "Send to TextureCraft", "file.sendToTextureCraft", None),
    it("File", "Reload resources", "ui.reloadResources", None),
    sep("File"),
    it("File", "Exit", "ui.quit", Some("Ctrl+Q")),
    it("Edit", "Undo", "edit.undo", Some("Ctrl+Z")),
    it("Edit", "Redo", "edit.redo", Some("Ctrl+Y")),
    sep("Edit"),
    it("Edit", "Copy", "edit.copy", Some("Ctrl+C")),
    it("Edit", "Cut", "edit.cut", Some("Ctrl+X")),
    it("Edit", "Paste", "edit.paste", Some("Ctrl+V")),
    it("Edit", "Duplicate", "edit.duplicate", Some("Ctrl+D")),
    it("Edit", "Delete", "edit.delete", Some("Delete")),
    it("Edit", "Delete and relink", "edit.deleteRelink", Some("Backspace")),
    it("Edit", "Select all", "edit.selectAll", Some("Ctrl+A")),
    it("Edit", "Edit node graph", "graph.node.edit", Some("Ctrl+E")),
    it("Edit", "Parent graph", "ui.graph.parent", Some("Alt+ArrowLeft")),
    sep("Edit"),
    it("Edit", "Preferences...", "ui.preferences", None),
    it("Tools", "Compute graph", "graph.cook", None),
    it("Tools", "Snap to grid", "ui.graph.snapToGrid", None),
    it("Tools", "Compute nodes thumbnails", "ui.graph.computeThumbnails", None),
    it("Tools", "Clear nodes thumbnails", "ui.graph.clearThumbnails", Some("F5")),
    it("Tools", "Previous pin", "ui.graph.previousPin", Some("Shift+F2")),
    it("Tools", "Next pin", "ui.graph.nextPin", Some("F2")),
    it("Tools", "View outputs in 3D view", "ui.graph.viewOutputs", None),
    it("Tools", "Reset and view outputs in 3D view", "ui.graph.resetViewOutputs", None),
    it("Tools", "Clear cache", "ui.clearCache", None),
    sub("Tools", "Preview size", "256 px", "ui.preview.256", None),
    sub("Tools", "Preview size", "512 px", "ui.preview.512", None),
    sub("Tools", "Preview size", "1024 px", "ui.preview.1024", None),
    sub("Tools", "Preview size", "2048 px", "ui.preview.2048", None),
    sub("Tools", "Preview size", "Full size", "ui.preview.0", None),
    it("Tools", "Create graph from selection", "graph.createFromSelection", None),
    it("Windows", "Explorer", "ui.view.explorer", None),
    it("Windows", "Graph", "ui.view.graph", None),
    it("Windows", "Properties", "ui.view.properties", None),
    it("Windows", "Console", "ui.view.console", None),
    it("Windows", "3D View", "ui.view.3d", None),
    it("Windows", "2D View", "ui.view.2d", None),
    it("Windows", "Library", "ui.view.library", None),
    it("Windows", "History", "ui.view.history", None),
    sep("Windows"),
    it("Windows", "Reset layout", "ui.resetLayout", None),
    it("Help", "Home screen...", "ui.home", None),
    it("Help", "Documentation...", "ui.help.docs", None),
    it("Help", "Shortcuts list...", "ui.help.shortcuts", None),
    sep("Help"),
    it("Help", "Join the community (Discord)...", "ui.help.discord", None),
    it("Help", "ArtCraft website...", "ui.help.website", None),
    it("Help", "Report a bug...", "ui.help.bug", None),
    sep("Help"),
    it("Help", "About MaterialCraft...", "ui.about", None),
];

fn is_ui(id: &str) -> bool {
    id.starts_with("ui.")
}

/// Whether an item can run now
pub fn enabled(app: &MaterialCraftApp, id: &str) -> bool {
    if let Some(enabled) = crate::graph::editor_actions::enabled(app, id) {
        return enabled;
    }

    if let Some(enabled) = crate::graph::actions::enabled(app, id) {
        return enabled;
    }

    if matches!(id, "edit.paste" | "edit.pasteWithoutLinks") {
        return app.session.graph().is_ok() && app.session.clipboard.as_ref().is_some_and(|clip| !clip.nodes.is_empty());
    }

    if is_ui(id) {
        return true;
    }
    materialcraft_engine::find(id).is_some_and(|c| (c.enabled)(&app.session).is_ok())
}

/// Whether an id is implemented at all (parity report)
pub fn implemented(id: &str) -> bool {
    is_ui(id) || materialcraft_engine::find(id).is_some()
}

/// Invoke a menu item (menus, shortcuts, the control channel's `ui.menu.invoke`)
pub fn invoke(app: &mut MaterialCraftApp, ctx: &egui::Context, id: &str) {
    if crate::graph::editor_actions::invoke(app, id) {
        return;
    }

    if crate::graph::actions::invoke(app, id) {
        return;
    }

    if let Some(panel) = crate::docking::view_panel(id) {
        app.ui.dock.toggle(panel);
        return;
    }
    if let Some(size) = id.strip_prefix("ui.preview.").and_then(|s| s.parse::<u32>().ok()) {
        app.ui.prefs.preview_size = size;
        app.apply_prefs();
        return;
    }
    match id {
        "ui.newGraph" | "ui.newFunctionGraph" => {
            app.ui.new_graph_kind =
                if id == "ui.newFunctionGraph" { materialcraft_engine::core::GraphKind::Function } else { materialcraft_engine::core::GraphKind::Material };
            app.ui.windows.new_graph = true;
        }
        "ui.export" => app.ui.windows.export = true,
        "ui.quit" => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
        "ui.preferences" => app.ui.windows.preferences = true,
        "ui.view.graph" => {}
        "ui.resetLayout" => app.ui.dock = crate::docking::Dock::default(),
        // These replace the open package: ask first when it has unsaved changes.
        "ui.closeAll" | "file.newPackage" => crate::unsaved::guard(app, crate::unsaved::Guarded::Menu { id: id.to_string() }),
        "ui.reloadResources" | "ui.clearCache" => {
            if let Ok(mut c) = app.session.cache.lock() {
                c.clear();
            }
            app.session.dirty = true;
        }
        "ui.home" => app.ui.windows.home = true,
        "ui.help.shortcuts" => app.ui.windows.shortcuts = true,
        "ui.about" => app.ui.windows.about = true,
        "ui.help.discord" => ctx.open_url(egui::OpenUrl::new_tab("https://discord.gg/artcraft")),
        "ui.help.website" => ctx.open_url(egui::OpenUrl::new_tab("https://getartcraft.com/apps/materialcraft")),
        "ui.help.docs" => ctx.open_url(egui::OpenUrl::new_tab("https://github.com/storytold/materialcraft#readme")),
        "ui.help.bug" => ctx.open_url(egui::OpenUrl::new_tab("https://github.com/storytold/materialcraft/issues")),
        _ => {
            if crate::windows::file_action(app, ctx, id) {
                return;
            }
            app.run(id, Value::Null);
        }
    }
}

/// Parse "Ctrl+Shift+E" style shortcuts
pub fn parse_shortcut(s: &str) -> Option<KeyboardShortcut> {
    let mut mods = Modifiers::NONE;
    let mut key = None;
    for part in s.split('+') {
        match part {
            "Ctrl" | "Cmd" => mods |= Modifiers::COMMAND,
            "Shift" => mods |= Modifiers::SHIFT,
            "Alt" => mods |= Modifiers::ALT,
            k => key = Key::from_name(k),
        }
    }
    key.map(|k| KeyboardShortcut::new(mods, k))
}

/// The shortcut text shown for `id` (the catalog's, else the engine command's)
pub fn shortcut_of(id: &str) -> Option<&'static str> {
    CATALOG.iter().find(|i| i.id == id).and_then(|i| i.shortcut).or_else(|| materialcraft_engine::find(id).and_then(|c| c.shortcut))
}

pub fn menu_bar(app: &mut MaterialCraftApp, ui: &mut egui::Ui) {
    let tk = Tokens::get(ui.ctx());
    egui::Panel::top("menu_bar").exact_size(24.0).frame(egui::Frame::NONE.fill(tk.chrome).inner_margin(egui::Margin::symmetric(6, 0))).show(ui, |ui| {
        egui::MenuBar::new().ui(ui, |ui| {
            let ctx = ui.ctx().clone();
            for top in TOP {
                ui.menu_button(tr(top), |ui| {
                    ui.set_min_width(220.0);
                    let mut current_sub: Option<&str> = None;
                    let items: Vec<MenuItem> = CATALOG.iter().filter(|i| i.menu == top).copied().collect();
                    for item in &items {
                        if let Some(s) = item.sub {
                            if current_sub == Some(s) {
                                continue;
                            }
                            current_sub = Some(s);
                            ui.menu_button(tr(s), |ui| {
                                for x in items.iter().filter(|x| x.sub == Some(s)) {
                                    menu_item(app, &ctx, ui, x);
                                }
                            });
                            continue;
                        }
                        if item.label == "-" {
                            ui.separator();
                            continue;
                        }
                        menu_item(app, &ctx, ui, item);
                    }
                });
            }
        });
    });
}

fn menu_item(app: &mut MaterialCraftApp, ctx: &egui::Context, ui: &mut egui::Ui, item: &MenuItem) {
    let on = enabled(app, item.id);
    let mut b = egui::Button::new(tr(item.label));
    if let Some(sc) = item.shortcut {
        b = b.shortcut_text(sc);
    }
    let checked = match item.id {
        "ui.view.graph" => Some(true),
        "ui.graph.snapToGrid" => Some(app.ui.prefs.snap_to_grid),
        id if id.starts_with("ui.preview.") => id.strip_prefix("ui.preview.").and_then(|s| s.parse::<u32>().ok()).map(|s| s == app.ui.prefs.preview_size),
        id => crate::docking::view_panel(id).map(|p| app.ui.dock.is_open(p)),
    };

    if let Some(c) = checked {
        b = b.selected(c);
    }
    if ui.add_enabled(on, b).clicked() {
        invoke(app, ctx, item.id);
        ui.close();
    }
    #[cfg(not(target_arch = "wasm32"))]
    if item.id == "file.open" {
        ui.add_enabled_ui(!app.ui.recent.is_empty(), |ui| {
            ui.menu_button(tr("Recent packages"), |ui| {
                for path in app.ui.recent.clone() {
                    if ui.button(&path).clicked() {
                        crate::windows::open_path(app, &path);
                        ui.close();
                    }
                }
                ui.separator();
                if ui.button(tr("Clear recent packages")).clicked() {
                    app.ui.recent.clear();
                    ui.close();
                }
            });
        });
    }
}

/// Dispatch shortcuts unless a widget or popup owns the keyboard
pub fn shortcuts(app: &mut MaterialCraftApp, ctx: &egui::Context) {
    if ctx.egui_wants_keyboard_input() || egui::Popup::is_any_open(ctx) {
        return;
    }
    if !ctx.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::Key { pressed: true, .. }))) {
        return;
    }

    // Selected wires handle deletion and swapping before nodes
    if crate::graph::editor_target::EditorTarget::resolve(app) == crate::graph::editor_target::EditorTarget::Material && !app.graph.selected_wires.is_empty() {
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Delete) | i.consume_key(egui::Modifiers::NONE, egui::Key::Backspace)) {
            crate::graph::delete_selected_wires(app);
        } else if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::X)) {
            crate::graph::swap_selected_wires(app);
        }
    }

    let mut table: Vec<(&'static str, KeyboardShortcut)> = CATALOG.iter().filter_map(|i| Some((i.id, parse_shortcut(i.shortcut?)?))).collect();
    for c in materialcraft_engine::command_specs() {
        if table.iter().any(|(id, _)| *id == c.id) {
            continue;
        }
        if let Some(sc) = c.shortcut.and_then(parse_shortcut) {
            table.push((c.id, sc));
        }
    }

    // Longer chords take priority
    table.sort_by_key(|(_, sc)| std::cmp::Reverse(sc.modifiers.ctrl as u8 + sc.modifiers.shift as u8 + sc.modifiers.alt as u8 + sc.modifiers.command as u8));
    let mut fire: Vec<&str> = Vec::new();
    ctx.input_mut(|i| {
        for (id, sc) in &table {
            if i.consume_shortcut(sc) {
                fire.push(*id);
            }
        }
    });
    for id in fire {
        invoke(app, ctx, id);
    }
}

/// Menu items for `ui.menu.list`
pub fn list(app: &MaterialCraftApp) -> Value {
    Value::Array(
        CATALOG
            .iter()
            .filter(|i| i.label != "-")
            .map(|i| {
                serde_json::json!({"menu": i.menu, "submenu": i.sub, "label": i.label, "id": i.id, "shortcut": i.shortcut,
                    "implemented": implemented(i.id), "enabled": enabled(app, i.id)})
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_shortcuts_parse_and_ids_are_unique() {
        let mut seen = std::collections::BTreeSet::new();
        for i in CATALOG.iter().filter(|i| i.label != "-") {
            assert!(seen.insert(i.id), "duplicate menu id {}", i.id);
            if let Some(sc) = i.shortcut {
                assert!(parse_shortcut(sc).is_some(), "{sc}");
            }
        }
        // Almost every item is live
        let live = CATALOG.iter().filter(|i| i.label != "-" && implemented(i.id)).count();
        assert!(live + 1 >= CATALOG.iter().filter(|i| i.label != "-").count());
    }
}
