//! Node definitions: ports, parameters and the cook function of every node in the library,
//! built once into a static registry ([`catalog`]).

use std::collections::BTreeMap;
use std::sync::OnceLock;

use materialcraft_core::function::FnGraph;
use materialcraft_core::{Curve, Gradient, Value};
use materialcraft_image::{Image, Kind};

/// What a port carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PortKind {
    Gray,
    Color,
    /// Either; outputs of this kind follow the main input.
    Any,
}

#[derive(Clone, Debug)]
pub struct PortDef {
    pub id: &'static str,
    pub label: &'static str,
    pub kind: PortKind,
}

/// The editor widget and range of a parameter.
#[derive(Clone, Debug)]
pub struct ParamDef {
    pub id: &'static str,
    pub label: &'static str,
    pub default: Value,
    pub min: f32,
    pub max: f32,
    /// Labels of an enum parameter.
    pub options: &'static [&'static str],
    /// Section heading in the Properties panel ("" = none).
    pub group: &'static str,
    /// An angle in turns (shown in degrees).
    pub angle: bool,
    /// The name of a package resource (bitmap / SVG).
    pub resource: bool,
}

/// Where a cook runs: output size, seed, tiling, inputs and resolved parameters.
pub struct Ctx<'a> {
    pub w: u32,
    pub h: u32,
    pub seed: u32,
    pub tile: bool,
    /// By input port order; `None` = not connected.
    pub inputs: &'a [Option<Image>],
    pub params: &'a BTreeMap<String, Value>,
    /// The package's bitmap resources, decoded on demand.
    pub resource: &'a (dyn Fn(&str) -> Option<Image> + Sync),
    /// Exposed graph inputs (for `get_*` nodes in function graphs).
    pub vars: &'a BTreeMap<String, Value>,
}

impl Ctx<'_> {
    pub fn value(&self, id: &str) -> Option<&Value> {
        self.params.get(id)
    }

    pub fn f(&self, id: &str) -> f32 {
        self.value(id).and_then(Value::as_f32).filter(|v| v.is_finite()).unwrap_or(0.0)
    }

    pub fn i(&self, id: &str) -> i32 {
        self.value(id).and_then(Value::as_i32).unwrap_or(0)
    }

    pub fn b(&self, id: &str) -> bool {
        self.value(id).and_then(Value::as_bool).unwrap_or(false)
    }

    pub fn v4(&self, id: &str) -> [f32; 4] {
        self.value(id).and_then(Value::as_vec4).unwrap_or([0.0; 4])
    }

    pub fn v2(&self, id: &str) -> [f32; 2] {
        let v = self.v4(id);
        [v[0], v[1]]
    }

    pub fn text(&self, id: &str) -> &str {
        match self.value(id) {
            Some(Value::Text(s)) => s,
            _ => "",
        }
    }

    pub fn curve(&self, id: &str) -> Curve {
        match self.value(id) {
            Some(Value::Curve(c)) => c.clone(),
            _ => Curve::default(),
        }
    }

    pub fn gradient(&self, id: &str) -> Gradient {
        match self.value(id) {
            Some(Value::Gradient(g)) => g.clone(),
            _ => Gradient::default(),
        }
    }

    pub fn function(&self, id: &str) -> Option<&FnGraph> {
        match self.value(id) {
            Some(Value::Function(g)) => Some(g.as_ref()),
            _ => None,
        }
    }

    /// Input `i` if connected.
    pub fn input(&self, i: usize) -> Option<&Image> {
        self.inputs.get(i).and_then(Option::as_ref)
    }

    /// Input `i` resampled to the output size (`None` if not connected).
    pub fn sized(&self, i: usize) -> Option<Image> {
        self.input(i).map(|img| img.resize(self.w, self.h, self.tile))
    }

    /// Input `i` at the output size as gray, or a constant gray `v` when unconnected.
    pub fn gray_or(&self, i: usize, v: f32) -> Image {
        self.sized(i).map(|img| img.to_gray()).unwrap_or_else(|| Image::filled(self.w, self.h, Kind::Gray, [v; 4]))
    }

    /// Input `i` at the output size (kind kept), or constant `v` of `kind` when unconnected.
    pub fn any_or(&self, i: usize, kind: Kind, v: [f32; 4]) -> Image {
        self.sized(i).unwrap_or_else(|| Image::filled(self.w, self.h, kind, v))
    }

    /// The kind an "Any" output takes: the main input's, else `fallback`.
    pub fn main_kind(&self, fallback: Kind) -> Kind {
        self.inputs.iter().flatten().next().map_or(fallback, |i| i.kind)
    }

    /// A gray image from `f(u, v)` at the output size.
    pub fn gen_gray(&self, f: impl Fn(f32, f32) -> f32 + Sync) -> Image {
        Image::generate_uv(self.w, self.h, Kind::Gray, |u, v| [f(u, v), 0.0, 0.0, 0.0])
    }

    pub fn gen_color(&self, f: impl Fn(f32, f32) -> [f32; 4] + Sync) -> Image {
        Image::generate_uv(self.w, self.h, Kind::Color, f)
    }
}

pub type CookResult = Result<Vec<Image>, String>;
pub type Cook = fn(&Ctx) -> CookResult;

/// A node in the library.
#[derive(Clone, Debug)]
pub struct NodeDef {
    pub id: &'static str,
    pub label: &'static str,
    /// Library path, e.g. `Filters/Blurs`, `Texture generators/Noises`, `Atomic nodes`.
    pub category: &'static str,
    pub description: &'static str,
    pub inputs: Vec<PortDef>,
    pub outputs: Vec<PortDef>,
    pub params: Vec<ParamDef>,
    /// A generator takes its size from the graph, not from an input.
    pub generator: bool,
    /// One of the engine's built-in ("atomic") nodes (shown with a grey header).
    pub atomic: bool,
    pub cook: Cook,
}

impl NodeDef {
    pub fn param(&self, id: &str) -> Option<&ParamDef> {
        self.params.iter().find(|p| p.id == id)
    }

    pub fn input_index(&self, id: &str) -> Option<usize> {
        self.inputs.iter().position(|p| p.id == id)
    }

    pub fn output_index(&self, id: &str) -> Option<usize> {
        self.outputs.iter().position(|p| p.id == id)
    }

    /// The last path element of the category ("Blurs").
    pub fn group(&self) -> &'static str {
        self.category.rsplit('/').next().unwrap_or(self.category)
    }
}

pub(crate) fn no_cook(_: &Ctx) -> CookResult {
    Err("this node is cooked by the graph evaluator".into())
}

/// Builder for [`NodeDef`]s.
pub(crate) struct B(NodeDef);

pub(crate) fn node(id: &'static str, label: &'static str, category: &'static str) -> B {
    B(NodeDef {
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

impl B {
    pub fn desc(mut self, d: &'static str) -> B {
        self.0.description = d;
        self
    }
    pub fn atomic(mut self) -> B {
        self.0.atomic = true;
        self
    }
    pub fn generator(mut self) -> B {
        self.0.generator = true;
        self
    }
    pub fn input(mut self, id: &'static str, label: &'static str, kind: PortKind) -> B {
        self.0.inputs.push(PortDef { id, label, kind });
        self
    }
    pub fn gray_in(self, id: &'static str, label: &'static str) -> B {
        self.input(id, label, PortKind::Gray)
    }
    pub fn color_in(self, id: &'static str, label: &'static str) -> B {
        self.input(id, label, PortKind::Color)
    }
    pub fn any_in(self, id: &'static str, label: &'static str) -> B {
        self.input(id, label, PortKind::Any)
    }
    pub fn output(mut self, id: &'static str, label: &'static str, kind: PortKind) -> B {
        self.0.outputs.push(PortDef { id, label, kind });
        self
    }
    pub fn gray_out(self) -> B {
        self.output("output", "Output", PortKind::Gray)
    }
    pub fn color_out(self) -> B {
        self.output("output", "Output", PortKind::Color)
    }
    pub fn any_out(self) -> B {
        self.output("output", "Output", PortKind::Any)
    }
    fn push(mut self, id: &'static str, label: &'static str, default: Value, min: f32, max: f32) -> B {
        self.0.params.push(ParamDef { id, label, default, min, max, options: &[], group: "", angle: false, resource: false });
        self
    }
    fn last(&mut self) -> Option<&mut ParamDef> {
        self.0.params.last_mut()
    }
    pub fn float(self, id: &'static str, label: &'static str, d: f32, min: f32, max: f32) -> B {
        self.push(id, label, Value::Float(d), min, max)
    }
    pub fn angle(mut self, id: &'static str, label: &'static str, d: f32) -> B {
        self = self.push(id, label, Value::Float(d), -1.0, 1.0);
        if let Some(p) = self.last() {
            p.angle = true;
        }
        self
    }
    pub fn int(self, id: &'static str, label: &'static str, d: i32, min: i32, max: i32) -> B {
        self.push(id, label, Value::Int(d), min as f32, max as f32)
    }
    pub fn float2(self, id: &'static str, label: &'static str, d: [f32; 2], min: f32, max: f32) -> B {
        self.push(id, label, Value::Float2(d), min, max)
    }
    pub fn boolean(self, id: &'static str, label: &'static str, d: bool) -> B {
        self.push(id, label, Value::Bool(d), 0.0, 1.0)
    }
    pub fn color(self, id: &'static str, label: &'static str, d: [f32; 4]) -> B {
        self.push(id, label, Value::Color(d), 0.0, 1.0)
    }
    pub fn choice(mut self, id: &'static str, label: &'static str, options: &'static [&'static str], d: u32) -> B {
        self = self.push(id, label, Value::Enum(d), 0.0, options.len().saturating_sub(1) as f32);
        if let Some(p) = self.last() {
            p.options = options;
        }
        self
    }
    pub fn text(self, id: &'static str, label: &'static str, d: &str) -> B {
        self.push(id, label, Value::Text(d.into()), 0.0, 0.0)
    }
    pub fn resource(mut self, id: &'static str, label: &'static str) -> B {
        self = self.push(id, label, Value::Text(String::new()), 0.0, 0.0);
        if let Some(p) = self.last() {
            p.resource = true;
        }
        self
    }
    pub fn curve(self, id: &'static str, label: &'static str) -> B {
        self.push(id, label, Value::Curve(Curve::default()), 0.0, 1.0)
    }
    pub fn gradient(self, id: &'static str, label: &'static str, g: Gradient) -> B {
        self.push(id, label, Value::Gradient(g), 0.0, 1.0)
    }
    pub fn function(self, id: &'static str, label: &'static str, g: FnGraph) -> B {
        self.push(id, label, Value::Function(Box::new(g)), 0.0, 0.0)
    }
    pub fn cook(mut self, f: Cook) -> NodeDef {
        if self.0.outputs.is_empty() {
            self = self.any_out();
        }
        self.0.cook = f;
        self.0
    }
}

/// Every node definition, in library order.
pub fn catalog() -> &'static [NodeDef] {
    static CAT: OnceLock<Vec<NodeDef>> = OnceLock::new();
    CAT.get_or_init(|| {
        let mut v = Vec::new();
        v.extend(crate::atomic::defs());
        v.extend(crate::fx_map::defs());
        v.extend(crate::svg::defs());
        v.extend(crate::text::defs());
        v.extend(crate::generators::defs());
        v.extend(crate::noises_more::defs());
        v.extend(crate::noise3d::defs());
        v.extend(crate::patterns::defs());
        v.extend(crate::patterns_more::defs());
        v.extend(crate::filters::defs());
        v.extend(crate::filters_more::defs());
        v.extend(crate::filters_extra::defs());
        v.extend(crate::flood_fill::defs());
        v.extend(crate::material::defs());
        v.extend(crate::material_more::defs());
        v.extend(crate::weathering::defs());
        v
    })
}

pub fn find(id: &str) -> Option<&'static NodeDef> {
    catalog().iter().find(|d| d.id == id)
}

/// The library's category tree, in display order: (path, ids).
pub fn categories() -> Vec<(&'static str, Vec<&'static str>)> {
    let mut out: Vec<(&'static str, Vec<&'static str>)> = Vec::new();
    for d in catalog() {
        match out.iter_mut().find(|(c, _)| *c == d.category) {
            Some((_, ids)) => ids.push(d.id),
            None => out.push((d.category, vec![d.id])),
        }
    }
    out
}

/// How many slider spans the hard limit extends past each end of a parameter's range.
const HARD_SPAN: f32 = 16.0;

/// Clamp `v` to `[lo, hi]`; a non-finite `v` becomes `fallback`.
fn clamp_finite(v: f32, lo: f32, hi: f32, fallback: f32) -> f32 {
    if v.is_finite() { v.clamp(lo, hi) } else { fallback }
}

/// Clamp numeric parameters to hard limits: the declared slider range widened by
/// `HARD_SPAN` spans on each side (the editor lets values past the slider through, so the
/// slider range itself is not a limit). Non-finite values become the default. Keeps cooks
/// bounded whatever a file, binding, function or command supplies.
pub fn clamp_params(def: &NodeDef, params: &mut BTreeMap<String, Value>) {
    for p in &def.params {
        let Some(v) = params.get_mut(p.id) else { continue };
        if let Value::Enum(i) = v {
            let fallback = match &p.default {
                Value::Enum(d) => *d,
                _ => 0,
            };
            let bounds = if !p.options.is_empty() {
                let hi = u32::try_from(p.options.len() - 1).unwrap_or(u32::MAX);
                Some((0u32, hi))
            } else if p.min.is_finite() && p.max.is_finite() && p.min <= p.max {
                if p.min < 0.0 { None } else { Some((p.min as u32, p.max as u32)) }
            } else {
                None
            };
            match bounds {
                Some((lo, hi)) if lo <= hi => *i = (*i).clamp(lo, hi),
                Some(_) => *i = fallback,
                None => *i = fallback,
            }
            continue;
        }
        if let Value::Color(c) = v {
            for x in c {
                *x = clamp_finite(*x, -HARD_SPAN, 1.0 + HARD_SPAN, 0.0);
            }
            continue;
        }
        if !p.min.is_finite() || !p.max.is_finite() || p.min > p.max {
            // No usable range: fall back to the parameter default.
            match (&mut *v, &p.default) {
                (Value::Float(x), Value::Float(d)) => *x = *d,
                (Value::Float2(a), Value::Float2(d)) => *a = *d,
                (Value::Float3(a), Value::Float3(d)) => *a = *d,
                (Value::Float4(a), Value::Float4(d)) => *a = *d,
                (Value::Int(x), Value::Int(d)) => *x = *d,
                (Value::Int2(a), Value::Int2(d)) => *a = *d,
                _ => {}
            }
            continue;
        }
        let span = p.max - p.min;
        let (lo, hi) = (p.min - HARD_SPAN * span, p.max + HARD_SPAN * span);
        // The default's component `i`, or 0 when the default is a different variant.
        let fallback = |i: usize| match &p.default {
            Value::Float(d) => [*d].get(i).copied(),
            Value::Float2(d) => d.get(i).copied(),
            Value::Float3(d) => d.get(i).copied(),
            Value::Float4(d) => d.get(i).copied(),
            Value::Int(d) => Some(*d as f32),
            Value::Int2(d) => d.get(i).map(|x| *x as f32),
            Value::Enum(d) => Some(*d as f32),
            _ => None,
        };
        let clamp_at = |x: &mut f32, i: usize| *x = clamp_finite(*x, lo, hi, fallback(i).unwrap_or(0.0));
        let clamp_int = |x: &mut i32| *x = (*x as f32).clamp(lo, hi).round() as i32;
        match v {
            Value::Float(x) => clamp_at(x, 0),
            Value::Float2(a) => a.iter_mut().enumerate().for_each(|(i, x)| clamp_at(x, i)),
            Value::Float3(a) => a.iter_mut().enumerate().for_each(|(i, x)| clamp_at(x, i)),
            Value::Float4(a) => a.iter_mut().enumerate().for_each(|(i, x)| clamp_at(x, i)),
            Value::Int(x) => clamp_int(x),
            Value::Int2(a) => a.iter_mut().for_each(clamp_int),
            _ => {}
        }
    }
}

/// Parameter values for a node: catalog defaults overlaid with `overrides`.
pub fn resolve_params(def: &NodeDef, overrides: &BTreeMap<String, Value>) -> BTreeMap<String, Value> {
    let mut m: BTreeMap<String, Value> = def.params.iter().map(|p| (p.id.to_string(), p.default.clone())).collect();
    for (k, v) in overrides {
        // Keep only known parameters whose type matches (a hand-edited file may have anything).
        if let Some(d) = m.get(k)
            && d.type_name() == v.type_name()
        {
            m.insert(k.clone(), v.clone());
        }
    }
    clamp_params(def, &mut m);
    m
}

#[cfg(test)]
#[path = "catalog.test.rs"]
mod tests;
