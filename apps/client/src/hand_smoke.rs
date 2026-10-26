//! Opt-in visual fixture, not an authoritative gameplay or network simulation.
use bevy::{
    app::AppExit,
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk},
};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use terra_core::{ItemKind, ItemStack, ToolKind};
use terra_network::client::{ClientSession, RemoteTrack, ReplicatedPlayers, SelectedInventorySlot};
use terra_player::{CameraPerspective, PlayerBody, PlayerSet, TerraCamera};

#[derive(Resource)]
struct Fixture {
    start: Instant,
    entered: Instant,
    stage: usize,
    captures: usize,
    requested: bool,
}
const CENTER: Vec3 = Vec3::new(0.5, 201.0, 0.5);
const STAGES: usize = 18;

fn third_person(stage: usize) -> bool {
    matches!(stage, 4 | 14 | 15)
}

pub(crate) fn install(app: &mut App) {
    if !std::env::args().any(|a| a == "--smoke-test-hands") {
        return;
    }
    match terra_core::PlayerProfile::selected_path(&terra_core::menu_config_directory())
        .and_then(terra_core::PlayerProfile::load_for_client)
    {
        Ok(profile) => {
            app.insert_resource(profile);
        }
        Err(error) => {
            error!(%error, "Cannot load held-item fixture profile");
            app.add_systems(Startup, |mut exit: MessageWriter<AppExit>| {
                exit.write(AppExit::error());
            });
            return;
        }
    }
    app.insert_resource(Fixture {
        start: Instant::now(),
        entered: Instant::now(),
        stage: 0,
        captures: 0,
        requested: false,
    })
    // This fixture presents transforms, not network prediction or locomotion.
    .insert_resource(Time::<Fixed>::from_seconds(3600.0))
    .init_resource::<ClientSession>()
    .init_resource::<SelectedInventorySlot>()
    .init_resource::<ReplicatedPlayers>()
    .insert_resource(bevy::winit::WinitSettings::continuous())
    .add_systems(PostStartup, setup)
    .add_systems(
        Update,
        drive
            .after(PlayerSet::Interaction)
            .before(terra_world::streaming::StreamingSet::Interest),
    )
    .add_systems(
        PostUpdate,
        inspect.after(bevy::transform::TransformSystems::Propagate),
    );
}
fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        DirectionalLight {
            illuminance: 200.0,
            ..default()
        },
        Transform::from_xyz(2.0, 4.0, 3.0).looking_at(Vec3::ZERO, Vec3::Y),
        bevy::camera::visibility::RenderLayers::from_layers(&(0..32).collect::<Vec<_>>()),
    ));
    commands.spawn((PlayerBody, Transform::from_translation(CENTER)));
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(12.0, 0.05, 12.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.23, 0.29, 0.20),
            perceptual_roughness: 0.9,
            ..default()
        })),
        Transform::from_translation(CENTER - Vec3::Y * 0.925),
    ));
}
fn drive(world: &mut World) {
    let stage = world.resource::<Fixture>().stage;
    let item = match stage {
        0 => ItemKind::Block(3),
        1 => ItemKind::Tool(ToolKind::StonePickaxe),
        2 | 3 => ItemKind::Tool(ToolKind::IronSword),
        7 => ItemKind::Tool(ToolKind::IronAxe),
        8 => ItemKind::Tool(ToolKind::DiamondPickaxe),
        9 => ItemKind::Tool(ToolKind::IronHoe),
        10 | 13 | 15 => ItemKind::Tool(ToolKind::IronWateringCan),
        11 | 12 | 14 => ItemKind::Tool(ToolKind::IronScythe),
        16 => ItemKind::Tool(ToolKind::Bucket),
        17 => ItemKind::Tool(ToolKind::WaterBucket),
        _ => ItemKind::Block(40),
    };
    let mut session = world.resource_mut::<ClientSession>();
    session.ready = true;
    session
        .inventory
        .creative_set(0, ItemStack { item, count: 1 });
    world.resource_mut::<SelectedInventorySlot>().0 = (stage != 6).then_some(0);
    world.resource_mut::<terra_player::PlayerInputBlock>().0 = false;
    world
        .resource_mut::<CameraPerspective>()
        .clone_from(&if third_person(stage) {
            CameraPerspective::ThirdPerson
        } else {
            CameraPerspective::FirstPerson
        });
    world
        .resource_mut::<terra_player::interaction::InteractionMotion>()
        .swinging = matches!(stage, 3 | 12..=15);
    let eye = if third_person(stage) {
        Transform::from_translation(CENTER + Vec3::new(2.0, 1.0, -3.7)).looking_at(CENTER, Vec3::Y)
    } else {
        Transform::from_translation(CENTER + Vec3::Y * 0.72)
    };
    for mut t in world
        .query_filtered::<&mut Transform, With<TerraCamera>>()
        .iter_mut(world)
    {
        *t = eye;
    }
    let pose = terra_network::protocol::session::PlayerState {
        player: terra_core::PlayerId(777),
        position: (CENTER + Vec3::X * 1.2).to_array(),
        held_item: (stage != 6).then_some(item),
        held_torch: item == ItemKind::Block(40),
        ..default()
    };
    world.resource_mut::<ReplicatedPlayers>().players.insert(
        pose.player,
        RemoteTrack {
            previous: pose,
            current: pose,
            received: Instant::now(),
            interval: 0.05,
        },
    );
}
fn inspect(world: &mut World) {
    let (stage, elapsed, requested, captures) = {
        let f = world.resource::<Fixture>();
        (f.stage, f.entered.elapsed(), f.requested, f.captures)
    };
    if stage >= STAGES {
        return;
    }
    if world.resource::<Fixture>().start.elapsed() > Duration::from_secs(90) {
        error!(stage, "Held item fixture timed out");
        world.write_message(AppExit::error());
        return;
    }
    if requested {
        if captures > stage {
            let mut f = world.resource_mut::<Fixture>();
            f.stage += 1;
            f.requested = false;
            f.entered = Instant::now();
            if f.stage >= STAGES {
                info!("Native equipped-item fixture passed");
                world.write_message(AppExit::Success);
            }
        }
        return;
    }
    if elapsed < Duration::from_secs(3) {
        return;
    }
    let held: Vec<_> = world
        .query::<(Entity, &Name)>()
        .iter(world)
        .filter(|(_, n)| n.as_str() == "Equipped item")
        .map(|(e, _)| e)
        .collect();
    if held.len() != if stage == 6 { 0 } else { 2 }
        || held
            .iter()
            .any(|e| world.get::<Children>(*e).is_none_or(|c| c.is_empty()))
    {
        return;
    }
    let hand_count = world
        .query::<&Name>()
        .iter(world)
        .filter(|name| name.as_str() == "First-person forearm and hand")
        .count();
    if hand_count != 0 {
        error!(hand_count, "First-person limbs must not be rendered");
        world.write_message(AppExit::error());
        return;
    }
    if !third_person(stage) && stage != 6 && !edge_anchored(world, &held) {
        error!(
            stage,
            "Equipped geometry must remain visible and cross the lower screen edge"
        );
        world.write_message(AppExit::error());
        return;
    }
    let folder = std::env::var_os("TERRA_CAPTURE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp/terra-hands"));
    if let Err(error) = std::fs::create_dir_all(&folder) {
        error!(%error);
        world.write_message(AppExit::error());
        return;
    }
    world.resource_mut::<Fixture>().requested = true;
    world
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(folder.join(format!("hands-{stage}.png"))))
        .observe(
            |event: On<ScreenshotCaptured>,
             mut f: ResMut<Fixture>,
             mut exit: MessageWriter<AppExit>| {
                if event.image.data.as_ref().is_none_or(|d| {
                    d.chunks_exact(4)
                        .filter(|p| p[0] > 20 || p[1] > 20 || p[2] > 20)
                        .count()
                        < 1000
                }) {
                    error!("Held item capture is blank");
                    exit.write(AppExit::error());
                    return;
                }
                f.captures += 1;
            },
        );
}

fn edge_anchored(world: &mut World, roots: &[Entity]) -> bool {
    use bevy::camera::primitives::Aabb;
    let Some((camera, projection)) = world
        .query_filtered::<(&GlobalTransform, &Projection), With<TerraCamera>>()
        .iter(world)
        .next()
    else {
        return false;
    };
    let Projection::Perspective(projection) = projection else {
        return false;
    };
    let tangent = (projection.fov * 0.5).tan();
    let view = camera.affine().inverse();
    let Some(root) = roots.iter().find(|e| world.get::<ChildOf>(**e).is_none()) else {
        return false;
    };
    let Some(children) = world.get::<Children>(*root) else {
        return false;
    };
    let mut bottom = f32::INFINITY;
    let mut top = f32::NEG_INFINITY;
    for child in children {
        let (Some(bounds), Some(transform)) = (
            world.get::<Aabb>(*child),
            world.get::<GlobalTransform>(*child),
        ) else {
            continue;
        };
        // Render-only meshes release their CPU vertices after extraction.
        for corner in 0..8 {
            let sign = Vec3::new(
                if corner & 1 == 0 { -1.0 } else { 1.0 },
                if corner & 2 == 0 { -1.0 } else { 1.0 },
                if corner & 4 == 0 { -1.0 } else { 1.0 },
            );
            let p = Vec3::from(bounds.center) + Vec3::from(bounds.half_extents) * sign;
            let local = view.transform_point3(transform.transform_point(p));
            if local.z >= -0.01 {
                continue;
            }
            let x = local.x / (-local.z * tangent * projection.aspect_ratio);
            if !(-1.0..=1.0).contains(&x) {
                continue;
            }
            let y = local.y / (-local.z * tangent);
            bottom = bottom.min(y);
            top = top.max(y);
        }
    }
    info!(
        bottom,
        top, "Equipped item measured screen-space vertical extent"
    );
    bottom < -1.0 && top > -0.90
}
