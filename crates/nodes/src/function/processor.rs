use super::{EvaluationContext, EvaluationState, compile};
use crate::{CookContext, CookResult};
use materialcraft_core::{FunctionGraph, Value};
use materialcraft_image::{Image, Kind};
use rayon::prelude::*;
use std::sync::Arc;

pub fn default_pixel_function() -> FunctionGraph {
    let mut graph = FunctionGraph::default();
    let position = graph.add("get_float2", &[], Some(Value::Text("$pos".into())));
    graph.output = Some(graph.add("samplecol", &[position], Some(Value::Int2([0, 1]))));
    graph
}

fn pixel(value: Value, gray: bool) -> Result<[f32; 4], String> {
    let values = super::value_type::floats(&value)?;
    Ok(if gray {
        [values[0], 0.0, 0.0, 0.0]
    } else {
        match super::value_type::dimensions(&value) {
            1 => [values[0], values[0], values[0], 1.0],
            2 => [values[0], values[1], 0.0, 1.0],
            3 => [values[0], values[1], values[2], 1.0],
            _ => values,
        }
    })
}

pub(crate) fn pixel_processor(context: &CookContext) -> CookResult {
    let graph = context.function("function").ok_or("missing pixel function")?;
    let program = compile(graph, context.seed)?;
    let gray = !context.b("color_mode");
    let kind = if gray { Kind::Gray } else { Kind::Color };

    let (width, height) = materialcraft_image::clamp_size(context.w, context.h);
    let channels = kind.channels();
    let mut data = vec![0.0; width as usize * height as usize * channels];
    data.par_chunks_mut(width as usize * channels).enumerate().try_for_each_init(EvaluationState::default, |state, (y, row)| -> Result<(), String> {
        for (x, target) in row.chunks_exact_mut(channels).enumerate() {
            let evaluation = EvaluationContext {
                position: [(x as f32 + 0.5) / width as f32, (y as f32 + 0.5) / height as f32],
                size: [width as f32, height as f32],
                images: context.inputs,
                tile: context.tile,
                variables: context.vars,
            };

            let result = pixel(program.evaluate_pixel(&evaluation, state)?, gray)?;
            target.copy_from_slice(result.get(..channels).ok_or("invalid output channels")?);
        }
        Ok(())
    })?;
    Ok(vec![Image { w: width, h: height, kind, data: Arc::new(data) }])
}
