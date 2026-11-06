use super::base_parameters::base_params_xml;
use super::canvas::write_canvas_items;
use super::component_usage;
use super::constant::NODE_HALF_SIZE;
use super::filter_map::filter_by_def;
use super::function_graph;
use super::node_identifiers::NodeIdentifiers;
use super::node_parameter::{param_xml, special_filter_params_xml, text_param};
use super::output::output_type;
use super::sbs_value::{constant_xml, exposed_type_code};
use super::type_code;
use super::xml::{format_float, is_parameters_element, xml_escape};
use crate::{FormatError, Result};
use materialcraft_core::{ComponentUsage, Connection, Graph, MAX_SIZE_LOG2, MIN_SIZE_LOG2, MISSING_NODE, Node, NodeId, Package, Value, missing_ports};
use materialcraft_nodes::find;
use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;

pub(super) struct SbsWriter {
    pub(super) last_uid: u64,
    pub(super) external_packages: BTreeMap<String, u64>,
}

impl SbsWriter {
    pub(super) fn next_uid(&mut self) -> u64 {
        self.last_uid = self.last_uid.saturating_add(1);
        self.last_uid
    }

    pub(super) fn external_package_uid(&mut self, file: &str) -> u64 {
        if let Some(uid) = self.external_packages.get(file) {
            return *uid;
        }

        let uid = self.next_uid();
        self.external_packages.insert(file.to_string(), uid);
        uid
    }
    pub(super) fn output_ports(pkg: &Package, graph: &Graph, node: &Node) -> Vec<(String, u32)> {
        match node.def.as_str() {
            "output" => Vec::new(),
            "graph_instance" => {
                let sub_graph = match node.params.get("graph") {
                    Some(Value::Text(name)) => pkg.graph_by_name(name).map(|(_, g)| g),
                    _ => None,
                };

                let outputs = sub_graph.into_iter().flat_map(|g| g.nodes.iter().filter(|o| o.def == "output"));
                outputs.map(|o| (text_param(o, "identifier", "output"), 1)).collect()
            }
            def if def == MISSING_NODE => missing_ports(node).1.into_iter().map(|(id, gray)| (id, if gray { 2 } else { 1 })).collect(),
            "value_processor" | "input_value" | "dot" => {
                let code = output_type(graph, node.id, "output");
                vec![("output".into(), code)]
            }
            def => match find(def) {
                Some(def) => def.outputs.iter().map(|o| (o.id.to_string(), output_type(graph, node.id, o.id))).collect(),
                None => vec![("output".into(), 1)],
            },
        }
    }

    pub(super) fn write_graph(&mut self, out: &mut String, pkg: &Package, graph: &Graph, self_uid: u64) -> Result<()> {
        let _ = write!(out, "<graph><identifier v=\"{}\"/><uid v=\"{}\"/><graphtype v=\"material\"/>", xml_escape(&graph.name), self.next_uid());
        let attributes = &graph.attributes;
        if !(attributes.category.is_empty() && attributes.description.is_empty() && attributes.author.is_empty() && attributes.tags.is_empty()) {
            let _ = write!(
                out,
                "<attributes><category v=\"{}\"/><description v=\"{}\"/><author v=\"{}\"/><tags v=\"{}\"/></attributes>",
                xml_escape(&attributes.category),
                xml_escape(&attributes.description),
                xml_escape(&attributes.author),
                xml_escape(&attributes.tags)
            );
        }

        let mut node_uids: HashMap<NodeId, u64> = HashMap::new();
        let mut node_outputs: HashMap<NodeId, WrittenOutputs> = HashMap::new();
        for node in &graph.nodes {
            node_uids.insert(node.id, self.next_uid());
            let outputs = Self::output_ports(pkg, graph, node).into_iter().map(|(port, kind)| (port, kind, self.next_uid())).collect();
            node_outputs.insert(node.id, outputs);
        }

        let input_entries = self.write_graph_inputs(out, graph);
        let output_uids = self.write_graph_outputs(out, graph);

        let mut wires_into: HashMap<NodeId, Vec<&Connection>> = HashMap::new();
        for wire in &graph.connections {
            wires_into.entry(wire.to).or_default().push(wire);
        }
        out.push_str("<compNodes>");
        for node in &graph.nodes {
            let uid = node_uids.get(&node.id).copied().unwrap_or(0);
            let (x, y) = (node.pos[0] + NODE_HALF_SIZE[0], node.pos[1] + NODE_HALF_SIZE[1]);
            let _ = write!(out, "<compNode><uid v=\"{uid}\"/><GUILayout><gpos v=\"{} {} 0\"/></GUILayout>", format_float(x), format_float(y));

            if !node.input_values.is_empty() {
                out.push_str("<inputValues>");
                for (identifier, value) in &node.input_values {
                    let code = *value as u32;
                    let _ = write!(out, "<compInputValue><identifier v=\"{}\"/><type v=\"{code}\"/></compInputValue>", xml_escape(identifier));
                }
                out.push_str("</inputValues>");
            }

            let outputs = node_outputs.get(&node.id).map_or(&[][..], Vec::as_slice);
            if !outputs.is_empty() {
                out.push_str("<compOutputs>");
                for (_, kind, output_uid) in outputs {
                    let _ = write!(out, "<compOutput><uid v=\"{output_uid}\"/><comptype v=\"{kind}\"/></compOutput>");
                }
                out.push_str("</compOutputs>");
            }

            let filter = filter_by_def(&node.def);
            let wires = wires_into.get(&node.id).map_or(&[][..], Vec::as_slice);
            if !wires.is_empty() {
                out.push_str("<connections>");
                for wire in wires {
                    let Some(from_uid) = node_uids.get(&wire.from) else { continue };

                    let Some((_, _, output_uid)) = node_outputs.get(&wire.from).and_then(|o| o.iter().find(|(port, _, _)| *port == wire.from_port)) else {
                        continue;
                    };

                    let identifier = if node.def == "output" {
                        "inputNodeOutput".to_string()
                    } else {
                        let mapped = filter.and_then(|f| f.inputs.iter().find(|(_, port)| *port == wire.to_port));
                        mapped
                            .map(|(sbs, _)| sbs.to_string())
                            .or_else(|| super::image_input::write(&node.def, &wire.to_port))
                            .unwrap_or_else(|| wire.to_port.clone())
                    };

                    let _ = write!(
                        out,
                        "<connection><identifier v=\"{}\"/><connRef v=\"{from_uid}\"/><connRefOutput v=\"{output_uid}\"/></connection>",
                        xml_escape(&identifier)
                    );
                }
                out.push_str("</connections>");
            }

            out.push_str("<compImplementation>");
            let ids = NodeIdentifiers { input_entry: input_entries.get(&node.id).copied(), graph_output: output_uids.get(&node.id).copied(), self_uid };
            self.write_implementation(out, node, outputs, ids)?;
            out.push_str("</compImplementation></compNode>");
        }
        out.push_str("</compNodes><baseParameters>");
        out.push_str(&param_xml("randomseed", &Value::Int(graph.seed)).unwrap_or_default());
        out.push_str("</baseParameters>");

        write_canvas_items(out, graph, self);
        let [width, height] = graph.size.map(|s| s.clamp(MIN_SIZE_LOG2, MAX_SIZE_LOG2));
        let _ = write!(out, "<options><option><name v=\"defaultParentSize\"/><value v=\"{width}x{height}\"/></option>");
        super::preview_binding::write(out, graph);
        out.push_str("</options><root><rootOutputs>");
        for uid in graph.nodes.iter().filter_map(|n| output_uids.get(&n.id)) {
            let _ = write!(out, "<rootOutput><output v=\"{uid}\"/><format v=\"0\"/><usertag v=\"\"/></rootOutput>");
        }
        out.push_str("</rootOutputs></root></graph>");
        Ok(())
    }
    pub(super) fn write_graph_inputs(&mut self, out: &mut String, graph: &Graph) -> HashMap<NodeId, u64> {
        let mut entries = HashMap::new();
        out.push_str("<paraminputs>");

        for node in graph.nodes.iter().filter(|n| matches!(n.def.as_str(), "input_color" | "input_grayscale" | "input_value")) {
            let uid = self.next_uid();
            entries.insert(node.id, uid);
            let fallback = if node.def == "input_value" { Value::Float(0.0) } else { Value::Color([0.5, 0.5, 0.5, 1.0]) };

            let value = node.params.get("default").unwrap_or(&fallback);
            let default = constant_xml(value).unwrap_or_default();
            let code = match node.def.as_str() {
                "input_value" => exposed_type_code(value),
                "input_grayscale" => type_code::GRAY_IMAGE,
                _ => type_code::COLOR_IMAGE,
            };

            let _ = write!(
                out,
                "<paraminput><identifier v=\"{}\"/><uid v=\"{uid}\"/><isConnectable v=\"1\"/><type v=\"{code}\"/><defaultValue>{default}</defaultValue><defaultWidget><name v=\"\"/><options/></defaultWidget>",
                xml_escape(&text_param(node, "identifier", "input"))
            );
            component_usage::write(out, node, ComponentUsage::ANY_INDEX);
            out.push_str("</paraminput>");
        }

        for exposed in &graph.exposed {
            let default = constant_xml(graph.inputs.get(&exposed.id).unwrap_or(&exposed.default)).unwrap_or_else(|| "<constantValueFloat1 v=\"0\"/>".into());
            let group = if exposed.group.is_empty() { String::new() } else { format!("<group v=\"{}\"/>", xml_escape(&exposed.group)) };

            let _ = write!(
                out,
                "<paraminput><identifier v=\"{}\"/><uid v=\"{}\"/><attributes><label v=\"{}\"/></attributes><type v=\"{}\"/><defaultValue>{default}</defaultValue><defaultWidget><name v=\"slider\"/><options><option><name v=\"min\"/><value v=\"{}\"/></option><option><name v=\"max\"/><value v=\"{}\"/></option></options></defaultWidget>{group}</paraminput>",
                xml_escape(&exposed.id),
                self.next_uid(),
                xml_escape(&exposed.label),
                exposed_type_code(&exposed.default),
                format_float(exposed.min),
                format_float(exposed.max)
            );
        }

        out.push_str("</paraminputs>");
        entries
    }
    pub(super) fn write_graph_outputs(&mut self, out: &mut String, graph: &Graph) -> HashMap<NodeId, u64> {
        let mut output_uids = HashMap::new();
        out.push_str("<graphOutputs>");

        for node in graph.nodes.iter().filter(|n| n.def == "output") {
            let uid = self.next_uid();
            output_uids.insert(node.id, uid);
            let code = output_type(graph, node.id, "output");
            let channels = format!("<channels v=\"{code}\"/>");
            let identifier = text_param(node, "identifier", "output");
            let label = if node.label.is_empty() { identifier.clone() } else { node.label.clone() };

            let _ = write!(
                out,
                "<graphoutput><identifier v=\"{}\"/><uid v=\"{uid}\"/><attributes><label v=\"{}\"/></attributes>",
                xml_escape(&identifier),
                xml_escape(&label)
            );
            component_usage::write(out, node, 0);
            let _ = write!(out, "<group v=\"Material\"/>{channels}</graphoutput>");
        }

        out.push_str("</graphOutputs>");
        output_uids
    }
    pub(super) fn write_implementation(&mut self, out: &mut String, node: &Node, outputs: &[(String, u32, u64)], ids: NodeIdentifiers) -> Result<()> {
        let base = base_params_xml(&node.base);

        match node.def.as_str() {
            "output" => {
                let _ = write!(out, "<compOutputBridge><output v=\"{}\"/></compOutputBridge>", ids.graph_output.unwrap_or(0));
            }
            "input_color" | "input_grayscale" | "input_value" => {
                let _ = write!(out, "<compInputBridge><entry v=\"{}\"/><parameters>{base}</parameters></compInputBridge>", ids.input_entry.unwrap_or(0));
            }
            "graph_instance" => {
                let sub_graph = text_param(node, "graph", "");
                let params: String = node.params.iter().filter(|(k, _)| *k != "graph").filter_map(|(k, v)| param_xml(k, v)).collect();
                let _ = write!(
                    out,
                    "<compInstance><path v=\"pkg:///{}?dependency={}\"/><parameters>{base}{params}</parameters>",
                    xml_escape(&sub_graph),
                    ids.self_uid
                );
                write_bridgings(out, outputs);
                out.push_str("</compInstance>");
            }
            def if def == MISSING_NODE => {
                let name = text_param(node, "sbs_name", &node.label);
                let raw_params = match node.params.get("sbs_parameters") {
                    Some(Value::Text(s)) if is_parameters_element(s) => s.clone(),
                    _ => "<parameters/>".into(),
                };

                if text_param(node, "sbs_kind", "instance") == "filter" {
                    let _ = write!(out, "<compFilter><filter v=\"{}\"/>{raw_params}</compFilter>", xml_escape(&name));
                    return Ok(());
                }

                let package = text_param(node, "sbs_package", "");
                if package.is_empty() {
                    return Err(FormatError::Io(format!("missing package reference for `{name}`")));
                }
                let dependency = self.external_package_uid(&package);
                let path = text_param(node, "sbs_path", &name);
                let _ = write!(out, "<compInstance><path v=\"pkg:///{}?dependency={dependency}\"/>{raw_params}", xml_escape(&path));
                write_bridgings(out, outputs);
                out.push_str("</compInstance>");
            }
            def => match (filter_by_def(def), find(def)) {
                (Some(filter), Some(catalog_def)) => {
                    let _ = write!(out, "<compFilter><filter v=\"{}\"/><parameters>{base}", filter.sbs);
                    let value_of = |id: &str| node.params.get(id).or_else(|| catalog_def.param(id).map(|p| &p.default));
                    for (sbs_name, mc_name) in filter.params {
                        let Some(value) = value_of(mc_name) else { continue };

                        let binding =
                            node.bindings.get(*mc_name).map(|identifier| function_graph::binding(identifier, value)).transpose().map_err(FormatError::Io)?;
                        let function = node.functions.get(*mc_name).or(binding.as_ref()).or_else(|| match value {
                            Value::Function(graph) => Some(graph.as_ref()),
                            _ => None,
                        });
                        if let Some(graph) = function {
                            let function = function_graph::write(graph, self).map_err(FormatError::Io)?;
                            let _ = write!(
                                out,
                                "<parameter><name v=\"{}\"/><relativeTo v=\"0\"/><paramValue>{function}</paramValue></parameter>",
                                xml_escape(sbs_name)
                            );
                            continue;
                        }
                        out.push_str(&param_xml(sbs_name, value).unwrap_or_default());
                    }
                    out.push_str(&special_filter_params_xml(filter.sbs, &value_of, ids.self_uid));
                    out.push_str("</parameters>");
                    for parameter in ["curve", "gradient"] {
                        if let Some(value) = value_of(parameter) {
                            out.push_str(&super::parameter_array::write(value, self));
                        }
                    }
                    if let Some(Value::PatternGraph(graph)) = value_of("pattern") {
                        out.push_str(&super::pattern_graph::write(graph, self).map_err(FormatError::Io)?);
                    }
                    out.push_str("</compFilter>");
                }
                _ => return Err(FormatError::Io(format!("`{def}` has no native primitive mapping; its SBS graph is required"))),
            },
        }
        Ok(())
    }
}
pub(super) type WrittenOutputs = Vec<(String, u32, u64)>;

pub(super) fn write_bridgings(out: &mut String, outputs: &[(String, u32, u64)]) {
    out.push_str("<outputBridgings>");
    for (port, _, uid) in outputs {
        let _ = write!(out, "<outputBridging><uid v=\"{uid}\"/><identifier v=\"{}\"/></outputBridging>", xml_escape(port));
    }
    out.push_str("</outputBridgings>");
}
