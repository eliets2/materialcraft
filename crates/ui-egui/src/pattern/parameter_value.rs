use egui::Ui;
use materialcraft_engine::core::Value;

pub(super) fn show(ui: &mut Ui, name: &str, value: &mut Value) -> bool {
    let options: &[(i32, &str)] = match name {
        "patterntype" => &[
            (0, "None"),
            (1, "Input image"),
            (2, "Square"),
            (3, "Disc"),
            (4, "Paraboloid"),
            (5, "Bell"),
            (6, "Gaussian"),
            (7, "Thorn"),
            (8, "Pyramid"),
            (9, "Brick"),
            (10, "Gradation"),
            (11, "Waves"),
            (12, "Half bell"),
            (13, "Ridged bell"),
            (14, "Crescent"),
            (15, "Capsule"),
            (16, "Cone"),
            (17, "Hemisphere"),
            (18, "Offset gradation"),
        ],
        "blendingmode" => &[(0, "Add"), (1, "Blend"), (2, "Max")],
        "imagefiltering" => &[(0, "Nearest"), (1, "Bilinear")],
        _ => return crate::value_editor::show(ui, value),
    };

    let Some(mut index) = value.as_i32() else { return false };
    let previous = index;
    let label = options.iter().find(|(id, _)| *id == index).map_or("Custom", |(_, label)| *label);
    egui::ComboBox::from_id_salt(name).selected_text(label).show_ui(ui, |ui| {
        for (id, label) in options {
            ui.selectable_value(&mut index, *id, *label);
        }
    });
    if index == previous {
        return false;
    }
    *value = Value::Int(index);
    true
}
