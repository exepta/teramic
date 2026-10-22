use super::{
    DetailLevel,
    builder::{FoliageOwner, MeshBatch, variation},
};
use crate::blocks::BlockTemplate;
use bevy::prelude::*;
use std::{
    collections::BTreeMap,
    sync::{LazyLock, OnceLock},
};
use terra_voxel::meshing::{BlockFace, PADDED_VOLUME, mesh_section};

struct CrownTemplate {
    resolution: u32,
    faces: Vec<BlockFace>,
    tones: Vec<f32>,
}

// Shared by asynchronous jobs. Build only requested variants, never a full cold
// library while all meshing workers wait on one initialization lock.
static CROWNS: LazyLock<Vec<OnceLock<Result<CrownTemplate, String>>>> =
    LazyLock::new(|| (0..1536).map(|_| OnceLock::new()).collect());
fn cached_crown(index: usize) -> Result<&'static CrownTemplate, String> {
    CROWNS[index]
        .get_or_init(|| {
            species_template(
                if (index / 256).is_multiple_of(2) {
                    6
                } else {
                    4
                },
                ((index / 4) % 64) as u8,
                (index % 4) as u64,
                (index / 512) as u8,
            )
        })
        .as_ref()
        .map_err(Clone::clone)
}

#[cfg(test)]
fn crown_template(
    resolution: u32,
    exposure: u8,
    variant: u64,
    birch: bool,
) -> Result<CrownTemplate, String> {
    species_template(resolution, exposure, variant, u8::from(birch))
}

fn species_template(
    resolution: u32,
    exposure: u8,
    variant: u64,
    species: u8,
) -> Result<CrownTemplate, String> {
    let birch = species == 1;
    let size = resolution as i32;
    let mut padded = vec![0; PADDED_VOLUME];
    for z in -1..=size {
        for y in -1..=size {
            for x in -1..=size {
                let point = [x, y, z];
                let mut boundary = 0_u8;
                let mut outside = 0_u8;
                for (axis, &coordinate) in point.iter().enumerate() {
                    if coordinate <= 0 {
                        boundary |= 1 << (axis * 2);
                    }
                    if coordinate >= size - 1 {
                        boundary |= 1 << (axis * 2 + 1);
                    }
                    if coordinate < 0 {
                        outside |= 1 << (axis * 2);
                    }
                    if coordinate >= size {
                        outside |= 1 << (axis * 2 + 1);
                    }
                }
                let occupied = if outside != 0 {
                    outside & exposure == 0
                } else if boundary & !exposure != 0 {
                    true
                } else {
                    (0_usize..6)
                        .filter(|side| exposure & (1 << side) != 0)
                        .all(|side| {
                            let axis = side / 2;
                            let depth = if side.is_multiple_of(2) {
                                point[axis]
                            } else {
                                size - 1 - point[axis]
                            };
                            let u = (point[(axis + 1) % 3] - 1).clamp(0, size - 3) as usize;
                            let v = (point[(axis + 2) % 3] - 1).clamp(0, size - 3) as usize;
                            let profile = if resolution == 6 {
                                const CLUMPS: [[i32; 4]; 4] =
                                    [[2, 2, 1, 1], [2, 2, 1, 1], [1, 1, 0, 0], [1, 1, 0, 0]];
                                const BIRCH: [[i32; 4]; 4] =
                                    [[2, 2, 1, 2], [2, 1, 1, 1], [1, 1, 2, 2], [2, 1, 2, 2]];
                                const SPRUCE: [[i32; 4]; 4] =
                                    [[1, 1, 1, 1], [1, 1, 1, 1], [0, 0, 0, 0], [0, 0, 0, 0]];
                                let clumps = if species == 2 {
                                    SPRUCE
                                } else if birch {
                                    BIRCH
                                } else {
                                    CLUMPS
                                };
                                clumps[(u + variant as usize) % 4][(v + variant as usize / 2) % 4]
                            } else if birch {
                                1
                            } else {
                                i32::from(!(u + v + variant as usize).is_multiple_of(3))
                            };
                            depth >= profile
                        })
                };
                if occupied {
                    let index = (x + 1) as usize + 18 * ((y + 1) as usize + 18 * (z + 1) as usize);
                    padded[index] = 1;
                }
            }
        }
    }
    let mut faces = mesh_section(&padded)
        .map_err(|error| error.to_string())?
        .faces;
    // Neighbor cells hide internal faces, but their own geometry is outside this block.
    faces.retain_mut(|face| {
        if face
            .minimum
            .iter()
            .any(|&coordinate| coordinate >= resolution)
        {
            return false;
        }
        face.width = face.width.min(resolution - face.minimum[face.u_axis]);
        face.height = face.height.min(resolution - face.minimum[face.v_axis]);
        true
    });
    let mut tones = Vec::with_capacity(faces.len());
    let mut total_area = 0.0;
    let mut weighted_tone = 0.0;
    for face in &faces {
        let area = (coordinate(face.minimum[face.u_axis] + face.width, resolution)
            - coordinate(face.minimum[face.u_axis], resolution))
            * (coordinate(face.minimum[face.v_axis] + face.height, resolution)
                - coordinate(face.minimum[face.v_axis], resolution));
        let ticket = variation(face.minimum.map(i64::from), face.side as u64 + variant * 7);
        let tone = (ticket >> 40) as f32 / 16_777_216.0 * 0.12 - 0.06;
        total_area += area;
        weighted_tone += tone * area;
        tones.push(tone);
    }
    if total_area > 0.0 {
        for tone in &mut tones {
            *tone -= weighted_tone / total_area;
        }
    }
    Ok(CrownTemplate {
        resolution,
        faces,
        tones,
    })
}

fn coordinate(index: u32, resolution: u32) -> f32 {
    if resolution == 6 {
        [0.0, 0.045, 0.27, 0.5, 0.73, 0.955, 1.0][index as usize]
    } else {
        [0.0, 0.045, 0.5, 0.955, 1.0][index as usize]
    }
}

pub(super) fn append_crown(
    batches: &mut BTreeMap<usize, MeshBatch>,
    block: &BlockTemplate,
    global: [i64; 3],
    local: [u32; 3],
    exposure: u8,
    level: DetailLevel,
    species_material: (u16, bool),
) -> Result<(), String> {
    let (material, spruce) = species_material;
    let variant = variation(global, 0x0043_524f_574e) % 4;
    let index = (if spruce {
        2
    } else {
        usize::from(material == 9)
    }) * 512
        + ((usize::from(level == DetailLevel::Middle) * 64 + usize::from(exposure)) * 4)
        + variant as usize;
    let template = cached_crown(index)?;
    let sign = if variation(global, 0x004c_4541_4653) & 1 == 0 {
        -1.0
    } else {
        1.0
    };
    let starts = block
        .face_materials
        .map(|slot| batches.get(&slot).map_or(0, |batch| batch.positions.len()));
    for (&face, &tone) in template.faces.iter().zip(&template.tones) {
        let material = block.face_materials[face.side];
        let batch = batches.entry(material).or_insert_with(|| MeshBatch {
            material,
            ..default()
        });
        let mut a: [f32; 3] = std::array::from_fn(|axis| {
            local[axis] as f32 + coordinate(face.minimum[axis], template.resolution)
        });
        if face.side % 2 == 1 {
            let axis = face.side / 2;
            a[axis] = local[axis] as f32 + coordinate(face.minimum[axis] + 1, template.resolution);
        }
        let mut b = a;
        b[face.u_axis] = local[face.u_axis] as f32
            + coordinate(face.minimum[face.u_axis] + face.width, template.resolution);
        let mut c = b;
        c[face.v_axis] = local[face.v_axis] as f32
            + coordinate(face.minimum[face.v_axis] + face.height, template.resolution);
        let mut d = a;
        d[face.v_axis] = c[face.v_axis];
        let mut normal = Vec3::ZERO;
        normal[face.side / 2] = if face.side.is_multiple_of(2) {
            -1.0
        } else {
            1.0
        };
        let base = u32::try_from(batch.positions.len()).map_err(|_| "Crown mesh overflow")?;
        base.checked_add(3).ok_or("Crown mesh overflow")?;
        batch.colors.resize(batch.positions.len(), [1.0; 4]);
        batch.positions.extend([a, b, c, d]);
        batch.normals.extend([normal.to_array(); 4]);
        let shade = 1.0 + tone * sign;
        batch.colors.extend([[shade, shade, shade, 1.0]; 4]);
        batch
            .uvs
            .extend([a, b, c, d].map(|p| [p[face.u_axis], p[face.v_axis]]));
        let cross = (Vec3::from_array(b) - Vec3::from_array(a))
            .cross(Vec3::from_array(d) - Vec3::from_array(a));
        let indices = if cross.dot(normal) > 0.0 {
            [0, 1, 2, 0, 2, 3]
        } else {
            [0, 2, 1, 0, 3, 2]
        };
        batch.indices.extend(indices.map(|i| base + i));
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

pub(super) fn append_twig(
    batches: &mut BTreeMap<usize, MeshBatch>,
    material: usize,
    local: Vec3,
    side: usize,
    seed: u64,
) -> Result<(), String> {
    let axis = side / 2;
    if axis == 1 || !seed.is_multiple_of(7) {
        return Ok(());
    }
    let mut direction = Vec3::ZERO;
    direction[axis] = if side.is_multiple_of(2) { -1.0 } else { 1.0 };
    let anchor = local + Vec3::splat(0.5) + direction * 0.46;
    let tip = anchor + direction * (0.24 + ((seed >> 8) % 5) as f32 * 0.035) + Vec3::Y * 0.14;
    let offset = tip - anchor;
    crate::props::add_box(
        batches,
        material,
        (anchor + tip) * 0.5,
        Vec3::new(0.075, offset.length(), 0.075),
        Quat::from_rotation_arc(Vec3::Y, offset.normalize()),
    )?;
    let fork = tip + Vec3::Y * 0.15 + direction * 0.07;
    let offset = fork - tip;
    crate::props::add_box(
        batches,
        material,
        (tip + fork) * 0.5,
        Vec3::new(0.045, offset.length(), 0.045),
        Quat::from_rotation_arc(Vec3::Y, offset.normalize()),
    )
}

#[cfg(test)]
mod tests;
