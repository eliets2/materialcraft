use materialcraft_engine::{View3d, execute, new_session};
use serde_json::json;

#[test]
fn normal_map_convention_is_inspectable_and_resets_to_opengl() {
    let mut s = new_session();
    assert!(!s.view3d.normal_y_flip);
    assert_eq!(materialcraft_engine::nodes::find("normal").unwrap().param("opengl").unwrap().default.as_bool(), Some(true));
    execute(&mut s, "view.3d", &json!({"normalYFlip": true})).unwrap();
    let inspection = execute(&mut s, "session.inspect", &json!({})).unwrap();
    assert_eq!(inspection["view3d"]["normalYFlip"], true);
    let saved = serde_json::to_value(&s.view3d).unwrap();
    assert!(serde_json::from_value::<View3d>(saved).unwrap().normal_y_flip);
    assert!(!serde_json::from_value::<View3d>(json!({"scene": "sphere"})).unwrap().normal_y_flip);
    execute(&mut s, "view.3d.reset", &json!({})).unwrap();
    assert!(!s.view3d.normal_y_flip);
}
