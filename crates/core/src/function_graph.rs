use serde::{Deserialize, Serialize};

use crate::Value;

use crate::FunctionNode;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FunctionGraph {
    pub nodes: Vec<FunctionNode>,
    #[serde(default)]
    pub output: Option<u32>,
}

impl FunctionGraph {
    pub fn node(&self, id: u32) -> Option<&FunctionNode> {
        self.nodes.iter().find(|node| node.id == id)
    }

    pub fn next_id(&self) -> u32 {
        let maximum = self.nodes.iter().map(|node| node.id).max().unwrap_or(0);
        if let Some(next) = maximum.checked_add(1) {
            return next;
        }

        (1..=u32::MAX).find(|id| self.node(*id).is_none()).unwrap_or(0)
    }

    pub fn add(&mut self, operation: &str, inputs: &[u32], value: Option<Value>) -> u32 {
        let id = self.next_id();
        let horizontal_position = self.nodes.len() as f32 * 140.0;
        let node =
            FunctionNode { id, type_code: 0, op: operation.into(), inputs: inputs.iter().copied().map(Some).collect(), value, pos: [horizontal_position, 0.0] };

        self.nodes.push(node);
        id
    }

    pub fn constant(value: Value) -> FunctionGraph {
        let mut graph = FunctionGraph::default();
        graph.output = Some(graph.add_constant(value));
        graph
    }

    pub fn add_constant(&mut self, value: Value) -> u32 {
        let operation = match &value {
            Value::Float(_) => "const_float1",
            Value::Float2(_) => "const_float2",
            Value::Float3(_) => "const_float3",
            Value::Float4(_) | Value::Color(_) => "const_float4",
            Value::Int(_) | Value::Enum(_) => "const_int1",
            Value::Int2(_) => "const_int2",
            Value::Int3(_) => "const_int3",
            Value::Int4(_) => "const_int4",
            Value::Text(_) => "const_string",
            Value::Bool(_) => "const_bool",
            _ => "const_float1",
        };

        let value = match value {
            Value::Color(color) => Value::Float4(color),
            Value::Enum(index) => Value::Int(index.min(i32::MAX as u32) as i32),
            value => value,
        };

        self.add(operation, &[], Some(value))
    }
}
