//! Neighbouring liquid cells share their top corners, including across sections.
//! A full caldera keeps its greedy terrain geometry.
mod lights;
pub(crate) use lights::install as install_lights;

use super::builder::MeshBatch;
use bevy::prelude::*;
use std::collections::{HashMap, HashSet};
use terra_voxel::{BlockSection, MaterialId, SectionPos, meshing::BlockFace};
use terra_world::{
    edits::EditJournal,
    lava::{LAVA, POROUS_DECORATIONS, SurfaceCorner, natural_slope_corner, surface_corner},
    streaming::ChunkStore,
};

pub(super) fn surface_corners(
    owner: SectionPos,
    chunks: &ChunkStore,
    journal: &EditJournal,
) -> HashMap<[i32; 3], SurfaceCorner> {
    let Ok(origin) = owner.origin() else {
        return HashMap::new();
    };
    let mut corners = HashSet::new();
    for dz in -1..=1 {
        for dx in -1..=1 {
            for dy in -1..=1 {
                let Ok(pos) = owner.offset([dx, dy, dz]) else {
                    continue;
                };
                for (local, material) in journal.blocks_in_section(pos) {
                    if material != LAVA {
                        continue;
                    }
                    let Ok(cell) = pos.global_block(local) else {
                        continue;
                    };
                    for x in 0..=1 {
                        for z in 0..=1 {
                            let vertex = [cell[0] + x, cell[1] + 1, cell[2] + z];
                            if (0..3).all(|i| (0..=16).contains(&(vertex[i] - origin[i]))) {
                                corners.insert(vertex);
                            }
                        }
                    }
                }
            }
        }
    }
    let mut result = HashMap::new();
    for v in corners {
        result.insert(
            std::array::from_fn(|i| (v[i] - origin[i]) as i32),
            surface_corner(chunks, journal, v),
        );
    }
    result
}

#[derive(Default)]
pub(super) struct Surface {
    corners: HashMap<[i32; 3], SurfaceCorner>,
    contacts: Vec<BlockFace>,
    moving: bool,
}

pub(super) fn near_lava(section: &BlockSection, chunks: &ChunkStore) -> bool {
    section.may_contain(LAVA)
        || (POROUS_DECORATIONS
            .into_iter()
            .any(|m| section.may_contain(m))
            && [[-1, 0, 0], [1, 0, 0], [0, 0, -1], [0, 0, 1]]
                .into_iter()
                .any(|d| {
                    section
                        .position()
                        .offset(d)
                        .ok()
                        .and_then(|p| chunks.loaded.get(&p))
                        .is_some_and(|neighbour| neighbour.data.may_contain(LAVA))
                }))
}

pub(super) fn capture(
    section: &BlockSection,
    chunks: &ChunkStore,
    journal: &EditJournal,
    is_model: impl Fn(u16) -> bool,
) -> Surface {
    let owner = section.position();
    let origin = owner.origin().expect("block section has a valid origin");
    let mut corners = surface_corners(owner, chunks, journal);
    let moving = !corners.is_empty();
    let mut contacts = Vec::new();
    let cell_index = |index: usize| {
        [
            (index % 16) as u32,
            ((index / 16) % 16) as u32,
            (index / 256) as u32,
        ]
    };
    let global_cell = |local: [u32; 3]| std::array::from_fn(|i| origin[i] + i64::from(local[i]));
    let mut visited = [false; 17 * 17 * 17];
    let mut block = chunks.block_sampler();
    for (index, &raw) in section.blocks().iter().enumerate() {
        if raw != LAVA {
            continue;
        }
        let cell = cell_index(index);
        let above = if cell[1] < 15 {
            Some(section.blocks()[index + 16])
        } else {
            block(global_cell([cell[0], 16, cell[2]]))
        };
        if above.is_none_or(|raw| raw == LAVA) {
            continue;
        }
        let y = cell[1] as usize + 1;
        for x in cell[0] as usize..=cell[0] as usize + 1 {
            for z in cell[2] as usize..=cell[2] as usize + 1 {
                let seen = &mut visited[x + 17 * (y + 17 * z)];
                if *seen {
                    continue;
                }
                *seen = true;
                let corner = [x, y, z].map(|v| v as i32);
                if !corners.contains_key(&corner)
                    && let Some(sample) = natural_slope_corner(
                        &mut block,
                        journal,
                        std::array::from_fn(|i| origin[i] + i64::from(corner[i])),
                    )
                {
                    corners.insert(corner, sample);
                }
            }
        }
    }
    // A natural lake keeps its original greedy batch unless it surrounds a
    // decorative cell. Normal caldera floors do not need a per-cell rebuild.
    if corners.is_empty()
        && !(POROUS_DECORATIONS
            .into_iter()
            .any(|raw| section.may_contain(raw))
            && section.blocks().iter().enumerate().any(|(i, &raw)| {
                terra_world::lava::immersed_decoration(chunks, global_cell(cell_index(i)), raw)
            }))
    {
        return Surface {
            corners,
            contacts,
            moving,
        };
    }
    for (index, &raw) in section.blocks().iter().enumerate() {
        if matches!(raw, 0 | 27) {
            continue;
        }
        let local = cell_index(index);
        let global = global_cell(local);
        let mut add_face = |side: usize| {
            contacts.push(BlockFace {
                minimum: local,
                side,
                width: 1,
                height: 1,
                material: LAVA,
                edge_mask: 15,
                occupied_neighbor: true,
                u_axis: (side / 2 + 1) % 3,
                v_axis: (side / 2 + 2) % 3,
            })
        };
        let immersed = terra_world::lava::immersed_decoration(chunks, global, raw);
        if raw != LAVA && !immersed {
            continue;
        }
        // Both lava and masked models cull their shared voxel face. The final
        // fluid mesh must restore that contact using its own sloping corners.
        for side in 0..6 {
            let mut neighbor = global;
            neighbor[side / 2] += if side % 2 == 0 { -1 } else { 1 };
            let Some(other) = chunks.block_at(neighbor) else {
                continue;
            };
            if matches!(other, 0 | 27) {
                if immersed {
                    add_face(side);
                }
                continue;
            }
            if other == LAVA || terra_world::lava::immersed_decoration(chunks, neighbor, other) {
                continue;
            }
            let lowered_ceiling = side == 3
                && [0, 1].into_iter().any(|x| {
                    [0, 1].into_iter().any(|z| {
                        let corner = [
                            local[0] as i32 + x,
                            local[1] as i32 + 1,
                            local[2] as i32 + z,
                        ];
                        corners
                            .get(&corner)
                            .is_some_and(|sample| sample.height < 1.)
                    })
                });
            if lowered_ceiling
                || is_model(other)
                || MaterialId::from_raw(other).is_some_and(|m| !m.is_opaque())
            {
                add_face(side);
            }
        }
    }
    Surface {
        corners,
        contacts,
        moving,
    }
}

pub(super) fn surface(
    batches: &mut Vec<MeshBatch>,
    faces: &[BlockFace],
    slot: usize,
    skin: &Surface,
    natural_current: impl Fn([f32; 3]) -> [f32; 2],
) {
    let corners = &skin.corners;
    let index = match batches.iter().position(|b| b.material == slot) {
        Some(i) => i,
        None if skin.contacts.is_empty() => return,
        None => {
            batches.push(MeshBatch {
                material: slot,
                ..default()
            });
            batches.len() - 1
        }
    };
    let batch = &mut batches[index];
    if !corners.is_empty() || !skin.contacts.is_empty() {
        *batch = MeshBatch {
            material: slot,
            flowing_lava: skin.moving || !skin.contacts.is_empty(),
            ..default()
        };
    } else {
        // A static reservoir keeps its greedy geometry and convection current.
        batch.surface_water.clear();
        batch
            .surface_water
            .extend(batch.positions.iter().copied().map(natural_current));
        return;
    }
    const QUADS: [[[i32; 3]; 4]; 6] = [
        [[0, 0, 0], [0, 0, 1], [0, 1, 1], [0, 1, 0]],
        [[1, 0, 1], [1, 0, 0], [1, 1, 0], [1, 1, 1]],
        [[0, 0, 1], [0, 0, 0], [1, 0, 0], [1, 0, 1]],
        [[0, 1, 0], [0, 1, 1], [1, 1, 1], [1, 1, 0]],
        [[1, 0, 0], [0, 0, 0], [0, 1, 0], [1, 1, 0]],
        [[0, 0, 1], [1, 0, 1], [1, 1, 1], [0, 1, 1]],
    ];
    for face in faces
        .iter()
        .filter(|f| f.material == LAVA)
        .chain(&skin.contacts)
    {
        let mut append = |cell: [u32; 3], width: u32, height: u32, adjust: bool| {
            let vertices = QUADS[face.side].map(|d| {
                let mut scale = [1i32; 3];
                scale[face.u_axis] = width as i32;
                scale[face.v_axis] = height as i32;
                let corner = std::array::from_fn(|i| cell[i] as i32 + d[i] * scale[i]);
                let mut position = Vec3::from_array(corner.map(|x| x as f32));
                let sample = corners.get(&corner).filter(|_| adjust && d[1] == 1);
                if let Some(sample) = sample {
                    position.y = (corner[1] - 1) as f32 + sample.height as f32;
                }
                (position, sample)
            });
            let [a, b, c, d] = vertices.map(|(position, _)| position);
            let first = (b - a).cross(c - a);
            let second = (c - a).cross(d - a);
            let valid_first = first.length_squared() > f32::EPSILON;
            let valid_second = second.length_squared() > f32::EPSILON;
            if !valid_first && !valid_second {
                // A one-cell riser disappears where both cap edges touch.
                return;
            }
            let start = batch.positions.len() as u32;
            let normal = (first + second).normalize_or_zero();
            for (position, sample) in vertices {
                batch.positions.push(position.to_array());
                batch.surface_water.push(
                    sample.map_or_else(|| natural_current(position.to_array()), |s| s.downhill),
                );
            }
            batch.normals.extend([normal.to_array(); 4]);
            batch.uvs.extend([[0., 1.], [1., 1.], [1., 0.], [0., 0.]]);
            if valid_first {
                batch.indices.extend([start, start + 1, start + 2]);
            }
            if valid_second {
                batch.indices.extend([start, start + 2, start + 3]);
            }
        };
        if skin.moving || intersects_lava_corners(face, corners) {
            for u in 0..face.width {
                for v in 0..face.height {
                    let mut cell = face.minimum;
                    cell[face.u_axis] += u;
                    cell[face.v_axis] += v;
                    append(cell, 1, 1, true);
                }
            }
        } else {
            append(face.minimum, face.width, face.height, false);
        }
    }
}

fn intersects_lava_corners(face: &BlockFace, corners: &HashMap<[i32; 3], SurfaceCorner>) -> bool {
    if face.side == 2 {
        return false;
    }
    let axis = face.side / 2;
    let minimum = face.minimum.map(|v| v as i32);
    let mut maximum = minimum;
    maximum[face.u_axis] += face.width as i32;
    maximum[face.v_axis] += face.height as i32;
    corners.keys().any(|corner| {
        corner[axis] == minimum[axis] + (face.side % 2) as i32
            && (0..3)
                .filter(|&i| i != axis)
                .all(|i| ((minimum[i] + i32::from(i == 1))..=maximum[i]).contains(&corner[i]))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(chunks: &mut ChunkStore, cell: [i64; 3], raw: u16) {
        let (section, local) = terra_voxel::global_to_section(cell);
        chunks
            .loaded
            .entry(section)
            .or_insert_with(|| terra_world::streaming::LoadedSection {
                data: BlockSection::empty(section).unwrap(),
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: vec![],
            })
            .data
            .set_block(local, raw)
            .unwrap();
    }

    fn render(chunks: &ChunkStore, journal: &EditJournal, owner: SectionPos) -> MeshBatch {
        let origin = owner.origin().unwrap();
        let mut padded = vec![0; terra_voxel::meshing::PADDED_VOLUME];
        for z in 0..18 {
            for y in 0..18 {
                for x in 0..18 {
                    let cell = [x, y, z].map(|v| v as i64 - 1);
                    padded[x + 18 * (y + 18 * z)] = chunks
                        .block_at(std::array::from_fn(|i| origin[i] + cell[i]))
                        .unwrap_or(0);
                }
            }
        }
        let geometry = terra_voxel::meshing::mesh_section(&padded).unwrap();
        let skin = capture(&chunks.loaded[&owner].data, chunks, journal, |_| false);
        let mut batches = vec![MeshBatch {
            material: 9,
            positions: geometry.positions,
            normals: geometry.normals,
            indices: geometry.indices,
            uvs: geometry.uvs,
            ..default()
        }];
        surface(&mut batches, &geometry.faces, 9, &skin, |_| [0.; 2]);
        batches.pop().unwrap()
    }

    #[test]
    fn natural_lava_ramp_joins_its_next_chunk_without_erasing_a_real_drop() {
        let mut chunks = ChunkStore::default();
        let journal = EditJournal::default();
        for z in 5..=7 {
            set(&mut chunks, [15, 5, z], LAVA);
            set(&mut chunks, [16, 4, z], LAVA);
        }
        for y in 7..=9 {
            set(&mut chunks, [15, y, 10], LAVA);
        }
        set(&mut chunks, [16, 7, 10], LAVA);
        let left = render(&chunks, &journal, SectionPos::default());
        let right = render(&chunks, &journal, SectionPos::new(1, 0, 0));
        assert!(!left.flowing_lava && !right.flowing_lava);
        for z in 5..=8 {
            assert!(left.positions.contains(&[16., 5., z as f32]));
            assert!(!left.positions.contains(&[16., 6., z as f32]));
        }
        assert!(right.positions.contains(&[0., 5., 5.]));
        assert!(right.positions.contains(&[0., 5., 8.]));
        let upper = [15, 5, 6];
        let lower = [16, 4, 6];
        let sample = |cell, x, z| terra_world::lava::height_at(&chunks, &journal, cell, x, z);
        assert_eq!(sample(upper, 15.5, 6.5), 5.5);
        assert_eq!(sample(upper, 16., 6.5), sample(lower, 16., 6.5));
        assert!(left.positions.contains(&[16., 10., 10.]));
        assert!(left.positions.contains(&[16., 8., 10.]));
        assert!(left.normals.iter().any(|n| n[0] > 0.5 && n[1] > 0.5));
        for tri in left.indices.chunks_exact(3) {
            let [a, b, c] =
                [tri[0], tri[1], tri[2]].map(|i| Vec3::from_array(left.positions[i as usize]));
            assert!((b - a).cross(c - a).length_squared() > f32::EPSILON);
        }
        let dose = |y| {
            terra_world::lava::thermal_damage(
                &chunks,
                &journal,
                [15.5, y, 6.5],
                [0.1, 1.8, 0.1],
                0.,
            )
        };
        assert_eq!(dose(5.4), 8.);
        assert_eq!(dose(5.6), 0.);
    }

    #[test]
    fn generated_volcanic_channel_has_a_sloping_cap_across_negative_chunks() {
        use terra_core::{WorldConfig, WorldSeed};
        use terra_world::{SectionGenerator, TerraGenerator, streaming::LoadedSection};
        let terrain = TerraGenerator::new(WorldConfig {
            seed: WorldSeed(5_186_794_449_079_436_280),
            ..default()
        })
        .unwrap();
        let generator = SectionGenerator::new(terrain);
        let mut chunks = ChunkStore::default();
        for z in -248..=-247 {
            for x in -141..=-140 {
                let owner = SectionPos::new(x, 2, z);
                chunks.loaded.insert(
                    owner,
                    LoadedSection {
                        data: generator.generate(owner).unwrap(),
                        mesh: None,
                        mesh_epoch: 0,
                        water_epoch: 0,
                        water: None,
                        props: vec![],
                    },
                );
            }
        }
        let journal = EditJournal::default();
        let high = [-2240, 36, -3953];
        let low = [-2240, 35, -3952];
        assert_eq!(chunks.block_at(high), Some(LAVA));
        assert_eq!(chunks.block_at(low), Some(LAVA));
        let sample = |cell, z| terra_world::lava::height_at(&chunks, &journal, cell, -2239.5, z);
        assert_eq!(sample(high, -3953.), 37.);
        assert_eq!(sample(high, -3952.5), 36.5);
        assert_eq!(sample(high, -3952.), sample(low, -3952.));
        let (owner, _) = terra_voxel::global_to_section(high);
        let batch = render(&chunks, &journal, owner);
        assert!(!batch.flowing_lava);
        assert!(batch.normals.iter().any(|n| n[2] > 0.5 && n[1] > 0.5));
        assert!(batch.positions.contains(&[0., 4., 16.]));
    }

    #[test]
    fn partial_flow_restores_the_cap_beneath_wood_and_cross_section_model_contacts() {
        let mut chunks = ChunkStore::default();
        let mut journal = EditJournal::default();
        let p = [15, 3, 6];
        set(&mut chunks, p, LAVA);
        journal.record(p, LAVA).unwrap();
        set(&mut chunks, [15, 4, 6], MaterialId::Wood as u16);
        set(&mut chunks, [16, 3, 6], MaterialId::FossilBones as u16);
        let section = &chunks.loaded[&SectionPos::default()].data;
        let skin = capture(section, &chunks, &journal, |raw| {
            raw == MaterialId::FossilBones as u16
        });
        assert!(
            skin.contacts
                .iter()
                .any(|f| f.minimum == [15, 3, 6] && f.side == 1)
        );
        assert!(
            skin.contacts
                .iter()
                .any(|f| f.minimum == [15, 3, 6] && f.side == 3)
        );
        let mut batches = Vec::new();
        surface(&mut batches, &[], 3, &skin, |_| [0.; 2]);
        let batch = &batches[0];
        assert_eq!(batch.indices.len(), 12);
        assert!(batch.positions.iter().all(|p| p[1] < 4.));
        assert!(batch.positions.iter().any(|p| p[1] > 3.));
    }

    #[test]
    fn ash_mushrooms_in_the_stream_share_the_lava_surface_but_a_shoreline_stays_dry() {
        let mut chunks = ChunkStore::default();
        let mut journal = EditJournal::default();
        let prop = [15, 2, 5];
        for (p, raw) in [
            (prop, MaterialId::AshMushrooms as u16),
            ([14, 2, 5], LAVA),
            ([16, 2, 5], LAVA),
            ([14, 2, 6], MaterialId::FossilBones as u16),
        ] {
            set(&mut chunks, p, raw);
        }
        for moving in [false, true] {
            if moving {
                journal.record([16, 2, 5], LAVA).unwrap();
            }
            let section = &chunks.loaded[&SectionPos::default()].data;
            let skin = capture(section, &chunks, &journal, |_| false);
            let caps: Vec<_> = skin.contacts.iter().filter(|f| f.side == 3).collect();
            assert_eq!(caps.len(), 1);
            assert_eq!(caps[0].minimum, [15, 2, 5]);
            let mut batches = Vec::new();
            surface(&mut batches, &[], 17, &skin, |_| [0.; 2]);
            let batch = &batches[0];
            let cap = batch
                .positions
                .chunks_exact(4)
                .find(|points| {
                    points.iter().all(|p| p[1] > 2.) && points[0][0] == 15. && points[2][0] == 16.
                })
                .unwrap();
            for p in cap {
                let h =
                    terra_world::lava::height_at(&chunks, &journal, prop, p[0] as f64, p[2] as f64);
                assert!((h - f64::from(p[1])).abs() < 1e-6);
            }
            assert_eq!(
                terra_world::lava::thermal_damage(
                    &chunks,
                    &journal,
                    [15.5, 2.01, 5.5],
                    [0.3, 1.5, 0.3],
                    0.
                ),
                8.
            );
        }
    }

    #[test]
    fn inner_bank_corner_takes_melt_while_an_exposed_shore_plant_stays_dry() {
        let mut chunks = ChunkStore::default();
        let corner = [15, 3, 5];
        let plant = MaterialId::BurntGrass as u16;
        set(&mut chunks, corner, plant);
        for p in [[16, 3, 5], [15, 3, 6]] {
            set(&mut chunks, p, LAVA);
        }
        for p in [[14, 3, 5], [15, 3, 4]] {
            set(&mut chunks, p, MaterialId::Stone as u16);
        }
        assert!(terra_world::lava::occupies(&chunks, corner));
        let base = &chunks.loaded[&SectionPos::default()].data;
        let skin = capture(base, &chunks, &EditJournal::default(), |_| false);
        assert!(
            skin.contacts
                .iter()
                .any(|f| f.minimum == [15, 3, 5] && f.side == 3)
        );
        set(&mut chunks, [14, 3, 5], 0);
        assert!(!terra_world::lava::occupies(&chunks, corner));
    }

    #[test]
    fn mesh_only_distant_lava_keeps_the_gpu_current_without_vertex_history() {
        let mut batches = vec![MeshBatch {
            material: 9,
            positions: vec![[0., 0., 0.], [0., 0., 16.], [16., 0., 16.], [16., 0., 0.]],
            indices: vec![0, 1, 2, 0, 2, 3],
            ..default()
        }];
        surface(&mut batches, &[], 9, &Surface::default(), |_| [0.6, -0.8]);
        assert_eq!(batches[0].surface_water, vec![[0.6, -0.8]; 4]);
        assert!(!batches[0].flowing_lava); // GPU convection needs no simulation history.
    }

    #[test]
    fn moving_top_meets_all_four_sides_and_has_a_single_opaque_skin() {
        let mut padded = vec![0; terra_voxel::meshing::PADDED_VOLUME];
        padded[1 + 18 * (1 + 18)] = LAVA;
        let geometry = terra_voxel::meshing::mesh_section(&padded).unwrap();
        let heights = [[0.72, 0.66], [0.12, 0.08]];
        let mut corners = HashMap::new();
        for (x, row) in heights.into_iter().enumerate() {
            for (z, height) in row.into_iter().enumerate() {
                corners.insert(
                    [x as i32, 1, z as i32],
                    SurfaceCorner {
                        height,
                        downhill: [0.5, 0.1],
                    },
                );
            }
        }
        let mut batches = vec![MeshBatch {
            material: 9,
            ..default()
        }];
        surface(
            &mut batches,
            &geometry.faces,
            9,
            &Surface {
                corners,
                contacts: vec![],
                moving: true,
            },
            |_| [0.; 2],
        );
        let mesh = &batches[0];
        assert!(mesh.flowing_lava);
        assert_eq!(mesh.indices.len(), 36);
        assert_eq!(mesh.surface_water.len(), 24);
        for (&p, &attr) in mesh.positions.iter().zip(&mesh.surface_water) {
            if p[1] > 0. {
                assert_eq!(p[1], heights[p[0] as usize][p[2] as usize] as f32);
                assert_eq!(attr, [0.5, 0.1]);
            }
        }
        for tri in mesh.indices.chunks_exact(3) {
            let [a, b, c] =
                [tri[0], tri[1], tri[2]].map(|i| Vec3::from_array(mesh.positions[i as usize]));
            assert!(
                (b - a)
                    .cross(c - a)
                    .dot(Vec3::from_array(mesh.normals[tri[0] as usize]))
                    > 0.
            );
        }
    }
}
