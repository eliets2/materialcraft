//! Original single-stroke font rendered as anti-aliased capsules

use crate::atomic::ATOMIC;
use crate::raster::{CapsuleRaster, CapsuleSegment, composite};
use crate::{CookContext, CookResult, NodeDefinition, node};

/// The font: each glyph is strokes separated by spaces, each stroke a polyline of `xy` digit pairs on a grid 4 wide where y = 0 is the cap height, 2 the x-height, 6 the baseline and 8 the descender
const GLYPHS: &[(char, &str)] = &[
    ('A', "062046 1333"),
    ('B', "06003041423303 3344453606"),
    ('C', "4130100105163645"),
    ('D', "06003041453606"),
    ('E', "40000646 0333"),
    ('F', "400006 0333"),
    ('G', "41301001051636454323"),
    ('H', "0006 4046 0343"),
    ('I', "1030 2026 1636"),
    ('J', "4045361605"),
    ('K', "0006 4004 1346"),
    ('L', "000646"),
    ('M', "0600234046"),
    ('N', "06004640"),
    ('O', "103041453616050110"),
    ('P', "06003041423303"),
    ('Q', "103041453616050110 2446"),
    ('R', "06003041423303 2346"),
    ('S', "413010010213334445361605"),
    ('T', "0040 2026"),
    ('U', "000516364540"),
    ('V', "002640"),
    ('W', "0016233640"),
    ('X', "0046 4006"),
    ('Y', "002340 2326"),
    ('Z', "00400646"),
    ('a', "4246 4332120305163645"),
    ('b', "0006 0312324345361605"),
    ('c', "4332120305163645"),
    ('d', "4046 4332120305163645"),
    ('e', "04444332120305163645"),
    ('f', "4130201116 0232"),
    ('g', "4247381807 4332120305163645"),
    ('h', "0006 0312324346"),
    ('i', "2226 20"),
    ('j', "3237281807 30"),
    ('k', "0006 3205 1436"),
    ('l', "10202536"),
    ('m', "0206 03122326 23324346"),
    ('n', "0206 0312324346"),
    ('o', "123243453616050312"),
    ('p', "0208 0312324345361605"),
    ('q', "4248 4332120305163645"),
    ('r', "0206 04223243"),
    ('s', "43321203143445361605"),
    ('t', "10152636 0232"),
    ('u', "0205163645 4246"),
    ('v', "022642"),
    ('w', "0216243642"),
    ('x', "0246 4206"),
    ('y', "0226 4218"),
    ('z', "02420646"),
    ('0', "103041453616050110 3115"),
    ('1', "112026 1636"),
    ('2', "01103041420646"),
    ('3', "01103041423313 334445361605"),
    ('4', "36300444"),
    ('5', "400003334445361605"),
    ('6', "30100105163645443303"),
    ('7', "004016"),
    ('8', "13020110304142331304051636454433"),
    ('9', "43130201103041453616"),
    ('.', "26"),
    (',', "2617"),
    ('!', "2024 26"),
    ('?', "0110304142332324 26"),
    (':', "22 26"),
    (';', "22 2617"),
    ('-', "1333"),
    ('+', "1333 2224"),
    ('=', "1232 1434"),
    ('*', "1135 3115 0333"),
    ('/', "4006"),
    ('\\', "0046"),
    ('(', "30212536"),
    (')', "10212516"),
    ('[', "30202636"),
    (']', "10202616"),
    ('<', "410345"),
    ('>', "014305"),
    ('\'', "2021"),
    ('"', "1011 3031"),
    ('_', "0747"),
    ('#', "1115 3135 0242 0444"),
    ('%', "4006 01 45"),
    (' ', ""),
];

/// A character's strokes; unknown characters show as `?`
fn glyph(c: char) -> &'static str {
    let find = |c: char| GLYPHS.iter().find(|(g, _)| *g == c).map(|(_, s)| *s);
    find(c).or_else(|| find('?')).unwrap_or("")
}

/// The strokes of `s` as polylines in grid units
fn strokes(s: &str) -> impl Iterator<Item = Vec<[f32; 2]>> + '_ {
    s.split(' ')
        .filter(|t| !t.is_empty())
        .map(|t| t.as_bytes().chunks_exact(2).map(|p| [p[0].saturating_sub(b'0') as f32, p[1].saturating_sub(b'0') as f32]).collect())
}

/// Longest text drawn (an allocation cap)
const MAX_CHARS: usize = 10_000;

pub(super) fn definition() -> NodeDefinition {
    node("text", "Text", ATOMIC)
        .desc("Draws text with a built-in single-stroke font (A–Z, a–z, 0–9, punctuation). \\n starts a new line.")
        .atomic()
        .generator()
        .any_in("background", "Background")
        .boolean("color_mode", "Color output", false)
        .color("background", "Background color", [0.0, 0.0, 0.0, 1.0])
        .text("text", "Text", "MaterialCraft")
        .text("fontdata", "Font reference", "")
        .float("fontsize", "Font size", 0.09, 0.0, 1.0)
        .float2("position", "Position", [0.0; 2], -1.0, 1.0)
        .float4("matrix", "Transform matrix", [1.0, 0.0, 0.0, 1.0], -10.0, 10.0)
        .choice("alignment", "Alignment", &["Left", "Center", "Right"], 1)
        .any_out()
        .cook(text)
}

/// The capsule segments of `text` in pixel space
fn layout(context: &CookContext, text: &str) -> Vec<CapsuleSegment> {
    let unit = context.f("fontsize").max(0.0) * context.h as f32 / 8.0;
    let (advance, line_height) = (6.0, 10.0);
    let (alignment, position) = (context.i("alignment"), context.v2("position"));
    let matrix = context.v4("matrix");
    let anchor = [(0.5 + position[0]) * context.w as f32, (0.5 + position[1]) * context.h as f32];
    let lines: Vec<&str> = text.split('\n').collect();
    // The block is centered vertically on the anchor (cap height of the first line to baseline of the last)
    let top = -((lines.len().max(1) - 1) as f32 * line_height + 6.0) * 0.5;
    let mut segments = Vec::new();
    for (line_index, line) in lines.iter().enumerate() {
        let n = line.chars().count() as f32;
        let width = if n > 0.0 { (n - 1.0) * advance + 4.0 } else { 0.0 };
        let x0 = match alignment {
            0 => 0.0,
            1 => -width * 0.5,
            _ => -width,
        };

        let y0 = top + line_index as f32 * line_height;
        for (character_index, character) in line.chars().enumerate() {
            let glyph_x = x0 + character_index as f32 * advance;
            for stroke in strokes(glyph(character)) {
                let points: Vec<[f32; 2]> = stroke
                    .iter()
                    .map(|p| {
                        let (x, y) = ((glyph_x + p[0]) * unit, (y0 + p[1]) * unit);
                        [anchor[0] + x * matrix[0] + y * matrix[1], anchor[1] + x * matrix[2] + y * matrix[3]]
                    })
                    .collect();
                match points.as_slice() {
                    [a] => segments.push(CapsuleSegment { start: *a, end: *a, weight: 1.0 }),
                    _ => segments.extend(points.windows(2).map(|w| CapsuleSegment { start: w[0], end: w[1], weight: 1.0 })),
                }
            }
        }
    }
    segments
}

fn text(context: &CookContext) -> CookResult {
    if !context.text("fontdata").is_empty() {
        return Err("external fonts are not supported yet; clear Font reference to use the built-in stroke font".into());
    }

    let text: String = context.text("text").replace("\\n", "\n").chars().take(MAX_CHARS).collect();
    let radius = 0.7 * context.f("fontsize").max(0.0) * context.h as f32 / 16.0;
    let raster = CapsuleRaster::new(layout(context, &text), radius, context.w, context.h, context.tile);
    Ok(vec![composite(context, |x, y| [1.0, 1.0, 1.0, raster.coverage(x as f32 + 0.5, y as f32 + 0.5)])])
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::{find, resolve_params};

    #[test]
    fn font_data_is_well_formed() {
        for (c, s) in GLYPHS {
            for t in s.split(' ').filter(|t| !t.is_empty()) {
                assert!(t.len() % 2 == 0 && t.bytes().all(|b| b.is_ascii_digit()), "{c:?}: {t}");
                assert!(t.as_bytes().chunks(2).all(|p| p[0] <= b'4' && p[1] <= b'8'), "{c:?}: {t} leaves the grid");
            }
        }
        for c in ('A'..='Z').chain('a'..='z').chain('0'..='9') {
            assert!(GLYPHS.iter().any(|(g, _)| *g == c), "{c} missing");
        }
    }

    #[test]
    fn renders_material_craft() {
        let def = find("text").unwrap();
        let p = resolve_params(def, &BTreeMap::new());
        let no_res = |_: &str| None;
        let vars = BTreeMap::new();
        let context = CookContext { w: 256, h: 256, seed: 0, tile: true, inputs: &[], params: &p, resource: &no_res, vars: &vars };
        let img = &text(&context).unwrap()[0];
        let lit = img.data.iter().filter(|value| **value > 0.5).count();
        assert!(lit > 300, "{lit} pixels");
        // Centered: nothing in the top and bottom quarters
        assert!((0..256).all(|x| img.get(x, 20, false)[0] == 0.0 && img.get(x, 236, false)[0] == 0.0));
    }
}
