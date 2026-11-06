use egui::{Button, Event, Key, Ui};

use crate::i18n::tr;

pub(crate) fn show(ui: &mut Ui, entries: impl IntoIterator<Item = (&'static str, &'static str, &'static str)>) -> Option<&'static str> {
    let filter_id = ui.make_persistent_id("add-node-filter");
    let frame = ui.ctx().cumulative_frame_nr();
    let previous = ui.ctx().data_mut(|data| data.get_temp::<(u64, Option<char>)>(filter_id));
    let mut filter = previous.filter(|(last_frame, _)| frame <= last_frame.saturating_add(1)).and_then(|(_, letter)| letter);

    ui.input(|input| {
        for event in &input.events {
            let Event::Key { key, pressed: true, modifiers, .. } = event else { continue };
            if modifiers.ctrl || modifiers.alt || modifiers.command {
                continue;
            }

            if *key == Key::Backspace {
                filter = None;
            } else if key.name().len() == 1 {
                filter = key.name().chars().next().map(|letter| letter.to_ascii_lowercase());
            }
        }
    });

    let mut entries: Vec<_> = entries.into_iter().collect();
    entries.sort_by_key(|(_, label, category)| (*category, *label));
    let mut selected = None;
    let enter = filter.is_some() && ui.input(|input| input.key_pressed(Key::Enter));
    ui.set_min_width(220.0);
    let height = (ui.ctx().content_rect().height() - 16.0).max(100.0);
    ui.set_max_height(height);
    egui::ScrollArea::vertical().max_height(height).show(ui, |ui| {
        let mut previous_category = None;
        for (id, label, category) in entries {
            if previous_category != Some(category) {
                if previous_category.is_some() {
                    ui.separator();
                }
                previous_category = Some(category);
            }

            let label = tr(label);
            let enabled = filter.is_none_or(|letter| label.chars().next().is_some_and(|first| first.eq_ignore_ascii_case(&letter)));
            if ui.add_enabled(enabled, Button::new(label)).clicked() || (enter && enabled && selected.is_none()) {
                selected = Some(id);
            }
        }
    });

    if selected.is_some() {
        ui.ctx().data_mut(|data| data.remove::<(u64, Option<char>)>(filter_id));
        ui.close();
    } else {
        ui.ctx().data_mut(|data| data.insert_temp(filter_id, (frame, filter)));
    }
    selected
}

#[cfg(test)]
mod tests {
    use egui_kittest::{
        Harness,
        kittest::{By, NodeT, Queryable},
    };

    #[test]
    fn letters_disable_other_initials_without_removing_entries() {
        let mut harness = Harness::builder().build_ui_state(
            |ui, selected: &mut Option<&'static str>| {
                if let Some(id) = super::show(
                    ui,
                    [("svg", "SVG", "Atomic nodes"), ("sharpen", "Sharpen", "Atomic nodes"), ("grayscale", "Grayscale conversion", "Atomic nodes")],
                ) {
                    *selected = Some(id);
                }
            },
            None,
        );
        harness.run_steps(3);
        harness.key_press(egui::Key::S);
        harness.run_steps(3);

        let button = |label| By::new().role(egui::accesskit::Role::Button).label(label);
        assert!(!harness.get(button("SVG")).accesskit_node().is_disabled());
        assert!(!harness.get(button("Sharpen")).accesskit_node().is_disabled());
        assert!(harness.get(button("Grayscale conversion")).accesskit_node().is_disabled());

        harness.key_press(egui::Key::G);
        harness.run_steps(3);
        assert!(harness.get(button("SVG")).accesskit_node().is_disabled());
        assert!(!harness.get(button("Grayscale conversion")).accesskit_node().is_disabled());

        harness.key_press(egui::Key::Backspace);
        harness.run_steps(3);
        for label in ["SVG", "Sharpen", "Grayscale conversion"] {
            assert!(!harness.get(button(label)).accesskit_node().is_disabled());
        }

        harness.key_press(egui::Key::S);
        harness.key_press(egui::Key::Enter);
        harness.step();
        assert_eq!(*harness.state(), Some("svg"));
    }
}
