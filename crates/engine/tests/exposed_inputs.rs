use materialcraft_engine::{core::NodeId, execute, new_session};
use serde_json::json;

#[test]
fn exposing_the_same_parameter_on_two_nodes_gives_each_its_own_input() {
    let mut s = new_session();
    let nodes: Vec<NodeId> = (0..3).map(|_| NodeId(execute(&mut s, "graph.node.add", &json!({"def": "blend"})).unwrap()["id"].as_u64().unwrap())).collect();

    // The properties panel's "Expose as graph input" sends no `id`
    let ids: Vec<String> = nodes
        .iter()
        .map(|node| {
            let reply = execute(&mut s, "graph.exposed.add", &json!({"label": "Opacity", "bind": {"node": node.0, "param": "opacity"}})).unwrap();
            reply["id"].as_str().unwrap().to_string()
        })
        .collect();
    assert_eq!(ids, ["opacity", "opacity_1", "opacity_2"]);

    let graph = s.graph().unwrap();
    assert_eq!(graph.exposed.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(), ids);
    for (node, id) in nodes.iter().zip(&ids) {
        assert_eq!(graph.node(*node).unwrap().bindings.get("opacity"), Some(id));
    }

    // An explicit id still has to be free, and an unbound input still needs one
    assert!(execute(&mut s, "graph.exposed.add", &json!({"id": "opacity_1", "bind": {"node": nodes[0].0, "param": "opacity"}})).is_err());
    assert!(execute(&mut s, "graph.exposed.add", &json!({"default": 0.5})).is_err());
}
