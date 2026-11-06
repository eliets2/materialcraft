use materialcraft_engine::{
    core::{FunctionGraph, PatternGraph, PatternNodeKind, Value},
    graph_edit::{self, EditableGraph},
};

fn exercise_clipboard<G: EditableGraph>(mut graph: G, chain: [u32; 3]) {
    let mut selection = vec![chain[1], chain[2]];
    let mut clipboard = Vec::new();
    let before = graph.nodes().len();
    graph_edit::run(&mut graph, &mut selection, &mut clipboard, "edit.copy", [0.0; 2], true).unwrap();
    for command in ["edit.paste", "edit.pasteWithoutLinks"] {
        graph_edit::run(&mut graph, &mut selection, &mut clipboard, command, [19.0, 19.0], true).unwrap();
        assert_eq!(selection.len(), 2);
        let first = graph.nodes().iter().find(|node| G::id(node) == selection[0]).unwrap();
        let second = graph.nodes().iter().find(|node| G::id(node) == selection[1]).unwrap();
        assert_eq!(G::links(second)[0], Some(selection[0]));
        assert_eq!(G::links(first)[0], if command == "edit.paste" { Some(chain[0]) } else { None });
        assert!(graph_edit::remove(&mut graph, &mut selection, false));
        assert_eq!(graph.nodes().len(), before);
    }

    for command in ["graph.selectUpstream", "graph.selectDownstream"] {
        selection = vec![chain[1]];
        graph_edit::run(&mut graph, &mut selection, &mut clipboard, command, [0.0; 2], false).unwrap();
        let follow_links = (command == "graph.selectUpstream") == G::LINKS_ARE_INPUTS;
        let mut expected = if follow_links { vec![chain[0], chain[1]] } else { vec![chain[1], chain[2]] };
        expected.sort();
        assert_eq!(selection, expected);
    }
}

#[test]
fn function_and_pattern_clipboards_keep_internal_links_and_respect_direction() {
    let mut function = FunctionGraph::default();
    let a = function.add_constant(Value::Float(1.0));
    let b = function.add("passthrough", &[a], None);
    let c = function.add("passthrough", &[b], None);
    exercise_clipboard(function, [a, b, c]);

    let mut pattern = PatternGraph::default();
    let a = pattern.add(PatternNodeKind::Dot).unwrap();
    let b = pattern.add(PatternNodeKind::Dot).unwrap();
    let c = pattern.add(PatternNodeKind::Dot).unwrap();
    pattern.nodes[1].children[0] = Some(a);
    pattern.nodes[2].children[0] = Some(b);
    exercise_clipboard(pattern, [a, b, c]);
}

#[test]
fn pattern_paste_capacity_failure_is_atomic_and_root_is_preserved() {
    let mut graph = PatternGraph::with_root(PatternNodeKind::Dot);
    let node = graph.nodes[0].clone();
    graph.nodes = (1..4096).map(|id| materialcraft_engine::core::PatternNode { id, ..node.clone() }).collect();
    let before = graph.clone();
    let mut clipboard = vec![node.clone(), node];
    let mut selection = vec![1];
    assert!(graph_edit::run(&mut graph, &mut selection, &mut clipboard, "edit.paste", [0.0; 2], true).is_err());
    assert_eq!(graph, before);
    assert_eq!(selection, vec![1]);
}

#[test]
fn function_delete_relinks_chains_and_clears_a_deleted_output() {
    let mut graph = FunctionGraph::default();
    let a = graph.add_constant(Value::Float(1.0));
    let b = graph.add("passthrough", &[a], None);
    let c = graph.add("passthrough", &[b], None);
    let d = graph.add("passthrough", &[c], None);
    graph.output = Some(d);
    assert!(graph_edit::remove(&mut graph, &mut vec![b, c], true));
    assert_eq!(graph.node(d).unwrap().inputs, vec![Some(a)]);
    assert_eq!(graph.output, Some(d));
    assert!(graph_edit::remove(&mut graph, &mut vec![d], false));
    assert_eq!(graph.output, None);
}

#[test]
fn pattern_clipboard_preserves_selection_order_when_ids_have_holes() {
    let mut graph = PatternGraph::default();
    for _ in 0..3 {
        graph.add(PatternNodeKind::Quadrant).unwrap();
    }
    graph.nodes.remove(1);
    let id = graph.add(PatternNodeKind::Quadrant).unwrap();
    assert_eq!(id, 2);
    let mut selection = vec![2, 3];
    let mut clipboard = Vec::new();
    graph_edit::run(&mut graph, &mut selection, &mut clipboard, "edit.duplicate", [0.0; 2], false).unwrap();
    assert_eq!(selection, vec![5, 4]);
}

#[test]
#[ignore = "release timing probe; run explicitly with --ignored --nocapture"]
fn connected_selection_performance() {
    use std::{collections::BTreeSet, time::Instant};

    let mut graph = FunctionGraph::default();
    for id in 1..=4096 {
        graph.nodes.push(materialcraft_engine::core::FunctionNode {
            id,
            op: "passthrough".into(),
            type_code: 0,
            inputs: if id == 1 { vec![] } else { vec![Some(id - 1)] },
            value: None,
            pos: [0.0; 2],
        });
    }
    let mut before = Vec::new();
    let mut after = Vec::new();
    for _ in 0..3 {
        let start = Instant::now();
        let mut visited = BTreeSet::new();
        let mut pending = vec![1];
        while let Some(id) = pending.pop() {
            if visited.insert(id) {
                pending.extend(graph.nodes.iter().filter(|node| node.inputs.contains(&Some(id))).map(|node| node.id));
            }
        }
        let expected: Vec<_> = visited.into_iter().collect();
        before.push(start.elapsed().as_secs_f64() * 1000.0);
        let mut selection = vec![1];
        let start = Instant::now();
        graph_edit::run(std::hint::black_box(&mut graph), &mut selection, &mut Vec::new(), "graph.selectDownstream", [0.0; 2], false).unwrap();
        after.push(start.elapsed().as_secs_f64() * 1000.0);
        assert_eq!(selection, expected);
    }
    before.sort_by(f64::total_cmp);
    after.sort_by(f64::total_cmp);
    println!("4096-node downstream selection, median of three: scan {:.3} ms; shared index {:.3} ms", before[1], after[1]);
}
