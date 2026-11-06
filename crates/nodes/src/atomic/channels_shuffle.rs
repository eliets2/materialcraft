use super::ATOMIC;
use crate::{CookContext, CookResult, NodeDefinition, node};
use materialcraft_image::{Image, Kind};

pub(super) fn definition() -> NodeDefinition {
    node("channels_shuffle", "Channels Shuffle", ATOMIC)
        .desc("Builds a color image from any channels of two inputs.")
        .atomic()
        .any_in("input1", "Input 1")
        .any_in("input2", "Input 2")
        .choice("red", "Red", CHANNEL_SOURCES, 0)
        .choice("green", "Green", CHANNEL_SOURCES, 1)
        .choice("blue", "Blue", CHANNEL_SOURCES, 2)
        .choice("alpha", "Alpha", CHANNEL_SOURCES, 3)
        .color_out()
        .cook(channels_shuffle)
}

fn channels_shuffle(context: &CookContext) -> CookResult {
    let a = context.any_or(0, Kind::Color, [0.0, 0.0, 0.0, 1.0]).to_color();
    let b = context.any_or(1, Kind::Color, [0.0, 0.0, 0.0, 1.0]).to_color();
    let src = [context.i("red"), context.i("green"), context.i("blue"), context.i("alpha")];
    Ok(vec![Image::generate(context.w, context.h, Kind::Color, |x, y| {
        let (pa, pb) = (a.get(x as i64, y as i64, false), b.get(x as i64, y as i64, false));
        src.map(|s| match s {
            0..=3 => pa[s as usize],
            4..=7 => pb[(s - 4) as usize],
            9 => 1.0,
            _ => 0.0,
        })
    })])
}

const CHANNEL_SOURCES: &[&str] =
    &["Input 1 red", "Input 1 green", "Input 1 blue", "Input 1 alpha", "Input 2 red", "Input 2 green", "Input 2 blue", "Input 2 alpha", "Black", "White"];
