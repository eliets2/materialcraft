use materialcraft_engine::{execute, new_session};
use serde_json::json;

#[test]
fn rejected_2d_settings_leave_the_view_unchanged() {
    let mut s = new_session();
    let a = execute(&mut s, "graph.node.add", &json!({"def": "channels_shuffle"})).unwrap()["id"].as_u64().unwrap();
    let b = execute(&mut s, "graph.node.add", &json!({"def": "channels_shuffle"})).unwrap()["id"].as_u64().unwrap();
    execute(&mut s, "view.2d", &json!({"node": a, "port": 1, "channel": "g"})).unwrap();
    let before = s.view2d.clone();
    // A bad channel used to leave all the preceding fields changed despite returning an error.
    assert!(execute(&mut s, "view.2d", &json!({"node": b, "port": 2, "tiled": true, "histogram": true, "channel": "invalid"})).is_err());
    assert_eq!(s.view2d, before);
    assert!(execute(&mut s, "view.2d", &json!({"node": u64::MAX, "channel": "b", "tiled": true})).is_err());
    assert_eq!(s.view2d, before);
    assert_eq!(execute(&mut s, "session.inspect", &json!({})).unwrap()["view2d"], serde_json::to_value(&before).unwrap());
}

#[test]
fn rejected_3d_settings_leave_the_view_unchanged() {
    let mut s = new_session();
    execute(&mut s, "view.3d", &json!({"scene": "sphere", "environment": "sunset", "yaw": 70})).unwrap();
    let before = s.view3d.clone();
    // A valid scene must not be applied when another setting in the request is rejected.
    for params in [
        json!({"scene": "cube", "environment": "invalid", "yaw": 10}),
        json!({"scene": "invalid", "environment": "studio"}),
        json!({"scene": "custom", "environment": "studio"}),
        json!({"scene": "cube", "target": [1, 2]}),
    ] {
        assert!(execute(&mut s, "view.3d", &params).is_err(), "{params}");
        assert_eq!(s.view3d, before, "{params}");
    }
    assert_eq!(execute(&mut s, "session.inspect", &json!({})).unwrap()["view3d"], serde_json::to_value(&before).unwrap());
}

#[test]
fn valid_view_updates_still_apply_and_return_the_complete_settings() {
    let mut s = new_session();
    let node = execute(&mut s, "graph.node.add", &json!({"def": "uniform"})).unwrap()["id"].as_u64().unwrap();
    let result = execute(&mut s, "view.2d", &json!({"node": node, "channel": "r", "tiled": true, "histogram": true})).unwrap();
    assert_eq!(result["node"], node);
    assert_eq!(result["channel"], "r");
    assert_eq!(result["tiled"], true);
    assert_eq!(result["histogram"], true);
    assert_eq!(result, serde_json::to_value(&s.view2d).unwrap());
    let result = execute(&mut s, "view.3d", &json!({"scene": "cube", "environment": "night", "yaw": 370, "target": [1, 2, 3]})).unwrap();
    assert_eq!(result["scene"], "cube");
    assert_eq!(result["environment"], "night");
    assert_eq!(result["yaw"], 10.0);
    assert_eq!(result["target"], json!([1.0, 2.0, 3.0]));
    assert_eq!(result, serde_json::to_value(&s.view3d).unwrap());
}
