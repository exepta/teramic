//! Individually sculpted blades and protruding blossoms/fruit form a layered
//! crown. Motifs share a material sheet, never a continuous cube face or filler.
use super::*;

// Compatibility for packs without an authored cluster sprite. Their whole-face
// artwork must not be arbitrarily cropped into individual leaf rectangles.
#[cfg(test)]
mod coverage_tests;
#[cfg(test)]
mod perspective_tests;
mod sheets;

/// Includes all static cluster overhang. Wind expands actual mesh bounds later.
pub(super) const RELIEF: f32 = 0.25;
const FAMILY_STRIDE: usize = 512;
const FAMILIES: usize = 3;

pub(super) struct Triangle {
    pub points: [Vec3; 3],
    pub normal: Vec3,
    pub uvs: [[f32; 2]; 3],
    #[cfg(test)]
    pub side: usize,
}

static LEAVES: LazyLock<Vec<OnceLock<Vec<Triangle>>>> = LazyLock::new(|| {
    (0..FAMILIES * FAMILY_STRIDE)
        .map(|_| OnceLock::new())
        .collect()
});

fn axes(side: usize) -> (Vec3, Vec3, Vec3) {
    let mut normal = Vec3::ZERO;
    normal[side / 2] = if side.is_multiple_of(2) { -1.0 } else { 1.0 };
    let (u, v) = match side / 2 {
        0 => (Vec3::Z, Vec3::Y),
        1 => (Vec3::X, Vec3::Z),
        _ => (Vec3::X, Vec3::Y),
    };
    (normal, u, v)
}

fn triangle(
    output: &mut Vec<Triangle>,
    side: usize,
    mut points: [Vec3; 3],
    mut uvs: [[f32; 2]; 3],
) {
    let outward = axes(side).0;
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
        #[cfg(test)]
        side,
    });
}

/// Complete motif cells in a 2×2 sheet. The clear cell borders are sampled
/// with the leaf contour, never by cropping arbitrary pieces of a canopy image.
fn motif_uv(tile: usize, mut uv: Vec2, turn: usize) -> [f32; 2] {
    for _ in 0..turn {
        uv = Vec2::new(1.0 - uv.y, uv.x);
    }
    ((uv + Vec2::new((tile % 2) as f32, (tile / 2) as f32)) * 0.5).to_array()
}

pub(super) const LEAVES_PER_SIDE: usize = 14;
pub(super) const NEAR_TRIANGLES: usize = 6;
pub(super) const MIDDLE_TRIANGLES: usize = 2;

pub(super) fn template(index: usize) -> &'static [Triangle] {
    LEAVES[index].get_or_init(|| {
        let family = index / FAMILY_STRIDE;
        let middle = !(index / 256).is_multiple_of(2);
        let exposure = (index / 4) % 64;
        let variant = index % 4;
        let mut output = Vec::new();
        for side in 0_usize..6 {
            if exposure & (1 << side) == 0 {
                continue;
            }
            let (outward, u_axis, v_axis) = axes(side);
            for leaf in 0..LEAVES_PER_SIDE {
                let ornament = leaf >= 12;
                let column = leaf % 4;
                let row = leaf / 4;
                let salt = variant * 79 + side * 43 + leaf * 31;
                let signed = |shift: usize| ((salt >> shift) % 7) as f32 / 6.0 - 0.5;
                let (mut cx, cy) = if ornament {
                    if leaf == 12 {
                        (0.28, 0.34)
                    } else {
                        (0.72, 0.69)
                    }
                } else {
                    (
                        0.10 + column as f32 * 0.267 + signed(1) * 0.025,
                        0.13 + row as f32 * 0.37 + signed(2) * 0.025,
                    )
                };
                if variant & 2 != 0 {
                    cx = 1.0 - cx;
                }
                // Leaves form staggered, overlapping tiers. No point is a
                // continuous block face: every motif has its own contour,
                // rotation, convex midrib, tilt and depth in the crown.
                let rotation =
                    ((column as f32 - 1.5) * 0.15 + (row as f32 - 1.0) * 0.11 + signed(0) * 0.20)
                        .clamp(-0.28, 0.28);
                let (sin, cos) = rotation.sin_cos();
                let width = if ornament {
                    0.31
                } else if family == 1 {
                    0.51
                } else {
                    0.52
                };
                let height = if ornament { 0.34 } else { 0.58 };
                let tilt_u = if salt.is_multiple_of(2) { 0.23 } else { -0.23 };
                let tilt_v = if salt.is_multiple_of(3) { -0.19 } else { 0.19 };
                let depth = if ornament {
                    0.145
                } else {
                    0.045 - 0.11 * ((cx - 0.5).abs() + (cy - 0.5).abs()) + signed(3) * 0.055
                };
                let tile = if ornament { leaf - 10 } else { salt % 2 };
                let turn = if ornament {
                    (salt % 2) * 2
                } else {
                    (salt / 7) % 4
                };
                let position = |uv: Vec2, fold: f32| {
                    let lx = (uv.x - 0.5) * width;
                    let ly = (uv.y - 0.5) * height;
                    let across = lx * cos - ly * sin;
                    let along = lx * sin + ly * cos;
                    Vec3::splat(0.5)
                        + outward * (0.5 + depth + lx * tilt_u + ly * tilt_v + fold)
                        + u_axis * (cx - 0.5 + across)
                        + v_axis * (cy - 0.5 + along)
                };
                if middle {
                    let uvs = [Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y];
                    let points = uvs.map(|uv| position(uv, 0.0));
                    triangle(
                        &mut output,
                        side,
                        [points[0], points[1], points[2]],
                        [
                            motif_uv(tile, uvs[0], turn),
                            motif_uv(tile, uvs[1], turn),
                            motif_uv(tile, uvs[2], turn),
                        ],
                    );
                    triangle(
                        &mut output,
                        side,
                        [points[0], points[2], points[3]],
                        [
                            motif_uv(tile, uvs[0], turn),
                            motif_uv(tile, uvs[2], turn),
                            motif_uv(tile, uvs[3], turn),
                        ],
                    );
                } else {
                    // Six-sided blade outline around a raised midrib. The
                    // transparent artwork rounds/lobes the complete contour.
                    let rim = [
                        Vec2::new(0.5, 0.0),
                        Vec2::new(1.0, 0.25),
                        Vec2::new(1.0, 0.75),
                        Vec2::new(0.5, 1.0),
                        Vec2::new(0.0, 0.75),
                        Vec2::new(0.0, 0.25),
                    ];
                    let centre = Vec2::splat(0.5);
                    for edge in 0..6 {
                        let next = (edge + 1) % 6;
                        triangle(
                            &mut output,
                            side,
                            [
                                position(centre, if ornament { 0.025 } else { 0.035 }),
                                position(rim[edge], 0.0),
                                position(rim[next], 0.0),
                            ],
                            [
                                motif_uv(tile, centre, turn),
                                motif_uv(tile, rim[edge], turn),
                                motif_uv(tile, rim[next], turn),
                            ],
                        );
                    }
                }
            }
        }
        output
    })
}

pub(super) fn append(
    batches: &mut BTreeMap<usize, MeshBatch>,
    block: &BlockTemplate,
    global: [i64; 3],
    local: [u32; 3],
    exposure: u8,
    level: DetailLevel,
    style: CrownStyle,
) -> Result<(), String> {
    let Some(cluster) = style.cluster else {
        return sheets::append(
            batches,
            block,
            global,
            local,
            exposure,
            level,
            style.material,
        );
    };
    let conifer = matches!(
        style.species,
        Some(TreeAppearance::Spruce | TreeAppearance::Fir)
    );
    let family = if conifer {
        2
    } else {
        usize::from(style.material == 9)
    };
    let slot = if conifer {
        cluster.needles.unwrap_or(cluster.leaves)
    } else {
        cluster.leaves
    };
    let index = family * FAMILY_STRIDE
        + usize::from(level == DetailLevel::Middle) * 256
        + usize::from(exposure) * 4
        + (variation(global, 0x004c_414d_494e) % 4) as usize;
    let triangles = template(index);
    if triangles.is_empty() {
        return Ok(());
    }
    let offset = Vec3::from_array(local.map(|p| p as f32));
    let batch = batches.entry(slot).or_insert_with(|| MeshBatch {
        material: slot,
        ..default()
    });
    let start = batch.positions.len();
    batch.colors.resize(start, [1.; 4]);
    let triangles_per_leaf = if level == DetailLevel::Middle {
        MIDDLE_TRIANGLES
    } else {
        NEAR_TRIANGLES
    };
    for leaf in triangles.chunks_exact(triangles_per_leaf) {
        let base = u32::try_from(batch.positions.len()).map_err(|_| "Leaf mesh overflow")?;
        let mut vertices = [(Vec3::ZERO, [0.; 2], Vec3::ZERO); 7];
        let mut used = 0;
        for triangle in leaf {
            for (&point, &uv) in triangle.points.iter().zip(&triangle.uvs) {
                debug_assert!(
                    point.cmpge(Vec3::splat(-RELIEF)).all()
                        && point.cmple(Vec3::splat(1.0 + RELIEF)).all()
                );
                let index = if let Some(i) = vertices[..used]
                    .iter()
                    .position(|v| v.0 == point && v.1 == uv)
                {
                    vertices[i].2 += triangle.normal;
                    i
                } else {
                    vertices[used] = (point, uv, triangle.normal);
                    used += 1;
                    used - 1
                };
                batch
                    .indices
                    .push(base.checked_add(index as u32).ok_or("Leaf mesh overflow")?);
            }
        }
        for &(position, uv, normal) in &vertices[..used] {
            batch.positions.push((position + offset).to_array());
            batch.uvs.push(uv);
            batch.normals.push(normal.normalize().to_array());
            batch.colors.push([1.; 4]);
        }
    }
    if batch.positions.len() > start {
        batch.foliage_owners.push(FoliageOwner {
            vertices: start..batch.positions.len(),
            cell: local,
            material: style.material,
        });
    }
    Ok(())
}
