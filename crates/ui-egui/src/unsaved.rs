//! Ask before unsaved work is thrown away
//!
//! Opening a package, closing all, making an empty package, dropping a file on the window and
//! closing the window or quitting all replace or end the open package. When it has unsaved
//! changes, those paths ask first (Save / Discard / Cancel). Only these UI paths ask: engine
//! commands run by the control channel, MCP, the CLI and scripts never open a dialog
//!
//! The decision is a pure state machine ([`request`], [`answer`], [`after_save`]) over the
//! serde [`Guarded`] action held in `Windows::unsaved`, so tests and the control channel
//! (`ui.inspect`) can see it. The glue below runs the held action once it is allowed

use egui::{Align2, Context, RichText, Window};
use serde_json::Value;

use crate::MaterialCraftApp;
use crate::i18n::tr;

mod choice;
mod guarded;
mod step;
pub use choice::Choice;
pub use guarded::Guarded;
pub use step::Step;

/// Does throwing this session away lose work?
pub fn needs_confirmation(session: &materialcraft_engine::Session) -> bool {
    session.modified
}

/// Ask to run `action`. Returns it when it can run now; otherwise holds it in `pending` for the
/// dialog (an action already waiting is kept) and returns `None`
pub fn request(pending: &mut Option<Guarded>, modified: bool, action: Guarded) -> Option<Guarded> {
    if pending.is_some() {
        return None;
    }
    if !modified {
        return Some(action);
    }
    *pending = Some(action);
    None
}

/// Apply the user's answer to the held action
pub fn answer(pending: &mut Option<Guarded>, choice: Choice) -> Step {
    match (pending.take(), choice) {
        (Some(a), Choice::Discard) => Step::Run(a),
        (Some(a), Choice::Save) => Step::Save(a),
        _ => Step::Stop,
    }
}

/// After a save attempt, the action may run only when the package really was saved
pub fn after_save(still_modified: bool, action: Guarded) -> Option<Guarded> {
    (!still_modified).then_some(action)
}

/// Should a window-close request be held back for the dialog?
pub fn holds_close(modified: bool, confirmed: bool) -> bool {
    modified && !confirmed
}

/// Run `action`, asking first when the package has unsaved changes
pub fn guard(app: &mut MaterialCraftApp, action: Guarded) {
    let modified = needs_confirmation(&app.session);
    if let Some(a) = request(&mut app.ui.windows.unsaved, modified, action) {
        perform(app, a);
    }
}

/// Run an allowed action
fn perform(app: &mut MaterialCraftApp, action: Guarded) {
    match action {
        Guarded::Menu { .. } => {
            app.run("file.newPackage", Value::Null);
        }
        #[cfg(not(target_arch = "wasm32"))]
        Guarded::OpenPath { path } => crate::windows::open_path_now(app, &path),
        #[cfg(target_arch = "wasm32")]
        Guarded::OpenPath { .. } => {}
        Guarded::LoadBytes { .. } => {
            if let Some((name, bytes)) = app.pending_load.take() {
                crate::web_files::load_now(app, &name, &bytes);
            }
        }
        Guarded::Quit => app.quit_confirmed = true,
    }
}

/// Per frame: hold back a window close while there is unsaved work, and finish a confirmed quit
pub fn handle_close(app: &mut MaterialCraftApp, ctx: &Context) {
    if ctx.input(|i| i.viewport().close_requested()) && holds_close(needs_confirmation(&app.session), app.quit_confirmed) {
        ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        guard(app, Guarded::Quit);
    }
    if app.quit_confirmed {
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }
}

/// The Save / Discard / Cancel dialog, while an action is held
pub fn show(app: &mut MaterialCraftApp, ctx: &Context) {
    if app.ui.windows.unsaved.is_none() {
        return;
    }
    let mut choice = None;
    Window::new(tr("Unsaved changes"))
        .id(egui::Id::unique("unsaved-changes"))
        .collapsible(false)
        .resizable(false)
        .pivot(Align2::CENTER_CENTER)
        .default_pos(ctx.content_rect().center())
        .show(ctx, |ui| {
            ui.label(RichText::new(format!("{} \"{}\"?", tr("Save changes to"), app.session.pkg.name)).strong());
            ui.label(tr("Your changes will be lost if you don't save them."));
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button(tr("Save")).clicked() {
                    choice = Some(Choice::Save);
                }
                if ui.button(tr("Discard")).clicked() {
                    choice = Some(Choice::Discard);
                }
                if ui.button(tr("Cancel")).clicked() {
                    choice = Some(Choice::Cancel);
                }
            });
        });
    if choice.is_none() && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        choice = Some(Choice::Cancel);
    }
    let Some(choice) = choice else { return };
    match answer(&mut app.ui.windows.unsaved, choice) {
        Step::Run(a) => perform(app, a),
        Step::Save(a) => {
            crate::windows::file_action(app, ctx, "file.save");
            match after_save(needs_confirmation(&app.session), a) {
                Some(a) => perform(app, a),
                None => app.pending_load = None,
            }
        }
        Step::Stop => app.pending_load = None,
    }
}

#[cfg(test)]
#[path = "unsaved.test.rs"]
mod tests;
