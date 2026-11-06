use materialcraft_core::Value;
use materialcraft_image::Image;
use std::collections::BTreeMap;

pub(crate) fn evaluate_value(
    parameters: &BTreeMap<String, Value>,
    variables: &BTreeMap<String, Value>,
    seed: u32,
    size: [u32; 2],
    images: &[Option<Image>],
    tile: bool,
) -> Result<Value, String> {
    use materialcraft_nodes::function::{EvaluationContext, EvaluationState, compile};

    let Some(Value::Function(graph)) = parameters.get("function") else {
        return Err("missing value function".into());
    };

    let program = compile(graph, seed)?;
    let context = EvaluationContext { position: [0.5; 2], size: size.map(|side| side as f32), images, tile, variables };

    program.evaluate(&context, &mut EvaluationState::default())
}
