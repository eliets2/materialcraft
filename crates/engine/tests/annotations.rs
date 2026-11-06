use materialcraft_engine::{
    AnnotationSelection,
    core::{NodeId, Pin},
    execute,
    formats::sbs::{read_sbs, write_sbs},
    new_session,
};
use serde_json::json;

#[test]
fn moving_a_frame_carries_every_contained_item_once_without_node_selection() {
    let mut session = new_session();
    let node = NodeId(execute(&mut session, "graph.node.add", &json!({"def": "uniform", "at": [35, 45]})).unwrap()["id"].as_u64().unwrap());
    let outside = NodeId(execute(&mut session, "graph.node.add", &json!({"def": "uniform", "at": [800, 800]})).unwrap()["id"].as_u64().unwrap());
    let nested = execute(&mut session, "graph.frame.add", &json!({"rect": [20, 20, 200, 160]})).unwrap()["id"].as_u64().unwrap();
    execute(&mut session, "graph.comment.add", &json!({"at": [50, 100], "text": "Carried comment"})).unwrap();
    session.graph_mut().unwrap().pins.push(Pin { id: 99, name: "Bookmark".into(), pos: [60.0, 60.0] });
    let frame = execute(&mut session, "graph.frame.add", &json!({"rect": [0, 0, 400, 300]})).unwrap()["id"].as_u64().unwrap();
    assert!(session.selection.is_empty());
    assert_eq!(session.annotation_selection, Some(AnnotationSelection::Frame(frame)));
    let original = session.pkg.clone();

    execute(&mut session, "graph.frame.move", &json!({"id": frame, "by": [50, 20], "snapToGrid": true})).unwrap();
    let graph = session.graph().unwrap();
    assert_eq!(graph.node(node).unwrap().pos, [99.0, 77.0]);
    assert_eq!(graph.node(outside).unwrap().pos, [800.0, 800.0]);
    assert_eq!(graph.comments[0].pos, [114.0, 132.0]);
    assert_eq!(graph.pins[0].pos, [124.0, 92.0]);
    assert_eq!(graph.frames.iter().find(|candidate| candidate.id == nested).unwrap().rect, [84.0, 52.0, 200.0, 160.0]);
    assert_eq!(graph.frames.iter().find(|candidate| candidate.id == frame).unwrap().rect, [64.0, 32.0, 400.0, 300.0]);

    session.undo().unwrap();
    assert_eq!(session.pkg, original);
    session.redo().unwrap();
    assert_eq!(session.graph().unwrap().node(node).unwrap().pos, [99.0, 77.0]);
}

#[test]
fn annotation_properties_and_selection_survive_edits_undo_and_native_save() {
    let mut session = new_session();
    let frame = execute(&mut session, "graph.frame.add", &json!({})).unwrap()["id"].as_u64().unwrap();
    execute(
        &mut session,
        "graph.frame.edit",
        &json!({"id": frame, "title": "Hidden title", "titleVisible": false, "description": "Description", "color": [0.1, 0.2, 0.3, 0.4]}),
    )
    .unwrap();
    let comment = execute(&mut session, "graph.comment.add", &json!({"text": "Plain text"})).unwrap()["id"].as_u64().unwrap();
    let loaded = read_sbs(&write_sbs(&session.pkg).unwrap(), "annotations.sbs", &|_| None).unwrap();
    assert!(loaded.warnings.is_empty());
    let graph = &loaded.pkg.graphs[0];
    assert_eq!(graph.frames[0].title, "Hidden title");
    assert!(!graph.frames[0].title_visible);
    assert_eq!(graph.frames[0].description, "Description");
    assert_eq!(graph.comments[0].text, "Plain text");
    for (actual, expected) in graph.frames[0].color.iter().zip([0.1, 0.2, 0.3, 0.4]) {
        assert!((actual - expected).abs() < 0.00001);
    }

    execute(&mut session, "edit.delete", &json!({})).unwrap();
    assert!(session.graph().unwrap().comments.is_empty());
    session.undo().unwrap();
    assert_eq!(session.annotation_selection, Some(AnnotationSelection::Comment(comment)));
    execute(&mut session, "graph.annotation.select", &json!({"kind": "frame", "id": frame})).unwrap();
    execute(&mut session, "edit.delete", &json!({})).unwrap();
    assert!(session.graph().unwrap().frames.is_empty());
    assert_eq!(session.graph().unwrap().comments.len(), 1);
}
