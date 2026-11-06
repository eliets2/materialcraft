//! The 3D view's renderer: one ray per pixel (plus 2×2 supersampling when idle) against the
//! scene mesh's BVH, the material's maps sampled with tiling, then metallic/roughness GGX shading
//! under image-based light (SH diffuse, prefiltered specular, an analytic environment BRDF),
//! exposure and ACES tone mapping. The height output displaces real geometry ([`Scene::with_relief`]).

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::{Displacement, Relief};
use materialcraft_geom::glam::{Vec2, Vec3};
use materialcraft_geom::{Bvh, Mesh, OrbitCamera};
use materialcraft_image::{Image, linear_to_srgb};
use rayon::prelude::*;

use crate::ibl::{self, EnvImage};

/// The material's maps (any may be missing: defaults are mid gray, flat, rough 0.5, dielectric).
#[derive(Clone, Default)]
pub struct Maps {
    pub base_color: Option<Image>,
    pub normal: Option<Image>,
    pub roughness: Option<Image>,
    pub metallic: Option<Image>,
    pub height: Option<Image>,
    pub ao: Option<Image>,
    pub emissive: Option<Image>,
    pub opacity: Option<Image>,
}

impl Maps {
    /// Maps from `(usage id, image)` pairs (`baseColor`, `normal`, …).
    pub fn from_usages<'a>(items: impl IntoIterator<Item = (&'a str, &'a Image)>) -> Maps {
        let mut m = Maps::default();
        for (usage, img) in items {
            use materialcraft_core::MaterialChannel;
            let Some(channel) = MaterialChannel::from_id(usage) else { continue };

            let slot = match channel {
                MaterialChannel::BaseColor => &mut m.base_color,
                MaterialChannel::Normal => &mut m.normal,
                MaterialChannel::Roughness => &mut m.roughness,
                MaterialChannel::Metallic => &mut m.metallic,
                MaterialChannel::Height => &mut m.height,
                MaterialChannel::AmbientOcclusion => &mut m.ao,
                MaterialChannel::Emissive => &mut m.emissive,
                MaterialChannel::Opacity => &mut m.opacity,
            };
            if slot.is_none() {
                *slot = Some(img.clone());
            }
        }
        m
    }
}

/// A mesh ready for ray casts.
pub struct Scene {
    pub mesh: Arc<Mesh>,
    pub bvh: Arc<Bvh>,
    pub radius: f32,
    /// Center of the mesh bounds (the camera orbits it).
    pub center: Vec3,
}

impl Scene {
    pub fn new(mut mesh: Mesh) -> Scene {
        if mesh.normals.len() != mesh.positions.len() {
            mesh.compute_normals();
        }
        if mesh.tangents.len() != mesh.positions.len() {
            mesh.compute_tangents();
        }
        let bvh = Bvh::new(&mesh);
        let (lo, hi) = mesh.bounds();
        let center = (lo + hi) * 0.5;
        let radius = ((hi - lo).length() * 0.5).max(1e-3);
        Scene { mesh: Arc::new(mesh), bvh: Arc::new(bvh), radius, center }
    }

    /// A built-in scene (`rounded_cube`, `sphere`, …); unknown ids give the rounded cube.
    pub fn builtin(id: &str) -> Scene {
        Scene::new(builtin_mesh(id))
    }

    /// `base` subdivided and moved along its normals by `depth × (height − level)`; vertices at one spot move together so it stays closed.
    pub fn displaced(base: &Mesh, height: &Image, displacement: &Displacement) -> Scene {
        let mut mesh = base.clone();
        if mesh.normals.len() != mesh.positions.len() {
            mesh.compute_normals();
        }
        while mesh.triangle_count() > 0 && mesh.triangle_count() * 4 <= displacement.max_triangles {
            mesh = mesh.subdivided();
        }

        // Pre-filtered so detail finer than the mesh (left to the normal map) does not spike the silhouette
        let height = height.resize(height.w.min(512), height.h.min(512), true);
        let level = if displacement.level.is_finite() { displacement.level.clamp(0.0, 1.0) } else { 0.0 };
        let depth = if displacement.depth.is_finite() { displacement.depth } else { 0.0 };
        let tiling = if displacement.tiling.is_finite() { displacement.tiling.max(0.01) } else { 1.0 };

        // Vertices at the same spot (to 0.1 mm) are one point of the surface
        let spots: Vec<[i32; 3]> = mesh.positions.iter().map(|p| (*p * 1e4).round().as_ivec3().to_array()).collect();
        let mut spot_sums: HashMap<[i32; 3], (Vec3, f32, f32)> = HashMap::new();
        for ((spot, n), uv) in spots.iter().zip(&mesh.normals).zip(&mesh.uvs) {
            let (normal_sum, height_sum, count) = spot_sums.entry(*spot).or_default();
            *normal_sum += *n;
            *height_sum += height.sample_gray(uv.x * tiling, uv.y * tiling, true);
            *count += 1.0;
        }
        let offsets: Vec<Vec3> =
            spots.iter().map(|spot| spot_sums.get(spot).map_or(Vec3::ZERO, |(n, h, count)| n.normalize_or_zero() * depth * (h / count - level))).collect();

        if displacement.shade_relief {
            // Normals of the displaced surface, shared per spot so seams don't show
            let moved = |i: u32| mesh.positions.get(i as usize).zip(offsets.get(i as usize)).map(|(p, o)| *p + *o);
            let mut face_sums: HashMap<[i32; 3], Vec3> = HashMap::new();
            for triangle in mesh.indices.chunks_exact(3) {
                let (Some(a), Some(b), Some(c)) = (moved(triangle[0]), moved(triangle[1]), moved(triangle[2])) else { continue };
                let face = (b - a).cross(c - a);
                for spot in triangle.iter().filter_map(|&i| spots.get(i as usize)) {
                    *face_sums.entry(*spot).or_default() += face;
                }
            }
            for (spot, n) in spots.iter().zip(mesh.normals.iter_mut()) {
                if let Some(sum) = face_sums.get(spot).and_then(|s| s.try_normalize()) {
                    *n = sum;
                }
            }
            mesh.tangents.clear();
        }

        for (p, offset) in mesh.positions.iter_mut().zip(&offsets) {
            *p += *offset;
        }
        Scene::new(mesh)
    }

    /// The scene the 3D view shows for `maps`: displaced by the height output, or `None` when nothing moves.
    pub fn with_relief(&self, maps: &Maps, relief: Relief, tiling: f32, max_triangles: usize) -> Option<Scene> {
        let height = maps.height.as_ref()?;
        if !(relief.depth.is_finite() && relief.depth > 0.0) {
            return None;
        }

        // Without a normal map the displaced surface's own normals show the relief
        let displacement = Displacement { depth: relief.depth * self.radius, level: relief.level, tiling, max_triangles, shade_relief: maps.normal.is_none() };
        Some(Scene::displaced(&self.mesh, height, &displacement))
    }
}

/// Triangle budget of a displaced scene in the interactive 3D view (builds in about 0.2–0.3 s on two cores).
pub const VIEW_TRIANGLES: usize = 500_000;

/// A built-in mesh by id; unknown ids give the rounded cube.
pub fn builtin_mesh(id: &str) -> Mesh {
    let mut m = materialcraft_geom::primitives::builtin(id).unwrap_or_else(|| materialcraft_geom::primitives::rounded_box(Vec3::ONE, 0.15));
    if id == "sphere" {
        // The texture wraps twice around, so texels are square at the equator.
        m.uvs.iter_mut().for_each(|uv| uv.x *= 2.0);
    }
    m
}

/// Prefiltered environment lighting.
pub struct Lighting {
    pub name: String,
    pub specular: Vec<EnvImage>,
    pub sh: [[f32; 3]; 9],
}

impl Lighting {
    pub fn new(id: &str) -> Lighting {
        let env = ibl::environment(ibl::by_id(id), 256);
        Lighting { name: id.into(), specular: ibl::prefilter(&env, 6), sh: ibl::sh9(&env) }
    }

    fn diffuse(&self, n: Vec3) -> Vec3 {
        let b = ibl::sh_basis(n);
        let mut c = Vec3::ZERO;
        for k in 0..9 {
            c += Vec3::from(self.sh[k]) * b[k];
        }
        c.max(Vec3::ZERO)
    }

    fn specular(&self, r: Vec3, rough: f32) -> Vec3 {
        let levels = self.specular.len();
        if levels == 0 {
            return Vec3::ZERO;
        }
        let l = rough.clamp(0.0, 1.0) * (levels - 1) as f32;
        let i = (l.floor() as usize).min(levels - 1);
        let j = (i + 1).min(levels - 1);
        let (a, b) = (self.specular[i].sample(r), self.specular[j].sample(r));
        a.lerp(b, l - i as f32)
    }
}

/// Camera and look.
#[derive(Clone, Copy, Debug)]
pub struct Settings {
    /// Degrees.
    pub yaw: f32,
    pub pitch: f32,
    /// In scene radii.
    pub distance: f32,
    /// Orbit target offset from the scene center, in scene radii (the caller bounds it).
    pub target: [f32; 3],
    pub tiling: f32,
    /// Stops.
    pub exposure: f32,
    /// Degrees.
    pub env_rotation: f32,
    /// Interpret the green channel as DirectX (-Y) instead of OpenGL (+Y).
    pub normal_y_flip: bool,
    /// Show the (blurred) environment behind the mesh instead of the flat backdrop.
    pub show_env: bool,
    /// 1 = one ray per pixel, 2 = 2×2 supersampling.
    pub supersample: u32,
    /// Ray-traced ambient occlusion rays per sample (0 = off): darkens the crevices of a
    /// displaced scene.
    pub ao_rays: u32,
    /// Strength of a shadow-casting key light from the camera's upper left (0 = off).
    pub key_light: f32,
    /// See through the surface where the material's alpha (the opacity output, else the base
    /// color's alpha channel) is below one: what lies behind, back faces included, shows through.
    pub transparency: bool,
    /// Show the material's alpha channel on the mesh, unlit (white = opaque, black = clear).
    pub alpha_view: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            yaw: 35.0,
            pitch: 25.0,
            distance: 3.2,
            target: [0.0; 3],
            tiling: 1.0,
            exposure: 0.0,
            env_rotation: 0.0,
            normal_y_flip: false,
            show_env: false,
            supersample: 1,
            ao_rays: 0,
            key_light: 0.0,
            transparency: true,
            alpha_view: false,
        }
    }
}

/// The orbit camera for `s` around `scene`.
pub fn camera(scene: &Scene, s: &Settings) -> OrbitCamera {
    let target = scene.center + Vec3::from(s.target) * scene.radius;
    OrbitCamera {
        target: if target.is_finite() { target } else { scene.center },
        yaw: s.yaw.to_radians(),
        pitch: s.pitch.clamp(-89.0, 89.0).to_radians(),
        distance: s.distance.clamp(1.05, 50.0) * scene.radius,
        fov: 35.0,
        scene_radius: scene.radius,
        ortho: false,
    }
}

/// `s.target` after panning the view by (dx, dy) viewport heights: the scene follows the pointer.
/// Unbounded; non-finite parts become 0.
pub fn pan(scene: &Scene, s: &Settings, dx: f32, dy: f32) -> [f32; 3] {
    let mut cam = camera(scene, s);
    cam.pan(dx, dy);
    let t = (cam.target - scene.center) / scene.radius.max(f32::EPSILON);
    t.to_array().map(|v| if v.is_finite() { v } else { 0.0 })
}

#[inline]
fn tex(img: &Option<Image>, uv: Vec2) -> Option<[f32; 4]> {
    img.as_ref().map(|i| i.sample(uv.x, uv.y, true))
}

#[inline]
fn tex_gray(img: &Option<Image>, uv: Vec2, default: f32) -> f32 {
    img.as_ref().map_or(default, |i| i.sample_gray(uv.x, uv.y, true))
}

/// Narkowicz's fit of the ACES filmic curve.
#[inline]
fn aces(x: f32) -> f32 {
    ((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14)).clamp(0.0, 1.0)
}

/// Karis' analytic approximation of the split-sum environment BRDF: (scale, bias) for F0.
#[inline]
fn env_brdf(roughness: f32, ndv: f32) -> (f32, f32) {
    let c0 = [-1.0, -0.0275, -0.572, 0.022];
    let c1 = [1.0, 0.0425, 1.04, -0.04];
    let r = [roughness * c0[0] + c1[0], roughness * c0[1] + c1[1], roughness * c0[2] + c1[2], roughness * c0[3] + c1[3]];
    let a004 = (r[0] * r[0]).min((-9.28 * ndv).exp2()) * r[0] + r[1];
    (a004 * -1.04 + r[2], a004 * 1.04 + r[3])
}

fn rotate_y(v: Vec3, deg: f32) -> Vec3 {
    let (s, c) = deg.to_radians().sin_cos();
    Vec3::new(v.x * c + v.z * s, v.y, -v.x * s + v.z * c)
}

/// The material's alpha at `uv`: the opacity output, else the base color's alpha channel, else
/// opaque.
fn alpha(maps: &Maps, uv: Vec2) -> f32 {
    let a = match (&maps.opacity, &maps.base_color) {
        (Some(o), _) => o.sample_gray(uv.x, uv.y, true),
        (None, Some(b)) if !b.is_gray() => b.sample(uv.x, uv.y, true)[3],
        _ => 1.0,
    };
    if a.is_finite() { a.clamp(0.0, 1.0) } else { 1.0 }
}

/// Transparent surfaces a ray passes through before it gives up (what is left shows the
/// background).
const MAX_LAYERS: usize = 8;

/// Per-pixel inputs of [`shade`] beyond the camera ray.
struct Px {
    /// Direction toward the key light.
    key: Vec3,
    /// Random rotation of the ambient occlusion ray pattern.
    jitter: Vec2,
}

/// The fraction of `n`'s hemisphere at `p` that is open within `reach` (cosine-weighted).
fn ambient_occlusion(scene: &Scene, p: Vec3, n: Vec3, rays: u32, jitter: Vec2) -> f32 {
    let (t, b) = n.any_orthonormal_pair();
    let reach = scene.radius * 0.3;
    let open = (0..rays)
        .filter(|&i| {
            let u = (ibl::hammersley(i, rays) + jitter).fract();
            let (r, phi) = (u.x.sqrt(), u.y * std::f32::consts::TAU);
            let d = t * (r * phi.cos()) + b * (r * phi.sin()) + n * (1.0 - u.x).max(0.0).sqrt();
            !scene.bvh.occluded(p, d, reach)
        })
        .count();
    open as f32 / rays as f32
}

/// GGX specular plus Lambert diffuse from a white directional light along `l`.
fn direct(n: Vec3, v: Vec3, l: Vec3, base: Vec3, f0: Vec3, metal: f32, rough: f32) -> Vec3 {
    let ndl = n.dot(l);
    if ndl <= 0.0 {
        return Vec3::ZERO;
    }
    let ndv = n.dot(v).max(1e-4);
    let h = (v + l).normalize_or_zero();
    let (ndh, vdh) = (n.dot(h).max(0.0), v.dot(h).max(0.0));
    let a2 = (rough * rough).powi(2).max(1e-4);
    let d = a2 / (std::f32::consts::PI * (ndh * ndh * (a2 - 1.0) + 1.0).powi(2));
    let k = (rough + 1.0).powi(2) / 8.0;
    let g = ndl / (ndl * (1.0 - k) + k) * ndv / (ndv * (1.0 - k) + k);
    let f = f0 + (Vec3::ONE - f0) * (1.0 - vdh).powi(5);
    let spec = f * (d * g / (4.0 * ndv * ndl));
    let diff = (Vec3::ONE - f) * base * (1.0 - metal) / std::f32::consts::PI;
    (diff + spec) * ndl
}

/// A shaded surface point: linear color (or the alpha value in [`Settings::alpha_view`]), its
/// alpha and the ray distance.
struct Shaded {
    color: Vec3,
    alpha: f32,
    t: f32,
}

/// Shade the first surface along one ray; `None` = missed the mesh.
fn shade(scene: &Scene, maps: &Maps, light: &Lighting, s: &Settings, origin: Vec3, dir: Vec3, px: &Px) -> Option<Shaded> {
    let hit = scene.bvh.closest(origin, dir, f32::MAX)?;
    let m = &scene.mesh;
    let [i0, i1, i2] = m.tri(hit.tri as usize)?;
    let (w1, w2) = (hit.u, hit.v);
    let w0 = 1.0 - w1 - w2;
    let lerp3 = |a: Vec3, b: Vec3, c: Vec3| a * w0 + b * w1 + c * w2;
    let nrm = lerp3(*m.normals.get(i0)?, *m.normals.get(i1)?, *m.normals.get(i2)?).normalize_or_zero();
    // A back face (seen through a transparent front, or under an open mesh) faces the viewer.
    let nrm = if nrm.dot(dir) > 0.0 { -nrm } else { nrm };
    let (t0, t1, t2) = (*m.tangents.get(i0)?, *m.tangents.get(i1)?, *m.tangents.get(i2)?);
    let tan = (t0.truncate() * w0 + t1.truncate() * w1 + t2.truncate() * w2).normalize_or_zero();
    let sign = if t0.w < 0.0 { -1.0 } else { 1.0 };
    let tan = (tan - nrm * nrm.dot(tan)).normalize_or_zero();
    let bit = nrm.cross(tan) * sign;
    let uv0 = *m.uvs.get(i0)? * w0 + *m.uvs.get(i1)? * w1 + *m.uvs.get(i2)? * w2;
    let uv = uv0 * s.tiling;
    let v = -dir;
    let a = alpha(maps, uv);
    if s.alpha_view {
        return Some(Shaded { color: Vec3::splat(a), alpha: 1.0, t: hit.t });
    }
    let base = tex(&maps.base_color, uv).map_or(Vec3::splat(0.5), |c| Vec3::new(c[0], c[1], c[2]));
    let rough = tex_gray(&maps.roughness, uv, 0.5).clamp(0.02, 1.0);
    let metal = tex_gray(&maps.metallic, uv, 0.0).clamp(0.0, 1.0);
    let ao = tex_gray(&maps.ao, uv, 1.0).clamp(0.0, 1.0);
    let n = match tex(&maps.normal, uv) {
        Some(c) => {
            let mut t = Vec3::new(c[0] * 2.0 - 1.0, c[1] * 2.0 - 1.0, c[2] * 2.0 - 1.0);
            if s.normal_y_flip {
                t.y = -t.y;
            }
            // OpenGL convention: green points along +v in texture space, which is -bitangent
            // with top-left UVs.
            (tan * t.x - bit * t.y + nrm * t.z.max(0.05)).normalize_or_zero()
        }
        None => nrm,
    };
    let n = if n.dot(v) < 0.0 { nrm } else { n };
    let ndv = n.dot(v).clamp(1e-4, 1.0);
    let f0 = Vec3::splat(0.04).lerp(base, metal);
    let diffuse_col = base * (1.0 - metal);
    let rot = |d: Vec3| rotate_y(d, -s.env_rotation);
    let r = (n * (2.0 * n.dot(v)) - v).normalize_or_zero();
    let (sa, sb) = env_brdf(rough, ndv);
    let spec = light.specular(rot(r), rough) * (f0 * sa + Vec3::splat(sb));
    let diff = light.diffuse(rot(n)) * diffuse_col;
    let mut c = (diff + spec) * ao;
    if s.ao_rays > 0 || s.key_light > 0.0 {
        // The true surface (a displaced scene's facets), offset to avoid self-hits.
        let (p0, p1, p2) = (*m.positions.get(i0)?, *m.positions.get(i1)?, *m.positions.get(i2)?);
        let face = (p1 - p0).cross(p2 - p0).normalize_or_zero();
        let face = if face.dot(nrm) < 0.0 { -face } else { face };
        let p = origin + dir * hit.t + face * scene.radius * 2e-4;
        if s.ao_rays > 0 {
            c *= ambient_occlusion(scene, p, face, s.ao_rays, px.jitter);
        }
        if s.key_light > 0.0 && face.dot(px.key) > 0.0 && !scene.bvh.occluded(p, px.key, f32::MAX) {
            c += direct(n, v, px.key, base, f0, metal, rough) * s.key_light * ao;
        }
    }
    if let Some(e) = tex(&maps.emissive, uv) {
        c += Vec3::new(e[0], e[1], e[2]);
    }
    Some(Shaded { color: c, alpha: if s.transparency { a } else { 1.0 }, t: hit.t })
}

/// Follow one camera ray through up to [`MAX_LAYERS`] transparent surfaces, compositing front to
/// back (`miss` is what shows behind the mesh). Shaded colors are scaled by the exposure `k`.
#[allow(clippy::too_many_arguments)]
fn trace(scene: &Scene, maps: &Maps, light: &Lighting, s: &Settings, origin: Vec3, dir: Vec3, px: &Px, k: f32, miss: Vec3) -> Vec3 {
    let mut acc = Vec3::ZERO;
    let mut through = 1.0;
    let mut o = origin;
    let step = scene.radius * 1e-4;
    for _ in 0..MAX_LAYERS {
        let Some(h) = shade(scene, maps, light, s, o, dir, px) else { return acc + miss * through };
        acc += h.color * k * h.alpha * through;
        through *= 1.0 - h.alpha;
        if through < 1e-3 {
            return acc;
        }
        o += dir * (h.t + step);
    }
    acc + miss * through
}

/// The studio backdrop at screen position `p` (−1..1): a soft vignette (linear).
fn backdrop(p: Vec2) -> Vec3 {
    let t = (p.length() * 0.75).clamp(0.0, 1.0);
    Vec3::splat(0.04).lerp(Vec3::splat(0.012), t * t * (3.0 - 2.0 * t))
}

/// A cheap integer hash to 0..1 pairs (the per-sample AO pattern rotation).
fn jitter(x: u32, y: u32) -> Vec2 {
    let mut h = x.wrapping_mul(0x9E37_79B1) ^ y.wrapping_mul(0x85EB_CA77);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    Vec2::new((h & 0xFFFF) as f32 / 65536.0, (h >> 16) as f32 / 65536.0)
}

/// Render `w × h` pixels to sRGB RGBA8.
pub fn render(scene: &Scene, maps: &Maps, light: &Lighting, s: &Settings, w: u32, h: u32) -> Vec<u8> {
    let (w, h) = (w.clamp(1, 4096), h.clamp(1, 4096));
    render_cancellable(scene, maps, light, s, w, h, &AtomicBool::new(false)).unwrap_or_else(|| vec![0; (w * h * 4) as usize])
}

/// [`render`] that gives up once `cancel` is set (rows not yet started are skipped) and then
/// returns `None`: a frame the viewer no longer needs (the camera moved, the view was resized)
/// stops holding the CPU.
pub fn render_cancellable(scene: &Scene, maps: &Maps, light: &Lighting, s: &Settings, w: u32, h: u32, cancel: &AtomicBool) -> Option<Vec<u8>> {
    let (w, h) = (w.clamp(1, 4096), h.clamp(1, 4096));
    let cam = camera(scene, s);
    let aspect = w as f32 / h as f32;
    let ss = s.supersample.clamp(1, 4);
    let k = 2f32.powf(s.exposure);
    let bg_env = light.specular.get(3).or(light.specular.last());
    // Key light from the camera's upper left, a little in front of the subject.
    let fwd = cam.ray(Vec2::ZERO, aspect).dir;
    let right = fwd.cross(Vec3::Y).normalize_or(Vec3::X);
    let key = (right * -0.6 + right.cross(fwd) * 0.75 - fwd * 0.5).normalize_or_zero();
    let mut out = vec![0u8; (w * h * 4) as usize];
    out.par_chunks_mut(w as usize * 4).enumerate().for_each(|(y, row)| {
        if cancel.load(Ordering::Relaxed) {
            return;
        }
        for x in 0..w as usize {
            let mut acc = Vec3::ZERO;
            for sy in 0..ss {
                for sx in 0..ss {
                    let px = (x as f32 + (sx as f32 + 0.5) / ss as f32) / w as f32 * 2.0 - 1.0;
                    let py = 1.0 - (y as f32 + (sy as f32 + 0.5) / ss as f32) / h as f32 * 2.0;
                    let ray = cam.ray(Vec2::new(px, py), aspect);
                    let bg = backdrop(Vec2::new(px, py));
                    let p = Px { key, jitter: jitter(x as u32 * ss + sx, y as u32 * ss + sy) };
                    let miss = match (s.show_env, bg_env) {
                        (true, Some(e)) => e.sample(rotate_y(ray.dir, -s.env_rotation)) * k,
                        _ => bg,
                    };
                    acc += if s.alpha_view {
                        // Display values: the alpha as is, the background tone mapped.
                        match shade(scene, maps, light, s, ray.origin, ray.dir, &p) {
                            Some(h) => h.color,
                            None => miss.map(|v| linear_to_srgb(aces(v))),
                        }
                    } else {
                        trace(scene, maps, light, s, ray.origin, ray.dir, &p, k, miss)
                    };
                }
            }
            let c = acc / (ss * ss) as f32;
            let q = |v: f32| {
                let d = if s.alpha_view { v } else { linear_to_srgb(aces(v)) };
                (d.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
            };
            if let Some(px) = row.get_mut(x * 4..x * 4 + 4) {
                px.copy_from_slice(&[q(c.x), q(c.y), q(c.z), 255]);
            }
        }
    });
    if cancel.load(Ordering::Relaxed) { None } else { Some(out) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use materialcraft_image::Kind;

    #[test]
    fn renders_a_material_on_every_scene() {
        let light = Lighting::new("studio");
        let maps = Maps {
            base_color: Some(Image::filled(16, 16, Kind::Color, [0.8, 0.1, 0.1, 1.0])),
            roughness: Some(Image::filled(16, 16, Kind::Gray, [0.3; 4])),
            height: Some(Image::gray(16, 16, |x, _| (x % 4) as f32 / 4.0)),
            ..Maps::default()
        };
        for id in ["rounded_cube", "sphere", "plane", "cylinder", "torus", "material_ball", "cube"] {
            let scene = Scene::builtin(id);
            let px = render(&scene, &maps, &light, &Settings::default(), 64, 48);
            assert_eq!(px.len(), 64 * 48 * 4);
            // The center hits the mesh (the torus' hole is left of it): reddish.
            let c = if id == "torus" { ((24 * 64 + 20) * 4) as usize } else { ((24 * 64 + 32) * 4) as usize };
            assert!(px[c] > px[c + 1], "{id}: center is not red ({:?})", &px[c..c + 3]);
        }
    }

    #[test]
    fn alpha_shows_what_lies_behind_and_the_alpha_view_shows_the_channel() {
        let (scene, light) = (Scene::builtin("sphere"), Lighting::new("studio"));
        let red = |a: f32| Maps { base_color: Some(Image::filled(16, 16, Kind::Color, [0.8, 0.1, 0.1, a])), ..Maps::default() };
        let c = ((24 * 64 + 32) * 4) as usize;
        let s = Settings::default();
        let opaque = render(&scene, &red(1.0), &light, &s, 64, 48);
        let clear = render(&scene, &red(0.0), &light, &s, 64, 48);
        assert!(opaque[c] > opaque[c + 1] + 40, "opaque center is red: {:?}", &opaque[c..c + 3]);
        // Fully clear: the backdrop shows through both the front and the back of the sphere.
        let bg = backdrop(Vec2::new(32.5 / 64.0 * 2.0 - 1.0, 1.0 - 24.5 / 48.0 * 2.0));
        let bg = (linear_to_srgb(aces(bg.x)) * 255.0 + 0.5) as u8;
        assert!(clear[c..c + 3].iter().all(|v| v.abs_diff(bg) <= 1), "clear {:?} vs backdrop {bg}", &clear[c..c + 3]);
        // Transparency off: alpha is ignored.
        assert_eq!(render(&scene, &red(0.0), &light, &Settings { transparency: false, ..s }, 64, 48), opaque);
        // An opacity output wins over the base color's alpha.
        let half = Maps { opacity: Some(Image::filled(16, 16, Kind::Gray, [0.5; 4])), ..red(1.0) };
        let px = render(&scene, &half, &light, &Settings { alpha_view: true, ..s }, 64, 48);
        assert!(px[c].abs_diff(128) <= 1 && px[c] == px[c + 1], "alpha view: {:?}", &px[c..c + 3]);
        let px = render(&scene, &red(1.0), &light, &Settings { alpha_view: true, ..s }, 64, 48);
        assert_eq!(&px[c..c + 3], &[255, 255, 255]);
    }

    fn flat(value: f32) -> Image {
        Image::filled(8, 8, Kind::Gray, [value; 4])
    }

    fn displacement(depth: f32, level: f32) -> Displacement {
        Displacement { depth, level, tiling: 1.0, max_triangles: 2048, shade_relief: false }
    }

    /// Heights of a displaced plane's vertices (it faces +y and starts at y = 0).
    fn plane_heights(height: &Image, displacement: &Displacement) -> Vec<f32> {
        let plane = materialcraft_geom::primitives::plane(2.0, 4);
        Scene::displaced(&plane, height, displacement).mesh.positions.iter().map(|p| p.y).collect()
    }

    /// At level 0 the relief rises out of the surface: black stays on it and nothing sinks in.
    #[test]
    fn the_relief_rises_out_of_the_surface() {
        let near = |heights: Vec<f32>, expected: f32| heights.iter().all(|y| (y - expected).abs() < 1e-5);

        assert!(near(plane_heights(&flat(0.0), &displacement(0.2, 0.0)), 0.0), "black stays on the surface");
        assert!(near(plane_heights(&flat(1.0), &displacement(0.2, 0.0)), 0.2), "white rises by the depth");
        assert!(near(plane_heights(&flat(0.5), &displacement(0.2, 0.5)), 0.0), "the level height stays on the surface");
        assert!(near(plane_heights(&flat(0.0), &displacement(0.2, 0.5)), -0.1), "below the level sinks");
        assert!(near(plane_heights(&flat(1.0), &displacement(0.2, f32::NAN)), 0.2), "a broken level reads as 0");
    }

    /// Vertices at one spot (UV seams, hard edges) move together, so a displaced cube stays closed.
    #[test]
    fn a_displaced_cube_stays_closed() {
        // No subdivision, so vertex i of the result is vertex i of the cube
        let cube = builtin_mesh("cube");
        let ramp = Image::gray(32, 32, |x, y| ((x + y) % 32) as f32 / 31.0);
        let displaced = Scene::displaced(&cube, &ramp, &Displacement { max_triangles: 0, ..displacement(0.1, 0.0) });
        assert_eq!(displaced.mesh.positions.len(), cube.positions.len());

        let spot = |p: Vec3| (p * 1e4).round().as_ivec3().to_array();
        let mut moved_to: HashMap<[i32; 3], Vec3> = HashMap::new();
        for (before, after) in cube.positions.iter().zip(&displaced.mesh.positions) {
            let first = *moved_to.entry(spot(*before)).or_insert(*after);
            assert!((first - *after).length() < 1e-4, "{before:?} split into {first:?} and {after:?}");
        }
    }

    /// Without a normal map the displaced surface's own normals shade the relief; with one, the original normals stay.
    #[test]
    fn relief_normals_follow_the_displaced_surface_only_when_asked() {
        let plane = materialcraft_geom::primitives::plane(2.0, 8);
        let slope = Image::gray(16, 16, |x, _| x as f32 / 15.0);
        let tilted = |shade_relief| {
            let scene = Scene::displaced(&plane, &slope, &Displacement { shade_relief, ..displacement(0.5, 0.0) });
            scene.mesh.normals.iter().map(|n| n.x.abs()).fold(0.0f32, f32::max)
        };

        assert!(tilted(true) > 0.1, "a slope tilts the normals");
        assert!(tilted(false) < 1e-5, "the original normals are kept");
    }

    #[test]
    fn with_relief_needs_a_height_output_and_a_depth() {
        let scene = Scene::builtin("plane");
        let maps = Maps { height: Some(flat(1.0)), ..Maps::default() };
        let relief = Relief { depth: 0.1, level: 0.0 };

        assert!(scene.with_relief(&Maps::default(), relief, 1.0, 2048).is_none());
        for depth in [0.0, -1.0, f32::NAN] {
            assert!(scene.with_relief(&maps, Relief { depth, ..relief }, 1.0, 2048).is_none(), "{depth}");
        }

        // The depth is in scene radii
        let raised = scene.with_relief(&maps, relief, 1.0, 2048).unwrap();
        let top = raised.mesh.positions.iter().map(|p| p.y).fold(f32::MIN, f32::max);
        assert!((top - 0.1 * scene.radius).abs() < 1e-4, "{top} vs {}", 0.1 * scene.radius);
    }

    #[test]
    fn a_cancelled_render_gives_up_and_an_uncancelled_one_matches_render() {
        let (scene, light, maps, s) = (Scene::builtin("sphere"), Lighting::new("studio"), Maps::default(), Settings::default());
        assert!(render_cancellable(&scene, &maps, &light, &s, 32, 24, &AtomicBool::new(true)).is_none());
        let px = render_cancellable(&scene, &maps, &light, &s, 32, 24, &AtomicBool::new(false));
        assert_eq!(px, Some(render(&scene, &maps, &light, &s, 32, 24)));
    }

    #[test]
    fn panning_moves_the_scene_with_the_pointer() {
        let scene = Scene::builtin("sphere");
        let s = Settings::default();
        let at = |s: &Settings| camera(&scene, s).project(scene.center, 1.0).unwrap();
        // Dragging right and down by a tenth of the view moves the center right and down by
        // the same fraction (NDC spans two view heights, y up).
        let right = Settings { target: pan(&scene, &s, 0.1, 0.0), ..s };
        let p = at(&right);
        assert!((p.x - 0.2).abs() < 1e-3 && p.y.abs() < 1e-3, "{p:?}");
        let down = Settings { target: pan(&scene, &s, 0.0, 0.1), ..s };
        let p = at(&down);
        assert!(p.x.abs() < 1e-3 && (p.y + 0.2).abs() < 1e-3, "{p:?}");
        // Orbiting turns around the panned target, not the scene center.
        let turned = Settings { yaw: s.yaw + 90.0, ..right };
        let cam = camera(&scene, &turned);
        assert!((cam.target - camera(&scene, &right).target).length() < 1e-5);
        // Panning back returns home; hostile inputs stay finite.
        let back = pan(&scene, &right, -0.1, 0.0);
        assert!(back.iter().all(|v| v.abs() < 1e-4), "{back:?}");
        let far = pan(&scene, &s, 1e9, f32::NAN);
        assert!(far.iter().all(|v| v.is_finite()), "{far:?}");
        for target in [[f32::NAN, 0.0, 0.0], [0.0, f32::INFINITY, 0.0], [0.0, 0.0, f32::MAX]] {
            let cam = camera(&scene, &Settings { target, ..s });
            assert_eq!(cam.target, scene.center, "{target:?}");
            assert!(cam.eye().is_finite());
        }
    }
}
