#[cfg(test)]
mod end_merge_tests {
    use crate::headless::{Filesystem, Headless};
    use serde_json::json;

    #[test]
    fn two_param_requests_are_two_undo_steps() {
        let mut h = Headless::new(Headless::default_session(), Filesystem::TrustedLocal);
        h.command_run("file.new", json!({"template": "empty"})).unwrap();
        let id = h.command_run("graph.node.add", json!({"def": "levels"})).unwrap()["id"].as_u64().unwrap();
        let sets = |h: &Headless| h.session.history().iter().filter(|l| *l == "Set parameter").count();
        h.command_run("graph.node.param", json!({"id": id, "param": "in_low", "value": [0.3, 0.3, 0.3, 0.3]})).unwrap();
        h.command_run("graph.node.param", json!({"id": id, "param": "in_low", "value": [0.4, 0.4, 0.4, 0.4]})).unwrap();
        assert_eq!(sets(&h), 2, "separate requests must not merge");
    }
}
