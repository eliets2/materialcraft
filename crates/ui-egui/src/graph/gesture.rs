use egui::{PointerButton, Ui};

pub(crate) fn pan_button(ui: &Ui) -> PointerButton {
    if ui.input(|input| input.pointer.button_pressed(PointerButton::Middle)) { PointerButton::Middle } else { PointerButton::Primary }
}

pub(crate) fn finished(ui: &Ui, owner: PointerButton) -> bool {
    ui.input(|input| input.pointer.button_released(owner))
}

pub(crate) fn cancelled(ui: &Ui) -> bool {
    ui.input(|input| !input.focused || input.key_pressed(egui::Key::Escape))
}
