use materialcraft_engine::{
    Session,
    core::{FunctionGraph, PatternGraph, PatternNodeKind, Value},
    execute, new_session,
};
use serde_json::json;

fn positions(session: &Session, kind: &str, owner: u64) -> Vec<[f32; 2]> {
    let graph = session.graph().unwrap();
    match kind {
        "function" => graph.function.as_ref().unwrap().nodes.iter().map(|node| node.pos).collect(),
        "pattern" => {
            let Value::PatternGraph(pattern) = &graph.node(materialcraft_engine::core::NodeId(owner)).unwrap().params["pattern"] else {
                panic!("missing pattern")
            };
            pattern.nodes.iter().map(|node| node.position).collect()
        }
        _ => graph.nodes.iter().map(|node| node.pos).collect(),
    }
}

#[test]
fn alignment_and_undo_target_only_the_active_graph_kind() {
    for kind in ["material", "function", "pattern"] {
        let mut session = new_session();
        let coordinates = [[0.0, 0.0], [32.0, 96.0], [256.0, 192.0]];
        let mut owner = 0;
        let ids: Vec<u64> = match kind {
            "function" => {
                execute(&mut session, "file.new", &json!({"kind": "function"})).unwrap();
                let mut graph = FunctionGraph::default();
                for position in coordinates {
                    graph.add_constant(Value::Float(1.0));
                    graph.nodes.last_mut().unwrap().pos = position;
                }
                let ids = graph.nodes.iter().map(|node| u64::from(node.id)).collect();
                execute(&mut session, "graph.function.set", &json!({"function": graph})).unwrap();
                ids
            }
            "pattern" => {
                owner = execute(&mut session, "graph.node.add", &json!({"def": "fx_map", "at": [700, 800]})).unwrap()["id"].as_u64().unwrap();
                let mut graph = PatternGraph::default();
                for position in coordinates {
                    graph.add(PatternNodeKind::Quadrant).unwrap();
                    graph.nodes.last_mut().unwrap().position = position;
                }
                let ids = graph.nodes.iter().map(|node| u64::from(node.id)).collect();
                execute(&mut session, "graph.pattern.set", &json!({"id": owner, "pattern": graph})).unwrap();
                ids
            }
            _ => coordinates
                .into_iter()
                .map(|position| execute(&mut session, "graph.node.add", &json!({"def": "uniform", "at": position})).unwrap()["id"].as_u64().unwrap())
                .collect(),
        };
        let original = session.pkg.clone();
        for (mode, expected) in [("left", vec![[0.0, 0.0], [0.0, 96.0], [0.0, 192.0]]), ("hdistribute", vec![[0.0, 0.0], [128.0, 96.0], [256.0, 192.0]])] {
            let mut parameters = json!({"ids": ids, "mode": mode});
            if kind == "pattern" {
                parameters["pattern"] = json!(owner);
            }
            execute(&mut session, "graph.align", &parameters).unwrap();
            assert_eq!(positions(&session, kind, owner), expected, "{kind}: {mode}");
            if kind == "pattern" {
                assert_eq!(session.graph().unwrap().nodes[0].pos, [700.0, 800.0]);
                assert_eq!(session.graph().unwrap().nodes.len(), 1);
            }
            session.undo().unwrap();
            assert_eq!(session.pkg, original);
            parameters["ids"] = json!([u64::MAX]);
            assert!(execute(&mut session, "graph.align", &parameters).is_err());
            assert_eq!(session.pkg, original);
            parameters["ids"] = json!(ids);
            parameters["mode"] = json!("invalid");
            assert!(execute(&mut session, "graph.align", &parameters).is_err());
            assert_eq!(session.pkg, original);
        }
    }
}

#[test]
fn malformed_embedded_targets_never_fall_back_to_parent_selection() {
    let mut session = new_session();
    for position in [[0, 0], [200, 100]] {
        execute(&mut session, "graph.node.add", &json!({"def": "fx_map", "at": position})).unwrap();
    }
    execute(&mut session, "edit.selectAll", &json!({})).unwrap();
    let original = session.pkg.clone();
    for parameters in [json!({"pattern": "invalid", "mode": "left"}), json!({"pattern": 1, "mode": "left"})] {
        assert!(execute(&mut session, "graph.align", &parameters).is_err());
        assert_eq!(session.pkg, original);
    }
}
