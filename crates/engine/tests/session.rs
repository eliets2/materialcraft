use materialcraft_engine::{Session, command_specs, execute, sample_session};
use serde_json::{Value, json};

fn run(s: &mut Session, id: &str, p: Value) -> Value {
    execute(s, id, &p).unwrap_or_else(|e| panic!("{id}: {e}"))
}

#[test]
fn every_template_cooks_cleanly() {
    let mut s = sample_session();
    s.preview_size = Some(128);
    for (id, _) in materialcraft_engine::samples::TEMPLATES {
        run(&mut s, "file.new", json!({"template": id, "size": 256}));
        let c = s.cook().clone();
        let g = s.graph().unwrap();
        for (nid, r) in &c.nodes {
            assert!(r.error.is_none(), "{id}: node {} ({}) failed: {:?}", nid.0, g.node(*nid).unwrap().def, r.error);
        }
        if *id != "empty" {
            assert!(!s.material_maps().is_empty(), "{id} has no outputs");
        }
    }
}

#[test]
fn build_a_graph_by_commands() {
    let mut s = sample_session();
    run(&mut s, "file.new", json!({"template": "empty", "name": "Mine"}));
    let noise = run(&mut s, "graph.node.add", json!({"def": "blur", "at": [0, 0], "params": {"intensity": 6}}))["id"].as_u64().unwrap();
    let lv = run(&mut s, "graph.node.add", json!({"def": "levels", "connectFrom": {"node": noise}}))["id"].as_u64().unwrap();
    let out = run(&mut s, "graph.node.add", json!({"def": "output", "at": [400, 0], "params": {"usage": 4}}))["id"].as_u64().unwrap();
    run(&mut s, "graph.connect", json!({"from": lv, "to": out}));
    run(&mut s, "graph.node.param", json!({"id": lv, "param": "in_low", "value": 0.3}));
    run(&mut s, "graph.node.param", json!({"id": lv, "param": "in_low", "value": 0.4}));
    // A slider drag is one undo step
    assert_eq!(s.history().iter().filter(|h| *h == "Set parameter").count(), 1);
    s.cook();
    let i = run(&mut s, "session.inspect", json!({}));
    let nodes = i["graph"]["nodes"].as_array().unwrap();
    assert_eq!(nodes.len(), 3);
    assert!(nodes.iter().all(|n| n["cooked"]["error"].is_null()));
    assert_eq!(i["graph"]["connections"].as_array().unwrap().len(), 2);
    // Bad input is an error, not a panic, and leaves the graph unchanged
    assert!(execute(&mut s, "graph.connect", &json!({"from": out, "to": noise})).is_err());
    assert!(execute(&mut s, "graph.node.param", &json!({"id": lv, "param": "nope", "value": 1})).is_err());
    assert!(execute(&mut s, "graph.node.add", &json!({"def": "nope"})).is_err());
    // Undo and redo
    run(&mut s, "edit.undo", json!({}));
    run(&mut s, "edit.redo", json!({}));
    // Copy/paste and duplicate
    run(&mut s, "graph.select", json!({"ids": [noise, lv]}));
    run(&mut s, "edit.copy", json!({}));
    run(&mut s, "edit.paste", json!({"at": [0, 300]}));
    assert_eq!(s.graph().unwrap().nodes.len(), 5);
    assert_eq!(s.graph().unwrap().connections.len(), 3);
    run(&mut s, "edit.duplicate", json!({}));
    assert_eq!(s.graph().unwrap().nodes.len(), 7);
    // Create a graph from a selection: the nodes move into a sub-graph instance
    run(&mut s, "graph.select", json!({"ids": [lv]}));
    let r = run(&mut s, "graph.createFromSelection", json!({"name": "Leveler"}));
    assert_eq!(r["graph"], "Leveler");
    s.cook();
    assert!(s.cooked.nodes.values().all(|r| r.error.is_none()));
    // Exposed inputs drive bound parameters
    run(&mut s, "graph.open", json!({"graph": "Mine"}));
    run(&mut s, "graph.exposed.add", json!({"id": "detail", "bind": {"node": noise, "param": "intensity"}}));
    run(&mut s, "graph.exposed.set", json!({"id": "detail", "value": 12}));
    s.cook();
    assert!(s.cooked.nodes.values().all(|r| r.error.is_none()));
}

#[test]
fn package_save_and_open_round_trip() {
    let mut s = Session::new(materialcraft_engine::atomic_sample().unwrap());
    let dir = std::env::temp_dir().join(format!("mc-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("atomics.sbs");
    run(&mut s, "file.save", json!({"path": path.to_str().unwrap()}));
    let before = s.pkg.clone();
    run(&mut s, "file.newPackage", json!({}));
    run(&mut s, "file.open", json!({"path": path.to_str().unwrap()}));
    assert_eq!(s.pkg.graphs[0].nodes.len(), before.graphs[0].nodes.len());
    let out = dir.join("tex");
    let r = run(&mut s, "file.export", json!({"dir": out.to_str().unwrap(), "format": "png", "depth": 8}));
    assert_eq!(r["files"].as_array().unwrap().len(), 1);
    let small = dir.join("small");
    run(&mut s, "file.export", json!({"dir": small.to_str().unwrap(), "format": "png", "depth": 8, "size": 64}));
    let file = std::fs::read_dir(&small).unwrap().next().unwrap().unwrap().path();
    let img = materialcraft_formats::decode(&std::fs::read(file).unwrap(), false).unwrap();
    assert_eq!((img.w, img.h), (64, 64));
    let _ = std::fs::remove_dir_all(&dir);
}

/// `file.export` `size` is the larger side in pixels; 0 means the graph's size and the proportions are kept
#[test]
fn export_size_keeps_proportions_and_clamps() {
    let mut s = sample_session();
    s.pkg.graphs[0].size = [7, 6];
    let dims = |s: &Session, size: Value| {
        let (gi, ext, pattern, depth, size) = materialcraft_engine::file_cmds::export_params(s, &json!({"size": size})).unwrap();
        let files = materialcraft_engine::file_cmds::export_outputs(s, gi, &ext, &pattern, depth, size).unwrap();
        files.iter().map(|f| materialcraft_formats::decode(&f.bytes, false).unwrap()).map(|i| (i.w, i.h)).collect::<Vec<_>>()
    };

    for size in [json!(0), json!(null)] {
        assert!(dims(&s, size).iter().all(|d| *d == (128, 64)));
    }
    assert!(dims(&s, json!(32)).iter().all(|d| *d == (32, 16)));
    let parsed = |size: Value| materialcraft_engine::file_cmds::export_params(&s, &json!({"size": size})).unwrap().4;
    assert_eq!(parsed(json!(0)), None);
    assert_eq!(parsed(json!(u64::MAX)), Some(u32::MAX));
    s.pkg.graphs[0].size = [u32::MAX, u32::MAX];
    assert!(dims(&s, json!(32)).iter().all(|d| *d == (32, 32)));
}

/// A scratch folder for one test, removed when dropped
struct TempDir(std::path::PathBuf);

impl TempDir {
    fn new(name: &str) -> TempDir {
        let dir = std::env::temp_dir().join(format!("mc-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        TempDir(dir)
    }

    fn file(&self, name: &str) -> String {
        self.0.join(name).to_str().unwrap().to_string()
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn native_sbs_is_the_default_save_format() {
    let directory = TempDir::new("sbs-default");
    let mut session = Session::new(materialcraft_engine::atomic_sample().unwrap());
    let saved = run(&mut session, "file.save", json!({"path": directory.file("atomics")}))["path"].as_str().unwrap().to_string();
    assert!(saved.ends_with("atomics.sbs"));
    assert!(std::fs::read(&saved).unwrap().starts_with(b"<?xml"));

    run(&mut session, "file.newPackage", json!({}));
    let opened = run(&mut session, "file.open", json!({"path": saved}));
    assert_eq!(opened["warnings"], json!([]));
    assert_eq!(run(&mut session, "file.save", json!({}))["path"], json!(saved));
}

/// Saving an SBS file another program wrote must not overwrite it: MaterialCraft can't write back what it couldn't read
#[test]
fn saving_an_sbs_file_from_another_program_needs_a_new_name() {
    let dir = TempDir::new("sbs-theirs");
    let theirs = dir.file("theirs.sbs");
    let xml = r#"<?xml version="1.0"?><package><content><graph><identifier v="G"/><compNodes><compNode><uid v="1"/><compImplementation><compFilter><filter v="curve"/><parameters><parameter><name v="curveluminance"/><paramValue><constantValueString v="POINTS"/></paramValue></parameter></parameters></compFilter></compImplementation></compNode></compNodes></graph><function><identifier v="their_function"/></function></content></package>"#;
    std::fs::write(&theirs, xml).unwrap();
    let mut s = sample_session();

    let opened = run(&mut s, "file.open", json!({"path": theirs}));
    assert_eq!(opened["path"], json!(null));
    assert!(execute(&mut s, "file.save", &json!({})).is_err());
    assert_eq!(std::fs::read_to_string(&theirs).unwrap(), xml, "the original is untouched");

    // Saved under a new name it becomes a MaterialCraft file, which saves in place from then on
    let mine = run(&mut s, "file.save", json!({"path": dir.file("mine.sbs")}))["path"].as_str().unwrap().to_string();
    run(&mut s, "file.open", json!({"path": mine}));
    assert_eq!(run(&mut s, "file.save", json!({}))["path"], json!(mine));
}

#[test]
fn a_broken_sbs_file_reports_an_sbs_error() {
    let dir = TempDir::new("sbs-broken");
    let broken = dir.file("broken.sbs");
    std::fs::write(&broken, b"\xff\xfe<\x00p\x00").unwrap();
    let mut s = sample_session();

    let error = execute(&mut s, "file.open", &json!({"path": broken})).unwrap_err().to_string();
    assert!(error.contains("SBS"), "{error}");
}

#[test]
fn an_sbs_file_with_missing_nodes_opens_softly() {
    let directory = TempDir::new("sbs-missing");
    let path = directory.file("missing.sbs");
    let xml = r#"<package><content><graph><identifier v="G"/><compNodes><compNode><uid v="1"/><compImplementation><compFilter><filter v="unknown_primitive"/></compFilter></compImplementation></compNode></compNodes></graph></content></package>"#;
    std::fs::write(&path, xml).unwrap();
    let mut s = sample_session();
    s.preview_size = Some(32);
    let opened = run(&mut s, "file.open", json!({"path": path}));
    let warnings = opened["warnings"].as_array().unwrap();
    assert!(warnings.iter().any(|warning| warning.as_str().unwrap().contains("missing node")));
    let inspect = run(&mut s, "session.inspect", json!({}));
    assert_eq!(inspect["package"]["warnings"].as_array().map(Vec::len), Some(warnings.len()));

    s.cook();
    let placeholder = s.graph().unwrap().nodes[0].id;
    assert!(s.cooked.nodes[&placeholder].error.is_some());
    let (_, placeholders) = s.cook_failures();
    assert_eq!(placeholders, 1);

    // Commands on a placeholder are errors or no-ops, never panics
    s.selection = vec![placeholder];
    for (id, params) in [
        ("graph.node.param", json!({"id": placeholder.0, "param": "intensity", "value": 2})),
        ("edit.copy", json!({})),
        ("edit.paste", json!({})),
        ("edit.duplicate", json!({})),
        ("graph.node.delete", json!({"ids": [placeholder.0]})),
        ("edit.undo", json!({})),
    ] {
        let _ = execute(&mut s, id, &params);
    }
    for spec in command_specs().iter().filter(|spec| !spec.id.starts_with("file.")) {
        let _ = execute(&mut s, spec.id, &json!({}));
    }
    s.cook();

    run(&mut s, "file.newPackage", json!({}));
    assert!(s.load_warnings.is_empty(), "a new package clears the warnings");
}

/// Every command, run with empty and hostile params, returns Ok or Err but never panics
#[test]
fn panic_hunt() {
    let hostile = [
        json!({}),
        json!(null),
        json!({"id": u64::MAX, "ids": [u64::MAX, 0], "node": -1, "value": "x", "param": "", "def": "", "graph": 999}),
        json!({"width": 0, "height": u64::MAX, "size": -5, "seed": f64::MAX, "at": [f64::MAX, -1e30], "by": ["a"], "rect": [1, 2]}),
        json!({"value": [1e38, -1e38, 0, 0], "param": "opacity", "def": "blend", "mode": "zzz", "format": "gif", "dir": "", "path": ""}),
        json!({"from": 0, "to": 0, "fromPort": "", "toPort": "", "clip": {"nodes": "x"}, "template": "../../x", "scene": "\u{0}"}),
    ];
    for spec in command_specs() {
        // Skip file writes to the working directory and opening paths
        if matches!(
            spec.id,
            "file.save" | "file.saveAs" | "file.export" | "file.open" | "file.importBitmap" | "file.exportMaterial" | "file.sendToTextureCraft"
        ) {
            continue;
        }
        for p in &hostile {
            let mut s = sample_session();
            s.preview_size = Some(64);
            let first = s.graph().unwrap().nodes[0].id;
            s.selection = vec![first];
            let _ = execute(&mut s, spec.id, p);
            s.cook();
        }
    }
}

#[test]
fn dynamic_parameters_follow_graph_inputs() {
    let mut s = sample_session();
    run(&mut s, "file.new", json!({"template": "empty"}));
    let u = run(&mut s, "graph.node.add", json!({"def": "uniform", "params": {"color_mode": false}}))["id"].as_u64().unwrap();
    run(&mut s, "graph.exposed.add", json!({"id": "level", "default": {"t": "float", "v": 0.25}}));
    let f = json!({"nodes": [{"id": 1, "op": "get_float1", "value": {"t": "text", "v": "level"}}], "output": 1});
    run(&mut s, "graph.node.function", json!({"id": u, "param": "color", "function": f}));
    s.cook();
    let mean = |s: &Session| s.cooked.image(materialcraft_engine::core::NodeId(u), 0).unwrap().mean()[0];
    assert!((mean(&s) - 0.25).abs() < 1e-4, "{}", mean(&s));
    run(&mut s, "graph.exposed.set", json!({"id": "level", "value": 0.75}));
    s.cook();
    assert!((mean(&s) - 0.75).abs() < 1e-4);
    // Text parameters can't take functions; removing restores the plain value
    assert!(execute(&mut s, "graph.node.function", &json!({"id": u, "param": "nope"})).is_err());
    run(&mut s, "graph.node.function", json!({"id": u, "param": "color", "remove": true}));
    s.cook();
    assert!((mean(&s) - 0.5).abs() < 0.05);
}

#[test]
fn exposed_input_change_recooks_only_readers() {
    let mut s = sample_session();
    run(&mut s, "file.new", json!({"template": "atomics", "size": 64}));
    s.preview_size = None;
    let u = run(&mut s, "graph.node.add", json!({"def": "uniform", "params": {"color_mode": false}}))["id"].as_u64().unwrap();
    run(&mut s, "graph.exposed.add", json!({"id": "level", "default": {"t": "float", "v": 0.25}}));
    let f = json!({"nodes": [{"id": 1, "op": "get_float1", "value": {"t": "text", "v": "level"}}], "output": 1});
    run(&mut s, "graph.node.function", json!({"id": u, "param": "color", "function": f}));
    s.cook();
    run(&mut s, "graph.exposed.set", json!({"id": "level", "value": 0.75}));
    s.cook();
    let recooked: Vec<u64> = s.cooked.nodes.iter().filter(|(_, r)| !r.cached).map(|(id, _)| id.0).collect();
    assert_eq!(recooked, [u], "only the node reading `level` re-cooks");
}

#[test]
fn presets_store_and_restore_inputs() {
    let mut s = sample_session();
    run(&mut s, "file.new", json!({"template": "empty"}));
    run(&mut s, "graph.exposed.add", json!({"id": "a", "default": {"t": "float", "v": 0.1}}));
    run(&mut s, "graph.preset.save", json!({"name": "Low"}));
    run(&mut s, "graph.exposed.set", json!({"id": "a", "value": 0.9}));
    run(&mut s, "graph.preset.apply", json!({"name": "Low"}));
    assert_eq!(s.graph().unwrap().input_value("a"), Some(&materialcraft_engine::core::Value::Float(0.1)));
    assert!(execute(&mut s, "graph.preset.apply", &json!({"name": "Nope"})).is_err());
    run(&mut s, "graph.preset.delete", json!({"name": "Low"}));
    assert!(s.graph().unwrap().presets.is_empty());
}

#[test]
fn view3d_target_is_validated_clamped_and_reset() {
    let mut s = sample_session();
    let limit = materialcraft_engine::CameraPrefs::default().pan_limit;
    run(&mut s, "view.3d", json!({"target": [0.5, -0.25, 99.0]}));
    assert_eq!(s.view3d.target, [0.5, -0.25, limit]);
    // A malformed target is refused and changes nothing, not even the other fields
    for bad in [json!([1.0, 2.0]), json!("up"), json!([0.0, "x", 0.0]), json!([0.0, 1e300, 0.0])] {
        assert!(execute(&mut s, "view.3d", &json!({"target": bad, "yaw": 90.0})).is_err());
        assert_eq!(s.view3d.target, [0.5, -0.25, limit]);
        assert_eq!(s.view3d.yaw, 35.0);
    }
    run(&mut s, "view.3d.reset", json!({}));
    assert_eq!(s.view3d.target, [0.0; 3]);
}

/// The height output rises out of the mesh by default; depth and level are clamped and reset with the scene.
#[test]
fn view3d_height_depth_and_level_are_clamped_and_reset() {
    let mut s = sample_session();
    assert_eq!(s.view3d.height_level, 0.0, "the relief rises out of the surface by default");
    assert!(s.view3d.height_scale > 0.0);

    let view = run(&mut s, "view.3d", json!({"heightScale": 0.1, "heightLevel": 0.5}));
    assert_eq!((view["heightScale"].as_f64(), view["heightLevel"].as_f64()), (Some(0.1f32 as f64), Some(0.5)));
    run(&mut s, "view.3d", json!({"heightScale": 9.0, "heightLevel": -3.0}));
    assert_eq!((s.view3d.height_scale, s.view3d.height_level), (0.5, 0.0));
    run(&mut s, "view.3d", json!({"heightLevel": 7.0}));
    assert_eq!(s.view3d.height_level, 1.0);

    run(&mut s, "view.3d.reset", json!({}));
    assert_eq!(s.view3d.height_level, 0.0);
}

#[test]
fn camera_prefs_bound_the_pan_and_survive_hostile_values() {
    use materialcraft_engine::CameraPrefs;
    let mut s = sample_session();
    run(&mut s, "view.3d", json!({"target": [3.0, -4.0, 0.5]}));
    // Lowering the limit pulls the current target in, and later targets respect it
    s.set_camera_prefs(CameraPrefs { pan_limit: 2.0, ..CameraPrefs::default() });
    assert_eq!(s.view3d.target, [2.0, -2.0, 0.5]);
    run(&mut s, "view.3d", json!({"target": [0.0, 9.0, -9.0]}));
    assert_eq!(s.view3d.target, [0.0, 2.0, -2.0]);
    assert_eq!(run(&mut s, "session.inspect", json!({}))["camera"]["panLimit"], 2.0);
    // A zero limit pins the camera to the mesh center
    s.set_camera_prefs(CameraPrefs { pan_limit: 0.0, ..CameraPrefs::default() });
    assert_eq!(s.view3d.target, [0.0; 3]);
    // Non-finite or negative values fall back to the defaults
    s.set_camera_prefs(CameraPrefs { pan_limit: f32::NAN, pan_speed: -1.0 });
    assert_eq!(s.camera, CameraPrefs::default());
    assert_eq!(CameraPrefs { pan_limit: f32::INFINITY, pan_speed: 2.0 }.sanitized().pan_speed, 2.0);
}

#[test]
fn reimported_bitmap_shows_new_pixels() {
    use materialcraft_formats::{Depth, encode};
    use materialcraft_image::{Image, Kind};
    let bmp = |v: f32| encode(&Image::filled(4, 4, Kind::Color, [v, v, v, 1.0]), "bmp", Depth::U8, true).unwrap();
    let (dark, light) = (bmp(0.1), bmp(0.9));
    assert_eq!(dark.len(), light.len(), "same-size files are the case under test");
    let mut s = sample_session();
    materialcraft_engine::file_cmds::add_resource(&mut s, "tex.bmp", dark, Some("tex")).unwrap();
    let before = s.resource_decoder()("tex").unwrap().data[0];
    s.undo();
    materialcraft_engine::file_cmds::add_resource(&mut s, "tex.bmp", light, Some("tex")).unwrap();
    let after = s.resource_decoder()("tex").unwrap().data[0];
    assert!(after - before > 0.5, "{before} -> {after}");
}

#[test]
fn a_wire_splits_with_a_reroute_dot_and_deletes() {
    let mut s = sample_session();
    run(&mut s, "file.new", json!({"template": "empty"}));
    let a = run(&mut s, "graph.node.add", json!({"def": "uniform", "at": [0, 0]}))["id"].as_u64().unwrap();
    let b = run(&mut s, "graph.node.add", json!({"def": "levels", "at": [400, 0]}))["id"].as_u64().unwrap();
    run(&mut s, "graph.connect", json!({"from": a, "to": b}));
    let dot = run(&mut s, "graph.wire.insertDot", json!({"to": b, "toPort": "input", "at": [200, 80]}))["id"].as_u64().unwrap();
    let g = s.graph().unwrap();
    let d = materialcraft_engine::core::NodeId(dot);
    assert_eq!(g.node(d).unwrap().def, "dot");
    assert_eq!(g.node(d).unwrap().pos, [200.0, 80.0]);
    assert_eq!(g.connections.len(), 2);
    assert_eq!(g.input_of(d, "input").unwrap().from.0, a);
    assert_eq!(g.input_of(materialcraft_engine::core::NodeId(b), "input").unwrap().from.0, dot);
    assert_eq!(s.selection, vec![d]);
    // The material still cooks through the dot
    s.cook();
    assert!(s.cooked.nodes.values().all(|r| r.error.is_none()));
    // One undo step restores the straight wire
    run(&mut s, "edit.undo", json!({}));
    assert_eq!(s.graph().unwrap().nodes.len(), 2);
    assert_eq!(s.graph().unwrap().input_of(materialcraft_engine::core::NodeId(b), "input").unwrap().from.0, a);
    // No wire there: an error, nothing changes
    assert!(execute(&mut s, "graph.wire.insertDot", &json!({"to": a, "toPort": "input"})).is_err());
    // Deleting the wire
    run(&mut s, "graph.disconnect", json!({"to": b, "toPort": "input"}));
    assert!(s.graph().unwrap().connections.is_empty());
}

#[test]
fn every_template_is_opaque_in_the_3d_view() {
    // The 3D view sees through a base color whose alpha is below one: the templates must not
    let mut s = sample_session();
    s.preview_size = Some(64);
    for (id, _) in materialcraft_engine::samples::TEMPLATES {
        run(&mut s, "file.new", json!({"template": id, "size": 64}));
        s.cook();
        for (usage, img) in s.material_maps() {
            if usage == "baseColor" && !img.is_gray() {
                let min = img.data.chunks(4).map(|p| p[3]).fold(f32::INFINITY, f32::min);
                assert!(min >= 0.999, "{id}: base color alpha goes down to {min}");
            }
        }
    }
}

#[test]
fn selected_wires_swap_their_sources_and_delete_together() {
    use materialcraft_engine::core::NodeId;
    let mut s = sample_session();
    run(&mut s, "file.new", json!({"template": "empty"}));
    let a = run(&mut s, "graph.node.add", json!({"def": "uniform", "at": [0, 0]}))["id"].as_u64().unwrap();
    let b = run(&mut s, "graph.node.add", json!({"def": "uniform", "at": [0, 200]}))["id"].as_u64().unwrap();
    let m = run(&mut s, "graph.node.add", json!({"def": "uniform", "at": [0, 400]}))["id"].as_u64().unwrap();
    let blend = run(&mut s, "graph.node.add", json!({"def": "blend", "at": [300, 100]}))["id"].as_u64().unwrap();
    run(&mut s, "graph.connect", json!({"from": a, "to": blend, "toPort": "foreground"}));
    run(&mut s, "graph.connect", json!({"from": b, "to": blend, "toPort": "background"}));
    run(&mut s, "graph.connect", json!({"from": m, "to": blend, "toPort": "mask"}));
    let src = |s: &Session, port: &str| s.graph().unwrap().input_of(NodeId(blend), port).unwrap().from.0;
    let fg_bg = json!({"wires": [{"to": blend, "toPort": "background"}, {"to": blend, "toPort": "foreground"}]});
    // Two wires swap, whatever order they are listed in; the other input is left alone
    run(&mut s, "graph.wire.swap", fg_bg.clone());
    assert_eq!((src(&s, "foreground"), src(&s, "background"), src(&s, "mask")), (b, a, m));
    run(&mut s, "graph.wire.swap", fg_bg.clone());
    assert_eq!((src(&s, "foreground"), src(&s, "background")), (a, b));
    // Three wires reverse in input order: foreground <-> mask, background stays
    let all = json!({"wires": [{"to": blend, "toPort": "foreground"}, {"to": blend, "toPort": "background"}, {"to": blend, "toPort": "mask"}]});
    run(&mut s, "graph.wire.swap", all.clone());
    assert_eq!((src(&s, "foreground"), src(&s, "background"), src(&s, "mask")), (m, b, a));
    // One undo step each
    run(&mut s, "edit.undo", json!({}));
    assert_eq!((src(&s, "foreground"), src(&s, "background"), src(&s, "mask")), (a, b, m));
    // Bad requests change nothing
    for bad in [
        json!({}),
        json!({"wires": [{"to": blend, "toPort": "foreground"}]}),
        json!({"wires": [{"to": blend, "toPort": "foreground"}, {"to": blend, "toPort": "foreground"}]}),
        json!({"wires": [{"to": blend, "toPort": "foreground"}, {"to": a, "toPort": "input"}]}),
    ] {
        assert!(execute(&mut s, "graph.wire.swap", &bad).is_err(), "{bad}");
    }
    assert_eq!(src(&s, "foreground"), a);
    // A swap that would make a cycle is refused and rolled back: blend -> levels, then swap the wire into levels with the wire into blend's foreground
    let lv = run(&mut s, "graph.node.add", json!({"def": "levels", "at": [600, 100]}))["id"].as_u64().unwrap();
    run(&mut s, "graph.connect", json!({"from": blend, "to": lv}));
    let cyc = json!({"wires": [{"to": blend, "toPort": "foreground"}, {"to": lv, "toPort": "input"}]});
    assert!(execute(&mut s, "graph.wire.swap", &cyc).is_err());
    assert_eq!(src(&s, "foreground"), a);
    assert_eq!(s.graph().unwrap().connections.len(), 4);
    // Deleting several wires is one step
    run(&mut s, "graph.wire.delete", all);
    assert_eq!(s.graph().unwrap().connections.len(), 1);
    run(&mut s, "edit.undo", json!({}));
    assert_eq!(s.graph().unwrap().connections.len(), 4);
}

#[test]
fn undo_to_unmodified_state_clears_modified_flag() {
    let mut s = sample_session();
    assert!(!s.modified);
    run(&mut s, "graph.node.add", json!({"def": "uniform", "at": [0, 0]}));
    assert!(s.modified);
    run(&mut s, "edit.undo", json!({}));
    assert!(!s.modified, "undoing to the unmodified state should clear the modified flag");
    run(&mut s, "edit.redo", json!({}));
    assert!(s.modified, "redoing to the modified state should set the modified flag");
}
