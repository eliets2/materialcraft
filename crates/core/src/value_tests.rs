use crate::*;

#[test]
fn curve_identity_and_monotone() {
    let c = Curve::default();
    for i in 0..=10 {
        let x = i as f32 / 10.0;
        assert!((c.eval(x) - x).abs() < 1e-4);
    }
    let s = Curve { points: vec![CurvePoint::new(0.0, 0.0), CurvePoint::new(0.3, 0.1), CurvePoint::new(0.7, 0.9), CurvePoint::new(1.0, 1.0)] };
    let mut last = -1.0;
    for i in 0..=100 {
        let v = s.eval(i as f32 / 100.0);
        assert!(v >= last - 1e-5);
        last = v;
    }
}

#[test]
fn coerce_from_json() {
    assert_eq!(Value::Float(0.0).coerce_json(&serde_json::json!(0.5)), Some(Value::Float(0.5)));
    assert_eq!(Value::Color([0.0; 4]).coerce_json(&serde_json::json!("#ffffff")), Some(Value::Color([1.0, 1.0, 1.0, 1.0])));
    assert_eq!(Value::Float2([0.0; 2]).coerce_json(&serde_json::json!([1, 2])), Some(Value::Float2([1.0, 2.0])));
    assert_eq!(Value::Float(0.0).coerce_json(&serde_json::json!("x")), None);
    assert_eq!(Value::Float(0.0).coerce_json(&serde_json::json!(f64::MAX)), None);
    // A typed value of another type converts through its payload
    assert_eq!(Value::Int(0).coerce_json(&serde_json::json!({"t": "float", "v": 8.6})), Some(Value::Int(9)));
    assert_eq!(Value::Int(0).coerce_json(&serde_json::json!({"t": "text", "v": "x"})), None);
}

#[test]
fn gradient_interpolates() {
    let g = Gradient::default();
    assert_eq!(g.eval(0.5), [0.5, 0.5, 0.5, 1.0]);
    assert_eq!(Gradient { keys: vec![] }.eval(0.25)[0], 0.25);
}
