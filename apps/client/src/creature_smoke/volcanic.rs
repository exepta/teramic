//! Native inspection with real edited lava, ordinary replicated tracks and the gameplay meshes.
use super::*;
use terra_voxel::MaterialId as M;

pub(super) fn setup(
    replicas: &mut ReplicatedCreatures,
    journal: &mut terra_world::edits::EditJournal,
) {
    for x in -3..=18 {
        for z in 12..=30 {
            let pool = 2 * (x - 6) * (x - 6) + (z - 20) * (z - 20) < 56;
            for y in 28..=40 {
                let material = match y {
                    28 => M::ColumnarBasalt,
                    29..=31 if pool => M::Lava,
                    29..=31 => M::CompactedAsh,
                    _ => M::Air,
                };
                journal.record([x, y, z], material as u16).unwrap();
            }
        }
    }
    for (slot, kind, x) in [
        (0, CreatureKind::LavaWorm, 6.5),
        (1, CreatureKind::Skeleton, 12.5),
    ] {
        let id = creature_id([0, 0], slot).unwrap();
        let animal = Creature {
            id,
            kind,
            home: [0, 0],
            position: [x, 32., 20.5],
            velocity: [0.; 3],
            yaw: 2.75,
            health: kind.health(),
            age: 1.,
            behavior: CreatureBehavior::Hold,
            timer: 1.,
            variant: 0,
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
    let state = world.resource::<Inspection>();
    let stage = state.captured;
    if state.failed || state.start.elapsed() > Duration::from_secs(120) {
        error!("Volcanic creature gallery failed or timed out");
        world.write_message(AppExit::error());
        return;
    }
    if stage >= 6 {
        info!(
            "Rendered lava worm breach, swimming wake, charged fireball and slam, plus two skeleton emergence stages"
        );
        world.write_message(AppExit::Success);
        return;
    }
    let swim_time = state.last.elapsed().as_secs_f32().min(3.8);
    for track in world
        .resource_mut::<ReplicatedCreatures>()
        .creatures
        .values_mut()
    {
        if stage == 3 && track.current.kind == CreatureKind::LavaWorm {
            track.current.behavior = CreatureBehavior::Swim;
            track.current.timer = 6. - swim_time;
            track.current.age = 60. + f64::from(swim_time);
            track.current.yaw = std::f32::consts::PI;
            track.current.position = [5.5, 32., 19.6 + f64::from(swim_time) * 1.5];
            track.current.velocity = [0., 0., 1.5];
        }
        if stage >= 4 && track.current.kind == CreatureKind::LavaWorm {
            use terra_core::creatures::lava_worm as worm;
            track.current.position = [6.5, 32., 20.5];
            track.current.yaw = 2.75;
            track.current.velocity = [0.; 3];
            track.current.age = 70.;
            if stage == 4 {
                track.current.behavior = CreatureBehavior::Spit;
                track.current.timer = worm::SPIT - worm::RELEASE - 0.055;
                track.current.needs.goal = Some(NeedGoal {
                    kind: NeedKind::AttackAim,
                    position: [5, 34, 26],
                });
            } else {
                track.current.behavior = CreatureBehavior::Attack;
                track.current.timer = worm::SLAM - worm::IMPACT;
            }
        }
        if track.current.kind == CreatureKind::Skeleton {
            track.current.behavior = CreatureBehavior::Emerge;
            track.current.timer = SKELETON_RISE_SECONDS * (if stage == 1 { 0.47 } else { 0.04 });
        }
        track.received = Instant::now();
        track.previous = track.current.clone();
    }
    let eye = if stage >= 4 {
        Transform::from_xyz(10.45, 34.2, 25.6).looking_at(Vec3::new(6.0, 33.18, 21.55), Vec3::Y)
    } else if stage == 0 {
        Transform::from_xyz(6.45, 35.8, 24.4).looking_at(Vec3::new(6.2, 33.5, 21.1), Vec3::Y)
    } else if stage == 3 {
        Transform::from_xyz(11.6, 36.2, 24.5).looking_at(Vec3::new(5.5, 32.1, 21.35), Vec3::Y)
    } else {
        Transform::from_xyz(13.65, 33.38, 23.1).looking_at(Vec3::new(12.5, 32.8, 20.5), Vec3::Y)
    };
    forest::capture(world, stage, eye, 2, "ashlands-creatures");
}
