use super::*;
use terra_world::conveyors::{Segment, Span};

struct Frame {
    mesh: Handle<Mesh>,
    materials: [Handle<StandardMaterial>; 4],
    bounds: Aabb,
}

#[derive(Default)]
pub(super) struct Cache {
    frames: Option<[Frame; 2]>,
}

impl Cache {
    pub(super) fn cached_meshes(&self) -> usize {
        self.frames.as_ref().map_or(0, |frames| frames.len())
    }

    fn prepare(
        &mut self,
        meshes: &mut Assets<Mesh>,
        materials: &mut Assets<StandardMaterial>,
        templates: &TemplateSet,
        assets: &BlockAssets,
    ) -> Option<()> {
        if self.frames.is_some() {
            return Some(());
        }
        let batches = geometry(templates)?;
        let sources = [
            assets.materials.get(batches[0].material)?.clone(),
            assets.materials.get(batches[1].material)?.clone(),
        ];
        if sources.iter().any(|handle| materials.get(handle).is_none()) {
            return None;
        }
        let wood = wood_materials(&sources[0], true, materials)?;
        let metal = std::array::from_fn(|_| sources[1].clone());
        let frames = batches
            .into_iter()
            .zip([wood, metal])
            .map(|(batch, materials)| {
                let mesh = Mesh::new(
                    PrimitiveTopology::TriangleList,
                    RenderAssetUsages::RENDER_WORLD,
                )
                .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, batch.positions)
                .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, batch.normals)
                .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, batch.uvs)
                .with_inserted_attribute(Mesh::ATTRIBUTE_TANGENT, batch.tangents)
                .with_inserted_indices(Indices::U32(batch.indices));
                let bounds = mesh.get_aabb()?;
                Some(Frame {
                    mesh: meshes.add(mesh),
                    materials,
                    bounds,
                })
            })
            .collect::<Option<Vec<_>>>()?;
        self.frames = Some(frames.try_into().ok()?);
        Some(())
    }
}

fn geometry(templates: &TemplateSet) -> Option<[MeshBatch; 2]> {
    let block = templates.blocks.get(&terra_core::kinetic::CONVEYOR)?;
    let mut groups: [Option<MeshBatch>; 2] = [None, None];
    for primitive in &block.model {
        if templates
            .crusher_moving_materials
            .contains_key(&primitive.material)
        {
            continue;
        }
        let index = usize::from(!templates.wood_materials.contains(&primitive.material));
        let batch = groups[index].get_or_insert_with(|| MeshBatch {
            material: primitive.material,
            ..Default::default()
        });
        if batch.material != primitive.material
            || primitive.positions.len() != primitive.normals.len()
            || primitive.positions.len() != primitive.uvs.len()
            || primitive
                .indices
                .iter()
                .any(|&i| i as usize >= primitive.positions.len())
        {
            return None;
        }
        let base = u32::try_from(batch.positions.len()).ok()?;
        batch.positions.extend(
            primitive
                .positions
                .iter()
                .map(|p| (Vec3::from_array(*p) - Vec3::splat(0.5)).to_array()),
        );
        batch.normals.extend_from_slice(&primitive.normals);
        batch.uvs.extend_from_slice(&primitive.uvs);
        batch
            .tangents
            .extend(primitive.normals.iter().enumerate().map(|(i, normal)| {
                primitive.tangents.get(i).copied().unwrap_or_else(|| {
                    crate::blocks::projected_tangent(Vec3::from_array(*normal), 3)
                })
            }));
        for &index in &primitive.indices {
            batch.indices.push(base.checked_add(index)?);
        }
    }
    let [wood, metal] = groups;
    let batches = [wood?, metal?];
    batches
        .iter()
        .all(|batch| !batch.indices.is_empty())
        .then_some(batches)
}

pub(super) fn mask(chunks: &ChunkStore, span: Span) -> u64 {
    if !span.linked
        || span.step[1] != 0
        || span.normal()[1] <= 0.0
        || span.length > terra_world::conveyors::MAX_SEGMENTS
    {
        return 0;
    }
    (-1..=i16::from(span.length)).fold(0, |mask, index| {
        if terra_world::interaction_shapes::conveyor_casing_supported(
            chunks,
            Segment { span, index },
        ) {
            mask | (1_u64 << (index + 1))
        } else {
            mask
        }
    })
}

fn cell_pose(span: Span, index: i16) -> Transform {
    let segment = Segment { span, index };
    let root = super::belts::pose(span);
    let inverse = root.rotation.inverse();
    // Subtract integer cells first so distant and negative worlds keep millimetre precision.
    let relative = Vec3::from_array(std::array::from_fn(|axis| {
        (segment.position()[axis] - span.start[axis]) as f32
    }));
    Transform::from_translation(inverse * relative).with_rotation(
        inverse * Quat::from_rotation_y(f32::from(span.facing % 4) * std::f32::consts::FRAC_PI_2),
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn spawn(
    commands: &mut Commands,
    cache: &mut Cache,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    templates: &TemplateSet,
    assets: &BlockAssets,
    span: Span,
    root: Entity,
    mask: u64,
) {
    if mask == 0
        || !span.linked
        || span.step[1] != 0
        || span.normal()[1] <= 0.0
        || span.length > terra_world::conveyors::MAX_SEGMENTS
        || cache
            .prepare(meshes, materials, templates, assets)
            .is_none()
    {
        return;
    }
    let frames = cache.frames.as_ref().unwrap();
    for index in -1..=i16::from(span.length) {
        if mask & (1_u64 << (index + 1)) == 0 {
            continue;
        }
        let pose = cell_pose(span, index);
        for frame in frames {
            commands.spawn((
                Name::new("Ground-supported conveyor casing"),
                Mesh3d(frame.mesh.clone()),
                MeshMaterial3d(frame.materials[wood_index(span.wood)].clone()),
                frame.bounds,
                NoAutoAabb,
                pose,
                crate::miniature::layers(),
                terra_core::miniature::MiniatureMesh::Terrain,
                ChildOf(root),
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks::{BlockTemplate, PrimitiveTemplate, definitions::BlockRenderKind};

    fn span(length: u8) -> Span {
        Span {
            controller: [-16, 4, -16],
            first: [-16, 4, -16],
            start: [-16, 4, -17],
            end: [-16, 4, -16 + i64::from(length)],
            step: [0, 0, 1],
            length,
            facing: 0,
            wood: terra_core::WoodSpecies::Birch,
            linked: true,
        }
    }

    fn fixture() -> (TemplateSet, BlockAssets, Assets<StandardMaterial>) {
        let mut templates = TemplateSet::default();
        templates.wood_materials.insert(0);
        templates.crusher_moving_materials.insert(2, 7);
        let primitive = |material| PrimitiveTemplate {
            positions: vec![[0.04, 0.0, 0.0], [0.16, 0.0, 0.0], [0.04, 0.6, 1.0]],
            normals: vec![[1.0, 0.0, 0.0]; 3],
            tangents: Vec::new(),
            uvs: vec![[0.0; 2]; 3],
            indices: vec![0, 1, 2],
            material,
        };
        templates.blocks.insert(
            148,
            BlockTemplate {
                surface: None,
                render_kind: BlockRenderKind::CustomMesh,
                bevel_width: 0.0,
                faces: std::array::from_fn(|_| Vec::new()),
                face_materials: [0; 6],
                side_cap: None,
                details: Vec::new(),
                model: vec![primitive(0), primitive(1), primitive(2)],
            },
        );
        let mut materials = Assets::default();
        let mut assets = BlockAssets::default();
        assets.materials = vec![
            materials.add(StandardMaterial::default()),
            materials.add(StandardMaterial::default()),
        ];
        (templates, assets, materials)
    }

    #[test]
    fn casing_geometry_excludes_moving_treads_and_cache_shares_two_meshes_and_metal() {
        let (templates, assets, mut materials) = fixture();
        let batches = geometry(&templates).unwrap();
        assert!(
            batches
                .iter()
                .all(|b| b.positions.len() == 3 && b.indices == [0, 1, 2])
        );
        assert!(
            batches
                .iter()
                .all(|b| b.positions.iter().all(|p| p[1] >= -0.5 && p[1] <= 0.101))
        );
        let mut meshes = Assets::default();
        let mut cache = Cache::default();
        for _ in 0..8 {
            cache
                .prepare(&mut meshes, &mut materials, &templates, &assets)
                .unwrap();
            assert_eq!(cache.cached_meshes(), 2);
            assert_eq!(meshes.len(), 2);
            assert_eq!(materials.len(), 5);
        }
        let frames = cache.frames.as_ref().unwrap();
        assert!(
            frames[0]
                .materials
                .windows(2)
                .all(|pair| pair[0] != pair[1])
        );
        assert!(
            frames[1]
                .materials
                .iter()
                .all(|m| *m == assets.materials[1])
        );
    }

    #[test]
    fn support_mask_includes_both_ends_and_all_32_cells_but_never_slopes_or_unloaded_floors() {
        use terra_voxel::{BlockSection, global_to_section};
        use terra_world::streaming::LoadedSection;
        let span = span(32);
        let mut chunks = ChunkStore::default();
        for index in -1..=32 {
            let mut p = Segment { span, index }.position();
            p[1] -= 1;
            let (section, local) = global_to_section(p);
            chunks
                .loaded
                .entry(section)
                .or_insert_with(|| LoadedSection {
                    data: BlockSection::empty(section).unwrap(),
                    mesh: None,
                    mesh_epoch: 0,
                    water_epoch: 0,
                    water: None,
                    props: Vec::new(),
                })
                .data
                .set_block(local, 3)
                .unwrap();
        }
        assert_eq!(mask(&chunks, span), (1_u64 << 34) - 1);
        let mut p = Segment { span, index: 0 }.position();
        p[1] -= 1;
        let (section, local) = global_to_section(p);
        chunks
            .loaded
            .get_mut(&section)
            .unwrap()
            .data
            .set_block(local, 82)
            .unwrap();
        assert_eq!(mask(&chunks, span) & 2, 0);
        chunks.loaded.remove(&section);
        assert_eq!(mask(&chunks, span) & 2, 0);
        assert_eq!(
            mask(
                &chunks,
                Span {
                    step: [0, 1, 1],
                    ..span
                }
            ),
            0
        );
        assert_eq!(
            mask(
                &chunks,
                Span {
                    step: [0, 1, 0],
                    ..span
                }
            ),
            0
        );
        assert_eq!(
            mask(
                &chunks,
                Span {
                    linked: false,
                    ..span
                }
            ),
            0
        );
        assert_eq!(
            mask(
                &chunks,
                Span {
                    step: [1, 0, 1],
                    facing: 4,
                    ..span
                }
            ),
            0
        );
    }

    #[test]
    fn casing_poses_preserve_authored_cell_coordinates_at_negative_borders_for_all_axes() {
        for facing in 0..4 {
            for sign in [-1, 1] {
                let step = terra_core::kinetic::forward(facing).map(|v| v * sign);
                let mut span = span(3);
                span.facing = facing;
                span.step = step;
                span.first = std::array::from_fn(|i| span.start[i] + step[i]);
                span.controller = span.first;
                span.end = std::array::from_fn(|i| span.start[i] + step[i] * 4);
                let root = super::super::belts::pose(span);
                for index in -1..=3 {
                    let pose = cell_pose(span, index);
                    let position = Segment { span, index }.position();
                    let yaw =
                        Quat::from_rotation_y(f32::from(facing) * std::f32::consts::FRAC_PI_2);
                    for point in [Vec3::new(-0.465, -0.5, -0.5), Vec3::new(0.465, 0.1, 0.5)] {
                        let expected =
                            Vec3::from_array(position.map(|v| v as f32 + 0.5)) + yaw * point;
                        assert!(
                            root.transform_point(pose.transform_point(point))
                                .abs_diff_eq(expected, 0.00001)
                        );
                    }
                }
            }
        }
    }
}
