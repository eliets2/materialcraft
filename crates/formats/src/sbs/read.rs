use super::{
    constant::{MAX_GRAPHS, MAX_SBS_BYTES, MAX_XML_NODES, SNIFF_BYTES},
    placeholder::report_placeholders,
    sbs_read::SbsRead,
    sbs_reader::{SbsReader, unique_name},
    warnings::Warnings,
    xml::{child_element, child_value, children_named},
};
use crate::{FormatError, Result};
use materialcraft_core::{Graph, Package};
use roxmltree::Node as XmlNode;
use std::collections::HashMap;

pub fn read_sbs(bytes: &[u8], name: &str, linked: &dyn Fn(&str) -> Option<Vec<u8>>) -> Result<SbsRead> {
    if bytes.len() > MAX_SBS_BYTES {
        return Err(FormatError::BadPackage(format!("the SBS file is over the {} MB limit", MAX_SBS_BYTES >> 20)));
    }

    let text = std::str::from_utf8(bytes).map_err(|e| FormatError::BadPackage(format!("the SBS file is not UTF-8 text: {e}")))?;
    let text = text.trim_start_matches('\u{feff}');
    let options = roxmltree::ParsingOptions { allow_dtd: false, nodes_limit: MAX_XML_NODES };
    let doc = roxmltree::Document::parse_with_options(text, options).map_err(|e| FormatError::BadPackage(format!("the SBS file is not valid XML: {e}")))?;
    let root = doc.root_element();
    if root.tag_name().name() != "package" {
        return Err(FormatError::BadPackage("not an SBS package (the root element is not <package>)".into()));
    }

    let mut warnings = Warnings::default();
    let file_stem = name.rsplit(['/', '\\']).next().unwrap_or(name);
    let file_stem = file_stem.rsplit_once('.').map_or(file_stem, |(stem, _)| stem).trim();
    let package_name = match child_value(root, "identifier").map(str::trim) {
        Some(id) if !id.is_empty() && id != "Unsaved Package" => id.to_string(),
        _ if !file_stem.is_empty() => file_stem.to_string(),
        _ => "Untitled package".to_string(),
    };

    let mut reader = SbsReader {
        text,
        warnings: &mut warnings,
        dependencies: HashMap::new(),
        graph_names: HashMap::new(),
        resource_names: HashMap::new(),
        node_count: 0,
        resource_bytes: 0,
    };
    for dependency in child_element(root, "dependencies").into_iter().flat_map(|d| children_named(d, "dependency")) {
        if let (Some(uid), Some(file)) = (child_value(dependency, "uid"), child_value(dependency, "filename")) {
            reader.dependencies.insert(uid.to_string(), file.to_string());
        }
    }

    // Load resources before resolving graph references
    let content = child_element(root, "content");
    let mut pkg = Package::new(&package_name);
    let mut graph_elements: Vec<(String, XmlNode)> = Vec::new();
    if let Some(content) = content {
        reader.collect_items(content, "", &mut pkg, &mut graph_elements, linked);
    }

    // Name every graph first so instances can refer to graphs defined later in the file
    let mut names: Vec<String> = Vec::new();
    for (path, graph) in graph_elements.iter().take(MAX_GRAPHS) {
        let identifier = child_value(*graph, "identifier").unwrap_or("").trim();
        let unique = unique_name(&names, if identifier.is_empty() { "Graph" } else { identifier });
        reader.graph_names.insert(path.clone(), unique.clone());
        names.push(unique);
    }
    if graph_elements.len() > MAX_GRAPHS {
        reader.warnings.add(format!("only the first {MAX_GRAPHS} graphs were read"));
    }

    for ((_, graph), name) in graph_elements.iter().zip(names) {
        let graph = reader.read_graph(*graph, name);
        pkg.graphs.push(graph);
    }
    if pkg.graphs.is_empty() {
        reader.warnings.add("the package has no graphs MaterialCraft can open; an empty graph was added");
        pkg.graphs.push(Graph::new("Graph"));
    }

    report_placeholders(&pkg, reader.warnings);
    Ok(SbsRead { pkg, warnings: warnings.finish() })
}
pub fn is_sbs(bytes: &[u8]) -> bool {
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(SNIFF_BYTES)]);
    let mut rest = head.trim_start_matches('\u{feff}');

    // Skip the XML declaration and comments before the root element
    loop {
        rest = rest.trim_start();
        let (skip_open, skip_close) = if rest.starts_with("<?") {
            ("<?", "?>")
        } else if rest.starts_with("<!--") {
            ("<!--", "-->")
        } else {
            break;
        };

        match rest[skip_open.len()..].find(skip_close) {
            Some(end) => rest = &rest[skip_open.len() + end + skip_close.len()..],
            None => return false,
        }
    }

    rest.strip_prefix("<package").is_some_and(|after| after.starts_with(|c: char| c == '>' || c == '/' || c.is_whitespace()))
}
