use materialcraft_automation::headless::{Filesystem, Headless, RenderOptions};
use materialcraft_engine::{
    execute,
    image::{Image, Kind},
    new_session,
    nodes::atomic::height_to_normal,
};
use materialcraft_render::{Lighting, Maps, Scene, Settings, render};
use serde_json::json;

#[test]
fn generated_normal_conventions_produce_the_same_surface_when_matched() {
    let height = Image::gray(32, 32, |x, y| ((x as f32 * 0.15).sin() + (y as f32 * 0.25).sin()) * 0.25 + 0.5);
    let gl = height_to_normal(&height, 8.0, false, false, false);
    let dx = height_to_normal(&height, 8.0, true, false, false);
    let normal_node = materialcraft_engine::nodes::find("normal").unwrap();
    assert_eq!(normal_node.param("opengl").unwrap().default.as_bool(), Some(true));
    let maps = |normal| Maps { normal: Some(normal), base_color: Some(Image::filled(4, 4, Kind::Color, [0.5, 0.5, 0.5, 1.0])), ..Maps::default() };
    let (scene, light) = (Scene::builtin("sphere"), Lighting::new("studio"));
    let settings = Settings::default();
    assert!(!settings.normal_y_flip);
    let expected = render(&scene, &maps(gl), &light, &settings, 64, 48);
    let flipped = maps(dx);
    let matched = render(&scene, &flipped, &light, &Settings { normal_y_flip: true, ..settings }, 64, 48);
    assert!(expected.iter().zip(&matched).all(|(a, b)| a.abs_diff(*b) <= 1));
    let wrong = render(&scene, &flipped, &light, &settings, 64, 48);
    assert!(expected.iter().zip(&wrong).filter(|(a, b)| a.abs_diff(**b) > 2).count() > 20);
}

#[test]
fn headless_render_honors_the_normal_y_command() {
    let mut s = new_session();
    s.preview_size = Some(64);
    let node = execute(&mut s, "graph.node.add", &json!({"def": "uniform", "params": {"color": [0.5, 0.75, 1.0, 1.0]}})).unwrap()["id"].as_u64().unwrap();
    execute(&mut s, "graph.node.add", &json!({"def": "output", "connectFrom": {"node": node}, "params": {"usage": 1}})).unwrap();
    let mut h = Headless::new(s, Filesystem::Denied);
    let options = RenderOptions { width: Some(64), height: Some(48), ..RenderOptions::default() };
    let gl = h.render_png(&options).unwrap().png;
    execute(&mut h.session, "view.3d", &json!({"normalYFlip": true})).unwrap();
    let wrong = h.render_png(&options).unwrap().png;
    assert_ne!(gl, wrong);
    execute(&mut h.session, "graph.node.param", &json!({"id": node, "param": "color", "value": [0.5, 0.25, 1.0, 1.0]})).unwrap();
    assert_eq!(gl, h.render_png(&options).unwrap().png);
    execute(&mut h.session, "view.3d.reset", &json!({})).unwrap();
    assert_eq!(wrong, h.render_png(&options).unwrap().png);
}
