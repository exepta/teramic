//! Native inspection of the two Savannah grazers and their articulated heads and legs.
use super::*;
use terra_voxel::MaterialId;

pub(super) fn setup(
    replicas: &mut ReplicatedCreatures,
    journal: &mut terra_world::edits::EditJournal,
) {
    for x in 1..17 {
        for z in 16..29 {
            journal
                .record([x, 31, z], MaterialId::SavannaTurf as u16)
                .unwrap();
            for y in 32..40 {
                journal.record([x, y, z], 0).unwrap();
            }
        }
    }
    for (slot, kind, x) in [
        (0, CreatureKind::Elephant, 6.),
        (1, CreatureKind::Zebra, 10.1),
    ] {
        let id = creature_id([0, 0], slot).unwrap();
        let animal = Creature {
            id,
            kind,
            home: [0, 0],
            position: [x, 32., 21.],
            velocity: [0.; 3],
            yaw: 2.25,
            health: kind.health(),
            age: 64.,
            behavior: CreatureBehavior::Rest,
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
        error!("Savanna creature inspection failed or timed out");
        world.write_message(AppExit::error());
        return;
    }
    if stage >= 2 {
        info!("Rendered adult elephant and zebra at rest and grazing");
        world.write_message(AppExit::Success);
        return;
    }
    let elapsed = state.start.elapsed().as_secs_f64();
    for track in world
        .resource_mut::<ReplicatedCreatures>()
        .creatures
        .values_mut()
    {
        track.current.behavior = if stage == 0 {
            CreatureBehavior::Rest
        } else {
            CreatureBehavior::Graze
        };
        track.current.age = 64. + elapsed;
        track.received = Instant::now();
        track.previous = track.current.clone();
    }
    let eye = if stage == 0 {
        Transform::from_xyz(8.45, 34.1, 27.).looking_at(Vec3::new(8., 33.35, 20.9), Vec3::Y)
    } else {
        Transform::from_xyz(10.9, 33.63, 24.).looking_at(Vec3::new(9.25, 32.90, 20.95), Vec3::Y)
    };
    forest::capture(world, stage, eye, 2, "savanna-wildlife");
}
