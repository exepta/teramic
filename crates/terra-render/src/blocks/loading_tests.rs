use super::*;

#[path = "hydraulics_tests.rs"]
mod hydraulics_tests;
#[path = "kinetic_art_tests.rs"]
mod kinetic_art_tests;
#[path = "lantern_tests.rs"]
mod lantern_tests;
#[path = "profession_station_tests.rs"]
mod profession_station_tests;
#[path = "road_tests.rs"]
mod road_tests;

#[test]
fn sculpted_palette_loads_tiled_pbr_and_opens_each_canopy_in_mesh_depth_and_color() {
    use bevy::{image::ImageFilterMode, render::render_resource::TextureFormat};

    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin {
            file_path: terra_core::asset_directory().to_string_lossy().into_owned(),
            ..default()
        },
    ))
    .init_asset::<Gltf>()
    .init_asset::<GltfMesh>()
    .init_asset::<GltfNode>()
    .init_asset::<Mesh>()
    .init_asset::<Image>()
    .init_asset::<StandardMaterial>()
    .register_asset_loader(bevy::image::ImageLoader::new(
        bevy::image::CompressedImageFormats::NONE,
    ));
    app.finish();
    app.cleanup();
    let mut assets = BlockAssets::default();
    let server = app.world().resource::<AssetServer>().clone();
    let mut materials = app.world_mut().resource_mut::<Assets<StandardMaterial>>();
    let mut blocks = BlockLibrary::load(&terra_core::asset_directory().join("terra/blocks"))
        .unwrap()
        .blocks;
    blocks.retain(|b| {
        terra_voxel::MaterialId::FOLIAGE.contains(&b.id) || [77, 85, 93].contains(&b.id)
    });
    for block in &blocks {
        assets.load_block_textures(&server, block);
        for (name, material) in &block.materials {
            let slot =
                assets.shared_material(material, Some(&block.textures[name]), &mut materials);
            assets
                .cube_material_slots
                .insert((block.id, name.clone()), slot);
        }
    }
    assets.load_reference_materials(&server, &mut materials);
    assets.library = Some(BlockLibrary {
        blocks: blocks.into(),
    });
    let mut state = bevy::ecs::system::SystemState::<SourceAssets>::new(app.world_mut());
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    loop {
        app.update();
        if prepare_mipmaps(&mut state.get_mut(app.world_mut()).unwrap(), &mut assets).unwrap() {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "texture mip preparation timed out"
        );
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    let mut source = state.get_mut(app.world_mut()).unwrap();
    let templates = collect_templates(&mut source, &mut assets)
        .unwrap()
        .unwrap();
    let palette = &assets.reference_palette;
    for slot in [
        palette.grass_top,
        palette.grass_side,
        palette.soil,
        palette.rock,
        palette.gravel,
        palette.leaves,
    ] {
        let slot = slot.unwrap();
        assert!(templates.normal_mapped_materials.contains(&slot));
        let material = source.materials.get(&assets.materials[slot]).unwrap();
        assert_eq!(
            material.alpha_mode,
            if Some(slot) == palette.leaves {
                AlphaMode::Mask(0.5)
            } else {
                AlphaMode::Opaque
            }
        );
        for (handle, format) in [
            (&material.base_color_texture, TextureFormat::Rgba8UnormSrgb),
            (&material.normal_map_texture, TextureFormat::Rgba8Unorm),
        ] {
            let image = source.images.get(handle.as_ref().unwrap()).unwrap();
            assert_eq!(image.texture_descriptor.format, format);
            assert_eq!(
                image.texture_descriptor.mip_level_count,
                image.width().max(image.height()).ilog2() + 1
            );
            let ImageSampler::Descriptor(sampler) = &image.sampler else {
                panic!("missing PBR sampler")
            };
            assert_eq!(sampler.address_mode_u, ImageAddressMode::Repeat);
            assert_eq!(sampler.address_mode_v, ImageAddressMode::Repeat);
            assert_eq!(sampler.mipmap_filter, ImageFilterMode::Linear);
        }
    }
    let rock = assets.reference_palette.rock.unwrap();
    for id in [77, 85, 93] {
        assert_eq!(templates.blocks[&id].face_materials, [rock; 6]);
        assert!(
            templates.blocks[&id]
                .model
                .iter()
                .all(|p| p.material == rock && p.positions.len() == p.tangents.len())
        );
    }
    // Only oak paints its side and top with the identical material after
    // the wind shader's overrides. Other species retain their distinct art.
    assert_eq!(
        templates
            .batched_wind_leaves
            .keys()
            .copied()
            .collect::<Vec<_>>(),
        vec![7]
    );
    assert_oak_wind_batch(&templates);
    for id in terra_voxel::MaterialId::FOLIAGE {
        for side in [0, 3] {
            let original = templates.blocks[&id].face_materials[side];
            let core = templates.sculpted_blocks[&id].core.face_materials[side];
            assert!(templates.foliage_material(original));
            assert!(templates.foliage_material(core));
            let paint = source.materials.get(&assets.materials[original]).unwrap();
            let pigment = paint.base_color_texture.as_ref().unwrap().id();
            assert_eq!(
                assets.opaque_oak_paint == Some(pigment),
                id == 7 && !server.watching_for_changes()
            );
            let core = source.materials.get(&assets.materials[core]).unwrap();
            assert_ne!(
                assets.opaque_oak_paint,
                core.base_color_texture.as_ref().map(Handle::id)
            );
            let image = source
                .images
                .get(core.base_color_texture.as_ref().unwrap())
                .unwrap();
            assert_eq!(core.alpha_mode, AlphaMode::Mask(0.5));
            assert!(core.double_sided && core.cull_mode.is_none());
            let size = image.width() as usize;
            let samples = &image.data.as_ref().unwrap()[..size * size * 4];
            let uncovered = samples.chunks_exact(4).filter(|p| p[3] < 128).count();
            assert!((0.06..0.38).contains(&(uncovered as f32 / (size * size) as f32)));
            let [u, v, span] = templates.sculpted_blocks[&id].leaf_openings[side].unwrap();
            let u = (u * size as f32).ceil() as usize;
            let v = (v * size as f32).ceil() as usize;
            let span = (span * size as f32).floor() as usize;
            assert!(span >= 2, "leaf {id} side {side}, span {span}");
            for y in v..v + span {
                for x in u..u + span {
                    assert!(samples[4 * (x + y * size) + 3] < 128);
                }
            }
        }
        let mut padded = vec![0; terra_voxel::meshing::PADDED_VOLUME];
        padded[1 + 18 + 18 * 18] = id;
        let faces = terra_voxel::meshing::mesh_section_faces(&padded)
            .unwrap()
            .faces;
        use crate::meshing::{
            DetailLevel,
            builder::{RenderOptions, compose_section_with_options},
        };
        for detail_level in [DetailLevel::Near, DetailLevel::Middle, DetailLevel::Far] {
            let batches = compose_section_with_options(
                &faces,
                &templates,
                terra_voxel::SectionPos::new(0, 0, 0),
                &RenderOptions {
                    detail_level,
                    water_level: None,
                    terrain: None,
                    exposure: None,
                    conditions: Default::default(),
                    tints: Default::default(),
                },
            )
            .unwrap();
            let count = canopy_sight_lines(
                &batches,
                &source.images,
                &source.materials,
                &assets.materials,
            );
            assert!(
                count > 2 && count < 1400,
                "foliage {id}, {detail_level:?}: {count} open rays"
            );
        }
    }
}

fn assert_oak_wind_batch(templates: &TemplateSet) {
    use crate::meshing::{
        DetailLevel,
        builder::{RenderOptions, compose_section_with_options},
    };
    let mut padded = vec![0; terra_voxel::meshing::PADDED_VOLUME];
    padded[1 + 18 + 18 * 18] = 7;
    let faces = terra_voxel::meshing::mesh_section_faces(&padded)
        .unwrap()
        .faces;
    let section = terra_voxel::SectionPos::new(0, 64, 0);
    let options = RenderOptions {
        detail_level: DetailLevel::Near,
        terrain: Some(terra_world::TerraGenerator::new(Default::default()).unwrap()),
        water_level: None,
        exposure: None,
        conditions: Default::default(),
        tints: Default::default(),
    };
    let mut separate = templates.clone();
    separate.batched_wind_leaves.clear();
    let before = compose_section_with_options(&faces, &separate, section, &options).unwrap();
    let after = compose_section_with_options(&faces, templates, section, &options).unwrap();
    let top = templates.sculpted_blocks[&7].slots[3];
    let side = templates.sculpted_blocks[&7].slots[0];
    assert_ne!(top, side);
    assert_eq!(before.len(), after.len() + 1);
    let triangles = |batches: &[crate::meshing::builder::MeshBatch]| {
        let mut triangles = Vec::new();
        for b in batches {
            let slot = if b.material == top { side } else { b.material };
            for t in b.indices.chunks_exact(3) {
                triangles.push((
                    slot,
                    [t[0], t[1], t[2]].map(|i| {
                        let i = i as usize;
                        (
                            b.positions[i].map(f32::to_bits),
                            b.normals[i].map(f32::to_bits),
                            b.uvs[i].map(f32::to_bits),
                            b.surface_water.get(i).map(|uv| uv.map(f32::to_bits)),
                            b.foliage_owners
                                .iter()
                                .find(|o| o.vertices.contains(&i))
                                .map(|o| (o.cell, o.material)),
                            b.tangents.get(i).map(|t| t.map(f32::to_bits)),
                            b.colors
                                .get(i)
                                .copied()
                                .unwrap_or([1.0; 4])
                                .map(f32::to_bits),
                        )
                    }),
                ));
            }
        }
        triangles.sort_unstable();
        triangles
    };
    assert!(
        triangles(&before) == triangles(&after),
        "wind batch must retain every PBR triangle exactly"
    );
    let preview = compose_section_with_options(
        &faces,
        templates,
        section,
        &RenderOptions {
            terrain: None,
            ..options
        },
    )
    .unwrap();
    assert!(preview.iter().any(|b| b.material == top));
    assert!(preview.iter().any(|b| b.material == side));
}

/// Composite both sides of one real block onto a grid of parallel sight lines.
/// Sampling each triangle's own material also catches an accidental opaque
/// backing or incorrect wrapping on the opposite side.
fn canopy_sight_lines(
    batches: &[crate::meshing::builder::MeshBatch],
    images: &Assets<Image>,
    materials: &Assets<StandardMaterial>,
    handles: &[Handle<StandardMaterial>],
) -> usize {
    const GRID: usize = 48;
    let mut covered = [[false; GRID]; GRID];
    for batch in batches {
        let material = materials.get(&handles[batch.material]).unwrap();
        let art = images
            .get(material.base_color_texture.as_ref().unwrap())
            .unwrap();
        let size = Vec2::new(art.width() as f32, art.height() as f32);
        let data = art.data.as_ref().unwrap();
        for tri in batch.indices.chunks_exact(3) {
            let [a, b, c] = [tri[0], tri[1], tri[2]].map(|i| {
                let [_, y, z] = batch.positions[i as usize];
                (Vec2::new(z, y), Vec2::from_array(batch.uvs[i as usize]))
            });
            let (ab, ac) = (b.0 - a.0, c.0 - a.0);
            let area = ab.perp_dot(ac);
            if area.abs() < 1e-8 {
                continue;
            }
            let lo = a.0.min(b.0).min(c.0) * GRID as f32;
            let hi = a.0.max(b.0).max(c.0) * GRID as f32;
            for y in lo.y.max(0.0) as usize..(hi.y.ceil() as usize).min(GRID) {
                for x in lo.x.max(0.0) as usize..(hi.x.ceil() as usize).min(GRID) {
                    let p = (Vec2::new(x as f32 + 0.5, y as f32 + 0.5) / GRID as f32) - a.0;
                    let (u, v) = (p.perp_dot(ac) / area, ab.perp_dot(p) / area);
                    if u.min(v) < 0.0 || u + v > 1.0 {
                        continue;
                    }
                    let uv = (a.1 + (b.1 - a.1) * u + (c.1 - a.1) * v).rem_euclid(Vec2::ONE) * size;
                    let texel = (uv.y as usize * art.width() as usize + uv.x as usize) * 4;
                    covered[y][x] |=
                        material.alpha_mode == AlphaMode::Opaque || data[texel + 3] >= 128;
                }
            }
        }
    }
    covered
        .into_iter()
        .flatten()
        .filter(|covered| !covered)
        .count()
}

#[test]
fn authored_tangents_follow_nonuniform_node_scale_rotation_and_reflection() {
    let direction = Vec3::new(1.0, 1.0, 0.0).normalize();
    let mut primitive = PrimitiveTemplate {
        positions: vec![[0.0, 0.0, 0.0], [0.25, 0.0, 0.0], [0.0, 0.25, 0.0]],
        normals: vec![[0.0, 0.0, 1.0]; 3],
        tangents: vec![[direction.x, direction.y, direction.z, 1.0]; 3],
        uvs: vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]],
        indices: vec![0, 1, 2],
        material: 0,
    };
    let transform = bevy::math::Affine3A::from_scale_rotation_translation(
        Vec3::new(-0.5, 0.25, 1.0),
        Quat::from_rotation_z(0.3),
        Vec3::splat(0.5),
    );
    bake_transform(1, &mut primitive, transform).unwrap();
    let expected = transform.transform_vector3(direction).normalize();
    for (&tangent, &normal) in primitive.tangents.iter().zip(&primitive.normals) {
        let direction = Vec3::from_array([tangent[0], tangent[1], tangent[2]]);
        assert!((direction - expected).length() < 0.0001);
        assert!(direction.dot(Vec3::from_array(normal)).abs() < 0.0001);
        assert_eq!(tangent[3], -1.0);
    }
    assert_eq!(primitive.indices, [0, 2, 1]);
}

#[test]
fn held_torch_material_has_no_emission_or_self_light_response() {
    let material = torch_item_material();
    assert!(material.unlit);
    assert_eq!(material.emissive, LinearRgba::BLACK);
    let model: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(
            terra_core::asset_directory().join("terra/blocks/models/torch.gltf"),
        )
        .unwrap(),
    )
    .unwrap();
    for material in model["materials"].as_array().unwrap() {
        let name = material["name"].as_str().unwrap();
        if !matches!(name, "flame" | "heart") {
            assert!(
                material.get("emissiveFactor").is_none(),
                "{name} must not glow"
            );
        }
    }
}

#[test]
fn painted_machine_albedo_does_not_receive_a_second_dark_vertex_or_material_tint() {
    use bevy::image::{CompressedImageFormats, ImageSampler, ImageType};
    let directory = terra_core::asset_directory().join("terra/blocks");
    let image = Image::from_buffer(
        &std::fs::read(directory.join("textures/forged-iron-warm.png")).unwrap(),
        ImageType::Extension("png"),
        CompressedImageFormats::empty(),
        true,
        ImageSampler::default(),
        bevy::asset::RenderAssetUsages::MAIN_WORLD,
    )
    .unwrap();
    let pixels = image.data.as_ref().unwrap();
    let luminance = pixels
        .chunks_exact(4)
        .map(|p| {
            (f64::from(p[0]) * 0.2126 + f64::from(p[1]) * 0.7152 + f64::from(p[2]) * 0.0722) / 255.0
        })
        .sum::<f64>()
        / (pixels.len() / 4) as f64;
    assert!(
        (0.35..0.75).contains(&luminance),
        "the albedo must stay readable under ordinary shadow lighting: {luminance}"
    );
    for name in ["crusher", "hand_crank"] {
        let document: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(directory.join(format!("models/{name}.gltf"))).unwrap(),
        )
        .unwrap();
        for material in document["materials"].as_array().unwrap() {
            if matches!(
                material["name"].as_str().unwrap(),
                "oak"
                    | "iron"
                    | "iron_edge"
                    | "crusher_roller_left"
                    | "crusher_roller_left_edge"
                    | "crusher_roller_right"
                    | "crusher_roller_right_edge"
                    | "crank_rotating"
                    | "crank_rotating_edge"
                    | "crank_rotating_grip"
            ) {
                let color = material["pbrMetallicRoughness"]["baseColorFactor"]
                    .as_array()
                    .unwrap();
                assert_eq!(color[0], color[1]);
                assert_eq!(color[1], color[2]);
                assert!(color[0].as_f64().unwrap() >= 0.78);
            }
        }
        for mesh in document["meshes"].as_array().unwrap() {
            for primitive in mesh["primitives"].as_array().unwrap() {
                assert!(primitive["attributes"].get("COLOR_0").is_none());
            }
        }
    }
}

#[test]
fn torch_emits_but_ores_only_reflect_existing_light() {
    let library = BlockLibrary::load(&terra_core::asset_directory().join("terra/blocks")).unwrap();
    let mut torch = library.blocks.iter().find(|b| b.id == 40).unwrap().clone();
    assert_eq!(torch.render_kind, BlockRenderKind::CustomMesh);
    assert_eq!(torch.light.unwrap().range, 28.0);
    assert!((1000.0..2000.0).contains(&torch.light.unwrap().intensity));
    assert_eq!(torch.light.unwrap().cave_intensity, 0.6);
    assert_eq!(
        torch.light.unwrap().falloff,
        super::super::definitions::LightFalloff::SoftCore
    );
    assert!(torch.gameplay.struct_block);
    assert!(
        torch
            .collision_boxes
            .iter()
            .all(terra_core::block_shapes::ShapeBox::valid)
    );
    assert!(torch.validate().is_ok());
    torch.light.as_mut().unwrap().flicker = 0.9;
    assert!(torch.validate().is_err());
    let mut assets = BlockAssets {
        library: Some(library),
        ..default()
    };
    assert_eq!(
        assets
            .lights()
            .map(|(id, _)| id)
            .collect::<std::collections::BTreeSet<_>>(),
        [
            28,
            40,
            56,
            99,
            terra_core::lantern::LANTERN,
            315,
            316,
            318,
            321,
            323,
            358,
            369,
            370,
            371,
            372,
            373,
            386,
            395,
            396,
            397,
            409,
            411,
            414,
            421,
            terra_voxel::MaterialId::AmberGeode as u16,
            terra_voxel::MaterialId::TideGeode as u16,
            terra_voxel::MaterialId::SaltCrystalCluster as u16,
            terra_voxel::MaterialId::CaveCrystalCluster as u16,
            terra_voxel::MaterialId::GlowVine as u16,
            terra_voxel::MaterialId::GlacialCrystalCluster as u16,
            terra_voxel::MaterialId::GoldenSporePods as u16,
            terra_voxel::MaterialId::BloodBrazier as u16,
            terra_voxel::MaterialId::BloodCrystalGrowth as u16,
            terra_voxel::MaterialId::StandingBloodLantern as u16,
            terra_voxel::MaterialId::HangingBloodLantern as u16,
            terra_voxel::MaterialId::LuminousCoral as u16,
            terra_voxel::MaterialId::SeaCrystal as u16,
            terra_voxel::MaterialId::SakuraGardenLantern as u16,
            terra_voxel::MaterialId::RunePillar as u16,
            terra_voxel::MaterialId::EmberCrystals as u16,
            terra_voxel::MaterialId::Fumarole as u16,
            terra_voxel::MaterialId::PyricRuneStone as u16,
            terra_voxel::MaterialId::AshenBrazier as u16,
            terra_voxel::MaterialId::CrystalLantern as u16,
            terra_voxel::MaterialId::VaultSigil as u16,
            terra_voxel::MaterialId::CrystalObservatoryLens as u16,
            terra_voxel::MaterialId::BeaconLens as u16,
            terra_voxel::MaterialId::AmethystPrismTip as u16,
            terra_voxel::MaterialId::VioletTorch as u16,
            terra_voxel::MaterialId::AltarCrystalHeart as u16,
            terra_voxel::MaterialId::TempleCrystalCluster as u16,
        ]
        .into_iter()
        .collect()
    );
    // The three altar pieces and the crafted handheld source really light room geometry.
    for id in [769, 791, 794, 796] {
        let (_, quartz) = assets.lights().find(|(m, _)| *m == id).unwrap();
        assert!(quartz.color[2] > quartz.color[0] && quartz.color[0] > 2.0 * quartz.color[1]);
        assert!(quartz.range >= 12.0 && quartz.intensity >= 1500.0);
    }
    let (_, beacon) = assets
        .lights()
        .find(|(id, _)| *id == terra_voxel::MaterialId::BeaconLens as u16)
        .unwrap();
    assert_eq!(beacon.color, [1., 0.76, 0.39]);
    assert_eq!(
        (beacon.range, beacon.intensity, beacon.shadows),
        (23., 2400., false)
    );
    let (_, seal) = assets
        .lights()
        .find(|(id, _)| *id == terra_voxel::MaterialId::VaultSigil as u16)
        .unwrap();
    assert_eq!(seal.color, [0.70, 0.30, 1.0]);
    assert_eq!((seal.range, seal.intensity), (10.0, 1200.0));
    assert_eq!(
        seal.falloff,
        super::super::definitions::LightFalloff::SoftCore
    );
    assert_eq!(seal.flicker, 0.0);
    assert!(!seal.shadows);
    for (id, light) in assets.lights().filter(|(id, _)| (306..=323).contains(id)) {
        assert!(
            !light.shadows,
            "Spirit light {id} stays in the bounded lamp budget"
        );
        assert_eq!(
            light.falloff,
            super::super::definitions::LightFalloff::SoftCore
        );
        let (range, intensity) = if id == 316 {
            (5.0, 70.0)
        } else {
            (13.0, 900.0)
        };
        assert_eq!(light.range, range);
        assert_eq!(light.intensity, intensity);
    }
    for (id, light) in assets.lights().filter(|(id, _)| (324..=399).contains(id)) {
        assert!(
            !light.shadows,
            "cemetery light {id} shares the unshadowed lamp pool"
        );
        assert_eq!(
            light.falloff,
            super::super::definitions::LightFalloff::SoftCore
        );
        let expected = match id {
            358 => (9.0, 450.0),
            369 => (28.0, 2400.0),
            370 | 373 => (11.0, 700.0),
            371 => (10.0, 500.0),
            372 => (13.0, 900.0),
            386 => (6.0, 100.0),
            395 => (5.0, 100.0),
            396 => (8.0, 240.0),
            397 => (8.0, 350.0),
            _ => panic!("Unspecified cemetery lighting profile {id}"),
        };
        assert_eq!((light.range, light.intensity), expected, "light {id}");
        assert_eq!(light.cave_intensity, 0.65);
        assert!(light.flicker <= 0.03);
        assert!(
            light
                .offset
                .is_some_and(|point| point.into_iter().all(|v| (0.0..=1.0).contains(&v))),
            "cemetery light {id} must stay on its authored flame, not an adjacent air cell"
        );
    }
    for (id, light) in assets.lights().filter(|(id, _)| (400..=421).contains(id)) {
        assert!(
            !light.shadows,
            "Blood Forest light {id} shares the unshadowed lamp pool"
        );
        assert_eq!(
            light.falloff,
            super::super::definitions::LightFalloff::SoftCore
        );
        let intensity = match id {
            409 | 411 | 414 => 140.0,
            421 => 260.0,
            _ => panic!("Unspecified Blood Forest lighting profile {id}"),
        };
        assert_eq!((light.range, light.intensity), (7.0, intensity));
        assert_eq!(light.color, [1.0, 0.10, 0.06]);
        assert_eq!(light.cave_intensity, 0.7);
        assert_eq!(light.flicker, 0.02);
    }
    for (material, color) in [
        (terra_voxel::MaterialId::AmberGeode, [1.0, 0.52, 0.08]),
        (terra_voxel::MaterialId::TideGeode, [0.14, 0.74, 1.0]),
    ] {
        let (_, light) = assets
            .lights()
            .find(|(id, _)| *id == material as u16)
            .unwrap();
        assert_eq!(light.color, color);
        assert_eq!((light.range, light.intensity), (12.0, 920.0));
        assert!(!light.shadows);
    }
    assert!(
        assets
            .library
            .as_ref()
            .unwrap()
            .blocks
            .iter()
            .filter(|b| matches!(b.id, 398 | 399))
            .all(|b| b.light.is_none()),
        "inactive runes and decorative mist never create lights or fog volumes"
    );
    let (_, crusher_lantern) = assets.lights().find(|(id, _)| *id == 99).unwrap();
    assert_eq!(crusher_lantern.range, 4.0);
    assert_eq!(crusher_lantern.offset, Some([0.0425, 0.454, 0.896]));
    assert!(!crusher_lantern.shadows && crusher_lantern.intensity <= 80.0);
    let (_, cave_mushroom) = assets.lights().find(|(id, _)| *id == 56).unwrap();
    assert_eq!((cave_mushroom.range, cave_mushroom.intensity), (7.0, 680.0));
    assert_eq!(cave_mushroom.color, [1.0, 0.57, 0.22]);
    let (_, lantern) = assets.lights().find(|(id, _)| *id == 28).unwrap();
    assert_eq!(lantern.range, 4.0);
    assert_eq!(
        lantern.falloff,
        super::super::definitions::LightFalloff::InverseSquare
    );
    assert!(!lantern.shadows);
    assert_eq!(lantern.cave_intensity, 1.0);
    assert_eq!(lantern.offset, Some([0.16, 0.87, 0.18]));
    for ore in assets
        .library
        .as_ref()
        .unwrap()
        .blocks
        .iter()
        .filter(|b| (30..=35).contains(&b.id))
    {
        assert!(ore.light.is_none());
        assert!(ore.materials.values().all(|m| m.emission == [0.0; 3]));
    }
    let mut materials = Assets::<StandardMaterial>::default();
    let normal = assets.shared_material(&CubeMaterialDefinition::default(), None, &mut materials);
    let emitting = assets.shared_material(
        &CubeMaterialDefinition {
            emission: [0.2, 1.0, 1.6],
            ..default()
        },
        None,
        &mut materials,
    );
    assert_ne!(normal, emitting);
    assert!(
        materials
            .get(&assets.materials[emitting])
            .unwrap()
            .emissive
            .blue
            > 1.0
    );
}

#[test]
fn every_shipped_item_description_is_editable_json_metadata() {
    let directory = terra_core::asset_directory().join("terra");
    let library = BlockLibrary::load(&directory.join("blocks")).unwrap();
    assert!(
        library
            .blocks
            .iter()
            .all(|b| !b.description.trim().is_empty())
    );
    let props = super::super::definitions::load_prop_metadata(&directory.join("props")).unwrap();
    assert_eq!(
        props.descriptions.len(),
        terra_core::ResourcePropKind::ALL.len()
    );
    assert!(
        props
            .descriptions
            .values()
            .all(|description| !description.trim().is_empty())
    );
    let mut definition = library.blocks[0].clone();
    definition.description = "A custom resource-pack description.".into();
    assert!(definition.validate().is_ok());
    assert_eq!(
        definition.description,
        "A custom resource-pack description."
    );
}

#[test]
fn prop_palette_matches_the_authored_grass_green_with_moderate_variation() {
    let mut assets = BlockAssets::default();
    let mut materials = Assets::<StandardMaterial>::default();
    let palette = load_prop_materials(&mut assets, &mut materials);
    let target = [0.28, 0.46, 0.08];
    let mut luminances = Vec::new();
    for (slot, expected) in [
        (palette.foliage[0], [0.245, 0.405, 0.072]),
        (palette.foliage[1], target),
        (palette.foliage[2], [0.310, 0.495, 0.095]),
        (palette.stem, [0.225, 0.360, 0.063]),
    ] {
        let material = materials
            .get(&assets.materials[slot])
            .expect("prop material");
        let color = material.base_color.to_linear();
        for (channel, expected) in [color.red, color.green, color.blue]
            .into_iter()
            .zip(expected)
        {
            assert!((channel - expected).abs() < 0.000001);
        }
        assert!(color.green > color.red * 1.5 && color.green > color.blue * 4.5);
        luminances.push(color.red * 0.2126 + color.green * 0.7152 + color.blue * 0.0722);
        assert_eq!(color.alpha, 1.0);
        assert!(material.base_color_texture.is_none());
    }
    assert!(luminances[..3].windows(2).all(|pair| pair[0] < pair[1]));
    assert!(luminances[2] / luminances[0] < 1.3);
    assert!(luminances[3] < luminances[0]);
    assert_eq!(palette.flowers.len(), 8);
    for (slot, expected) in [
        (palette.dry_grass[0], [0.36, 0.31, 0.10, 1.0]),
        (palette.dry_grass[1], [0.47, 0.39, 0.16, 1.0]),
        (palette.dry_grass[2], [0.32, 0.34, 0.12, 1.0]),
        (palette.twig, [0.26, 0.17, 0.075, 1.0]),
        (palette.flowers[6], [0.94, 0.29, 0.045, 1.0]),
        (palette.flowers[7], [0.27, 0.69, 0.20, 1.0]),
    ] {
        assert_eq!(
            materials
                .get(&assets.materials[slot])
                .expect("flower material")
                .base_color
                .to_linear()
                .to_f32_array(),
            expected
        );
    }
}

#[test]
fn cube_materials_share_handles_without_merging_distinct_colors() {
    let mut assets = BlockAssets::default();
    let mut materials = Assets::<StandardMaterial>::default();
    let plain = CubeMaterialDefinition::default();
    let first = assets.shared_material(&plain, Some("textures/dirt.png"), &mut materials);
    let same = assets.shared_material(&plain, Some("textures/dirt.png"), &mut materials);
    let stone = assets.shared_material(&plain, Some("textures/stone.png"), &mut materials);
    let tinted = assets.shared_material(
        &CubeMaterialDefinition {
            base_color: [0.35, 0.70, 0.20, 1.0],
            ..plain.clone()
        },
        Some("textures/dirt.png"),
        &mut materials,
    );
    assert_eq!(first, same);
    assert_ne!(first, stone);
    assert_ne!(first, tinted);
    let matte = assets.shared_material(
        &CubeMaterialDefinition {
            reflectance: 0.08,
            ..plain.clone()
        },
        Some("textures/dirt.png"),
        &mut materials,
    );
    assert_ne!(
        first, matte,
        "Reflectance is part of the shared material identity"
    );
    assert_eq!(
        materials.get(&assets.materials[matte]).unwrap().reflectance,
        0.08
    );
    assert_eq!(materials.len(), 4);
    assert_eq!(assets.materials.len(), 4);
}

#[test]
fn shipped_cube_materials_share_soil_but_keep_whole_snow_distinct_from_rock() {
    let library = BlockLibrary::load(&terra_core::asset_directory().join("terra/blocks"))
        .expect("definitions");
    let mut assets = BlockAssets::default();
    let mut materials = Assets::<StandardMaterial>::default();
    let mut slots = HashMap::new();
    for block in &library.blocks {
        for (name, definition) in &block.materials {
            let slot = assets.shared_material(
                definition,
                block.textures.get(name).map(String::as_str),
                &mut materials,
            );
            slots.insert((block.id, name.as_str()), slot);
        }
    }
    assert!(
        !slots.keys().any(|(id, _)| *id == 1),
        "authored grass uses GLTF PBR, not procedural soil"
    );
    assert_ne!(
        slots[&(3, "Weathered blue stone")],
        slots[&(5, "Soft snow")]
    );
    for id in [79, 87, 95] {
        assert_eq!(
            slots[&(4, "Fine sand")],
            slots[&(id, "Fine sand")],
            "Sand construction shapes keep the exact shared PBR material"
        );
    }
    let sand = materials
        .get(&assets.materials[slots[&(4, "Fine sand")]])
        .unwrap();
    assert_eq!(sand.perceptual_roughness, 0.94);
    assert_eq!(sand.reflectance, 0.12);
    // Each surface has its own shading/texture identity. Every paver shares
    // the same side material, while each painted deck owns a single extra slot.
    let mut new_slots = std::collections::BTreeSet::from([
        slots[&(terra_core::bile::FLUID, "Bile")],
        slots[&(terra_core::bile::BLOCK, "Surface")],
        slots[&(terra_core::street::BLOCK, "Surface")],
        slots[&(terra_core::street::STRAW, "Surface")],
    ]);
    for id in terra_core::street::NORMAL..=terra_core::street::LOWERED {
        assert_eq!(
            slots[&(id, "Surface")],
            slots[&(terra_core::street::BLOCK, "Surface")]
        );
        assert!(new_slots.insert(slots[&(id, "Deck")]));
    }
    assert_eq!(new_slots.len(), 10);
    assert!(
        slots
            .iter()
            .all(|((id, _), s)| *id >= terra_core::bile::FLUID || !new_slots.contains(s))
    );
    let plains_faces = [
        (terra_voxel::MaterialId::BlueSlateShingles as u16, "surface"),
        (terra_voxel::MaterialId::BoundHayBale as u16, "end"),
        (terra_voxel::MaterialId::BoundHayBale as u16, "surface"),
        (terra_voxel::MaterialId::MeadowLimestone as u16, "surface"),
    ];
    let plains_slots = std::collections::BTreeSet::from(plains_faces.map(|face| slots[&face]));
    assert_eq!(plains_slots.len(), plains_faces.len());
    assert!(
        slots
            .iter()
            .all(|(face, slot)| plains_faces.contains(face) || !plains_slots.contains(slot))
    );
    let savanna_slots: std::collections::BTreeSet<_> = slots
        .iter()
        .filter(|((id, _), _)| (632..=647).contains(id))
        .map(|(_, slot)| *slot)
        .collect();
    assert_ne!(slots[&(632, "top")], slots[&(633, "top")]);
    assert_ne!(slots[&(644, "spray")], slots[&(647, "spray")]);
    let petrified_slots: std::collections::BTreeSet<_> = slots
        .iter()
        .filter(|((id, _), _)| matches!(id,703..=709|720..=721|724..=731))
        .map(|(_, slot)| *slot)
        .collect();
    let swamp_slots: std::collections::BTreeSet<_> = slots
        .iter()
        .filter(|((id, _), _)| (670..=682).contains(id))
        .map(|(_, slot)| *slot)
        .collect();
    assert_ne!(slots[&(682, "side")], slots[&(682, "spray")]);
    use terra_voxel::MaterialId as M;
    let coast_slots: std::collections::BTreeSet<_> = (M::CoastalGranite as u16
        ..=M::HarborRoofTiles as u16)
        .map(|id| slots[&(id, "side")])
        .collect();
    assert_eq!(coast_slots.len(), 11);
    let crystal_faces: std::collections::BTreeSet<_> = library
        .blocks
        .iter()
        .filter(|b| (M::CrystalStone as u16..=M::AmethystPrismTip as u16).contains(&b.id))
        .flat_map(|b| {
            b.face_materials
                .iter()
                .flatten()
                .map(move |name| (b.id, name.as_str()))
        })
        .collect();
    // Twelve cube textures. The two spires also need their own shaded edge and cap.
    let expected_crystal_faces: std::collections::BTreeSet<_> = (M::CrystalStone as u16
        ..=M::VioletRoofSlate as u16)
        .chain([M::VaultKeystone as u16])
        .map(|id| (id, "side"))
        .chain(
            [M::AmethystSpire, M::CyanSpire]
                .into_iter()
                .flat_map(|m| [(m as u16, "edge"), (m as u16, "cap")]),
        )
        .collect();
    assert_eq!(crystal_faces, expected_crystal_faces);
    let crystal_slots: std::collections::BTreeSet<_> =
        crystal_faces.iter().map(|face| slots[face]).collect();
    assert_eq!(crystal_slots.len(), crystal_faces.len());
    assert!(
        slots
            .iter()
            .all(|(face, slot)| crystal_faces.contains(face) || !crystal_slots.contains(slot))
    );
    let regional_faces = [
        (519, "surface"),
        (519, "side"),
        (520, "surface"),
        (520, "side"),
        (527, "top"),
        (527, "side"),
        (528, "top"),
        (528, "side"),
        (529, "top"),
        (529, "side"),
        (530, "surface"),
        (531, "surface"),
        (541, "wet_sand"),
        (542, "marine_limestone"),
        (543, "reef_stone"),
        (544, "sea_basalt"),
        (581, "side"),
        (581, "top"),
        (582, "side"),
        (582, "top"),
        (583, "side"),
        (583, "top"),
        (584, "side"),
        (584, "top"),
        (584, "soil"),
        (585, "side"),
        (585, "top"),
        (585, "spray"),
        (586, "side"),
        (586, "top"),
        (586, "spray"),
        // The fence has its own lacquer texture. Stairs and slabs still share planks.
        (138, "Cherry boards"),
        (596, "side"),
        (596, "top"),
        (596, "bottom"),
        (597, "side"),
        (597, "top"),
        (598, "side"),
        (598, "top"),
        (606, "side"),
        (606, "top"),
        (606, "spray"),
        (607, "side"),
        (607, "top"),
        (607, "spray"),
        (613, "side"),
        (614, "side"),
        (615, "side"),
        (616, "side"),
        (617, "side"),
        (617, "end"),
        (618, "side"),
        (619, "side"),
        (620, "side"),
        (621, "side"),
        (621, "end"),
        (622, "side"),
        (623, "side"),
        (629, "side"),
        (629, "top"),
    ];
    let regional_slots = std::collections::BTreeSet::from(regional_faces.map(|face| slots[&face]));
    assert_eq!(regional_slots.len(), regional_faces.len());
    assert!(
        slots
            .iter()
            .all(|(face, slot)| regional_faces.contains(face) || !regional_slots.contains(slot))
    );
    assert_eq!(
        materials.len(),
        72 + terra_core::OreKind::ALL.len() + terra_core::beds::DyeColor::ALL.len()
            + 1 // Distinct apple canopy.
            + 1 // One new angular ballast material shared by its three shapes.
            + 6 // Independent top art for the six original foliage blocks.
            + 16 // Ten Spirit cubes, including three foliage tops.
            + 23 // Thirteen clay, seven grave stones, two rune states and mist.
            + 20 // Twelve Blood cubes, turf side/bottom, log end and foliage top.
            + 3 // Cactus stem, top cut and root cap have independent textures.
            + 11 // Independent near leaf sprays, including the needle profile.
            + 1 // Solid fog stone has its own matte material.
            + 2 // Shared asphalt aggregate and contrasting stone curb textures.
            + 2 // New independent oak and birch end grain.
            + 2 // Wet and cured tar surface materials.
            + 14 // Salt gallery bedding, cleavage faces and halite ore matrix.
            + 9 // Five karst bedding/flank pairs replace one procedural dripstone material.
            + 24 // Twelve living cave surfaces, each with a separate side and top.
            + 4 // Root soil, wet mud and the two sanctuary stones.
            + 13 // Parasol gills, caps, woodland soil, shingles and timber.
            + savanna_slots.len()
            + swamp_slots.len()
            + petrified_slots.len()
            + crystal_slots.len()
            + coast_slots.len()
            + regional_slots.len() // Independently painted regional surfaces and foliage sprays.
            + plains_slots.len() // Blue slate, meadow limestone and both hay bale textures.
            + new_slots.len()
    );
    let spirit_slots: std::collections::BTreeSet<_> = slots
        .iter()
        .filter(|((id, _), _)| (306..=315).contains(id))
        .map(|(_, slot)| *slot)
        .collect();
    assert_eq!(
        spirit_slots.len(),
        19,
        "individual Spirit textures retain their material identity"
    );
    assert!(
        slots
            .iter()
            .all(|((id, _), slot)| *id >= 306 || !spirit_slots.contains(slot))
    );
    let ballast = slots[&(206, "Angular ballast")];
    assert_eq!(ballast, slots[&(207, "Angular ballast")]);
    assert_eq!(ballast, slots[&(208, "Angular ballast")]);
    assert_ne!(
        ballast,
        slots[&(42, "Rounded gravel")],
        "angular compacted ballast retains its own texture"
    );
    let ballast_material = materials.get(&assets.materials[ballast]).unwrap();
    assert_eq!(ballast_material.perceptual_roughness, 0.99);
    assert_eq!(ballast_material.reflectance, 0.08);
    assert_ne!(
        slots[&(195, "Painted moss canopy")],
        slots[&(7, "Painted moss canopy")],
        "Apple canopy keeps its own yellow-green material"
    );
    assert_eq!(slots[&(103, "Woven wool")], slots[&(104, "Woven wool")]);
    assert_eq!(slots[&(103, "Woven wool")], slots[&(105, "Woven wool")]);
    let mut textile_slots = std::collections::BTreeSet::new();
    for color in terra_core::beds::DyeColor::ALL {
        let carpet = terra_core::textiles::dyed(terra_core::textiles::WHITE_CARPET, color).unwrap();
        let wool = terra_core::textiles::dyed(terra_core::textiles::WHITE_WOOL, color).unwrap();
        let carpet_slot = slots[&(carpet, "Woven wool")];
        let wool_slot = slots[&(wool, "Woven wool")];
        assert!(
            textile_slots.insert(carpet_slot),
            "{color:?} needs its own tint"
        );
        assert_eq!(
            materials
                .get(&assets.materials[carpet_slot])
                .unwrap()
                .base_color
                .to_linear()
                .to_f32_array(),
            color.tint()
        );
        if color == terra_core::beds::DyeColor::White {
            assert_ne!(
                carpet_slot, wool_slot,
                "legacy white wool keeps its untinted material"
            );
            assert_eq!(
                materials
                    .get(&assets.materials[wool_slot])
                    .unwrap()
                    .base_color
                    .to_linear()
                    .to_f32_array(),
                [1.0; 4]
            );
        } else {
            assert_eq!(
                carpet_slot, wool_slot,
                "matching dyed wool and carpet must share a handle"
            );
        }
    }
    assert_eq!(textile_slots.len(), terra_core::beds::DyeColor::ALL.len());
    let fired_clay = slots[&(terra_core::flower_pot::MATERIAL, "Fired clay")];
    assert_ne!(fired_clay, slots[&(19, "River clay")]);
    assert!(!textile_slots.contains(&fired_clay));
    assert_eq!(
        materials
            .get(&assets.materials[fired_clay])
            .unwrap()
            .base_color
            .to_linear()
            .to_f32_array(),
        [0.54, 0.25, 0.15, 1.0]
    );
    assert_ne!(
        slots[&(102, "Compacted path")],
        slots[&(102, "Packed earth")]
    );
    assert_ne!(
        slots[&(terra_core::farming::FARMLAND, "Cultivated soil")],
        slots[&(terra_core::farming::FARMLAND, "Rooted cultivated soil")],
        "Cultivated furrows and rooted sides keep their separate authored textures"
    );
    assert_ne!(
        slots[&(terra_core::farming::FARMLAND, "Cultivated soil")],
        slots[&(2, "Root-rich earth")],
        "Cultivated soil keeps its darker, matte material while sharing geometry templates"
    );
    assert_ne!(slots[&(57, "Mineral rock")], slots[&(58, "Mineral rock")]);
    assert_ne!(slots[&(57, "Mineral rock")], slots[&(30, "Host rock")]);
    assert_ne!(slots[&(49, "Sea ice")], slots[&(5, "Soft snow")]);
    assert_ne!(slots[&(38, "Salt body")], slots[&(30, "Host rock")]);
    assert_eq!(slots[&(37, "Dripstone")], slots[&(39, "Dripstone")]);
    for ore in terra_core::OreKind::ALL {
        let host = slots[&(ore.material(), "Host rock")];
        if ore == terra_core::OreKind::Salt {
            assert_ne!(host, slots[&(3, "Weathered blue stone")]);
        } else {
            assert_eq!(host, slots[&(3, "Weathered blue stone")]);
        }
    }
    assert_eq!(
        slots[&(14, "Animated water")],
        slots[&(27, "Flowing water")]
    );
    for id in 22..=26 {
        assert_eq!(
            slots[&(21, "Mushroom gills")],
            slots[&(id, "Mushroom gills")]
        );
        assert_ne!(slots[&(21, "Mushroom cap")], slots[&(id, "Mushroom cap")]);
    }
    assert!(assets.models.is_empty());
}

#[test]
fn weather_cycles_share_surface_factors_and_restore_registry_materials() {
    use terra_core::{GameConfig, WeatherKind};

    let mut library = BlockLibrary::load(&terra_core::asset_directory().join("terra/blocks"))
        .expect("definitions");
    // Keep coverage of the legacy procedural cube contract alongside authored grass.
    let mut legacy_grass = library
        .blocks
        .iter()
        .find(|block| block.id == 4)
        .unwrap()
        .clone();
    legacy_grass.id = 1;
    let grass = library
        .blocks
        .iter_mut()
        .find(|block| block.id == 1)
        .unwrap();
    *grass = legacy_grass;
    let top = grass.face_materials.as_ref().unwrap()[3].clone();
    let custom = grass.materials.get_mut(&top).unwrap();
    custom.base_color = [0.83, 0.91, 0.76, 0.93];
    custom.roughness = 0.66;

    let mut assets = BlockAssets::default();
    let mut materials = Assets::<StandardMaterial>::default();
    let mut templates = TemplateSet::default();
    let mut originals = Vec::new();
    for block in library
        .blocks
        .iter()
        .filter(|block| [1, 3, 4, 5].contains(&block.id))
    {
        let name = &block.face_materials.as_ref().unwrap()[3];
        let definition = &block.materials[name];
        let slot = assets.shared_material(
            definition,
            block.textures.get(name).map(String::as_str),
            &mut materials,
        );
        originals.push((block.id, assets.materials[slot].clone(), definition.clone()));
        templates.blocks.insert(
            block.id,
            BlockTemplate {
                surface: None,
                render_kind: BlockRenderKind::Cube,
                bevel_width: 0.0,
                faces: std::array::from_fn(|_| Vec::new()),
                face_materials: [slot; 6],
                side_cap: None,
                details: Vec::new(),
                model: Vec::new(),
            },
        );
    }
    assets.library = Some(library);
    assets.templates = Some(Arc::new(templates));
    let count = materials.len();
    let mut app = App::new();
    app.insert_resource(GameConfig::default())
        .init_resource::<terra_core::WorldClock>()
        .insert_resource(assets)
        .insert_resource(materials)
        .add_systems(Update, crate::appearance::update_weather_materials);

    for (enabled, day) in [
        (false, 0),
        (true, 7),
        (true, 14),
        (true, 21),
        (true, 0),
        (false, 0),
    ] {
        app.world_mut()
            .resource_mut::<GameConfig>()
            .world
            .seasons
            .enabled = enabled;
        app.world_mut().resource_mut::<terra_core::WorldClock>().day = day;
        let grass_tint = app
            .world()
            .resource::<terra_core::WorldClock>()
            .season(app.world().resource::<GameConfig>().world.seasons)
            .grass_tint()
            .map(|value| (value * 128.0).round() / 128.0);
        for (weather, intensity) in [
            (WeatherKind::Clear, 1.0),
            (WeatherKind::Rain, 0.75),
            (WeatherKind::Rain, 0.25),
            (WeatherKind::Snow, 1.0),
            (WeatherKind::Clear, 1.0),
        ] {
            {
                let mut config = app.world_mut().resource_mut::<GameConfig>();
                config.client.weather = weather;
                config.client.weather_intensity = intensity;
            }
            app.update();
            let materials = app.world().resource::<Assets<StandardMaterial>>();
            assert_eq!(
                materials.len(),
                count,
                "weather must reuse existing materials"
            );
            for (id, handle, original) in &originals {
                let material = materials.get(handle).unwrap();
                let mut factor = if *id == 3 {
                    [1.0; 3]
                } else {
                    crate::appearance::weather_tint(weather, intensity)
                };
                if *id == 1 {
                    for axis in 0..3 {
                        factor[axis] *= grass_tint[axis];
                    }
                }
                let actual = material.base_color.to_linear().to_f32_array();
                for axis in 0..3 {
                    assert!(
                        (actual[axis] / original.base_color[axis] - factor[axis]).abs() < 1e-6,
                        "surface {id} must combine {weather:?} with its own seasonal baseline (enabled={enabled}, day={day})"
                    );
                }
                assert_eq!(actual[3], original.base_color[3]);
                let roughness = if *id != 3 && weather == WeatherKind::Rain {
                    (original.roughness - 0.28 * intensity).max(0.1)
                } else {
                    original.roughness
                };
                assert!((material.perceptual_roughness - roughness).abs() < 1e-6);
                if weather == WeatherKind::Clear && (*id != 1 || grass_tint == [1.0; 3]) {
                    assert_eq!(
                        actual, original.base_color,
                        "restore definition, not a prior tint"
                    );
                }
            }
        }
    }
}

#[test]
fn custom_material_selection_uses_surface_area_not_primitive_order() {
    let mut faces = cube_faces(0.0, [5; 6], None);
    let cap = faces[3].remove(0);
    let mut bevel = cap.clone();
    bevel.material = 2;
    for position in &mut bevel.positions {
        position[0] *= 0.01;
    }
    assert_eq!(dominant_material(&[bevel, cap]), 5);
}

#[test]
fn publishing_mips_keeps_image_handles_and_invalidates_only_dependent_materials() {
    use bevy::asset::AssetEvent;
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<Image>()
        .init_asset::<StandardMaterial>();
    let image = Image::new(
        Extent3d {
            width: 2,
            height: 2,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        [128, 128, 128, 255].repeat(4),
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD,
    );
    let texture = app
        .world_mut()
        .resource_mut::<Assets<Image>>()
        .add(image.clone());
    let material = app
        .world_mut()
        .resource_mut::<Assets<StandardMaterial>>()
        .add(StandardMaterial {
            base_color_texture: Some(texture.clone()),
            ..default()
        });
    let other = app
        .world_mut()
        .resource_mut::<Assets<StandardMaterial>>()
        .add(StandardMaterial::default());
    app.update();
    app.world_mut()
        .resource_mut::<Messages<AssetEvent<StandardMaterial>>>()
        .clear();
    let mipmapped = super::super::mipmaps::generate(image).expect("mip chain");
    app.world_mut()
        .resource_scope(|world, mut images: Mut<Assets<Image>>| {
            let changed = publish_mipmapped_image(
                &mut images,
                &mut world.resource_mut::<Assets<StandardMaterial>>(),
                &[material.clone(), other.clone()],
                &texture,
                mipmapped,
            )
            .expect("publish image");
            assert_eq!(changed, 1);
        });
    app.update();
    let events: Vec<_> = app
        .world_mut()
        .resource_mut::<Messages<AssetEvent<StandardMaterial>>>()
        .drain()
        .collect();
    assert!(
        events
            .iter()
            .any(|event| matches!(event, AssetEvent::Modified { id } if *id == material.id()))
    );
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, AssetEvent::Modified { id } if *id == other.id()))
    );
    let images = app.world().resource::<Assets<Image>>();
    let published = images.get(&texture).expect("same image handle");
    assert_eq!(published.texture_descriptor.mip_level_count, 2);
    assert!(
        !published
            .asset_usage
            .contains(RenderAssetUsages::MAIN_WORLD)
    );
    assert!(
        published
            .asset_usage
            .contains(RenderAssetUsages::RENDER_WORLD)
    );
    assert_eq!(
        app.world()
            .resource::<Assets<StandardMaterial>>()
            .get(&material)
            .expect("same material")
            .base_color_texture,
        Some(texture)
    );
}

fn load_pedestal() -> Vec<PrimitiveTemplate> {
    load_static_scene("terra/blocks/fixtures/pedestal.gltf", 65000)
}

pub(super) fn load_static_scene(path: &'static str, id: u16) -> Vec<PrimitiveTemplate> {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin {
            file_path: terra_core::asset_directory().to_string_lossy().into_owned(),
            ..default()
        },
        bevy::world_serialization::WorldSerializationPlugin,
        bevy::mesh::MeshPlugin,
        bevy::gltf::GltfPlugin::default(),
    ))
    .init_asset::<Image>()
    // This headless fixture has no TexturePlugin::finish to register the PNG
    // loader. Hydraulics use real external PBR maps, unlike older color-only glTFs.
    .register_asset_loader(bevy::image::ImageLoader::new(
        bevy::image::CompressedImageFormats::NONE,
    ))
    .init_asset::<StandardMaterial>();
    app.finish();
    app.cleanup();
    let handle: Handle<Gltf> = app
        .world()
        .resource::<AssetServer>()
        .load_builder()
        .with_settings(|settings: &mut GltfLoaderSettings| {
            settings.include_source = true;
            settings.load_meshes = RenderAssetUsages::MAIN_WORLD;
        })
        .load(path);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        app.update();
        let server = app.world().resource::<AssetServer>();
        let dependencies = server.get_recursive_dependency_load_state(handle.id());
        assert!(
            !matches!(
                server.get_load_state(handle.id()),
                Some(LoadState::Failed(_))
            ) && !matches!(dependencies, Some(RecursiveDependencyLoadState::Failed(_))),
            "fixture {path} load failed: {:?}; dependencies: {dependencies:?}",
            server.get_load_state(handle.id()),
        );
        if server.is_loaded_with_dependencies(handle.id()) {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "fixture {path} loading timed out: {:?}; dependencies: {dependencies:?}",
            server.get_load_state(handle.id()),
        );
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    let mut assets = BlockAssets::default();
    let count = app
        .world()
        .resource::<Assets<Gltf>>()
        .get(&handle)
        .unwrap()
        .materials
        .len();
    for index in 0..count {
        assets.material_slots.insert((id, index), index);
    }
    let mut state = bevy::ecs::system::SystemState::<SourceAssets>::new(app.world_mut());
    let source = state.get_mut(app.world_mut()).expect("asset resources");
    read_scene(
        id,
        source.gltfs.get(&handle).expect("loaded fixture"),
        &source,
        &assets,
    )
    .expect("baked static scene")
}

#[test]
fn shipped_furniture_furnace_and_chimneys_have_valid_non_degenerate_static_geometry() {
    for (path, id) in [
        ("terra/blocks/models/workbench.gltf", 28),
        ("terra/blocks/models/chest.gltf", 29),
        ("terra/blocks/models/furnace.gltf", 59),
        ("terra/blocks/models/chimney_straight.gltf", 60),
        ("terra/blocks/models/chimney_elbow.gltf", 61),
        ("terra/blocks/models/chimney_horizontal.gltf", 62),
        ("terra/blocks/models/chimney_bend_up.gltf", 63),
        ("terra/blocks/models/chimney_bend_down.gltf", 64),
        ("terra/blocks/models/oak_door.gltf", 65),
        ("terra/blocks/models/birch_door.gltf", 66),
        ("terra/blocks/models/spruce_door.gltf", 67),
        ("terra/blocks/models/oak_window.gltf", 68),
        ("terra/blocks/models/birch_window.gltf", 69),
        ("terra/blocks/models/spruce_window.gltf", 70),
        ("terra/blocks/models/oak_bed.gltf", 71),
        ("terra/blocks/models/birch_bed.gltf", 72),
        ("terra/blocks/models/spruce_bed.gltf", 73),
        ("terra/blocks/models/crusher.gltf", 99),
        ("terra/blocks/models/hand_crank.gltf", 100),
        ("terra/blocks/models/cauldron.gltf", 161),
        ("terra/blocks/models/campfire.gltf", 162),
        ("terra/blocks/models/wood_pipe.gltf", 163),
        ("terra/blocks/models/stone_pipe.gltf", 164),
        ("terra/blocks/models/iron_pipe.gltf", 165),
        ("terra/blocks/models/valve.gltf", 166),
        ("terra/blocks/models/faucet.gltf", 167),
        ("terra/blocks/models/pump.gltf", 168),
        ("terra/blocks/models/basin.gltf", 169),
        ("terra/blocks/models/hold_block.gltf", 170),
        ("terra/blocks/models/fluid_infuser.gltf", 171),
        ("terra/blocks/models/glass_pipe.gltf", 172),
    ] {
        let model = load_static_scene(path, id);
        let bottom = model
            .iter()
            .flat_map(|p| &p.positions)
            .map(|p| p[1])
            .fold(f32::INFINITY, f32::min);
        if matches!(id, 28 | 29 | 59 | 65..=73 | 99) {
            assert!(
                bottom.abs() < 0.00001,
                "Furniture feet must touch the block floor: {path}"
            );
        }
        if matches!(id, 59 | 65..=67) {
            let top = model
                .iter()
                .flat_map(|primitive| &primitive.positions)
                .map(|point| point[1])
                .fold(f32::NEG_INFINITY, f32::max);
            assert!((top - 2.0).abs() < 0.00001);
        }
        if id == 99 {
            let top = model
                .iter()
                .flat_map(|primitive| &primitive.positions)
                .map(|point| point[1])
                .fold(f32::NEG_INFINITY, f32::max);
            assert!((top - 1.0).abs() < 0.00001);
            assert_eq!(model.len(), 13);
        }
        if id == 100 {
            let top = model
                .iter()
                .flat_map(|primitive| &primitive.positions)
                .map(|point| point[1])
                .fold(f32::NEG_INFINITY, f32::max);
            assert!((bottom - 0.205).abs() < 0.00001);
            assert!((top - 0.715).abs() < 0.00001);
            assert_eq!(model.len(), 7);
        }
        assert!(model.iter().map(|p| p.positions.len()).sum::<usize>() < 16_384);
        for primitive in model {
            assert_eq!(primitive.positions.len(), primitive.normals.len());
            for point in &primitive.positions {
                assert!(point.iter().enumerate().all(|(axis, value)| {
                    let maximum = if (matches!(id, 59 | 65..=67) && axis == 1)
                        || (terra_core::beds::is_bed(id) && axis == 2)
                    {
                        2.0
                    } else {
                        1.0
                    };
                    value.is_finite() && (-0.001..=maximum + 0.001).contains(value)
                }));
            }
            for triangle in primitive.indices.chunks_exact(3) {
                let [a, b, c]: [u32; 3] = triangle.try_into().unwrap();
                let normal = Vec3::from_array(primitive.normals[a as usize]);
                let a = Vec3::from_array(primitive.positions[a as usize]);
                let b = Vec3::from_array(primitive.positions[b as usize]);
                let c = Vec3::from_array(primitive.positions[c as usize]);
                assert!((b - a).cross(c - a).length_squared() > 1e-10);
                assert!(
                    (b - a).cross(c - a).dot(normal) > 0.0,
                    "Furniture faces must point outwards"
                );
            }
        }
    }
}

#[test]
fn compact_cauldron_composes_the_actual_fire_inside_its_stand_with_shared_materials() {
    let cauldron = load_static_scene("terra/blocks/models/cauldron.gltf", 161);
    let fire = load_static_scene("terra/blocks/models/campfire.gltf", 162);
    let mut templates = TemplateSet::default();
    for (id, model) in [(161, cauldron.clone()), (162, fire.clone())] {
        templates.blocks.insert(
            id,
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
    for facing in 0..4 {
        let state = |facing| terra_world::edits::BlockObjectState {
            facing,
            ..Default::default()
        };
        let plain = crate::meshing::custom::model_geometry(
            161,
            state(facing),
            Default::default(),
            [Some(0); 6],
            [0; 6],
            &templates,
        )
        .unwrap();
        let compact = crate::meshing::custom::model_geometry(
            161,
            state(facing | terra_core::brewing::ATTACHED_FIRE),
            Default::default(),
            [Some(0); 6],
            [0; 6],
            &templates,
        )
        .unwrap();
        let count = |parts: &[crate::meshing::builder::MeshBatch]| {
            parts.iter().map(|part| part.positions.len()).sum::<usize>()
        };
        assert_eq!(
            count(&plain),
            cauldron
                .iter()
                .map(|part| part.positions.len())
                .sum::<usize>()
        );
        assert_eq!(
            count(&compact) - count(&plain),
            fire.iter().map(|part| part.positions.len()).sum::<usize>()
        );
        assert!(count(&compact) < 8192);
        let rotation = Quat::from_rotation_y(f32::from(facing) * std::f32::consts::FRAC_PI_2);
        for original in &fire {
            let batch = compact
                .iter()
                .find(|part| part.material == original.material)
                .unwrap();
            let existing = plain
                .iter()
                .find(|part| part.material == original.material)
                .map_or(0, |part| part.positions.len());
            for (&actual, point) in batch.positions[existing..].iter().zip(&original.positions) {
                let point =
                    terra_world::interaction_shapes::attached_campfire_point(point.map(f64::from));
                assert!(point[0] > 0.175 && point[2] > 0.175);
                assert!(point[0] < 0.825 && point[2] < 0.825);
                assert!(point[1] < 0.34);
                let point = Vec3::from_array(point.map(|v| v as f32));
                let expected =
                    rotation * (point - Vec3::new(0.5, 0.0, 0.5)) + Vec3::new(0.5, 0.0, 0.5);
                assert!((Vec3::from_array(actual) - expected).length() < 1e-6);
            }
        }
        for batch in compact {
            assert!(
                batch
                    .positions
                    .iter()
                    .flatten()
                    .all(|v| v.is_finite() && (-1e-6..=1.000001).contains(v))
            );
            for triangle in batch.indices.chunks_exact(3) {
                let [a, b, c] = [triangle[0], triangle[1], triangle[2]]
                    .map(|index| Vec3::from_array(batch.positions[index as usize]));
                assert!(
                    (b - a)
                        .cross(c - a)
                        .dot(Vec3::from_array(batch.normals[triangle[0] as usize]))
                        > 0.0
                );
            }
            for (tangent, normal) in batch.tangents.iter().zip(&batch.normals) {
                let tangent = Vec3::new(tangent[0], tangent[1], tangent[2]);
                assert!((tangent.length() - 1.0).abs() < 1e-4);
                assert!(tangent.dot(Vec3::from_array(*normal)).abs() < 1e-4);
            }
        }
    }
}

#[test]
fn mixer_matches_its_one_cell_hopper_paddle_and_axial_drive_contract() {
    let model = load_static_scene("terra/blocks/models/mixer.gltf", 156);
    assert_eq!(
        model.len(),
        8,
        "four rigid and four shared rotating material groups"
    );
    assert!(model.iter().map(|p| p.positions.len()).sum::<usize>() <= 8192);
    for primitive in &model {
        assert_eq!(primitive.positions.len(), primitive.normals.len());
        assert_eq!(primitive.positions.len(), primitive.uvs.len());
        for point in &primitive.positions {
            assert!(
                point
                    .iter()
                    .all(|v| v.is_finite() && (-0.00001..=1.00001).contains(v))
            );
        }
        for triangle in primitive.indices.chunks_exact(3) {
            let [a, b, c] = [triangle[0], triangle[1], triangle[2]]
                .map(|i| Vec3::from_array(primitive.positions[i as usize]));
            let normal = Vec3::from_array(primitive.normals[triangle[0] as usize]);
            assert!((b - a).cross(c - a).dot(normal) > 1e-10);
            assert!(
                (b - a).cross(c - a).normalize().dot(normal) > 0.9999,
                "the authored mixer normal must agree with its actual sloped surface"
            );
        }
        if primitive.material >= 4 {
            for point in &primitive.positions {
                let radius = (point[1] - 0.5).hypot(point[2] - 0.5);
                assert!(
                    radius <= 0.211,
                    "every animation phase stays inside the chamber/gear envelope"
                );
            }
        }
    }
    let axle = model
        .iter()
        .flat_map(|p| &p.positions)
        .filter(|point| point[0] > 0.99999)
        .collect::<Vec<_>>();
    assert!(!axle.is_empty());
    for axis in [1, 2] {
        let low = axle.iter().map(|p| p[axis]).fold(f32::INFINITY, f32::min);
        let high = axle
            .iter()
            .map(|p| p[axis])
            .fold(f32::NEG_INFINITY, f32::max);
        assert!((low + high - 1.0).abs() < 0.00001);
    }
    let library = BlockLibrary::load(&terra_core::asset_directory().join("terra/blocks")).unwrap();
    let definition = library.blocks.iter().find(|block| block.id == 156).unwrap();
    assert_eq!(definition.name, "Mixer");
    assert!(definition.gameplay.struct_block);
    assert!(definition.collision_boxes.len() <= 64);
    let solid = |point: [f64; 3]| {
        definition.collision_boxes.iter().any(|bounds| {
            (0..3).all(|axis| {
                point[axis] >= bounds.minimum[axis] && point[axis] <= bounds.maximum[axis]
            })
        })
    };
    assert!(
        !solid([0.5, 0.96, 0.5]),
        "the input hopper center is genuinely open"
    );
    assert!(
        !solid([0.5, 0.22, 0.94]),
        "the front output channel stays open above its floor"
    );
    assert!(
        solid([0.5, 0.50, 0.25]),
        "the rear plank housing has matching collision"
    );
    assert!(solid([0.10, 0.05, 0.10]), "reinforced feet touch the floor");
}

#[test]
fn crusher_housing_closes_rear_and_sides_but_keeps_output_and_crank_mount_clear() {
    let model = load_static_scene("terra/blocks/models/crusher.gltf", 99);
    let face_area = |axis: usize, coordinate: f32, sign: f32| {
        model
            .iter()
            .flat_map(|primitive| {
                primitive
                    .indices
                    .chunks_exact(3)
                    .filter_map(move |triangle| {
                        let points = [triangle[0], triangle[1], triangle[2]]
                            .map(|index| Vec3::from_array(primitive.positions[index as usize]));
                        let normal = primitive.normals[triangle[0] as usize];
                        (normal[axis] * sign > 0.9
                            && points
                                .iter()
                                .all(|point| (point[axis] - coordinate).abs() < 0.00001))
                        .then(|| {
                            (points[1] - points[0])
                                .cross(points[2] - points[0])
                                .length()
                                * 0.5
                        })
                    })
            })
            .sum::<f32>()
    };
    assert!(
        face_area(2, 0.16, 1.0) > 0.35,
        "solid rear face must cover the crushing chamber"
    );
    assert!(face_area(0, 0.075, -1.0) > 0.33, "left housing panel");
    assert!(
        face_area(0, 0.925, 1.0) > 0.33,
        "right housing panel behind the gear"
    );
    let library = BlockLibrary::load(&terra_core::asset_directory().join("terra/blocks")).unwrap();
    let block = library.blocks.iter().find(|block| block.id == 99).unwrap();
    let solid = |point: [f64; 3]| {
        block.collision_boxes.iter().any(|bounds| {
            (0..3).all(|axis| {
                point[axis] >= bounds.minimum[axis] && point[axis] <= bounds.maximum[axis]
            })
        })
    };
    for point in [
        [0.5, 0.45, 0.15],
        [0.08, 0.45, 0.5],
        [0.92, 0.45, 0.5],
        [0.5, 0.45, 0.35],
        [0.5, 0.45, 0.65],
        [0.15, 0.85, 0.4],
        [0.85, 0.85, 0.4],
    ] {
        assert!(
            solid(point),
            "closed visible panel needs matching collision at {point:?}"
        );
    }
    assert!(
        !solid([0.2725, 0.30, 0.90]),
        "front output mouth must remain open"
    );
    assert!(
        !solid([1.005, 0.455, 0.515]),
        "housing must not intrude into the crank socket cell"
    );
    assert!(
        !solid([0.5, 0.95, 0.5]),
        "the real input center must be inside the open intake"
    );
}

#[test]
fn logistics_ports_use_bounded_reusable_open_housings_and_real_authored_volumes() {
    for (file, id) in [
        ("terra/blocks/models/item_hopper.gltf", 152),
        ("terra/blocks/models/item_importer.gltf", 153),
        ("terra/blocks/models/filtered_funnel.gltf", 157),
        ("terra/blocks/models/belt_merger.gltf", 158),
        ("terra/blocks/models/filtered_tunnel.gltf", 159),
        ("terra/blocks/models/belt_distributor.gltf", 160),
    ] {
        let model = load_static_scene(file, id);
        let count = model.iter().map(|p| p.positions.len()).sum::<usize>();
        assert!((200..=4096).contains(&count));
        for primitive in &model {
            assert!(primitive.positions.iter().all(|point| {
                point.iter().enumerate().all(|(axis, v)| {
                    v.is_finite() && (if axis == 1 { -0.375..=1.0 } else { 0.0..=1.0 }).contains(v)
                })
            }));
            assert!(
                primitive
                    .indices
                    .iter()
                    .all(|i| (*i as usize) < primitive.positions.len())
            );
            for triangle in primitive.indices.chunks_exact(3) {
                let p = triangle
                    .iter()
                    .map(|i| Vec3::from_array(primitive.positions[*i as usize]))
                    .collect::<Vec<_>>();
                let n = Vec3::from_array(primitive.normals[triangle[0] as usize]);
                assert!((p[1] - p[0]).cross(p[2] - p[0]).dot(n) > 0.0);
            }
        }
        let boxes = terra_core::block_shapes::builtin_boxes(id);
        assert!(!boxes.is_empty());
        assert!(terra_core::kinetic::is_extended(id));
        assert!(boxes.iter().any(|b| (b.minimum[1] + 0.375).abs() < 1e-6));
        let upper = if matches!(id, 158 | 160) {
            -0.125
        } else {
            -0.08
        };
        for (low, high) in [
            ([0.38, -0.37, 0.0], [0.62, upper, 1.0]),
            ([0.0, -0.37, 0.38], [1.0, upper, 0.62]),
        ] {
            assert!(
                !boxes.iter().any(|b| b.intersects(low, high)),
                "the belt and carried items pass through both axes of the open port"
            );
        }
        assert!(!terra_voxel::MaterialId::from_raw(id).unwrap().is_opaque());
    }
    let directory = terra_core::asset_directory().join("terra/blocks");
    let funnel: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(directory.join("models/item_importer.gltf")).unwrap(),
    )
    .unwrap();
    let support_slots: Vec<_> = funnel["materials"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
        .filter(|(_, material)| {
            material["name"]
                .as_str()
                .unwrap()
                .starts_with("port_support_")
        })
        .map(|(slot, _)| slot as u64)
        .collect();
    assert_eq!(support_slots.len(), 2);
    for primitive in funnel["meshes"][0]["primitives"].as_array().unwrap() {
        let accessor =
            &funnel["accessors"][primitive["attributes"]["POSITION"].as_u64().unwrap() as usize];
        let low = accessor["min"][1].as_f64().unwrap();
        if support_slots.contains(&primitive["material"].as_u64().unwrap()) {
            assert!(low <= -0.305 + 1e-6);
        } else {
            assert!(
                low >= -0.305 - 1e-6,
                "only dedicated legs may extend below the funnel"
            );
        }
    }
    let tunnel: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(directory.join("item_hopper.json")).unwrap())
            .unwrap();
    assert_eq!(tunnel["name"], "Tunnel");
}

#[test]
fn logistics_contact_pads_meet_belt_edges_without_entering_the_four_item_passages() {
    let library = BlockLibrary::load(&terra_core::asset_directory().join("terra/blocks")).unwrap();
    let contact = f64::from(terra_core::kinetic::CONVEYOR_TOP) - 1.0;
    for material in [153, 157, 158, 160] {
        let definition = library
            .blocks
            .iter()
            .find(|block| block.id == material)
            .unwrap();
        for axis in [0, 2] {
            for edge in [0.18, 0.82] {
                assert!(
                    definition
                        .collision_boxes
                        .iter()
                        .any(|bounds| (bounds.minimum[1] - contact).abs() < 1e-6
                            && bounds.minimum[axis] < edge
                            && bounds.maximum[axis] > edge),
                    "material {material} must touch ribbon edge {axis}/{edge}"
                );
            }
        }
        for (low, high) in [
            ([0.38, -0.37, 0.0], [0.62, -0.125, 1.0]),
            ([0.0, -0.37, 0.38], [1.0, -0.125, 0.62]),
        ] {
            assert!(
                !definition
                    .collision_boxes
                    .iter()
                    .any(|bounds| bounds.intersects(low, high))
            );
        }
        if matches!(material, 158 | 160) {
            let path = if material == 158 {
                "terra/blocks/models/belt_merger.gltf"
            } else {
                "terra/blocks/models/belt_distributor.gltf"
            };
            let model = load_static_scene(path, material);
            assert!(model.iter().map(|p| p.positions.len()).sum::<usize>() <= 2048);
        }
    }
}

#[test]
fn real_gearbox_assets_have_distinct_shared_rotors_centered_ports_and_bounded_housings() {
    for (file, material, cross) in [
        ("terra/blocks/models/gearbox_horizontal.gltf", 154, 2),
        ("terra/blocks/models/gearbox_vertical.gltf", 155, 1),
    ] {
        let model = load_static_scene(file, material);
        assert_eq!(model.len(), 6);
        assert_eq!(model.iter().map(|p| p.positions.len()).sum::<usize>(), 3608);
        for axis in [0, cross] {
            for end in [0.0, 1.0] {
                let port: Vec<_> = model
                    .iter()
                    .flat_map(|p| &p.positions)
                    .filter(|point| (point[axis] - end).abs() < 1e-6)
                    .collect();
                assert!(!port.is_empty());
                for other in (0..3).filter(|&other| other != axis) {
                    let low = port.iter().map(|p| p[other]).fold(f32::INFINITY, f32::min);
                    let high = port
                        .iter()
                        .map(|p| p[other])
                        .fold(f32::NEG_INFINITY, f32::max);
                    assert!(
                        (low + high - 1.0).abs() < 1e-6,
                        "the axle pivot stays centered at each cell boundary"
                    );
                }
            }
        }
        assert!(
            model
                .iter()
                .flat_map(|p| &p.positions)
                .flatten()
                .all(|value| (0.0..=1.0).contains(value))
        );
        assert!(
            terra_core::block_shapes::builtin_boxes(material)
                .iter()
                .all(|bounds| bounds.valid_for(material))
        );
    }
}

#[test]
fn real_non_cube_gltf_scene_bakes_node_hierarchy_normals_and_reflections() {
    let model = load_pedestal();
    assert_eq!(model.len(), 1);
    assert_eq!(model[0].indices.len(), 18);
    let points = &model[0].positions;
    for axis in 0..3 {
        let minimum = points.iter().map(|p| p[axis]).fold(f32::INFINITY, f32::min);
        let maximum = points
            .iter()
            .map(|p| p[axis])
            .fold(f32::NEG_INFINITY, f32::max);
        assert!((minimum - if axis == 1 { 0.0 } else { 0.1 }).abs() < 0.00001);
        assert!((maximum - if axis == 1 { 0.75 } else { 0.9 }).abs() < 0.00001);
    }
    let mut reflected = model[0].clone();
    bake_transform(
        65000,
        &mut reflected,
        bevy::math::Affine3A::from_scale_rotation_translation(
            Vec3::new(-1.0, 1.0, 1.0),
            Quat::IDENTITY,
            Vec3::X,
        ),
    )
    .expect("reflection");
    for primitive in [&model[0], &reflected] {
        for triangle in primitive.indices.chunks_exact(3) {
            let [a, b, c] = [triangle[0], triangle[1], triangle[2]]
                .map(|i| Vec3::from_array(primitive.positions[i as usize]));
            let normal = Vec3::from_array(primitive.normals[triangle[0] as usize]);
            assert!(((b - a).cross(c - a).normalize().dot(normal) - 1.0).abs() < 0.00001);
        }
    }
    assert!(
        bake_transform(
            65000,
            &mut reflected,
            bevy::math::Affine3A::from_scale(Vec3::ZERO)
        )
        .is_err()
    );
    assert!(
        bake_transform(
            65000,
            &mut reflected,
            bevy::math::Affine3A::from_translation(Vec3::X)
        )
        .is_err()
    );
}

#[test]
fn only_two_cell_furniture_scene_geometry_can_extend_into_a_second_vertical_cell() {
    let tall = PrimitiveTemplate {
        tangents: Vec::new(),
        positions: vec![[0.0; 3], [1.0, 2.0, 0.0], [1.0, 2.0, 1.0]],
        normals: vec![[1.0, 0.0, 0.0]; 3],
        uvs: vec![[0.0; 2]; 3],
        indices: vec![0, 1, 2],
        material: 0,
    };
    for id in [59, 65, 66, 67, 248] {
        assert!(bake_transform(id, &mut tall.clone(), bevy::math::Affine3A::IDENTITY).is_ok());
    }
    for id in [
        28, 29, 40, 60, 61, 62, 63, 64, 68, 69, 70, 71, 72, 73, 99, 65000,
    ] {
        assert!(
            bake_transform(id, &mut tall.clone(), bevy::math::Affine3A::IDENTITY).is_err(),
            "Only declared two-cell furniture may exceed one meter: {id}"
        );
    }
    for translation in [
        Vec3::X * 0.001,
        Vec3::Y * 0.001,
        Vec3::Z * 0.001,
        -Vec3::Y * 0.001,
    ] {
        assert!(
            bake_transform(
                59,
                &mut tall.clone(),
                bevy::math::Affine3A::from_translation(translation)
            )
            .is_err()
        );
    }
}

#[test]
fn bogie_gauge_bounds_are_shared_and_never_expand_other_models_or_vertical_limits() {
    let primitive = PrimitiveTemplate {
        positions: vec![[-0.61, 0.0, 0.0], [1.61, 1.0, 0.0], [1.61, 1.0, 1.0]],
        normals: vec![[0.0, 0.0, 1.0]; 3],
        tangents: Vec::new(),
        uvs: vec![[0.0; 2]; 3],
        indices: vec![0, 1, 2],
        material: 0,
    };
    assert!(bake_transform(211, &mut primitive.clone(), bevy::math::Affine3A::IDENTITY).is_ok());
    for id in [210, 212, 213, 214, 215, 216, 217, 65000] {
        assert!(
            bake_transform(id, &mut primitive.clone(), bevy::math::Affine3A::IDENTITY).is_err()
        );
    }
    for translation in [
        Vec3::X * 0.001,
        -Vec3::X * 0.001,
        Vec3::Y * 0.001,
        -Vec3::Y * 0.001,
    ] {
        assert!(
            bake_transform(
                211,
                &mut primitive.clone(),
                bevy::math::Affine3A::from_translation(translation)
            )
            .is_err()
        );
    }
    assert!(!test_static_scene("terra/blocks/models/train_bogie.gltf", 211).is_empty());
}

#[test]
fn only_beds_extend_into_the_paired_horizontal_cell() {
    let long = PrimitiveTemplate {
        tangents: Vec::new(),
        positions: vec![[0.0; 3], [1.0, 1.0, 0.0], [1.0, 1.0, 2.0]],
        normals: vec![[1.0, 0.0, 0.0]; 3],
        uvs: vec![[0.0; 2]; 3],
        indices: vec![0, 1, 2],
        material: 0,
    };
    for id in 71..=73 {
        assert!(bake_transform(id, &mut long.clone(), bevy::math::Affine3A::IDENTITY).is_ok());
        for offset in [Vec3::X, Vec3::Y, Vec3::Z, -Vec3::Z] {
            assert!(
                bake_transform(
                    id,
                    &mut long.clone(),
                    bevy::math::Affine3A::from_translation(offset * 0.001)
                )
                .is_err()
            );
        }
    }
    for id in [28, 29, 59, 65, 70, 74, 97, 65000] {
        assert!(bake_transform(id, &mut long.clone(), bevy::math::Affine3A::IDENTITY).is_err());
    }
}

#[test]
fn static_models_batch_once_and_do_not_occlude_terrain_or_each_other() {
    use crate::meshing::{DetailLevel, builder::RenderOptions, custom};
    use terra_voxel::{
        BlockSection, SECTION_VOLUME, SectionPos,
        meshing::{PADDED_VOLUME, mesh_section},
    };
    let mut templates = TemplateSet::default();
    templates.blocks.insert(
        65000,
        BlockTemplate {
            surface: None,
            render_kind: BlockRenderKind::CustomMesh,
            bevel_width: 0.0,
            faces: std::array::from_fn(|_| Vec::new()),
            face_materials: [0; 6],
            side_cap: None,
            details: Vec::new(),
            model: load_pedestal(),
        },
    );
    templates.blocks.insert(
        3,
        BlockTemplate {
            surface: None,
            render_kind: BlockRenderKind::Cube,
            bevel_width: 0.0,
            faces: cube_faces(0.0, [1; 6], None),
            face_materials: [1; 6],
            side_cap: None,
            details: Vec::new(),
            model: Vec::new(),
        },
    );
    let position = SectionPos::new(-1, 0, -1);
    let mut blocks = vec![0; SECTION_VOLUME];
    for z in 1..=3 {
        for y in 1..=3 {
            for x in 1..=3 {
                blocks[x + 16 * (y + 16 * z)] = 65000;
            }
        }
    }
    blocks[4 + 16 * (2 + 16 * 2)] = 3;
    let section = BlockSection::from_blocks(position, 0, blocks).expect("section");
    let snapshot = custom::capture(&section, &templates, &Default::default(), |_| None)
        .expect("scene snapshot");
    let mut padded = vec![0; PADDED_VOLUME];
    for z in 0..16 {
        for y in 0..16 {
            for x in 0..16 {
                padded[x + 1 + 18 * (y + 1 + 18 * (z + 1))] =
                    section.block([x, y, z]).expect("cell");
            }
        }
    }
    let geometry = mesh_section(&padded).expect("voxel mesher");
    for level in [DetailLevel::Near, DetailLevel::Middle, DetailLevel::Far] {
        let batches = custom::compose(
            &geometry,
            &snapshot,
            &templates,
            position,
            &RenderOptions {
                detail_level: level,
                water_level: None,
                terrain: None,
                exposure: None,
                tints: Default::default(),
                conditions: Default::default(),
            },
        )
        .expect("models");
        assert_eq!(
            batches.len(),
            2,
            "shared materials determine draws, not model count"
        );
        assert_eq!(
            batches[0].indices.len(),
            27 * 18,
            "interior pedestal must survive face culling"
        );
        assert_eq!(
            batches[1].indices.len(),
            36,
            "neighboring cube must retain all six surfaces"
        );
    }
    templates.blocks.insert(
        7,
        BlockTemplate {
            surface: None,
            render_kind: BlockRenderKind::Cube,
            bevel_width: 0.0,
            faces: cube_faces(0.0, [0; 6], None),
            face_materials: [0; 6],
            side_cap: None,
            details: Vec::new(),
            model: Vec::new(),
        },
    );
    let mut mixed = section.clone();
    mixed.set_block([6, 2, 2], 7).expect("colored leaves");
    padded[7 + 18 * (3 + 18 * 3)] = 7;
    let mixed_geometry = mesh_section(&padded).expect("mixed model and foliage");
    let mixed_snapshot =
        custom::capture(&mixed, &templates, &Default::default(), |_| None).expect("mixed snapshot");
    let batches = custom::compose(
        &mixed_geometry,
        &mixed_snapshot,
        &templates,
        position,
        &RenderOptions {
            detail_level: DetailLevel::Near,
            water_level: None,
            terrain: None,
            exposure: None,
            tints: Default::default(),
            // A real surface condition colors the leaf range; static models
            // sharing this slot must still retain neutral vertex colors.
            conditions: [(
                [6, 2, 2],
                terra_world::environment::SurfaceCondition {
                    wetness: 0.7,
                    ..Default::default()
                },
            )]
            .into(),
        },
    )
    .expect("mixed material batch");
    let combined = batches.iter().find(|batch| batch.material == 0).unwrap();
    assert_eq!(combined.colors.len(), combined.positions.len());
    assert!(combined.colors.iter().any(|color| *color != [1.0; 4]));
    assert!(
        combined.colors[combined.colors.len() - 27 * 18..]
            .iter()
            .all(|color| *color == [1.0; 4]),
        "models sharing a leaf material must retain neutral vertex colors"
    );
    let mut boundary = BlockSection::empty(position).expect("boundary section");
    boundary.set_block([15, 2, 2], 3).expect("boundary cube");
    let neighboring_model = [0, 2, -14];
    let snapshot = custom::capture(&boundary, &templates, &Default::default(), |global| {
        (global == neighboring_model).then_some(65000)
    })
    .expect("unloaded neighbor model from session delta");
    let mut padded = vec![0; PADDED_VOLUME];
    padded[16 + 18 * (3 + 18 * 3)] = 3;
    padded[17 + 18 * (3 + 18 * 3)] = 65000;
    let geometry = mesh_section(&padded).expect("section boundary");
    let batches = custom::compose(
        &geometry,
        &snapshot,
        &templates,
        position,
        &RenderOptions {
            detail_level: DetailLevel::Far,
            water_level: None,
            terrain: None,
            exposure: None,
            tints: Default::default(),
            conditions: Default::default(),
        },
    )
    .expect("boundary correction");
    assert_eq!(
        batches.len(),
        1,
        "neighbor's model belongs only to its owning section"
    );
    assert_eq!(batches[0].material, 1);
    assert_eq!(
        batches[0].indices.len(),
        36,
        "negative section restores the east cube face"
    );
}

#[test]
fn scene_model_definition_is_opt_in_and_does_not_change_shipped_ids() {
    let definition: super::super::definitions::BlockDefinition = serde_json::from_str(
        include_str!("../../../../assets/terra/blocks/fixtures/pedestal.json"),
    )
    .expect("fixture definition");
    definition.validate().expect("static scene schema");
    assert_eq!(definition.model_layout, ModelLayout::Scene);
    assert_eq!(
        BlockLibrary::load(&terra_core::asset_directory().join("terra/blocks"))
            .expect("shipped library")
            .blocks
            .len(),
        terra_voxel::MaterialId::BLOCK_COUNT
    );
}

#[test]
fn br52_metadata_and_the_complete_recipe_catalog_fit_the_transport_budgets() {
    use terra_core::locomotive_parts::{self, Kind};
    let library = BlockLibrary::load(&terra_core::asset_directory().join("terra/blocks"))
        .expect("complete block definitions");
    for part in locomotive_parts::DEFINITIONS {
        let definition = library
            .blocks
            .iter()
            .find(|definition| definition.id == part.id)
            .unwrap_or_else(|| panic!("missing {}", part.slug));
        assert_eq!(definition.name, part.name);
        assert_eq!(definition.render_kind, BlockRenderKind::CustomMesh);
        assert_eq!(definition.model_layout, ModelLayout::Scene);
        assert_eq!(definition.model, format!("models/{}.gltf", part.slug));
        assert!(definition.gameplay.struct_block);
        assert!(!definition.description.is_empty());
        assert_eq!(definition.creative_category.as_deref(), Some("trains"));
        assert_eq!(
            definition.collision_boxes.as_slice(),
            locomotive_parts::base_boxes(part.id)
        );
        if matches!(part.kind, Kind::Valve | Kind::Gauge | Kind::Tank) {
            assert!(!definition.description.starts_with("Decorative "));
            assert!(definition.description.to_lowercase().contains("steam"));
        }
    }
    let recipes = terra_core::crafting::RecipeBook::builtins();
    // The complete preexisting catalog plus the 76 BR52 additions must remain
    // available while leaving room inside the network's catalog limits.
    let count = recipes.iter().count();
    assert!((415..=terra_core::crafting::MAX_RECIPES).contains(&count));
    let json = recipes.to_json().expect("bounded recipe payload");
    let received = terra_core::crafting::RecipeBook::from_json(&json).unwrap();
    assert!(
        received.iter().eq(recipes.iter()),
        "recipe transport changed the catalog"
    );
}

#[test]
fn extended_kinetic_models_keep_full_rotor_bounds_and_cell_to_cell_axles() {
    for (path, id, radius) in [
        ("terra/blocks/models/wooden_gear.gltf", 141, 0.55),
        ("terra/blocks/models/wooden_large_gear.gltf", 149, 1.05),
        ("terra/blocks/models/wooden_shaft.gltf", 144, 0.132),
        ("terra/blocks/models/water_wheel.gltf", 147, 1.5),
    ] {
        let model = load_static_scene(path, id);
        let mut low = Vec3::splat(f32::INFINITY);
        let mut high = Vec3::splat(f32::NEG_INFINITY);
        for point in model.iter().flat_map(|primitive| &primitive.positions) {
            let point = Vec3::from_array(*point);
            low = low.min(point);
            high = high.max(point);
            assert!((0.0..=1.0).contains(&point.x));
            assert!(point.y >= 0.5 - radius - 0.00001 && point.y <= 0.5 + radius + 0.00001);
            assert!(point.z >= 0.5 - radius - 0.00001 && point.z <= 0.5 + radius + 0.00001);
        }
        assert!(
            low.x.abs() < 0.00001 && (high.x - 1.0).abs() < 0.00001,
            "axial connectors must meet at cell boundaries: {id}"
        );
        if id == 147 {
            assert!((high.y - low.y - 3.0).abs() < 0.00001);
            assert!((high.z - low.z - 3.0).abs() < 0.00001);
        } else if id == 149 {
            assert!(
                high.y - low.y > 2.0 && high.z - low.z > 2.0,
                "a large gear cannot be clipped or scaled into one cell"
            );
        } else if id == 141 {
            assert!((high.y - low.y - 1.1).abs() < 0.00001);
            assert!((high.z - low.z - 1.1).abs() < 0.00001);
            assert!(
                model
                    .iter()
                    .flat_map(|primitive| &primitive.positions)
                    .all(|point| { (point[1] - 0.5).hypot(point[2] - 0.5) <= 0.55001 })
            );
        }
    }
}

#[test]
fn kinetic_envelope_allowance_never_relaxes_unrelated_models_or_axial_limits() {
    for (id, radius) in [(141, 0.55), (149, 1.05), (147, 1.5)] {
        let primitive = PrimitiveTemplate {
            positions: vec![
                [0.0, 0.5 - radius, 0.5 - radius],
                [1.0, 0.5 + radius, 0.5 - radius],
                [1.0, 0.5 + radius, 0.5 + radius],
            ],
            normals: vec![[1.0, 0.0, 0.0]; 3],
            tangents: Vec::new(),
            uvs: vec![[0.0; 2]; 3],
            indices: vec![0, 1, 2],
            material: 0,
        };
        assert!(bake_transform(id, &mut primitive.clone(), bevy::math::Affine3A::IDENTITY).is_ok());
        let mut ordinary_overhang = primitive.clone();
        ordinary_overhang.positions[0][1] = -0.001;
        assert!(
            bake_transform(
                65000,
                &mut ordinary_overhang,
                bevy::math::Affine3A::IDENTITY
            )
            .is_err()
        );
        for axis in 0..3 {
            for sign in [-1.0, 1.0] {
                let offset = Vec3::from_array(std::array::from_fn(|a| {
                    if a == axis { sign * 0.001 } else { 0.0 }
                }));
                assert!(
                    bake_transform(
                        id,
                        &mut primitive.clone(),
                        bevy::math::Affine3A::from_translation(offset)
                    )
                    .is_err()
                );
            }
        }
    }
}

#[test]
fn raised_conveyor_preserves_cell_centered_ports_and_matches_authoritative_top() {
    let model = load_static_scene("terra/blocks/models/conveyor.gltf", 148);
    let positions: Vec<_> = model
        .iter()
        .flat_map(|primitive| &primitive.positions)
        .map(|p| Vec3::from_array(*p))
        .collect();
    let top = positions.iter().map(|p| p.y).fold(0.0_f32, f32::max);
    assert!((top - terra_core::kinetic::CONVEYOR_TOP).abs() < 1e-6);
    assert!(
        positions.iter().all(|point| point.x > 0.0 && point.x < 1.0),
        "a real shaft, not a capped dummy axle, owns both cell boundaries"
    );
    for end in [0.125, 0.18, 0.82, 0.875] {
        let port: Vec<_> = positions
            .iter()
            .filter(|point| {
                (point.x - end).abs() < 1e-6
                    && (0.1109..=0.1251).contains(&(point.y - 0.5).hypot(point.z - 0.5))
            })
            .collect();
        assert!(
            !port.is_empty(),
            "the open bearing must remain centered behind the real shaft collar at {end}"
        );
        for axis in [1, 2] {
            let low = port.iter().map(|p| p[axis]).fold(f32::INFINITY, f32::min);
            let high = port
                .iter()
                .map(|p| p[axis])
                .fold(f32::NEG_INFINITY, f32::max);
            let center = if axis == 1 {
                terra_core::kinetic::CONVEYOR_AXIS_HEIGHT
            } else {
                0.5
            };
            assert!(((low + high) * 0.5 - center).abs() < 1e-6);
            assert!((high - low - 0.25).abs() < 1e-6);
        }
    }
    let definition: super::super::definitions::BlockDefinition = serde_json::from_str(
        include_str!("../../../../assets/terra/blocks/conveyor.json"),
    )
    .unwrap();
    let collision_top = definition
        .collision_boxes
        .iter()
        .map(|shape| shape.maximum[1])
        .fold(0.0_f64, f64::max);
    assert_eq!(collision_top, f64::from(terra_core::kinetic::CONVEYOR_TOP));
}

#[test]
fn non_rotary_extended_conveyor_metadata_does_not_relax_authored_unit_bounds() {
    let id = terra_core::kinetic::CONVEYOR;
    assert!(terra_core::kinetic::is_extended(id));
    assert_eq!(terra_core::kinetic::radial_extent(id), None);
    let primitive = PrimitiveTemplate {
        positions: vec![[0.0, 0.0, 0.0], [1.0, 0.625, 0.0], [1.0, 0.625, 1.0]],
        normals: vec![[0.0, 1.0, 0.0]; 3],
        tangents: Vec::new(),
        uvs: vec![[0.0; 2]; 3],
        indices: vec![0, 1, 2],
        material: 0,
    };
    assert!(bake_transform(id, &mut primitive.clone(), bevy::math::Affine3A::IDENTITY).is_ok());
    for axis in 0..3 {
        let mut overhang = primitive.clone();
        overhang.positions[0][axis] = -0.001;
        assert!(bake_transform(id, &mut overhang, bevy::math::Affine3A::IDENTITY).is_err());
    }
}

#[test]
fn locomotive_saddles_have_a_bounded_lower_skin_allowance_without_relaxing_other_parts() {
    let primitive = PrimitiveTemplate {
        positions: vec![[0.12, -0.14, 0.12], [0.88, 0.12, 0.12], [0.88, 0.12, 0.88]],
        normals: vec![[0.0, 0.0, 1.0]; 3],
        tangents: vec![],
        uvs: vec![[0.0; 2]; 3],
        indices: vec![0, 1, 2],
        material: 0,
    };
    for id in 224..=226 {
        assert!(bake_transform(id, &mut primitive.clone(), bevy::math::Affine3A::IDENTITY).is_ok());
        let mut beyond = primitive.clone();
        beyond.positions[0][1] = -0.15;
        assert!(bake_transform(id, &mut beyond, bevy::math::Affine3A::IDENTITY).is_err());
    }
    assert!(bake_transform(245, &mut primitive.clone(), bevy::math::Affine3A::IDENTITY).is_err());
}

#[test]
fn rail_builder_scoop_accepts_three_metre_width_but_keeps_height_and_other_blocks_bounded() {
    let primitive = PrimitiveTemplate {
        positions: vec![[-1.0, 0.0, 0.0], [2.0, 1.0, 0.0], [2.0, 1.0, 1.0]],
        normals: vec![[0.0, 0.0, 1.0]; 3],
        tangents: vec![],
        uvs: vec![[0.0; 2]; 3],
        indices: vec![0, 1, 2],
        material: 0,
    };
    let id = terra_core::train_parts::RAIL_BUILDER;
    assert!(bake_transform(id, &mut primitive.clone(), bevy::math::Affine3A::IDENTITY).is_ok());
    for (axis, value) in [(0, -1.01), (0, 2.01), (1, -0.01), (1, 1.01)] {
        let mut beyond = primitive.clone();
        beyond.positions[0][axis] = value;
        assert!(bake_transform(id, &mut beyond, bevy::math::Affine3A::IDENTITY).is_err());
    }
    assert!(bake_transform(1, &mut primitive.clone(), bevy::math::Affine3A::IDENTITY).is_err());
}

#[test]
fn cutout_leaf_materials_keep_alpha_and_reverse_sides_in_base_and_preview_paths() {
    let mut assets = BlockAssets::default();
    let mut materials = Assets::<StandardMaterial>::default();
    let definition = CubeMaterialDefinition::default();
    for texture in [
        "textures/foliage/oak_side.png",
        "textures/foliage/cherry_pink_top.png",
        "textures/stone.png",
    ] {
        let slot = assets.shared_material(&definition, Some(texture), &mut materials);
        let material = materials.get(&assets.materials[slot]).unwrap();
        if authored_foliage_texture(texture) {
            assert_eq!(material.alpha_mode, AlphaMode::Mask(0.5));
            assert!(material.double_sided);
            assert_eq!(material.cull_mode, None);
        } else {
            assert_eq!(material.alpha_mode, AlphaMode::Opaque);
        }
    }
    assert!(!authored_foliage_texture("model:foliage/0"));
    assert!(!authored_foliage_texture("textures/foliage/README.md"));
}

#[test]
fn palette_normal_maps_are_bound_without_aliasing_distinct_shared_materials() {
    let mut assets = BlockAssets::default();
    let mut materials = Assets::<StandardMaterial>::default();
    let mut images = Assets::<Image>::default();
    let first_map = images.add(Image::default());
    let second_map = images.add(Image::default());
    assets
        .textures
        .insert("textures/palette-a.png".into(), first_map.clone());
    assets
        .textures
        .insert("textures/palette-b.png".into(), second_map.clone());
    let mut definition = CubeMaterialDefinition {
        normal_texture: Some("textures/palette-a.png".into()),
        ..default()
    };
    let first = assets.shared_material(&definition, None, &mut materials);
    assert_eq!(
        materials
            .get(&assets.materials[first])
            .unwrap()
            .normal_map_texture,
        Some(first_map)
    );
    definition.normal_texture = Some("textures/palette-b.png".into());
    let second = assets.shared_material(&definition, None, &mut materials);
    assert_ne!(first, second);
    assert_eq!(
        materials
            .get(&assets.materials[second])
            .unwrap()
            .normal_map_texture,
        Some(second_map)
    );
}
