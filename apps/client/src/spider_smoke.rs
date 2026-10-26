//! Short real-render gallery; AI, poison and attachment collision have separate authority tests.
use bevy::{
    asset::RenderAssetUsages,
    mesh::Indices,
    prelude::*,
    render::{
        render_resource::PrimitiveTopology,
        view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk},
    },
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
struct Inspection {
    start: Instant,
    last: Instant,
    requested: usize,
    captured: usize,
    failed: bool,
}

pub(crate) fn install(app: &mut App) {
    if !std::env::args().any(|arg| arg == "--smoke-test-spiders") {
        return;
    }
    app.init_resource::<ReplicatedCreatures>()
        .insert_resource(terra_player::ControllerMode::Fly)
        .insert_resource(Inspection {
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
    mut commands: Commands,
    mut animals: ResMut<ReplicatedCreatures>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (index, kind) in [Kind::SmallSpider, Kind::MediumSpider, Kind::LargeSpider]
        .into_iter()
        .enumerate()
    {
        let animal = Creature {
            id: 0x7370696465720000 + index as u64,
            home: [0, 0],
            kind,
            position: [4.0 + index as f64 * 6.0, 32.0, 20.0],
            velocity: [0.0; 3],
            yaw: 0.0,
            health: kind.health(),
            behavior: Behavior::Rest,
            timer: 3.0,
            age: 0.0,
            variant: 0,
            needs: CreatureNeeds::full(kind),
        };
        animals.creatures.insert(
            animal.id,
            CreatureTrack {
                interval: 0.1,
                previous: animal.clone(),
                current: animal,
                received: Instant::now(),
                playback: Default::default(),
            },
        );
    }
    let silk = materials.add(StandardMaterial {
        perceptual_roughness: 0.95,
        reflectance: 0.05,
        ..default()
    });
    let rock = materials.add(StandardMaterial {
        base_color: Color::srgb(0.20, 0.22, 0.23),
        perceptual_roughness: 0.95,
        ..default()
    });
    let cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    for variant in 0..10 {
        let origin = Vec3::new(
            (variant % 5) as f32 * 3.2,
            35.0,
            30.0 + (variant / 5) as f32 * 4.0,
        );
        for part in terra_world::props::visual::template(
            terra_world::props::PropKind::SpiderWeb,
            variant,
            terra_world::props::visual::PropDetail::Near,
            &default(),
        )
        .expect("authored web")
        {
            let mut mesh = Mesh::new(
                PrimitiveTopology::TriangleList,
                RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
            );
            mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, part.positions);
            mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, part.normals);
            mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, part.uvs);
            mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, part.colors);
            mesh.insert_indices(Indices::U32(part.indices));
            commands.spawn((
                Name::new(format!("Spider web gallery variant {variant}")),
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(silk.clone()),
                Transform::from_translation(origin),
            ));
        }
        let frame: Vec<(Vec3, Vec3)> = if matches!(variant, 2 | 3) {
            vec![(Vec3::new(0.0, 0.15, 0.0), Vec3::new(2.6, 0.3, 1.6))]
        } else if variant == 5 {
            vec![(Vec3::new(0.0, -0.12, 0.0), Vec3::new(2.6, 0.2, 2.6))]
        } else {
            vec![
                (Vec3::new(-1.1, -0.9, -0.13), Vec3::new(0.25, 2.4, 0.35)),
                (Vec3::new(1.1, -0.9, -0.13), Vec3::new(0.25, 2.4, 0.35)),
                (Vec3::new(0.0, 0.12, -0.13), Vec3::new(2.5, 0.28, 0.35)),
                (Vec3::new(0.0, -1.9, -0.13), Vec3::new(2.5, 0.25, 0.35)),
            ]
        };
        for (position, scale) in frame {
            commands.spawn((
                Mesh3d(cube.clone()),
                MeshMaterial3d(rock.clone()),
                Transform::from_translation(origin + position).with_scale(scale),
            ));
        }
    }
}

fn run(world: &mut World) {
    let stage = world.resource::<Inspection>().captured;
    if world.resource::<Inspection>().failed
        || world.resource::<Inspection>().start.elapsed() > Duration::from_secs(90)
    {
        error!(stage, "Spider visual fixture failed or timed out");
        world.write_message(AppExit::error());
        return;
    }
    if stage >= 9 {
        info!(
            "Spider visual fixture passed: three body sizes front/side, articulated wall/ceiling/descending poses and ten actual web mesh templates; model-only fixture"
        );
        world.write_message(AppExit::Success);
        return;
    }
    let eye = if stage < 6 {
        let index = stage / 2;
        let size = [0.15, 0.68, 1.8][index];
        let target = Vec3::new(
            4.0 + index as f32 * 6.0,
            32.0 + [0.08, 0.39, 0.96][index],
            20.0,
        );
        target
            + if stage.is_multiple_of(2) {
                Vec3::new(size * 1.4, size * 0.75, -size * 3.0)
            } else {
                Vec3::new(size * 3.1, size * 0.9, size * 0.9)
            }
    } else if stage == 6 {
        Vec3::new(12.0, 36.5, 14.0)
    } else if stage == 7 {
        Vec3::new(6.2, 37.0, 38.5)
    } else {
        Vec3::new(8.0, 35.8, 40.5)
    };
    let target = if stage < 6 {
        Vec3::new(
            4.0 + (stage / 2) as f32 * 6.0,
            32.0 + [0.08, 0.39, 0.96][stage / 2],
            20.0,
        )
    } else if stage == 6 {
        Vec3::new(10.0, 33.5, 20.0)
    } else {
        Vec3::new(6.4, 34.2, if stage == 7 { 30.0 } else { 34.0 })
    };
    for track in world
        .resource_mut::<ReplicatedCreatures>()
        .creatures
        .values_mut()
    {
        let index = match track.current.kind {
            Kind::SmallSpider => 0,
            Kind::MediumSpider => 1,
            _ => 2,
        };
        track.current.behavior = if stage == 6 {
            [Behavior::Climb, Behavior::Ceiling, Behavior::Descend][index]
        } else {
            Behavior::Wander
        };
        track.current.velocity = if stage == 6 {
            [0.0, -0.05, 0.0]
        } else {
            [0.25, 0.0, 0.0]
        };
        track.current.position[1] = if stage == 6 { 34.0 } else { 32.0 };
        track.current.needs.goal = if stage == 6 && index == 2 {
            Some(terra_core::creatures::NeedGoal {
                kind: terra_core::creatures::NeedKind::Food,
                position: [16, 39, 20],
            })
        } else {
            None
        };
        track.previous = track.current.clone();
        track.received = Instant::now();
    }
    for mut camera in world
        .query_filtered::<&mut Transform, With<terra_player::TerraCamera>>()
        .iter_mut(world)
    {
        *camera = Transform::from_translation(eye).looking_at(target, Vec3::Y);
    }
    world.resource_mut::<terra_world::streaming::ChunkFocus>().0 = [10.0, 33.0, 25.0];
    let ready = world.resource::<terra_render::RenderStatus>().ready;
    let state = world.resource::<Inspection>();
    if !ready
        || state.start.elapsed() < Duration::from_secs(8)
        || state.last.elapsed() < Duration::from_secs(2)
        || state.requested != state.captured
    {
        return;
    }
    if world
        .query::<&terra_render::creatures::CreatureAvatar>()
        .iter(world)
        .count()
        != 3
    {
        return;
    }
    let directory = PathBuf::from(
        std::env::var("TERRA_CAPTURE_DIR").unwrap_or_else(|_| "/tmp/terra-spiders".into()),
    );
    if let Err(error) = std::fs::create_dir_all(&directory) {
        error!(%error);
        world.write_message(AppExit::error());
        return;
    }
    world.resource_mut::<Inspection>().requested += 1;
    world
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(directory.join(format!("spiders-{stage}.png"))))
        .observe(
            |event: On<ScreenshotCaptured>, mut state: ResMut<Inspection>| {
                let nonblank = event.image.data.as_ref().is_some_and(|data| {
                    data.chunks_exact(4)
                        .step_by(16)
                        .filter(|p| p[..3].iter().any(|v| *v > 16))
                        .count()
                        > data.len() / 4 / 16 / 100
                });
                if nonblank {
                    state.captured += 1;
                    state.last = Instant::now();
                } else {
                    state.failed = true;
                }
            },
        );
}
