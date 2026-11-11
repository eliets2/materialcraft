use materialcraft_engine::{Session, core::NodeId, execute, file_cmds::export_outputs, formats, new_session};
use serde_json::json;

fn session() -> (Session, Vec<u64>) {
    let mut s = new_session();
    execute(&mut s, "graph.size", &json!({"width": 16})).unwrap();
    let mut outputs = Vec::new();
    for (name, value) in [("first", 0.2), ("second", 0.8)] {
        let source =
            execute(&mut s, "graph.node.add", &json!({"def": "uniform", "params": {"color": [value, value, value, 1.0]}})).unwrap()["id"].as_u64().unwrap();
        let output =
            execute(&mut s, "graph.node.add", &json!({"def": "output", "params": {"identifier": name}, "connectFrom": {"node": source}})).unwrap()["id"]
                .as_u64()
                .unwrap();
        outputs.push(output);
    }
    (s, outputs)
}

#[test]
fn export_rejects_duplicate_final_names_from_patterns_identifiers_and_sanitization() {
    for (identifiers, pattern) in [
        (["first", "second"], "same"),
        (["first", "second"], "$(usage)"),
        (["same", "same"], "$(identifier)"),
        (["Map", "map"], "$(identifier)"),
        (["map?", "map*"], "$(identifier)"),
        (["a/map", "b/map"], "$(identifier)"),
    ] {
        let (mut s, outputs) = session();
        for (&id, name) in outputs.iter().zip(identifiers) {
            execute(&mut s, "graph.node.param", &json!({"id": id, "param": "identifier", "value": name})).unwrap();
        }
        let result = export_outputs(&s, 0, "png", pattern, None, None);
        assert!(result.is_err(), "colliding names were accepted: {identifiers:?}, {pattern}");
        let error = result.err().unwrap().to_string();
        assert!(error.contains("filename") && error.contains("identifier"), "{error}");
    }
}

#[cfg(not(target_arch = "wasm32"))]
struct TempDir(std::path::PathBuf);

#[cfg(not(target_arch = "wasm32"))]
impl TempDir {
    fn new(name: &str) -> Self {
        let nonce = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("mc-export-{name}-{}-{nonce}", std::process::id()));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn colliding_export_preserves_existing_files_and_writes_no_partial_outputs() {
    let (mut s, outputs) = session();
    let source = s.graph().unwrap().input_of(NodeId(outputs[0]), "input").unwrap().from;
    // The third output collides only after the first two have distinct names.
    execute(&mut s, "graph.node.add", &json!({"def": "output", "params": {"identifier": "first"}, "connectFrom": {"node": source.0}})).unwrap();
    let dir = TempDir::new("collision");
    let sentinel = b"existing file must survive a rejected export";
    std::fs::write(dir.0.join("first.png"), sentinel).unwrap();
    let result = execute(&mut s, "file.export", &json!({"dir": dir.0, "pattern": "$(identifier)"}));
    let existing = std::fs::read(dir.0.join("first.png")).unwrap();
    let files = std::fs::read_dir(&dir.0).unwrap().count();
    assert!(
        result.is_err() && existing == sentinel && files == 1,
        "colliding export modified files: response={result:?}, files={files}, sentinel preserved={}",
        existing == sentinel
    );
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn export_rejects_existing_file_in_destination_before_writing() {
    let (mut s, _) = session();
    let dir = TempDir::new("existing");
    let sentinel = b"existing file must survive a rejected export";
    std::fs::write(dir.0.join("first.png"), sentinel).unwrap();
    let result = execute(&mut s, "file.export", &json!({"dir": dir.0, "pattern": "$(identifier)"}));
    let existing = std::fs::read(dir.0.join("first.png")).unwrap();
    let files = std::fs::read_dir(&dir.0).unwrap().count();
    assert!(
        result.is_err() && existing == sentinel && files == 1,
        "export overwrote an existing destination file: response={result:?}, files={files}, sentinel preserved={}",
        existing == sentinel
    );
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn unique_export_writes_both_maps_with_their_expected_pixels() {
    let (mut s, _) = session();
    let dir = TempDir::new("unique");
    let result = execute(&mut s, "file.export", &json!({"dir": dir.0, "pattern": "$(identifier)"})).unwrap();
    assert_eq!(result["files"].as_array().unwrap().len(), 2);
    for (name, value) in [("first.png", 0.2), ("second.png", 0.8)] {
        let image = formats::decode(&std::fs::read(dir.0.join(name)).unwrap(), false).unwrap();
        assert_eq!((image.w, image.h), (16, 16));
        assert!((image.get(0, 0, false)[0] - value).abs() < 0.001, "{name}");
    }
}
