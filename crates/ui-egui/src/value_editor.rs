use egui::Ui;
use materialcraft_engine::core::Value;

/// Edit a function value within the available width
pub(crate) fn show(ui: &mut Ui, value: &mut Value) -> bool {
    ui.horizontal_wrapped(|ui| match value {
        Value::Float(value) => float(ui, value, "Value"),
        Value::Float2(values) => floats(ui, values, &["X", "Y"]),
        Value::Float3(values) => floats(ui, values, &["X", "Y", "Z"]),
        Value::Float4(values) => floats(ui, values, &["X", "Y", "Z", "W"]),
        Value::Color(values) => floats(ui, values, &["Red", "Green", "Blue", "Alpha"]),
        Value::Int(value) => integer(ui, value, "Value"),
        Value::Int2(values) => integers(ui, values),
        Value::Int3(values) => integers(ui, values),
        Value::Int4(values) => integers(ui, values),
        Value::Bool(value) => {
            let response = ui.checkbox(value, "");
            crate::widgets::name(&response, egui::Role::CheckBox, "Value");
            response.changed()
        }
        Value::Text(value) => {
            let response = ui.add(egui::TextEdit::singleline(value).desired_width(ui.available_width()));
            crate::widgets::name(&response, egui::Role::TextInput, "Value");
            response.changed()
        }
        _ => false,
    })
    .inner
}

fn float(ui: &mut Ui, value: &mut f32, label: &str) -> bool {
    let response = ui.add(egui::DragValue::new(value).speed(0.01).max_decimals(3));
    crate::widgets::name(&response, egui::Role::SpinButton, label);
    response.changed()
}

fn floats(ui: &mut Ui, values: &mut [f32], labels: &[&str]) -> bool {
    let mut changed = false;
    for (value, label) in values.iter_mut().zip(labels) {
        changed |= float(ui, value, label);
    }
    changed
}

fn integer(ui: &mut Ui, value: &mut i32, label: &str) -> bool {
    let response = ui.add(egui::DragValue::new(value).speed(0.1));
    crate::widgets::name(&response, egui::Role::SpinButton, label);
    response.changed()
}

fn integers(ui: &mut Ui, values: &mut [i32]) -> bool {
    let mut changed = false;
    for (value, label) in values.iter_mut().zip(["X", "Y", "Z", "W"]) {
        changed |= integer(ui, value, label);
    }
    changed
}
