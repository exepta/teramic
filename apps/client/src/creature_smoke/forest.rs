//! `TERRA_FOREST_INSPECT=1 --smoke-test-creatures`: real cached creature models,
//! animation and screenshots. Deliberately separate from authoritative AI tests.
use super::*;

pub(super) fn setup(replicas: &mut ReplicatedCreatures) {
    let specs = [
        (CreatureKind::Skeleton, 0),
        (CreatureKind::Skeleton, 1),
        (CreatureKind::Skeleton, 2),
        (CreatureKind::ThornBrute, 0),
        (CreatureKind::Raven, 0),
        (CreatureKind::BloodSlime, 0),
        (CreatureKind::BloodStalker, 0),
        (CreatureKind::BloodWisp, 0),
    ]
    .into_iter();
    for (slot, (kind, variant)) in specs.enumerate() {
        let home = [slot as i64, 0];
        let Some(id) = creature_id(home, 0) else {
            continue;
        };
        let position = [
            4.0 + (slot % 4) as f64 * 3.0,
            32.3 + if kind.is_flying() { 0.7 } else { 0.0 },
            19.0 + (slot / 4) as f64 * 5.0,
        ];
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

pub(super) fn run(world: &mut World) {
    let stage = world.resource::<Inspection>().captured;
    let state = world.resource::<Inspection>();
    if state.failed || state.start.elapsed() > Duration::from_secs(120) {
        error!("Forest creature gallery failed or timed out");
        world.write_message(AppExit::error());
        return;
    }
    if stage >= 6 {
        if state.last.elapsed() >= Duration::from_millis(500) {
            info!(
                "Forest gallery rendered three Skeleton loadouts, Raven, Blood Slime, Blood Stalker, Blood Wisp and Thorn Brute in idle/walk/attack poses plus two guardian close-ups; six native views, visual fixture, not an AI test"
            );
            world.write_message(AppExit::Success);
        }
        return;
    }
    let animation_clock = world.resource::<Inspection>().start.elapsed().as_secs_f64();
    for track in world
        .resource_mut::<ReplicatedCreatures>()
        .creatures
        .values_mut()
    {
        track.current.behavior = if stage == 2 && track.current.kind.hostile_to_players() {
            CreatureBehavior::Attack
        } else if track.current.kind.is_flying() {
            CreatureBehavior::Fly
        } else if stage == 1 {
            CreatureBehavior::Wander
        } else {
            CreatureBehavior::Rest
        };
        // Hold the real animation's wind-up for the close-up. Gameplay attack
        // timers are untouched; this fixture supplies replicated pose input.
        track.current.timer = if stage == 2 {
            terra_core::creatures::definitions::get(track.current.kind)
                .combat
                .map_or(0.32, |combat| combat.cooldown * 0.70)
        } else {
            1.0
        };
        let slot = track.current.home[0] as usize;
        track.current.position[0] = 4.0
            + (slot % 4) as f64 * 3.0
            + if stage == 1 {
                (animation_clock * 2.0).sin() * 0.3
            } else {
                0.0
            };
        track.current.velocity = if stage == 1 {
            [0.0, 0.0, 1.0]
        } else {
            [0.0; 3]
        };
        track.previous = track.current.clone();
        track.received = Instant::now();
    }
    let eye = match stage {
        0 => Transform::from_xyz(8.5, 38.0, 34.0).looking_at(Vec3::new(8.5, 33.3, 21.0), Vec3::Y),
        1 => Transform::from_xyz(7.0, 34.1, 23.7).looking_at(Vec3::new(7.0, 33.55, 19.0), Vec3::Y),
        2 => {
            Transform::from_xyz(10.8, 34.0, 22.8).looking_at(Vec3::new(10.0, 33.45, 19.0), Vec3::Y)
        }
        3 => Transform::from_xyz(8.05, 33.2, 26.7).looking_at(Vec3::new(7.0, 32.62, 24.0), Vec3::Y),
        4 => {
            Transform::from_xyz(13.0, 34.15, 23.9).looking_at(Vec3::new(13.0, 33.6, 19.0), Vec3::Y)
        }
        _ => {
            Transform::from_xyz(16.0, 34.35, 21.7).looking_at(Vec3::new(13.0, 33.65, 19.0), Vec3::Y)
        }
    };
    capture(world, stage, eye, 8, "forest-creatures");
}

pub(super) fn capture(world: &mut World, stage: usize, eye: Transform, count: usize, prefix: &str) {
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
        || avatars != count
        || state.start.elapsed() < Duration::from_secs(8)
        || state.requested != state.captured
        || state.last.elapsed() < Duration::from_secs(3)
    {
        return;
    }
    let folder = PathBuf::from(
        std::env::var("TERRA_CAPTURE_DIR").unwrap_or_else(|_| format!("/tmp/terra-{prefix}")),
    );
    if let Err(error) = std::fs::create_dir_all(&folder) {
        error!(%error, "Cannot create creature capture directory");
        world.write_message(AppExit::error());
        return;
    }
    world.resource_mut::<Inspection>().requested += 1;
    world
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(folder.join(format!("{prefix}-{stage}.png"))))
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
