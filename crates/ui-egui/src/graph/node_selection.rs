use egui::Modifiers;

pub(crate) fn select<T: Copy + Eq>(selection: &mut Vec<T>, node: Option<T>, modifiers: Modifiers) {
    let toggle = modifiers.ctrl || modifiers.command;
    let additive = toggle || modifiers.shift;
    let Some(node) = node else {
        if !additive {
            selection.clear();
        }
        return;
    };

    if selection.contains(&node) {
        if toggle {
            selection.retain(|selected| *selected != node);
        }
    } else {
        if !additive {
            selection.clear();
        }
        selection.push(node);
    }
}

pub(crate) fn inspected<T: Copy>(selection: &[T], previous: Option<T>) -> Option<T> {
    selection.last().copied().or(previous)
}

pub(crate) fn select_box<T: Copy + Eq>(
    selection: &mut Vec<T>,
    nodes: impl IntoIterator<Item = (T, egui::Rect)>,
    start: egui::Pos2,
    end: egui::Pos2,
    modifiers: Modifiers,
) {
    let bounds = egui::Rect::from_two_pos(start, end);
    select(selection, None, modifiers);
    if bounds.width() <= 3.0 && bounds.height() <= 3.0 {
        return;
    }

    for (id, rect) in nodes {
        if bounds.intersects(rect) && !selection.contains(&id) {
            selection.push(id);
        }
    }
}

pub(crate) fn paint_box(painter: &egui::Painter, start: egui::Pos2, end: egui::Pos2, color: egui::Color32) {
    painter.rect(egui::Rect::from_two_pos(start, end), 0.0, color.gamma_multiply(0.12), egui::Stroke::new(1.0, color), egui::StrokeKind::Inside);
}
