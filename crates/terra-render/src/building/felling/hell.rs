//! Independently falling cubes, one GPU-animated batch per material, not per block.
use super::{geometry::Snapshot, *};
use bevy::{
    mesh::{MeshVertexAttribute, MeshVertexBufferLayoutRef},
    pbr::{ExtendedMaterial, MaterialExtension, MaterialExtensionKey, MaterialExtensionPipeline},
    render::render_resource::{
        AsBindGroup, RenderPipelineDescriptor, SpecializedMeshPipelineError, VertexFormat,
    },
    shader::ShaderRef,
};
use std::collections::{BTreeMap, HashMap};

pub(super) const ATTRIBUTE: MeshVertexAttribute =
    MeshVertexAttribute::new("TreeFallCell", 1_244_125, VertexFormat::Float32x4);

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub(in crate::building) struct FallExtension {
    #[uniform(100)]
    pub age: Vec4,
}

impl MaterialExtension for FallExtension {
    fn vertex_shader() -> ShaderRef {
        "shaders/terra_tree_fall.wgsl".into()
    }
    fn prepass_vertex_shader() -> ShaderRef {
        Self::vertex_shader()
    }
    fn deferred_vertex_shader() -> ShaderRef {
        Self::vertex_shader()
    }
    fn specialize(
        _: &MaterialExtensionPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        layout: &MeshVertexBufferLayoutRef,
        _: MaterialExtensionKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        let extra = layout.0.get_layout(&[ATTRIBUTE.at_shader_location(8)])?;
        descriptor.vertex.buffers[0]
            .attributes
            .extend(extra.attributes);
        Ok(())
    }
}

pub(in crate::building) type FallMaterial = ExtendedMaterial<StandardMaterial, FallExtension>;

pub(super) struct Batch {
    pub mesh: crate::meshing::builder::MeshBatch,
    pub cells: Vec<[f32; 4]>,
}

#[derive(Component)]
pub(super) struct FallingBatch {
    pub age: f32,
    pub materials: Vec<Handle<FallMaterial>>,
}

/// Snapshot each footprint column once. Unknown ground is conservatively the
/// tree base; the render effect never loads chunks or starts a physics body.
pub(super) fn grounds(
    event: &Collapse,
    chunks: &terra_world::streaming::ChunkStore,
) -> HashMap<[i64; 2], f32> {
    let mut lowest = HashMap::<[i64; 2], i64>::new();
    let pivot = event.visual_origin();
    for section in &event.sections {
        for (index, _) in section.cells() {
            let Some(cell) = event.cell_position(section.position, index) else {
                continue;
            };
            lowest
                .entry([cell[0], cell[2]])
                .and_modify(|y| *y = (*y).min(cell[1]))
                .or_insert(cell[1]);
        }
    }
    lowest
        .into_iter()
        .map(|(column, y)| {
            // Begin below every owned cell in this column, not below the trunk's
            // global pivot: a hillside can rise beneath the side of the crown.
            (
                column,
                ground_below(chunks, column, y).unwrap_or(pivot[1] as f32),
            )
        })
        .collect()
}

fn ground_below(
    chunks: &terra_world::streaming::ChunkStore,
    column: [i64; 2],
    lowest: i64,
) -> Option<f32> {
    for offset in 1..=24 {
        let y = lowest.checked_sub(offset)?;
        // Never look through an unknown section and mistake deeper loaded
        // terrain for the missing hillside. Unknown uses the base fallback.
        let material = chunks.block_at([column[0], y, column[1]])?;
        if !matches!(material, 0 | 7..=9 | 132 | 133 | 14 | 27 | 40 | 50..=56) {
            return Some(y as f32 + 1.0);
        }
    }
    None
}

pub(super) fn prepare(
    event: &Collapse,
    snapshots: &[Snapshot],
    templates: &crate::blocks::TemplateSet,
    grounds: &HashMap<[i64; 2], f32>,
) -> Result<Vec<Batch>, String> {
    let mut result = BTreeMap::<usize, Batch>::new();
    let pivot = event.visual_origin();
    for section in &event.sections {
        let origin = section
            .position
            .origin()
            .map_err(|error| error.to_string())?;
        let options = snapshots
            .first()
            .map(|s| s.options.clone())
            .unwrap_or(RenderOptions {
                water_level: None,
                detail_level: crate::meshing::DetailLevel::Middle,
                terrain: None,
                exposure: None,
                conditions: Default::default(),
            });
        for (index, wood) in section.cells() {
            let cell = event
                .cell_position(section.position, index)
                .ok_or("Invalid falling tree cell")?;
            let local = [
                u32::from(index % 16),
                u32::from(index / 16 % 16),
                u32::from(index / 256),
            ];
            let faces = super::super::mining_geometry::full_faces(
                local,
                if wood {
                    event.wood_material
                } else {
                    event.leaves_material
                },
            );
            let batches = crate::meshing::builder::compose_section_with_options(
                &faces,
                templates,
                section.position,
                &options,
            )?;
            let center =
                std::array::from_fn::<_, 3, _>(|axis| (cell[axis] - pivot[axis]) as f32 + 0.5);
            let distance = (cell[1] as f32
                - grounds
                    .get(&[cell[0], cell[2]])
                    .copied()
                    .unwrap_or(pivot[1] as f32))
            .max(0.0);
            let trajectory = [center[0], center[1], center[2], distance];
            for mut mesh in batches {
                for position in &mut mesh.positions {
                    for axis in 0..3 {
                        position[axis] += (origin[axis] - pivot[axis]) as f32;
                    }
                }
                let batch = result.entry(mesh.material).or_insert_with(|| Batch {
                    mesh: crate::meshing::builder::MeshBatch {
                        material: mesh.material,
                        ..Default::default()
                    },
                    cells: Vec::new(),
                });
                batch
                    .cells
                    .extend(std::iter::repeat_n(trajectory, mesh.positions.len()));
                let offset = batch.mesh.positions.len() as u32;
                batch.mesh.positions.extend(mesh.positions);
                batch.mesh.normals.extend(mesh.normals);
                batch.mesh.uvs.extend(mesh.uvs);
                // Crown metadata is not alpha in a normal PBR material.
                batch
                    .mesh
                    .colors
                    .extend(mesh.colors.into_iter().map(|mut color| {
                        color[3] = 1.0;
                        color
                    }));
                batch
                    .mesh
                    .indices
                    .extend(mesh.indices.into_iter().map(|i| i + offset));
            }
        }
    }
    Ok(result.into_values().collect())
}

pub(super) fn animate(
    mut commands: Commands,
    world: BuildingWorld,
    mut batches: Query<(Entity, &mut FallingBatch)>,
    mut materials: ResMut<Assets<FallMaterial>>,
) {
    for (entity, mut batch) in &mut batches {
        batch.age += world.time.delta_secs().min(0.1);
        if !world.active() || batch.age >= 5.0 {
            commands.entity(entity).try_despawn();
            continue;
        }
        for handle in &batch.materials {
            if let Some(mut material) = materials.get_mut(handle) {
                material.extension.age =
                    Vec4::new(batch.age, 5.0, world.time.delta_secs().min(0.1), 0.0);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_world::tree_felling::CollapseSection;

    fn terrain_cell(
        chunks: &mut terra_world::streaming::ChunkStore,
        position: [i64; 3],
        material: u16,
    ) {
        let (section, local) = terra_voxel::global_to_section(position);
        chunks
            .loaded
            .entry(section)
            .or_insert_with(|| terra_world::streaming::LoadedSection {
                data: terra_voxel::BlockSection::empty(section).unwrap(),
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: vec![],
            })
            .data
            .set_block(local, material)
            .unwrap();
    }

    #[test]
    fn hillside_crown_uses_local_high_ground_without_landing_on_its_own_root() {
        let mut chunks = terra_world::streaming::ChunkStore::default();
        let mut event = Collapse {
            foot: [0, 8, 0],
            direction: [1.0, 0.0],
            wood_material: 6,
            leaves_material: 7,
            sections: vec![],
        };
        for (cell, wood) in [
            ([0, 8, 0], true),
            ([0, 10, 0], true),
            ([4, 18, 0], true),
            ([4, 22, 0], false),
        ] {
            let (position, local) = terra_voxel::global_to_section(cell);
            let i = (local[0] + 16 * (local[1] + 16 * local[2])) as u16;
            let slot = if let Some(index) = event
                .sections
                .iter()
                .position(|section| section.position == position)
            {
                index
            } else {
                event.sections.push(CollapseSection {
                    position,
                    wood: vec![],
                    leaves: vec![],
                });
                event.sections.len() - 1
            };
            if wood {
                event.sections[slot].wood.push(i);
            } else {
                event.sections[slot].leaves.push(i);
            }
            terrain_cell(&mut chunks, cell, if wood { 6 } else { 7 });
        }
        terrain_cell(&mut chunks, [0, 7, 0], 3);
        terrain_cell(&mut chunks, [4, 16, 0], 3);
        assert!(event.valid());
        let result = grounds(&event, &chunks);
        assert_eq!(
            result[&[0, 0]],
            8.0,
            "start below the lowest own root, not an upper log"
        );
        assert_eq!(
            result[&[4, 0]],
            17.0,
            "crown must land on the higher hillside"
        );
    }

    #[test]
    fn missing_sections_and_exhausted_ground_probe_keep_the_tree_base_fallback() {
        let mut chunks = terra_world::streaming::ChunkStore::default();
        let event = Collapse {
            foot: [0, 8, 0],
            direction: [1.0, 0.0],
            wood_material: 6,
            leaves_material: 7,
            sections: vec![
                CollapseSection {
                    position: SectionPos::new(0, 0, 0),
                    wood: vec![16 * 8],
                    leaves: vec![],
                },
                CollapseSection {
                    position: SectionPos::new(0, 2, 0),
                    wood: vec![],
                    leaves: vec![2 + 16 * 8],
                },
                CollapseSection {
                    position: SectionPos::new(1, 1, 0),
                    wood: vec![],
                    leaves: vec![4 + 16 * 6],
                },
            ],
        };
        terrain_cell(&mut chunks, [0, 7, 0], 3);
        terrain_cell(&mut chunks, [2, 40, 0], 7);
        terrain_cell(&mut chunks, [2, 7, 0], 3);
        assert!(event.valid());
        let result = grounds(&event, &chunks);
        assert_eq!(
            result[&[2, 0]],
            8.0,
            "unknown middle section must stop the search"
        );
        assert_eq!(
            result[&[20, 0]],
            8.0,
            "unloaded footprint uses the base fallback"
        );
        terrain_cell(&mut chunks, [2, 16, 0], 0);
        assert_eq!(
            ground_below(&chunks, [2, 0], 40),
            None,
            "search remains bounded to 24 cells"
        );
    }

    #[test]
    fn every_cube_keeps_closed_faces_and_own_trajectory_in_one_material_batch() {
        let event = Collapse {
            foot: [-1, 4, -1],
            direction: [1.0, 0.0],
            wood_material: 6,
            leaves_material: 7,
            sections: vec![CollapseSection {
                position: SectionPos::new(-1, 0, -1),
                wood: vec![15 + 16 * 3 + 256 * 15, 15 + 16 * 4 + 256 * 15],
                leaves: vec![],
            }],
        };
        let mut templates = crate::blocks::TemplateSet::default();
        templates.blocks.insert(
            6,
            crate::blocks::BlockTemplate {
                surface: None,
                render_kind: crate::blocks::definitions::BlockRenderKind::Cube,
                bevel_width: 0.0,
                faces: crate::blocks::cube_faces(0.0, [0; 6], None),
                face_materials: [0; 6],
                side_cap: None,
                details: vec![],
                model: vec![],
            },
        );
        let batches = prepare(&event, &[], &templates, &HashMap::from([([-1, -1], 2.0)])).unwrap();
        assert_eq!(
            batches.len(),
            1,
            "batch by material, never one entity per block"
        );
        let batch = &batches[0];
        assert_eq!(batch.mesh.positions.len(), 48);
        assert_eq!(batch.cells.len(), batch.mesh.positions.len());
        assert_eq!(batch.mesh.indices.len(), 72);
        assert!(batch.cells[..24].iter().all(|cell| cell[3] == 1.0));
        assert!(batch.cells[24..].iter().all(|cell| cell[3] == 2.0));
        assert!(
            batch
                .mesh
                .indices
                .iter()
                .all(|&i| (i as usize) < batch.cells.len())
        );
        let mesh = crate::previews::batch_mesh(&batch.mesh)
            .with_inserted_attribute(ATTRIBUTE, batch.cells.clone());
        assert!(mesh.attribute(ATTRIBUTE).is_some());
        assert_eq!(mesh.count_vertices(), 48);
    }
}
