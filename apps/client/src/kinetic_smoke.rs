//! Short offline render fixture. RPM is injected only here; gameplay uses the
//! authoritative mechanical network and its replicated visual states.
use bevy::{
    camera::{Exposure, Hdr, visibility::RenderLayers},
    prelude::*,
    render::{
        gpu_readback::{Readback, ReadbackComplete},
        render_resource::TextureFormat,
        view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk},
    },
};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    time::{Duration, Instant},
};
use terra_voxel::{BlockSection, global_to_section, meshing::mesh_section};
use terra_world::{
    edits::{BlockObjectState, EditJournal},
    streaming::{
        ChunkFocus, ChunkStore, GenerationStatus, LoadedSection, WorldAccess, WorldGeneration,
    },
};

#[derive(Component)]
struct GalleryCamera;

const SCOOP_WATER_CELL: [i64; 3] = [2, 1, 4];

#[derive(Resource)]
struct Gallery {
    started: Instant,
    changed: Instant,
    stage: u8,
    captures: usize,
    bad_capture: bool,
    initial_phase: f32,
    stopped_phase: f32,
    stopped_assets: [usize; 2],
    icon_signatures: BTreeMap<u16, u64>,
    icons_requested: bool,
    transport: terra_world::conveyors::Transport,
    initial_drops: BTreeMap<u64, [f64; 3]>,
}

fn spans() -> [terra_world::conveyors::Span; 4] {
    use terra_core::WoodSpecies::*;
    [
        ([2, 2, 9], [0, 0, 1], 3, Birch, 0),
        ([6, 2, 9], [0, 1, 1], 2, Cherry, 0),
        ([10, 2, 10], [0, 1, 0], 2, Spruce, 0),
        ([13, 2, 9], [1, 0, 1], 2, Oak, 4),
    ]
    .map(
        |(start, step, length, wood, facing)| terra_world::conveyors::Span {
            controller: std::array::from_fn(|i| {
                start[i] + step[i] * if facing == 4 { 2 } else { 1 }
            }),
            first: std::array::from_fn(|i| start[i] + step[i]),
            start,
            end: std::array::from_fn(|i| start[i] + step[i] * (i64::from(length) + 1)),
            step,
            length,
            facing,
            wood,
            linked: true,
        },
    )
}

fn samples() -> Vec<([i64; 3], u16, u8)> {
    let mut result = vec![
        ([2, 2, 4], 147, 0),
        ([3, 2, 4], 144, 0),
        ([4, 2, 4], 141, 0),
        ([4, 3, 5], 149, 0),
        ([5, 3, 5], 145, 0),
        ([6, 3, 5], 142, 0),
        ([6, 4, 6], 150, 0),
        ([7, 4, 6], 146, 0),
        ([8, 4, 6], 143, 0),
        ([8, 5, 7], 151, 0),
        ([9, 6, 7], 151, 4),
        ([9, 4, 6], 148, 0),
        // The reduced-speed output needs iron's torque capacity, not a wooden shaft.
        ([10, 4, 6], 146, 0),
        ([5, 2, 4], 100, 0),
        ([12, 2, 1], 146, 4),
        ([12, 3, 1], 100, 4),
        ([1, 1, 1], 99, 0),
        ([2, 1, 1], 100, 0),
        // All four ports of each gearbox are occupied by actual connected axles.
        ([11, 2, 4], 147, 0),
        ([12, 2, 4], 154, 0),
        ([13, 2, 4], 146, 0),
        ([12, 2, 3], 144, 1),
        ([12, 2, 5], 144, 3),
        ([15, 2, 4], 147, 0),
        ([16, 2, 4], 155, 0),
        ([17, 2, 4], 146, 0),
        ([16, 3, 4], 144, 4),
        ([16, 1, 4], 146, 4),
    ];
    for span in spans() {
        if span.facing == 4 {
            result.extend([
                (span.start, 144, 4),
                (span.first, 145, 4),
                (span.controller, 148, terra_core::kinetic::belt_pose(4)),
                (span.end, 146, 4),
                ([13, 3, 9], 100, 4),
            ]);
            continue;
        }
        result.extend([
            ([span.start[0] - 1, span.start[1], span.start[2]], 147, 0),
            (span.start, 144, 0),
            (span.end, 146, 0),
            ([span.start[0] + 1, span.start[1], span.start[2]], 141, 0),
            ([span.end[0] - 1, span.end[1], span.end[2]], 149, 0),
        ]);
        result.extend(span.cells().map(|p| (p, 148, 0)));
    }
    result
}

fn object_state(position: [i64; 3], facing: u8) -> BlockObjectState {
    let (peer, secondary) = match position {
        [2, 1, 1] => (Some([1, 1, 1]), false),
        [5, 2, 4] => (Some([4, 2, 4]), false),
        [12, 3, 1] => (Some([12, 2, 1]), false),
        [13, 3, 9] => (Some([13, 2, 9]), false),
        _ => (None, false),
    };
    let mut state = BlockObjectState {
        facing,
        peer,
        secondary,
        ..default()
    };
    for span in spans() {
        if span.cells().any(|p| p == position) {
            state.peer = Some(if position == span.controller {
                span.end
            } else {
                span.controller
            });
            state.wood = span.wood;
            if span.step == [0, 1, 1] {
                state.facing = terra_core::kinetic::dyed_belt_facing(
                    state.facing,
                    terra_core::beds::DyeColor::Blue,
                );
            } else if span.step == [0, 1, 0] {
                state.facing = terra_core::kinetic::dyed_belt_facing(
                    state.facing,
                    terra_core::beds::DyeColor::Red,
                );
            }
        } else if [
            span.start,
            span.end,
            [span.start[0] + 1, span.start[1], span.start[2]],
            [span.end[0] - 1, span.end[1], span.end[2]],
        ]
        .contains(&position)
        {
            state.wood = span.wood;
        }
        if span.facing == 4 {
            if position == span.start {
                state.peer = Some(span.controller);
            }
            if position == span.end {
                state.peer = Some(span.start);
            }
        }
    }
    state
}

fn fixture_journal() -> EditJournal {
    let mut journal = EditJournal::default();
    for (position, material, facing) in samples() {
        journal.record(position, material).unwrap();
        journal
            .set_object_state(position, Some(object_state(position, facing)))
            .unwrap();
    }
    journal
        .record(SCOOP_WATER_CELL, terra_voxel::MaterialId::FluidWater as u16)
        .unwrap();
    journal
}

pub(crate) fn install(app: &mut App) {
    if !std::env::args().any(|a| a == "--smoke-test-kinetics") {
        return;
    }
    let now = Instant::now();
    app.insert_resource(Gallery {
        started: now,
        changed: now,
        stage: 0,
        captures: 0,
        bad_capture: false,
        initial_phase: 0.0,
        stopped_phase: 0.0,
        stopped_assets: [0; 2],
        icon_signatures: BTreeMap::new(),
        icons_requested: false,
        transport: Default::default(),
        initial_drops: BTreeMap::new(),
    })
    .insert_resource(terra_player::ControllerMode::Fly)
    .insert_resource(bevy::winit::WinitSettings::continuous())
    .add_systems(PostStartup, setup)
    .add_systems(Last, run);
}

fn detach_gameplay_environment(world: &mut World) {
    let entities: Vec<_> = world
        .query_filtered::<Entity, With<terra_player::TerraCamera>>()
        .iter(world)
        .collect();
    for e in entities {
        if let Some(mut camera) = world.get_mut::<Camera>(e) {
            camera.is_active = false;
        }
        world.entity_mut(e).remove::<(
            bevy::light::AtmosphereEnvironmentMapLight,
            bevy::light::GeneratedEnvironmentMapLight,
            bevy::light::EnvironmentMapLight,
            bevy::pbr::AtmosphereSettings,
        )>();
    }
}

fn setup(world: &mut World) {
    detach_gameplay_environment(world);
    let mut session = terra_network::client::ClientSession::default();
    session.ready = true;
    world.insert_resource(session);
    world.insert_resource(WorldAccess::Replica { ready: false });
    world.insert_resource(ChunkStore::default());
    world.insert_resource(EditJournal::default());
    world.resource_mut::<WorldGeneration>().status = GenerationStatus::Ready;
    world.resource_mut::<ChunkFocus>().0 = [7.0, 4.0, 5.0];
    world
        .resource_mut::<terra_core::GameConfig>()
        .client
        .chunk_animation = terra_core::ChunkAnimation::None;
    world.resource_mut::<terra_core::DebugSettings>().enabled = false;
    let mut cells = Vec::new();
    for x in 0..18 {
        for z in 0..16 {
            cells.push(([x, 0, z], terra_voxel::MaterialId::SmoothStone as u16));
        }
    }
    cells.extend(samples().into_iter().map(|(p, m, _)| (p, m)));
    cells.push((SCOOP_WATER_CELL, terra_voxel::MaterialId::FluidWater as u16));
    cells.extend((9..=13).map(|z| ([2, 1, z], 3)));
    cells.extend([
        ([2, 3, 10], 29),
        ([2, 3, 11], 152),
        ([3, 3, 12], 29),
        ([2, 3, 12], 153),
        ([3, 2, 12], 3),
        ([3, 1, 12], 3),
    ]);
    for (position, material) in cells {
        let (owner, local) = global_to_section(position);
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
            .set_block(local, material)
            .unwrap();
        world
            .resource_mut::<EditJournal>()
            .record(position, material)
            .unwrap();
    }
    for (position, material, facing) in samples() {
        world
            .resource_mut::<EditJournal>()
            .set_object_state(position, Some(object_state(position, facing)))
            .unwrap();
        assert!(terra_core::kinetic::part(material).is_some());
    }
    for (position, facing, peer) in [([2, 3, 12], 1, [3, 3, 12]), ([2, 3, 11], 4, [2, 3, 10])] {
        world
            .resource_mut::<EditJournal>()
            .set_object_state(
                position,
                Some(BlockObjectState {
                    facing,
                    peer: Some(peer),
                    ..default()
                }),
            )
            .unwrap();
    }
    let positions: Vec<_> = world
        .resource::<ChunkStore>()
        .loaded
        .keys()
        .copied()
        .collect();
    for position in &positions {
        let origin = position.origin().unwrap();
        let mut padded = Vec::new();
        for z in -1..=16 {
            for y in -1..=16 {
                for x in -1..=16 {
                    padded.push(
                        world
                            .resource::<ChunkStore>()
                            .block_at([origin[0] + x, origin[1] + y, origin[2] + z])
                            .unwrap_or(0),
                    );
                }
            }
        }
        let epoch = world.resource::<EditJournal>().mesh_epoch(*position);
        let mut chunks = world.resource_mut::<ChunkStore>();
        let section = chunks.loaded.get_mut(position).unwrap();
        section.mesh = Some(mesh_section(&padded).unwrap());
        section.mesh_epoch = epoch;
    }
    terra_render::meshing::forget_section_snapshots(world, &positions);
    set_rpm(world, 30.0);
    for span in spans()
        .into_iter()
        .filter(|span| span.step[2] != 0 && span.normal()[1] > 0.0)
    {
        let point = terra_world::conveyors::Segment { span, index: 0 }
            .drop_center()
            .unwrap();
        let (id, point) = {
            let mut journal = world.resource_mut::<EditJournal>();
            assert!(journal.gameplay_mut().drop_stack(
                point,
                terra_core::ItemStack {
                    item: terra_core::ItemKind::Block(3),
                    count: 1
                },
                [0.0; 3],
                10.0
            ));
            let drop = journal.gameplay_mut().drops.last_mut().unwrap();
            drop.grounded = true;
            drop.angular_velocity = [0.0; 3];
            (drop.id, drop.position)
        };
        world
            .resource_mut::<Gallery>()
            .initial_drops
            .insert(id, point);
    }
    world.spawn((
        GalleryCamera,
        Camera3d::default(),
        Hdr,
        Exposure { ev100: 7.0 },
        bevy::core_pipeline::tonemapping::Tonemapping::AcesFitted,
        AmbientLight {
            color: Color::WHITE,
            brightness: 180.0,
            ..default()
        },
        Transform::from_xyz(25.0, 17.0, 26.0).looking_at(Vec3::new(7.0, 3.0, 7.0), Vec3::Y),
    ));
    for position in [[3.0, 8.0, 5.0], [11.0, 8.0, 6.0]] {
        world.spawn((
            PointLight {
                intensity: 90_000.0,
                range: 28.0,
                shadow_maps_enabled: false,
                ..default()
            },
            Transform::from_translation(Vec3::from_array(position)),
        ));
    }
}

fn assets_grew(before: [usize; 2], after: [usize; 2]) -> bool {
    before
        .into_iter()
        .zip(after)
        .any(|(before, after)| after > before)
}

fn connected_states(rpm: f32) -> BTreeMap<[i64; 3], terra_core::kinetic::KineticState> {
    let journal = fixture_journal();
    let nodes: BTreeMap<_, _> = samples()
        .into_iter()
        .map(|(position, material, facing)| {
            let source = material == 147 || matches!(position, [2, 1, 1] | [12, 3, 1] | [13, 3, 9]);
            (
                position,
                terra_core::kinetic::Node {
                    position,
                    material,
                    facing,
                    links: terra_world::conveyors::segment_at(&journal, position)
                        .map_or([None; 2], |segment| segment.span.links(position)),
                    secondary: material == 148
                        && terra_world::conveyors::segment_at(&journal, position).is_some_and(
                            |segment| segment.span.linked && position != segment.span.controller,
                        ),
                    source_rpm: if source { rpm } else { 0.0 },
                    belt_load: matches!(material, 144..=146)
                        && terra_world::conveyors::segment_at(&journal, position).is_some_and(
                            |segment| segment.span.linked && segment.span.controller == position,
                        ),
                    source_capacity: match (source, material) {
                        (true, 147) => 3200,
                        (true, 100) => 12_800,
                        _ => 0,
                    },
                },
            )
        })
        .collect();
    let mut budget = 512;
    let mut states = BTreeMap::new();
    for seed in [[2, 2, 4], [2, 1, 1], [12, 3, 1], [11, 2, 4], [15, 2, 4]]
        .into_iter()
        .chain(spans().map(|span| span.start))
    {
        states.extend(
            terra_core::kinetic::solve_component(seed, &mut budget, |p| nodes.get(&p).copied())
                .into_iter()
                .map(|(node, state)| (node.position, state)),
        );
    }
    assert_eq!(
        states.len(),
        nodes.len(),
        "native assemblies must form their declared connected networks"
    );
    assert!(states.values().all(|state| !state.overloaded));
    states
}

fn visible_samples() -> Vec<([i64; 3], u16, u8)> {
    let journal = fixture_journal();
    samples()
        .into_iter()
        .filter(|(position, material, _)| {
            *material != 148
                || terra_world::conveyors::span_at(&journal, *position)
                    .is_none_or(|span| !span.linked || span.controller == *position)
        })
        .collect()
}

fn motion_counts(rpm: f32) -> (usize, usize) {
    let states = connected_states(rpm);
    visible_samples()
        .into_iter()
        .fold((0, 0), |(forward, reverse), (position, _, _)| {
            let speed = states[&position].rpm;
            (
                forward + usize::from(speed > 0.0),
                reverse + usize::from(speed < 0.0),
            )
        })
}

fn set_rpm(world: &mut World, rpm: f32) {
    let states = connected_states(rpm);
    world
        .resource_mut::<EditJournal>()
        .replace_kinetic_states(states.clone());
    // Offline bootstrap may still own an idle client session. Populate only its
    // visual snapshot, never transport or authority state, for this render test.
    if let Some(mut session) = world.get_resource_mut::<terra_network::client::ClientSession>() {
        session.kinetics = states
            .into_iter()
            .map(
                |(position, state)| terra_network::protocol::session::KineticVisual {
                    position,
                    rpm: state.rpm,
                    overloaded: state.overloaded,
                    stress: state.stress,
                    capacity: state.capacity,
                    available_capacity: state.available_capacity,
                },
            )
            .collect();
    }
}

fn capture_gear_icons(world: &mut World) -> Result<(), String> {
    let directory = std::env::var_os("TERRA_CAPTURE_DIR")
        .map(PathBuf::from)
        .ok_or("Set TERRA_CAPTURE_DIR for kinetic icons")?;
    let previews = world.resource::<terra_render::RenderBlockPreviews>();
    let images = [141, 142, 143, 148, 149, 150, 151, 152, 153, 154, 155]
        .into_iter()
        .map(|id| {
            previews
                .images
                .get(&id)
                .cloned()
                .map(|image| (id, image))
                .ok_or_else(|| format!("Missing kinetic icon {id}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    for (id, image) in images {
        let readback = request_icon_readback(world, id, image);
        world.entity_mut(readback).observe(save_to_disk(
            directory.join(format!("kinetic-icon-{id}.png")),
        ));
    }
    Ok(())
}

fn icon_readback_image(source: &Image, data: &[u8]) -> Option<Image> {
    if !matches!(
        source.texture_descriptor.format,
        TextureFormat::Rgba8Unorm | TextureFormat::Rgba8UnormSrgb
    ) || source.texture_descriptor.size.depth_or_array_layers != 1
    {
        return None;
    }
    let width = source.texture_descriptor.size.width as usize * 4;
    let height = source.texture_descriptor.size.height as usize;
    let stride = width.div_ceil(256) * 256;
    if width == 0 || data.len() < stride * height {
        return None;
    }
    let mut image = source.clone();
    image.data = Some(
        data.chunks_exact(stride)
            .take(height)
            .flat_map(|row| row[..width].iter().copied())
            .collect(),
    );
    Some(image)
}

fn request_icon_readback(world: &mut World, id: u16, image: Handle<Image>) -> Entity {
    // Screenshot::image redirects an active camera and overwrites its target.
    // Finished inventory targets have no camera, so copy their retained GPU pixels.
    world
        .spawn(Readback::texture(image.clone()))
        .observe(
            move |event: On<ReadbackComplete>,
                  images: Res<Assets<Image>>,
                  mut commands: Commands,
                  mut gallery: ResMut<Gallery>| {
                if gallery.icon_signatures.contains_key(&id) {
                    return;
                }
                commands.entity(event.entity).remove::<Readback>();
                let Some(image) = images
                    .get(&image)
                    .and_then(|source| icon_readback_image(source, &event.data))
                else {
                    gallery.bad_capture = true;
                    commands.entity(event.entity).try_despawn();
                    return;
                };
                let bytes = image.data.as_ref().unwrap();
                gallery.bad_capture |= bytes
                    .chunks_exact(4)
                    .filter(|pixel| {
                        pixel[3] > 20 && (pixel[0] > 20 || pixel[1] > 20 || pixel[2] > 20)
                    })
                    .count()
                    < 100;
                let signature = bytes.iter().fold(0xcbf29ce484222325_u64, |hash, byte| {
                    (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
                });
                gallery.icon_signatures.insert(id, signature);
                commands.trigger(ScreenshotCaptured {
                    entity: event.entity,
                    image,
                });
                commands.entity(event.entity).try_despawn();
            },
        )
        .id()
}

fn capture(world: &mut World, name: &str) -> Result<(), String> {
    let directory = std::env::var_os("TERRA_CAPTURE_DIR")
        .map(PathBuf::from)
        .ok_or("Set TERRA_CAPTURE_DIR for kinetic gallery")?;
    std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    world
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(directory.join(format!("{name}.png"))))
        .observe(
            |event: On<ScreenshotCaptured>, mut gallery: ResMut<Gallery>| {
                gallery.bad_capture |= event.image.data.as_ref().is_none_or(|bytes| {
                    bytes
                        .chunks_exact(4)
                        .filter(|p| p[0] > 25 || p[1] > 25 || p[2] > 25)
                        .count()
                        < 10_000
                });
                gallery.captures += 1;
            },
        );
    Ok(())
}

fn mute_world_sun(world: &mut World) {
    for (mut light, layers) in world
        .query::<(&mut DirectionalLight, Option<&RenderLayers>)>()
        .iter_mut(world)
    {
        if layers.is_none_or(|layers| layers.intersects(&RenderLayers::default()))
            && light.illuminance != 0.0
        {
            light.illuminance = 0.0;
        }
    }
}

fn transport_drops(world: &mut World, transport: &mut terra_world::conveyors::Transport) {
    static PROPERTIES: std::sync::OnceLock<terra_core::block_properties::BlockPropertiesCatalog> =
        std::sync::OnceLock::new();
    let elapsed = world.resource::<Time>().delta_secs();
    world.resource_scope(|world, mut journal: Mut<EditJournal>| {
        terra_world::conveyors::transport(
            transport,
            world.resource::<ChunkStore>(),
            &mut journal,
            PROPERTIES.get_or_init(Default::default),
            elapsed,
        );
    });
    let drops = world.resource::<EditJournal>().gameplay().drops.clone();
    world
        .resource_mut::<terra_network::client::ClientSession>()
        .drops = drops;
}

fn verify_visible_drops(world: &mut World) -> Result<(), String> {
    let rendered = world
        .query::<(&Name, &Transform, &Children)>()
        .iter(world)
        .filter(|(name, _, _)| name.as_str() == "Dropped resource")
        .filter(|(_, _, children)| {
            children
                .iter()
                .any(|child| world.get::<Mesh3d>(child).is_some())
        })
        .map(|(_, pose, _)| pose.translation)
        .collect::<Vec<_>>();
    let drops = &world.resource::<EditJournal>().gameplay().drops;
    if drops.len() != 2
        || rendered.len() != 2
        || drops.iter().any(|drop| {
            let position = Vec3::from_array(drop.position.map(|v| v as f32));
            !rendered.iter().any(|pose| pose.distance(position) < 0.2)
        })
    {
        return Err(
            "Transported items are missing rendered meshes or lag behind their authoritative poses"
                .into(),
        );
    }
    Ok(())
}

fn verify_ribbon_colors(world: &mut World) -> Result<(), String> {
    let handles = world
        .query::<(&Name, &MeshMaterial3d<StandardMaterial>)>()
        .iter(world)
        .filter(|(name, _)| name.as_str() == "Stretched conveyor ribbon")
        .map(|(_, material)| material.0.clone())
        .collect::<Vec<_>>();
    let materials = world.resource::<Assets<StandardMaterial>>();
    let mut found = [false; 3];
    for handle in &handles {
        let Some(material) = materials.get(handle) else {
            continue;
        };
        let c = material.base_color.to_srgba();
        found[0] |= (c.red - c.green).abs() < 0.03 && (c.green - c.blue).abs() < 0.03;
        found[1] |= c.blue > c.red * 1.6 && c.blue > c.green * 1.5;
        found[2] |= c.red > c.blue * 2.0 && c.red > c.green * 2.0;
    }
    if handles.len() != 4 || !found.into_iter().all(|present| present) {
        return Err("Expected gray, blue and red ribbons independent of wooden casings".into());
    }
    Ok(())
}

fn run(world: &mut World) {
    mute_world_sun(world);
    let mut state = world.remove_resource::<Gallery>().unwrap();
    transport_drops(world, &mut state.transport);
    let result = (|| -> Result<bool, String> {
        if state.started.elapsed() > Duration::from_secs(55) {
            return Err("Kinetic gallery timed out".into());
        }
        if state.bad_capture {
            return Err("Kinetic screenshot is blank".into());
        }
        let status = world.resource::<terra_render::CrusherRenderStatus>();
        if status.kinetic_parts != 46
            || status.machines != 1
            || status.cranks != 4
            || status.belt_spans != 4
            || status.grounded_belt_cells != 5
            || !world.resource::<terra_render::RenderStatus>().ready
            || world.resource::<terra_render::BuildingStatus>().drops != 2
        {
            return Ok(false);
        }
        if status.cached_meshes != 48 {
            return Err("Kinetic asset import did not retain all shared parts, including the conveyor surfaces".into());
        }
        let elapsed = state.changed.elapsed();
        match state.stage {
            0 if elapsed > Duration::from_secs(3)
                && (status.forward_parts, status.reverse_parts) == motion_counts(30.0) =>
            {
                state.initial_phase = status.phase_checksum;
                capture(world, "kinetics-forward")?;
                state.stage = 1;
                state.changed = Instant::now();
            }
            1 if state.captures == 1 && elapsed > Duration::from_millis(700) => {
                if (status.phase_checksum - state.initial_phase).abs() < 0.01 {
                    return Err("Forward rotors did not animate".into());
                }
                if state.initial_drops.iter().any(|(id, initial)| {
                    world
                        .resource::<EditJournal>()
                        .gameplay()
                        .drops
                        .iter()
                        .find(|drop| drop.id == *id)
                        .is_none_or(|drop| {
                            drop.position
                                .iter()
                                .zip(initial)
                                .map(|(a, b)| (a - b).powi(2))
                                .sum::<f64>()
                                < 0.01
                        })
                }) {
                    return Err(
                        "Real world items did not travel on the horizontal and sloped belts".into(),
                    );
                }
                set_rpm(world, -30.0);
                state.stage = 2;
                state.changed = Instant::now();
            }
            2 if elapsed > Duration::from_secs(2)
                && (status.forward_parts, status.reverse_parts) == motion_counts(-30.0) =>
            {
                capture(world, "kinetics-reverse")?;
                state.stage = 3;
                state.changed = Instant::now();
            }
            3 if state.captures == 2 => {
                set_rpm(world, 0.0);
                state.stage = 4;
                state.changed = Instant::now();
            }
            4 if elapsed > Duration::from_secs(2)
                && status.forward_parts == 0
                && status.reverse_parts == 0
                && world
                    .resource::<terra_render::RenderBlockPreviews>()
                    .pending
                    == 0
                && world.resource::<terra_render::PlayerPreviewImage>().ready =>
            {
                state.stopped_phase = status.phase_checksum;
                state.stopped_assets = [
                    world.resource::<Assets<Mesh>>().len(),
                    world.resource::<Assets<StandardMaterial>>().len(),
                ];
                capture(world, "kinetics-stopped")?;
                if !state.icons_requested {
                    capture_gear_icons(world)?;
                    state.icons_requested = true;
                }
                state.stage = 5;
                state.changed = Instant::now();
            }
            5 if state.captures == 3
                && state.icon_signatures.len() == 11
                && elapsed > Duration::from_secs(2) =>
            {
                if (status.phase_checksum - state.stopped_phase).abs() > 0.0001 {
                    return Err("Stopped rotors still change phase".into());
                }
                let assets = [
                    world.resource::<Assets<Mesh>>().len(),
                    world.resource::<Assets<StandardMaterial>>().len(),
                ];
                if assets_grew(state.stopped_assets, assets) {
                    return Err(format!(
                        "Stationary machines grew mesh/material assets: {:?} -> {:?}",
                        state.stopped_assets, assets
                    ));
                }
                for ids in [[141, 142, 143], [149, 150, 151]] {
                    let signatures: std::collections::BTreeSet<_> = ids
                        .into_iter()
                        .map(|id| state.icon_signatures[&id])
                        .collect();
                    if signatures.len() != 3 {
                        return Err(format!(
                            "Kinetic material variants share identical rendered icons: {ids:?}"
                        ));
                    }
                }
                if state.icon_signatures[&154] == state.icon_signatures[&155] {
                    return Err(
                        "Horizontal and vertical gearboxes have identical rendered icons".into(),
                    );
                }
                if world
                    .resource::<terra_render::RenderBlockPreviews>()
                    .pending
                    == 0
                {
                    for mut pose in world
                        .query_filtered::<&mut Transform, With<GalleryCamera>>()
                        .iter_mut(world)
                    {
                        *pose = Transform::from_xyz(9.0, 7.0, 17.0)
                            .looking_at(Vec3::new(5.0, 3.0, 11.0), Vec3::Y);
                    }
                    for drop in &mut world.resource_mut::<EditJournal>().gameplay_mut().drops {
                        drop.position = state.initial_drops[&drop.id];
                    }
                    set_rpm(world, 30.0);
                    state.stage = 6;
                    state.changed = Instant::now();
                }
            }
            6 if elapsed > Duration::from_millis(500) => {
                verify_visible_drops(world)?;
                verify_ribbon_colors(world)?;
                capture(world, "kinetics-stretched-belts-items-woods")?;
                state.stage = 7;
                state.changed = Instant::now();
            }
            7 if state.captures == 4 && elapsed > Duration::from_millis(500) => {
                for mut pose in world
                    .query_filtered::<&mut Transform, With<GalleryCamera>>()
                    .iter_mut(world)
                {
                    *pose = Transform::from_xyz(22.0, 8.0, 18.0)
                        .looking_at(Vec3::new(14.0, 2.8, 7.0), Vec3::Y);
                }
                state.stage = 8;
                state.changed = Instant::now();
            }
            8 if elapsed > Duration::from_millis(500) => {
                capture(world, "kinetics-gearboxes-diagonal-three-shafts")?;
                state.stage = 9;
                state.changed = Instant::now();
            }
            9 if state.captures == 5 && elapsed > Duration::from_millis(500) => {
                for mut pose in world
                    .query_filtered::<&mut Transform, With<GalleryCamera>>()
                    .iter_mut(world)
                {
                    *pose = Transform::from_xyz(8.0, 6.5, -1.0)
                        .looking_at(Vec3::new(14.0, 2.6, 4.0), Vec3::Y);
                }
                state.stage = 10;
                state.changed = Instant::now();
            }
            10 if elapsed > Duration::from_millis(500) => {
                capture(world, "kinetics-gearbox-opposite-ports")?;
                state.stage = 11;
                state.changed = Instant::now();
            }
            11 if state.captures == 6 && elapsed > Duration::from_millis(500) => {
                info!(
                    cached_meshes = status.cached_meshes,
                    captures = state.captures,
                    "Kinetic native gallery passed: horizontal/sloped/standing diagonal three-shaft spans, transported items, four wood profiles, gearbox ports, logistics models, fractional RPM and shared assets"
                );
                return Ok(true);
            }
            _ => {}
        }
        Ok(false)
    })();
    world.insert_resource(state);
    match result {
        Ok(true) => {
            world.write_message(AppExit::Success);
        }
        Err(error) => {
            error!(%error,"Kinetic native gallery failed");
            world.write_message(AppExit::error());
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neutral_gallery_detaches_atmosphere_before_disabling_gameplay_view() {
        let mut world = World::new();
        let original = world
            .spawn((
                Camera3d::default(),
                terra_player::TerraCamera,
                bevy::light::AtmosphereEnvironmentMapLight::default(),
                bevy::pbr::AtmosphereSettings::default(),
            ))
            .id();
        let inspection = world.spawn((Camera3d::default(), GalleryCamera)).id();
        detach_gameplay_environment(&mut world);
        assert!(!world.get::<Camera>(original).unwrap().is_active);
        assert!(
            world
                .get::<bevy::light::AtmosphereEnvironmentMapLight>(original)
                .is_none()
        );
        assert!(
            world
                .get::<bevy::pbr::AtmosphereSettings>(original)
                .is_none()
        );
        assert!(world.get::<Camera>(inspection).unwrap().is_active);
    }

    #[test]
    fn releasing_preview_meshes_is_not_machine_asset_growth() {
        assert!(!assets_grew([1416, 334], [1312, 334]));
        assert!(!assets_grew([1312, 334], [1312, 334]));
        assert!(assets_grew([1312, 334], [1313, 334]));
        assert!(assets_grew([1312, 334], [1312, 335]));
    }

    #[test]
    fn neutral_gallery_mutes_only_world_lights_and_preserves_preview_lighting() {
        let mut world = World::new();
        let light = || DirectionalLight {
            illuminance: 12_345.0,
            ..default()
        };
        let implicit_world = world.spawn(light()).id();
        let explicit_world = world.spawn((light(), RenderLayers::default())).id();
        let preview = world.spawn((light(), RenderLayers::layer(30))).id();
        let other = world.spawn((light(), RenderLayers::layer(62))).id();
        mute_world_sun(&mut world);
        for entity in [implicit_world, explicit_world] {
            assert_eq!(
                world.get::<DirectionalLight>(entity).unwrap().illuminance,
                0.0
            );
        }
        for entity in [preview, other] {
            assert_eq!(
                world.get::<DirectionalLight>(entity).unwrap().illuminance,
                12_345.0
            );
        }
    }

    #[test]
    fn connected_gallery_uses_diagonal_ratios_fractional_rpm_and_a_right_angle_pair() {
        assert!(samples().contains(&([10, 4, 6], 146, 0)));
        for direction in [-1.0, 1.0] {
            let states = connected_states(direction * 30.0);
            for (position, rpm) in [
                ([2, 2, 4], 30.0),
                ([4, 2, 4], 30.0),
                ([4, 3, 5], -15.0),
                ([6, 3, 5], -15.0),
                ([6, 4, 6], 7.5),
                ([8, 4, 6], 7.5),
                ([8, 5, 7], -3.75),
                ([9, 6, 7], -3.75),
                ([9, 4, 6], 7.5),
                ([10, 4, 6], 7.5),
                ([1, 1, 1], 30.0),
                ([2, 1, 1], 30.0),
                ([5, 2, 4], 30.0),
                ([12, 2, 1], 30.0),
                ([12, 3, 1], 30.0),
            ] {
                assert_eq!(states[&position].rpm, direction * rpm);
                assert!(!states[&position].overloaded);
            }
            let drives = samples()
                .into_iter()
                .map(|(p, _, _)| states[&p].rpm)
                .collect::<Vec<_>>();
            let forward = drives.iter().filter(|&&rpm| rpm > 0.0).count();
            let reverse = drives.iter().filter(|&&rpm| rpm < 0.0).count();
            assert_eq!(forward + reverse, 55);
            for position in [[12, 2, 4], [16, 2, 4], [14, 2, 10], [15, 2, 11]] {
                assert_eq!(states[&position].rpm.abs(), 30.0);
            }
        }
        assert!(
            connected_states(0.0)
                .values()
                .all(|state| state.rpm == 0.0 && !state.overloaded)
        );
    }

    #[test]
    fn native_spans_use_persistent_ownership_four_wood_profiles_and_one_visual_per_loop() {
        let journal = fixture_journal();
        let mut hidden = std::collections::BTreeSet::new();
        for span in spans() {
            assert_eq!(
                terra_world::conveyors::span_at(&journal, span.first),
                Some(span)
            );
            for position in span.cells().filter(|p| *p != span.controller) {
                if journal.block(position) == Some(148) {
                    hidden.insert(position);
                }
            }
            for material in if span.facing < 4 {
                &[141, 144, 148, 149][..]
            } else {
                &[144, 145, 146, 148][..]
            } {
                assert!(samples().into_iter().any(|(position, id, _)| {
                    id == *material && journal.object_state(position).unwrap().wood == span.wood
                }));
            }
        }
        let visible = samples()
            .into_iter()
            .filter(|(position, _, _)| !hidden.contains(position))
            .count();
        assert_eq!(visible, 51);
        assert_eq!(visible - 4 - 1, 46);
        assert_eq!(visible_samples().len(), visible);
    }

    #[test]
    fn gallery_crusher_is_one_cell_with_an_adjacent_real_crank() {
        let journal = fixture_journal();
        assert_eq!(
            samples()
                .into_iter()
                .filter(|(_, material, _)| *material == 99)
                .count(),
            1
        );
        assert!(journal.valid_crusher([1, 1, 1]));
        assert_eq!(journal.object_state([1, 1, 1]).unwrap().peer, None);
        assert_eq!(journal.crusher_crank_position([1, 1, 1]), Some([2, 1, 1]));
        assert_eq!(journal.crank_anchor([2, 1, 1]), Some([1, 1, 1]));
        assert!(visible_samples().contains(&([2, 1, 1], 100, 0)));
    }

    #[test]
    fn first_wheel_scoop_contacts_a_real_loaded_source_while_the_next_wheel_is_dry() {
        let journal = fixture_journal();
        let mut chunks = ChunkStore::default();
        let (owner, local) = global_to_section(SCOOP_WATER_CELL);
        let mut data = BlockSection::empty(owner).unwrap();
        data.set_block(local, terra_voxel::MaterialId::FluidWater as u16)
            .unwrap();
        chunks.loaded.insert(
            owner,
            LoadedSection {
                data,
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            },
        );
        let config = terra_core::WorldConfig {
            water: true,
            biomes: false,
            caves: false,
            rivers: false,
            base_height: 16.0,
            sea_level: 0.0,
            ..Default::default()
        };
        let terrain = terra_world::TerraGenerator::new(config).unwrap();
        let sample: [f64; 3] = [2.5, 2.5 - 1.44, 4.5];
        let cell = sample.map(|v| v.floor() as i64);
        assert_eq!(cell, SCOOP_WATER_CELL);
        let block = chunks.block_at(cell).unwrap();
        assert_eq!(block, terra_voxel::MaterialId::FluidWater as u16);
        let fluid = chunks.fluid_cell_at(&terrain, &journal, cell, block);
        assert!(fluid.source && fluid.amount == 8);
        assert!(sample[1] < cell[1] as f64 + fluid.height());
        let dry = [11, 1, 4];
        assert_eq!(
            chunks
                .fluid_cell_at(&terrain, &journal, dry, chunks.block_at(dry).unwrap())
                .amount,
            0
        );
        assert!(samples().contains(&([11, 2, 4], 147, 0)));
    }

    #[test]
    fn finished_icons_use_nondestructive_gpu_readback_and_strip_row_padding() {
        let mut world = World::new();
        world.init_resource::<Assets<Image>>();
        let now = Instant::now();
        world.insert_resource(Gallery {
            started: now,
            changed: now,
            stage: 0,
            captures: 0,
            bad_capture: false,
            initial_phase: 0.0,
            stopped_phase: 0.0,
            stopped_assets: [0; 2],
            icon_signatures: BTreeMap::new(),
            icons_requested: false,
            transport: Default::default(),
            initial_drops: BTreeMap::new(),
        });
        let source = Image::new_target_texture(65, 2, TextureFormat::Rgba8UnormSrgb, None);
        let original_data = source.data.clone();
        let image = world.resource_mut::<Assets<Image>>().add(source);
        let entity = request_icon_readback(&mut world, 141, image.clone());
        assert!(matches!(
            world.get::<Readback>(entity),
            Some(Readback::Texture(handle)) if *handle == image
        ));
        assert!(world.get::<Screenshot>(entity).is_none());

        let row = [90, 120, 150, 255].repeat(65);
        let mut gpu_bytes = vec![0xee; 512 * 2];
        gpu_bytes[..row.len()].copy_from_slice(&row);
        gpu_bytes[512..512 + row.len()].copy_from_slice(&row);
        let source = world.resource::<Assets<Image>>().get(&image).unwrap();
        let copied = icon_readback_image(source, &gpu_bytes).unwrap();
        assert_eq!(copied.data.as_ref().unwrap(), &row.repeat(2));
        assert!(icon_readback_image(source, &gpu_bytes[..1023]).is_none());

        world.trigger(ReadbackComplete {
            entity,
            data: gpu_bytes,
        });
        world.flush();
        assert!(!world.resource::<Gallery>().bad_capture);
        assert!(
            world
                .resource::<Gallery>()
                .icon_signatures
                .contains_key(&141)
        );
        assert!(world.get_entity(entity).is_err());
        assert_eq!(
            world.resource::<Assets<Image>>().get(&image).unwrap().data,
            original_data,
            "reading an icon must not replace or clear the published texture"
        );
    }
}
