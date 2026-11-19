#[cfg(any(test, not(target_arch = "wasm32")))]
use std::collections::BTreeMap;

#[cfg(test)]
use craft_texture::Image;
#[cfg(test)]
use materialcraft_core::ComponentUsage;
#[cfg(any(test, not(target_arch = "wasm32")))]
use materialcraft_core::Value;
#[cfg(any(test, not(target_arch = "wasm32")))]
use materialcraft_formats::Depth;
#[cfg(test)]
use materialcraft_formats::encode;
#[cfg(any(test, not(target_arch = "wasm32")))]
use materialcraft_formats::material::srgb_usage;

#[cfg(not(target_arch = "wasm32"))]
use crate::commands::u64p;
use crate::commands::{CommandSpec, bad, has_graph, strp};
use crate::{Result, Session, cmd};
use materialcraft_core::Graph;
#[cfg(any(test, not(target_arch = "wasm32")))]
use materialcraft_core::{MAX_SIZE_LOG2, MIN_SIZE_LOG2};
use materialcraft_formats::material::ImageInput;
#[cfg(not(target_arch = "wasm32"))]
use materialcraft_formats::material::MATERIAL_EXT;
use serde_json::{Value as J, json};

pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "file.exportMaterial",
            "Export material...",
            ["File"],
            None,
            "{path, graph? (index or SBS path)} publishes one root graph with its source dependencies in a v3 .mcmat",
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

/// Publish a self-contained source snapshot after checking the selected outputs can cook.
pub fn material_bytes(session: &Session, graph: usize) -> Result<Vec<u8>> {
    crate::material_bake::check_publish(session, graph)?;
    Ok(materialcraft_formats::material::write_material(&session.pkg, graph)?)
}

/// Cooks the options into a copy of the package, so the live package keeps its size, defaults and
/// bindings. `maximum_resource_bytes` bounds the source assets before the archive is built.
#[cfg(any(test, not(target_arch = "wasm32")))]
pub fn material_bytes_with_options(session: &Session, graph: usize, options: &MaterialOptions, maximum_resource_bytes: Option<u64>) -> Result<Vec<u8>> {
    crate::material_bake::check_publish(session, graph)?;
    let mut pkg = session.pkg.clone();
    {
        let g = pkg.graph_mut(graph)?;
        if let Some(size) = options.size {
            // `Graph.size` stores log2 exponents, as `graph.size` does
            let exponent = (31 - size.clamp(1, 1 << 20).leading_zeros()).clamp(MIN_SIZE_LOG2, MAX_SIZE_LOG2);
            g.size = [exponent, exponent];
        }
        // Text inputs are not portable: the material carries only the numeric and colour inputs
        g.exposed.retain(|entry| !matches!(entry.default, Value::Text(_)));
        for (id, value) in &options.values {
            if let Some(input) = g.exposed.iter_mut().find(|entry| entry.id == *id) {
                input.default = value.clone();
            }
        }
    }
    for (id, image) in &options.inputs {
        let srgb = srgb_usage(id);
        let bytes = materialcraft_formats::encode(image, "png", Depth::U16, srgb).map_err(|e| bad("material.export", e.to_string()))?;
        let resource = crate::file_cmds::push_resource(&mut pkg, &format!("{id}.png"), bytes, Some(id));
        let g = pkg.graph_mut(graph)?;
        g.input_images.insert(id.clone(), resource);
        if srgb {
            g.input_images_linear.remove(id);
        } else {
            g.input_images_linear.insert(id.clone());
        }
    }
    Ok(match maximum_resource_bytes {
        Some(limit) => materialcraft_formats::material::write_material_with_resource_limit(&pkg, graph, limit)?,
        None => materialcraft_formats::material::write_material(&pkg, graph)?,
    })
}

/// Publish with an additional budget for the combined uncompressed source assets.
pub fn material_bytes_with_resource_limit(session: &Session, graph: usize, maximum_resource_bytes: u64) -> Result<Vec<u8>> {
    crate::material_bake::check_publish(session, graph)?;
    Ok(materialcraft_formats::material::write_material_with_resource_limit(&session.pkg, graph, maximum_resource_bytes)?)
}

pub fn publish_graph(s: &Session, p: &J) -> Result<usize> {
    if ["size", "values", "inputs", "preset"].iter().any(|key| p.get(key).is_some_and(|v| !v.is_null())) {
        return Err(bad("file.exportMaterial", "pass size, values, preset and image inputs when cooking or exporting outputs"));
    }

    crate::graph_cmds::graph_index(s, p)
}

/// The graph's input nodes (one entry per identifier)
pub fn image_inputs(g: &Graph) -> Vec<ImageInput> {
    let mut out: Vec<ImageInput> = Vec::new();
    for n in g.nodes.iter().filter(|n| matches!(n.def.as_str(), "input_grayscale" | "input_color")) {
        let identifier = materialcraft_eval::identifier(n);
        if !out.iter().any(|i| i.identifier == identifier) {
            let usage = if n.params.contains_key("usage") { materialcraft_eval::output_usage(n) } else { "any" };
            out.push(ImageInput { identifier, kind: if n.def == "input_grayscale" { "gray" } else { "color" }.into(), usage: usage.into() });
        }
    }
    out
}

#[cfg(any(test, not(target_arch = "wasm32")))]
#[derive(Clone, Default)]
pub struct MaterialOptions {
    pub size: Option<u32>,
    pub values: BTreeMap<String, Value>,
    pub inputs: BTreeMap<String, craft_texture::Image>,
}

/// `values` as typed exposed input values of graph `g`
#[cfg(any(test, not(target_arch = "wasm32")))]
fn values_param(cmd: &str, g: &Graph, p: &J) -> Result<BTreeMap<String, Value>> {
    let obj = match p.get("values") {
        None | Some(J::Null) => return Ok(BTreeMap::new()),
        Some(v) => v.as_object().ok_or_else(|| bad(cmd, "`values` maps input ids to values"))?,
    };
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
    crate::graph_cmds::graph_index(s, p)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn material_request(s: &Session, cmd: &str, p: &J) -> Result<(usize, MaterialOptions)> {
    let gi = graph_param(s, p)?;
    let o = MaterialOptions { size: u64p(p, "size").map(|v| v.min(8192) as u32), values: values_param(cmd, s.pkg.graph(gi)?, p)?, ..Default::default() };
    Ok((gi, o))
}

/// Decoded bytes (four f32 channels per pixel) that all `inputs` of one request may hold together:
/// two 8192 px colour inputs. A request's peak stays near 3 GiB: this budget plus the one image being decoded.
#[cfg(not(target_arch = "wasm32"))]
pub const MAX_INPUT_DECODED_BYTES: u64 = 2 << 30;

/// Reads the `inputs` map (input id to image path) of an export request into `inputs`. The decoded
/// images must fit `MAX_INPUT_DECODED_BYTES` together. When `strict` (sandboxed callers), ids must
/// name an image input of graph `g`; trusted callers may pass any id, as before.
#[cfg(not(target_arch = "wasm32"))]
pub fn load_inputs(
    cmd: &str,
    read: impl Fn(&str) -> std::result::Result<Vec<u8>, String>,
    g: &Graph,
    p: &J,
    inputs: &mut BTreeMap<String, craft_texture::Image>,
    strict: bool,
) -> Result<()> {
    load_inputs_within(cmd, read, g, p, inputs, MAX_INPUT_DECODED_BYTES, strict)
}

#[cfg(not(target_arch = "wasm32"))]
fn load_inputs_within(
    cmd: &str,
    read: impl Fn(&str) -> std::result::Result<Vec<u8>, String>,
    g: &Graph,
    p: &J,
    inputs: &mut BTreeMap<String, craft_texture::Image>,
    budget: u64,
    strict: bool,
) -> Result<()> {
    let entries = match p.get("inputs") {
        None | Some(J::Null) => return Ok(()),
        Some(v) => v.as_object().ok_or_else(|| bad(cmd, "`inputs` maps input ids to image paths"))?,
    };
    let declared = image_inputs(g);
    let mut total = 0u64;
    for (id, file) in entries {
        if strict && !declared.iter().any(|i| i.identifier == *id) {
            let names: Vec<&str> = declared.iter().map(|i| i.identifier.as_str()).collect();
            return Err(bad(cmd, format!("no image input `{id}` in this graph (image inputs: {})", names.join(", "))));
        }
        let file = file.as_str().ok_or_else(|| bad(cmd, format!("inputs.{id} must be an image path")))?;
        let bytes = read(file).map_err(|e| bad(cmd, e))?;
        let image = materialcraft_formats::decode(&bytes, !srgb_usage(id))?;
        let decoded = image.width() as u64 * image.height() as u64 * 16;
        total =
            total.checked_add(decoded).filter(|t| *t <= budget).ok_or_else(|| bad(cmd, format!("the inputs decode to more than {budget} bytes in total")))?;
        inputs.insert(id.clone(), image);
    }
    Ok(())
}

/// `load_inputs` for a request that cooks graph `gi` of session `s`.
#[cfg(not(target_arch = "wasm32"))]
pub fn load_request_inputs(
    cmd: &str,
    read: impl Fn(&str) -> std::result::Result<Vec<u8>, String>,
    s: &Session,
    gi: usize,
    p: &J,
    o: &mut MaterialOptions,
    strict: bool,
) -> Result<()> {
    load_inputs(cmd, read, s.pkg.graph(gi)?, p, &mut o.inputs, strict)
}
#[cfg(not(target_arch = "wasm32"))]
fn export_material(s: &mut Session, p: &J) -> Result<J> {
    const CMD: &str = "file.exportMaterial";
    let path = strp(p, "path").ok_or_else(|| bad(CMD, "`path` is required"))?;
    let path = if path.to_ascii_lowercase().ends_with(&format!(".{MATERIAL_EXT}")) { path.to_string() } else { format!("{path}.{MATERIAL_EXT}") };
    let (gi, mut o) = material_request(s, CMD, p)?;
    load_request_inputs(CMD, |f| std::fs::read(f).map_err(|e| format!("{f}: {e}")), s, gi, p, &mut o, false)?;
    let bytes = material_bytes_with_options(&*s, gi, &o, None)?;
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
    let gi = publish_graph(s, p)?;
    let bytes = material_bytes(&*s, gi)?;
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
#[path = "material_cmds.test.rs"]
mod tests;

#[cfg(test)]
mod bake_tests {
    use super::*;
    use materialcraft_formats::material::read_material;

    #[test]
    fn exports_maps_inputs_and_values() {
        let mut s = crate::sample_session();
        crate::execute(&mut s, "graph.exposed.add", &json!({"id": "wear", "default": 0.5})).unwrap();
        crate::execute(&mut s, "graph.exposed.add", &json!({"id": "label", "default": {"t": "text", "v": "x"}})).unwrap();
        let values = values_param("t", s.graph().unwrap(), &json!({"values": {"wear": 0.9}})).unwrap();
        let graph = s.graph;
        let bytes = material_bytes_with_options(&s, graph, &MaterialOptions { size: Some(32), values, ..Default::default() }, None).unwrap();
        let (pkg, entry) = read_material(&bytes).unwrap();
        let graph = pkg.graph(entry).unwrap();
        assert_eq!(graph.size, [5, 5], "sizes are stored as log2 exponents");
        assert!(graph.nodes.iter().any(|node| node.def == "output" && materialcraft_eval::output_usage(node) == "height"));
        let ids: Vec<&str> = graph.exposed.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(ids, ["wear"], "text inputs are not portable");
        assert_eq!(graph.exposed[0].default, Value::Float(0.9), "the manifest carries the cooked value");
        let source = materialcraft_formats::sbs::write_sbs(&pkg).unwrap();
        assert_eq!(
            materialcraft_formats::sbs::read_sbs(&source, "source.sbs", &|_| None).unwrap().pkg.graphs[entry].input_value("wear"),
            Some(&Value::Float(0.9))
        );
        // The source package opens again, on the material's graph, with no file to save over
        crate::file_cmds::open_bytes(&mut s, &bytes, Some("x.mcmat".into())).unwrap();
        assert_eq!(s.graph().unwrap().name, pkg.graph(entry).unwrap().name);
        assert!(s.path.is_none());
    }

    #[test]
    fn load_inputs_rejects_unknown_ids_and_repeated_paths_over_budget() {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let mut s = crate::sample_session();
            crate::execute(&mut s, "graph.node.add", &json!({"def": "input_grayscale", "params": {"identifier": "curvature"}})).unwrap();
            crate::execute(&mut s, "graph.node.add", &json!({"def": "input_grayscale", "params": {"identifier": "roughness"}})).unwrap();
            let g = s.graph().unwrap();
            let png = encode(&Image::filled(4, 4, craft_texture::Kind::Gray, [0.7; 4]), "png", Depth::U16, false).unwrap();
            let read = |_: &str| Ok::<Vec<u8>, String>(png.clone());
            let one_image = 4 * 4 * 16;

            let mut inputs = BTreeMap::new();
            assert!(load_inputs_within("t", read, g, &json!({"inputs": {"nope": "a.png"}}), &mut inputs, one_image, true).is_err());
            assert!(inputs.is_empty());
            // Trusted callers may name an id the graph does not declare
            assert!(load_inputs_within("t", read, g, &json!({"inputs": {"nope": "a.png"}}), &mut BTreeMap::new(), one_image, false).is_ok());

            // Repeated paths under distinct ids each retain a decoded copy, so the budget counts them
            let mut inputs = BTreeMap::new();
            let same_path = json!({"inputs": {"curvature": "a.png", "roughness": "a.png"}});
            assert!(load_inputs_within("t", read, g, &same_path, &mut inputs, one_image, true).is_err());

            let mut inputs = BTreeMap::new();
            assert!(load_inputs_within("t", read, g, &json!({"inputs": {"curvature": "a.png"}}), &mut inputs, one_image, true).is_ok());
            assert_eq!(inputs.len(), 1);
            assert!(load_inputs_within("t", read, g, &json!({"inputs": "a.png"}), &mut BTreeMap::new(), one_image, true).is_err());
        }
    }

    #[test]
    fn binds_preview_images_to_input_nodes() {
        let mut s = crate::sample_session();
        crate::execute(&mut s, "graph.node.add", &json!({"def": "input_grayscale", "params": {"identifier": "curvature"}})).unwrap();
        let png = encode(&Image::filled(4, 4, craft_texture::Kind::Gray, [0.7; 4]), "png", Depth::U16, false).unwrap();
        let res = crate::file_cmds::add_resource(&mut s, "curv.png", png, None).unwrap();

        assert!(crate::execute(&mut s, "graph.input.bind", &json!({"identifier": "nope", "resource": res})).is_err());

        crate::execute(&mut s, "graph.input.bind", &json!({"identifier": "curvature", "resource": res, "linear": true})).unwrap();

        assert_eq!(s.graph().unwrap().input_images.get("curvature"), Some(&res));

        let bytes = materialcraft_formats::sbs::write_sbs(&s.pkg).unwrap();
        let reopened = materialcraft_formats::sbs::read_sbs(&bytes, "input.sbs", &|_| None).unwrap().pkg;

        assert_eq!(reopened.graphs[s.graph].input_images.get("curvature"), Some(&res));
        assert!(reopened.graphs[s.graph].input_images_linear.contains("curvature"));

        let graph_index = s.graph;
        let bytes = material_bytes_with_options(&s, graph_index, &MaterialOptions { size: Some(16), ..Default::default() }, None).unwrap();
        let (pkg, entry) = read_material(&bytes).unwrap();
        let graph = pkg.graph(entry).unwrap();

        assert!(graph.nodes.iter().any(|node| node.def == "input_grayscale" && materialcraft_eval::identifier(node) == "curvature"));
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

    #[test]
    fn export_material_decodes_data_inputs_as_linear() {
        #[cfg(not(target_arch = "wasm32"))]
        {
            use materialcraft_formats::decode;

            let mut s = crate::sample_session();
            let input =
                crate::execute(&mut s, "graph.node.add", &json!({"def": "input_color", "params": {"identifier": "normal"}})).unwrap()["id"].as_u64().unwrap();
            let output =
                crate::execute(&mut s, "graph.node.add", &json!({"def": "output", "params": {"usage": ComponentUsage::index("normal")}})).unwrap()["id"]
                    .as_u64()
                    .unwrap();
            crate::execute(&mut s, "graph.connect", &json!({"from": input, "to": output})).unwrap();

            let png = encode(&Image::filled(16, 16, craft_texture::Kind::Color, [128.0 / 255.0; 4]), "png", Depth::U16, false).unwrap();
            let dir = std::env::temp_dir().join(format!("materialcraft-linear-input-{}", std::process::id()));
            std::fs::create_dir_all(&dir).unwrap();
            let file = dir.join("normal.png");
            std::fs::write(&file, &png).unwrap();
            let input_path = file.to_str().unwrap().to_string();
            let output_path = dir.join("out.mcmat").to_str().unwrap().to_string();

            let result = crate::execute(&mut s, "file.exportMaterial", &json!({"path": output_path, "size": 16, "inputs": {"normal": input_path}}));

            assert!(result.is_ok());

            let bytes = std::fs::read(&output_path).unwrap();
            let _ = std::fs::remove_dir_all(&dir);

            // PNG resources are stored uncompressed: the only source resource follows its local file header
            let name = b"resources/";
            let pos = bytes.windows(name.len()).position(|w| w == name).unwrap();
            let header = pos - 30;

            assert_eq!(&bytes[header..header + 4], b"PK\x03\x04");

            let name_len = u16::from_le_bytes([bytes[header + 26], bytes[header + 27]]) as usize;
            let extra_len = u16::from_le_bytes([bytes[header + 28], bytes[header + 29]]) as usize;
            let compressed = u32::from_le_bytes([bytes[header + 18], bytes[header + 19], bytes[header + 20], bytes[header + 21]]) as usize;
            let start = header + 30 + name_len + extra_len;
            let map = &bytes[start..start + compressed];
            let decoded = decode(map, true).unwrap();

            assert!((decoded.get(0, 0, false)[0] - 128.0 / 255.0).abs() < 1e-3, "data inputs must be decoded as linear");
        }
    }
}
