use materialcraft_engine::{Session, core::NodeId, execute, new_session};
use serde_json::{Value, json};

fn run(s: &mut Session, id: &str, p: Value) -> Value {
    execute(s, id, &p).unwrap()
}

fn node(s: &mut Session, def: &str) -> u64 {
    run(s, "graph.node.add", json!({"def": def}))["id"].as_u64().unwrap()
}

fn connect(s: &mut Session, from: u64, to: u64, port: &str) {
    run(s, "graph.connect", json!({"from": from, "to": to, "toPort": port}));
}

#[test]
fn relink_uses_the_first_declared_input_regardless_of_wire_creation_order() {
    for background_first in [true, false] {
        let mut s = new_session();
        s.preview_size = Some(32);
        let foreground = node(&mut s, "uniform");
        let background = node(&mut s, "uniform");
        run(&mut s, "graph.node.param", json!({"id": foreground, "param": "color", "value": [0.8, 0.4, 0.2, 1.0]}));
        run(&mut s, "graph.node.param", json!({"id": background, "param": "color", "value": [0.1, 0.2, 0.3, 1.0]}));
        let blend = node(&mut s, "blend");
        let output = node(&mut s, "output");
        let wires = [(foreground, "foreground"), (background, "background")];
        for index in if background_first { [1, 0] } else { [0, 1] } {
            connect(&mut s, wires[index].0, blend, wires[index].1);
        }
        connect(&mut s, blend, output, "input");
        run(&mut s, "graph.select", json!({"ids": [blend]}));
        let before = s.pkg.graphs.clone();
        let history = s.history().len();
        run(&mut s, "edit.deleteRelink", json!({}));
        assert_eq!(s.graph().unwrap().input_of(NodeId(output), "input").unwrap().from, NodeId(foreground));
        assert_eq!(s.history().len(), history + 1);
        assert!(s.selection.is_empty());
        let cooked = s.cook();
        assert_eq!(cooked.image(NodeId(output), 0).unwrap().get(0, 0, false), [0.8, 0.4, 0.2, 1.0]);
        run(&mut s, "edit.undo", json!({}));
        assert_eq!(s.pkg.graphs, before);
        run(&mut s, "edit.redo", json!({}));
        assert_eq!(s.graph().unwrap().input_of(NodeId(output), "input").unwrap().from, NodeId(foreground));
    }
}

#[test]
fn relink_does_not_substitute_a_secondary_input_when_the_first_is_unconnected() {
    let mut s = new_session();
    let background = node(&mut s, "uniform");
    let blend = node(&mut s, "blend");
    let output = node(&mut s, "output");
    connect(&mut s, background, blend, "background");
    connect(&mut s, blend, output, "input");
    run(&mut s, "graph.select", json!({"ids": [blend]}));
    run(&mut s, "edit.deleteRelink", json!({}));
    assert!(s.graph().unwrap().input_of(NodeId(output), "input").is_none());
}

#[test]
fn relink_preserves_multi_node_chain_connections_in_either_selection_order() {
    for reverse in [false, true] {
        let mut s = new_session();
        let source = node(&mut s, "uniform");
        let a = node(&mut s, "levels");
        let b = node(&mut s, "blur");
        let output = node(&mut s, "output");
        connect(&mut s, source, a, "input");
        connect(&mut s, a, b, "input");
        connect(&mut s, b, output, "input");
        let ids = if reverse { vec![b, a] } else { vec![a, b] };
        run(&mut s, "graph.select", json!({"ids": ids}));
        run(&mut s, "edit.deleteRelink", json!({}));
        assert_eq!(s.graph().unwrap().input_of(NodeId(output), "input").unwrap().from, NodeId(source));
    }
}
