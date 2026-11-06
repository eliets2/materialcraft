use super::{NODE_W, node_rect};
use crate::MaterialCraftApp;
use egui::{Pos2, Rect, pos2, vec2};
use serde_json::json;

pub(super) fn view_center(app: &MaterialCraftApp) -> Pos2 {
    let c = &app.ui.graph_camera;
    let v = app.graph.view;
    pos2(c.pan[0] + v.width() * 0.5 / c.zoom, c.pan[1] + v.height() * 0.5 / c.zoom)
}

/// Add `def` right of the last selected node (wired from it when it fits), else mid-view
pub fn add_node_near_selection(app: &mut MaterialCraftApp, def: &str) {
    let sel = app.session.selection.last().copied();
    let has_inputs = materialcraft_engine::nodes::find(def).is_some_and(|d| !d.inputs.is_empty());
    let params = match sel {
        Some(id) if has_inputs && !app.session.output_ports(id).is_empty() => json!({"def": def, "connectFrom": {"node": id.0}}),
        _ => {
            let c = view_center(app);
            json!({"def": def, "at": [c.x - NODE_W * 0.5, c.y - 60.0]})
        }
    };

    app.run("graph.node.add", params);
}

/// Frame the graph (or the selection) in the view
pub(super) fn frame(app: &mut MaterialCraftApp, view: Rect, only_selection: bool) {
    let thumbs = app.ui.prefs.thumbnails;
    let Ok(g) = app.session.graph() else { return };

    let mut r = Rect::NOTHING;
    for n in &g.nodes {
        if only_selection && !app.session.selection.contains(&n.id) {
            continue;
        }

        let (inputs, outputs) = materialcraft_engine::eval::ports(&app.session.pkg, g, n);
        let visible = super::input_visibility::count(inputs.iter().map(|port| g.input_of(n.id, &port.id).is_some()), if n.def == "fx_map" { 2 } else { 1 });
        r = r.union(node_rect(n, visible, outputs.len(), thumbs));
    }
    for f in &g.frames {
        if !only_selection {
            r = r.union(Rect::from_min_size(pos2(f.rect[0], f.rect[1] - 32.0), vec2(f.rect[2], f.rect[3] + 32.0)));
        }
    }
    app.ui.graph_camera = super::GraphCamera::fit(view, r).state();
}

/// Import the resource and its graph node in one atomic undo entry
fn import_bitmap(app: &mut MaterialCraftApp, import: impl FnOnce(&mut materialcraft_engine::Session) -> materialcraft_engine::Result<String>) {
    let center = view_center(app);
    let snap = app.ui.prefs.snap_to_grid;
    let result = app.session.group("Import bitmap", |session| {
        let resource = import(session)?;
        materialcraft_engine::execute(
            session,
            "graph.node.add",
            &json!({
                "def": "bitmap", "at": [center.x - NODE_W * 0.5, center.y - 60.0],
                "snapToGrid": snap, "params": {"resource": resource}
            }),
        )?;
        Ok(resource)
    });
    match result {
        Ok(resource) => app.toast(format!("{} {resource}", crate::i18n::tr("Imported")), false),
        Err(error) => app.toast(error.to_string(), true),
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn import_bitmap_file(app: &mut MaterialCraftApp, path: &str) {
    import_bitmap(app, |session| {
        let result = materialcraft_engine::execute(session, "file.importBitmap", &json!({"path": path}))?;
        Ok(result["name"].as_str().unwrap_or_default().to_string())
    });
}

pub fn import_bitmap_bytes(app: &mut MaterialCraftApp, name: &str, bytes: Vec<u8>) {
    import_bitmap(app, |session| materialcraft_engine::file_cmds::add_resource(session, name, bytes, None));
}
