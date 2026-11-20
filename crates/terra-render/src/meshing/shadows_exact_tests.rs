use super::*;
use crate::blocks::{BlockAssets, BlockTemplate, definitions::BlockRenderKind};

fn triangle(material: usize, offset: f32) -> MeshBatch {
    MeshBatch {
        material,
        positions: vec![
            [offset, 0.0, 0.0],
            [offset + 1.0, 0.0, 0.0],
            [offset, 1.0, 0.0],
        ],
        normals: vec![[0.0, 0.0, 1.0]; 3],
        tangents: vec![[1.0, 0.0, 0.0, 1.0]; 3],
        uvs: vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]],
        colors: vec![[1.2, 0.9, 0.7, 1.0]; 3],
        indices: vec![0, 1, 2],
        ..default()
    }
}

fn triangles<'a>(batches: impl IntoIterator<Item = &'a MeshBatch>) -> Vec<[[u32; 3]; 3]> {
    let mut result = Vec::new();
    for batch in batches {
        let indices: Vec<_> = if batch.compact_indices.is_empty() {
            batch.indices.clone()
        } else {
            batch
                .compact_indices
                .iter()
                .map(|&i| u32::from(i))
                .collect()
        };
        result.extend(indices.chunks_exact(3).map(|face| {
            std::array::from_fn(|i| batch.positions[face[i] as usize].map(f32::to_bits))
        }));
    }
    result.sort_unstable();
    result
}

#[test]
fn exact_fusion_defaults_on_and_wet_surfaces_use_matching_depth_vertices() {
    use crate::weather::surfaces::{TerrainDepthExtension, WetExtension};
    use bevy::{pbr::MaterialExtension, shader::ShaderRef};
    for value in [None, Some("0"), Some("1"), Some("true")] {
        for full_reference in [false, true] {
            assert!(exact_control(false, value, full_reference));
            assert_eq!(
                exact_control(true, value, full_reference),
                value != Some("0") && !full_reference,
            );
        }
    }
    assert!(matches!(
        TerrainDepthExtension::vertex_shader(),
        ShaderRef::Default
    ));
    assert!(matches!(
        TerrainDepthExtension::prepass_vertex_shader(),
        ShaderRef::Default
    ));
    for shader in [
        WetExtension::vertex_shader(),
        WetExtension::prepass_vertex_shader(),
        WetExtension::deferred_vertex_shader(),
    ] {
        assert!(
            matches!(shader, ShaderRef::Path(path) if path.path() == std::path::Path::new("shaders/terra_liquid_vertex.wesl"))
        );
    }
}

#[test]
fn material_proof_requires_opaque_zero_bias_two_sided_depth_and_no_special_shader() {
    let mut standard = Assets::<StandardMaterial>::default();
    let mut blocks = BlockAssets::default();
    let mut templates = TemplateSet::default();
    let mut foliage = crate::foliage::FoliageAssets::default();
    let mut wet = crate::weather::surfaces::WetMaterials::default();
    let mut depth = crate::weather::surfaces::TerrainDepthMaterials::default();
    for slot in 0..21 {
        let mut material = StandardMaterial::default();
        match slot {
            2 | 16 => material.alpha_mode = AlphaMode::Blend,
            3 => material.alpha_mode = AlphaMode::Mask(0.5),
            4 => material.unlit = true,
            5 => material.depth_bias = 1.0,
            _ => {}
        }
        blocks.materials.push(standard.add(material));
        if slot != 6 {
            wet.0.insert(slot, Handle::default());
        }
        if slot != 7 {
            depth.0.insert(slot, Handle::default());
        }
    }
    templates.ore_materials.insert(30, 8);
    foliage.materials.insert(9, Handle::default());
    foliage.fibres.insert(10, Handle::default());
    foliage.flames.insert(11, Handle::default());
    foliage.reference.insert(12, Handle::default());
    templates.torch_palette.insert(13, [1.0; 4]);
    for id in [14, 15] {
        templates.blocks.insert(
            id,
            BlockTemplate {
                surface: None,
                render_kind: BlockRenderKind::Cube,
                bevel_width: 0.0,
                faces: std::array::from_fn(|_| Vec::new()),
                face_materials: [usize::from(id); 6],
                side_cap: None,
                details: Vec::new(),
                model: Vec::new(),
            },
        );
    }
    let mut fog = templates.blocks[&14].clone();
    fog.face_materials = [17; 6];
    templates
        .blocks
        .insert(terra_core::fog_blocks::MATERIAL, fog);
    let mut mystic = templates.blocks[&14].clone();
    mystic.face_materials = [18; 6];
    templates.blocks.insert(399, mystic);
    for (material, slot) in [
        (terra_core::roads::WET_TAR, 19),
        (terra_core::roads::DRIED_TAR, 20),
    ] {
        let mut tar = templates.blocks[&14].clone();
        tar.face_materials[3] = slot;
        templates.blocks.insert(material, tar);
    }
    blocks.templates = Some(Arc::new(templates));
    assert_eq!(
        exact_materials(&blocks, &standard, &foliage, &wet, &depth),
        BTreeSet::from([0, 1, 20])
    );
}

#[test]
fn middle_and_far_copy_complete_geometry_with_negative_extents_and_mixed_indices() {
    for level in [DetailLevel::Middle, DetailLevel::Far] {
        let mut original = vec![triangle(0, -0.375), triangle(1, 15.75), triangle(2, 8.0)];
        original[1].compact_indices();
        original[0].surface_water = vec![[0.6, 0.1]; 3];
        let mut batches = vec![triangle(0, -0.375), triangle(1, 15.75), triangle(2, 8.0)];
        batches[1].compact_indices();
        batches[0].surface_water = vec![[0.6, 0.1]; 3];
        append_exact(&mut batches, &BTreeSet::from([0, 1]), level).unwrap();
        assert_eq!(&batches[..3], &original);
        let proxies = &batches[3..];
        assert_eq!(proxies.len(), 1);
        let proxy = &proxies[0];
        assert_eq!(proxy.shadow_sources, [0, 1]);
        assert!(proxy.shadow_only);
        assert!(proxy.normals.is_empty() && proxy.tangents.is_empty() && proxy.uvs.is_empty());
        assert!(proxy.colors.is_empty() && proxy.surface_water.is_empty());
        assert_eq!(triangles(proxies), triangles(&original[..2]));
    }
}

#[test]
fn whole_material_groups_are_all_or_nothing_and_near_and_single_sources_do_not_change() {
    let changes: &[fn(&mut MeshBatch)] = &[
        |b| b.rooted_wind = vec![[0.0, 0.5]; 3],
        |b| b.growth_anchors = b.positions.clone(),
        |b| b.growth_ranges.push((terra_world::props::PropId(4), 0..3)),
        |b| b.positions[0][0] = f32::INFINITY,
        |b| b.indices[0] = 3,
        |b| b.compact_indices = vec![0, 1, 2],
    ];
    for change in changes {
        let mut invalid = triangle(0, 3.0);
        change(&mut invalid);
        let mut batches = vec![
            triangle(0, 0.0),
            invalid,
            triangle(1, 6.0),
            triangle(2, 9.0),
        ];
        append_exact(
            &mut batches,
            &BTreeSet::from([0, 1, 2]),
            DetailLevel::Middle,
        )
        .unwrap();
        assert_eq!(batches.len(), 5);
        assert_eq!(batches[4].shadow_sources, [1, 2]);
        assert_eq!(triangles(&batches[4..]), triangles(&batches[2..4]));
    }
    let mut near = vec![triangle(0, 0.0), triangle(1, 1.0)];
    append_exact(&mut near, &BTreeSet::from([0, 1]), DetailLevel::Near).unwrap();
    assert_eq!(near.len(), 2);
    let mut one = vec![triangle(0, 0.0)];
    append_exact(&mut one, &BTreeSet::from([0]), DetailLevel::Far).unwrap();
    assert_eq!(one.len(), 1);
}

fn large(material: usize, vertices: usize) -> MeshBatch {
    MeshBatch {
        material,
        positions: (0..vertices)
            .map(|i| [(i % 16) as f32, ((i / 16) % 16) as f32, (i / 256) as f32])
            .collect(),
        indices: (0..vertices as u32).collect(),
        ..default()
    }
}

#[test]
fn fusion_keeps_handoff_bounds_and_only_publishes_a_strict_caster_reduction() {
    let mut no_gain = vec![large(0, 42_000), large(1, 42_000)];
    append_exact(&mut no_gain, &BTreeSet::from([0, 1]), DetailLevel::Far).unwrap();
    assert_eq!(
        no_gain.len(),
        2,
        "two bounded proxies cannot improve two sources"
    );
    let mut batches = vec![large(0, 30_000), large(1, 30_000), large(2, 30_000)];
    let expected = triangles(&batches);
    append_exact(&mut batches, &BTreeSet::from([0, 1, 2]), DetailLevel::Far).unwrap();
    assert_eq!(batches.len(), 5);
    assert_eq!(triangles(&batches[3..]), expected);
    for proxy in &batches[3..] {
        assert!(proxy.positions.len() <= crate::upload_budget::MAX_BATCH_VERTICES);
        assert!(crate::upload_budget::batch_bytes(proxy) <= crate::upload_budget::MAX_BATCH_BYTES);
        assert_eq!(proxy.shadow_sources, [0, 1, 2]);
    }
}

#[test]
fn exact_casters_publish_atomically_restore_when_disabled_and_release_on_unload() {
    use crate::meshing::{
        RenderStatus, RenderedSections,
        jobs::{RenderJob, RenderJobs},
    };
    use terra_world::{
        edits::EditJournal,
        streaming::{ChunkStore, LoadedSection},
    };
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .init_resource::<Assets<Mesh>>()
        .init_resource::<Assets<StandardMaterial>>()
        .init_resource::<BlockAssets>()
        .init_resource::<RenderedSections>()
        .init_resource::<RenderStatus>()
        .init_resource::<EditJournal>()
        .init_resource::<ChunkStore>()
        .init_resource::<RenderJobs>()
        .init_resource::<crate::MeshUploadStatus>()
        .insert_resource(terra_core::GraphicsConfig::default())
        .add_systems(
            Update,
            (crate::meshing::upload_sections, release_disabled).chain(),
        );
    let handles: Vec<_> = (0..3)
        .map(|_| {
            app.world_mut()
                .resource_mut::<Assets<StandardMaterial>>()
                .add(StandardMaterial::default())
        })
        .collect();
    app.insert_resource(OpaqueShadowMaterial(handles[0].clone()));
    app.world_mut().resource_mut::<BlockAssets>().materials = handles;
    let position = terra_voxel::SectionPos::new(-1, -1, -1);
    app.world_mut().resource_mut::<ChunkStore>().loaded.insert(
        position,
        LoadedSection {
            data: terra_voxel::BlockSection::empty(position).unwrap(),
            mesh: None,
            mesh_epoch: 0,
            water_epoch: 0,
            water: None,
            props: Vec::new(),
        },
    );
    for quality in [
        terra_core::ShadowQuality::High,
        terra_core::ShadowQuality::Off,
        terra_core::ShadowQuality::High,
    ] {
        app.world_mut()
            .resource_mut::<terra_core::GraphicsConfig>()
            .shadow_quality = quality;
        let mut batches = vec![triangle(0, 0.0), triangle(1, 1.0), triangle(2, 2.0)];
        batches[2].rooted_wind = vec![[0.0, 0.5]; 3];
        append_exact(
            &mut batches,
            &BTreeSet::from([0, 1, 2]),
            DetailLevel::Middle,
        )
        .unwrap();
        app.world_mut().resource_mut::<RenderJobs>().0.insert(
            position,
            RenderJob {
                ready: Some((Ok(batches), 0.0)),
                micro_detail: true,
                epoch: 0,
                appearance_epoch: 0,
                level: DetailLevel::Middle,
                task: bevy::tasks::AsyncComputeTaskPool::get().spawn(std::future::pending()),
            },
        );
        *app.world_mut().resource_mut::<crate::MeshUploadStatus>() = Default::default();
        if quality != terra_core::ShadowQuality::Off {
            assert!(
                app.world_mut()
                    .resource_mut::<crate::MeshUploadStatus>()
                    .reserve_batch_bytes(
                        crate::upload_budget::UploadKind::Terrain,
                        crate::upload_budget::TERRAIN_VERTICES - 3,
                        0,
                    )
            );
            let old_root = app
                .world()
                .resource::<RenderedSections>()
                .sections
                .get(&position)
                .map(|old| old.entity);
            app.update();
            assert!(!app.world().resource::<RenderJobs>().0.is_empty());
            assert_eq!(
                app.world()
                    .resource::<RenderedSections>()
                    .sections
                    .get(&position)
                    .map(|old| old.entity),
                old_root
            );
            *app.world_mut().resource_mut::<crate::MeshUploadStatus>() = Default::default();
        }
        app.update();
        assert!(app.world().resource::<RenderJobs>().0.is_empty());
        let expected = if quality == terra_core::ShadowQuality::Off {
            3
        } else {
            4
        };
        assert_eq!(app.world().resource::<Assets<Mesh>>().len(), expected);
        let root = app.world().resource::<RenderedSections>().sections[&position].entity;
        assert_eq!(app.world().get::<Children>(root).unwrap().len(), expected);
        for child in app.world().get::<Children>(root).unwrap().iter() {
            let proxy = app.world().get::<ShadowProxy>(child).is_some();
            let rooted = app
                .world()
                .get::<Mesh3d>(child)
                .and_then(|handle| app.world().resource::<Assets<Mesh>>().get(&handle.0))
                .is_some_and(|mesh| mesh.attribute(Mesh::ATTRIBUTE_UV_1).is_some());
            assert_eq!(
                app.world()
                    .get::<bevy::light::NotShadowCaster>(child)
                    .is_some(),
                !proxy && !rooted && quality != terra_core::ShadowQuality::Off
            );
        }
    }
    app.world_mut().resource_mut::<ChunkStore>().loaded.clear();
    app.update();
    assert!(
        app.world()
            .resource::<RenderedSections>()
            .sections
            .is_empty()
    );
    assert!(app.world().resource::<Assets<Mesh>>().is_empty());
}
