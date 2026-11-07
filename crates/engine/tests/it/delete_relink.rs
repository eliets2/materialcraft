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

#[test]
fn relink_chain_deleted_in_reverse_order_keeps_declared_source() {
    let mut s = new_session();
    s.preview_size = Some(32);
    let chain_source = node(&mut s, "uniform");
    run(&mut s, "graph.node.param", json!({"id": chain_source, "param": "color", "value": [0.8, 0.4, 0.2, 1.0]}));
    let multi_input = node(&mut s, "blend");
    run(&mut s, "graph.node.value_input", json!({"id": multi_input, "name": "brightness", "type": "Float"}));
    let value_input = node(&mut s, "input_value");
    let downstream = node(&mut s, "levels");
    let output = node(&mut s, "output");
    connect(&mut s, chain_source, multi_input, "foreground");
    connect(&mut s, value_input, multi_input, "brightness");
    connect(&mut s, multi_input, downstream, "input");
    connect(&mut s, downstream, output, "input");
    run(&mut s, "graph.select", json!({"ids": [downstream, multi_input]}));
    run(&mut s, "edit.deleteRelink", json!({}));
    assert_eq!(s.graph().unwrap().input_of(NodeId(output), "input").unwrap().from, NodeId(chain_source));
    let cooked = s.cook();
    assert_eq!(cooked.image(NodeId(output), 0).unwrap().get(0, 0, false), [0.8, 0.4, 0.2, 1.0]);
}

#[test]
fn relink_does_not_substitute_secondary_input_when_first_unconnected_with_value_input() {
    let mut s = new_session();
    let background = node(&mut s, "uniform");
    let blend = node(&mut s, "blend");
    run(&mut s, "graph.node.value_input", json!({"id": blend, "name": "brightness", "type": "Float"}));
    let output = node(&mut s, "output");
    connect(&mut s, background, blend, "background");
    connect(&mut s, blend, output, "input");
    run(&mut s, "graph.select", json!({"ids": [blend]}));
    run(&mut s, "edit.deleteRelink", json!({}));
    assert!(s.graph().unwrap().input_of(NodeId(output), "input").is_none());
}

#[test]
fn relink_cycle_between_deleted_nodes_does_not_relink() {
    let mut s = new_session();
    let a = node(&mut s, "levels");
    let b = node(&mut s, "blur");
    let output = node(&mut s, "output");
    connect(&mut s, a, b, "input");
    connect(&mut s, b, output, "input");
    // Upstream graph.connect now rejects cycles with Core(Cycle), so the
    // closing edge of the cycle is inserted directly to set up the
    // delete-relink scenario this test covers.
    s.graph_mut().unwrap().connections.push(materialcraft_engine::core::Connection {
        from: NodeId(b),
        from_port: "output".into(),
        to: NodeId(a),
        to_port: "input".into(),
    });
    run(&mut s, "graph.select", json!({"ids": [a, b]}));
    run(&mut s, "edit.deleteRelink", json!({}));
    assert!(s.graph().unwrap().input_of(NodeId(output), "input").is_none());
}
