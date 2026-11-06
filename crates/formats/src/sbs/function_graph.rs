use super::sbs_value::{constant_xml, sbs_constant, sbs_constant_element, to_natural_value};
use super::sbs_writer::SbsWriter;
use super::xml::{child_element, child_value, children_named, parse_floats, xml_escape};
use materialcraft_core::{FunctionGraph, FunctionNode, Value};
use materialcraft_nodes::function;
use roxmltree::Node as XmlNode;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;

pub(super) fn read(element: XmlNode<'_, '_>) -> Result<FunctionGraph, String> {
    let mut graph = FunctionGraph { output: child_value(element, "rootnode").and_then(|value| value.parse().ok()), nodes: Vec::new() };
    let mut ids = BTreeSet::new();
    for element in child_element(element, "paramNodes").into_iter().flat_map(|nodes| children_named(nodes, "paramNode")) {
        if graph.nodes.len() >= 4096 {
            return Err("function exceeds the 4096-node limit".into());
        }
        let id = child_value(element, "uid").and_then(|value| value.parse().ok()).ok_or("function node has no valid uid")?;
        if !ids.insert(id) {
            return Err(format!("duplicate function uid {id}"));
        }
        let op = child_value(element, "function").ok_or("function node has no operation")?;
        let definition = function::definition(op).ok_or_else(|| format!("unsupported function `{op}`"))?;
        let mut inputs = vec![None; definition.inputs.len()];
        for connection in child_element(element, "connections").into_iter().flat_map(|connections| children_named(connections, "connection")) {
            let port = child_value(connection, "identifier").unwrap_or("");
            let index = definition.inputs.iter().position(|input| *input == port).ok_or_else(|| format!("{op}: unknown input `{port}`"))?;
            let source = child_value(connection, "connRef").and_then(|value| value.parse().ok()).ok_or("invalid function connection")?;
            let input = inputs.get_mut(index).ok_or("invalid input index")?;
            if input.replace(source).is_some() {
                return Err(format!("{op}: duplicate input `{port}`"));
            }
        }

        let value = child_element(element, "funcDatas")
            .into_iter()
            .flat_map(|data| children_named(data, "funcData"))
            .find(|data| child_value(*data, "name") == Some(op))
            .and_then(|data| data.children().filter(|child| child.is_element()).find_map(|child| sbs_constant_element(child).or_else(|| sbs_constant(child))))
            .map(|value| to_natural_value(&value));
        let position = child_element(element, "GUILayout").and_then(|layout| child_value(layout, "gpos")).map(parse_floats).unwrap_or_default();
        graph.nodes.push(FunctionNode {
            id,
            op: op.into(),
            inputs,
            type_code: child_value(element, "type").and_then(|value| value.parse().ok()).unwrap_or(0),
            value: value.or_else(|| native_default(op)),
            pos: [position.first().copied().unwrap_or(0.0), position.get(1).copied().unwrap_or(0.0)],
        });
    }

    if graph.output.is_some_and(|id| !ids.contains(&id)) {
        return Err("function result references a missing node".into());
    }
    if graph.nodes.iter().flat_map(|node| node.inputs.iter().flatten()).any(|id| !ids.contains(id)) {
        return Err("function connection references a missing node".into());
    }
    Ok(graph)
}

fn native_default(op: &str) -> Option<Value> {
    Some(match op {
        "const_float1" => Value::Float(0.0),
        "const_float4" => Value::Float4([0.0; 4]),
        "swizzle2" | "iswizzle2" => Value::Int2([0; 2]),
        "swizzle3" | "iswizzle3" => Value::Int3([0; 3]),
        "swizzle4" | "iswizzle4" => Value::Int4([0; 4]),
        _ => return function::default_value(op),
    })
}

pub(super) fn write(graph: &FunctionGraph, writer: &mut SbsWriter) -> Result<String, String> {
    if graph.nodes.len() > 4096 {
        return Err("function exceeds the 4096-node limit".into());
    }
    let types = function::node_types(graph)?;
    let identifiers: BTreeMap<_, _> = graph.nodes.iter().map(|node| (node.id, writer.next_uid())).collect();
    let mut output = String::from("<dynamicValue>");
    if let Some(root) = graph.output.and_then(|root| identifiers.get(&root)) {
        let _ = write!(output, "<rootnode v=\"{root}\"/>");
    }
    output.push_str("<paramNodes>");
    for node in &graph.nodes {
        let definition = function::definition(&node.op).ok_or_else(|| format!("cannot save unknown function `{}`", node.op))?;
        let inferred = types.get(&node.id).copied().unwrap_or(256);
        let code = if node.type_code == 0 { inferred } else { node.type_code };
        let _ = write!(
            output,
            "<paramNode><uid v=\"{}\"/><function v=\"{}\"/><type v=\"{code}\"/><GUILayout><gpos v=\"{} {} 0\"/></GUILayout><connections>",
            identifiers.get(&node.id).copied().ok_or("missing function uid")?,
            xml_escape(&node.op),
            node.pos[0],
            node.pos[1]
        );
        for (input, source) in definition.inputs.iter().zip(&node.inputs) {
            if let Some(source) = source.and_then(|source| identifiers.get(&source)) {
                let _ = write!(output, "<connection><identifier v=\"{}\"/><connRef v=\"{source}\"/></connection>", xml_escape(input));
            }
        }
        output.push_str("</connections>");
        if let Some(value) = &node.value {
            let constant = constant_xml(value).ok_or("function data must be a scalar or vector constant")?;
            let _ =
                write!(output, "<funcDatas><funcData><name v=\"{}\"/><constantValue>{constant}</constantValue></funcData></funcDatas>", xml_escape(&node.op));
        }
        output.push_str("</paramNode>");
    }
    output.push_str("</paramNodes></dynamicValue>");
    Ok(output)
}

pub(super) fn binding(identifier: &str, value: &Value) -> Result<FunctionGraph, String> {
    use materialcraft_core::ValueType;

    let operation = match ValueType::of(value).ok_or("only scalar and vector parameters can be exposed")? {
        ValueType::Boolean => "get_bool",
        ValueType::Integer => "get_integer1",
        ValueType::Integer2 => "get_integer2",
        ValueType::Integer3 => "get_integer3",
        ValueType::Integer4 => "get_integer4",
        ValueType::Float => "get_float1",
        ValueType::Float2 => "get_float2",
        ValueType::Float3 => "get_float3",
        ValueType::Float4 => "get_float4",
        ValueType::String => "get_string",
    };

    let mut graph = FunctionGraph::default();
    graph.output = Some(graph.add(operation, &[], Some(Value::Text(identifier.into()))));
    Ok(graph)
}
