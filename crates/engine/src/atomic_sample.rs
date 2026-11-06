use crate::{EngineError, Result};
use materialcraft_core::{FunctionGraph, Graph, Package, PatternGraph, PatternNodeKind, Value, ValueType};

pub fn atomic_sample() -> Result<Package> {
    let mut package = Package::new("Atomic sample");
    let mut graph = Graph::new("Atomic_sample");
    graph.size = [8, 8];
    let input = graph.add_node("input_value", [0.0, 0.0]);
    graph.node_mut(input)?.params.insert("default".into(), Value::Float(0.35));
    graph.node_mut(input)?.params.insert("identifier".into(), Value::Text("brightness".into()));

    let value = graph.add_node("value_processor", [200.0, 0.0]);
    let mut function = FunctionGraph::default();
    let get = function.add("get_float1", &[], Some(Value::Text("brightness".into())));
    let two = function.add_constant(Value::Float(2.0));
    function.output = Some(function.add("mul", &[get, two], None));
    graph.node_mut(value)?.input_values.insert("brightness".into(), ValueType::Float);
    graph.node_mut(value)?.params.insert("function".into(), Value::Function(Box::new(function)));
    graph.connect(input, "output", value, "brightness")?;

    let pixel = graph.add_node("pixel_processor", [400.0, 0.0]);
    let mut function = FunctionGraph::default();
    let position = function.add("get_float2", &[], Some(Value::Text("$pos".into())));
    let horizontal = function.add("swizzle1", &[position], Some(Value::Int(0)));
    let brightness = function.add("get_float1", &[], Some(Value::Text("brightness".into())));
    function.output = Some(function.add("mul", &[horizontal, brightness], None));
    graph.node_mut(pixel)?.params.insert("function".into(), Value::Function(Box::new(function)));
    graph.node_mut(pixel)?.params.insert("color_mode".into(), Value::Bool(false));
    graph.node_mut(pixel)?.input_values.insert("brightness".into(), ValueType::Float);
    graph.connect(value, "output", pixel, "brightness")?;

    let pattern = graph.add_node("fx_map", [400.0, 220.0]);
    let mut patterns = PatternGraph::default();
    let iterate = patterns.add(PatternNodeKind::Iterate).map_err(EngineError::Other)?;
    let switch = patterns.add(PatternNodeKind::Switch).map_err(EngineError::Other)?;
    let quadrant = patterns.add(PatternNodeKind::Quadrant).map_err(EngineError::Other)?;
    for node in &mut patterns.nodes {
        match node.kind {
            PatternNodeKind::Dot => {}
            PatternNodeKind::Iterate => {
                node.position = [160.0, 0.0];
                node.children = vec![Some(switch), Some(quadrant)];
                node.parameters.insert("numberadded".into(), Value::Int(8));
            }
            PatternNodeKind::Switch => {
                node.position = [0.0, 192.0];
                node.children = vec![None, Some(quadrant)];
            }
            PatternNodeKind::Quadrant => {
                node.position = [160.0, 384.0];
                node.parameters.insert("patterntype".into(), Value::Int(3));
                node.parameters.insert("patternsize".into(), Value::Float2([0.12; 2]));
                node.parameters.insert("opacity".into(), Value::Float4([0.5, 0.5, 0.5, 1.0]));
                let mut function = FunctionGraph::default();
                let number = function.add("get_float1", &[], Some(Value::Text("$number".into())));
                let step = function.add_constant(Value::Float(std::f32::consts::TAU / 8.0));
                let angle = function.add("mul", &[number, step], None);
                let radius = function.add_constant(Value::Float(0.3));
                function.output = Some(function.add("cartesian", &[radius, angle], None));
                node.parameters.insert("frameoffset".into(), Value::Function(Box::new(function)));
            }
        }
    }
    patterns.root = Some(iterate);
    graph.node_mut(pattern)?.params.insert("pattern".into(), Value::PatternGraph(Box::new(patterns)));
    let blend = graph.add_node("blend", [600.0, 0.0]);
    graph.node_mut(blend)?.params.insert("mode".into(), Value::Enum(1));
    graph.connect(pattern, "output", blend, "foreground")?;
    graph.connect(pixel, "output", blend, "background")?;
    let blur = graph.add_node("blur", [800.0, 0.0]);
    graph.node_mut(blur)?.params.insert("intensity".into(), Value::Float(1.5));
    graph.connect(blend, "output", blur, "input")?;
    let output = graph.add_node("output", [1000.0, 0.0]);
    graph.node_mut(output)?.params.insert("usage".into(), Value::Enum(4));
    graph.node_mut(output)?.params.insert("identifier".into(), Value::Text("height".into()));
    graph.connect(blur, "output", output, "input")?;
    let output = graph.add_node("output", [400.0, -200.0]);
    graph.node_mut(output)?.params.insert("identifier".into(), Value::Text("brightness_value".into()));
    graph.node_mut(output)?.params.insert("usage".into(), Value::Enum(9));
    graph.connect(value, "output", output, "input")?;
    package.graphs.push(graph);
    Ok(package)
}
