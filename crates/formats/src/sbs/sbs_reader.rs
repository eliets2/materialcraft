use super::base_parameters::read_base_params;
use super::component_usage;
use super::connection::connect_all;
use super::constant::{MAX_NODES, MAX_RESOURCES};
use super::constant::{NODE_HALF_SIZE, SELF_DEPENDENCY};
use super::embedded::decode_embedded;
use super::filter_map::{BASE_PARAMS, FilterMap, HANDLED_PARAMS, filter_by_sbs_name};
use super::function_graph;
use super::input_bridge::InputBridge;
use super::placeholder::{add_placeholder_input, fill_placeholder, placeholder_outputs};
use super::read_node::ReadNode;
use super::sbs_value::{SbsValue, exposed_default, sbs_constant, to_natural_value, to_param_value};
use super::type_code;
use super::warnings::Warnings;
use super::xml::{child_element, child_value, children_named, parse_floats};
use materialcraft_core::{
    ComponentUsage, Connection, ExposedParameter, Graph, MAX_SIZE_LOG2, MIN_SIZE_LOG2, MISSING_NODE, Node, NodeId, Package, Resource, ResourceBytes, Value,
};
use materialcraft_nodes::find;
use roxmltree::Node as XmlNode;
use std::collections::HashMap;

pub(super) struct SbsReader<'text, 'warnings> {
    pub(super) text: &'text str,
    pub(super) warnings: &'warnings mut Warnings,
    pub(super) dependencies: HashMap<String, String>,
    pub(super) graph_names: HashMap<String, String>,
    pub(super) resource_names: HashMap<String, String>,
    pub(super) node_count: usize,
    pub(super) resource_bytes: usize,
}

impl<'text> SbsReader<'text, '_> {
    pub(super) fn collect_items<'a>(
        &mut self,
        content: XmlNode<'a, 'text>,
        folder: &str,
        pkg: &mut Package,
        graphs: &mut Vec<(String, XmlNode<'a, 'text>)>,
        linked: &dyn Fn(&str) -> Option<Vec<u8>>,
    ) {
        let mut pending = vec![(folder.to_string(), content)];
        while let Some((folder, content)) = pending.pop() {
            for item in content.children().filter(|child| child.is_element()) {
                let identifier = child_value(item, "identifier").unwrap_or("").trim().to_string();
                let path = if folder.is_empty() { identifier.clone() } else { format!("{folder}/{identifier}") };

                match item.tag_name().name() {
                    "graph" | "function" => graphs.push((path, item)),
                    "group" => {
                        if let Some(group_content) = child_element(item, "content") {
                            pending.push((path, group_content));
                        }
                    }
                    "resource" if pkg.resources.len() >= MAX_RESOURCES => self.warnings.add(format!("only the first {MAX_RESOURCES} resources were read")),
                    "resource" => self.read_resource(item, &identifier, &path, pkg, linked),
                    "mdl_graph" | "mdlGraph" => self.warnings.add(format!("MDL graph `{path}` is not supported")),
                    other => self.warnings.add(format!("package item <{other}> `{path}` is not supported")),
                }
            }
        }
    }

    pub(super) fn read_resource(&mut self, item: XmlNode, identifier: &str, path: &str, pkg: &mut Package, linked: &dyn Fn(&str) -> Option<Vec<u8>>) {
        let remaining = super::constant::MAX_PACKAGE_BYTES.saturating_sub(self.resource_bytes).min(super::constant::MAX_EMBEDDED_BYTES);
        let kind = child_value(item, "type").unwrap_or("bitmap");
        let format = child_value(item, "format").unwrap_or("png").trim().to_ascii_lowercase();
        let source = child_element(item, "source");
        let mut bytes: Option<Vec<u8>> = None;
        let mut why_missing = String::new();

        if let Some(embedded) = source.and_then(|s| child_element(s, "binembedded")) {
            match decode_embedded(child_value(embedded, "strdata").unwrap_or(""), remaining) {
                Ok(data) => bytes = Some(data),
                Err(e) => why_missing = format!("its embedded data is damaged ({e})"),
            }
        }

        if bytes.is_none() {
            let external_copy = source.and_then(|s| child_element(s, "externalcopy")).and_then(|e| child_value(e, "filename"));
            for file in [external_copy, child_value(item, "filepath")].into_iter().flatten().filter(|p| !p.trim().is_empty()) {
                if file.contains("://") {
                    why_missing = format!("it links to `{file}`, which is not available");
                    continue;
                }
                match linked(file) {
                    Some(data) => {
                        bytes = Some(data);
                        break;
                    }
                    None => why_missing = format!("its file `{file}` was not found next to the package"),
                }
            }
        }

        let taken: Vec<String> = pkg.resources.iter().map(|r| r.name.clone()).collect();
        let name = unique_name(&taken, if identifier.is_empty() { "resource" } else { identifier });
        self.resource_names.insert(path.to_string(), name.clone());
        match bytes {
            Some(data) if data.len() > remaining => self.warnings.add(format!("resource `{path}` exceeds the remaining {remaining}-byte package budget")),
            Some(data) => {
                self.resource_bytes += data.len();
                let ext = if kind == "svg" { "svg".to_string() } else { format };

                pkg.resources.push(Resource::new(name.clone(), crate::safe_file_name(&format!("{name}.{ext}")), ResourceBytes::new(data)));
            }
            None => {
                let why = if why_missing.is_empty() { "it has no data".to_string() } else { why_missing };

                self.warnings.add(format!("resource `{path}` could not be loaded: {why}"));
            }
        }
    }
    pub(super) fn resolve_reference(&self, url: &str) -> (String, Option<String>) {
        let url = url.trim();
        let rest = url.strip_prefix("pkg:///").or_else(|| url.strip_prefix("pkg://")).unwrap_or(url);
        let (path, query) = rest.split_once('?').unwrap_or((rest, ""));
        let dependency = query.split('&').find_map(|kv| kv.strip_prefix("dependency=")).and_then(|uid| self.dependencies.get(uid).cloned());
        (path.trim_matches('/').to_string(), dependency)
    }

    pub(super) fn read_graph(&mut self, element: XmlNode<'_, 'text>, name: String) -> Graph {
        let mut graph = Graph::new(&name);
        graph.seed = read_base_params(child_element(element, "baseParameters")).seed;

        // The default parent size option ("11x11", log2 sizes)
        for option in child_element(element, "options").into_iter().flat_map(|o| children_named(o, "option")) {
            super::preview_binding::read(option, &mut graph);
            if child_value(option, "name") == Some("defaultParentSize")
                && let Some((width, height)) = child_value(option, "value").and_then(|v| v.split_once('x'))
            {
                let log2 = |s: &str| s.trim().parse::<i64>().unwrap_or(10).clamp(i64::from(MIN_SIZE_LOG2), i64::from(MAX_SIZE_LOG2)) as u32;
                graph.size = [log2(width), log2(height)];
            }
        }

        if let Some(attributes) = child_element(element, "attributes") {
            let text = |tag: &str| child_value(attributes, tag).unwrap_or("").to_string();
            graph.attributes.category = text("category");
            graph.attributes.description = text("description");
            graph.attributes.author = text("author");
            graph.attributes.tags = text("tags");
        }

        if element.has_tag_name("function") {
            self.read_graph_inputs(element, &mut graph);
            let dynamic = child_element(element, "paramValue").and_then(|value| child_element(value, "dynamicValue"));
            let function = dynamic.ok_or_else(|| "missing function body".to_string()).and_then(super::function_graph::read);
            graph.function = Some(match function {
                Ok(function) => function,
                Err(error) => {
                    self.warnings.add(format!("function graph `{name}`: {error}"));
                    materialcraft_core::FunctionGraph::default()
                }
            });
            if let Some(dynamic) = dynamic {
                super::canvas::read_canvas_items(dynamic, &mut graph, self.warnings);
            }
            return graph;
        }

        let image_inputs = self.read_graph_inputs(element, &mut graph);
        let outputs = read_graph_outputs(element);

        let mut read_nodes: HashMap<String, ReadNode> = HashMap::new();
        let comp_nodes: Vec<XmlNode> = child_element(element, "compNodes").into_iter().flat_map(|c| children_named(c, "compNode")).collect();
        for comp_node in &comp_nodes {
            if self.node_count >= MAX_NODES {
                self.warnings.add(format!("only the first {MAX_NODES} nodes were read"));
                break;
            }

            let Some(uid) = child_value(*comp_node, "uid").map(str::to_string) else { continue };

            if read_nodes.contains_key(&uid) {
                self.warnings.add(format!("graph `{name}`: duplicate node uid {uid} skipped"));
                continue;
            }

            self.node_count += 1;
            let mut node = self.read_node(&mut graph, *comp_node, &image_inputs, &outputs);
            for input in child_element(*comp_node, "inputValues").into_iter().flat_map(|inputs| children_named(inputs, "compInputValue")).take(64) {
                let identifier = child_value(input, "identifier").unwrap_or("");
                let value_type = child_value(input, "type").and_then(|code| code.parse().ok()).and_then(materialcraft_core::ValueType::from_code);
                if let Some(value_type) = value_type {
                    if let Some(document_node) = graph.nodes.get_mut(node.index) {
                        document_node.input_values.insert(identifier.to_string(), value_type);
                    }
                    node.inputs.insert(identifier.to_string(), identifier.to_string());
                }
            }
            read_nodes.insert(uid, node);
        }

        let wires = self.read_wires(&mut graph, &comp_nodes, &read_nodes);
        connect_all(&mut graph, wires, self.warnings);
        super::canvas::read_canvas_items(element, &mut graph, self.warnings);
        graph
    }
    pub(super) fn read_graph_inputs(&mut self, element: XmlNode, graph: &mut Graph) -> HashMap<String, InputBridge> {
        let mut image_inputs = HashMap::new();
        for input in child_element(element, "paraminputs").into_iter().flat_map(|p| children_named(p, "paraminput")) {
            let id = child_value(input, "identifier").unwrap_or("input").to_string();
            let code = child_value(input, "type").and_then(|t| t.trim().parse::<i64>().ok()).unwrap_or(type_code::FLOAT1);
            if code == type_code::COLOR_IMAGE || code == type_code::GRAY_IMAGE {
                let default = child_element(input, "defaultValue")
                    .and_then(sbs_constant)
                    .and_then(|value| to_param_value(&Value::Color([0.5, 0.5, 0.5, 1.0]), &value, 0))
                    .unwrap_or(Value::Color([0.5, 0.5, 0.5, 1.0]));
                image_inputs.insert(
                    child_value(input, "uid").unwrap_or("").to_string(),
                    InputBridge { identifier: id, type_code: code as u32, usage: component_usage::read(input), default },
                );
                continue;
            }
            if graph.exposed.iter().any(|e| e.id == id) {
                continue;
            }

            let default = exposed_default(code, child_element(input, "defaultValue").and_then(sbs_constant).as_ref());
            if child_value(input, "isConnectable") == Some("1") {
                image_inputs.insert(
                    child_value(input, "uid").unwrap_or("").to_string(),
                    InputBridge { identifier: id, type_code: code as u32, usage: component_usage::read(input), default },
                );
                continue;
            }
            let label = child_element(input, "attributes").and_then(|a| child_value(a, "label")).filter(|l| !l.is_empty()).unwrap_or(&id).to_string();
            let (mut min, mut max) = (0.0, 1.0);
            let widget_options = child_element(input, "defaultWidget").and_then(|w| child_element(w, "options"));
            for option in widget_options.into_iter().flat_map(|o| children_named(o, "option")) {
                let value =
                    child_value(option, "value").and_then(|v| v.split_whitespace().next()).and_then(|v| v.parse::<f32>().ok()).filter(|v| v.is_finite());
                match (child_value(option, "name"), value) {
                    (Some("min"), Some(v)) => min = v,
                    (Some("max"), Some(v)) => max = v,
                    _ => {}
                }
            }

            let group = child_value(input, "group").unwrap_or("").to_string();
            graph.exposed.push(ExposedParameter { id, label, group, default, min, max, options: Vec::new() });
        }
        image_inputs
    }
    pub(super) fn read_wires(&mut self, graph: &mut Graph, comp_nodes: &[XmlNode], read_nodes: &HashMap<String, ReadNode>) -> Vec<Connection> {
        let name = graph.name.clone();
        let mut wires = Vec::new();

        for comp_node in comp_nodes {
            let Some(to) = child_value(*comp_node, "uid").and_then(|u| read_nodes.get(u)) else { continue };

            for connection in child_element(*comp_node, "connections").into_iter().flat_map(|c| children_named(c, "connection")) {
                let sbs_input = child_value(connection, "identifier").unwrap_or("");
                let Some(from) = child_value(connection, "connRef").and_then(|u| read_nodes.get(u)) else {
                    self.warnings.add(format!("graph `{name}`: a wire from a node that is not in the file was dropped"));
                    continue;
                };

                let output_uid = child_value(connection, "connRefOutput").unwrap_or("");
                let from_port = from.outputs.iter().find(|(uid, _)| uid == output_uid).or(from.outputs.first()).map(|(_, port)| port.clone());
                let Some(from_port) = from_port else {
                    self.warnings.add(format!("graph `{name}`: a wire from a node with no outputs was dropped"));
                    continue;
                };

                let port =
                    to.inputs.get(sbs_input).cloned().or_else(|| graph.nodes.get(to.index).and_then(|node| super::image_input::read(&node.def, sbs_input)));
                let to_port = match port {
                    Some(port) => port,
                    None if to.open_inputs => {
                        if let Some(node) = graph.nodes.get_mut(to.index) {
                            add_placeholder_input(node, sbs_input);
                        }
                        sbs_input.to_string()
                    }
                    None => {
                        let def = graph.nodes.get(to.index).map_or("", |n| n.def.as_str());
                        self.warnings.add(format!("graph `{name}`: `{def}` has no input `{sbs_input}`; its wire was dropped"));
                        continue;
                    }
                };

                wires.push(Connection { from: from.id, from_port, to: to.id, to_port });
            }
        }
        wires
    }

    pub(super) fn read_node(
        &mut self,
        graph: &mut Graph,
        comp_node: XmlNode<'_, 'text>,
        image_inputs: &HashMap<String, InputBridge>,
        outputs: &HashMap<String, (String, u32)>,
    ) -> ReadNode {
        let gpos = child_element(comp_node, "GUILayout").and_then(|l| child_value(l, "gpos")).map(parse_floats).unwrap_or_default();
        let pos = [gpos.first().copied().unwrap_or(0.0) - NODE_HALF_SIZE[0], gpos.get(1).copied().unwrap_or(0.0) - NODE_HALF_SIZE[1]];
        let output_uids: Vec<(String, bool)> = child_element(comp_node, "compOutputs")
            .into_iter()
            .flat_map(|o| children_named(o, "compOutput"))
            .map(|o| (child_value(o, "uid").unwrap_or("").to_string(), child_value(o, "comptype") == Some("2")))
            .collect();
        let all_outputs_to = |port: &str| output_uids.iter().map(|(uid, _)| (uid.clone(), port.to_string())).collect::<Vec<_>>();
        let text = self.text;

        let Some(implementation) = child_element(comp_node, "compImplementation").and_then(|i| i.children().find(|c| c.is_element())) else {
            let (id, index, node) = push_node(graph, MISSING_NODE, pos);
            if let Some(node) = node {
                fill_placeholder(node, "other", "empty node", "", None, &output_uids, None);
            }
            return ReadNode { id, index, inputs: HashMap::new(), outputs: Vec::new(), open_inputs: true };
        };

        let params = child_element(implementation, "parameters");
        match implementation.tag_name().name() {
            "compInputBridge" => {
                let entry = child_value(implementation, "entry").unwrap_or("");
                let input = image_inputs.get(entry);
                let code = input.map_or(1, |input| input.type_code);
                let definition = match code {
                    1 => "input_color",
                    2 => "input_grayscale",
                    _ => "input_value",
                };

                let (id, index, node) = push_node(graph, definition, pos);
                if let Some(node) = node {
                    let identifier = input.map_or("input", |input| input.identifier.as_str());
                    node.params.insert("identifier".into(), Value::Text(identifier.to_string()));
                    node.params.insert("usage".into(), Value::Enum(input.map_or(ComponentUsage::ANY_INDEX, |input| input.usage)));
                    if let Some(input) = input {
                        node.params.insert("default".into(), input.default.clone());
                    }
                    node.base = read_base_params(params);
                }
                ReadNode { id, index, inputs: HashMap::new(), outputs: all_outputs_to("output"), open_inputs: false }
            }
            "compOutputBridge" => {
                let output = child_value(implementation, "output").unwrap_or("");
                let (identifier, usage) = outputs.get(output).cloned().unwrap_or_else(|| ("output".into(), ComponentUsage::ANY_INDEX));
                let (id, index, node) = push_node(graph, "output", pos);
                if let Some(node) = node {
                    node.params.insert("identifier".into(), Value::Text(identifier));
                    node.params.insert("usage".into(), Value::Enum(usage));
                }
                let inputs = HashMap::from([("inputNodeOutput".to_string(), "input".to_string())]);
                ReadNode { id, index, inputs, outputs: Vec::new(), open_inputs: false }
            }
            "compFilter" => {
                let filter_name = child_value(implementation, "filter").unwrap_or("");
                let graph_name = graph.name.clone();
                match filter_by_sbs_name(filter_name).filter(|f| find(f.def).is_some()) {
                    Some(filter) => {
                        let (id, index, node) = push_node(graph, filter.def, pos);
                        if let Some(node) = node {
                            self.read_filter_params(node, filter, params, &graph_name);
                            super::parameter_array::read(node, implementation);
                            node.base = read_base_params(params);
                            if filter.sbs == "fxmaps"
                                && let Some(pattern) = child_element(implementation, "paramsGraphs").and_then(|graphs| child_element(graphs, "paramsGraph"))
                            {
                                match super::pattern_graph::read(pattern) {
                                    Ok(pattern) => {
                                        node.params.insert("pattern".into(), Value::PatternGraph(Box::new(pattern)));
                                    }
                                    Err(error) => self.warnings.add(format!("pattern graph: {error}")),
                                }
                            }
                        }
                        let inputs = filter.inputs.iter().map(|(sbs, port)| (sbs.to_string(), port.to_string())).collect();
                        ReadNode { id, index, inputs, outputs: all_outputs_to("output"), open_inputs: false }
                    }
                    None => {
                        let (id, index, node) = push_node(graph, MISSING_NODE, pos);
                        if let Some(node) = node {
                            fill_placeholder(node, "filter", filter_name, "", params.map(|p| &text[p.range()]), &output_uids, None);
                        }
                        ReadNode { id, index, inputs: HashMap::new(), outputs: placeholder_outputs(&output_uids, None), open_inputs: true }
                    }
                }
            }
            "compInstance" => self.read_instance(graph, implementation, pos, &output_uids),
            other => {
                let (id, index, node) = push_node(graph, MISSING_NODE, pos);
                if let Some(node) = node {
                    fill_placeholder(node, "other", other, "", None, &output_uids, None);
                }
                ReadNode { id, index, inputs: HashMap::new(), outputs: placeholder_outputs(&output_uids, None), open_inputs: true }
            }
        }
    }
    pub(super) fn read_instance(&mut self, graph: &mut Graph, implementation: XmlNode<'_, 'text>, pos: [f32; 2], output_uids: &[(String, bool)]) -> ReadNode {
        let (path, dependency) = self.resolve_reference(child_value(implementation, "path").unwrap_or(""));
        let params = child_element(implementation, "parameters");
        let bridgings: Vec<(String, String)> = child_element(implementation, "outputBridgings")
            .into_iter()
            .flat_map(|b| children_named(b, "outputBridging"))
            .map(|b| (child_value(b, "uid").unwrap_or("").to_string(), child_value(b, "identifier").unwrap_or("output").to_string()))
            .collect();
        let instance_params =
            || params.into_iter().flat_map(|p| children_named(p, "parameter")).filter(|p| !BASE_PARAMS.contains(&child_value(*p, "name").unwrap_or("")));

        let own_graph = dependency.as_deref().is_none_or(|d| d == SELF_DEPENDENCY).then(|| self.graph_names.get(&path).cloned()).flatten();
        if let Some(sub_graph) = own_graph {
            let (id, index, node) = push_node(graph, "graph_instance", pos);
            if let Some(node) = node {
                node.params.insert("graph".into(), Value::Text(sub_graph));
                for param in instance_params() {
                    if let Some(value) = child_element(param, "paramValue").and_then(sbs_constant) {
                        node.params.insert(child_value(param, "name").unwrap_or("").to_string(), to_natural_value(&value));
                    }
                }
                node.base = read_base_params(params);
            }
            return ReadNode { id, index, inputs: HashMap::new(), outputs: bridgings, open_inputs: true };
        }

        let short_name = path.rsplit('/').next().unwrap_or(&path).to_string();
        let (id, index, node) = push_node(graph, MISSING_NODE, pos);
        if let Some(node) = node {
            let raw_params = params.map(|parameters| &self.text[parameters.range()]);
            fill_placeholder(node, "instance", &short_name, dependency.as_deref().unwrap_or(""), raw_params, output_uids, Some(&bridgings));
            node.params.insert("sbs_path".into(), Value::Text(path));
        }

        ReadNode { id, index, inputs: HashMap::new(), outputs: placeholder_outputs(output_uids, Some(&bridgings)), open_inputs: true }
    }
    pub(super) fn set_param(&mut self, node: &mut Node, id: &str, like: &Value, options: usize, param: XmlNode, graph_name: &str) {
        let Some(param_value) = child_element(param, "paramValue") else { return };

        if let Some(value) = sbs_constant(param_value) {
            match to_param_value(like, &value, options) {
                Some(value) => {
                    node.params.insert(id.to_string(), value);
                }
                None => self.warnings.add(format!("`{}`: parameter `{id}` has a value type MaterialCraft can't read yet; kept the default", node.def)),
            }
            return;
        }

        let Some(dynamic) = child_element(param_value, "dynamicValue") else { return };

        match function_graph::read(dynamic) {
            Ok(graph) if matches!(like, Value::Function(_)) => {
                node.params.insert(id.to_string(), Value::Function(Box::new(graph)));
            }
            Ok(graph) => {
                node.functions.insert(id.to_string(), graph);
            }
            Err(error) => self.warnings.add(format!("graph `{graph_name}`: {} parameter `{id}`: {error}", node.def)),
        }
    }

    pub(super) fn read_filter_params(&mut self, node: &mut Node, filter: &FilterMap, params: Option<XmlNode>, graph_name: &str) {
        let Some(def) = find(filter.def) else { return };

        let filter_params: Vec<XmlNode> =
            params.into_iter().flat_map(|p| children_named(p, "parameter")).filter(|p| !BASE_PARAMS.contains(&child_value(*p, "name").unwrap_or(""))).collect();
        for param in &filter_params {
            let param_name = child_value(*param, "name").unwrap_or("");
            let value = child_element(*param, "paramValue").and_then(sbs_constant);

            if let Some(&(_, mc_name)) = filter.params.iter().find(|(sbs, _)| *sbs == param_name) {
                let Some(param_def) = def.param(mc_name) else { continue };

                self.set_param(node, mc_name, &param_def.default, param_def.options.len(), *param, graph_name);
                continue;
            }

            match (filter.sbs, param_name, value) {
                ("bitmap", "bitmapresourcepath", Some(SbsValue::Text(url))) | ("svg", "vectorresourcepath", Some(SbsValue::Text(url))) => {
                    let (path, _) = self.resolve_reference(&url);
                    match self.resource_names.get(&path) {
                        Some(name) => {
                            node.params.insert("resource".into(), Value::Text(name.clone()));
                        }
                        None => self.warnings.add(format!("graph `{graph_name}`: Node uses resource `{path}`, which is not in the package")),
                    }
                }
                ("grayscaleconversion", "channelsweights", Some(weights)) => {
                    for (i, channel) in ["red", "green", "blue", "alpha"].into_iter().enumerate() {
                        node.params.insert(channel.into(), Value::Float(weights.float(i)));
                    }
                }
                _ if HANDLED_PARAMS.contains(&(filter.sbs, param_name)) => {}
                _ => self.warnings.add(format!("`{}`: SBS parameter `{param_name}` is not supported yet and was ignored", filter.def)),
            }
        }
    }
}
pub(super) fn unique_name(taken: &[String], base: &str) -> String {
    let free = |name: &str| !taken.iter().any(|t| t == name);
    if free(base) {
        return base.to_string();
    }
    (1..).map(|i| format!("{base}_{i}")).find(|name| free(name)).unwrap_or_else(|| base.to_string())
}
pub(super) fn push_node<'g>(graph: &'g mut Graph, def: &str, pos: [f32; 2]) -> (NodeId, usize, Option<&'g mut Node>) {
    let id = graph.add_node(def, pos);
    let index = graph.nodes.len() - 1;
    (id, index, graph.nodes.last_mut())
}
pub(super) fn read_graph_outputs(element: XmlNode) -> HashMap<String, (String, u32)> {
    let mut outputs = HashMap::new();
    for output in child_element(element, "graphOutputs").into_iter().flat_map(|o| children_named(o, "graphoutput")) {
        let identifier = child_value(output, "identifier").unwrap_or("output").to_string();
        let usage = component_usage::read(output);
        outputs.insert(child_value(output, "uid").unwrap_or("").to_string(), (identifier, usage));
    }
    outputs
}
