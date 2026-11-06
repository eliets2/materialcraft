#[cfg(test)]
mod unsaved_tests {
    use crate::unsaved::{Choice, Guarded, Step, answer};

    fn open() -> Guarded {
        Guarded::OpenPath { path: "a.sbs".into() }
    }

    #[test]
    fn cancel_drops_discard_runs_save_saves_first() {
        let mut pending = Some(open());
        assert_eq!(answer(&mut pending, Choice::Cancel), Step::Stop);
        assert_eq!(pending, None);

        let mut pending = Some(open());
        assert_eq!(answer(&mut pending, Choice::Discard), Step::Run(open()));
        assert_eq!(pending, None);

        let mut pending = Some(open());
        assert_eq!(answer(&mut pending, Choice::Save), Step::Save(open()));
        assert_eq!(pending, None);

        assert_eq!(answer(&mut None, Choice::Discard), Step::Stop, "nothing pending, nothing to run");
    }
}

#[test]
fn a_second_package_cannot_replace_the_payload_waiting_for_confirmation() {
    use crate::unsaved::{Choice, Guarded, Step};
    let mut app = crate::MaterialCraftApp::headless(materialcraft_engine::sample_session(), crate::UiState::default());
    app.session.modified = true;
    let bytes = |name: &str| materialcraft_engine::formats::sbs::write_sbs(&materialcraft_engine::core::Package::new(name)).unwrap();
    crate::web_files::load(&mut app, "first.sbs", bytes("First"), false);
    crate::web_files::load(&mut app, "second.sbs", bytes("Second"), false);
    assert_eq!(app.ui.windows.unsaved, Some(Guarded::LoadBytes { name: "first.sbs".into() }));
    let result = crate::control::handle(&mut app, &egui::Context::default(), "ui.set", &serde_json::json!({"prefs": {"thumbnails": false}}));
    assert_eq!(result["ok"], true);
    assert!(app.ui.windows.unsaved.is_some());
    if let Step::Run(action) = crate::unsaved::answer(&mut app.ui.windows.unsaved, Choice::Discard) {
        crate::unsaved::perform(&mut app, action);
    } else {
        panic!("missing held action");
    }
    assert_eq!(app.session.pkg.name, "First");
}

#[test]
fn a_pending_action_is_kept_even_if_the_package_was_saved_elsewhere() {
    let first = crate::unsaved::Guarded::Quit;
    let mut pending = Some(first.clone());
    assert!(crate::unsaved::request(&mut pending, false, crate::unsaved::Guarded::Menu { id: "ui.closeAll".into() }).is_none());
    assert_eq!(pending, Some(first));
}
