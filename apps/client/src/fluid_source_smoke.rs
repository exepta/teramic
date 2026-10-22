//! Native source-block acceptance through real integrated-server edits and replica deltas.
use bevy::{
    app::AppExit,
    camera::{Exposure, Hdr},
    core_pipeline::tonemapping::Tonemapping,
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk},
};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use terra_network::{
    client::{ClientSession, InventoryRequest, SelectedInventorySlot},
    protocol::{
        inventory::{InventoryAction, InventoryCommand},
        session::EditOutcome,
    },
};
use terra_player::{CameraInputState, ControllerMode, PlayerInputBlock, TerraCamera};
use terra_voxel::{MaterialId, SectionPos};
use terra_world::{
    edits::EditJournal,
    props::PropId,
    streaming::{BlockEditRequest, ChunkStore},
};

#[derive(Component)]
struct SourceStudio;

#[derive(Resource)]
struct SourceSmoke {
    started: Instant,
    entered: Instant,
    stage: u8,
    source: [i64; 3],
    support: [i64; 3],
    aim: Quat,
    props: Vec<(SectionPos, PropId)>,
    water_before: usize,
    captures: u8,
    failed: bool,
}

pub(crate) fn install(app: &mut App) {
    if !std::env::args().any(|arg| arg == "--smoke-test-fluid-source") {
        return;
    }
    let now = Instant::now();
    app.insert_resource(SourceSmoke {
        started: now,
        entered: now,
        stage: 0,
        source: [0; 3],
        support: [0; 3],
        aim: Quat::IDENTITY,
        props: Vec::new(),
        water_before: 0,
        captures: 0,
        failed: false,
    })
    .insert_resource(ControllerMode::Walk)
    .insert_resource(bevy::winit::WinitSettings::continuous())
    .add_systems(PreUpdate, capture_input.after(bevy::input::InputSystems))
    .add_systems(PostUpdate, inspect);
}

fn capture_input(world: &mut World) {
    if world
        .get_resource::<PlayerInputBlock>()
        .is_some_and(|blocked| !blocked.0)
        && let Some(mut capture) = world.get_resource_mut::<CameraInputState>()
    {
        capture.captured = true;
    }
}

fn inspect(world: &mut World) {
    if world.resource::<SourceSmoke>().stage == 255 {
        return;
    }
    if let Err(error) = advance(world) {
        error!(%error,stage=world.resource::<SourceSmoke>().stage,"Native authoritative fluid-source fixture failed");
        world.resource_mut::<SourceSmoke>().stage = 255;
        world.write_message(AppExit::error());
    }
}

fn advance(world: &mut World) -> Result<(), String> {
    let state = world.resource::<SourceSmoke>();
    if state.started.elapsed() > Duration::from_secs(90) || state.failed {
        return Err("Timed out or source screenshot was blank".into());
    }
    let stage = state.stage;
    let elapsed = state.entered.elapsed();
    let Some(session) = world.get_resource::<ClientSession>() else {
        return Ok(());
    };
    if let Some(error) = &session.error {
        return Err(error.clone());
    }
    if !session.ready {
        return Ok(());
    }
    if stage == 0 {
        if !world.contains_resource::<terra_server::IntegratedServer>() {
            return Err("Expected a real integrated server".into());
        }
        let confirmed = session.confirmed_state();
        if elapsed < Duration::from_secs(1)
            || !confirmed.is_some_and(|body| body.grounded)
            || !world
                .get_resource::<terra_render::RenderStatus>()
                .is_some_and(|status| status.ready)
            || !world
                .get_resource::<terra_render::PropRenderStatus>()
                .is_some_and(|status| status.ready)
        {
            return Ok(());
        }
        if session.game_mode != terra_core::GameMode::Creative {
            return Err("Source preparation requires Creative".into());
        }
        let expected_eye = Vec3::from_array(confirmed.ok_or("Missing confirmed player")?.position)
            + Vec3::Y * terra_physics::character::PLAYER_EYE_OFFSET;
        let eye = camera_eye(world).ok_or("Missing actual player camera")?;
        if eye.distance(expected_eye) > 0.25 {
            if elapsed < Duration::from_secs(8) {
                return Ok(());
            }
            return Err(format!(
                "Player camera is not following the synchronized body: eye={eye:?}, expected={expected_eye:?}"
            ));
        }
        if !world
            .resource::<terra_world::streaming::WorldTerrain>()
            .0
            .config()
            .water
        {
            return Err("Water must be enabled for the native source acceptance world".into());
        }
        let Some((support, source, aim, props)) = placement(world) else {
            if elapsed < Duration::from_secs(8) {
                return Ok(());
            }
            return Err(format!(
                "No visible natural ground beside real vegetation: eye={eye:?}, loaded_sections={}",
                world.resource::<ChunkStore>().loaded.len()
            ));
        };
        let revision = world.resource::<ClientSession>().inventory.revision;
        world.write_message(InventoryRequest(InventoryCommand {
            revision,
            action: InventoryAction::CreativeSet {
                slot: 0,
                item: terra_core::ItemKind::Block(MaterialId::FluidWater as u16),
                count: 128,
            },
        }));
        let triangles = world.resource::<terra_render::WaterStatus>().triangles;
        let mut smoke = world.resource_mut::<SourceSmoke>();
        smoke.source = source;
        smoke.support = support;
        smoke.aim = aim;
        smoke.props = props;
        smoke.water_before = triangles;
        next(world, 1);
        return Ok(());
    }
    let source = world.resource::<SourceSmoke>().source;
    match stage {
        1 if world
            .resource::<ClientSession>()
            .inventory
            .slot(0)
            .is_some_and(|stack| {
                stack.item == terra_core::ItemKind::Block(MaterialId::FluidWater as u16)
                    && stack.count == 128
            }) =>
        {
            let rotation = world.resource::<SourceSmoke>().aim;
            for mut camera in world
                .query_filtered::<&mut Transform, With<TerraCamera>>()
                .iter_mut(world)
            {
                camera.rotation = rotation;
            }
            world.write_message(terra_ui::InventoryAction::Hotbar(0));
            next(world, 2);
        }
        2 if elapsed > Duration::from_millis(200) => {
            let support = world.resource::<SourceSmoke>().support;
            if !matches!(world.resource::<terra_player::TargetWorld>().0,Some(terra_player::WorldTarget::Block(hit)) if hit.position==support&&hit.adjacent()==source)
            {
                let target = world.resource::<terra_player::TargetWorld>().0;
                let pose = world
                    .query_filtered::<(&Transform, &terra_player::CameraLook), With<TerraCamera>>()
                    .iter(world)
                    .next()
                    .map(|(camera, look)| (camera.translation, camera.rotation, look.rotation()));
                return Err(format!(
                    "Production picking did not retain source support {support:?}, source {source:?}: target={target:?}, pose={pose:?}"
                ));
            }
            if world.resource::<SelectedInventorySlot>().0 != Some(0) {
                return Err("Source item was not selected through the production hotbar".into());
            }
            world.write_message(BlockEditRequest {
                position: source,
                expected_material: 0,
                material: MaterialId::FluidWater as u16,
            });
            next(world, 3);
        }
        3 => {
            let present = world.resource::<ChunkStore>().block_at(source)
                == Some(MaterialId::FluidWater as u16);
            let cell = world.resource::<EditJournal>().fluid(source);
            if present && cell.is_some_and(|cell| cell.source && cell.amount == 8) {
                if !world
                    .resource::<ClientSession>()
                    .last_edit
                    .is_some_and(|(_, outcome)| outcome == EditOutcome::Applied)
                {
                    return Err(
                        "Source appeared without an applied server edit acknowledgement".into(),
                    );
                }
                info!(
                    ?source,
                    ?cell,
                    "Creative source block and full source cell arrived through authoritative section deltas"
                );
                studio(world, source);
                next(world, 4);
            } else if elapsed > Duration::from_secs(8) {
                return Err(format!(
                    "Source edit did not replicate: block={:?}, fluid={cell:?}, result={:?}",
                    world.resource::<ChunkStore>().block_at(source),
                    world.resource::<ClientSession>().last_edit
                ));
            }
        }
        4 => {
            let journal = world.resource::<EditJournal>();
            let spread = journal
                .fluid_cells()
                .filter(|(p, cell)| {
                    *p != source
                        && cell.amount > 0
                        && (p[0] - source[0]).abs() <= 6
                        && (p[2] - source[2]).abs() <= 6
                })
                .count();
            let removed = world
                .resource::<SourceSmoke>()
                .props
                .iter()
                .filter(|(owner, id)| journal.prop_removed(*owner, *id))
                .count();
            let water = world.resource::<terra_render::WaterStatus>();
            let triangles = water.triangles;
            let visible_tiles = water.visible_tiles;
            if spread >= 3
                && removed > 0
                && triangles > world.resource::<SourceSmoke>().water_before
                && visible_tiles > 0
                && elapsed > Duration::from_secs(4)
            {
                let meshes = world
                    .query::<(&Name, &Mesh3d)>()
                    .iter(world)
                    .filter(|(name, _)| name.as_str().starts_with("Water "))
                    .count();
                if meshes == 0 {
                    return Err("Fluid deltas produced no real water-surface mesh entities".into());
                }
                info!(
                    spread,
                    removed,
                    meshes,
                    triangles,
                    "Source flows into real neighboring cells and removes individually persisted flooded props"
                );
                screenshot(world, "source-flow")?;
                next(world, 5);
            }
        }
        5 if world.resource::<SourceSmoke>().captures >= 1 => {
            let eye = camera_eye(world).ok_or("Missing actual player camera")?;
            let point = Vec3::from_array(source.map(|v| v as f32 + 0.5));
            let rotation = Transform::from_translation(eye)
                .looking_at(point, Vec3::Y)
                .rotation;
            for mut camera in world
                .query_filtered::<&mut Transform, With<TerraCamera>>()
                .iter_mut(world)
            {
                camera.rotation = rotation;
            }
            next(world, 9);
        }
        9 if elapsed > Duration::from_millis(200) => {
            world.write_message(BlockEditRequest {
                position: source,
                expected_material: MaterialId::FluidWater as u16,
                material: 0,
            });
            next(world, 10);
        }
        10 if world
            .resource::<ClientSession>()
            .last_edit
            .is_some_and(|(_, outcome)| outcome == EditOutcome::Invalid) =>
        {
            if world.resource::<ChunkStore>().block_at(source)
                != Some(MaterialId::FluidWater as u16)
                || !world
                    .resource::<EditJournal>()
                    .fluid(source)
                    .is_some_and(|cell| cell.source)
            {
                return Err("Rejected mining removed the source".into());
            }
            info!("Server rejected source mining and retained the actual source");
            let revision = world.resource::<ClientSession>().inventory.revision;
            world.write_message(InventoryRequest(InventoryCommand {
                revision,
                action: InventoryAction::CreativeSet {
                    slot: 0,
                    item: terra_core::ItemKind::Block(3),
                    count: 128,
                },
            }));
            next(world, 6);
        }
        6 if elapsed > Duration::from_millis(200)
            && world
                .resource::<ClientSession>()
                .inventory
                .slot(0)
                .is_some_and(|stack| stack.item == terra_core::ItemKind::Block(3)) =>
        {
            if !matches!(world.resource::<terra_player::TargetWorld>().0,Some(terra_player::WorldTarget::Block(hit)) if hit.position==source)
            {
                return Err("Actual source anchor is not selectable for removal".into());
            }
            world.write_message(BlockEditRequest {
                position: source,
                expected_material: MaterialId::FluidWater as u16,
                material: 3,
            });
            next(world, 7);
        }
        7 => {
            let removed = world.resource::<ChunkStore>().block_at(source) == Some(3);
            let no_source = world
                .resource::<EditJournal>()
                .fluid_cells()
                .all(|(p, cell)| {
                    !cell.source || (p[0] - source[0]).abs() > 6 || (p[2] - source[2]).abs() > 6
                });
            if removed && no_source && elapsed > Duration::from_millis(300) {
                if !world
                    .resource::<ClientSession>()
                    .last_edit
                    .is_some_and(|(_, outcome)| outcome == EditOutcome::Applied)
                {
                    return Err("Source removal lacks server acknowledgement".into());
                }
                info!(
                    ?source,
                    "Source anchor and liquid removed by acknowledged edit"
                );
                screenshot(world, "source-replaced")?;
                next(world, 8);
            }
        }
        8 if world.resource::<SourceSmoke>().captures >= 2 => {
            let remaining = world.resource::<EditJournal>().fluid_cells().count();
            if remaining > 0 {
                if elapsed > Duration::from_secs(25) {
                    return Err(format!(
                        "Source replacement left {remaining} permanent water cells"
                    ));
                }
                return Ok(());
            }
            info!(
                captures = world.resource::<SourceSmoke>().captures,
                "Native authoritative source placement, flow, prop removal, mining rejection, replacement and complete downstream recession passed"
            );
            next(world, 255);
            world.write_message(AppExit::Success);
        }
        _ => {}
    }
    Ok(())
}

type Placement = ([i64; 3], [i64; 3], Quat, Vec<(SectionPos, PropId)>);

fn camera_eye(world: &mut World) -> Option<Vec3> {
    Some(
        world
            .query_filtered::<&Transform, With<TerraCamera>>()
            .iter(world)
            .next()?
            .translation,
    )
}

fn placement(world: &mut World) -> Option<Placement> {
    let eye = camera_eye(world)?;
    let chunks = world.resource::<ChunkStore>();
    let props: Vec<_> = chunks
        .loaded
        .values()
        .flat_map(|section| &section.props)
        .filter(|prop| {
            !prop.removed
                && (prop.position[0] - f64::from(eye.x)).hypot(prop.position[2] - f64::from(eye.z))
                    < 10.0
        })
        .copied()
        .collect();
    let mut best = None;
    let mut best_count = 0;
    for pitch in [-0.35, -0.5, -0.7, -0.9] {
        for sample in 0..64 {
            let yaw = sample as f32 * std::f32::consts::TAU / 64.0 - std::f32::consts::PI;
            let aim = Quat::from_euler(EulerRot::YXZ, yaw, pitch, 0.0);
            let (_, target) = terra_world::targeting::pick_world(
                chunks,
                eye.to_array().map(f64::from),
                (aim * Vec3::NEG_Z).to_array().map(f64::from),
            );
            let Some(terra_player::WorldTarget::Block(hit)) = target else {
                continue;
            };
            let source = hit.adjacent();
            if hit.normal != [0, 1, 0] || hit.distance < 2.5 || chunks.block_at(source) != Some(0) {
                continue;
            }
            // Aim at the interior of the real supporting face, avoiding edge changes
            // from the player's interpolated eye between authority snapshots.
            let face = Vec3::new(
                hit.position[0] as f32 + 0.5,
                hit.position[1] as f32 + 1.0,
                hit.position[2] as f32 + 0.5,
            );
            let aim = Transform::from_translation(eye)
                .looking_at(face, Vec3::Y)
                .rotation;
            if !matches!(
                terra_world::targeting::pick_world(
                    chunks,
                    eye.to_array().map(f64::from),
                    (aim * Vec3::NEG_Z).to_array().map(f64::from),
                ).1,
                Some(terra_player::WorldTarget::Block(center))
                    if center.position == hit.position && center.normal == [0, 1, 0]
            ) {
                continue;
            }
            let nearby: Vec<_> = props
                .iter()
                .filter(|prop| {
                    (prop.position[1] - source[1] as f64).abs() < 0.5
                        && (prop.position[0] - source[0] as f64 - 0.5)
                            .hypot(prop.position[2] - source[2] as f64 - 0.5)
                            < 2.2
                })
                .map(|prop| (prop.section, prop.id))
                .collect();
            if nearby.len() > best_count {
                best_count = nearby.len();
                best = Some((hit.position, source, aim, nearby));
            }
        }
    }
    best
}

fn next(world: &mut World, stage: u8) {
    let mut state = world.resource_mut::<SourceSmoke>();
    state.stage = stage;
    state.entered = Instant::now();
}

fn studio(world: &mut World, source: [i64; 3]) {
    if world
        .query_filtered::<Entity, With<SourceStudio>>()
        .iter(world)
        .next()
        .is_some()
    {
        return;
    }
    let msaa = world
        .query_filtered::<&Msaa, With<TerraCamera>>()
        .iter(world)
        .next()
        .copied()
        .unwrap_or_default();
    for mut camera in world
        .query_filtered::<&mut Camera, With<TerraCamera>>()
        .iter_mut(world)
    {
        camera.order = -1;
    }
    let center = Vec3::from_array(source.map(|v| v as f32 + 0.5));
    world.spawn((
        SourceStudio,
        Camera3d::default(),
        Camera::default(),
        Hdr,
        Exposure { ev100: 13.7 },
        AmbientLight {
            brightness: 700.0,
            ..default()
        },
        Tonemapping::AcesFitted,
        bevy::core_pipeline::prepass::DepthPrepass,
        bevy::pbr::ScreenSpaceTransmission {
            steps: 1,
            ..default()
        },
        msaa,
        Transform::from_translation(center + Vec3::new(5.0, 5.0, 7.0)).looking_at(center, Vec3::Y),
    ));
}

fn screenshot(world: &mut World, name: &str) -> Result<(), String> {
    let directory = std::env::var_os("TERRA_CAPTURE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp/terra-fluid-source"));
    std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    world
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(directory.join(format!("{name}.png"))))
        .observe(
            |event: On<ScreenshotCaptured>, mut state: ResMut<SourceSmoke>| {
                let colors: std::collections::HashSet<_> = event
                    .image
                    .data
                    .as_deref()
                    .unwrap_or_default()
                    .chunks_exact(4)
                    .step_by(31)
                    .take(4096)
                    .map(|p| [p[0], p[1], p[2]])
                    .collect();
                state.failed |= colors.len() < 16;
                state.captures += 1;
            },
        );
    Ok(())
}
