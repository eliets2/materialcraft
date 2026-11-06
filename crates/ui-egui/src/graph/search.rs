use super::{GraphCamera, NodeSearch};
use crate::{MaterialCraftApp, i18n::tr, theme::Tokens};
use egui::{Pos2, Ui};
use egui_phosphor::regular as icon;
use materialcraft_engine::core::NodeId;
use serde_json::json;

pub(super) fn open_search(app: &mut MaterialCraftApp, at: Pos2, connect: Option<(NodeId, String)>) {
    app.graph.search = Some(NodeSearch { at, text: String::new(), sel: 0, connect, focus: true });
}

/// Catalog entries matching `text` (label or id contains every word)
pub(super) fn matches(text: &str) -> Vec<&'static materialcraft_engine::nodes::NodeDefinition> {
    let words: Vec<String> = text.split_whitespace().map(str::to_ascii_lowercase).collect();
    let mut v: Vec<_> = materialcraft_engine::nodes::catalog()
        .iter()
        .filter(|d| d.id != "graph_instance")
        .filter(|d| {
            let hay = format!("{} {} {}", d.label.to_ascii_lowercase(), d.id, d.category.to_ascii_lowercase());
            words.iter().all(|w| hay.contains(w.as_str()))
        })
        .collect();
    if let Some(w) = words.first() {
        v.sort_by_key(|d| !d.label.to_ascii_lowercase().starts_with(w.as_str()));
    }
    v
}

pub(super) fn show(app: &mut MaterialCraftApp, ui: &mut Ui, cam: &GraphCamera) {
    let Some(s) = app.graph.search.as_mut() else { return };

    let tk = Tokens::get(ui.ctx());
    let screen = cam.to_screen(s.at);
    let mut close = false;
    let mut pick: Option<&'static str> = None;
    egui::Area::new(egui::Id::unique("node-search")).fixed_pos(screen).order(egui::Order::Foreground).show(ui.ctx(), |ui| {
        egui::Frame::popup(ui.style()).show(ui, |ui| {
            ui.set_width(260.0);
            let mut reveal_selection = s.focus;
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(icon::MAGNIFYING_GLASS).color(tk.text_dim));
                let r = ui.add(egui::TextEdit::singleline(&mut s.text).hint_text(tr("Search nodes...")).desired_width(220.0));
                crate::widgets::name(&r, egui::Role::SearchInput, "Search nodes");
                if s.focus {
                    r.request_focus();
                    s.focus = false;
                }
                if r.changed() {
                    s.sel = 0;
                    reveal_selection = true;
                }
            });
            let list = matches(&s.text);
            let (up, down, enter, esc) = ui.input(|i| {
                (i.key_pressed(egui::Key::ArrowUp), i.key_pressed(egui::Key::ArrowDown), i.key_pressed(egui::Key::Enter), i.key_pressed(egui::Key::Escape))
            });
            if down {
                s.sel = (s.sel + 1).min(list.len().saturating_sub(1));
            }
            if up {
                s.sel = s.sel.saturating_sub(1);
            }
            if esc {
                close = true;
            }
            if enter && let Some(d) = list.get(s.sel) {
                pick = Some(d.id);
            }
            egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                for (i, d) in list.iter().enumerate() {
                    let r = ui.add(
                        egui::Button::selectable(i == s.sel, format!("{}   ", d.label))
                            .min_size(egui::vec2(ui.available_width(), 0.0))
                            .right_text(egui::RichText::new(d.group()).size(10.0).color(tk.text_dim)),
                    );
                    if i == s.sel && (up || down || reveal_selection) {
                        r.scroll_to_me(None);
                    }
                    if r.clicked() {
                        pick = Some(d.id);
                    }
                }
                if list.is_empty() {
                    ui.label(egui::RichText::new(tr("No matching node")).color(tk.text_dim));
                }
            });
        });
    });
    let clicked_out = ui.input(|i| i.pointer.any_pressed())
        && ui.ctx().pointer_interact_pos().is_some_and(|p| !ui.ctx().layer_id_at(p).is_some_and(|l| l.id == egui::Id::unique("node-search")));
    if let Some(def) = pick {
        let s = app.graph.search.take();
        if let Some(s) = s {
            let mut p = json!({"def": def, "at": [s.at.x, s.at.y - 20.0]});
            if let Some((n, port)) = s.connect {
                p["connectFrom"] = json!({"node": n.0, "port": port});
            }
            app.run("graph.node.add", p);
        }
    } else if close || clicked_out {
        app.graph.search = None;
    }
}
