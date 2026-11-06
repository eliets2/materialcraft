use materialcraft_engine::{
    Session, atomic_sample,
    core::{PatternGraph, PatternNodeKind, Value},
    execute,
    formats::sbs::{read_sbs, write_sbs},
};
use serde_json::json;

#[test]
fn pattern_edits_are_atomic_undoable_and_preserve_native_layout() {
    let mut session = Session::new(atomic_sample().unwrap());
    let owner = session.graph().unwrap().nodes.iter().find(|node| node.def == "fx_map").unwrap().id;
    let original = session.pkg.clone();
    let Value::PatternGraph(pattern) = &session.graph().unwrap().node(owner).unwrap().params["pattern"] else { panic!("missing pattern") };
    let mut pattern: PatternGraph = (**pattern).clone();
    pattern.nodes[2].position = [256.0, 512.0];
    let dot = pattern.add(PatternNodeKind::Dot).unwrap();
    pattern.nodes[0].children[1] = Some(dot);
    pattern.nodes[3].children[0] = Some(pattern.nodes[2].id);
    execute(&mut session, "graph.pattern.set", &json!({"id": owner.0, "pattern": pattern})).unwrap();
    let edited = session.pkg.clone();
    let history = session.history();
    let root = pattern.root.unwrap();
    pattern.nodes[2].children[0] = Some(root);
    assert!(execute(&mut session, "graph.pattern.set", &json!({"id": owner.0, "pattern": pattern})).is_err());
    assert_eq!(session.pkg, edited);
    assert_eq!(session.history(), history);
    session.undo().unwrap();
    assert_eq!(session.pkg, original);
    session.redo().unwrap();
    assert_eq!(session.pkg, edited);

    let loaded = read_sbs(&write_sbs(&session.pkg).unwrap(), "pattern.sbs", &|_| None).unwrap();
    assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);
    let node = loaded.pkg.graphs[0].nodes.iter().find(|node| node.def == "fx_map").unwrap();
    let Value::PatternGraph(graph) = &node.params["pattern"] else { panic!("missing pattern") };
    assert_eq!(graph.nodes[2].position, [256.0, 512.0]);
    assert_eq!(graph.root, Some(graph.nodes[0].id));
    assert_eq!(graph.nodes[0].children, vec![Some(graph.nodes[1].id), Some(graph.nodes[3].id)]);
    assert_eq!(graph.nodes[1].children, vec![None, Some(graph.nodes[2].id)]);
    assert_eq!(graph.nodes[3].kind, PatternNodeKind::Dot);
    assert_eq!(graph.nodes[3].children, vec![Some(graph.nodes[2].id)]);
    assert!(matches!(graph.nodes[2].parameters["frameoffset"], Value::Function(_)));
}
