use super::ATOMIC;
use crate::{CookContext, CookResult, NodeDefinition, node};
use materialcraft_image::{Image, Kind};

pub(super) fn definition() -> NodeDefinition {
    node("transformation", "Transformation 2D", ATOMIC)
        .desc("Transforms the input with an offset and a 2 by 2 matrix.")
        .atomic()
        .any_in("input", "Input")
        .float2("offset", "Offset", [0.0; 2], -1.0, 1.0)
        .float4("matrix", "Transform matrix", [1.0, 0.0, 0.0, 1.0], -10.0, 10.0)
        .choice("filtering", "Filtering", &["Bilinear", "Nearest"], 0)
        .color("matte", "Matte color", [0.0; 4])
        .any_out()
        .cook(transformation)
}

fn transformation(context: &CookContext) -> CookResult {
    let input = context.any_or(0, Kind::Gray, [0.0; 4]);
    let offset = context.v2("offset");
    let matrix = context.v4("matrix");
    let determinant = matrix[0] * matrix[3] - matrix[1] * matrix[2];
    if !determinant.is_finite() || determinant.abs() < 1e-8 {
        return Err("transform matrix is singular".into());
    }

    let nearest = context.i("filtering") == 1;
    let matte = context.v4("matte");
    Ok(vec![Image::generate_uv(context.w, context.h, input.kind, |u, v| {
        let x = u - 0.5 - offset[0];
        let y = v - 0.5 - offset[1];
        let source_u = (matrix[3] * x - matrix[1] * y) / determinant + 0.5;
        let source_v = (-matrix[2] * x + matrix[0] * y) / determinant + 0.5;
        if !context.tile && (!(0.0..1.0).contains(&source_u) || !(0.0..1.0).contains(&source_v)) {
            matte
        } else if nearest {
            input.sample_nearest(source_u, source_v, context.tile)
        } else {
            input.sample(source_u, source_v, context.tile)
        }
    })])
}
