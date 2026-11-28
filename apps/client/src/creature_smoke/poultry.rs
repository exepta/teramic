//! Render every plumage and capture a close view of the pecking animation.
use super::*;

pub(super) fn setup(
    replicas: &mut ReplicatedCreatures,
    journal: &mut terra_world::edits::EditJournal,
) {
    for x in 3..15 {
        for z in 16..27 {
            journal
                .record([x, 31, z], terra_voxel::MaterialId::Grass as u16)
                .unwrap();
            for y in 32..38 {
                journal.record([x, y, z], 0).unwrap();
            }
        }
    }
    for (kind, start, spacing, z) in [
        (CreatureKind::Hen, 5.4, 1.04, 20.4),
        (CreatureKind::Rooster, 6.25, 1.2, 22.3),
    ] {
        for variant in 0..kind.variant_count() {
            let home = [i64::from(variant), 0];
            let slot = u8::from(kind == CreatureKind::Hen);
            let id = poultry_id(home, slot).unwrap();
            let bird = Creature {
                id,
                kind,
                home,
                variant,
                position: [start + f64::from(variant) * spacing, 32.0, z],
                yaw: 2.5,
                velocity: [0.0; 3],
                health: kind.health(),
                behavior: CreatureBehavior::Rest,
                age: 32.0,
                timer: 1.0,
                needs: CreatureNeeds::full(kind),
            };
            replicas.creatures.insert(
                id,
                CreatureTrack {
                    previous: bird.clone(),
                    current: bird,
                    received: Instant::now(),
                    interval: 0.1,
                    playback: Default::default(),
                },
            );
        }
    }
}

pub(super) fn run(world: &mut World) {
    let state = world.resource::<Inspection>();
    if state.failed || state.start.elapsed() > Duration::from_secs(120) {
        error!("Poultry model inspection failed or timed out");
        world.write_message(AppExit::error());
        return;
    }
    let stage = state.captured;
    if stage >= 3 {
        info!("Rendered six hens and four roosters with pecking animation");
        world.write_message(AppExit::Success);
        return;
    }
    for track in world
        .resource_mut::<ReplicatedCreatures>()
        .creatures
        .values_mut()
    {
        track.current.behavior = if stage == 2 {
            CreatureBehavior::Graze
        } else {
            CreatureBehavior::Rest
        };
        track.received = Instant::now();
        track.previous = track.current.clone();
    }
    let (eye, aim) = match stage {
        0 => (Vec3::new(7.9, 33.5, 24.7), Vec3::new(8.0, 32.34, 20.7)),
        1 => (Vec3::new(8.05, 33.4, 26.0), Vec3::new(8.05, 32.40, 22.3)),
        _ => (Vec3::new(7.4, 32.95, 23.85), Vec3::new(6.7, 32.30, 21.9)),
    };
    forest::capture(
        world,
        stage,
        Transform::from_translation(eye).looking_at(aim, Vec3::Y),
        10,
        "poultry",
    );
}
