//! Cutout leaf clusters on gently bowed sheets. Authored alpha supplies the
//! individual blade silhouettes and gaps; there is no opaque backing or inset
//! filler. Shared cached sheets remain in the ordinary section batches.
use super::*;

/// Maximum sheet overhang; the actual mesh bounds include it before wind padding.
pub(super) const RELIEF: f32 = 0.10;

pub(super) struct Triangle {
    pub points: [Vec3; 3],
    pub normal: Vec3,
    pub uvs: [[f32; 2]; 3],
    pub side: usize,
}

static LEAVES: LazyLock<Vec<OnceLock<Vec<Triangle>>>> =
    LazyLock::new(|| (0..1024).map(|_| OnceLock::new()).collect());

fn surface_axes(side: usize) -> (usize, usize) {
    match side / 2 {
        0 => (2, 1), // Every vertical face hangs down world Y.
        1 => (0, 2),
        _ => (0, 1),
    }
}

fn triangle(
    output: &mut Vec<Triangle>,
    side: usize,
    coordinates: [[f32; 3]; 3],
    variant: usize,
    width: f32,
) {
    let (u, v) = surface_axes(side);
    let mut outward = Vec3::ZERO;
    outward[side / 2] = if side.is_multiple_of(2) { -1.0 } else { 1.0 };
    let mut points = coordinates.map(|p| {
        let mut point = Vec3::splat(0.5) + outward * (0.5 + p[2]);
        point[u] = p[0];
        point[v] = p[1];
        point
    });
    // Keep complete blade contours inside the sheet. UV translations would
    // slice leaves at card edges; use whole-image mirrors/rotations instead.
    let mut uvs = coordinates.map(|p| {
        let u = ((p[0] - 0.5) / ((1.0 + 2.0 * RELIEF) * width) + 0.5).clamp(0.0, 1.0);
        let v = ((p[1] + RELIEF) / (1.0 + 2.0 * RELIEF)).clamp(0.0, 1.0);
        if side / 2 == 1 {
            match variant {
                0 => [u, v],
                1 => [1.0 - v, u],
                2 => [1.0 - u, 1.0 - v],
                _ => [v, 1.0 - u],
            }
        } else {
            [
                if variant.is_multiple_of(2) {
                    u
                } else {
                    1.0 - u
                },
                v,
            ]
        }
    });
    let mut cross = (points[1] - points[0]).cross(points[2] - points[0]);
    if cross.dot(outward) < 0.0 {
        points.swap(1, 2);
        uvs.swap(1, 2);
        cross = -cross;
    }
    output.push(Triangle {
        points,
        normal: cross.normalize(),
        uvs,
        side,
    });
}

pub(super) fn template(index: usize) -> &'static [Triangle] {
    LEAVES[index].get_or_init(|| {
        let birch = index / 512 != 0;
        let middle = !(index / 256).is_multiple_of(2);
        let exposure = (index / 4) % 64;
        let variant = index % 4;
        let mut triangles = Vec::new();
        for side in 0..6 {
            if exposure & (1 << side) == 0 {
                continue;
            }
            // One sheet only: a second offset layer would fill the intended
            // holes. Alpha also applies to reverse faces, depth and shadows.
            let segments = if middle { 1 } else { 2 };
            let width = if birch { 0.97 } else { 1.0 };
            let point = |x: usize, y: usize| {
                let u = x as f32 / segments as f32;
                let v = y as f32 / segments as f32;
                let center = (1.0 - (u * 2.0 - 1.0).abs()) * (1.0 - (v * 2.0 - 1.0).abs());
                let wave = if (x + y + side + variant).is_multiple_of(2) {
                    0.012
                } else {
                    -0.012
                };
                [
                    0.5 + (u - 0.5) * (1.0 + 2.0 * RELIEF) * width,
                    -RELIEF + v * (1.0 + 2.0 * RELIEF),
                    0.014 + center * 0.045 + wave,
                ]
            };
            for y in 0..segments {
                for x in 0..segments {
                    let corners = [
                        point(x, y),
                        point(x + 1, y),
                        point(x + 1, y + 1),
                        point(x, y + 1),
                    ];
                    triangle(
                        &mut triangles,
                        side,
                        [corners[0], corners[1], corners[2]],
                        variant,
                        width,
                    );
                    triangle(
                        &mut triangles,
                        side,
                        [corners[0], corners[2], corners[3]],
                        variant,
                        width,
                    );
                }
            }
        }
        triangles
    })
}

pub(super) fn append(
    batches: &mut BTreeMap<usize, MeshBatch>,
    block: &BlockTemplate,
    global: [i64; 3],
    local: [u32; 3],
    exposure: u8,
    level: DetailLevel,
    material: u16,
) -> Result<(), String> {
    let index = usize::from(material == 9) * 512
        + usize::from(level == DetailLevel::Middle) * 256
        + usize::from(exposure) * 4
        + (variation(global, 0x004c_414d_494e) % 4) as usize;
    let starts = block
        .face_materials
        .map(|slot| batches.get(&slot).map_or(0, |b| b.positions.len()));
    let offset = Vec3::from_array(local.map(|p| p as f32));
    for triangle in template(index) {
        let slot = block.face_materials[triangle.side];
        let batch = batches.entry(slot).or_insert_with(|| MeshBatch {
            material: slot,
            ..default()
        });
        let base = u32::try_from(batch.positions.len()).map_err(|_| "Leaf mesh overflow")?;
        base.checked_add(2).ok_or("Leaf mesh overflow")?;
        batch.colors.resize(batch.positions.len(), [1.; 4]);
        batch
            .positions
            .extend(triangle.points.map(|p| (p + offset).to_array()));
        // A painted lamina has a shallow bend, not four hard pyramid
        // facets. Keep the geometric winding but soften its shading frame.
        let mut outward = Vec3::ZERO;
        outward[triangle.side / 2] = if triangle.side.is_multiple_of(2) {
            -1.0
        } else {
            1.0
        };
        let shading = outward.lerp(triangle.normal, 0.35).normalize();
        batch.normals.extend([shading.to_array(); 3]);
        // No random vertex darkening: authored leaf palettes stay stable in LOD.
        batch.colors.extend([[1.; 4]; 3]);
        batch.uvs.extend(triangle.uvs);
        batch.indices.extend([base, base + 1, base + 2]);
    }
    for (index, &slot) in block.face_materials.iter().enumerate() {
        if block.face_materials[..index].contains(&slot) {
            continue;
        }
        if let Some(batch) = batches.get_mut(&slot)
            && starts[index] < batch.positions.len()
        {
            batch.foliage_owners.push(FoliageOwner {
                vertices: starts[index]..batch.positions.len(),
                cell: local,
                material,
            });
        }
    }
    Ok(())
}
