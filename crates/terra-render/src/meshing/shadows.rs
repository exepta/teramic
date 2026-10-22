//! Shadow-only cube shells omit subpixel relief, retaining leaf cutouts and wind.
use crate::{
    blocks::TemplateSet,
    meshing::{
        DetailLevel,
        builder::{MeshBatch, append_greedy},
    },
};
use bevy::prelude::*;
use std::collections::{BTreeMap, BTreeSet};
use terra_voxel::meshing::BlockFace;

pub(crate) const LAYER: usize = 61;
#[derive(Component)]
pub(crate) struct ShadowProxy;

#[derive(Component)]
pub(crate) struct ReplacedShadowCaster;

pub(crate) fn enabled(graphics: Option<&terra_core::GraphicsConfig>) -> bool {
    graphics.is_none_or(|graphics| graphics.shadow_quality != terra_core::ShadowQuality::Off)
}

pub(crate) fn release_disabled(
    mut commands: Commands,
    graphics: Option<Res<terra_core::GraphicsConfig>>,
    proxies: Query<(Entity, &Mesh3d), With<ShadowProxy>>,
    originals: Query<Entity, With<ReplacedShadowCaster>>,
) {
    if enabled(graphics.as_deref()) || (proxies.is_empty() && originals.is_empty()) {
        return;
    }
    let proxies: Vec<_> = proxies
        .iter()
        .map(|(entity, mesh)| (entity, mesh.id()))
        .collect();
    let originals: Vec<_> = originals.iter().collect();
    commands.queue(move |world: &mut World| {
        let _scope = terra_core::profiling::ScopeTimer::new("terrain.shadows.release");
        let removed: std::collections::HashSet<_> = proxies.iter().map(|(_, mesh)| *mesh).collect();
        for (entity, _) in proxies {
            let _ = world.despawn(entity);
        }
        if let Some(mut meshes) = world.get_resource_mut::<Assets<Mesh>>() {
            for &mesh in &removed {
                meshes.remove(mesh);
            }
        }
        if !removed.is_empty()
            && let Some(mut rendered) = world.get_resource_mut::<super::RenderedSections>()
        {
            for section in rendered.bypass_change_detection().sections.values_mut() {
                section.meshes.retain(|mesh| !removed.contains(&mesh.id()));
            }
        }
        for entity in originals {
            if world.get::<ReplacedShadowCaster>(entity).is_some() {
                world.entity_mut(entity).remove::<ReplacedShadowCaster>();
                // Full color geometry remains a correct caster when shadows
                // return before the next optimized section composition.
                crate::transitions::restore_proxy_caster(world, entity);
            }
        }
    });
}

pub(crate) fn append(
    batches: &mut Vec<MeshBatch>,
    faces: &[BlockFace],
    templates: &TemplateSet,
    level: DetailLevel,
) -> Result<(), String> {
    if level != DetailLevel::Near || full_geometry_reference() {
        return Ok(());
    }
    let mut shadows = BTreeMap::new();
    let mut masked_materials = BTreeSet::new();
    // Color batches can combine model and cube geometry with shared materials.
    // Keep those casters intact rather than replacing the model's shadow too.
    let model_materials: BTreeSet<_> = templates
        .blocks
        .values()
        .flat_map(|block| block.model.iter().map(|primitive| primitive.material))
        .collect();
    for &face in faces {
        if let Some(block) = templates.blocks.get(&face.material) {
            if block.render_kind != crate::blocks::definitions::BlockRenderKind::Cube
                || model_materials.contains(&block.face_materials[face.side])
                || block.side_cap.is_some_and(|cap| {
                    face.side / 2 != 1 && model_materials.contains(&cap.material)
                })
                || templates.water_material(block.face_materials[face.side])
                || templates.glass_material(block.face_materials[face.side])
                || templates
                    .ore_materials
                    .get(&face.material)
                    .is_some_and(|material| block.face_materials.contains(material))
            {
                continue;
            }
            if matches!(face.material, 7..=9) {
                masked_materials.insert(block.face_materials[face.side]);
            }
            append_greedy(&mut shadows, face, block)?;
        }
    }
    let mut opaque: Option<MeshBatch> = None;
    for mut batch in shadows.into_values() {
        batch.shadow_only = true;
        batch.shadow_sources.push(batch.material);
        // Opaque depth casters need only position; cutout foliage keeps its UVs
        // and original wind shader. No reduced color or collision geometry.
        if !masked_materials.contains(&batch.material) {
            recessed_shell(&mut batch)?;
            batch.normals = Vec::new();
            batch.uvs = Vec::new();
            // A visible cave ceiling faces down. Its back must still block
            // sunlight when the terrain surface above it is not streamed.
            // Reuse the same vertices/material/draw, leaving cutout leaves alone.
            let front_indices = batch.indices.len();
            for i in (0..front_indices).step_by(3) {
                let (a, b, c) = (batch.indices[i], batch.indices[i + 1], batch.indices[i + 2]);
                batch.indices.extend_from_slice(&[a, c, b]);
            }
            if let Some(combined) = &mut opaque {
                let base = u32::try_from(combined.positions.len())
                    .map_err(|_| "Shadow mesh index overflow")?;
                combined.positions.extend(batch.positions);
                for index in batch.indices {
                    combined.indices.push(
                        base.checked_add(index)
                            .ok_or("Shadow mesh index overflow")?,
                    );
                }
                combined.shadow_sources.extend(batch.shadow_sources);
            } else {
                opaque = Some(batch);
            }
        } else {
            batches.push(batch);
        }
    }
    if let Some(batch) = opaque {
        batches.push(batch);
    }
    Ok(())
}

fn recessed_shell(batch: &mut MeshBatch) -> Result<(), String> {
    let mut positions = Vec::with_capacity(batch.positions.len() * 2);
    let mut indices = Vec::with_capacity(batch.indices.len() * 5);
    for (quad, normal) in batch
        .positions
        .chunks_exact(4)
        .zip(batch.normals.chunks_exact(4))
    {
        let base = u32::try_from(positions.len()).map_err(|_| "Shadow shell overflow")?;
        let center: [f32; 3] =
            std::array::from_fn(|axis| quad.iter().map(|p| p[axis]).sum::<f32>() * 0.25);
        positions.extend_from_slice(quad);
        for point in quad {
            positions.push(std::array::from_fn(|axis| {
                if normal[0][axis] != 0.0 {
                    point[axis] - normal[0][axis] * 0.065
                } else {
                    let delta = center[axis] - point[axis];
                    point[axis] + delta.signum() * delta.abs().min(0.065)
                }
            }));
        }
        // Canonical shared outer edges remain closed across cells/sections.
        // Only the broad interior retreats behind the color surface relief.
        for [a, b, c, d] in [
            [0, 1, 5, 4],
            [1, 2, 6, 5],
            [2, 3, 7, 6],
            [3, 0, 4, 7],
            [4, 5, 6, 7],
        ] {
            indices.extend([a, b, c, a, c, d].map(|i| base + i));
        }
    }
    batch.positions = positions;
    batch.indices = indices;
    Ok(())
}

fn full_geometry_reference() -> bool {
    static FULL: std::sync::LazyLock<bool> = std::sync::LazyLock::new(|| {
        std::env::var("TERRA_BENCH_FULL_SHADOWS").is_ok_and(|value| value == "1")
            && std::env::args().any(|arg| arg.starts_with("--smoke-test"))
    });
    *FULL
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_explicitly_disabled_shadow_quality_skips_proxies() {
        assert!(enabled(None));
        for quality in [
            terra_core::ShadowQuality::Off,
            terra_core::ShadowQuality::Low,
            terra_core::ShadowQuality::Medium,
            terra_core::ShadowQuality::High,
        ] {
            assert_eq!(
                enabled(Some(&terra_core::GraphicsConfig {
                    shadow_quality: quality,
                    ..default()
                })),
                quality != terra_core::ShadowQuality::Off
            );
        }
    }

    #[test]
    fn disabling_proxies_releases_active_and_retired_meshes_but_preserves_color_geometry() {
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<super::super::RenderedSections>()
            .init_resource::<terra_core::GraphicsConfig>()
            .add_systems(Update, release_disabled);
        let handles: Vec<_> = (0..4)
            .map(|_| {
                app.world_mut()
                    .resource_mut::<Assets<Mesh>>()
                    .add(Rectangle::new(1.0, 1.0))
            })
            .collect();
        let root = app
            .world_mut()
            .spawn((Transform::default(), Visibility::Inherited))
            .id();
        let color = app
            .world_mut()
            .spawn((
                Mesh3d(handles[0].clone()),
                ReplacedShadowCaster,
                bevy::light::NotShadowCaster,
                ChildOf(root),
            ))
            .id();
        let glass = app
            .world_mut()
            .spawn((
                Mesh3d(handles[1].clone()),
                bevy::light::NotShadowCaster,
                ChildOf(root),
            ))
            .id();
        let proxy = app
            .world_mut()
            .spawn((Mesh3d(handles[2].clone()), ShadowProxy, ChildOf(root)))
            .id();
        let retired_root = app.world_mut().spawn_empty().id();
        let retired = app
            .world_mut()
            .spawn((
                Mesh3d(handles[3].clone()),
                ShadowProxy,
                ChildOf(retired_root),
            ))
            .id();
        let position = terra_voxel::SectionPos::default();
        app.world_mut()
            .resource_mut::<super::super::RenderedSections>()
            .sections
            .insert(
                position,
                super::super::RenderedSection {
                    micro_detail: true,
                    appearance_epoch: 0,
                    entity: root,
                    meshes: handles[..3].to_vec(),
                    mesh_epoch: 0,
                    detail_level: DetailLevel::Near,
                },
            );
        app.update();
        assert!(app.world().get_entity(proxy).is_ok());
        app.world_mut()
            .resource_mut::<terra_core::GraphicsConfig>()
            .shadow_quality = terra_core::ShadowQuality::Off;
        app.update();
        assert!(app.world().get_entity(proxy).is_err());
        assert!(app.world().get_entity(retired).is_err());
        assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 2);
        assert_eq!(app.world().get::<Mesh3d>(color).unwrap().0, handles[0]);
        assert!(
            app.world()
                .get::<bevy::light::NotShadowCaster>(color)
                .is_none()
        );
        assert!(
            app.world()
                .get::<bevy::light::NotShadowCaster>(glass)
                .is_some()
        );
        assert!(app.world().get::<ReplacedShadowCaster>(color).is_none());
        assert_eq!(
            app.world()
                .resource::<super::super::RenderedSections>()
                .sections[&position]
                .meshes,
            handles[..2]
        );
        for quality in [
            terra_core::ShadowQuality::High,
            terra_core::ShadowQuality::Off,
        ] {
            app.world_mut()
                .resource_mut::<terra_core::GraphicsConfig>()
                .shadow_quality = quality;
            app.update();
            assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 2);
            assert!(
                app.world()
                    .get::<bevy::light::NotShadowCaster>(color)
                    .is_none()
            );
            assert!(
                app.world()
                    .get::<bevy::light::NotShadowCaster>(glass)
                    .is_some()
            );
        }
    }

    #[test]
    fn custom_models_sharing_a_cube_material_keep_their_original_depth_geometry() {
        let mut templates = TemplateSet::default();
        let cube = crate::blocks::BlockTemplate {
            render_kind: crate::blocks::definitions::BlockRenderKind::Cube,
            face_materials: [4; 6],
            side_cap: None,
            faces: Default::default(),
            model: vec![],
            details: vec![],
            bevel_width: 0.22,
            surface: None,
        };
        let mut model = cube.clone();
        model.render_kind = crate::blocks::definitions::BlockRenderKind::CustomMesh;
        model.model.push(crate::blocks::PrimitiveTemplate {
            positions: vec![],
            normals: vec![],
            uvs: vec![],
            indices: vec![],
            material: 4,
        });
        templates.blocks.insert(3, cube);
        templates.blocks.insert(20, model);
        let face = BlockFace {
            minimum: [0; 3],
            width: 16,
            height: 8,
            side: 3,
            u_axis: 0,
            v_axis: 2,
            material: 3,
            edge_mask: 15,
            occupied_neighbor: false,
        };
        let mut batches = Vec::new();
        append(&mut batches, &[face], &templates, DetailLevel::Near).unwrap();
        assert!(batches.is_empty());
        templates.blocks.get_mut(&20).unwrap().model[0].material = 5;
        append(&mut batches, &[face], &templates, DetailLevel::Near).unwrap();
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].shadow_sources, vec![4]);
    }
    #[test]
    fn proxy_is_a_separate_tiny_masked_shell_without_changing_color_geometry() {
        let mut templates = TemplateSet::default();
        templates.blocks.insert(
            7,
            crate::blocks::BlockTemplate {
                render_kind: crate::blocks::definitions::BlockRenderKind::Cube,
                face_materials: [4; 6],
                side_cap: None,
                faces: Default::default(),
                model: vec![],
                details: vec![],
                bevel_width: 0.22,
                surface: None,
            },
        );
        let faces = [BlockFace {
            minimum: [0; 3],
            width: 16,
            height: 8,
            side: 3,
            u_axis: 0,
            v_axis: 2,
            material: 7,
            edge_mask: 15,
            occupied_neighbor: false,
        }];
        let original = MeshBatch {
            positions: vec![[0.; 3]; 100],
            material: 4,
            ..default()
        };
        let mut batches = vec![original];
        append(&mut batches, &faces, &templates, DetailLevel::Near).unwrap();
        assert_eq!(batches.len(), 2);
        assert!(!batches[0].shadow_only);
        assert_eq!(batches[0].positions.len(), 100);
        assert!(batches[1].shadow_only);
        assert_eq!(batches[1].material, 4);
        assert_eq!(batches[1].positions.len(), 4);
        assert_eq!(batches[1].indices.len(), 6);
        assert_eq!(batches[1].uvs[2], [16., 8.]);
        assert!(batches[1].indices.iter().all(|&i| (i as usize) < 4));
    }

    #[test]
    fn opaque_cube_proxy_uses_positions_only_and_custom_geometry_keeps_its_own_shadow() {
        let mut templates = TemplateSet::default();
        let cube = crate::blocks::BlockTemplate {
            render_kind: crate::blocks::definitions::BlockRenderKind::Cube,
            face_materials: [4; 6],
            side_cap: None,
            faces: Default::default(),
            model: vec![],
            details: vec![],
            bevel_width: 0.22,
            surface: None,
        };
        let mut custom = cube.clone();
        custom.render_kind = crate::blocks::definitions::BlockRenderKind::CustomMesh;
        let mut second_cube = cube.clone();
        second_cube.face_materials = [5; 6];
        templates.blocks.insert(3, cube);
        templates.blocks.insert(4, second_cube);
        templates.blocks.insert(20, custom);
        let face = BlockFace {
            minimum: [0; 3],
            width: 16,
            height: 8,
            side: 3,
            u_axis: 0,
            v_axis: 2,
            material: 3,
            edge_mask: 15,
            occupied_neighbor: false,
        };
        let mut batches = Vec::new();
        append(&mut batches, &[face], &templates, DetailLevel::Near).unwrap();
        assert_eq!(batches.len(), 1);
        assert!(batches[0].shadow_only);
        assert_eq!(batches[0].positions.len(), 8);
        assert_eq!(batches[0].indices.len(), 60);
        for (front, back) in batches[0].indices[..30]
            .chunks_exact(3)
            .zip(batches[0].indices[30..].chunks_exact(3))
        {
            assert_eq!(back, &[front[0], front[2], front[1]]);
        }
        assert!(batches[0].normals.is_empty());
        assert!(batches[0].uvs.is_empty());
        batches.clear();
        append(
            &mut batches,
            &[
                face,
                BlockFace {
                    material: 4,
                    ..face
                },
            ],
            &templates,
            DetailLevel::Near,
        )
        .unwrap();
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].positions.len(), 16);
        assert_eq!(batches[0].shadow_sources, vec![4, 5]);
        assert!(batches[0].indices.iter().all(|&index| index < 16));
        batches.clear();
        append(
            &mut batches,
            &[BlockFace {
                material: 20,
                ..face
            }],
            &templates,
            DetailLevel::Near,
        )
        .unwrap();
        assert!(batches.is_empty());
        append(&mut batches, &[face], &templates, DetailLevel::Middle).unwrap();
        assert!(batches.is_empty());
    }
}
