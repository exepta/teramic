//! Pose inspection of the normal cached guardian meshes, materials and animation.
use super::*;
use terra_voxel::MaterialId as M;
pub(super) fn setup(
    replicas: &mut ReplicatedCreatures,
    journal: &mut terra_world::edits::EditJournal,
) {
    for x in -1i64..24 {
        for z in 11i64..36 {
            journal
                .record(
                    [x, 31, z],
                    if (x - z).rem_euclid(7) < 2 {
                        M::CrackedStoneSoil
                    } else {
                        M::DustyGround
                    } as u16,
                )
                .unwrap();
            for y in 32..41 {
                journal.record([x, y, z], 0).unwrap();
            }
        }
    }
    for (x, z, m) in [
        (2, 21, M::ShatteredStoneRoot),
        (17, 19, M::BrokenPetrifiedStump),
        (15, 18, M::DarkMushrooms),
        (3, 18, M::BloodMushrooms),
        (7, 17, M::PetrifiedRubble),
    ] {
        journal.record([x, 32, z], m as u16).unwrap();
    }
    for (variant, x) in [(0, 6.5), (1, 12.6)] {
        let home = [0, 0];
        let id = creature_id(home, variant).unwrap();
        let kind = CreatureKind::StoneGolem;
        let a = Creature {
            id,
            kind,
            home,
            position: [x, 32., 20.],
            velocity: [0.; 3],
            yaw: if variant == 0 { 3.72 } else { 3.00 },
            health: kind.health(),
            behavior: CreatureBehavior::Sleep,
            timer: 0.,
            age: 20.,
            variant,
            needs: CreatureNeeds::full(kind),
        };
        replicas.creatures.insert(
            id,
            CreatureTrack {
                previous: a.clone(),
                current: a,
                received: Instant::now(),
                interval: 0.1,
                playback: Default::default(),
            },
        );
    }
}
pub(super) fn run(world: &mut World) {
    use CreatureBehavior as B;
    let state = world.resource::<Inspection>();
    let stage = state.captured;
    if state.failed || state.start.elapsed() > Duration::from_secs(120) {
        error!("Stone guardian inspection timed out");
        world.write_message(AppExit::error());
        return;
    }
    if stage >= 7 {
        info!(
            "Stone guardian rubble, two stages of runic assembly, standing, strike, ground slam and walking captured"
        );
        world.write_message(AppExit::Success);
        return;
    }
    let elapsed = state.last.elapsed().as_secs_f32();
    for track in world
        .resource_mut::<ReplicatedCreatures>()
        .creatures
        .values_mut()
    {
        let (action, phase) = match stage {
            0 => (B::Sleep, 0.),
            1 => (B::Emerge, 0.30),
            2 => (B::Emerge, 0.61),
            4 => (B::Attack, 0.28),
            5 => (B::Tremor, 0.64),
            6 => (B::Wander, 0.),
            _ => (B::Hunt, 0.),
        };
        if stage == 6 {
            let base = if track.current.variant == 0 {
                6.5
            } else {
                12.6
            };
            let stride = (elapsed.min(5.) - 2.5) * 0.75;
            track.current.position[0] = base + f64::from(stride) * 0.3;
            track.current.position[2] = 20. + f64::from(stride) * 0.9;
            track.current.velocity = [0.225, 0., 0.675];
        }
        track.current.behavior = action;
        track.current.timer = terra_core::creatures::golem::duration(action) * (1. - phase);
        track.received = Instant::now();
        track.previous = track.current.clone();
    }
    let eye = if stage == 6 {
        Transform::from_xyz(10.1, 34.15, 26.1).looking_at(Vec3::new(9.5, 33.85, 20.), Vec3::Y)
    } else {
        Transform::from_xyz(9.5, 34.65, 25.0).looking_at(Vec3::new(6.5, 33.65, 20.), Vec3::Y)
    };
    forest::capture(world, stage, eye, 2, "stone-golem");
}
