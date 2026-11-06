//! `session.inspect`: everything an agent needs to verify its work without a screenshot

use serde_json::{Value as J, json};

use crate::Session;

pub fn session(s: &Session, graph: Option<usize>) -> J {
    let gi = graph.unwrap_or(s.graph);
    let g = s.pkg.graphs.get(gi);
    let cooked = (gi == s.graph).then_some(&s.cooked);
    let graph_json = g.map(|g| {
        json!({
            "index": gi,
            "name": g.name,
            "kind": g.kind(),
            "function": g.function,
            "size": g.size_px(),
            "format": g.format,
            "seed": g.seed,
            "attributes": g.attributes,
            "exposed": g.exposed,
            "presets": g.presets,
            "inputs": g.inputs,
            "inputImages": g.input_images,
            "inputImagesLinear": g.input_images_linear,
            "nodes": g.nodes.iter().map(|n| {
                let r = cooked.and_then(|c| c.nodes.get(&n.id));
                let (ins, outs) = materialcraft_eval::ports(&s.pkg, g, n);
                json!({
                    "id": n.id.0,
                    "def": n.def,
                    "label": if n.label.is_empty() { materialcraft_nodes::find(&n.def).map_or(n.def.as_str(), |d| d.label).to_string() } else { n.label.clone() },
                    "pos": n.pos,
                    "params": n.params,
                    "bindings": n.bindings,
                    "functions": n.functions,
                    "inputValues": n.input_values,
                    "base": n.base,
                    "disabled": n.disabled,
                    "inputs": ins.iter().map(|p| &p.id).collect::<Vec<_>>(),
                    "outputs": outs.iter().map(|p| &p.id).collect::<Vec<_>>(),
                    "cooked": r.map(|r| json!({
                        "error": r.error, "ms": r.ms, "cached": r.cached,
                        "value": r.outputs.first().and_then(materialcraft_eval::NodeOutput::as_value),
                        "size": r.outputs.first().and_then(materialcraft_eval::NodeOutput::as_image).map(|i| [i.w, i.h]),
                        "kind": r.outputs.first().and_then(materialcraft_eval::NodeOutput::as_image).map(|i| if i.is_gray() { "gray" } else { "color" }),
                        "mean": r.outputs.first().and_then(materialcraft_eval::NodeOutput::as_image).map(|i| i.mean()),
                    })),
                })
            }).collect::<Vec<_>>(),
            "connections": g.connections,
            "frames": g.frames,
            "comments": g.comments,
            "pins": g.pins,
        })
    });
    json!({
        "package": {"name": s.pkg.name, "path": s.path, "modified": s.modified, "warnings": s.load_warnings, "resources": s.pkg.resources.iter().map(|r| json!({"name": r.name, "bytes": r.bytes.len()})).collect::<Vec<_>>()},
        "graphs": s.pkg.graphs.iter().enumerate().map(|(i, g)| json!({"index": i, "name": g.name, "nodes": g.nodes.len(), "kind": g.kind()})).collect::<Vec<_>>(),
        "openGraph": s.graph,
        "graph": graph_json,
        "annotationSelection": s.annotation_selection,
        "selection": s.selection.iter().map(|i| i.0).collect::<Vec<_>>(),
        "view2d": s.view2d,
        "view3d": s.view3d,
        "material3d": {
            "assignments": s.material_preview,
            "channels": s.material_preview.sources(&s.pkg).into_iter().map(|(channel, source)| {
                let image = s.material_image(&source);
                json!({"channel": channel, "source": source, "size": image.map(|image| [image.w, image.h]), "mean": image.map(|image| image.mean())})
            }).collect::<Vec<_>>(),
        },
        "previewSize": s.preview_size,
        "camera": s.camera,
        "cookMs": s.cooked.ms,
        "dirty": s.dirty,
        "history": s.history(),
        "canUndo": s.can_undo(),
        "canRedo": s.can_redo(),
        "log": s.log.iter().rev().take(20).collect::<Vec<_>>(),
    })
}
