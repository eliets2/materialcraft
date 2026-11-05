//! Meshes for the 3D view: OBJ and glTF/GLB (self-contained: embedded or data-URI buffers),
//! normalized to fit a unit sphere at the origin, with normals, tangents and (when the file has
//! none) generated UVs.

use materialcraft_geom::Mesh;
use materialcraft_geom::glam::{Vec2, Vec3};

use crate::{FormatError, Result};

/// Mesh file extensions [`load_mesh`] reads.
pub const MESH_EXTENSIONS: &[&str] = &["obj", "glb", "gltf"];

/// Largest mesh accepted (triangles).
const MAX_TRIANGLES: usize = 4_000_000;

fn bad(msg: impl Into<String>) -> FormatError {
    FormatError::BadPackage(msg.into())
}

/// Append the triangles of `ix` to `out`, offset by `base`. Whole triangles are dropped when any
/// of their indices is `>= n` (or when a trailing partial triangle is left over), so the
/// triangles that follow keep their vertices.
fn push_triangles(out: &mut Vec<u32>, ix: impl IntoIterator<Item = u32>, n: usize, base: u32) -> Result<()> {
    append_triangles(out, ix, n, base, MAX_TRIANGLES)
}

fn append_triangles(out: &mut Vec<u32>, ix: impl IntoIterator<Item = u32>, n: usize, base: u32, limit: usize) -> Result<()> {
    let mut ix = ix.into_iter();
    while let (Some(a), Some(b), Some(c)) = (ix.next(), ix.next(), ix.next()) {
        let t = [a, b, c];
        if t.iter().all(|i| (*i as usize) < n) {
            if out.len() / 3 >= limit {
                return Err(bad("the mesh is too large"));
            }
            out.extend(t.iter().map(|i| i.saturating_add(base)));
        }
    }
    Ok(())
}

/// Build the mesh, rejecting non-finite positions, normals and UVs before anything computes
/// normals, tangents or bounds from them.
fn finish(positions: Vec<Vec3>, normals: Option<Vec<Vec3>>, uvs: Vec<Vec2>, indices: Vec<u32>) -> Result<Mesh> {
    if positions.iter().any(|p| !p.is_finite()) {
        return Err(bad("the mesh has non-finite (NaN or infinite) positions"));
    }
    if normals.iter().flatten().any(|n| !n.is_finite()) {
        return Err(bad("the mesh has non-finite (NaN or infinite) normals"));
    }
    if uvs.iter().any(|u| !u.is_finite()) {
        return Err(bad("the mesh has non-finite (NaN or infinite) UVs"));
    }
    Mesh::from_parts(positions, normals, uvs, indices, "material").map_err(|e| bad(e.to_string()))
}

/// Parse a mesh file by extension.
pub fn load_mesh(name: &str, bytes: &[u8]) -> Result<Mesh> {
    let ext = name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
    let mut mesh = match ext.as_str() {
        "obj" => obj(bytes)?,
        "glb" | "gltf" => gltf(bytes)?,
        other => return Err(bad(format!("unsupported mesh format `{other}` (OBJ, glTF or GLB)"))),
    };
    if mesh.indices.is_empty() || mesh.positions.is_empty() {
        return Err(bad("the mesh has no triangles"));
    }
    if mesh.triangle_count() > MAX_TRIANGLES {
        return Err(bad(format!("the mesh has {} triangles (limit {MAX_TRIANGLES})", mesh.triangle_count())));
    }
    if mesh.uvs.len() != mesh.positions.len() || mesh.uvs.iter().all(|u| *u == mesh.uvs[0]) {
        // No usable UVs: a box projection keeps materials tiling across the surface.
        mesh.uvs = box_uvs(&mesh);
    }
    mesh.normalize_to(1.0);
    if mesh.normals.len() != mesh.positions.len() {
        mesh.compute_normals();
    }
    mesh.compute_tangents();
    mesh.validate().map_err(|e| bad(e.to_string()))?;
    Ok(mesh)
}

fn obj(bytes: &[u8]) -> Result<Mesh> {
    let mut reader = std::io::BufReader::new(bytes);
    let opts = tobj::LoadOptions { triangulate: true, single_index: true, ..Default::default() };
    let (models, _) = crate::guard(
        || tobj::load_obj_buf(&mut reader, &opts, |_| Ok((Vec::new(), Default::default()))).map_err(|e| bad(format!("OBJ: {e}"))),
        || bad("the OBJ parser failed on this file"),
    )?;
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();
    for m in models {
        let base = positions.len() as u32;
        let mesh = &m.mesh;
        let n = mesh.positions.len() / 3;
        for i in 0..n {
            positions.push(Vec3::new(mesh.positions[i * 3], mesh.positions[i * 3 + 1], mesh.positions[i * 3 + 2]));
            if mesh.normals.len() == mesh.positions.len() {
                normals.push(Vec3::new(mesh.normals[i * 3], mesh.normals[i * 3 + 1], mesh.normals[i * 3 + 2]));
            }
            // OBJ UVs have their origin at the bottom left.
            uvs.push(mesh.texcoords.get(i * 2..i * 2 + 2).map_or(Vec2::ZERO, |t| Vec2::new(t[0], 1.0 - t[1])));
        }
        push_triangles(&mut indices, mesh.indices.iter().copied(), n, base)?;
        if indices.len() / 3 > MAX_TRIANGLES {
            return Err(bad("the mesh is too large"));
        }
    }
    let normals = (normals.len() == positions.len()).then_some(normals);
    finish(positions, normals, uvs, indices)
}

fn gltf(bytes: &[u8]) -> Result<Mesh> {
    let (doc, buffers, _) = crate::guard(|| gltf::import_slice(bytes).map_err(|e| bad(format!("glTF: {e}"))), || bad("the glTF parser failed on this file"))?;
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();
    let mut all_normals = true;
    for node in doc.nodes() {
        let Some(mesh) = node.mesh() else { continue };
        let m = materialcraft_geom::glam::Mat4::from_cols_array_2d(&node.transform().matrix());
        for prim in mesh.primitives() {
            if prim.mode() != gltf::mesh::Mode::Triangles {
                continue;
            }
            let reader = prim.reader(|b| buffers.get(b.index()).map(|d| d.0.as_slice()));
            let Some(pos) = reader.read_positions() else { continue };
            let base = positions.len() as u32;
            let pos: Vec<Vec3> = pos.map(|p| m.transform_point3(Vec3::from(p))).collect();
            let n = pos.len();
            match reader.read_normals() {
                Some(ns) => {
                    for q in ns {
                        let v = m.transform_vector3(Vec3::from(q));
                        if !v.is_finite() {
                            return Err(bad("the mesh has non-finite (NaN or infinite) normals"));
                        }
                        normals.push(v.normalize_or_zero());
                    }
                }
                None => all_normals = false,
            }
            match reader.read_tex_coords(0) {
                Some(t) => uvs.extend(t.into_f32().map(Vec2::from)),
                None => uvs.extend(std::iter::repeat_n(Vec2::ZERO, n)),
            }
            match reader.read_indices() {
                Some(ix) => push_triangles(&mut indices, ix.into_u32(), n, base)?,
                None => push_triangles(&mut indices, 0..n as u32, n, base)?,
            }
            positions.extend(pos);
            if indices.len() / 3 > MAX_TRIANGLES {
                return Err(bad("the mesh is too large"));
            }
        }
    }
    let normals = (all_normals && normals.len() == positions.len()).then_some(normals);
    finish(positions, normals, uvs, indices)
}

/// Box-projected UVs: each vertex takes the plane of its normal's largest axis.
fn box_uvs(mesh: &Mesh) -> Vec<Vec2> {
    let (lo, hi) = mesh.bounds();
    let size = (hi - lo).max_element().max(1e-6);
    mesh.positions
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let q = (*p - lo) / size;
            let n = mesh.normals.get(i).copied().unwrap_or(Vec3::Y).abs();
            if n.x >= n.y && n.x >= n.z {
                Vec2::new(q.z, 1.0 - q.y)
            } else if n.y >= n.z {
                Vec2::new(q.x, q.z)
            } else {
                Vec2::new(q.x, 1.0 - q.y)
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn obj_quad_loads_with_uvs_and_tangents() {
        let obj = b"v 0 0 0\nv 1 0 0\nv 1 1 0\nv 0 1 0\nvt 0 0\nvt 1 0\nvt 1 1\nvt 0 1\nf 1/1 2/2 3/3 4/4\n";
        let m = load_mesh("quad.obj", obj).unwrap();
        assert_eq!(m.triangle_count(), 2);
        assert_eq!(m.tangents.len(), m.positions.len());
        assert!(m.radius() <= 1.0 + 1e-4);
        // A mesh without UVs gets projected ones.
        let m = load_mesh("tri.obj", b"v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\n").unwrap();
        assert!(m.uvs.iter().any(|u| *u != m.uvs[0]));
    }

    /// A glTF with 4 vertices (positions given by `pos`, optional UVs) and the given indices.
    fn gltf_json(pos: [[f32; 3]; 4], uvs: Option<[[f32; 2]; 4]>, idx: &[u16]) -> Vec<u8> {
        use base64::Engine as _;
        let mut buf: Vec<u8> = pos.iter().flatten().flat_map(|f| f.to_le_bytes()).collect();
        let uv_off = buf.len();
        if let Some(uvs) = uvs {
            buf.extend(uvs.iter().flatten().flat_map(|f| f.to_le_bytes()));
        }
        let idx_off = buf.len();
        buf.extend(idx.iter().flat_map(|i| i.to_le_bytes()));
        let b64 = base64::engine::general_purpose::STANDARD.encode(&buf);
        let uv_attr = if uvs.is_some() { r#","TEXCOORD_0":1"# } else { "" };
        let uv_view = if uvs.is_some() { format!(r#"{{"buffer":0,"byteOffset":{uv_off},"byteLength":32}},"#) } else { String::new() };
        let uv_acc = if uvs.is_some() { r#"{"bufferView":1,"componentType":5126,"count":4,"type":"VEC2"},"# } else { "" };
        // View/accessor layout: with UVs, views are [pos, uv, idx] and accessors [pos, uv, idx].
        let idx_view = if uvs.is_some() { 2 } else { 1 };
        let idx_acc = if uvs.is_some() { 2 } else { 1 };
        format!(
            r#"{{"asset":{{"version":"2.0"}},"scene":0,"scenes":[{{"nodes":[0]}}],"nodes":[{{"mesh":0}}],
"meshes":[{{"primitives":[{{"attributes":{{"POSITION":0{uv_attr}}},"indices":{idx_acc}}}]}}],
"accessors":[{{"bufferView":0,"componentType":5126,"count":4,"type":"VEC3","min":[-1e9,-1e9,-1e9],"max":[1e9,1e9,1e9]}},{uv_acc}{{"bufferView":{idx_view},"componentType":5123,"count":{},"type":"SCALAR"}}],
"bufferViews":[{{"buffer":0,"byteOffset":0,"byteLength":48}},{uv_view}{{"buffer":0,"byteOffset":{idx_off},"byteLength":{}}}],
"buffers":[{{"byteLength":{},"uri":"data:application/octet-stream;base64,{b64}"}}]}}"#,
            idx.len(),
            idx.len() * 2,
            buf.len()
        )
        .into_bytes()
    }

    const QUAD: [[f32; 3]; 4] = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0]];

    #[test]
    fn triangle_limit_stops_a_stream_before_collecting_the_rest() {
        let mut out = Vec::new();
        let mut read = 0;
        let indices = std::iter::from_fn(|| {
            read += 1;
            assert!(read <= 6, "read beyond the first triangle over the limit");
            Some(0)
        });
        assert!(append_triangles(&mut out, indices, 1, 0, 1).is_err());
        assert_eq!(out, [0, 0, 0]);
        assert_eq!(read, 6);
    }

    #[test]
    fn obj_non_finite_normals_are_rejected() {
        assert!(load_mesh("x.obj", b"v 0 0 0\nv 1 0 0\nv 0 1 0\nvn nan 0 1\nf 1//1 2//1 3//1\n").is_err());
    }

    #[test]
    fn gltf_non_finite_normals_are_rejected() {
        use base64::Engine as _;
        let gltf_with_normals = |normals: [[f32; 3]; 4]| {
            let idx: [u16; 6] = [0, 1, 2, 0, 2, 3];
            let mut buf: Vec<u8> = QUAD.iter().flatten().flat_map(|f| f.to_le_bytes()).collect();
            buf.extend(normals.iter().flatten().flat_map(|f| f.to_le_bytes()));
            buf.extend(idx.iter().flat_map(|i| i.to_le_bytes()));
            let b64 = base64::engine::general_purpose::STANDARD.encode(&buf);
            format!(
                r#"{{"asset":{{"version":"2.0"}},"scene":0,"scenes":[{{"nodes":[0]}}],"nodes":[{{"mesh":0}}],
"meshes":[{{"primitives":[{{"attributes":{{"POSITION":0,"NORMAL":1}},"indices":2}}]}}],
"accessors":[{{"bufferView":0,"componentType":5126,"count":4,"type":"VEC3","min":[-1e9,-1e9,-1e9],"max":[1e9,1e9,1e9]}},{{"bufferView":1,"componentType":5126,"count":4,"type":"VEC3"}},{{"bufferView":2,"componentType":5123,"count":6,"type":"SCALAR"}}],
"bufferViews":[{{"buffer":0,"byteOffset":0,"byteLength":48}},{{"buffer":0,"byteOffset":48,"byteLength":48}},{{"buffer":0,"byteOffset":96,"byteLength":12}}],
"buffers":[{{"byteLength":{},"uri":"data:application/octet-stream;base64,{b64}"}}]}}"#,
                buf.len()
            )
            .into_bytes()
        };
        let mut nan_normals = [[0.0f32, 0.0, 1.0]; 4];
        nan_normals[2][0] = f32::NAN;
        assert!(load_mesh("x.gltf", &gltf_with_normals(nan_normals)).is_err());
        let mut inf_normals = [[0.0f32, 0.0, 1.0]; 4];
        inf_normals[1][2] = f32::INFINITY;
        assert!(load_mesh("x.gltf", &gltf_with_normals(inf_normals)).is_err());
    }

    #[test]
    fn a_bad_index_drops_its_whole_triangle_and_later_ones_keep_their_vertices() {
        // Triangles: (0,1,2), (0,1,9) out of range, (0,2,3).
        let m = load_mesh("x.gltf", &gltf_json(QUAD, None, &[0, 1, 2, 0, 1, 9, 0, 2, 3])).unwrap();
        assert_eq!(m.indices, vec![0, 1, 2, 0, 2, 3]);
        // An index count that is not a multiple of 3 drops only the partial triangle.
        let m = load_mesh("x.gltf", &gltf_json(QUAD, None, &[0, 1, 2, 0, 2, 3, 1, 2])).unwrap();
        assert_eq!(m.indices, vec![0, 1, 2, 0, 2, 3]);
    }

    #[test]
    fn non_finite_positions_normals_and_uvs_are_rejected_before_processing() {
        let mut nan_pos = QUAD;
        nan_pos[2][0] = f32::NAN;
        let e = load_mesh("x.gltf", &gltf_json(nan_pos, None, &[0, 1, 2, 0, 2, 3])).unwrap_err();
        assert!(e.to_string().contains("non-finite"), "{e}");
        let mut nan_uv = [[0.0f32, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
        nan_uv[1][1] = f32::NAN;
        let e = load_mesh("x.gltf", &gltf_json(QUAD, Some(nan_uv), &[0, 1, 2, 0, 2, 3])).unwrap_err();
        assert!(e.to_string().contains("non-finite"), "{e}");
        assert!(load_mesh("x.obj", b"v 0 0 0\nv 1 0 0\nv 0 1 0\nvt nan 0\nvt 1 0\nvt 0 1\nf 1/1 2/2 3/3\n").is_err());
    }

    #[test]
    fn bad_meshes_are_errors() {
        assert!(load_mesh("x.obj", b"").is_err());
        assert!(load_mesh("x.glb", b"glTF garbage").is_err());
        assert!(load_mesh("x.fbx", b"").is_err());
        assert!(load_mesh("x.obj", b"v 0 0 0\nf 1 2 3\n").is_err());
    }
}
