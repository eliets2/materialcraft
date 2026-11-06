#[cfg(any(test, not(target_arch = "wasm32")))]
use std::collections::BTreeMap;

use serde_json::{Value as J, json};

use materialcraft_core::ComponentUsage;
use materialcraft_core::Graph;
#[cfg(any(test, not(target_arch = "wasm32")))]
use materialcraft_core::Value;
use materialcraft_formats::material::{FORMAT, ImageInput, Manifest, MapEntry, Parameter, PresetEntry, VERSION, portable, srgb_usage};
use materialcraft_formats::{Depth, encode};
#[cfg(test)]
use materialcraft_image::Image;

#[cfg(not(target_arch = "wasm32"))]
use crate::commands::u64p;
use crate::commands::{CommandSpec, bad, has_graph, strp};
use crate::{Result, Session, cmd};
#[cfg(not(target_arch = "wasm32"))]
use materialcraft_formats::material::MATERIAL_EXT;

pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "file.exportMaterial",
            "Export material...",
            ["File"],
            None,
            "{path, graph?, size? (px, power of two), values? {input: value}, inputs? {identifier: image path}} cooks the graph and writes a portable .mcmat material (maps, exposed inputs, source package) for TextureCraft",
            has_graph,
            export_material
        ),
        cmd!(
            "file.sendToTextureCraft",
            "Send to TextureCraft",
            ["File"],
            None,
            "{graph?} exports the graph as a portable material into TextureCraft's library folder (Documents/TextureCraft/library), where its shelf lists it",
            has_graph,
            send_to_texturecraft
        ),
        cmd!(
            "graph.input.bind",
            "Set input preview image",
            [],
            None,
            "{identifier, resource? | path?, linear?} shows a bitmap resource (or an image file, embedded) on the graph's input node `identifier`, e.g. a mesh map baked in TextureCraft",
            has_graph,
            bind
        ),
        cmd!("graph.input.unbind", "Clear input preview image", [], None, "{identifier}", has_graph, unbind),
    ]
}

pub use crate::MaterialOptions;

/// Cook graph `gi` with `o` and pack it as a portable material (`.mcmat` bytes)
pub fn material_bytes(s: &Session, gi: usize, o: &MaterialOptions) -> Result<Vec<u8>> {
    let mut pkg = s.pkg.clone();
    let g = pkg.graph_mut(gi)?;
    if let Some(px) = o.size {
        let log2 = 31 - px.clamp(16, 8192).leading_zeros();
        g.size = [log2, log2];
    }
    g.inputs.extend(o.values.clone());
    let g = pkg.graph(gi)?;
    let dec = s.resource_decoder();
    let mut opts = materialcraft_eval::Options::new(&dec);
    opts.inputs = o.inputs.clone();
    let cooked = materialcraft_eval::cook(&pkg, gi, &opts, &s.cache);
    let mut maps: Vec<(String, Vec<u8>)> = Vec::new();
    let mut entries = Vec::new();
    let mut size = [0, 0];
    for (_, usage, _, img) in materialcraft_eval::outputs(g, &cooked) {
        let Some(img) = img else { continue };
        if usage == "any" || entries.iter().any(|e: &MapEntry| e.usage == usage) {
            continue;
        }
        let srgb = srgb_usage(usage);
        let file = format!("maps/{usage}.png");
        maps.push((file.clone(), encode(img, "png", if srgb { Depth::U8 } else { Depth::U16 }, srgb)?));
        entries.push(MapEntry { usage: usage.into(), file, color_space: if srgb { "srgb" } else { "linear" }.into() });
        size = [img.w, img.h];
    }
    if entries.is_empty() {
        return Err(bad(
            "file.exportMaterial",
            format!(
                "graph `{}` has no outputs with a material usage ({})",
                g.name,
                ComponentUsage::ALL.iter().filter(|usage| **usage != ComponentUsage::ANY).map(|usage| usage.id).collect::<Vec<_>>().join(", ")
            ),
        ));
    }
    let manifest = Manifest {
        format: FORMAT.into(),
        version: VERSION,
        name: g.name.replace('_', " "),
        graph: g.name.clone(),
        size,
        maps: entries,
        parameters: g
            .exposed
            .iter()
            .filter(|e| portable(&e.default))
            .map(|e| Parameter { param: e.clone(), value: g.input_value(&e.id).cloned().unwrap_or_else(|| e.default.clone()) })
            .collect(),
        inputs: image_inputs(g),
        presets: g.presets.iter().map(|p| PresetEntry { name: p.name.clone(), values: p.values.clone() }).collect(),
        source: String::new(),
    };

    let source = materialcraft_formats::sbs::write_sbs(&pkg)?;
    Ok(materialcraft_formats::material::write_material(&manifest, &maps, &source)?)
}

/// The graph's input nodes (one entry per identifier)
pub fn image_inputs(g: &Graph) -> Vec<ImageInput> {
    let mut out: Vec<ImageInput> = Vec::new();
    for n in g.nodes.iter().filter(|n| matches!(n.def.as_str(), "input_grayscale" | "input_color")) {
        let identifier = materialcraft_eval::identifier(n);
        if !out.iter().any(|i| i.identifier == identifier) {
            out.push(ImageInput { identifier, kind: if n.def == "input_grayscale" { "gray" } else { "color" }.into() });
        }
    }
    out
}

/// `values` as typed exposed input values of graph `g`
#[cfg(any(test, not(target_arch = "wasm32")))]
fn values_param(cmd: &str, g: &Graph, p: &J) -> Result<BTreeMap<String, Value>> {
    let Some(obj) = p.get("values").and_then(J::as_object) else { return Ok(BTreeMap::new()) };
    obj.iter()
        .map(|(id, v)| {
            let e = g.exposed.iter().find(|e| e.id == *id).ok_or_else(|| bad(cmd, format!("no input `{id}`")))?;
            let val = e.default.coerce_json(v).ok_or_else(|| bad(cmd, format!("`{id}` takes a {}", e.default.type_name())))?;
            Ok((id.clone(), val))
        })
        .collect()
}

#[cfg(not(target_arch = "wasm32"))]
fn graph_param(s: &Session, p: &J) -> Result<usize> {
    let gi = u64p(p, "graph").map_or(s.graph, |g| g as usize);
    s.pkg.graph(gi)?;
    Ok(gi)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn material_request(s: &Session, cmd: &str, p: &J) -> Result<(usize, MaterialOptions)> {
    let gi = graph_param(s, p)?;
    let o = MaterialOptions { size: u64p(p, "size").map(|v| v.min(8192) as u32), values: values_param(cmd, s.pkg.graph(gi)?, p)?, ..Default::default() };
    Ok((gi, o))
}

#[cfg(not(target_arch = "wasm32"))]
fn export_material(s: &mut Session, p: &J) -> Result<J> {
    const CMD: &str = "file.exportMaterial";
    let path = strp(p, "path").ok_or_else(|| bad(CMD, "`path` is required"))?;
    let path = if path.to_ascii_lowercase().ends_with(&format!(".{MATERIAL_EXT}")) { path.to_string() } else { format!("{path}.{MATERIAL_EXT}") };
    let (gi, mut o) = material_request(s, CMD, p)?;
    for (id, file) in p.get("inputs").and_then(J::as_object).into_iter().flatten() {
        let file = file.as_str().ok_or_else(|| bad(CMD, format!("inputs.{id} must be an image path")))?;
        let bytes = std::fs::read(file).map_err(|e| bad(CMD, format!("{file}: {e}")))?;
        o.inputs.insert(id.clone(), materialcraft_formats::decode(&bytes, false)?);
    }
    let bytes = material_bytes(s, gi, &o)?;
    materialcraft_formats::write_atomic(std::path::Path::new(&path), &bytes)?;
    s.log(format!("Exported material {path}"));
    Ok(json!({"path": path, "bytes": bytes.len()}))
}

#[cfg(target_arch = "wasm32")]
fn export_material(_: &mut Session, _: &J) -> Result<J> {
    Err(bad("file.exportMaterial", "the browser exports with a download"))
}

/// TextureCraft's library folder: `Documents/TextureCraft/library` in the home folder
#[cfg(not(target_arch = "wasm32"))]
pub fn texturecraft_library() -> std::path::PathBuf {
    let home = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")).unwrap_or_else(|_| ".".into());
    std::path::Path::new(&home).join("Documents").join("TextureCraft").join("library")
}

#[cfg(not(target_arch = "wasm32"))]
fn send_to_texturecraft(s: &mut Session, p: &J) -> Result<J> {
    const CMD: &str = "file.sendToTextureCraft";
    let gi = graph_param(s, p)?;
    let bytes = material_bytes(s, gi, &MaterialOptions::default())?;
    let dir = texturecraft_library();
    std::fs::create_dir_all(&dir).map_err(|e| bad(CMD, format!("{}: {e}", dir.display())))?;
    let path = dir.join(format!("{}.{MATERIAL_EXT}", materialcraft_formats::safe_file_name(&s.pkg.graph(gi)?.name)));
    materialcraft_formats::write_atomic(&path, &bytes)?;
    let path = path.display().to_string();
    s.log(format!("Sent to TextureCraft: {path}"));
    Ok(json!({"path": path}))
}

#[cfg(target_arch = "wasm32")]
fn send_to_texturecraft(_: &mut Session, _: &J) -> Result<J> {
    Err(bad("file.sendToTextureCraft", "the browser has no shared library folder: export the material instead"))
}

fn identifier_param(s: &Session, cmd: &str, p: &J) -> Result<String> {
    let id = strp(p, "identifier").ok_or_else(|| bad(cmd, "`identifier` is required"))?;
    if !image_inputs(s.graph()?).iter().any(|i| i.identifier == id) {
        return Err(bad(cmd, format!("the graph has no input node `{id}`")));
    }
    Ok(id.to_string())
}

fn bind(s: &mut Session, p: &J) -> Result<J> {
    const CMD: &str = "graph.input.bind";
    let id = identifier_param(s, CMD, p)?;
    let file = match (strp(p, "resource"), strp(p, "path")) {
        (Some(r), _) if s.pkg.resource(r).is_some() => None,
        (Some(r), _) => return Err(bad(CMD, format!("no bitmap resource `{r}`"))),
        (None, Some(path)) => Some(read_image_file(CMD, path)?),
        (None, None) => return Err(bad(CMD, "give a `resource` or a `path`")),
    };
    s.edit("Set input preview image", |s| {
        let res = match file {
            Some((name, bytes)) => crate::file_cmds::push_resource(&mut s.pkg, &name, bytes, Some(&id)),
            None => strp(p, "resource").unwrap_or_default().to_string(),
        };
        let linear = p.get("linear").and_then(J::as_bool).unwrap_or(false);
        let g = s.graph_mut()?;
        g.input_images.insert(id.clone(), res.clone());
        if linear {
            g.input_images_linear.insert(id.clone());
        } else {
            g.input_images_linear.remove(&id);
        }
        Ok(json!({"identifier": id, "resource": res}))
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn read_image_file(cmd: &str, path: &str) -> Result<(String, Vec<u8>)> {
    let bytes = std::fs::read(path).map_err(|e| bad(cmd, format!("{path}: {e}")))?;
    materialcraft_formats::decode(&bytes, false)?;
    let name = std::path::Path::new(path).file_name().and_then(|f| f.to_str()).unwrap_or("input.png").to_string();
    Ok((name, bytes))
}

#[cfg(target_arch = "wasm32")]
fn read_image_file(cmd: &str, _: &str) -> Result<(String, Vec<u8>)> {
    Err(bad(cmd, "the browser imports images with the file picker: import a bitmap, then pass its `resource`"))
}

fn unbind(s: &mut Session, p: &J) -> Result<J> {
    let id = strp(p, "identifier").ok_or_else(|| bad("graph.input.unbind", "`identifier` is required"))?.to_string();
    s.edit("Clear input preview image", |s| {
        s.graph_mut()?.input_images_linear.remove(&id);
        Ok(J::Bool(s.graph_mut()?.input_images.remove(&id).is_some()))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use materialcraft_formats::material::read_material;

    #[test]
    fn exports_maps_inputs_and_values() {
        let mut s = crate::sample_session();
        crate::execute(&mut s, "graph.exposed.add", &json!({"id": "wear", "default": 0.5})).unwrap();
        crate::execute(&mut s, "graph.exposed.add", &json!({"id": "label", "default": {"t": "text", "v": "x"}})).unwrap();
        let values = values_param("t", s.graph().unwrap(), &json!({"values": {"wear": 0.9}})).unwrap();
        let bytes = material_bytes(&s, s.graph, &MaterialOptions { size: Some(32), values, ..Default::default() }).unwrap();
        let (m, source) = read_material(&bytes).unwrap();
        assert_eq!(m.size, [32, 32]);
        assert!(m.maps.iter().any(|entry| entry.usage == "height" && entry.color_space == "linear"));
        let ids: Vec<&str> = m.parameters.iter().map(|p| p.param.id.as_str()).collect();
        assert_eq!(ids, ["wear"], "text inputs are not portable");
        assert_eq!(m.parameters[0].value, Value::Float(0.9), "the manifest carries the cooked value");
        assert_eq!(
            materialcraft_formats::sbs::read_sbs(&source, "source.sbs", &|_| None).unwrap().pkg.graphs[s.graph].input_value("wear"),
            Some(&Value::Float(0.9))
        );
        // The source package opens again, on the material's graph, with no file to save over
        crate::file_cmds::open_bytes(&mut s, &bytes, Some("x.mcmat".into())).unwrap();
        assert_eq!(s.graph().unwrap().name, m.graph);
        assert!(s.path.is_none());
    }

    #[test]
    fn binds_preview_images_to_input_nodes() {
        let mut s = crate::sample_session();
        crate::execute(&mut s, "graph.node.add", &json!({"def": "input_grayscale", "params": {"identifier": "curvature"}})).unwrap();
        let png = encode(&Image::filled(4, 4, materialcraft_image::Kind::Gray, [0.7; 4]), "png", Depth::U16, false).unwrap();
        let res = crate::file_cmds::add_resource(&mut s, "curv.png", png, None).unwrap();
        assert!(crate::execute(&mut s, "graph.input.bind", &json!({"identifier": "nope", "resource": res})).is_err());
        crate::execute(&mut s, "graph.input.bind", &json!({"identifier": "curvature", "resource": res, "linear": true})).unwrap();
        assert_eq!(s.graph().unwrap().input_images.get("curvature"), Some(&res));
        let bytes = materialcraft_formats::sbs::write_sbs(&s.pkg).unwrap();
        let reopened = materialcraft_formats::sbs::read_sbs(&bytes, "input.sbs", &|_| None).unwrap().pkg;
        assert_eq!(reopened.graphs[s.graph].input_images.get("curvature"), Some(&res));
        assert!(reopened.graphs[s.graph].input_images_linear.contains("curvature"));
        let bytes = material_bytes(&s, s.graph, &MaterialOptions { size: Some(16), ..Default::default() }).unwrap();
        let (m, _) = read_material(&bytes).unwrap();
        assert!(m.inputs.iter().any(|i| i.identifier == "curvature" && i.kind == "gray"));
        assert_eq!(crate::execute(&mut s, "graph.input.unbind", &json!({"identifier": "curvature"})).unwrap(), J::Bool(true));
        assert!(s.graph().unwrap().input_images.is_empty());
        assert!(s.graph().unwrap().input_images_linear.is_empty());
    }

    #[test]
    fn rejects_unknown_values() {
        let s = crate::sample_session();
        let g = s.graph().unwrap();
        assert!(values_param("t", g, &json!({"values": {"no_such_input": 1}})).is_err());
        assert!(values_param("t", g, &json!({})).unwrap().is_empty());
    }
}
