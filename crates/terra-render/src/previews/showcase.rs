//! Isolated native capture of the same cached meshes used by inventory and drops.
use super::*;
use bevy::{app::AppExit, camera::CameraProjection, render::view::screenshot::save_to_disk};
use terra_core::{ToolKind, creatures::AnimalProduct as Product};

#[derive(Resource)]
struct Studio {
    furnaces: bool,
    construction: bool,
    material_reference: bool,
    initialized: bool,
    stage: usize,
    since: Instant,
    started: Instant,
    pending: bool,
    captured: bool,
    models: Vec<Entity>,
    smoke: Option<Entity>,
}

#[derive(Component)]
struct StudioCamera;

const ITEMS: [ItemKind; 24] = [
    ItemKind::Animal(Product::Beef),
    ItemKind::Animal(Product::CookedBeef),
    ItemKind::Animal(Product::Pork),
    ItemKind::Animal(Product::CookedPork),
    ItemKind::Animal(Product::Leather),
    ItemKind::Animal(Product::Wool),
    ItemKind::Animal(Product::Mutton),
    ItemKind::Animal(Product::CookedMutton),
    ItemKind::Animal(Product::Honey),
    ItemKind::Animal(Product::Fish),
    ItemKind::Animal(Product::CookedFish),
    ItemKind::Animal(Product::Clownfish),
    ItemKind::Animal(Product::CookedClownfish),
    ItemKind::Tool(ToolKind::WoodPickaxe),
    ItemKind::Tool(ToolKind::StonePickaxe),
    ItemKind::Tool(ToolKind::WoodAxe),
    ItemKind::Tool(ToolKind::WoodShovel),
    ItemKind::Tool(ToolKind::WoodSword),
    ItemKind::Tool(ToolKind::StoneAxe),
    ItemKind::Tool(ToolKind::StoneShovel),
    ItemKind::Tool(ToolKind::StoneSword),
    ItemKind::Block(40),
    ItemKind::Mineral(terra_core::MineralItem::IronIngot),
    ItemKind::Mineral(terra_core::MineralItem::GoldIngot),
];

const FURNACES: [ItemKind; 16] = [
    ItemKind::Block(59),
    ItemKind::Block(60),
    ItemKind::Block(61),
    ItemKind::Block(62),
    ItemKind::Block(63),
    ItemKind::Block(64),
    ItemKind::Bellows(terra_core::BellowsKind::Stone),
    ItemKind::Bellows(terra_core::BellowsKind::Iron),
    ItemKind::Bellows(terra_core::BellowsKind::Gold),
    ItemKind::Bellows(terra_core::BellowsKind::Ruby),
    ItemKind::Block(65),
    ItemKind::Block(66),
    ItemKind::Block(67),
    ItemKind::Block(68),
    ItemKind::Block(69),
    ItemKind::Block(70),
];

pub(super) fn install(app: &mut App) {
    app.insert_resource(Studio {
        furnaces: std::env::args().any(|a| a == "--smoke-test-furnaces"),
        construction: std::env::args().any(|a| a == "--smoke-test-construction"),
        material_reference: std::env::args().any(|a| a == "--smoke-test-material-reference"),
        initialized: false,
        stage: 0,
        since: Instant::now(),
        started: Instant::now(),
        pending: false,
        captured: false,
        models: Vec::new(),
        smoke: None,
    })
    .add_systems(Update, (prepare, capture).chain());
}

fn prepare(
    mut commands: Commands,
    blocks: Res<BlockAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut state: ResMut<Studio>,
    mut game_cameras: Query<&mut Camera, With<terra_player::TerraCamera>>,
) {
    if state.initialized {
        return;
    }
    let Some(templates) = &blocks.templates else {
        return;
    };
    // Bevy shares prepass depth by render target, not sample count. The isolated
    // studio must not render onto the game camera's window with different MSAA.
    for mut camera in &mut game_cameras {
        camera.is_active = false;
    }
    let construction: Vec<_> = (74..=97).chain(71..=73).map(ItemKind::Block).collect();
    let reference = material_reference_items();
    let items: &[ItemKind] = if state.material_reference {
        &reference
    } else if state.construction {
        &construction
    } else if state.furnaces {
        &FURNACES
    } else {
        &ITEMS
    };
    for (index, &item) in items.iter().enumerate() {
        let batches = match item {
            ItemKind::Animal(kind) => animal_batches(kind, templates),
            ItemKind::Mineral(kind) => mineral_batches(kind, templates),
            ItemKind::Bellows(kind) => bellows_batches(kind, templates),
            ItemKind::Dye(color) => dye_batches(color, templates),
            ItemKind::Block(material) => block_batches(
                material,
                templates,
                &RenderOptions {
                    water_level: None,
                    detail_level: DetailLevel::Near,
                    terrain: None,
                    exposure: None,
                    tints: Default::default(),
                    conditions: default(),
                },
            ),
            _ => tool_batches(item, templates),
        };
        let batches = match batches {
            Ok(batches) => batches,
            Err(error) => {
                error!(%error, ?item);
                return;
            }
        };
        let vertices: usize = batches.iter().map(|b| b.positions.len()).sum();
        let triangles: usize = batches.iter().map(|b| b.indices.len() / 3).sum();
        info!(
            ?item,
            vertices,
            triangles,
            batches = batches.len(),
            "Item reference geometry"
        );
        let root = commands
            .spawn((
                PreviewScene,
                Transform::from_translation(if state.material_reference {
                    reference_location(index, 0).unwrap_or(Vec3::ZERO)
                } else if state.construction {
                    location(index, 8, 4)
                } else if state.furnaces {
                    location(index, 4, 4)
                } else {
                    location(index, 8, 3)
                })
                .with_rotation(if state.construction {
                    Quat::from_rotation_y(std::f32::consts::PI)
                } else {
                    Quat::IDENTITY
                })
                .with_scale(Vec3::splat(if state.material_reference {
                    tools::world_length(item).unwrap_or(1.0)
                } else {
                    1.0
                })),
                if !state.material_reference || reference_location(index, 0).is_some() {
                    Visibility::Visible
                } else {
                    Visibility::Hidden
                },
            ))
            .id();
        for batch in batches {
            let Some(material) = blocks.materials.get(batch.material) else {
                continue;
            };
            commands.spawn((
                Mesh3d(meshes.add(batch_mesh(&batch))),
                MeshMaterial3d(material.clone()),
                Transform::from_translation(-Vec3::splat(0.5)),
                ChildOf(root),
                RenderLayers::layer(PREVIEW_LAYER),
            ));
        }
        state.models.push(root);
    }
    commands.spawn((
        PreviewScene,
        StudioCamera,
        Camera3d::default(),
        Camera {
            order: 100,
            clear_color: ClearColorConfig::Custom(Color::srgb(0.055, 0.065, 0.070)),
            ..default()
        },
        if state.material_reference {
            reference_camera()
        } else {
            Transform::from_xyz(8.0, 6.0, 11.0).looking_at(Vec3::ZERO, Vec3::Y)
        },
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: bevy::camera::ScalingMode::FixedVertical {
                viewport_height: if state.material_reference { 7.0 } else { 6.0 },
            },
            near: 0.01,
            far: 100.0,
            ..OrthographicProjection::default_3d()
        }),
        AmbientLight {
            brightness: 700.0,
            color: Color::srgb(0.93, 0.96, 1.0),
            ..default()
        },
        Exposure { ev100: 13.7 },
        Hdr,
        Tonemapping::AcesFitted,
        Msaa::Off,
        bevy::core_pipeline::prepass::DepthPrepass,
        crate::scene::caves::shader::CaveSettings::default(),
        RenderLayers::layer(PREVIEW_LAYER),
    ));
    commands.spawn((
        PreviewScene,
        DirectionalLight {
            color: Color::srgb(1.0, 0.96, 0.88),
            illuminance: 100_000.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_xyz(-4.0, 8.0, 6.0).looking_at(Vec3::ZERO, Vec3::Y),
        RenderLayers::layer(PREVIEW_LAYER),
    ));
    commands.spawn((
        PreviewScene,
        DirectionalLight {
            color: Color::srgb(0.90, 0.94, 1.0),
            illuminance: 42_000.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_xyz(8.0, 3.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y),
        RenderLayers::layer(PREVIEW_LAYER),
    ));
    state.initialized = true;
    state.since = Instant::now();
}

fn location(index: usize, columns: usize, rows: usize) -> Vec3 {
    let camera = Transform::from_xyz(8.0, 6.0, 11.0).looking_at(Vec3::ZERO, Vec3::Y);
    *camera.right() * (index as f32 % columns as f32 - (columns - 1) as f32 * 0.5) * 1.15
        + *camera.up() * ((rows - 1) as f32 * 0.5 - (index / columns) as f32) * 1.3
}

fn material_reference_items() -> Vec<ItemKind> {
    use ToolKind::*;
    [
        WoodSword,
        StoneSword,
        IronSword,
        GoldSword,
        DiamondSword,
        RubySword,
        WoodPickaxe,
        StonePickaxe,
        IronPickaxe,
        GoldPickaxe,
        DiamondPickaxe,
        RubyPickaxe,
        WoodAxe,
        StoneAxe,
        IronAxe,
        GoldAxe,
        DiamondAxe,
        RubyAxe,
    ]
    .into_iter()
    .map(ItemKind::Tool)
    .chain(terra_core::beds::DyeColor::ALL.map(ItemKind::Dye))
    .chain([1, 2, 3, 4, 5, 7, 8, 9].map(ItemKind::Block))
    .collect()
}

fn reference_location(index: usize, stage: usize) -> Option<Vec3> {
    let position = |slot: usize, columns: usize, rows: usize, spacing: f32| {
        let camera = reference_camera();
        *camera.right() * (slot % columns) as f32 * spacing
            + *camera.up() * ((rows - 1) as f32 * 0.5 - (slot / columns) as f32) * spacing
            - *camera.right() * (columns - 1) as f32 * 0.5 * spacing
    };
    match stage {
        0 if index < 18 => Some(position(index, 6, 3, 1.9)),
        1 if (18..31).contains(&index) => Some(position(index - 18, 7, 2, 1.15)),
        2 if (31..39).contains(&index) => Some(position(index - 31, 4, 2, 1.45)),
        3 => [2, 8, 14, 17]
            .iter()
            .position(|i| *i == index)
            .map(|slot| position(slot, 2, 2, 1.9)),
        _ => None,
    }
}

fn reference_camera() -> Transform {
    Transform::from_xyz(2.0, 2.0, 14.0).looking_at(Vec3::ZERO, Vec3::Y)
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn capture(
    mut commands: Commands,
    mut state: ResMut<Studio>,
    mut transforms: Query<(&mut Transform, &mut Visibility), Without<StudioCamera>>,
    mut camera: Query<
        (
            &mut Projection,
            &mut Transform,
            &mut crate::scene::caves::shader::CaveSettings,
        ),
        With<StudioCamera>,
    >,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut smoke_materials: ResMut<Assets<crate::furnaces::SmokeMaterial>>,
    blocks: Res<BlockAssets>,
    room_meshes: Query<Entity, (With<PreviewScene>, With<Mesh3d>)>,
    mut exit: MessageWriter<AppExit>,
) {
    if state.started.elapsed() > Duration::from_secs(120) {
        error!("Item reference fixture timed out");
        exit.write(AppExit::error());
        return;
    }
    if !state.initialized {
        return;
    }
    if state.captured {
        state.stage += 1;
        state.captured = false;
        state.pending = false;
        if state.stage
            >= if state.material_reference {
                4
            } else if state.construction {
                5
            } else if state.furnaces {
                14
            } else {
                6
            }
        {
            info!("Native item reference fixture passed");
            exit.write(AppExit::Success);
            return;
        }
        if state.material_reference {
            for (index, entity) in state.models.iter().enumerate() {
                if let Ok((mut transform, mut visibility)) = transforms.get_mut(*entity) {
                    *visibility = if let Some(position) = reference_location(index, state.stage) {
                        transform.translation = position;
                        transform.rotation = if state.stage == 3 {
                            Quat::from_euler(EulerRot::XYZ, 0.16, -0.60, -0.18)
                        } else {
                            Quat::IDENTITY
                        };
                        Visibility::Visible
                    } else {
                        Visibility::Hidden
                    };
                }
            }
            if let Ok((mut projection, _, _)) = camera.single_mut()
                && let Projection::Orthographic(p) = &mut *projection
            {
                p.scaling_mode = bevy::camera::ScalingMode::FixedVertical {
                    viewport_height: if state.stage == 3 { 4.6 } else { 5.5 },
                };
            }
            state.since = Instant::now();
        } else {
            let selected: &[usize] = if state.construction {
                match state.stage {
                    1 => &[0, 1, 2, 3],
                    2 => &[8, 9, 10, 11],
                    3 => &[16, 17, 18, 19],
                    _ => &[24, 25, 26],
                }
            } else if state.furnaces {
                match state.stage {
                    1 => &[0, 1, 2],
                    2 => &[3, 4, 5],
                    3 => &[6, 7, 8, 9],
                    4 => &[10, 11, 12],
                    5 => &[13, 14, 15],
                    9 => &[0, 2, 3],
                    10 | 11 => &[],
                    12 => &[10, 11, 12],
                    13 => &[],
                    _ => &[0],
                }
            } else {
                match state.stage {
                    1 => &[0, 1, 6, 8],
                    2 => &[9, 10, 11, 12],
                    3 => &[13, 14, 15, 16],
                    4 => &[17, 18, 19, 20],
                    _ => &[21, 22, 23],
                }
            };
            for (index, entity) in state.models.iter().enumerate() {
                if let Ok((mut transform, mut visibility)) = transforms.get_mut(*entity) {
                    if let Some(slot) = selected.iter().position(|i| *i == index) {
                        *visibility = Visibility::Visible;
                        transform.translation = if state.stage == 12 {
                            transform.rotation = Quat::from_rotation_y(std::f32::consts::PI);
                            location(slot, 2, 2)
                        } else if state.stage >= 6 {
                            match index {
                                0 => Vec3::new(-1.5, 0.5, 0.0),
                                2 => Vec3::new(-1.5, 2.5, 0.0),
                                _ => Vec3::new(-1.5, 2.5, 1.0),
                            }
                        } else {
                            location(slot, 2, 2)
                        };
                    } else {
                        *visibility = Visibility::Hidden;
                    }
                }
            }
            if let Ok((mut projection, mut transform, mut optics)) = camera.single_mut() {
                if state.furnaces && state.stage >= 6 {
                    *projection = Projection::Perspective(PerspectiveProjection {
                        fov: 65.0_f32.to_radians(),
                        near: 0.05,
                        far: 80.0,
                        ..default()
                    });
                    *transform = Transform::from_xyz(2.4, 1.8, 2.8)
                        .looking_at(Vec3::new(-0.7, 1.5, -1.0), Vec3::Y);
                    if let Projection::Perspective(p) = &*projection {
                        optics.view_from_clip = p.get_clip_from_view().inverse();
                    }
                    optics.smoke = Vec4::new(
                        match state.stage {
                            7 => 0.22,
                            8 => 0.85,
                            _ => 0.0,
                        },
                        0.0,
                        0.0,
                        0.0,
                    );
                    if let Some(smoke) = state.smoke.take() {
                        commands.entity(smoke).despawn();
                    }
                    if state.stage > 6 && state.stage < 10 {
                        state.smoke = Some(crate::furnaces::showcase_wisps(
                            &mut commands,
                            &mut meshes,
                            &mut smoke_materials,
                            &transform,
                            state.stage == 9,
                            match state.stage {
                                7 => 16,
                                8 => 96,
                                _ => 0,
                            },
                            PREVIEW_LAYER,
                        ));
                    }
                    if matches!(state.stage, 10 | 11 | 13) {
                        for entity in &room_meshes {
                            commands.entity(entity).despawn();
                        }
                        *transform =
                            Transform::from_xyz(2.8, 4.6, 3.3).looking_at(Vec3::ZERO, Vec3::Y);
                        if let Err(error) =
                            fitted_roof_fixture(&mut commands, &mut meshes, &blocks, state.stage)
                        {
                            error!(%error, "Fitted roof fixture failed");
                            exit.write(AppExit::error());
                            return;
                        }
                    }
                    if state.stage == 12 {
                        for entity in &room_meshes {
                            commands.entity(entity).despawn();
                        }
                        *transform =
                            Transform::from_xyz(8.0, 6.0, 11.0).looking_at(Vec3::ZERO, Vec3::Y);
                        *projection = Projection::Orthographic(OrthographicProjection {
                            scaling_mode: bevy::camera::ScalingMode::FixedVertical {
                                viewport_height: 2.8,
                            },
                            near: 0.01,
                            far: 100.0,
                            ..OrthographicProjection::default_3d()
                        });
                    }
                    if state.stage == 6 {
                        let material = materials.add(StandardMaterial {
                            base_color: Color::srgb(0.47, 0.44, 0.39),
                            perceptual_roughness: 0.95,
                            ..default()
                        });
                        for (position, size) in [
                            (Vec3::new(0.0, -0.1, 0.0), Vec3::new(7.0, 0.2, 7.0)),
                            (Vec3::new(0.0, 2.1, -3.5), Vec3::new(7.0, 4.2, 0.2)),
                            (Vec3::new(-3.5, 2.1, 0.0), Vec3::new(0.2, 4.2, 7.0)),
                            (Vec3::new(3.5, 2.1, 0.0), Vec3::new(0.2, 4.2, 7.0)),
                            (Vec3::new(0.0, 4.2, 0.0), Vec3::new(7.0, 0.2, 7.0)),
                            (Vec3::new(1.0, 0.3, -2.0), Vec3::new(1.8, 0.6, 1.0)),
                            (Vec3::new(1.0, 0.9, -2.5), Vec3::new(1.4, 0.6, 0.7)),
                        ] {
                            commands.spawn((
                                PreviewScene,
                                Mesh3d(meshes.add(Cuboid::from_size(size))),
                                MeshMaterial3d(material.clone()),
                                Transform::from_translation(position),
                                RenderLayers::layer(PREVIEW_LAYER),
                            ));
                        }
                    }
                } else if let Projection::Orthographic(p) = &mut *projection {
                    p.scaling_mode = bevy::camera::ScalingMode::FixedVertical {
                        viewport_height: if state.construction { 4.6 } else { 2.8 },
                    };
                }
            }
            state.since = Instant::now();
        }
    }
    if state.pending || state.since.elapsed() < Duration::from_secs(3) {
        return;
    }
    let folder = std::env::var_os("TERRA_CAPTURE_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| "/tmp/terra-items".into());
    if let Err(error) = std::fs::create_dir_all(&folder) {
        error!(%error);
        exit.write(AppExit::error());
        return;
    }
    state.pending = true;
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(folder.join(format!(
            "{}-{}.png",
            if state.material_reference {
                "material-reference"
            } else if state.construction {
                "construction"
            } else if state.furnaces {
                "furnaces"
            } else {
                "items"
            },
            state.stage
        ))))
        .observe(
            |event: On<ScreenshotCaptured>,
             mut state: ResMut<Studio>,
             mut exit: MessageWriter<AppExit>| {
                let nonblank = event.image.data.as_ref().is_some_and(|data| {
                    data.chunks_exact(4)
                        .filter(|p| p[..3].iter().any(|v| *v > 80))
                        .take(1001)
                        .count()
                        > 1000
                });
                if !nonblank {
                    error!("Blank item studio capture");
                    exit.write(AppExit::error());
                }
                state.captured = true;
            },
        );
}

fn fitted_roof_fixture(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    blocks: &BlockAssets,
    stage: usize,
) -> Result<(), String> {
    use terra_voxel::{BlockSection, SectionPos};
    use terra_world::edits::{BlockObjectState, EditJournal};
    let section = SectionPos::default();
    let mut data = BlockSection::empty(section).map_err(|e| e.to_string())?;
    for x in 5..10 {
        for z in 5..10 {
            if stage == 13 && ![[6, 7], [7, 6], [8, 7]].contains(&[x, z]) {
                continue;
            }
            data.set_block([x, 1, z], if x < 7 || z < 7 { 3 } else { 16 })
                .map_err(|e| e.to_string())?;
        }
    }
    let mut journal = EditJournal::default();
    if stage == 11 {
        for (p, peer, secondary) in [([7, 0, 7], [7, 1, 7], false), ([7, 1, 7], [7, 0, 7], true)] {
            data.set_block(p.map(|v| v as usize), 59)
                .map_err(|e| e.to_string())?;
            journal.record(p, 59).map_err(|e| e.to_string())?;
            journal.set_object_state(
                p,
                Some(BlockObjectState {
                    peer: Some(peer),
                    secondary,
                    ..default()
                }),
            )?;
        }
    } else {
        for y in 1..=2 {
            data.set_block([7, y, 7], 60).map_err(|e| e.to_string())?;
        }
    }
    let templates = blocks
        .templates
        .as_ref()
        .ok_or("Missing roof fixture templates")?;
    let mut padded = vec![0; terra_voxel::meshing::PADDED_VOLUME];
    for z in 0..16 {
        for y in 0..16 {
            for x in 0..16 {
                padded[x + 1 + 18 * (y + 1 + 18 * (z + 1))] = data.blocks()[x + 16 * (y + 16 * z)];
            }
        }
    }
    let geometry = terra_voxel::meshing::mesh_section(&padded).map_err(|e| e.to_string())?;
    let snapshot = crate::meshing::custom::capture(&data, templates, &journal, |p| {
        if p.iter().all(|v| (0..16).contains(v)) {
            Some(data.blocks()[p[0] as usize + 16 * (p[1] as usize + 16 * p[2] as usize)])
        } else {
            Some(0)
        }
    })
    .ok_or("Missing roof model snapshot")?;
    let batches = crate::meshing::custom::compose(
        &geometry,
        &snapshot,
        templates,
        section,
        &RenderOptions {
            water_level: None,
            detail_level: DetailLevel::Near,
            terrain: None,
            exposure: None,
            tints: Default::default(),
            conditions: default(),
        },
    )?;
    for batch in batches {
        let material = blocks
            .materials
            .get(batch.material)
            .ok_or("Missing fitted roof material")?;
        commands.spawn((
            PreviewScene,
            Mesh3d(meshes.add(batch_mesh(&batch))),
            MeshMaterial3d(material.clone()),
            Transform::from_translation(Vec3::new(-7.5, -2.0, -7.5)),
            RenderLayers::layer(PREVIEW_LAYER),
        ));
    }
    Ok(())
}
