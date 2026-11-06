#[cfg(test)]
mod lib_tests {
    use crate::MaterialCraftApp;
    use crate::state::UiState;
    use materialcraft_engine::sample_session;
    use serde_json::json;

    #[test]
    fn run_reports_errors_as_toasts_and_keeps_the_session() {
        let mut app = MaterialCraftApp::headless(sample_session(), UiState::default());
        assert!(app.run("no.such.command", json!({})).is_none());
        assert!(app.toasts.iter().any(|t| t.error));
        assert!(app.run("graph.node.add", json!({"def": "levels"})).is_some());
    }
}

#[cfg(test)]
mod unsaved_state_tests {
    use crate::unsaved::{Choice, Guarded, Step, answer, guard};
    use crate::{MaterialCraftApp, state::UiState};

    fn app(modified: bool) -> MaterialCraftApp {
        let mut s = materialcraft_engine::sample_session();
        s.modified = modified;
        MaterialCraftApp::headless(s, UiState::default())
    }

    #[test]
    fn closing_all_over_unsaved_work_waits_for_an_answer() {
        let mut a = app(true);
        let before = a.session.pkg.name.clone();
        guard(&mut a, Guarded::Menu { id: "ui.closeAll".into() });
        assert!(a.ui.windows.unsaved.is_some());
        assert_eq!(a.session.pkg.name, before, "nothing was replaced yet");
        // The control channel can inspect the held action
        let v = serde_json::to_value(&a.ui).unwrap();
        assert_eq!(v["windows"]["unsaved"]["kind"], "menu");

        assert_eq!(answer(&mut a.ui.windows.unsaved, Choice::Cancel), Step::Stop);
        assert!(a.session.modified);
        assert_eq!(a.session.pkg.name, before);
    }

    #[test]
    fn closing_all_over_a_clean_package_just_happens() {
        let mut a = app(false);
        guard(&mut a, Guarded::Menu { id: "ui.closeAll".into() });
        assert!(a.ui.windows.unsaved.is_none());
        assert_eq!(a.session.pkg.graphs.first().map(|g| g.name.as_str()), Some("New_graph"), "the empty package replaced it");
    }

    #[test]
    fn a_pending_dialog_is_not_restored_from_saved_state() {
        let mut a = app(true);
        guard(&mut a, Guarded::Quit);
        let saved = serde_json::to_string(&a.ui).unwrap();
        let back: UiState = serde_json::from_str(&saved).unwrap();
        assert!(back.windows.unsaved.is_none());
    }
}
