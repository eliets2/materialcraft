use materialcraft_core::{PatternGraph, PatternNode, PatternNodeKind, Value};
use roxmltree::Node as XmlNode;
use std::collections::BTreeMap;
use std::fmt::Write;

use super::node_parameter::param_xml;
use super::sbs_value::{sbs_constant, to_natural_value};
use super::xml::{child_element, child_value, children_named, parse_floats, xml_escape};
use super::{function_graph, sbs_writer::SbsWriter};

pub(super) fn read(element: XmlNode<'_, '_>) -> Result<PatternGraph, String> {
    let mut data = BTreeMap::new();
    for element in child_element(element, "paramsGraphDatas").into_iter().flat_map(|data| children_named(data, "paramsGraphData")) {
        if data.len() >= 4096 {
            return Err("pattern graph exceeds the 4096-node limit".into());
        }

        let uid = child_value(element, "uid").ok_or("pattern data has no uid")?;
        let mut parameters = BTreeMap::new();
        for parameter in child_element(element, "parameters").into_iter().flat_map(|parameters| children_named(parameters, "parameter")) {
            let name = child_value(parameter, "name").ok_or("pattern parameter has no name")?;
            let value = child_element(parameter, "paramValue").ok_or("pattern parameter has no value")?;
            let value = match child_element(value, "dynamicValue") {
                Some(function) => Value::Function(Box::new(function_graph::read(function)?)),
                None => to_natural_value(&sbs_constant(value).ok_or("invalid pattern parameter value")?),
            };

            parameters.insert(name.to_string(), value);
        }
        if data.insert(uid, parameters).is_some() {
            return Err("duplicate pattern data uid".into());
        }
    }

    let mut graph = PatternGraph { root: child_value(element, "rootnode").and_then(|id| id.parse().ok()), nodes: Vec::new() };
    for element in child_element(element, "paramsGraphNodes").into_iter().flat_map(|nodes| children_named(nodes, "paramsGraphNode")) {
        if graph.nodes.len() >= 4096 {
            return Err("pattern graph exceeds the 4096-node limit".into());
        }

        let id = child_value(element, "uid").and_then(|id| id.parse().ok()).ok_or("invalid pattern node uid")?;
        let kind = PatternNodeKind::ALL.into_iter().find(|kind| Some(kind.native_id()) == child_value(element, "type")).ok_or("unknown pattern node type")?;
        let mut children = vec![None; kind.child_count()];
        for connection in child_element(element, "connections").into_iter().flat_map(|connections| children_named(connections, "connection")) {
            let identifier = child_value(connection, "identifier").ok_or("missing pattern child port")?;
            let slot = (0..kind.child_count()).find(|slot| kind.child_port(*slot) == Some(identifier)).ok_or("invalid pattern child slot")?;
            let child = child_value(connection, "connRef").and_then(|id| id.parse().ok()).ok_or("invalid pattern child uid")?;
            if children.get_mut(slot).ok_or("invalid pattern child slot")?.replace(child).is_some() {
                return Err("duplicate pattern connection".into());
            }
        }

        let parameters = child_value(element, "data").and_then(|uid| data.get(uid)).cloned();
        let parameters = match parameters {
            Some(parameters) => parameters,
            None if kind == PatternNodeKind::Dot => BTreeMap::new(),
            None => return Err("missing pattern data".into()),
        };
        let position = child_element(element, "GUILayout").and_then(|layout| child_value(layout, "gpos")).map(parse_floats).unwrap_or_default();
        graph.nodes.push(PatternNode {
            id,
            kind,
            children,
            parameters,
            position: [position.first().copied().unwrap_or(0.0), position.get(1).copied().unwrap_or(0.0)],
        });
    }
    graph.validate()?;
    Ok(graph)
}

pub(super) fn write(graph: &PatternGraph, writer: &mut SbsWriter) -> Result<String, String> {
    graph.validate()?;
    let identifiers: BTreeMap<_, _> = graph.nodes.iter().map(|node| (node.id, (writer.next_uid(), writer.next_uid()))).collect();
    let mut output = format!("<paramsGraphs><paramsGraph><name v=\"\"/><uid v=\"{}\"/>", writer.next_uid());
    if let Some((root, _)) = graph.root.and_then(|root| identifiers.get(&root)) {
        let _ = write!(output, "<rootnode v=\"{root}\"/>");
    }
    output.push_str("<paramsGraphDatas>");
    for node in &graph.nodes {
        let (_, data) = identifiers.get(&node.id).ok_or("missing pattern uid")?;
        let _ = write!(output, "<paramsGraphData><uid v=\"{data}\"/><type v=\"{}\"/><parameters>", node.kind.native_id());
        let definition = materialcraft_nodes::pattern::definition(node.kind).ok_or("missing pattern definition")?;
        let mut parameters: BTreeMap<_, _> = definition.params.iter().map(|parameter| (parameter.id.to_string(), parameter.default.clone())).collect();
        parameters.extend(node.parameters.clone());
        for (name, value) in &parameters {
            if let Value::Function(function) = value {
                let function = function_graph::write(function, writer)?;
                let _ = write!(output, "<parameter><name v=\"{}\"/><paramValue>{function}</paramValue></parameter>", xml_escape(name));
            } else {
                output.push_str(&param_xml(name, value).ok_or("invalid pattern parameter")?);
            }
        }
        output.push_str("</parameters></paramsGraphData>");
    }
    output.push_str("</paramsGraphDatas><paramsGraphNodes>");
    for node in &graph.nodes {
        let (uid, data) = identifiers.get(&node.id).ok_or("missing pattern uid")?;
        let _ = write!(
            output,
            "<paramsGraphNode><uid v=\"{uid}\"/><type v=\"{}\"/><data v=\"{data}\"/><GUILayout><gpos v=\"{} {} 0\"/></GUILayout><connections>",
            node.kind.native_id(),
            node.position[0],
            node.position[1]
        );
        for (slot, child) in node.children.iter().enumerate() {
            if let Some((uid, _)) = child.and_then(|child| identifiers.get(&child)) {
                let port = node.kind.child_port(slot).ok_or("invalid pattern child slot")?;
                let _ = write!(output, "<connection><identifier v=\"{port}\"/><connRef v=\"{uid}\"/></connection>");
            }
        }
        output.push_str("</connections></paramsGraphNode>");
    }
    output.push_str("</paramsGraphNodes></paramsGraph></paramsGraphs>");
    Ok(output)
}
