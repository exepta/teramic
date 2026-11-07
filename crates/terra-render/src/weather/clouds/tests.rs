use super::*;

#[test]
fn celestial_background_sorts_before_clouds_at_short_view_distances_and_all_layer_views() {
    use bevy::{
        core_pipeline::core_3d::TransparentSortingInfo3d, render::render_phase::ViewRangefinder3d,
    };
    let background_bias = crate::scene::horizon::BACKGROUND_SORT_BIAS;
    for far in [32.0, 64.0, 272.0, 1024.0] {
        for height in [-128.0, 0.0, 145.0, 256.0, 400.0, 100_000.0] {
            for direction in [
                Vec3::new(0.0, 0.9, 0.4358899),
                Vec3::new(0.7, 0.03, -1.0),
                Vec3::new(-0.3, -0.8, -1.0),
            ] {
                let eye = Vec3::new(1150.5, height, 480.5);
                let camera = Transform::from_translation(eye).looking_to(direction, Vec3::Y);
                let rangefinder = ViewRangefinder3d::from_world_from_view(&camera.compute_affine());
                let cloud_center = Vec3::new(eye.x, (MIN_HEIGHT + MAX_HEIGHT) * 0.5, eye.z);
                let key = |mesh_center, depth_bias| {
                    TransparentSortingInfo3d::Sorted {
                        mesh_center,
                        depth_bias,
                    }
                    .sort_distance(&rangefinder)
                };
                let background = key(eye, background_bias);
                let clouds = key(cloud_center, -far);
                let local = key(eye + direction.normalize() * 5.0, 0.0);
                assert!(background.is_finite() && clouds.is_finite() && local.is_finite());
                assert!(
                    background < clouds && background < local,
                    "Celestial background sorted in front: far={far}, height={height}, direction={direction:?}, background={background}, clouds={clouds}"
                );
                if far == 64.0 && height == 145.0 && direction.y > 0.8 {
                    assert!(
                        clouds < key(eye, -2.0 * far),
                        "This fixture must retain the original short-view-distance ordering regression"
                    );
                }
            }
        }
    }
}

fn load_section(world: &mut World) {
    let position = terra_voxel::SectionPos::new(0, 0, 0);
    world.resource_mut::<ChunkStore>().loaded.insert(
        position,
        terra_world::streaming::LoadedSection {
            data: terra_voxel::BlockSection::empty(position).unwrap(),
            mesh: None,
            mesh_epoch: 0,
            water_epoch: 0,
            water: None,
            props: Vec::new(),
        },
    );
}

#[test]
fn movement_quality_weather_and_world_reload_reuse_one_mesh_material_and_texture() {
    let mut app = App::new();
    app.init_resource::<Time>()
        .init_resource::<GameConfig>()
        .init_resource::<WeatherVisual>()
        .init_resource::<crate::scene::daylight::Daylight>()
        .init_resource::<crate::UnderwaterState>()
        .init_resource::<ChunkStore>()
        .init_resource::<Sky>()
        .init_resource::<WeatherRenderStatus>()
        .init_resource::<Assets<Mesh>>()
        .init_resource::<Assets<CloudMaterial>>()
        .init_resource::<Assets<Image>>()
        .add_systems(Update, update);
    let camera = app
        .world_mut()
        .spawn((
            TerraCamera,
            Transform::from_xyz(0.0, 60.0, 0.0),
            Projection::Perspective(PerspectiveProjection {
                far: 2_000.0,
                ..default()
            }),
        ))
        .id();
    load_section(app.world_mut());
    app.world_mut()
        .resource_mut::<GameConfig>()
        .client
        .weather_quality = WeatherQuality::Off;
    app.update();
    assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 0);
    app.world_mut()
        .resource_mut::<GameConfig>()
        .client
        .weather_quality = WeatherQuality::Low;
    app.update();
    let entity = app.world().resource::<Sky>().entity.unwrap();
    let mesh = app.world().resource::<Sky>().mesh.as_ref().unwrap().id();
    let material = app
        .world()
        .resource::<Sky>()
        .material
        .as_ref()
        .unwrap()
        .id();
    let texture = app
        .world()
        .resource::<Assets<CloudMaterial>>()
        .get(material)
        .unwrap()
        .extension
        .noise
        .id();
    for (index, quality) in [
        WeatherQuality::Medium,
        WeatherQuality::High,
        WeatherQuality::Ultra,
        WeatherQuality::Off,
        WeatherQuality::Low,
    ]
    .into_iter()
    .enumerate()
    {
        app.world_mut()
            .resource_mut::<GameConfig>()
            .client
            .weather_quality = quality;
        let observer = Vec3::new(
            index as f32 * 20_000.0,
            [60.0, 255.0, 400.0][index % 3],
            -1_900.0,
        );
        app.world_mut()
            .get_mut::<Transform>(camera)
            .unwrap()
            .translation = observer;
        *app.world_mut().resource_mut::<WeatherVisual>() = WeatherVisual {
            clouds: 0.30 + index as f32 * 0.15,
            rain: if index == 2 { 0.9 } else { 0.0 },
            snow: if index == 4 { 0.8 } else { 0.0 },
            wind: Vec2::new(-8.0, 4.0),
            flash: 0.7,
            ..default()
        };
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs(1));
        app.update();
        let sky = app.world().resource::<Sky>();
        assert_eq!(sky.entity, Some(entity));
        assert_eq!(sky.mesh.as_ref().unwrap().id(), mesh);
        assert_eq!(sky.material.as_ref().unwrap().id(), material);
        assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 1);
        assert_eq!(app.world().resource::<Assets<CloudMaterial>>().len(), 1);
        assert_eq!(app.world().resource::<Assets<Image>>().len(), 1);
        let cloud = app
            .world()
            .resource::<Assets<CloudMaterial>>()
            .get(material)
            .unwrap();
        assert_eq!(cloud.extension.noise.id(), texture);
        assert_eq!(cloud.base.alpha_mode, AlphaMode::Blend);
        assert_eq!(cloud.base.cull_mode, Some(Face::Front));
        assert!(!cloud.base.fog_enabled);
        let status = app.world().resource::<WeatherRenderStatus>();
        assert_eq!(status.cloud_uploads, 1);
        assert_eq!(
            (status.cloud_shaft_batches, status.cloud_shaft_vertices),
            (0, 0)
        );
        if quality == WeatherQuality::Off {
            assert_eq!(
                app.world().get::<Visibility>(entity),
                Some(&Visibility::Hidden)
            );
            assert_eq!((status.cloud_faces, status.cloud_batches), (0, 0));
        } else {
            assert_eq!(cloud.extension.observer.truncate(), observer);
            assert_eq!(cloud.extension.volume.z, sample_count(quality) as f32);
            assert_eq!(cloud.extension.weather.w, 0.7);
            assert_eq!(cloud.base.depth_bias, -2_000.0);
            assert_eq!((status.cloud_faces, status.cloud_batches), (6, 1));
        }
    }
    app.world_mut().resource_mut::<ChunkStore>().loaded.clear();
    app.update();
    assert_eq!(
        app.world().get::<Visibility>(entity),
        Some(&Visibility::Hidden)
    );
    load_section(app.world_mut());
    app.update();
    assert_eq!(
        app.world().get::<Visibility>(entity),
        Some(&Visibility::Inherited)
    );
    assert_eq!(
        app.world().resource::<WeatherRenderStatus>().cloud_uploads,
        1
    );
    assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 1);
    assert_eq!(app.world().resource::<Assets<CloudMaterial>>().len(), 1);
    assert_eq!(app.world().resource::<Assets<Image>>().len(), 1);
}

#[test]
fn proxy_is_a_fixed_bounded_ray_carrier_without_per_cloud_geometry() {
    let mesh = proxy_mesh();
    assert_eq!(mesh.count_vertices(), 24);
    assert_eq!(mesh.indices().unwrap().len(), 36);
    assert!(mesh.indices().unwrap().iter().all(|index| index < 24));
    let Some(bevy::mesh::VertexAttributeValues::Float32x3(positions)) =
        mesh.attribute(Mesh::ATTRIBUTE_POSITION)
    else {
        panic!("Cloud proxy positions");
    };
    for p in positions {
        let height = p[1] + (MIN_HEIGHT + MAX_HEIGHT) * 0.5;
        assert!((MIN_HEIGHT..=MAX_HEIGHT).contains(&height));
        assert!(p[0].abs() <= EXTENT && p[2].abs() <= EXTENT);
    }
    assert!(!CloudExtension::enable_prepass());
    assert!(!CloudExtension::enable_shadows());
}

#[test]
fn shader_variants_keep_the_volume_bindings_and_use_terrain_and_first_hit_depth() {
    use wesl::{
        pass::{Features, condcomp},
        syntax::TranslationUnit,
    };
    let shader = include_str!("../../../../../assets/shaders/terra_clouds.wesl");
    for prepass in [false, true] {
        for multisampled in [false, true] {
            let mut module = shader.parse::<TranslationUnit>().unwrap();
            let mut features = Features::new();
            features.set("DEPTH_PREPASS", prepass);
            features.set("MULTISAMPLED", multisampled);
            condcomp(&mut module, &features).unwrap();
            let source = module.to_string();
            for binding in [100, 108, 109] {
                assert!(source.contains(&format!("@binding({binding})")));
            }
            assert!(source.contains("texture_3d<f32>"));
            assert!(source.contains("frag_depth"));
            assert!(source.contains("first_hit"));
            assert!(source.contains("position_world_to_clip"));
            assert_eq!(source.contains("prepass_depth(input.position"), prepass);
            assert_eq!(
                source.contains("dot(terrain - cloud.observer.xyz, ray)"),
                prepass
            );
        }
    }
    assert!(shader.contains("min(u32(cloud.volume.z), 28u)"));
    assert!(!shader.contains("sun.w"));
    assert!(!shader.contains("shaft"));
    assert!(!shader.contains("input.color"));
}

#[test]
fn wind_wrap_preserves_both_periodic_noise_domains() {
    for scale in [2_048.0, 8_192.0] {
        assert_eq!((WIND_PERIOD / scale).fract(), 0.0);
    }
    assert_eq!(sample_count(WeatherQuality::Off), 0);
    for quality in [
        WeatherQuality::Low,
        WeatherQuality::Medium,
        WeatherQuality::High,
        WeatherQuality::Ultra,
    ] {
        assert!((1..=28).contains(&sample_count(quality)));
    }
}
