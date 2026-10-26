//! Short actual-render gallery; navigation and grabbing have separate authority tests.
use bevy::{
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk},
};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use terra_core::creatures::{
    Creature, CreatureBehavior as Behavior, CreatureKind as Kind, CreatureNeeds,
};
use terra_network::client::{CreatureTrack, ReplicatedCreatures};

#[derive(Resource)]
struct Gallery {
    start: Instant,
    last: Instant,
    requested: usize,
    captured: usize,
    failed: bool,
}
pub(crate) fn install(app: &mut App) {
    if !std::env::args().any(|a| a == "--smoke-test-stalker") {
        return;
    }
    app.init_resource::<ReplicatedCreatures>()
        .insert_resource(terra_player::ControllerMode::Fly)
        .insert_resource(Gallery {
            start: Instant::now(),
            last: Instant::now(),
            requested: 0,
            captured: 0,
            failed: false,
        })
        .add_systems(PostStartup, setup)
        .add_systems(
            Update,
            run.after(terra_player::PlayerSet::Movement)
                .before(terra_player::PlayerSet::Targeting),
        );
}
fn setup(
    mut animals: ResMut<ReplicatedCreatures>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let a = Creature {
        id: terra_core::creatures::stalker_id([0, 0]).unwrap(),
        kind: Kind::Stalker,
        home: [0, 0],
        position: [8.0, 32.05, 18.0],
        velocity: [0.0; 3],
        yaw: 0.0,
        health: 45.0,
        behavior: Behavior::Rest,
        timer: 1.0,
        age: 0.0,
        variant: 0,
        needs: CreatureNeeds::full(Kind::Stalker),
    };
    animals.creatures.insert(
        a.id,
        CreatureTrack {
            previous: a.clone(),
            current: a,
            interval: 0.1,
            received: Instant::now(),
            playback: Default::default(),
        },
    );
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(10.0, 0.2, 10.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.19, 0.21, 0.22),
            perceptual_roughness: 0.9,
            ..default()
        })),
        Transform::from_xyz(8.0, 31.95, 18.0),
    ));
    commands.spawn((
        PointLight {
            intensity: 1200.0,
            range: 12.0,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(6.0, 37.0, 14.0),
    ));
}
fn run(world: &mut World) {
    let stage = world.resource::<Gallery>().captured;
    if world.resource::<Gallery>().failed
        || world.resource::<Gallery>().start.elapsed() > Duration::from_secs(60)
    {
        error!(stage, "Stalker model fixture failed or timed out");
        world.write_message(AppExit::error());
        return;
    }
    if stage >= 5 {
        if world.resource::<Gallery>().last.elapsed() > Duration::from_millis(600) {
            info!(
                "Stalker native gallery passed: front/side/back, articulated grab and angry run; model-only visual fixture"
            );
            world.write_message(AppExit::Success);
        }
        return;
    }
    let target = Vec3::new(8.0, 33.4, 18.0);
    let offset = match stage {
        0 => Vec3::new(1.8, 0.9, -5.4),
        1 => Vec3::new(5.3, 0.5, 1.6),
        2 => Vec3::new(-1.8, 0.8, 5.4),
        _ => Vec3::new(2.5, 0.6, -5.1),
    };
    for mut camera in world
        .query_filtered::<&mut Transform, With<terra_player::TerraCamera>>()
        .iter_mut(world)
    {
        *camera = Transform::from_translation(target + offset).looking_at(target, Vec3::Y);
    }
    for track in world
        .resource_mut::<ReplicatedCreatures>()
        .creatures
        .values_mut()
    {
        track.current.behavior = match stage {
            3 => Behavior::Hold,
            4 => Behavior::Attack,
            _ => Behavior::Rest,
        };
        track.current.velocity = if stage == 4 {
            [0.0, 0.0, -3.5]
        } else {
            [0.0; 3]
        };
        track.previous = track.current.clone();
        track.received = Instant::now();
    }
    world.resource_mut::<terra_world::streaming::ChunkFocus>().0 = [8.0, 33.0, 18.0];
    let gallery = world.resource::<Gallery>();
    if !world.resource::<terra_render::RenderStatus>().ready
        || gallery.start.elapsed() < Duration::from_secs(7)
        || gallery.last.elapsed() < Duration::from_secs(2)
        || gallery.requested != gallery.captured
    {
        return;
    }
    if world
        .query::<&terra_render::creatures::CreatureAvatar>()
        .iter(world)
        .count()
        != 1
    {
        return;
    }
    let directory = PathBuf::from(
        std::env::var("TERRA_CAPTURE_DIR").unwrap_or_else(|_| "/tmp/terra-stalker".into()),
    );
    if std::fs::create_dir_all(&directory).is_err() {
        world.write_message(AppExit::error());
        return;
    }
    world.resource_mut::<Gallery>().requested += 1;
    world
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(directory.join(format!("stalker-{stage}.png"))))
        .observe(
            |event: On<ScreenshotCaptured>, mut gallery: ResMut<Gallery>| {
                let nonblank = event.image.data.as_ref().is_some_and(|data| {
                    data.chunks_exact(4)
                        .step_by(16)
                        .filter(|p| p[..3].iter().any(|v| *v > 16))
                        .count()
                        > data.len() / 4 / 16 / 100
                });
                if nonblank {
                    gallery.captured += 1;
                    gallery.last = Instant::now();
                } else {
                    gallery.failed = true;
                }
            },
        );
}
