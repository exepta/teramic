//! Native view of the three hut residents using the gameplay rigs and stable identities.
use super::*;
use terra_core::creatures::villagers::{SWAMP_HERMIT, SWAMP_WARLOCK, SWAMP_WITCH, swamp_id};
use terra_world::edits::EditJournal;

pub(super) fn setup(replicas: &mut ReplicatedCreatures, journal: &mut EditJournal) {
    for x in 4..=12 {
        for z in 18..=24 {
            journal
                .record([x, 31, z], terra_voxel::MaterialId::WillowPlanks as u16)
                .unwrap();
            for y in 32..=36 {
                journal.record([x, y, z], 0).unwrap();
            }
        }
    }
    for (index, role) in [SWAMP_WITCH, SWAMP_WARLOCK, SWAMP_HERMIT]
        .into_iter()
        .enumerate()
    {
        let home = [index as i64, 0];
        let id = swamp_id(home, role).unwrap();
        let resident = Creature {
            id,
            kind: CreatureKind::Villager,
            home,
            position: [6.6 + index as f64 * 1.4, 32., 21.5],
            velocity: [0.; 3],
            yaw: std::f32::consts::PI - 0.18,
            health: CreatureKind::Villager.health(),
            behavior: CreatureBehavior::Rest,
            timer: 1.,
            age: 32.,
            variant: role,
            needs: CreatureNeeds::full(CreatureKind::Villager),
        };
        replicas.creatures.insert(
            id,
            CreatureTrack {
                previous: resident.clone(),
                current: resident,
                received: Instant::now(),
                interval: 0.1,
                playback: Default::default(),
            },
        );
    }
}

pub(super) fn run(world: &mut World) {
    let state = world.resource::<Inspection>();
    if state.failed || state.start.elapsed() > Duration::from_secs(100) {
        error!("Swamp resident inspection timed out or produced a blank image");
        world.write_message(AppExit::error());
    } else if state.captured > 0 {
        if state.last.elapsed() > Duration::from_millis(500) {
            world.write_message(AppExit::Success);
        }
    } else {
        let eye =
            Transform::from_xyz(8., 33.3, 25.1).looking_at(Vec3::new(8., 33.04, 21.5), Vec3::Y);
        forest::capture(world, 0, eye, 3, "swamp-residents");
    }
}
