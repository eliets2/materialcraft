use materialcraft_core::{Package, Resource};
use materialcraft_formats::{Depth, encode};
use materialcraft_image::{Image, Kind, resource_key};

use crate::session::Session;

fn normal_map_session() -> Session {
    let mut pkg = Package::new("T");
    let png = encode(&Image::filled(4, 4, Kind::Color, [0.5, 0.5, 1.0, 1.0]), "png", Depth::U8, false).unwrap();
    pkg.resources.push(Resource { name: "n".into(), file: "n.png".into(), bytes: materialcraft_core::ResourceBytes::new(png) });
    Session::new(pkg)
}

#[test]
fn the_resource_decoder_reads_data_maps_as_stored() {
    let s = normal_map_session();
    let dec = s.resource_decoder();
    let color = dec(&resource_key("n", false)).unwrap();
    let data = dec(&resource_key("n", true)).unwrap();
    // 8-bit color is sRGB-decoded (0.5 -> ~0.214); a data map keeps 0.5.
    assert!((color.get(0, 0, false)[0] - 0.214).abs() < 0.01);
    assert!((data.get(0, 0, false)[0] - 0.5).abs() < 0.01);
    assert!((data.get(0, 0, false)[2] - 1.0).abs() < 0.01);
}

#[cfg(test)]
mod merge_tests {
    use crate::Session;

    fn set(s: &mut Session, key: &str) {
        s.edit_merged(key, "Set parameter", |s| {
            s.modified = true;
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn same_key_merges_until_finish_edit() {
        let mut s = crate::new_session();
        let base = s.history().len();
        set(&mut s, "k");
        set(&mut s, "k");
        assert_eq!(s.history().len(), base + 1, "one drag is one entry");
        s.finish_edit();
        set(&mut s, "k");
        assert_eq!(s.history().len(), base + 2, "a second drag after finish_edit is a new entry");
    }
}

#[test]
fn failed_merged_edits_restore_clean_flags_and_history() {
    let mut s = crate::sample_session();
    s.edit_merged("k", "Set", |_| Ok(())).unwrap();
    s.modified = false;
    s.dirty = false;
    let name = s.pkg.name.clone();
    let history = s.history();
    let result: crate::Result<()> = s.edit_merged("k", "Set", |s| {
        s.pkg.name = "partial".into();
        s.changed();
        Err(crate::EngineError::Other("failed".into()))
    });
    assert!(result.is_err());
    assert_eq!(s.pkg.name, name);
    assert_eq!(s.history(), history);
    assert!(!s.modified && !s.dirty);
}

#[cfg(test)]
mod group_tests {
    use crate::{EngineError, Session, execute};
    use serde_json::json;

    fn two_nodes() -> (Session, u64, u64) {
        let mut s = crate::sample_session();
        execute(&mut s, "file.new", &json!({"template": "empty"})).unwrap();
        let a = execute(&mut s, "graph.node.add", &json!({"def": "uniform"})).unwrap()["id"].as_u64().unwrap();
        let b = execute(&mut s, "graph.node.add", &json!({"def": "levels"})).unwrap()["id"].as_u64().unwrap();
        (s, a, b)
    }

    #[test]
    fn a_group_is_one_undo_entry_and_one_undo_reverts_all() {
        let (mut s, a, b) = two_nodes();
        let base = s.history().len();
        s.group("Rewire", |s| {
            execute(s, "graph.connect", &json!({"from": a, "to": b, "toPort": "input"}))?;
            execute(s, "graph.disconnect", &json!({"to": b, "toPort": "input"}))?;
            execute(s, "graph.connect", &json!({"from": a, "to": b, "toPort": "input"}))?;
            Ok(())
        })
        .unwrap();
        assert_eq!(s.history().len(), base + 1);
        assert_eq!(s.history().last().map(String::as_str), Some("Rewire"));
        assert_eq!(s.undo().as_deref(), Some("Rewire"));
        assert!(s.graph().unwrap().connections.is_empty(), "the single undo removed every step");
    }

    #[test]
    fn a_failing_group_restores_everything_and_records_nothing() {
        let (mut s, a, b) = two_nodes();
        let base = s.history().len();
        let r: Result<(), EngineError> = s.group("Rewire", |s| {
            execute(s, "graph.connect", &json!({"from": a, "to": b, "toPort": "input"}))?;
            Err(EngineError::Other("boom".into()))
        });
        assert!(r.is_err());
        assert_eq!(s.history().len(), base);
        assert!(s.graph().unwrap().connections.is_empty());
    }

    #[test]
    fn begin_and_end_group_span_several_calls_and_an_untouched_group_records_nothing() {
        let (mut s, a, b) = two_nodes();
        let base = s.history().len();
        s.begin_group("Nothing");
        s.end_group();
        assert_eq!(s.history().len(), base);
        s.begin_group("Both");
        execute(&mut s, "graph.connect", &json!({"from": a, "to": b, "toPort": "input"})).unwrap();
        execute(&mut s, "graph.node.param", &json!({"id": b, "param": "in_low", "value": [0.2, 0.2, 0.2, 0.2]})).unwrap();
        s.end_group();
        assert_eq!(s.history().len(), base + 1);
    }
}

#[test]
fn a_group_with_only_failed_edits_preserves_redo_and_clean_flags() {
    let mut s = crate::sample_session();
    s.edit("Prior", |s| {
        s.pkg.name = "Prior".into();
        Ok(())
    })
    .unwrap();
    s.undo();
    assert!(s.can_redo());
    s.modified = false;
    s.dirty = false;
    let history = s.history();
    let name = s.pkg.name.clone();
    s.begin_group("Rejected");
    let result: crate::Result<()> = s.edit("Bad", |s| {
        s.pkg.name = "Partial".into();
        Err(crate::EngineError::Other("failed".into()))
    });
    assert!(result.is_err());
    s.end_group();
    assert_eq!(s.pkg.name, name);
    assert_eq!(s.history(), history);
    assert!(s.can_redo());
    assert!(!s.modified && !s.dirty);
}

#[test]
fn a_caught_nested_group_error_rolls_back_only_its_own_edits() {
    let mut s = crate::sample_session();
    let original = s.pkg.name.clone();
    let count = s.history().len();
    s.group("Outer", |s| {
        s.edit("Outer edit", |s| {
            s.pkg.name = "Outer".into();
            Ok(())
        })?;
        let result: crate::Result<()> = s.group("Inner", |s| {
            s.edit("Inner edit", |s| {
                s.pkg.name = "Partial inner".into();
                Ok(())
            })?;
            Err(crate::EngineError::Other("failed".into()))
        });
        assert!(result.is_err());
        assert_eq!(s.pkg.name, "Outer");
        Ok(())
    })
    .unwrap();
    assert_eq!(s.history().len(), count + 1);
    s.undo();
    assert_eq!(s.pkg.name, original);
}

#[test]
fn a_failed_group_recooks_if_it_cooked_a_temporary_document() {
    use crate::{core::NodeId, execute};
    use serde_json::json;
    let mut s = crate::new_session();
    s.preview_size = Some(16);
    let id = NodeId(execute(&mut s, "graph.node.add", &json!({"def": "uniform"})).unwrap()["id"].as_u64().unwrap());
    execute(&mut s, "graph.node.param", &json!({"id": id.0, "param": "color_mode", "value": true})).unwrap();
    let before = s.cook().image(id, 0).unwrap().data.clone();
    let result: crate::Result<()> = s.group("Temporary", |s| {
        execute(s, "graph.node.param", &json!({"id": id.0, "param": "color", "value": [0.1, 0.8, 0.2, 1.0]}))?;
        s.cook();
        Err(crate::EngineError::Other("failed".into()))
    });
    assert!(result.is_err());
    s.cook_if_dirty();
    assert_eq!(s.cooked.image(id, 0).unwrap().data, before);
}
