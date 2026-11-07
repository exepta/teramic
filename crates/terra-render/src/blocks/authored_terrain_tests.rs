use super::*;
use crate::meshing::{
    DetailLevel,
    builder::{RenderOptions, compose_section_with_options},
};

pub(crate) fn templates() -> TemplateSet {
    templates_with_alpha(AlphaMode::Opaque)
}

fn templates_with_alpha(alpha_mode: AlphaMode) -> TemplateSet {
    let definition: super::super::definitions::BlockDefinition =
        serde_json::from_str(include_str!("../../../../assets/terra/blocks/grass.json")).unwrap();
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin {
            file_path: terra_core::asset_directory().to_string_lossy().into_owned(),
            ..default()
        },
        bevy::world_serialization::WorldSerializationPlugin,
        bevy::mesh::MeshPlugin,
        bevy::image::ImagePlugin::default(),
        bevy::gltf::GltfPlugin::default(),
    ))
    .init_asset::<StandardMaterial>();
    app.finish();
    app.cleanup();
    let handle: Handle<Gltf> = app
        .world()
        .resource::<AssetServer>()
        .load_builder()
        .with_settings(|s: &mut GltfLoaderSettings| s.load_meshes = RenderAssetUsages::MAIN_WORLD)
        .load(format!("terra/blocks/{}", definition.model));
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
    loop {
        app.update();
        let server = app.world().resource::<AssetServer>();
        assert!(
            !matches!(
                server.get_load_state(handle.id()),
                Some(LoadState::Failed(_))
            ),
            "{:?}",
            server.get_load_state(handle.id())
        );
        if server.is_loaded_with_dependencies(handle.id()) {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "authored terrain GLTF load timed out"
        );
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    let gltf = app.world().resource::<Assets<Gltf>>().get(&handle).unwrap();
    assert_eq!(gltf.meshes.len(), 12);
    assert_eq!(
        gltf.scenes.len(),
        2,
        "both near and coarse PBR handles must remain scene-owned"
    );
    let pbr: Vec<_> = gltf
        .materials
        .iter()
        .map(|h| {
            let material = app
                .world()
                .resource::<Assets<bevy::gltf::GltfMaterial>>()
                .get(h)
                .unwrap();
            bevy::pbr::gltf::standard_material_from_gltf_material(material)
        })
        .collect();
    assert_eq!(pbr.len(), 7);
    for material in &pbr {
        assert_eq!(material.alpha_mode, AlphaMode::Opaque);
        assert!(material.base_color_texture.is_some() && material.normal_map_texture.is_some());
        assert!(
            material.metallic_roughness_texture.is_some() && material.occlusion_texture.is_some()
        );
    }
    let dirt: Vec<super::super::definitions::BlockDefinition> = [
        include_str!("../../../../assets/terra/blocks/dirt.json"),
        include_str!("../../../../assets/terra/blocks/dirt_slab.json"),
        include_str!("../../../../assets/terra/blocks/dirt_stairs.json"),
        include_str!("../../../../assets/terra/blocks/dirt_fence.json"),
    ]
    .into_iter()
    .map(|source| serde_json::from_str(source).unwrap())
    .collect();
    let mut assets = BlockAssets {
        library: Some(BlockLibrary {
            blocks: std::iter::once(definition).chain(dirt).collect(),
        }),
        ..default()
    };
    assets.models.insert(1, handle);
    for (index, mut material) in pbr.into_iter().enumerate() {
        material.alpha_mode = alpha_mode;
        assets.material_slots.insert((1, index), index);
        assets.materials.push(
            app.world_mut()
                .resource_mut::<Assets<StandardMaterial>>()
                .add(material),
        );
    }
    let mut state = bevy::ecs::system::SystemState::<SourceAssets>::new(app.world_mut());
    let mut source = state.get_mut(app.world_mut()).unwrap();
    let templates = collect_templates(&mut source, &mut assets)
        .unwrap()
        .unwrap();
    assert_eq!(
        assets.materials.len(),
        7,
        "dirt reuses the exact authored PBR handle"
    );
    templates
}

#[test]
fn dirt_and_its_construction_shapes_share_authored_grass_soil_pbr_and_tangent_uvs() {
    let templates = templates();
    let soil = templates.cube_lods[&1].face_materials[2];
    assert_ne!(soil, templates.cube_lods[&1].face_materials[3]);
    for id in [2, 78, 86, 94] {
        let block = &templates.blocks[&id];
        assert_eq!(block.face_materials, [soil; 6]);
        for primitive in block.faces.iter().flatten().chain(&block.model) {
            assert_eq!(primitive.material, soil);
            assert_eq!(primitive.positions.len(), primitive.tangents.len());
            for ((normal, tangent), uv) in primitive
                .normals
                .iter()
                .zip(&primitive.tangents)
                .zip(&primitive.uvs)
            {
                let n = Vec3::from_array(*normal);
                let t = Vec3::new(tangent[0], tangent[1], tangent[2]);
                assert!(n.dot(t).abs() < 1e-6 && (t.length() - 1.0).abs() < 1e-6);
                assert!(uv.iter().all(|value| value.is_finite()));
            }
        }
        if id != 2 {
            let geometry = crate::meshing::custom::model_geometry(
                id,
                default(),
                default(),
                [None; 6],
                [0; 6],
                &templates,
            )
            .unwrap();
            assert!(geometry.iter().all(
                |batch| batch.material == soil && batch.positions.len() == batch.tangents.len()
            ));
        }
    }
    let dirt = &templates.blocks[&2];
    for side in 0..6 {
        let surface = dirt
            .surface
            .as_ref()
            .unwrap()
            .face(&dirt.faces[side], side, 0, 63, |p| p);
        for patch in surface {
            assert_eq!(
                patch.primitive.positions.len(),
                patch.primitive.tangents.len()
            );
        }
    }
    let face = terra_voxel::meshing::BlockFace {
        minimum: [0; 3],
        width: 2,
        height: 2,
        side: 3,
        u_axis: 0,
        v_axis: 2,
        material: 2,
        edge_mask: 15,
        occupied_neighbor: false,
    };
    for level in [DetailLevel::Near, DetailLevel::Middle, DetailLevel::Far] {
        let batches = compose_section_with_options(
            &[face],
            &templates,
            terra_voxel::SectionPos::new(-1, 3, -2),
            &RenderOptions {
                water_level: None,
                detail_level: level,
                terrain: None,
                exposure: None,
                tints: Default::default(),
                conditions: default(),
            },
        )
        .unwrap();
        assert!(batches.iter().all(|batch| batch.material == soil
            && batch.positions.len() == batch.tangents.len()
            && batch.positions.len() == batch.surface_water.len()));
    }
}

#[test]
fn authored_grass_palette_matches_the_same_biome_field_in_every_lod_and_negative_section() {
    let templates = templates();
    let terrain = terra_world::TerraGenerator::new(terra_core::WorldConfig {
        rivers: false,
        structures_revision: 0,
        caves: false,
        cave_structure_revision: 0,
        ..default()
    })
    .unwrap();
    let mut edge_colors = Vec::new();
    for (section, minimum, edge) in [
        (terra_voxel::SectionPos::new(-1, 3, -1), [12, 0, 0], 16.0),
        (terra_voxel::SectionPos::new(0, 3, -1), [0, 0, 0], 0.0),
    ] {
        let tint = crate::appearance::GroundTint::new(&terrain, section.origin().unwrap());
        let face = terra_voxel::meshing::BlockFace {
            minimum,
            width: 4,
            height: 4,
            side: 3,
            u_axis: 0,
            v_axis: 2,
            material: 1,
            edge_mask: 15,
            occupied_neighbor: false,
        };
        for level in [DetailLevel::Near, DetailLevel::Middle, DetailLevel::Far] {
            let batches = compose_section_with_options(
                &[face],
                &templates,
                section,
                &RenderOptions {
                    water_level: None,
                    detail_level: level,
                    terrain: Some(terrain.clone()),
                    exposure: None,
                    tints: Default::default(),
                    conditions: default(),
                },
            )
            .unwrap();
            let mut ends = [[0.0; 3]; 2];
            for batch in &batches {
                for (&position, color) in batch.positions.iter().zip(&batch.colors) {
                    let expected = tint.authored_grass_at(position);
                    for axis in 0..3 {
                        assert!((color[axis] - expected[axis]).abs() < 1e-6);
                    }
                    for (i, z) in [0.0, 4.0].into_iter().enumerate() {
                        if position[0] == edge && position[2] == z {
                            ends[i] = expected;
                        }
                    }
                }
                assert_eq!(batch.positions.len(), batch.surface_water.len());
            }
            assert!(ends.iter().flatten().all(|value| *value > 0.0));
            edge_colors.push(ends);
        }
    }
    for edge in &edge_colors[1..] {
        for (left, right) in edge_colors[0].iter().flatten().zip(edge.iter().flatten()) {
            assert!((left - right).abs() < 1e-5);
        }
    }
}

#[test]
fn coarse_material_references_reject_missing_sources_faces_and_competing_textures() {
    let grass: super::super::definitions::BlockDefinition =
        serde_json::from_str(include_str!("../../../../assets/terra/blocks/grass.json")).unwrap();
    let dirt: super::super::definitions::BlockDefinition =
        serde_json::from_str(include_str!("../../../../assets/terra/blocks/dirt.json")).unwrap();
    let library = BlockLibrary {
        blocks: vec![grass, dirt],
    };
    assert!(library.validate().is_ok());
    for (source, side) in [(0, 2), (2, 2), (3, 2), (1, 6)] {
        let mut invalid = library.clone();
        invalid.blocks[1]
            .materials
            .values_mut()
            .next()
            .unwrap()
            .coarse_face = Some(super::super::definitions::CoarseFaceMaterial {
            block: source,
            side,
        });
        assert!(invalid.validate().is_err());
    }
    let mut invalid = library;
    invalid.blocks[1]
        .materials
        .values_mut()
        .next()
        .unwrap()
        .texture_pattern = Some(super::super::definitions::TexturePattern::Earth);
    assert!(invalid.validate().is_err());
}

#[test]
fn shipped_grass_uses_original_pbr_faces_and_tileable_pbr_lods() {
    let templates = templates();
    let near = &templates.blocks[&1];
    assert_eq!(near.render_kind, BlockRenderKind::CustomMesh);
    assert!(near.model.is_empty() && near.surface.is_none());
    assert_eq!(near.bevel_width, 0.02);
    for (side, faces) in near.faces.iter().enumerate() {
        assert_eq!(faces.len(), 1);
        let p = &faces[0];
        if side / 2 == 1 {
            assert_eq!(p.indices.len(), 128 * 3);
        } else {
            assert!(p.indices.len() > 128 * 3);
            assert!(p.indices.len() <= (128 + 16 * 28) * 3);
            let mut face_normal = Vec3::ZERO;
            face_normal[side / 2] = if side.is_multiple_of(2) { -1.0 } else { 1.0 };
            assert!(
                p.normals
                    .iter()
                    .any(|n| { Vec3::from_array(*n).dot(face_normal).abs() < 0.8 }),
                "grass lips and pebbles have real side walls"
            );
        }
        assert!(
            p.positions
                .iter()
                .flatten()
                .all(|v| (0.0..=1.0).contains(v))
        );
        assert_eq!(p.positions.len(), p.tangents.len());
        assert!(p.tangents.iter().all(|t| t.iter().all(|v| v.is_finite())));
        assert_eq!(p.material, 0, "original shared material on face {side}");
    }
    let coarse = &templates.cube_lods[&1];
    assert!(templates.opaque_cube_lods.contains(&1));
    assert_eq!(coarse.render_kind, BlockRenderKind::Cube);
    for side in 0..6 {
        validate_tiled_face(&coarse.faces[side], side).unwrap();
        assert_eq!(coarse.face_materials[side], side + 1);
        assert_eq!(coarse.faces[side][0].tangents.len(), 4);
    }
    assert!((0..7).all(|slot| templates.authored_terrain_material(slot)));
    assert!(!templates.authored_terrain_material(7));
}

#[test]
fn transparent_authored_cubes_do_not_receive_opaque_shadow_proxies() {
    for alpha in [AlphaMode::Blend, AlphaMode::Mask(0.5)] {
        let templates = templates_with_alpha(alpha);
        assert!(templates.cube_lods.contains_key(&1));
        assert!(!templates.opaque_cube_lods.contains(&1));
        let faces = [terra_voxel::meshing::BlockFace {
            minimum: [0; 3],
            width: 1,
            height: 1,
            side: 3,
            u_axis: 0,
            v_axis: 2,
            material: 1,
            edge_mask: 15,
            occupied_neighbor: false,
        }];
        let mut batches = Vec::new();
        crate::meshing::shadows::append(&mut batches, &faces, &templates, DetailLevel::Near)
            .unwrap();
        assert!(batches.is_empty());
    }
}

#[test]
fn authored_grass_culls_shared_faces_closes_joins_and_keeps_far_greedy_rectangles() {
    let templates = templates();
    let mut padded = [0; terra_voxel::meshing::PADDED_VOLUME];
    for x in 1..=2 {
        padded[x + 18 * (1 + 18)] = 1;
    }
    let mesh = terra_voxel::meshing::mesh_section(&padded).unwrap();
    let compose = |level, section| {
        compose_section_with_options(
            &mesh.faces,
            &templates,
            section,
            &RenderOptions {
                water_level: None,
                detail_level: level,
                terrain: None,
                exposure: None,
                tints: Default::default(),
                conditions: default(),
            },
        )
        .unwrap()
    };
    let near = compose(DetailLevel::Near, terra_voxel::SectionPos::default());
    assert_eq!(
        near.len(),
        1,
        "one material batch, no mesh/entity per block"
    );
    assert_eq!(near[0].positions.len(), near[0].tangents.len());
    assert!(
        !near[0].surface_water.is_empty(),
        "authored seasonal/snow shader payload retained"
    );
    assert!(near[0].indices.len() > 36 && near[0].indices.len() <= 10 * (128 + 16 * 28) * 3);
    assert!(near[0].positions.iter().any(|p| p[0] == 1.0 && p[1] == 1.0));
    assert!(
        !near[0].indices.chunks_exact(3).any(|triangle| {
            triangle
                .iter()
                .all(|&i| near[0].positions[i as usize][0] == 1.0)
                && triangle
                    .iter()
                    .all(|&i| near[0].normals[i as usize][0].abs() > 0.99)
        }),
        "no hidden interface between the two cubes"
    );
    for level in [DetailLevel::Middle, DetailLevel::Far] {
        let batches = compose(level, terra_voxel::SectionPos::default());
        assert_eq!(batches.len(), 6);
        assert_eq!(batches.iter().map(|b| b.indices.len()).sum::<usize>(), 36);
        assert!(
            batches
                .iter()
                .all(|b| b.tangents.len() == b.positions.len())
        );
        assert_eq!(
            batches,
            compose(level, terra_voxel::SectionPos::new(-1, -2, -1))
        );
    }
}

#[test]
fn authored_near_without_micro_detail_keeps_pbr_but_reuses_greedy_coarse_faces() {
    let templates = templates();
    let mut coarse = templates.clone();
    coarse.blocks.insert(1, coarse.cube_lods[&1].clone());
    let mut padded = [0; terra_voxel::meshing::PADDED_VOLUME];
    for x in 1..=2 {
        padded[x + 18 * (1 + 18)] = 1;
    }
    let geometry = terra_voxel::meshing::mesh_section(&padded).unwrap();
    let compose = |templates: &TemplateSet, level, section| {
        compose_section_with_options(
            &geometry.faces,
            templates,
            section,
            &RenderOptions {
                water_level: None,
                detail_level: level,
                terrain: None,
                exposure: None,
                tints: Default::default(),
                conditions: default(),
            },
        )
        .unwrap()
    };
    let position = terra_voxel::SectionPos::new(-2, -1, -3);
    let detailed = compose(&templates, DetailLevel::Near, position);
    assert!(detailed[0].indices.len() >= 10 * 128 * 3);
    let near = compose(&coarse, DetailLevel::Near, position);
    assert_eq!(near, compose(&coarse, DetailLevel::Middle, position));
    assert_eq!(
        near,
        compose(
            &coarse,
            DetailLevel::Near,
            terra_voxel::SectionPos::default()
        )
    );
    assert_eq!(near.len(), 6);
    assert_eq!(
        near.iter()
            .map(|batch| batch.positions.len())
            .sum::<usize>(),
        24
    );
    assert_eq!(
        near.iter().map(|batch| batch.indices.len()).sum::<usize>(),
        36
    );
    assert!(near.iter().all(|batch| {
        (1..=6).contains(&batch.material)
            && batch.positions.len() == batch.tangents.len()
            && batch.positions.len() == batch.surface_water.len()
    }));
    let mut with_shadow = near;
    crate::meshing::shadows::append(
        &mut with_shadow,
        &geometry.faces,
        &coarse,
        DetailLevel::Near,
    )
    .unwrap();
    assert_eq!(with_shadow.len(), 7);
    assert!(
        with_shadow[..6]
            .iter()
            .all(|batch| { !batch.shadow_only && batch.shadow_sources.is_empty() })
    );
    let proxy = with_shadow.last().unwrap();
    assert!(proxy.shadow_only);
    // Soil shares its PBR batch with shaped Dirt models. Its original caster
    // must remain active rather than replacing those model shadows with cubes.
    let soil = coarse.cube_lods[&1].face_materials[2];
    assert_eq!(soil, 3);
    assert!(!proxy.shadow_sources.contains(&soil));
    assert_eq!(proxy.shadow_sources, vec![1, 2, 4, 5, 6]);
    assert_eq!(proxy.positions.len(), 20);
    assert_eq!(proxy.indices.len(), 30);
    assert!(proxy.normals.is_empty() && proxy.tangents.is_empty() && proxy.uvs.is_empty());

    // Missing opacity validation must never opt another custom asset into this path.
    coarse.opaque_cube_lods.clear();
    let unvalidated = compose(&coarse, DetailLevel::Near, position);
    assert!(
        unvalidated
            .iter()
            .map(|batch| batch.indices.len())
            .sum::<usize>()
            > 36
    );
}

#[test]
fn coarse_authored_near_preserves_weather_biome_tints_and_negative_section_seams() {
    let mut templates = templates();
    templates.blocks.insert(1, templates.cube_lods[&1].clone());
    let terrain = terra_world::TerraGenerator::new(terra_core::WorldConfig {
        rivers: false,
        structures_revision: 0,
        // Exercise explicit surface weather without unrelated underground dampness.
        caves: false,
        cave_structure_revision: 0,
        ..default()
    })
    .unwrap();
    let face = terra_voxel::meshing::BlockFace {
        minimum: [0; 3],
        width: 16,
        height: 16,
        side: 3,
        u_axis: 0,
        v_axis: 2,
        material: 1,
        edge_mask: 15,
        occupied_neighbor: false,
    };
    let mut exposure = crate::weather::surfaces::exposure::Exposure {
        natural: None,
        roofs: [0; 256],
        snow_roofs: [0; 256],
        coast: [0.0; 256],
        coast_vertices: [0.0; 289],
        water: [f32::NEG_INFINITY; 256],
    };
    let mut conditions = terra_world::environment::SurfaceSnapshot::default();
    for z in 0..16 {
        for x in 0..16 {
            let index = x + z * 16;
            if z < 4 {
                exposure.water[index] = 100.0;
            } else if z < 8 {
                exposure.roofs[index] = 100;
                exposure.snow_roofs[index] = 100;
            } else if z >= 12 {
                exposure.coast[index] = 0.5;
            }
            if z < 8 {
                conditions.insert(
                    [x, 0, z],
                    terra_world::environment::SurfaceCondition::new(0.45, 0.0).unwrap(),
                );
            }
        }
    }
    let options = RenderOptions {
        water_level: None,
        detail_level: DetailLevel::Near,
        terrain: Some(terrain),
        exposure: Some(std::sync::Arc::new(exposure)),
        tints: Default::default(),
        conditions,
    };
    let mut edges = Vec::new();
    for (section, x_edge) in [
        (terra_voxel::SectionPos::new(-1, 3, -1), 16.0),
        (terra_voxel::SectionPos::new(0, 3, -1), 0.0),
    ] {
        let near = compose_section_with_options(&[face], &templates, section, &options).unwrap();
        let middle = compose_section_with_options(
            &[face],
            &templates,
            section,
            &RenderOptions {
                detail_level: DetailLevel::Middle,
                ..options.clone()
            },
        )
        .unwrap();
        assert_eq!(
            near, middle,
            "micro-detail cutoff retains the same coarse PBR and weather path"
        );
        let batch = &near[0];
        for wanted in [-1.0, 0.0, 1.0, 1.5] {
            assert!(batch.surface_water.iter().any(|water| water[1] == wanted));
        }
        let mut edge: Vec<_> = batch
            .positions
            .iter()
            .enumerate()
            .filter(|(_, point)| point[0] == x_edge)
            .map(|(index, point)| (point[2], batch.colors[index], batch.surface_water[index]))
            .collect();
        edge.sort_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.2[1].total_cmp(&b.2[1])));
        edges.push(edge);
    }
    assert_eq!(edges[0].len(), edges[1].len());
    for (left, right) in edges[0].iter().zip(&edges[1]) {
        assert_eq!(left.0, right.0);
        assert_eq!(left.2, right.2);
        for axis in 0..3 {
            assert!((left.1[axis] - right.1[axis]).abs() < 1.0e-5);
        }
    }
}

#[test]
fn coarse_faces_reject_wrong_uvs_missing_corners_or_non_cube_planes() {
    let templates = templates();
    for side in 0..6 {
        let face = &templates.cube_lods[&1].faces[side];
        let mut invalid = face.clone();
        invalid[0].uvs[0][0] += 0.1;
        assert!(validate_tiled_face(&invalid, side).is_err());
        invalid = face.clone();
        invalid[0].positions[0][side / 2] += 0.1;
        assert!(validate_tiled_face(&invalid, side).is_err());
        invalid = face.clone();
        invalid[0].positions.pop();
        assert!(validate_tiled_face(&invalid, side).is_err());
    }
}

#[test]
fn mipmap_publication_rebinds_normal_and_shared_orm_without_touching_other_materials() {
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
    let mut images = Assets::<Image>::default();
    let image = Image::new_fill(
        Extent3d {
            width: 2,
            height: 2,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[128, 128, 255, 255],
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::default(),
    );
    let texture = images.add(image.clone());
    let mut materials = Assets::<StandardMaterial>::default();
    let normal = materials.add(StandardMaterial {
        normal_map_texture: Some(texture.clone()),
        ..default()
    });
    let orm = materials.add(StandardMaterial {
        occlusion_texture: Some(texture.clone()),
        metallic_roughness_texture: Some(texture.clone()),
        ..default()
    });
    let unrelated = materials.add(StandardMaterial::default());
    let mipmapped = super::super::mipmaps::generate(image).unwrap();
    assert_eq!(
        publish_mipmapped_image(
            &mut images,
            &mut materials,
            &[normal.clone(), orm.clone(), unrelated],
            &texture,
            mipmapped
        )
        .unwrap(),
        2
    );
    assert_eq!(
        images
            .get(&texture)
            .unwrap()
            .texture_descriptor
            .mip_level_count,
        2
    );
    assert_eq!(
        materials.get(&normal).unwrap().normal_map_texture,
        Some(texture.clone())
    );
    assert_eq!(
        materials.get(&orm).unwrap().occlusion_texture,
        Some(texture)
    );
}
