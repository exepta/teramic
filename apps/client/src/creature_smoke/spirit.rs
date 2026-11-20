//! `TERRA_SPIRIT_INSPECT=1 --smoke-test-creatures`: real cached creature models,
//! animation and screenshots. Deliberately separate from authoritative AI tests.
use super::*;

pub(super) fn setup(replicas: &mut ReplicatedCreatures) {
    let specs = (0..7)
        .map(|variant| (CreatureKind::Butterfly, variant))
        .chain(
            (0..terra_core::creatures::gloom::COUNT)
                .map(|variant| (CreatureKind::SpiritGloom, variant)),
        )
        .chain([(CreatureKind::SpiritFox, 0)]);
    for (slot, (kind, variant)) in specs.enumerate() {
        let home = [slot as i64, 0];
        let Some(id) = creature_id(home, 0) else {
            continue;
        };
        let position = if kind == CreatureKind::Butterfly {
            [4.0 + slot as f64 * 1.3, 33.3, 24.0]
        } else if kind == CreatureKind::SpiritGloom {
            gloom_center(variant)
        } else {
            [17.0, 32.3, 20.0]
        };
        let animal = Creature {
            id,
            kind,
            home,
            position,
            velocity: [0.0; 3],
            yaw: std::f32::consts::PI,
            health: kind.health(),
            behavior: if kind.is_flying() {
                CreatureBehavior::Fly
            } else {
                CreatureBehavior::Wander
            },
            timer: 2.0,
            age: 0.0,
            variant,
            needs: CreatureNeeds::full(kind),
        };
        replicas.creatures.insert(
            id,
            CreatureTrack {
                previous: animal.clone(),
                current: animal,
                received: Instant::now(),
                interval: 0.1,
                playback: Default::default(),
            },
        );
    }
}

fn gloom_center(variant: u8) -> [f64; 3] {
    let row = variant / 4;
    let column = variant % 4;
    [
        4.0 + f64::from(column) * 3.0 + f64::from(row) * 1.5,
        32.3,
        20.0 - f64::from(row) * 3.5,
    ]
}

pub(super) fn run(world: &mut World) {
    // Real motion snapshots exercise the bounded cached light trail as well as
    // hover/wing/tail animation in this visual fixture.
    let now = world.resource::<Time>().elapsed_secs_f64();
    for track in world
        .resource_mut::<ReplicatedCreatures>()
        .creatures
        .values_mut()
    {
        if track.current.kind != CreatureKind::SpiritGloom {
            continue;
        }
        let center = gloom_center(track.current.variant);
        let phase = now * 0.7 + f64::from(track.current.variant) * 0.5;
        track.previous = track.current.clone();
        track.current.position = [
            center[0] + phase.sin() * 0.65,
            center[1],
            center[2] + phase.cos() * 0.4,
        ];
        track.current.velocity = [
            (phase.cos() * 0.455) as f32,
            0.0,
            (-phase.sin() * 0.28) as f32,
        ];
        track.received = Instant::now();
    }
    let stage = world.resource::<Inspection>().captured;
    let state = world.resource::<Inspection>();
    if state.failed || state.start.elapsed() > Duration::from_secs(120) {
        error!("Spirit creature gallery failed or timed out");
        world.write_message(AppExit::error());
        return;
    }
    if stage >= 4 {
        if state.last.elapsed() >= Duration::from_millis(500) {
            info!(
                "Spirit gallery rendered seven butterfly variants, all seven Gloom variants, bounded light trails and Spirit Fox with wing/float/tail animation; visual fixture, not an AI test"
            );
            world.write_message(AppExit::Success);
        }
        return;
    }
    let eye = match stage {
        0 => Transform::from_xyz(10.0, 37.5, 34.0).looking_at(Vec3::new(10.0, 33.0, 21.0), Vec3::Y),
        1 => Transform::from_xyz(8.0, 34.1, 27.5).looking_at(Vec3::new(7.9, 33.4, 24.0), Vec3::Y),
        2 => Transform::from_xyz(8.0, 34.3, 26.5).looking_at(Vec3::new(8.0, 32.8, 20.0), Vec3::Y),
        _ => Transform::from_xyz(10.0, 34.1, 20.6).looking_at(Vec3::new(9.0, 32.8, 16.5), Vec3::Y),
    };
    for mut body in world
        .query_filtered::<&mut Transform, With<terra_player::PlayerBody>>()
        .iter_mut(world)
    {
        body.translation = eye.translation - Vec3::Y * terra_physics::character::PLAYER_EYE_OFFSET;
    }
    for mut camera in world
        .query_filtered::<&mut Transform, With<terra_player::TerraCamera>>()
        .iter_mut(world)
    {
        *camera = eye;
    }
    // Keep streaming and controller focus in the same bucket when a view changes.
    world.resource_mut::<terra_world::streaming::ChunkFocus>().0 =
        eye.translation.to_array().map(f64::from);
    let ready = world.resource::<terra_render::RenderStatus>().ready
        && world
            .resource::<terra_world::streaming::WorldGeneration>()
            .status
            == terra_world::streaming::GenerationStatus::Ready;
    let avatars = world
        .query::<&terra_render::creatures::CreatureAvatar>()
        .iter(world)
        .count();
    let state = world.resource::<Inspection>();
    if !ready
        || avatars != 15
        || state.start.elapsed() < Duration::from_secs(8)
        || state.requested != state.captured
        || state.last.elapsed() < Duration::from_secs(3)
    {
        return;
    }
    let folder = PathBuf::from(
        std::env::var("TERRA_CAPTURE_DIR").unwrap_or_else(|_| "/tmp/terra-spirit-creatures".into()),
    );
    if let Err(error) = std::fs::create_dir_all(&folder) {
        error!(%error, "Cannot create Spirit gallery capture directory");
        world.write_message(AppExit::error());
        return;
    }
    world.resource_mut::<Inspection>().requested += 1;
    world
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(
            folder.join(format!("spirit-creatures-{stage}.png")),
        ))
        .observe(
            |event: On<ScreenshotCaptured>, mut state: ResMut<Inspection>| {
                let nonblank = event.image.data.as_ref().is_some_and(|data| {
                    data.chunks_exact(4)
                        .step_by(16)
                        .filter(|p| p[..3].iter().any(|v| *v > 20))
                        .count()
                        > 1000
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
