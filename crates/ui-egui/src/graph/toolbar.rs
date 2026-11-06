use super::navigation::{add_node_near_selection, view_center};
use crate::{MaterialCraftApp, i18n::tr, theme::Tokens};
use egui::Ui;
use egui_phosphor::regular as icon;
use serde_json::json;

pub(super) fn show(app: &mut MaterialCraftApp, ui: &mut Ui) {
    let function = app.function_editor.is_some();
    let pattern = !function && crate::pattern::active(app);
    let embedded_function = app.function_editor.as_ref().is_some_and(|editor| editor.target != crate::function::FunctionTarget::Graph);
    let function_target = app.function_editor.as_ref().and_then(|editor| editor.command_target());
    let selection: Vec<u64> = if pattern {
        app.pattern_editor.as_ref().map(|editor| editor.selection.iter().copied().map(u64::from).collect()).unwrap_or_default()
    } else if function {
        app.function_editor.as_ref().map(|editor| editor.selection.iter().copied().map(u64::from).collect()).unwrap_or_default()
    } else {
        app.session.selection.iter().map(|id| id.0).collect()
    };
    let graph = app.session.graph;
    let owner = pattern.then(|| app.pattern_editor.as_ref().map(|editor| editor.node.0)).flatten();
    row(ui, |ui| {
        let b = button;
        if common(app, ui, selection.len(), || {
            let mut parameters = json!({"graph": graph, "ids": selection});
            if let Some(owner) = owner {
                parameters["pattern"] = json!(owner);
            }
            if let Some(target) = &function_target {
                parameters["function"] = json!(target);
            }
            parameters
        }) {
            app.ui.frame_all = true;
        }
        if !pattern && !embedded_function {
            ui.separator();
            if b(ui, icon::SELECTION, tr("Add frame around the selection")) {
                let parameters = if function { function_frame(app, &selection) } else { json!({}) };
                app.run("graph.frame.add", parameters);
            }
            if b(ui, icon::NOTE, tr("Add comment")) {
                app.run("graph.comment.add", json!({}));
            }
            if b(ui, icon::MAP_PIN, tr("Add navigation pin")) {
                let center = view_center(app);
                app.run("graph.pin.add", json!({"at": [center.x, center.y]}));
            }
        }

        if !pattern && !function {
            // Quick insert of the atomic nodes
            for (ic, def) in [
                (icon::IMAGE, "bitmap"),
                (icon::PALETTE, "uniform"),
                (icon::STACK, "blend"),
                (icon::DROP, "blur"),
                (icon::SHUFFLE, "channels_shuffle"),
                (icon::CHART_LINE, "curve"),
                (icon::WIND, "directional_blur"),
                (icon::ARROWS_OUT_LINE_HORIZONTAL, "directional_warp"),
                (icon::RULER, "distance"),
                (icon::MOUNTAINS, "emboss"),
                (icon::GRADIENT, "gradient_map"),
                (icon::CIRCLE_HALF, "grayscale_conversion"),
                (icon::SWATCHES, "hsl"),
                (icon::SLIDERS_HORIZONTAL, "levels"),
                (icon::CUBE, "normal"),
                (icon::CODE, "pixel_processor"),
                (icon::DIAMOND, "sharpen"),
                (icon::ARROWS_CLOCKWISE, "transformation"),
                (icon::WAVES, "warp"),
                (icon::SIGN_IN, "input_grayscale"),
                (icon::SIGN_OUT, "output"),
            ] {
                let label = materialcraft_engine::nodes::find(def).map_or(def, |d| d.label);
                if b(ui, ic, label) {
                    add_node_near_selection(app, def);
                }
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if super::subgraph_navigation::can_return(app) {
                if b(ui, icon::ARROW_BEND_UP_LEFT, tr("Return to parent graph")) {
                    crate::menus::invoke(app, ui.ctx(), "ui.graph.parent");
                }
                ui.label(super::subgraph_navigation::breadcrumb(app));
            }
            if pattern {
                if app.pattern_editor.as_ref().and_then(|editor| editor.current(app)).is_some_and(|graph| graph.root.is_none()) {
                    ui.label(tr("Set a node as the root"));
                }
            } else {
                thumbnails(app, ui);
                if function {
                    if app.function_editor.as_ref().and_then(|editor| editor.current(app)).is_some_and(|graph| graph.output.is_none()) {
                        ui.label(tr("Set a node as the output"));
                    }
                } else {
                    parent_size(app, ui);
                }
            }
        });
    });
}

fn row(ui: &mut Ui, draw: impl FnOnce(&mut Ui)) {
    let tokens = Tokens::get(ui.ctx());
    egui::Frame::NONE.fill(tokens.panel).inner_margin(egui::Margin::symmetric(6, 3)).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            draw(ui);
        });
    });
}

fn button(ui: &mut Ui, icon: &str, tooltip: &str) -> bool {
    crate::widgets::icon_button(ui, icon, tooltip, false, 22.0).clicked()
}

fn common(app: &mut MaterialCraftApp, ui: &mut Ui, selection_count: usize, parameters: impl Fn() -> serde_json::Value) -> bool {
    let frame = button(ui, icon::CORNERS_OUT, tr("Frame all (F)"));
    let can_align = selection_count > 1;
    for (icon, mode, tooltip) in [
        (icon::ALIGN_LEFT, "left", "Align left"),
        (icon::ALIGN_TOP, "top", "Align top"),
        (icon::ALIGN_CENTER_HORIZONTAL, "hcenter", "Align centers horizontally"),
        (icon::ALIGN_CENTER_VERTICAL, "vcenter", "Align centers vertically"),
        (icon::COLUMNS, "hdistribute", "Distribute horizontally"),
        (icon::ROWS, "vdistribute", "Distribute vertically"),
    ] {
        if ui.add_enabled_ui(can_align, |ui| button(ui, icon, tr(tooltip))).inner {
            let mut parameters = parameters();
            parameters["mode"] = json!(mode);
            app.run("graph.align", parameters);
        }
    }
    ui.separator();
    if crate::widgets::icon_button(ui, icon::GRID_FOUR, tr("Snap to grid"), app.ui.prefs.snap_to_grid, 22.0).clicked() {
        crate::menus::invoke(app, ui.ctx(), "ui.graph.snapToGrid");
    }
    frame
}

fn thumbnails(app: &mut MaterialCraftApp, ui: &mut Ui) {
    let thumbnails = app.ui.prefs.thumbnails;
    if crate::widgets::icon_button(ui, icon::IMAGE, tr("Show node thumbnails"), thumbnails, 22.0).clicked() {
        app.ui.prefs.thumbnails = !thumbnails;
    }
}

/// "Parent size: [1024 ▾] × [1024 ▾]"
fn parent_size(app: &mut MaterialCraftApp, ui: &mut Ui) {
    let Ok(g) = app.session.graph() else { return };

    let [w, h] = g.size;
    let mut nw = w;
    let mut nh = h;
    let combo = |ui: &mut Ui, id: &str, v: &mut u32| {
        let r = egui::ComboBox::from_id_salt(id).width(64.0).selected_text(format!("{}", 1u32 << *v)).show_ui(ui, |ui| {
            for l in 4..=13u32 {
                ui.selectable_value(v, l, format!("{}", 1u32 << l));
            }
        });
        crate::widgets::name(&r.response, egui::Role::ComboBox, if id == "parent-w" { "Parent width" } else { "Parent height" });
    };

    combo(ui, "parent-h", &mut nh);
    ui.label("x");
    combo(ui, "parent-w", &mut nw);
    ui.label(egui::RichText::new(tr("Parent size:")).color(Tokens::get(ui.ctx()).text_dim));
    if (nw, nh) != (w, h) {
        app.run("graph.size", json!({"log2": [nw, nh]}));
    }
}

fn function_frame(app: &MaterialCraftApp, selection: &[u64]) -> serde_json::Value {
    let camera = super::GraphCamera { origin: app.graph.view.min, pan: app.ui.graph_camera.pan.into(), zoom: app.ui.graph_camera.zoom };
    let bounds = app.graph.rects.iter().filter(|(id, _)| selection.contains(&id.0)).fold(egui::Rect::NOTHING, |bounds, (_, rect)| bounds.union(*rect));
    if !bounds.is_finite() {
        return json!({});
    }

    let start = camera.to_graph(bounds.min);
    let size = bounds.size() / camera.zoom;
    json!({"rect": [start.x - 24.0, start.y - 44.0, size.x + 48.0, size.y + 68.0]})
}
