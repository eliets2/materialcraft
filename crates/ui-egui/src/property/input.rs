use egui::Ui;
use materialcraft_engine::core::Value;
use serde_json::{Value as JsonValue, json};

/// The editor for one exposed input's value; returns the new value as JSON when it changed
pub(super) fn input_editor(ui: &mut Ui, exposed: &materialcraft_engine::core::ExposedParameter, current: &Value) -> Option<JsonValue> {
    match *current {
        Value::Float(mut f) => {
            let initial = f;
            crate::widgets::slider(ui, &exposed.label, &mut f, exposed.min.min(initial)..=exposed.max.max(initial), 3).changed().then(|| json!(f))
        }
        Value::Int(i) => {
            let mut f = i as f32;
            let initial = f;
            crate::widgets::slider(ui, &exposed.label, &mut f, exposed.min.min(initial)..=exposed.max.max(initial), 0).changed().then(|| json!(f.round()))
        }
        Value::Bool(mut b) => ui.checkbox(&mut b, &exposed.label).changed().then(|| json!(b)),
        Value::Enum(i) => {
            let mut out = None;
            crate::widgets::combo(ui, &exposed.label, exposed.options.get(i as usize).cloned().unwrap_or_default(), |ui| {
                for (k, o) in exposed.options.iter().enumerate() {
                    if ui.selectable_label(k as u32 == i, o).clicked() {
                        out = Some(json!(k));
                    }
                }
            });
            out
        }
        Value::Color(c) => {
            let mut rgb = [c[0], c[1], c[2]];
            let mut out = None;
            ui.horizontal(|ui| {
                ui.label(&exposed.label);
                if crate::widgets::color_swatch(ui, &format!("exp-{}", exposed.id), &mut rgb, 80.0) {
                    out = Some(json!([rgb[0], rgb[1], rgb[2], c[3]]));
                }
            });
            out
        }
        ref other => {
            ui.label(format!("{}: {}", exposed.label, other.type_name()));
            None
        }
    }
}
