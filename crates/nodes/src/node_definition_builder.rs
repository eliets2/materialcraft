use crate::{Cook, CookContext, CookResult, NodeDefinition, ParameterDefinition, PortDefinition, PortKind};
use materialcraft_core::{ComponentUsage, Curve, FunctionGraph, Gradient, PatternGraph, Value};

pub(crate) fn no_cook(_: &CookContext) -> CookResult {
    Err("this node is cooked by the graph evaluator".into())
}

/// Builder for [`NodeDefinition`]s
pub(crate) struct NodeDefinitionBuilder(NodeDefinition);

pub(crate) fn node(id: &'static str, label: &'static str, category: &'static str) -> NodeDefinitionBuilder {
    NodeDefinitionBuilder(NodeDefinition {
        id,
        label,
        category,
        description: "",
        inputs: Vec::new(),
        outputs: Vec::new(),
        params: Vec::new(),
        generator: false,
        atomic: false,
        cook: no_cook,
    })
}

impl NodeDefinitionBuilder {
    pub fn desc(mut self, d: &'static str) -> NodeDefinitionBuilder {
        self.0.description = d;
        self
    }

    pub fn atomic(mut self) -> NodeDefinitionBuilder {
        self.0.atomic = true;
        self
    }

    pub fn generator(mut self) -> NodeDefinitionBuilder {
        self.0.generator = true;
        self
    }

    pub fn input(mut self, id: &'static str, label: &'static str, kind: PortKind) -> NodeDefinitionBuilder {
        self.0.inputs.push(PortDefinition { id, label, kind });
        self
    }

    pub fn gray_in(self, id: &'static str, label: &'static str) -> NodeDefinitionBuilder {
        self.input(id, label, PortKind::Gray)
    }

    pub fn color_in(self, id: &'static str, label: &'static str) -> NodeDefinitionBuilder {
        self.input(id, label, PortKind::Color)
    }

    pub fn any_in(self, id: &'static str, label: &'static str) -> NodeDefinitionBuilder {
        self.input(id, label, PortKind::Any)
    }

    pub fn output(mut self, id: &'static str, label: &'static str, kind: PortKind) -> NodeDefinitionBuilder {
        self.0.outputs.push(PortDefinition { id, label, kind });
        self
    }

    pub fn gray_out(self) -> NodeDefinitionBuilder {
        self.output("output", "Output", PortKind::Gray)
    }

    pub fn color_out(self) -> NodeDefinitionBuilder {
        self.output("output", "Output", PortKind::Color)
    }

    pub fn any_out(self) -> NodeDefinitionBuilder {
        self.output("output", "Output", PortKind::Any)
    }
    fn push(mut self, id: &'static str, label: &'static str, default: Value, min: f32, max: f32) -> NodeDefinitionBuilder {
        self.0.params.push(ParameterDefinition { id, label, default, min, max, options: &[], group: "", angle: false, resource: false });
        self
    }
    fn last(&mut self) -> Option<&mut ParameterDefinition> {
        self.0.params.last_mut()
    }

    pub fn float(self, id: &'static str, label: &'static str, d: f32, min: f32, max: f32) -> NodeDefinitionBuilder {
        self.push(id, label, Value::Float(d), min, max)
    }

    pub fn angle(mut self, id: &'static str, label: &'static str, d: f32) -> NodeDefinitionBuilder {
        self = self.push(id, label, Value::Float(d), -1.0, 1.0);
        if let Some(p) = self.last() {
            p.angle = true;
        }
        self
    }

    pub fn int(self, id: &'static str, label: &'static str, d: i32, min: i32, max: i32) -> NodeDefinitionBuilder {
        self.push(id, label, Value::Int(d), min as f32, max as f32)
    }

    pub fn float2(self, id: &'static str, label: &'static str, d: [f32; 2], min: f32, max: f32) -> NodeDefinitionBuilder {
        self.push(id, label, Value::Float2(d), min, max)
    }

    pub fn float4(self, id: &'static str, label: &'static str, default: [f32; 4], min: f32, max: f32) -> NodeDefinitionBuilder {
        self.push(id, label, Value::Float4(default), min, max)
    }

    pub fn boolean(self, id: &'static str, label: &'static str, d: bool) -> NodeDefinitionBuilder {
        self.push(id, label, Value::Bool(d), 0.0, 1.0)
    }

    pub fn color(self, id: &'static str, label: &'static str, d: [f32; 4]) -> NodeDefinitionBuilder {
        self.push(id, label, Value::Color(d), 0.0, 1.0)
    }

    pub fn choice(mut self, id: &'static str, label: &'static str, options: &'static [&'static str], d: u32) -> NodeDefinitionBuilder {
        self = self.push(id, label, Value::Enum(d), 0.0, options.len().saturating_sub(1) as f32);
        if let Some(p) = self.last() {
            p.options = options;
        }
        self
    }

    pub fn component_usage(self, default: u32) -> NodeDefinitionBuilder {
        self.choice("usage", "Component Usage", ComponentUsage::LABELS, default)
    }

    pub fn text(self, id: &'static str, label: &'static str, d: &str) -> NodeDefinitionBuilder {
        self.push(id, label, Value::Text(d.into()), 0.0, 0.0)
    }

    pub fn resource(mut self, id: &'static str, label: &'static str) -> NodeDefinitionBuilder {
        self = self.push(id, label, Value::Text(String::new()), 0.0, 0.0);
        if let Some(p) = self.last() {
            p.resource = true;
        }
        self
    }

    pub fn curve(self, id: &'static str, label: &'static str) -> NodeDefinitionBuilder {
        self.push(id, label, Value::Curve(Curve::default()), 0.0, 1.0)
    }

    pub fn gradient(self, id: &'static str, label: &'static str, g: Gradient) -> NodeDefinitionBuilder {
        self.push(id, label, Value::Gradient(g), 0.0, 1.0)
    }

    pub fn function(self, id: &'static str, label: &'static str, g: FunctionGraph) -> NodeDefinitionBuilder {
        self.push(id, label, Value::Function(Box::new(g)), 0.0, 0.0)
    }

    pub fn pattern_graph(self, id: &'static str, label: &'static str, graph: PatternGraph) -> NodeDefinitionBuilder {
        self.push(id, label, Value::PatternGraph(Box::new(graph)), 0.0, 0.0)
    }

    pub fn cook(mut self, f: Cook) -> NodeDefinition {
        self.0.cook = f;
        self.0
    }
}
