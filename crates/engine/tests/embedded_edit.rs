use materialcraft_engine::{
    FunctionGraphTarget, Session,
    core::{FunctionGraph, NodeId, PatternGraph, PatternNodeKind, Value},
    execute, new_session,
};
use serde_json::json;

fn function_session(kind: &str) -> (Session, FunctionGraphTarget) {
    let mut session = new_session();
    let target = match kind {
        "graph" => {
            execute(&mut session, "file.new", &json!({"kind": "function"})).unwrap();
            FunctionGraphTarget::Graph
        }
        "pattern" => {
            let owner = NodeId(execute(&mut session, "graph.node.add", &json!({"def": "fx_map"})).unwrap()["id"].as_u64().unwrap());
            let mut graph = PatternGraph::with_root(PatternNodeKind::Quadrant);
            let node = graph.nodes[0].id;
            graph.nodes[0].parameters.insert("patternrotation".into(), Value::Function(Box::default()));
            execute(&mut session, "graph.pattern.set", &json!({"id": owner.0, "pattern": graph})).unwrap();
            FunctionGraphTarget::Pattern { owner, node, parameter: "patternrotation".into() }
        }
        _ => {
            let node = NodeId(execute(&mut session, "graph.node.add", &json!({"def": "pixel_processor"})).unwrap()["id"].as_u64().unwrap());
            if kind == "dynamic" {
                session.graph_mut().unwrap().node_mut(node).unwrap().functions.insert("test".into(), FunctionGraph::default());
                FunctionGraphTarget::Dynamic { node, parameter: "test".into() }
            } else {
                FunctionGraphTarget::Parameter { node, parameter: "function".into() }
            }
        }
    };
    execute(&mut session, "graph.function.set", &json!({"target": target, "function": FunctionGraph::default()})).unwrap();
    session.reset_history();
    (session, target)
}

#[test]
fn creation_and_connections_use_the_same_rules_for_every_function_target() {
    for kind in ["graph", "parameter", "dynamic", "pattern"] {
        let (mut session, target) = function_session(kind);
        let mut ids = Vec::new();
        for operation in ["const_float1", "mul"] {
            let result = execute(
                &mut session,
                "graph.function.edit",
                &json!({"target": target, "edit": {"action": "add", "operation": operation, "at": [19, -19], "snap": true}}),
            )
            .unwrap();
            ids.push(result["id"].as_u64().unwrap() as u32);
        }
        let graph = target.current(&session).unwrap();
        assert_eq!(graph.node(ids[1]).unwrap().inputs, vec![None, None]);
        assert_eq!(graph.node(ids[1]).unwrap().pos, [32.0, -32.0]);
        assert_eq!(graph.output, Some(ids[0]));
        execute(&mut session, "graph.function.edit", &json!({"target": target, "edit": {"action": "connect", "from": ids[0], "to": ids[1], "input": 0}}))
            .unwrap();
        assert_eq!(target.current(&session).unwrap().node(ids[1]).unwrap().inputs[0], Some(ids[0]));
        let before = session.pkg.clone();
        for edit in [
            json!({"action": "connect", "from": ids[1], "to": ids[1], "input": 0}),
            json!({"action": "connect", "from": ids[0], "to": ids[1], "input": 1000000}),
            json!({"action": "add", "operation": "unknown", "at": [0, 0]}),
        ] {
            assert!(execute(&mut session, "graph.function.edit", &json!({"target": target, "edit": edit})).is_err());
            assert_eq!(session.pkg, before);
        }
    }
}

#[test]
fn value_gestures_merge_but_separate_edits_undo_independently() {
    for kind in ["graph", "parameter", "dynamic", "pattern"] {
        let (mut session, target) = function_session(kind);
        let added =
            execute(&mut session, "graph.function.edit", &json!({"target": target, "edit": {"action": "add", "operation": "const_float1", "at": [0, 0]}}))
                .unwrap();
        let id = added["id"].as_u64().unwrap();
        let initial = target.current(&session).unwrap().node(id as u32).unwrap().value.clone();
        session.reset_history();
        for value in [1.0, 2.0, 3.0] {
            execute(
                &mut session,
                "graph.function.edit",
                &json!({"target": target, "merge": true, "edit": {"action": "value", "id": id, "value": Value::Float(value)}}),
            )
            .unwrap();
        }
        assert_eq!(session.history().len(), 1, "{kind}");
        execute(&mut session, "edit.finish", &json!({})).unwrap();
        for value in [4.0, 5.0] {
            execute(
                &mut session,
                "graph.function.edit",
                &json!({"target": target, "merge": true, "edit": {"action": "value", "id": id, "value": Value::Float(value)}}),
            )
            .unwrap();
        }
        assert_eq!(session.history().len(), 2, "{kind}");
        session.undo();
        assert_eq!(target.current(&session).unwrap().node(id as u32).unwrap().value, Some(Value::Float(3.0)));
        session.undo();
        assert_eq!(target.current(&session).unwrap().node(id as u32).unwrap().value, initial);
        for _ in 0..2 {
            execute(&mut session, "graph.function.edit", &json!({"target": target, "edit": {"action": "add", "operation": "mul", "at": [0, 0]}})).unwrap();
        }
        session.undo();
        assert_eq!(target.current(&session).unwrap().nodes.len(), 2);
    }
}

#[test]
fn pattern_value_gestures_and_invalid_connections_are_atomic() {
    let mut session = new_session();
    let owner = execute(&mut session, "graph.node.add", &json!({"def": "fx_map"})).unwrap()["id"].as_u64().unwrap();
    let result = execute(&mut session, "graph.pattern.edit", &json!({"id": owner, "edit": {"action": "add", "kind": "paramset", "at": [0, 0]}})).unwrap();
    let id = result["id"].as_u64().unwrap();
    session.reset_history();
    for value in [1.0, 2.0, 3.0] {
        execute(
            &mut session,
            "graph.pattern.edit",
            &json!({"id": owner, "merge": true, "edit": {"action": "value", "id": id, "parameter": "patternrotation", "value": Value::Float(value)}}),
        )
        .unwrap();
    }
    assert_eq!(session.history().len(), 1);
    execute(&mut session, "edit.finish", &json!({})).unwrap();
    execute(
        &mut session,
        "graph.pattern.edit",
        &json!({"id": owner, "merge": true, "edit": {"action": "value", "id": id, "parameter": "patternrotation", "value": Value::Float(4.0)}}),
    )
    .unwrap();
    assert_eq!(session.history().len(), 2);
    let before = session.pkg.clone();
    assert!(execute(&mut session, "graph.pattern.edit", &json!({"id": owner, "edit": {"action": "connect", "parent": id, "slot": 0, "child": id}})).is_err());
    assert_eq!(session.pkg, before);
    assert!(execute(&mut session, "graph.function.edit", &json!({"target": {"kind": "pattern", "owner": owner, "node": id, "parameter": "absent"}, "edit": {"action": "add", "operation": "mul", "at": [0, 0]}})).is_err());
    assert_eq!(session.pkg, before);
}

#[test]
fn selection_commands_work_without_a_ui_for_every_function_target() {
    for kind in ["graph", "parameter", "dynamic", "pattern"] {
        let (mut session, target) = function_session(kind);
        for operation in ["const_float1", "passthrough"] {
            execute(&mut session, "graph.function.edit", &json!({"target": target, "edit": {"action": "add", "operation": operation, "at": [0, 0]}})).unwrap();
        }
        let target_parameters = if kind == "graph" { json!({}) } else { json!({"function": target}) };
        let parent_selection = session.selection.clone();
        session.reset_history();
        let selected = execute(&mut session, "edit.selectAll", &target_parameters).unwrap();
        assert_eq!(selected["selected"], 2, "{kind}");
        let mut parameters = target_parameters.clone();
        parameters["ids"] = selected["ids"].clone();
        let copied = execute(&mut session, "edit.copy", &parameters).unwrap();
        assert_eq!(copied["copied"], 2);
        assert!(session.history().is_empty());
        let duplicated = execute(&mut session, "edit.duplicate", &parameters).unwrap();
        assert_eq!(target.current(&session).unwrap().nodes.len(), 4);
        parameters["ids"] = duplicated["ids"].clone();
        execute(&mut session, "edit.delete", &parameters).unwrap();
        assert_eq!(target.current(&session).unwrap().nodes.len(), 2);
        parameters["ids"] = json!([]);
        parameters["clip"] = copied["clip"].clone();
        execute(&mut session, "edit.paste", &parameters).unwrap();
        assert_eq!(target.current(&session).unwrap().nodes.len(), 4);
        if kind != "graph" {
            assert_eq!(session.selection, parent_selection, "embedded commands changed the parent selection");
        }
    }
}

#[test]
fn rejected_function_duplicate_preserves_document_selection_clipboard_and_history() {
    let (mut session, target) = function_session("graph");
    let mut graph = FunctionGraph::default();
    for _ in 0..2049 {
        graph.add("passthrough", &[], None);
    }
    execute(&mut session, "graph.function.set", &json!({"function": graph})).unwrap();
    execute(&mut session, "edit.selectAll", &json!({})).unwrap();
    execute(&mut session, "edit.copy", &json!({})).unwrap();
    let before = session.pkg.clone();
    let selection = session.selection.clone();
    let clipboard = serde_json::to_value(&session.embedded_clipboard).unwrap();
    let history = session.history();
    assert!(execute(&mut session, "edit.duplicate", &json!({})).is_err());
    assert_eq!(session.pkg, before);
    assert_eq!(session.selection, selection);
    assert_eq!(serde_json::to_value(&session.embedded_clipboard).unwrap(), clipboard);
    assert_eq!(session.history(), history);
    assert_eq!(target.current(&session).unwrap().nodes.len(), 2049);
}

#[test]
fn graph_validation_is_shared_by_creation_legacy_setters_and_targeted_edits() {
    let mut cyclic = FunctionGraph::default();
    let id = cyclic.add("passthrough", &[], None);
    cyclic.nodes[0].inputs = vec![Some(id)];
    cyclic.output = Some(id);
    let mut session = new_session();
    let owner = execute(&mut session, "graph.node.add", &json!({"def": "pixel_processor"})).unwrap()["id"].as_u64().unwrap();
    let before = session.pkg.clone();
    for (command, parameters) in [
        ("graph.node.add", json!({"def": "pixel_processor", "params": {"function": Value::Function(Box::new(cyclic.clone()))}})),
        ("graph.node.param", json!({"id": owner, "param": "function", "value": Value::Function(Box::new(cyclic.clone()))})),
        ("graph.node.function", json!({"id": owner, "param": "color_mode", "function": cyclic})),
        ("graph.function.set", json!({"target": {"kind": "parameter", "node": owner, "parameter": "function"}, "function": cyclic})),
    ] {
        let error = execute(&mut session, command, &parameters).unwrap_err().to_string();
        assert!(error.contains("cycle"), "{command}: {error}");
        assert_eq!(session.pkg, before);
    }
    let owner = execute(&mut session, "graph.node.add", &json!({"def": "fx_map"})).unwrap()["id"].as_u64().unwrap();
    let mut graph = PatternGraph::with_root(PatternNodeKind::Quadrant);
    graph.nodes[0].parameters.insert("patternrotation".into(), Value::Function(Box::new(cyclic.clone())));
    let before = session.pkg.clone();
    for (command, parameters) in [
        ("graph.pattern.set", json!({"id": owner, "pattern": graph})),
        ("graph.node.param", json!({"id": owner, "param": "pattern", "value": Value::PatternGraph(Box::new(graph))})),
        (
            "graph.pattern.edit",
            json!({"id": owner, "edit": {"action": "value", "id": 1, "parameter": "patternrotation", "value": Value::Function(Box::new(cyclic))}}),
        ),
    ] {
        assert!(execute(&mut session, command, &parameters).unwrap_err().to_string().contains("cycle"), "{command}");
        assert_eq!(session.pkg, before);
    }
}

#[test]
fn pattern_selection_commands_and_cross_graph_clipboards_keep_ownership() {
    let mut session = new_session();
    let owner = execute(&mut session, "graph.node.add", &json!({"def": "fx_map"})).unwrap()["id"].as_u64().unwrap();
    let selected = execute(&mut session, "edit.selectAll", &json!({"pattern": owner})).unwrap();
    let parameters = json!({"pattern": owner, "ids": selected["ids"]});
    execute(&mut session, "edit.copy", &parameters).unwrap();
    let before = session.pkg.clone();
    assert!(execute(&mut session, "edit.deleteRelink", &parameters).is_err());
    assert!(execute(&mut session, "edit.delete", &json!({"pattern": owner, "ids": [1], "graph": 99})).is_err());
    assert_eq!(session.pkg, before);
    execute(&mut session, "edit.duplicate", &parameters).unwrap();
    assert_eq!(session.selection, vec![NodeId(owner)]);

    let (mut session, _) = function_session("graph");
    let mut graph = FunctionGraph::default();
    let source = graph.add_constant(Value::Float(1.0));
    let child = graph.add("passthrough", &[source], None);
    execute(&mut session, "graph.function.set", &json!({"function": graph})).unwrap();
    let copied = execute(&mut session, "edit.copy", &json!({"ids": [child]})).unwrap();
    execute(&mut session, "graph.duplicate", &json!({})).unwrap();
    let pasted = execute(&mut session, "edit.paste", &json!({"clip": copied["clip"]})).unwrap();
    let id = pasted["ids"][0].as_u64().unwrap() as u32;
    assert_eq!(session.graph().unwrap().function.as_ref().unwrap().node(id).unwrap().inputs, vec![None]);
}

#[test]
fn alignment_targets_embedded_functions_and_undo_restores_them() {
    for kind in ["parameter", "dynamic", "pattern"] {
        let (mut session, target) = function_session(kind);
        for at in [[10, 30], [50, 90]] {
            execute(&mut session, "graph.function.edit", &json!({"target": target, "edit": {"action": "add", "operation": "const_float1", "at": at}})).unwrap();
        }
        let before = session.pkg.clone();
        execute(&mut session, "graph.align", &json!({"function": target, "ids": [1, 2], "mode": "left"})).unwrap();
        let positions: Vec<_> = target.current(&session).unwrap().nodes.iter().map(|node| node.pos).collect();
        assert_eq!(positions, vec![[10.0, 30.0], [10.0, 90.0]]);
        session.undo().unwrap();
        assert_eq!(session.pkg, before);
    }
}

#[test]
fn replacing_a_package_discards_embedded_clipboard_provenance() {
    let (mut session, _) = function_session("graph");
    execute(&mut session, "graph.function.edit", &json!({"edit": {"action": "add", "operation": "const_float1", "at": [0, 0]}})).unwrap();
    execute(&mut session, "edit.selectAll", &json!({})).unwrap();
    execute(&mut session, "edit.copy", &json!({})).unwrap();
    assert!(session.embedded_clipboard.is_some());
    let package = session.pkg.clone();
    materialcraft_engine::file_cmds::replace_package(&mut session, package, None);
    assert!(session.embedded_clipboard.is_none());
}

#[test]
fn processor_node_availability_is_shared_and_preserves_existing_sampler_graphs() {
    for definition in ["pixel_processor", "value_processor"] {
        let mut session = new_session();
        let node = NodeId(execute(&mut session, "graph.node.add", &json!({"def": definition})).unwrap()["id"].as_u64().unwrap());
        let target = FunctionGraphTarget::Parameter { node, parameter: "function".into() };
        let operations: Vec<_> = target.available_operations(&session).map(|operation| operation.id).collect();
        let full_catalog = materialcraft_engine::nodes::function::FUNCTIONS;
        let sampling = definition == "pixel_processor";
        assert_eq!(operations.len(), full_catalog.len() - if sampling { 0 } else { 2 });
        assert!(operations.contains(&"add"));
        for sampler in ["samplelum", "samplecol"] {
            assert_eq!(operations.contains(&sampler), sampling);
            let original = session.pkg.clone();
            let history = session.history();
            let result =
                execute(&mut session, "graph.function.edit", &json!({"target": target, "edit": {"action": "add", "operation": sampler, "at": [0, 0]}}));
            assert_eq!(result.is_ok(), sampling);
            if !sampling {
                assert_eq!(session.pkg, original);
                assert_eq!(session.history(), history);
            }
        }
        let mut existing = FunctionGraph::default();
        existing.add("samplelum", &[], Some(Value::Int2([0, 1])));
        execute(&mut session, "graph.function.set", &json!({"target": target, "function": existing})).unwrap();
        assert_eq!(target.current(&session).unwrap().nodes[0].op, "samplelum");
    }
    for kind in ["graph", "pattern", "dynamic"] {
        let (session, target) = function_session(kind);
        assert_eq!(target.available_operations(&session).count(), materialcraft_engine::nodes::function::FUNCTIONS.len());
    }
}
