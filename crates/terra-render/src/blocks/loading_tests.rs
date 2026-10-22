use super::*;

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
    assert_eq!(assets.lights().count(), 3);
    assert_eq!(
        assets.lights().find(|(id, _)| *id == 56).unwrap().1.range,
        4.0
    );
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
fn prop_palette_preserves_linear_color_and_restrains_highlights() {
    let mut assets = BlockAssets::default();
    let mut materials = Assets::<StandardMaterial>::default();
    let palette = load_prop_materials(&mut assets, &mut materials);
    for (slot, previous, factor) in [
        (palette.foliage[0], [0.16, 0.32, 0.075], 0.80),
        (palette.foliage[1], [0.28, 0.46, 0.12], 0.80),
        (palette.foliage[2], [0.40, 0.57, 0.20], 0.75),
        (palette.stem, [0.18, 0.29, 0.075], 0.80),
    ] {
        let material = materials
            .get(&assets.materials[slot])
            .expect("prop material");
        let color = material.base_color.to_linear();
        for (channel, previous) in [color.red, color.green, color.blue]
            .into_iter()
            .zip(previous)
        {
            assert!((channel - previous * factor).abs() < 0.000001);
        }
        assert_eq!(color.alpha, 1.0);
        assert!(material.base_color_texture.is_none());
    }
    assert_eq!(palette.flowers.len(), 6);
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
            ..plain
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
            ..plain
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
    assert_eq!(
        slots[&(1, "Root-rich earth")],
        slots[&(2, "Root-rich earth")]
    );
    assert_ne!(
        slots[&(3, "Weathered blue stone")],
        slots[&(5, "Soft snow")]
    );
    assert_eq!(materials.len(), 50 + terra_core::OreKind::ALL.len());
    assert_ne!(slots[&(57, "Mineral rock")], slots[&(58, "Mineral rock")]);
    assert_ne!(slots[&(57, "Mineral rock")], slots[&(30, "Host rock")]);
    assert_ne!(slots[&(49, "Sea ice")], slots[&(5, "Soft snow")]);
    assert_ne!(slots[&(38, "Salt body")], slots[&(30, "Host rock")]);
    assert_eq!(slots[&(37, "Dripstone")], slots[&(39, "Dripstone")]);
    for ore in terra_core::OreKind::ALL {
        assert_eq!(
            slots[&(ore.material(), "Host rock")],
            slots[&(3, "Weathered blue stone")]
        );
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
    let grass = library
        .blocks
        .iter_mut()
        .find(|block| block.id == 1)
        .unwrap();
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

fn load_static_scene(path: &'static str, id: u16) -> Vec<PrimitiveTemplate> {
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
        assert!(
            !matches!(
                server.get_load_state(handle.id()),
                Some(LoadState::Failed(_))
            ),
            "fixture load failed: {:?}",
            server.get_load_state(handle.id())
        );
        if server.is_loaded_with_dependencies(handle.id()) {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "fixture loading timed out"
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
    ] {
        let model = load_static_scene(path, id);
        let bottom = model
            .iter()
            .flat_map(|p| &p.positions)
            .map(|p| p[1])
            .fold(f32::INFINITY, f32::min);
        if matches!(id, 28 | 29 | 59 | 65..=73) {
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
        positions: vec![[0.0; 3], [1.0, 2.0, 0.0], [1.0, 2.0, 1.0]],
        normals: vec![[1.0, 0.0, 0.0]; 3],
        uvs: vec![[0.0; 2]; 3],
        indices: vec![0, 1, 2],
        material: 0,
    };
    for id in [59, 65, 66, 67] {
        assert!(bake_transform(id, &mut tall.clone(), bevy::math::Affine3A::IDENTITY).is_ok());
    }
    for id in [
        28, 29, 40, 60, 61, 62, 63, 64, 68, 69, 70, 71, 72, 73, 65000,
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
fn only_beds_extend_into_the_paired_horizontal_cell() {
    let long = PrimitiveTemplate {
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
            conditions: Default::default(),
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
