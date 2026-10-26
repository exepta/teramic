//! One cached edge-only placement mesh, composed through the real placed-model path.
use super::*;
use crate::{
    blocks::TemplateSet,
    meshing::builder::{MeshBatch, RenderOptions},
};
use std::collections::{BTreeMap, BTreeSet};
use terra_world::edits::{BlockObjectState, EditJournal};

#[derive(Clone, Debug, PartialEq, Eq)]
struct Cell {
    relative: [i64; 3],
    material: u16,
    state: BlockObjectState,
    peer_wood: terra_core::WoodSpecies,
    neighbors: [Option<u16>; 6],
    neighbor_facing: [u8; 6],
    segment: Option<Segment>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Segment {
    position: [i64; 3],
    below: u8,
    above: u8,
    hanging: bool,
}

#[derive(Resource, Default)]
pub(super) struct Silhouette {
    cells: Vec<Cell>,
    mesh: Option<Handle<Mesh>>,
}

fn cells(
    plan: &terra_world::placement::Layout,
    origin: [i64; 3],
    chunks: &ChunkStore,
    journal: &EditJournal,
) -> Vec<Cell> {
    let material = |position| {
        plan.edits
            .iter()
            .find(|e| e.position == position)
            .map(|e| e.material)
            .or_else(|| chunks.block_at(position))
    };
    let state = |position| {
        plan.states
            .iter()
            .find(|(p, _)| *p == position)
            .map(|(_, s)| *s)
            .unwrap_or_else(|| journal.object_state(position))
            .unwrap_or_default()
    };
    let positions: BTreeSet<_> = plan
        .edits
        .iter()
        .map(|e| e.position)
        .chain(plan.states.iter().map(|(p, _)| *p))
        .collect();
    positions
        .into_iter()
        .filter_map(|position| {
            let material = material(position)?;
            let mut object = state(position);
            if material == 0 || (object.secondary && material != 59) {
                return None;
            }
            let peer_wood = object.peer.map(state).map_or(object.wood, |s| s.wood);
            object.peer = object
                .peer
                .map(|p| std::array::from_fn(|i| p[i] - origin[i]));
            let neighbors = std::array::from_fn(|side| {
                let mut neighbor = position;
                neighbor[side / 2] += if side % 2 == 0 { -1 } else { 1 };
                material_at(plan, chunks, neighbor)
            });
            let neighbor_facing = std::array::from_fn(|side| {
                let mut neighbor = position;
                neighbor[side / 2] += if side % 2 == 0 { -1 } else { 1 };
                state(neighbor).facing
            });
            let segment = terra_world::segments::is_segment(material).then(|| {
                let profile = terra_world::segments::profile(position, material, |p| {
                    material_at(plan, chunks, p)
                });
                Segment {
                    position,
                    below: profile.below,
                    above: profile.above,
                    hanging: profile.hanging,
                }
            });
            Some(Cell {
                relative: std::array::from_fn(|i| position[i] - origin[i]),
                material,
                state: object,
                peer_wood,
                neighbors,
                neighbor_facing,
                segment,
            })
        })
        .collect()
}

fn material_at(
    plan: &terra_world::placement::Layout,
    chunks: &ChunkStore,
    position: [i64; 3],
) -> Option<u16> {
    plan.edits
        .iter()
        .find(|e| e.position == position)
        .map(|e| e.material)
        .or_else(|| chunks.block_at(position))
}

fn geometry(cells: &[Cell], templates: &TemplateSet) -> Result<MeshBatch, String> {
    let mut combined = MeshBatch::default();
    for cell in cells {
        let block = templates
            .blocks
            .get(&cell.material)
            .ok_or("Missing placement template")?;
        let batches = if let Some(segment) = &cell.segment {
            let mut batches = std::collections::BTreeMap::new();
            crate::meshing::segments::append(
                &mut batches,
                block.face_materials[0],
                cell.material,
                segment.position,
                [0; 3],
                terra_world::segments::Profile {
                    below: segment.below,
                    above: segment.above,
                    hanging: segment.hanging,
                },
                crate::meshing::DetailLevel::Near,
            )?;
            batches.into_values().collect()
        } else if !block.model.is_empty() {
            crate::meshing::custom::model_geometry(
                cell.material,
                cell.state,
                cell.peer_wood,
                cell.neighbors,
                cell.neighbor_facing,
                templates,
            )?
        } else {
            let faces: Vec<_> = (0..6)
                .map(|side| terra_voxel::meshing::BlockFace {
                    minimum: [0; 3],
                    width: 1,
                    height: 1,
                    u_axis: (side / 2 + 1) % 3,
                    v_axis: (side / 2 + 2) % 3,
                    side,
                    material: cell.material,
                    occupied_neighbor: false,
                    edge_mask: 15,
                })
                .collect();
            crate::meshing::builder::compose_section_with_options(
                &faces,
                templates,
                terra_voxel::SectionPos::default(),
                &RenderOptions {
                    water_level: None,
                    detail_level: crate::meshing::DetailLevel::Far,
                    terrain: None,
                    exposure: None,
                    conditions: default(),
                },
            )?
        };
        for batch in batches {
            if batch.shadow_only {
                continue;
            }
            let base = combined.positions.len() as u32;
            combined.positions.extend(
                batch
                    .positions
                    .iter()
                    .map(|p| std::array::from_fn(|i| p[i] + cell.relative[i] as f32 - 0.5)),
            );
            combined.normals.extend(batch.normals);
            combined.uvs.extend(batch.uvs);
            combined
                .indices
                .extend(batch.indices.into_iter().map(|i| i + base));
            combined.indices.extend(
                batch
                    .compact_indices
                    .into_iter()
                    .map(|i| u32::from(i) + base),
            );
        }
    }
    Ok(combined)
}

struct OutlineEdge {
    ends: [[f32; 3]; 2],
    normal: Vec3,
    faces: usize,
    crease: bool,
}

fn outline_mesh(batch: &MeshBatch) -> Option<Mesh> {
    // Weld face-local vertices and discard coplanar triangle diagonals. A real
    // crease stays visible even where several authored mesh pieces share it.
    let key = |p: [f32; 3]| p.map(|v| (f64::from(v) * 100_000.0).round() as i64);
    let mut edges = BTreeMap::<_, OutlineEdge>::new();
    for triangle in batch.indices.chunks_exact(3) {
        let points = [
            *batch.positions.get(triangle[0] as usize)?,
            *batch.positions.get(triangle[1] as usize)?,
            *batch.positions.get(triangle[2] as usize)?,
        ];
        let normal = (Vec3::from(points[1]) - Vec3::from(points[0]))
            .cross(Vec3::from(points[2]) - Vec3::from(points[0]))
            .try_normalize();
        let Some(normal) = normal else { continue };
        for (a, b) in [(0, 1), (1, 2), (2, 0)] {
            let (a, b) = if key(points[a]) <= key(points[b]) {
                (points[a], points[b])
            } else {
                (points[b], points[a])
            };
            let pair = (key(a), key(b));
            if pair.0 == pair.1 {
                continue;
            }
            edges
                .entry(pair)
                .and_modify(|edge| {
                    edge.faces += 1;
                    edge.crease |= edge.normal.dot(normal).abs() < 0.9995;
                })
                .or_insert(OutlineEdge {
                    ends: [a, b],
                    normal,
                    faces: 1,
                    crease: false,
                });
        }
    }
    let edges: Vec<_> = edges
        .into_values()
        .filter(|edge| edge.faces == 1 || edge.crease)
        .collect();
    if edges.is_empty() {
        return None;
    }
    // Each edge is a degenerate local quad. Only the vertex shader expands its
    // two sides, so the outline stays exactly two pixels wide at any distance.
    let mut positions = Vec::with_capacity(edges.len() * 4);
    let mut deltas = Vec::with_capacity(edges.len() * 4);
    let mut corners = Vec::with_capacity(edges.len() * 4);
    let mut indices = Vec::with_capacity(edges.len() * 6);
    for edge in edges {
        let base = positions.len() as u32;
        let delta = (Vec3::from(edge.ends[1]) - Vec3::from(edge.ends[0])).to_array();
        positions.extend([edge.ends[0], edge.ends[0], edge.ends[1], edge.ends[1]]);
        deltas.extend([delta; 4]);
        corners.extend([[0.0, -1.0], [0.0, 1.0], [1.0, 1.0], [1.0, -1.0]]);
        indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    let mut mesh = Mesh::new(
        bevy::mesh::PrimitiveTopology::TriangleList,
        bevy::asset::RenderAssetUsages::MAIN_WORLD | bevy::asset::RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, deltas);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, corners);
    mesh.insert_indices(bevy::mesh::Indices::U32(indices));
    Some(mesh)
}

impl Silhouette {
    pub(super) fn prepare(
        &mut self,
        plan: &terra_world::placement::Layout,
        origin: [i64; 3],
        chunks: &ChunkStore,
        journal: &EditJournal,
        templates: &TemplateSet,
        meshes: &mut Assets<Mesh>,
    ) -> Option<Handle<Mesh>> {
        let cells = cells(plan, origin, chunks, journal);
        if self.mesh.is_none() || self.cells != cells {
            let batch = geometry(&cells, templates).ok()?;
            let next = outline_mesh(&batch)?;
            if let Some(mut mesh) = self.mesh.as_ref().and_then(|h| meshes.get_mut(h)) {
                *mesh = next;
            } else {
                self.mesh = Some(meshes.add(next));
            }
            self.cells = cells;
        }
        self.mesh.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks::{BlockTemplate, definitions::BlockRenderKind};
    use terra_core::block_properties::BlockPropertiesCatalog;

    #[test]
    fn outline_has_only_boundary_edges_and_never_coplanar_triangle_diagonals() {
        let batch = MeshBatch {
            positions: vec![
                [-0.5, 0.0, -0.5],
                [0.5, 0.0, -0.5],
                [0.5, 0.0, 0.5],
                [-0.5, 0.0, 0.5],
            ],
            indices: vec![0, 1, 2, 0, 2, 3],
            ..default()
        };
        let mesh = outline_mesh(&batch).unwrap();
        assert_eq!(
            mesh.primitive_topology(),
            bevy::mesh::PrimitiveTopology::TriangleList
        );
        assert_eq!(mesh.count_vertices(), 16);
        let Some(bevy::mesh::VertexAttributeValues::Float32x3(points)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("Missing line endpoints")
        };
        let Some(bevy::mesh::VertexAttributeValues::Float32x3(deltas)) =
            mesh.attribute(Mesh::ATTRIBUTE_NORMAL)
        else {
            panic!("Missing edge deltas")
        };
        let Some(bevy::mesh::VertexAttributeValues::Float32x2(corners)) =
            mesh.attribute(Mesh::ATTRIBUTE_UV_0)
        else {
            panic!("Missing screen-space corners")
        };
        for quad in points.chunks_exact(4) {
            assert_eq!(quad[0], quad[1]);
            assert_eq!(quad[2], quad[3]);
            let delta = Vec3::from(quad[0]) - Vec3::from(quad[2]);
            assert_eq!(delta.to_array().iter().filter(|&&v| v != 0.0).count(), 1);
        }
        for (edge, (quad, corners)) in points
            .chunks_exact(4)
            .zip(deltas.chunks_exact(4).zip(corners.chunks_exact(4)))
        {
            let delta = (Vec3::from(edge[2]) - Vec3::from(edge[0])).to_array();
            assert_eq!(quad, [delta; 4]);
            assert_eq!(corners, [[0.0, -1.0], [0.0, 1.0], [1.0, 1.0], [1.0, -1.0]]);
        }
        let indices: Vec<_> = mesh.indices().unwrap().iter().collect();
        for triangle in indices.chunks_exact(3) {
            let a = Vec3::from(points[triangle[0]]);
            let b = Vec3::from(points[triangle[1]]);
            let c = Vec3::from(points[triangle[2]]);
            assert_eq!((b - a).cross(c - a), Vec3::ZERO);
        }
        let mut invalid = batch;
        invalid.indices[0] = 99;
        assert!(outline_mesh(&invalid).is_none());
        assert!(outline_mesh(&MeshBatch::default()).is_none());
    }

    fn templates() -> TemplateSet {
        let properties = BlockPropertiesCatalog::default();
        let mut templates = TemplateSet::default();
        for material in [
            28, 29, 40, 50, 51, 52, 53, 54, 55, 56, 59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 70,
            71, 72, 73, 74, 75, 76, 77, 78, 79, 80, 81, 82, 83, 84, 85, 86, 87, 88, 89, 90, 91, 92,
            93, 94, 95, 96, 97, 98, 99, 100, 104, 105,
        ] {
            let model = crate::blocks::cube::box_model(properties.shape(material), 0.0, [0; 6]);
            templates.blocks.insert(
                material,
                BlockTemplate {
                    surface: None,
                    render_kind: BlockRenderKind::CustomMesh,
                    bevel_width: 0.0,
                    faces: std::array::from_fn(|_| Vec::new()),
                    face_materials: [0; 6],
                    side_cap: None,
                    details: Vec::new(),
                    model,
                },
            );
        }
        templates
    }

    #[test]
    fn ghost_matches_actual_model_vertices_for_all_orientations_and_mixed_halves() {
        let templates = templates();
        for material in [
            28, 29, 40, 59, 60, 65, 68, 71, 74, 75, 76, 80, 81, 82, 98, 99, 100, 104, 105,
        ] {
            let facings: Vec<_> = if terra_core::construction::is_slab(material) {
                (0..=terra_core::construction::LAST_MIXED_SLAB)
                    .filter(|&f| terra_core::construction::valid_facing(material, f))
                    .collect()
            } else if terra_core::construction::is_stair(material) {
                (0..24).collect()
            } else if material == 60 {
                (0..64)
                    .filter(|&f| terra_world::objects::chimney_basis(f).is_some())
                    .collect()
            } else if material == 40 {
                vec![0, 4, 5, 6, 7]
            } else if material == 98 {
                (0..8).collect()
            } else {
                (0..4).collect()
            };
            for facing in facings {
                let state = BlockObjectState {
                    facing,
                    peer: matches!(material, 28 | 29 | 59 | 65 | 71 | 99 | 100)
                        .then_some([1, 0, 0]),
                    ..default()
                };
                let cell = Cell {
                    relative: [0; 3],
                    material,
                    state,
                    peer_wood: default(),
                    neighbors: [None; 6],
                    neighbor_facing: [0; 6],
                    segment: None,
                };
                let expected = crate::meshing::custom::model_geometry(
                    material,
                    state,
                    default(),
                    [None; 6],
                    [0; 6],
                    &templates,
                )
                .unwrap();
                let actual = geometry(&[cell], &templates).unwrap();
                let positions: Vec<_> = expected
                    .iter()
                    .filter(|b| !b.shadow_only)
                    .flat_map(|b| b.positions.iter().map(|p| p.map(|v| v - 0.5)))
                    .collect();
                assert!(!positions.is_empty(), "{material}/{facing}");
                assert_eq!(actual.positions, positions, "{material}/{facing}");
                assert!(
                    actual
                        .indices
                        .iter()
                        .all(|&i| i < (actual.positions.len() as u32))
                );
            }
        }
    }

    #[test]
    fn ghost_preserves_existing_double_chest_owner_and_keeps_one_mesh_asset_across_rebuilds() {
        let templates = templates();
        let mut journal = EditJournal::default();
        let owner = [-17, 2, -1];
        journal.record(owner, 29).unwrap();
        journal
            .set_object_state(
                owner,
                Some(BlockObjectState {
                    wood: terra_core::WoodSpecies::Birch,
                    ..default()
                }),
            )
            .unwrap();
        let origin = [-16, 2, -1];
        let plan = terra_world::placement::Layout {
            edits: vec![terra_world::streaming::BlockEditRequest {
                position: origin,
                expected_material: 0,
                material: 29,
            }],
            states: vec![
                (
                    owner,
                    Some(BlockObjectState {
                        peer: Some(origin),
                        wood: terra_core::WoodSpecies::Birch,
                        ..default()
                    }),
                ),
                (
                    origin,
                    Some(BlockObjectState {
                        peer: Some(owner),
                        secondary: true,
                        ..default()
                    }),
                ),
            ],
        };
        let mut chunks = ChunkStore::default();
        for position in [owner, origin] {
            let (section, local) = terra_voxel::global_to_section(position);
            let mut data = terra_voxel::BlockSection::empty(section).unwrap();
            let mut floor = local;
            floor[1] -= 1;
            data.set_block(floor, 3).unwrap();
            if position == owner {
                data.set_block(local, 29).unwrap();
            }
            chunks.loaded.insert(
                section,
                terra_world::streaming::LoadedSection {
                    data,
                    mesh: None,
                    mesh_epoch: 0,
                    water_epoch: 0,
                    water: None,
                    props: Vec::new(),
                },
            );
        }
        let entries = cells(&plan, origin, &chunks, &journal);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].relative, [-1, 0, 0]);
        assert_eq!(entries[0].state.wood, terra_core::WoodSpecies::Birch);
        let mut meshes = Assets::<Mesh>::default();
        let mut cache = Silhouette::default();
        let handle = cache
            .prepare(&plan, origin, &chunks, &journal, &templates, &mut meshes)
            .unwrap();
        for facing in (0..4).cycle().take(200) {
            let mut rotated = plan.clone();
            rotated.states[0].1.as_mut().unwrap().facing = facing;
            assert_eq!(
                cache
                    .prepare(&rotated, origin, &chunks, &journal, &templates, &mut meshes)
                    .unwrap(),
                handle
            );
            assert_eq!(meshes.len(), 1);
        }
        assert!(
            meshes
                .get(&handle)
                .unwrap()
                .asset_usage
                .contains(bevy::asset::RenderAssetUsages::MAIN_WORLD)
        );
        let cleared = terra_world::placement::Layout {
            edits: Vec::new(),
            states: vec![(owner, None)],
        };
        let cleared_cells = cells(&cleared, owner, &chunks, &journal);
        assert_eq!(cleared_cells.len(), 1);
        assert_eq!(
            cleared_cells[0].state,
            BlockObjectState::default(),
            "an explicit None clears the old loaded object's state"
        );
        let mut cleared = plan;
        cleared.states = vec![
            (owner, None),
            (
                origin,
                Some(BlockObjectState {
                    secondary: true,
                    peer: Some(owner),
                    ..default()
                }),
            ),
        ];
        cleared
            .edits
            .push(terra_world::streaming::BlockEditRequest {
                position: owner,
                expected_material: 29,
                material: 29,
            });
        assert_eq!(
            cells(&cleared, owner, &chunks, &journal)[0].state,
            BlockObjectState::default(),
            "an explicit None clears old facing and wood instead of inheriting the journal"
        );
    }

    #[test]
    fn mineral_ghost_uses_the_actual_floor_or_ceiling_chain_profile_across_negative_section_edges()
    {
        let templates = templates();
        let journal = EditJournal::default();
        let mut cache = Silhouette::default();
        let mut meshes = Assets::<Mesh>::default();
        let mut original = None;
        for position in [[-17, 15, -1], [-16, 16, 0]] {
            for material in 51..=53 {
                for hanging in [false, true] {
                    let mut chunks = ChunkStore::default();
                    for n in 0..=3 {
                        let p = [
                            position[0],
                            position[1] + n * if hanging { 1 } else { -1 },
                            position[2],
                        ];
                        let (section, local) = terra_voxel::global_to_section(p);
                        let loaded = chunks.loaded.entry(section).or_insert_with(|| {
                            terra_world::streaming::LoadedSection {
                                data: terra_voxel::BlockSection::empty(section).unwrap(),
                                mesh: None,
                                mesh_epoch: 0,
                                water_epoch: 0,
                                water: None,
                                props: Vec::new(),
                            }
                        });
                        loaded
                            .data
                            .set_block(
                                local,
                                if n == 0 {
                                    0
                                } else if n == 3 {
                                    3
                                } else {
                                    material
                                },
                            )
                            .unwrap();
                    }
                    let plan = terra_world::placement::Layout {
                        edits: vec![terra_world::streaming::BlockEditRequest {
                            position,
                            expected_material: 0,
                            material,
                        }],
                        states: Vec::new(),
                    };
                    let entries = cells(&plan, position, &chunks, &journal);
                    let segment = entries[0].segment.as_ref().unwrap();
                    assert_eq!(segment.hanging, hanging);
                    assert_eq!(
                        (segment.below, segment.above),
                        if hanging { (0, 2) } else { (2, 0) }
                    );
                    let profile = terra_world::segments::profile(position, material, |p| {
                        material_at(&plan, &chunks, p)
                    });
                    let mut rendered = std::collections::BTreeMap::new();
                    crate::meshing::segments::append(
                        &mut rendered,
                        0,
                        material,
                        position,
                        [0; 3],
                        profile,
                        crate::meshing::DetailLevel::Near,
                    )
                    .unwrap();
                    let expected: Vec<_> = rendered
                        .values()
                        .flat_map(|b| b.positions.iter().map(|p| p.map(|v| v - 0.5)))
                        .collect();
                    let actual = geometry(&entries, &templates).unwrap();
                    assert!(!actual.positions.is_empty());
                    assert_eq!(
                        actual.positions, expected,
                        "{position:?}/{material}/{hanging}"
                    );
                    let handle = cache
                        .prepare(&plan, position, &chunks, &journal, &templates, &mut meshes)
                        .unwrap();
                    if let Some(original) = &original {
                        assert_eq!(&handle, original);
                    } else {
                        original = Some(handle);
                    }
                    assert_eq!(meshes.len(), 1);
                }
            }
        }
    }
}
