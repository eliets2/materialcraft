//! Clean-room import of a bounded subset of numeric SBS parameter expressions.
//! Unsupported operations retain the reader's warning-and-default behavior.

use materialcraft_core::{
    Value,
    function::{FnGraph, FnNode},
};
use std::collections::{BTreeMap, HashMap};

use super::{XmlNode, child_element, child_value, children_named, sbs_constant, sbs_constant_element, to_natural_value};

pub(super) const MAX_NODES: usize = 4096;

fn float_dims(value: &Value) -> Option<usize> {
    match value {
        Value::Float(_) => Some(1),
        Value::Float2(_) => Some(2),
        Value::Float3(_) => Some(3),
        Value::Float4(_) => Some(4),
        _ => None,
    }
}

pub(super) fn read(dynamic: XmlNode, variables: &BTreeMap<String, Value>, target: &Value) -> Result<FnGraph, String> {
    let expected = float_dims(target).ok_or("only float parameter expressions are supported")?;
    let elements: Vec<_> = child_element(dynamic, "paramNodes").into_iter().flat_map(|p| children_named(p, "paramNode")).take(MAX_NODES + 1).collect();
    if elements.is_empty() || elements.len() > MAX_NODES {
        return Err(format!("expected 1..{MAX_NODES} function nodes"));
    }
    let mut ids = HashMap::new();
    for (index, node) in elements.iter().enumerate() {
        let uid = child_value(*node, "uid").filter(|s| !s.is_empty()).ok_or("function node has no uid")?;
        if ids.insert(uid, index as u32 + 1).is_some() {
            return Err(format!("duplicate function uid `{uid}`"));
        }
    }
    let output = child_value(dynamic, "rootnode").and_then(|uid| ids.get(uid)).copied().ok_or("function rootnode is missing or unknown")?;
    let mut graph = FnGraph { nodes: Vec::with_capacity(elements.len()), output: Some(output) };
    for (index, node) in elements.iter().enumerate() {
        if child_value(*node, "disabled").is_some_and(|v| v != "0") {
            return Err("disabled function nodes are not supported".into());
        }
        let name = child_value(*node, "function").ok_or("function node has no operation")?;
        if child_value(*node, "type").is_some_and(|t| !matches!(t, "256" | "512" | "1024" | "2048")) {
            return Err(format!("`{name}` has an unsupported result type"));
        }
        let (op, ports): (&str, &[&str]) = match name {
            "const_float1" => ("const_float", &[]),
            "const_float2" | "const_float3" | "const_float4" => (name, &[]),
            "get_float1" | "get_float2" | "get_float3" | "get_float4" => ("get_var", &[]),
            "add" | "sub" | "mul" | "div" | "cartesian" => (name, &["a", "b"]),
            "neg" | "abs" | "floor" | "ceil" | "cos" | "sin" | "sqrt" | "log" | "exp" | "pow2" | "log2" => (name, &["a"]),
            _ => return Err(format!("unsupported function operation `{name}`")),
        };
        let value = if ports.is_empty() {
            let data = child_element(*node, "funcDatas")
                .into_iter()
                .flat_map(|p| children_named(p, "funcData"))
                .find(|p| child_value(*p, "name") == Some(name))
                .ok_or_else(|| format!("`{name}` has no function data"))?;
            let value = data
                .children()
                .filter(|n| n.is_element())
                .find_map(|n| sbs_constant_element(n).or_else(|| sbs_constant(n)))
                .ok_or_else(|| format!("`{name}` has no constant value"))?;
            let value = to_natural_value(&value);
            let dims = name.chars().last().and_then(|c| c.to_digit(10)).unwrap_or(0) as usize;
            if op == "get_var" {
                let Value::Text(variable) = &value else { return Err(format!("`{name}` needs a variable name")) };
                if variables.get(variable).and_then(float_dims) != Some(dims) {
                    return Err(format!("`{name}` references an unknown or differently typed input `{variable}`"));
                }
            } else if float_dims(&value) != Some(dims) {
                return Err(format!("`{name}` has the wrong constant type"));
            }
            Some(value)
        } else {
            None
        };
        let mut inputs = vec![None; ports.len()];
        for connection in child_element(*node, "connections").into_iter().flat_map(|p| children_named(p, "connection")) {
            let port = child_value(connection, "identifier").unwrap_or("");
            let slot = ports.iter().position(|p| *p == port).and_then(|i| inputs.get_mut(i)).ok_or_else(|| format!("`{name}` has no input `{port}`"))?;
            if slot.is_some() {
                return Err(format!("duplicate function input `{port}`"));
            }
            *slot = Some(child_value(connection, "connRef").and_then(|uid| ids.get(uid)).copied().ok_or("function connection references an unknown node")?);
        }
        if inputs.iter().any(Option::is_none) {
            return Err(format!("`{name}` has an unconnected input"));
        }
        graph.nodes.push(FnNode { id: index as u32 + 1, op: op.into(), inputs, value, pos: [index as f32 * 140.0, 0.0] });
    }
    let program = materialcraft_nodes::function::compile(&graph, variables, 0)?;
    if program.dims() != expected {
        return Err("function result type does not match the parameter".into());
    }
    Ok(graph)
}
