//! Thin stepped plant silhouettes with only exterior faces. These are generated
//! once per shared template; no per-leaf entities or runtime voxel grids.
use super::PropMesh;
use bevy_math::{Vec2, Vec3};
use std::collections::BTreeMap;

/// Extrude a counter-clockwise orthogonal outline in a local leaf plane. Shared
/// face vertices keep an eight-corner bent blade at 48 vertices / 28 triangles.
pub(super) fn add_profile(
    batches: &mut BTreeMap<usize, PropMesh>,
    material: usize,
    root: Vec3,
    outline: &[Vec2],
    across: Vec3,
    along: Vec3,
    thickness: f32,
) -> Result<(), String> {
    let triangles = triangulate(outline)?;
    let normal = across.cross(along).normalize_or_zero();
    if normal == Vec3::ZERO || !thickness.is_finite() || thickness <= 0.0 {
        return Err("Invalid leaf plane".into());
    }
    let min = outline
        .iter()
        .copied()
        .fold(Vec2::splat(f32::MAX), Vec2::min);
    let max = outline
        .iter()
        .copied()
        .fold(Vec2::splat(f32::MIN), Vec2::max);
    let batch = batches.entry(material).or_insert_with(|| PropMesh {
        material,
        ..Default::default()
    });
    let base = u32::try_from(batch.positions.len()).map_err(|_| "Leaf mesh overflow")?;
    let count = u32::try_from(outline.len()).map_err(|_| "Leaf mesh overflow")?;
    base.checked_add(count * 6).ok_or("Leaf mesh overflow")?;
    let position = |point: Vec2, sign: f32| {
        root + across * point.x + along * point.y + normal * thickness * 0.5 * sign
    };
    for sign in [1.0, -1.0] {
        let face_base = batch.positions.len() as u32;
        for &point in outline {
            batch.positions.push(position(point, sign).to_array());
            batch.normals.push((normal * sign).to_array());
            batch.uvs.push(((point - min) / (max - min)).to_array());
        }
        for &[a, b, c] in &triangles {
            let triangle = if sign > 0.0 { [a, b, c] } else { [a, c, b] };
            batch.indices.extend(triangle.map(|i| face_base + i as u32));
        }
    }
    for i in 0..outline.len() {
        let a = outline[i];
        let b = outline[(i + 1) % outline.len()];
        let quad = [
            position(a, -1.0),
            position(b, -1.0),
            position(b, 1.0),
            position(a, 1.0),
        ];
        let side_normal = (quad[1] - quad[0]).cross(quad[2] - quad[0]).normalize();
        let base = batch.positions.len() as u32;
        batch.positions.extend(quad.map(|p| p.to_array()));
        batch.normals.extend([side_normal.to_array(); 4]);
        batch
            .uvs
            .extend([[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]);
        batch
            .indices
            .extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    if !batch.colors.is_empty() {
        batch.colors.resize(batch.positions.len(), [1.0; 4]);
    }
    Ok(())
}

fn triangulate(outline: &[Vec2]) -> Result<Vec<[usize; 3]>, String> {
    if !(3..=32).contains(&outline.len()) || outline.iter().any(|p| !p.is_finite()) {
        return Err("Invalid leaf outline".into());
    }
    let mut remaining: Vec<_> = (0..outline.len()).collect();
    let mut triangles = Vec::with_capacity(outline.len() - 2);
    while remaining.len() > 3 {
        let ear = (0..remaining.len())
            .find(|&i| {
                let a = remaining[(i + remaining.len() - 1) % remaining.len()];
                let b = remaining[i];
                let c = remaining[(i + 1) % remaining.len()];
                let [pa, pb, pc] = [a, b, c].map(|p| outline[p]);
                if (pb - pa).perp_dot(pc - pa) <= 1e-9 {
                    return false;
                }
                !remaining.iter().any(|&p| {
                    p != a
                        && p != b
                        && p != c
                        && (pb - pa).perp_dot(outline[p] - pa) >= -1e-9
                        && (pc - pb).perp_dot(outline[p] - pb) >= -1e-9
                        && (pa - pc).perp_dot(outline[p] - pc) >= -1e-9
                })
            })
            .ok_or("Leaf outline must be a simple counter-clockwise polygon")?;
        triangles.push([
            remaining[(ear + remaining.len() - 1) % remaining.len()],
            remaining[ear],
            remaining[(ear + 1) % remaining.len()],
        ]);
        remaining.remove(ear);
    }
    triangles.push([remaining[0], remaining[1], remaining[2]]);
    Ok(triangles)
}

/// The overlapping shelves of the longer blades follow the outward curve to
/// a short, narrow tip. Inner leaves make the same turn in three tiers.
pub(super) fn grass_profile(reach: f32, height: f32, outer: bool) -> Vec<Vec2> {
    let outline: &[[f32; 2]] = if outer {
        &[
            [-0.32, 0.00],
            [0.32, 0.00],
            [0.32, 0.28],
            [0.60, 0.28],
            [0.60, 0.56],
            [0.84, 0.56],
            [0.84, 0.82],
            [1.00, 0.82],
            [1.00, 1.00],
            [0.72, 1.00],
            [0.72, 0.82],
            [0.45, 0.82],
            [0.45, 0.56],
            [0.11, 0.56],
            [0.11, 0.28],
            [-0.32, 0.28],
        ]
    } else {
        &[
            [-0.32, 0.00],
            [0.32, 0.00],
            [0.32, 0.42],
            [0.76, 0.42],
            [0.76, 0.78],
            [1.00, 0.78],
            [1.00, 1.00],
            [0.72, 1.00],
            [0.72, 0.78],
            [0.17, 0.78],
            [0.17, 0.42],
            [-0.32, 0.42],
        ]
    };
    outline
        .iter()
        .map(|[x, y]| Vec2::new(x * reach, y * height))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bent_leaf_is_a_closed_indexed_staircase_without_internal_caps() {
        let mut batches = BTreeMap::new();
        let outline = grass_profile(0.15, 0.5, true);
        add_profile(
            &mut batches,
            0,
            Vec3::ZERO,
            &outline,
            Vec3::X,
            Vec3::Y,
            0.018,
        )
        .unwrap();
        let mesh = &batches[&0];
        assert_eq!(mesh.positions.len(), 96);
        assert_eq!(mesh.indices.len(), 180);
        let mut edges = BTreeMap::new();
        for triangle in mesh.indices.chunks_exact(3) {
            for edge in 0..3 {
                let a = mesh.positions[triangle[edge] as usize].map(f32::to_bits);
                let b = mesh.positions[triangle[(edge + 1) % 3] as usize].map(f32::to_bits);
                *edges
                    .entry(if a < b { (a, b) } else { (b, a) })
                    .or_insert(0) += 1;
            }
        }
        assert!(
            edges.values().all(|&count| count == 2),
            "closed exterior shell"
        );
        let mut front_area = 0.0;
        for t in mesh.indices.chunks_exact(3) {
            let [a, b, c] =
                [t[0], t[1], t[2]].map(|i| Vec3::from_array(mesh.positions[i as usize]));
            let n = Vec3::from_array(mesh.normals[t[0] as usize]);
            let cross = (b - a).cross(c - a);
            assert!(cross.length_squared() > 1e-12);
            assert!(cross.normalize().dot(n) > 0.999);
            assert_eq!(
                n.to_array().into_iter().filter(|v| v.abs() > 1e-5).count(),
                1
            );
            if n.z > 0.5 {
                front_area += cross.length() * 0.5;
            }
        }
        // Exact union area of the four connected tiers, not a filled AABB.
        let expected = (0.64 * 0.28 + 0.49 * 0.28 + 0.39 * 0.26 + 0.28 * 0.18) * 0.15 * 0.5;
        assert!((front_area - expected).abs() < 1e-6);
        assert!(mesh.positions.iter().all(|p| p[1] >= 0.0 && p[1] <= 0.5));
    }
}
