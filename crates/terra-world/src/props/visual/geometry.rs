use super::PropMesh;
use bevy_math::{Quat, Vec2, Vec3};
use std::collections::BTreeMap;

/// One small visual voxel; these vertices share a section batch, never an ECS entity.
pub(crate) fn add_box(
    batches: &mut BTreeMap<usize, PropMesh>,
    material: usize,
    center: Vec3,
    size: Vec3,
    rotation: Quat,
) -> Result<(), String> {
    let batch = batches.entry(material).or_insert_with(|| PropMesh {
        material,
        ..Default::default()
    });
    for axis in 0..3 {
        for sign in [-1.0, 1.0] {
            let mut normal = Vec3::ZERO;
            normal[axis] = sign;
            let u = (axis + 1) % 3;
            let v = (axis + 2) % 3;
            let base = u32::try_from(batch.positions.len()).map_err(|_| "Micro mesh overflow")?;
            base.checked_add(3).ok_or("Micro mesh overflow")?;
            for [a, b] in [[-0.5, -0.5], [0.5, -0.5], [0.5, 0.5], [-0.5, 0.5]] {
                let mut point = normal * size * 0.5;
                point[u] = a * size[u];
                point[v] = b * size[v];
                batch.positions.push((center + rotation * point).to_array());
                batch.normals.push((rotation * normal).to_array());
                batch.uvs.push([a + 0.5, b + 0.5]);
            }
            let indices = if sign > 0.0 {
                [0, 1, 2, 0, 2, 3]
            } else {
                [0, 2, 1, 0, 3, 2]
            };
            batch.indices.extend(indices.map(|i| base + i));
        }
    }
    if !batch.colors.is_empty() {
        batch.colors.resize(batch.positions.len(), [1.0; 4]);
    }
    Ok(())
}

/// A square-ended, leaning grass blade; the buried bottom face is omitted.
pub(super) fn add_blade(
    batches: &mut BTreeMap<usize, PropMesh>,
    material: usize,
    root: Vec3,
    height: f32,
    width: f32,
    bend: Vec3,
    yaw: f32,
) -> Result<(), String> {
    add_stalk(
        batches,
        material,
        root,
        Vec2::new(width, width * 0.4),
        &[Vec3::ZERO, Vec3::Y * height + bend],
        yaw,
    )
}

/// Joined rectangular stalk segments: no internal caps, taper or disconnected voxels.
pub(super) fn add_stalk(
    batches: &mut BTreeMap<usize, PropMesh>,
    material: usize,
    root: Vec3,
    cross_section: Vec2,
    path: &[Vec3],
    yaw: f32,
) -> Result<(), String> {
    let batch = batches.entry(material).or_insert_with(|| PropMesh {
        material,
        ..Default::default()
    });
    let bottom = [
        Vec3::new(-0.5, 0.0, -0.5),
        Vec3::new(0.5, 0.0, -0.5),
        Vec3::new(0.5, 0.0, 0.5),
        Vec3::new(-0.5, 0.0, 0.5),
    ];
    let rotation = Quat::from_rotation_y(yaw);
    let size = Vec3::new(cross_section.x, 1.0, cross_section.y);
    let ring = |point: Vec3| bottom.map(|p| root + rotation * (p * size) + point);
    let mut quads = Vec::with_capacity(path.len() * 4 + 1);
    for segment in path.windows(2) {
        let a = ring(segment[0]);
        let b = ring(segment[1]);
        for edge in 0..4 {
            let next = (edge + 1) % 4;
            quads.push([a[edge], b[edge], b[next], a[next]]);
        }
    }
    if let Some(&tip) = path.last() {
        let tip = ring(tip);
        quads.push([tip[0], tip[3], tip[2], tip[1]]);
    }
    for quad in quads {
        let normal = (quad[1] - quad[0])
            .cross(quad[2] - quad[0])
            .normalize_or_zero();
        let base = u32::try_from(batch.positions.len()).map_err(|_| "Blade mesh overflow")?;
        base.checked_add(3).ok_or("Blade mesh overflow")?;
        batch.positions.extend(quad.map(|p| p.to_array()));
        batch.normals.extend([normal.to_array(); 4]);
        batch
            .uvs
            .extend([[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0]]);
        batch
            .indices
            .extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    if !batch.colors.is_empty() {
        batch.colors.resize(batch.positions.len(), [1.0; 4]);
    }
    Ok(())
}

/// Distant, subpixel stalk thickness does not need four solid sides. Explicit
/// front/back faces keep the existing culled material and correct lighting.
pub(super) fn add_ribbon(
    batches: &mut BTreeMap<usize, PropMesh>,
    material: usize,
    root: Vec3,
    width: f32,
    tip: Vec3,
    yaw: f32,
) -> Result<(), String> {
    let batch = batches.entry(material).or_insert_with(|| PropMesh {
        material,
        ..Default::default()
    });
    let half = Quat::from_rotation_y(yaw) * Vec3::X * (width * 0.5);
    let front = [
        root - half,
        root + tip - half,
        root + tip + half,
        root + half,
    ];
    for quad in [front, [front[3], front[2], front[1], front[0]]] {
        let normal = (quad[1] - quad[0])
            .cross(quad[2] - quad[0])
            .normalize_or_zero();
        let base = u32::try_from(batch.positions.len()).map_err(|_| "Grass ribbon overflow")?;
        base.checked_add(3).ok_or("Grass ribbon overflow")?;
        batch.positions.extend(quad.map(|p| p.to_array()));
        batch.normals.extend([normal.to_array(); 4]);
        batch
            .uvs
            .extend([[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0]]);
        batch
            .indices
            .extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    if !batch.colors.is_empty() {
        batch.colors.resize(batch.positions.len(), [1.0; 4]);
    }
    Ok(())
}
