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
    lava::{LAVA, POROUS_DECORATIONS, SurfaceCorner, surface_corner},
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
    let corners = surface_corners(owner, chunks, journal);
    let mut contacts = Vec::new();
    let cell_index = |index: usize| {
        [
            (index % 16) as u32,
            ((index / 16) % 16) as u32,
            (index / 256) as u32,
        ]
    };
    let global_cell = |local: [u32; 3]| std::array::from_fn(|i| origin[i] + i64::from(local[i]));
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
        return Surface { corners, contacts };
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
    Surface { corners, contacts }
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
            flowing_lava: true,
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
        for u in 0..face.width {
            for v in 0..face.height {
                let mut cell = face.minimum;
                cell[face.u_axis] += u;
                cell[face.v_axis] += v;
                let points = QUADS[face.side].map(|d| {
                    let corner = std::array::from_fn(|i| cell[i] as i32 + d[i]);
                    let mut pos = Vec3::from_array(corner.map(|x| x as f32));
                    let sample = corners.get(&corner).filter(|_| d[1] == 1);
                    if let Some(sample) = sample {
                        pos.y = (corner[1] - 1) as f32 + sample.height as f32;
                    }
                    batch.surface_water.push(
                        sample.map_or_else(|| natural_current(pos.to_array()), |s| s.downhill),
                    );
                    pos
                });
                let start = batch.positions.len() as u32;
                batch.positions.extend(points.map(|v| v.to_array()));
                let normal = (points[2] - points[0])
                    .cross(points[3] - points[1])
                    .normalize_or_zero();
                batch.normals.extend([normal.to_array(); 4]);
                batch.uvs.extend([[0., 1.], [1., 1.], [1., 0.], [0., 0.]]);
                batch
                    .indices
                    .extend([start, start + 1, start + 2, start, start + 2, start + 3]);
            }
        }
    }
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
