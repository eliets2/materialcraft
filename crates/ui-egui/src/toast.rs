use crate::{MaterialCraftApp, theme};

pub struct Toast {
    pub text: String,
    pub error: bool,
    pub until: f64,
}

pub(crate) fn show(app: &mut MaterialCraftApp, ctx: &egui::Context) {
    if app.toasts.is_empty() {
        return;
    }
    ctx.request_repaint_after(std::time::Duration::from_millis(250));
    let tk = theme::Tokens::get(ctx);
    egui::Area::new(egui::Id::unique("toasts")).anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-16.0, -36.0)).show(ctx, |ui| {
        for t in &app.toasts {
            egui::Frame::NONE
                .fill(tk.panel)
                .stroke(egui::Stroke::new(1.0, if t.error { tk.danger } else { tk.accent }))
                .corner_radius(theme::CARD_RADIUS)
                .shadow(ui.visuals().popup_shadow)
                .inner_margin(egui::Margin::symmetric(10, 6))
                .show(ui, |ui| {
                    ui.set_max_width(380.0);
                    ui.label(egui::RichText::new(&t.text).color(tk.text_strong));
                });
            ui.add_space(4.0);
        }
    });
}
