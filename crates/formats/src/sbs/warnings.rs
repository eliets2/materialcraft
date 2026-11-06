use super::constant::MAX_WARNINGS;
use std::collections::HashMap;

#[derive(Default)]
pub(super) struct Warnings {
    pub(super) lines: Vec<(String, usize)>,
    pub(super) line_index: HashMap<String, usize>,
}

impl Warnings {
    pub(super) fn add(&mut self, message: impl Into<String>) {
        let message = message.into();
        if let Some(line) = self.line_index.get(&message).and_then(|&i| self.lines.get_mut(i)) {
            line.1 += 1;
            return;
        }

        self.line_index.insert(message.clone(), self.lines.len());
        self.lines.push((message, 1));
    }

    pub(super) fn finish(self) -> Vec<String> {
        let total = self.lines.len();
        let mut out: Vec<String> =
            self.lines.into_iter().take(MAX_WARNINGS).map(|(line, count)| if count > 1 { format!("{line} (×{count})") } else { line }).collect();
        if total > MAX_WARNINGS {
            out.push(format!("…and {} more warnings", total - MAX_WARNINGS));
        }
        out
    }
}
