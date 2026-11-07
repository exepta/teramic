//! Short opt-in native gallery of the real connected-window render/physics path.
use bevy::{
    camera::{Exposure, Hdr},
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk},
};
use std::{
    collections::HashSet,
    path::PathBuf,
    time::{Duration, Instant},
};
use terra_voxel::{BlockSection, global_to_section};
use terra_world::{
    edits::{BlockObjectState, EditJournal},
    objects::windows::Group,
    streaming::{
        BlockEditRequest, ChunkFocus, ChunkStore, LoadedSection, StreamingSettings,
        commit_block_edit, commit_object_state,
    },
};

#[derive(Component)]
struct InspectionCamera;

#[derive(Resource)]
struct Gallery {
    started: Instant,
    changed: Instant,
    captures: usize,
    pending: bool,
    failure: Option<String>,
    opened: bool,
    removed: bool,
    finish_frames: u8,
    rectangles: Vec<[u32; 4]>,
}

fn groups() -> impl Iterator<Item = Group> {
    if door_gallery() {
        return [65, 66, 67, 139]
            .into_iter()
            .enumerate()
            .map(|(index, material)| Group {
                anchor: [-1 + index as i64 * 6, 2, 8],
                material,
                facing: 0,
                width: 2,
                height: 2,
            })
            .collect::<Vec<_>>()
            .into_iter();
    }
    [(1, 2, 10), (2, 1, 6), (2, 2, 2)]
        .into_iter()
        .flat_map(|(width, height, y)| {
            (68..=70).map(move |material| Group {
                anchor: [-1 + i64::from(material - 68) * 6, y, 8],
                material,
                facing: 0,
                width,
                height,
            })
        })
        .collect::<Vec<_>>()
        .into_iter()
}

fn door_gallery() -> bool {
    std::env::var("TERRA_DOOR_GALLERY").is_ok_and(|value| value == "1")
}

pub(crate) fn install(app: &mut App) {
    if !std::env::args().any(|a| a == "--smoke-test-window-gallery") {
        return;
    }
    let now = Instant::now();
    app.insert_resource(Gallery {
        started: now,
        changed: now,
        captures: 0,
        pending: false,
        failure: None,
        opened: false,
        removed: false,
        finish_frames: 0,
        rectangles: Vec::new(),
    })
    .insert_resource(terra_player::ControllerMode::Fly)
    .insert_resource(bevy::winit::WinitSettings::continuous())
    .add_systems(PostStartup, setup)
    .add_systems(
        Update,
        pin.after(terra_player::PlayerSet::Movement)
            .before(terra_world::streaming::StreamingSet::Interest),
    )
    .add_systems(Last, run);
}

fn setup(world: &mut World) {
    let cameras: Vec<_> = world
        .query_filtered::<Entity, With<terra_player::TerraCamera>>()
        .iter(world)
        .collect();
    for entity in cameras {
        let mut entity = world.entity_mut(entity);
        if let Some(mut camera) = entity.get_mut::<Camera>() {
            camera.is_active = false;
        }
        entity.remove::<(
            bevy::light::AtmosphereEnvironmentMapLight,
            bevy::light::GeneratedEnvironmentMapLight,
            bevy::light::EnvironmentMapLight,
            bevy::pbr::AtmosphereSettings,
        )>();
    }
    {
        let mut settings = world.resource_mut::<StreamingSettings>();
        settings.radius = 2;
        settings.vertical_radius = 1;
        settings.max_jobs = 0;
        settings.meshes = false;
    }
    world.resource_mut::<ChunkStore>().loaded.clear();
    world.resource_mut::<ChunkFocus>().0 = [6.0, 7.0, 8.0];
    for group in groups() {
        for p in group.cells() {
            let (owner, local) = global_to_section(p);
            world
                .resource_mut::<ChunkStore>()
                .loaded
                .entry(owner)
                .or_insert_with(|| LoadedSection {
                    data: BlockSection::empty(owner).unwrap(),
                    mesh: None,
                    mesh_epoch: 0,
                    water_epoch: 0,
                    water: None,
                    props: Vec::new(),
                })
                .data
                .set_block(local, group.material)
                .unwrap();
            let mut journal = world.resource_mut::<EditJournal>();
            journal.record(p, group.material).unwrap();
            let object = if door_gallery() {
                BlockObjectState {
                    peer: Some([
                        p[0],
                        p[1] + if p[1] == group.anchor[1] { 1 } else { -1 },
                        p[2],
                    ]),
                    secondary: p[1] != group.anchor[1],
                    ..default()
                }
            } else {
                default()
            };
            journal.set_object_state(p, Some(object)).unwrap();
        }
    }
    let cube = world
        .resource_mut::<Assets<Mesh>>()
        .add(Cuboid::new(1.0, 1.0, 1.0));
    let backdrop = world
        .resource_mut::<Assets<StandardMaterial>>()
        .add(StandardMaterial {
            base_color: Color::srgb(0.32, 0.35, 0.34),
            perceptual_roughness: 0.9,
            ..default()
        });
    for (center, scale) in [
        ([9.0, 6.5, 11.0], [29.0, 15.0, 0.5]),
        ([9.0, 0.0, 7.0], [29.0, 0.5, 10.0]),
    ] {
        world.spawn((
            Mesh3d(cube.clone()),
            MeshMaterial3d(backdrop.clone()),
            Transform::from_translation(Vec3::from_array(center))
                .with_scale(Vec3::from_array(scale)),
        ));
    }
    for x in [0.0, 6.0, 12.0, 18.0] {
        world.spawn((
            PointLight {
                intensity: 65_000.0,
                range: 28.0,
                shadow_maps_enabled: false,
                ..default()
            },
            Transform::from_xyz(x, 8.0, 1.5),
        ));
    }
    world.spawn((
        InspectionCamera,
        Camera3d::default(),
        Hdr,
        Exposure { ev100: 6.0 },
        bevy::core_pipeline::tonemapping::Tonemapping::AcesFitted,
        AmbientLight {
            color: Color::WHITE,
            brightness: 120.0,
            ..default()
        },
        Transform::default(),
    ));
    world.resource_mut::<terra_core::DebugSettings>().enabled = false;
}

fn pin(world: &mut World) {
    let captures = world.resource::<Gallery>().captures;
    let (target, eye) = match captures {
        0 | 1 if door_gallery() => (Vec3::new(9.0, 3.0, 8.5), Vec3::new(9.0, 3.5, -15.0)),
        0 | 1 => (Vec3::new(6.0, 6.8, 8.5), Vec3::new(6.0, 6.8, -9.5)),
        2 => (Vec3::new(0.0, 3.0, 8.5), Vec3::new(-2.5, 4.0, 3.0)),
        _ => (Vec3::new(6.0, 3.0, 8.5), Vec3::new(3.5, 4.0, 3.0)),
    };
    let view = Transform::from_translation(eye).looking_at(target, Vec3::Y);
    for mut transform in world
        .query_filtered::<&mut Transform, With<InspectionCamera>>()
        .iter_mut(world)
    {
        *transform = view;
    }
    for mut camera in world
        .query_filtered::<&mut Camera, With<terra_player::TerraCamera>>()
        .iter_mut(world)
    {
        camera.is_active = false;
    }
    world.resource_mut::<ChunkFocus>().0 = [6.0, 7.0, 8.0];
}

fn run(world: &mut World) {
    for mut light in world.query::<&mut DirectionalLight>().iter_mut(world) {
        light.illuminance = 0.0;
    }
    world.resource_scope(|world, mut state: Mut<Gallery>| {
        if let Err(error) = advance(world, &mut state) {
            error!(%error,"Native connected-window gallery failed");
            world.write_message(AppExit::error());
        }
    });
}

fn advance(world: &mut World, state: &mut Gallery) -> Result<(), String> {
    if let Some(error) = &state.failure {
        return Err(error.clone());
    }
    if state.started.elapsed() > Duration::from_secs(30) {
        return Err("window gallery timed out".into());
    }
    if state.captures == 4 {
        state.finish_frames += 1;
        if state.finish_frames >= 3 {
            info!(
                "Native connected-opening gallery passed: authored woods, closed/open opposing hinges, cross-section owner, actual group splitting, projected material pixels"
            );
            world.write_message(AppExit::Success);
        }
        return Ok(());
    }
    if state.pending || state.started.elapsed() < Duration::from_secs(3) {
        return Ok(());
    }
    if state.captures == 1 && !state.opened {
        world.resource_scope(|world, mut chunks: Mut<ChunkStore>| {
            let mut journal = world.resource_mut::<EditJournal>();
            for p in groups().flat_map(Group::cells) {
                let object = journal.object_state(p).unwrap_or_default();
                commit_object_state(
                    &mut chunks,
                    &mut journal,
                    p,
                    Some(BlockObjectState {
                        facing: 4,
                        ..object
                    }),
                )
                .unwrap();
            }
        });
        state.opened = true;
        state.changed = Instant::now();
        return Ok(());
    }
    if state.captures == 3 && !state.removed {
        let properties = world
            .resource::<terra_core::block_properties::BlockPropertiesCatalog>()
            .clone();
        world.resource_scope(|world, mut chunks: Mut<ChunkStore>| {
            for position in if door_gallery() {
                vec![[6, 2, 8], [6, 3, 8]]
            } else {
                vec![[6, 3, 8]]
            } {
                commit_block_edit(
                    &mut chunks,
                    &mut world.resource_mut::<EditJournal>(),
                    &BlockEditRequest {
                        position,
                        expected_material: if door_gallery() { 66 } else { 69 },
                        material: 0,
                    },
                    &properties,
                )?;
            }
            Ok::<_, String>(true)
        })?;
        state.removed = true;
        state.changed = Instant::now();
        return Ok(());
    }
    if state.changed.elapsed() < Duration::from_millis(700) {
        return Ok(());
    }
    let names: Vec<_> = world
        .query::<&Name>()
        .iter(world)
        .map(|name| name.as_str().to_string())
        .collect();
    let roots = names
        .iter()
        .filter(|n| n.as_str() == "Framed opening")
        .count();
    let leaves = names.iter().filter(|n| n.as_str() == "Hinged leaf").count();
    let expected = if door_gallery() {
        if state.removed { (4, 7) } else { (4, 8) }
    } else if state.removed {
        (10, 16)
    } else {
        (9, 15)
    };
    if (roots, leaves) != expected {
        return Ok(());
    }
    let angles: Vec<_> = world
        .query::<(&Name, &Transform)>()
        .iter(world)
        .filter(|(name, _)| name.as_str() == "Hinged leaf")
        .map(|(_, t)| t.rotation.to_euler(EulerRot::YXZ).0)
        .collect();
    if state.opened {
        if !angles
            .iter()
            .all(|v| (v.abs() - std::f32::consts::FRAC_PI_2).abs() < 0.01)
            || !angles.iter().any(|v| *v > 1.0)
            || !angles.iter().any(|v| *v < -1.0)
        {
            return Err(format!("opposing leaves did not open: {angles:?}"));
        }
    } else if angles.iter().any(|v| v.abs() > 0.01) {
        return Err("closed gallery has rotated glass".into());
    }
    let mut camera_query =
        world.query_filtered::<(&Camera, &GlobalTransform), With<InspectionCamera>>();
    let (camera, transform) = camera_query.single(world).map_err(|e| e.to_string())?;
    state.rectangles.clear();
    let subjects: Vec<_> = if state.captures < 2 {
        groups().collect()
    } else {
        groups()
            .filter(|g| {
                g.width == 2
                    && g.height == 2
                    && g.material
                        == if door_gallery() {
                            if state.captures == 2 { 65 } else { 66 }
                        } else if state.captures == 2 {
                            68
                        } else {
                            69
                        }
            })
            .collect()
    };
    for group in subjects {
        let mut low = Vec2::splat(f32::INFINITY);
        let mut high = Vec2::splat(f32::NEG_INFINITY);
        for x in [0.0, f32::from(group.width)] {
            for y in [0.0, f32::from(group.height)] {
                let p = Vec3::new(group.anchor[0] as f32 + x, group.anchor[1] as f32 + y, 8.5);
                let pixel = camera
                    .world_to_viewport(transform, p)
                    .map_err(|e| format!("window off camera: {e:?}"))?;
                low = low.min(pixel);
                high = high.max(pixel);
            }
        }
        let viewport = camera.logical_viewport_size().ok_or("missing viewport")?;
        if low.min_element() < 0.0 || high.x >= viewport.x || high.y >= viewport.y {
            return Err(format!(
                "window group clipped {low:?}..{high:?} / {viewport:?}"
            ));
        }
        let scale = camera
            .physical_viewport_size()
            .ok_or("missing physical viewport")?
            .as_vec2()
            / viewport;
        state.rectangles.push([
            (low.x * scale.x) as u32,
            (low.y * scale.y) as u32,
            (high.x * scale.x) as u32,
            (high.y * scale.y) as u32,
        ]);
    }
    let directory = std::env::var_os("TERRA_CAPTURE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("teramic-window-gallery"));
    std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    let label = if door_gallery() {
        [
            "doors-closed",
            "doors-open",
            "oak-double-door-open",
            "birch-door-separated",
        ]
    } else {
        [
            "windows-closed",
            "windows-open",
            "oak-2x2-open",
            "birch-group-split",
        ]
    }[state.captures];
    state.pending = true;
    world.spawn(Screenshot::primary_window()).observe(save_to_disk(directory.join(format!("{label}.png")))).observe(|event:On<ScreenshotCaptured>,mut state:ResMut<Gallery>| {
        if let Some(bytes)=&event.image.data {
            let width=event.image.width();let height=event.image.height();
            let mut failure=None;
            for rect in &state.rectangles {
                let mut colors=HashSet::new();let mut min=255u8;let mut max=0u8;
                for y in rect[1]..rect[3].min(height) {for x in rect[0]..rect[2].min(width) {
                    let i=((y*width+x)*4) as usize;
                    if i+3<bytes.len() {let p=&bytes[i..i+3];colors.insert((p[0]/8,p[1]/8,p[2]/8));for v in p {min=min.min(*v);max=max.max(*v);}}
                }}
                if colors.len()<8||max.saturating_sub(min)<35 {failure=Some(format!("actual projected window has no material detail: rect={rect:?} colors={} range={}",colors.len(),max.saturating_sub(min)));break;}
            }
            state.failure=failure;
        } else {state.failure=Some("missing screenshot pixels".into());}
        state.pending=false;state.captures+=1;state.changed=Instant::now();
    });
    Ok(())
}
