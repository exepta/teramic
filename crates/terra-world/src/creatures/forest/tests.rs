use super::*;
use crate::{
    TerraGenerator,
    edits::EditJournal,
    streaming::{ChunkStore, LoadedSection},
};
use terra_core::{
    WorldConfig,
    creatures::{CreatureNeeds, creature_id},
};

fn fixture() -> (TerraGenerator, ChunkStore, EditJournal) {
    let terrain = TerraGenerator::new(WorldConfig {
        caves: false,
        trees: false,
        biomes: false,
        water: false,
        ..Default::default()
    })
    .unwrap();
    let mut chunks = ChunkStore::default();
    for x in -1..=1 {
        for z in -1..=1 {
            let position = terra_voxel::SectionPos::new(x, 0, z);
            let mut data = terra_voxel::BlockSection::empty(position).unwrap();
            for x in 0..16 {
                for z in 0..16 {
                    data.set_block([x, 0, z], 3).unwrap();
                }
            }
            chunks.loaded.insert(
                position,
                LoadedSection {
                    data,
                    mesh: None,
                    mesh_epoch: 0,
                    water_epoch: 0,
                    water: None,
                    props: Vec::new(),
                },
            );
        }
    }
    (terrain, chunks, EditJournal::default())
}
fn animal(kind: Kind) -> Creature {
    Creature {
        id: creature_id([0, 0], 0).unwrap(),
        kind,
        home: [0, 0],
        position: [8.5, 1.0, 8.5],
        velocity: [0.0; 3],
        yaw: 0.0,
        health: kind.health(),
        behavior: Behavior::Rest,
        timer: 0.0,
        age: 0.0,
        variant: 0,
        needs: CreatureNeeds::full(kind),
    }
}

#[test]
fn natural_selector_is_deterministic_and_exclusive_to_blood_forest() {
    let found: std::collections::BTreeSet<_> = (0..100)
        .filter_map(|n| surface_species("blood_forest", n << 48))
        .collect();
    assert_eq!(
        found,
        [
            Kind::Skeleton,
            Kind::Raven,
            Kind::BloodSlime,
            Kind::BloodStalker,
            Kind::BloodWisp,
            Kind::ThornBrute
        ]
        .into()
    );
    for biome in ["forest", "plains", "desert", "ocean", "spirit_blossom"] {
        assert!(surface_species(biome, 0).is_none());
    }
    assert!(!hostile(Kind::Raven) && !hostile(Kind::BloodSlime));
}

#[test]
fn hostile_species_approach_visible_survival_target_and_strike_with_cooldown() {
    let (t, c, j) = fixture();
    let h = Habitat {
        terrain: &t,
        chunks: &c,
        journal: &j,
    };
    for kind in [
        Kind::Skeleton,
        Kind::BloodStalker,
        Kind::BloodWisp,
        Kind::ThornBrute,
    ] {
        let mut a = animal(kind);
        let target = [8.5, 1.9, 4.5];
        let start = distance(a.position, target);
        let mut nav = navigation::Navigation::default();
        let mut hits = Vec::new();
        for tick in 0..100 {
            if let Effect::StrikePlayer { index, damage } =
                step(&mut a, &h, &[target], 0.1, true, Some(&mut nav))
            {
                assert_eq!(index, 0);
                assert!(damage > 0.0);
                hits.push(tick);
            }
            assert!(a.valid(), "{kind:?}: {a:?}");
        }
        assert!(
            distance(a.position, target) < start,
            "{kind:?} must approach"
        );
        assert!(hits.len() >= 2, "{kind:?} must attack");
        assert!(
            hits.windows(2).all(|v| v[1] - v[0] >= 12),
            "{kind:?} cooldown"
        );
    }
}

#[test]
fn walls_unknown_chunks_and_invalid_time_never_allow_damage_or_teleport() {
    let (t, c, mut j) = fixture();
    for x in 0..16 {
        for y in 1..8 {
            j.record([x, y, 7], 3).unwrap();
        }
    }
    let h = Habitat {
        terrain: &t,
        chunks: &c,
        journal: &j,
    };
    for kind in [
        Kind::Skeleton,
        Kind::BloodStalker,
        Kind::BloodWisp,
        Kind::ThornBrute,
    ] {
        let mut a = animal(kind);
        let original = a.clone();
        for dt in [0.0, -0.1, f32::NAN, 1.0] {
            assert_eq!(
                step(&mut a, &h, &[[8.5, 1.9, 6.5]], dt, true, None),
                Effect::None
            );
            assert_eq!(a, original);
        }
        for _ in 0..30 {
            assert_eq!(
                step(&mut a, &h, &[[8.5, 1.9, 6.5]], 0.1, false, None),
                Effect::None
            );
            assert!(a.position[2] >= 8.0, "{kind:?} crossed wall");
        }
    }
}

#[test]
fn peaceful_fauna_never_use_the_hostile_attack_path() {
    let (t, c, j) = fixture();
    let h = Habitat {
        terrain: &t,
        chunks: &c,
        journal: &j,
    };
    for kind in [Kind::Raven, Kind::BloodSlime] {
        let mut a = animal(kind);
        let original = a.clone();
        assert_eq!(
            step(&mut a, &h, &[[8.5, 1.9, 8.0]], 0.1, true, None),
            Effect::None
        );
        assert_eq!(a, original);
    }
}

#[test]
fn melee_windup_is_visible_can_be_dodged_and_does_not_bank_an_instant_hit() {
    let (t, c, j) = fixture();
    let h = Habitat {
        terrain: &t,
        chunks: &c,
        journal: &j,
    };
    for kind in [
        Kind::Skeleton,
        Kind::ThornBrute,
        Kind::BloodStalker,
        Kind::BloodWisp,
    ] {
        let mut a = animal(kind);
        let close = [8.5, 1.9, 7.5];
        let combat = definitions::get(kind).combat.unwrap();
        assert_eq!(step(&mut a, &h, &[close], 0.1, true, None), Effect::None);
        assert_eq!(a.behavior, Behavior::Attack);
        assert_eq!(a.timer, combat.cooldown);
        for _ in 0..2 {
            assert_eq!(step(&mut a, &h, &[close], 0.1, true, None), Effect::None);
            assert_eq!(a.velocity, [0.0; 3]);
        }
        // Leave during the telegraph, then immediately approach again. The
        // cancelled wind-up cannot cause damage on this new arrival.
        assert_eq!(
            step(&mut a, &h, &[[8.5, 1.9, 2.0]], 0.1, true, None),
            Effect::None
        );
        assert_eq!(a.behavior, Behavior::Hunt);
        let close = [a.position[0], a.position[1] + 0.9, a.position[2] - 0.5];
        assert_eq!(step(&mut a, &h, &[close], 0.1, true, None), Effect::None);
        assert_eq!(a.timer, combat.cooldown);
        let mut strikes = 0;
        for _ in 0..(combat.cooldown / 0.1) as usize {
            if let Effect::StrikePlayer { .. } = step(&mut a, &h, &[close], 0.1, true, None) {
                assert!(combat.melee_phase(a.timer) >= 0.42 - 1e-5);
                assert!(combat.melee_phase(a.timer) < 0.60);
                strikes += 1;
            }
        }
        assert_eq!(strikes, 1, "{kind:?}: one hit, then visible recovery");
    }
}

#[test]
fn distant_hostiles_run_faster_than_their_patrol_and_keep_supported_movement() {
    let (t, c, j) = fixture();
    let h = Habitat {
        terrain: &t,
        chunks: &c,
        journal: &j,
    };
    for kind in [Kind::Skeleton, Kind::ThornBrute, Kind::BloodStalker] {
        let mut a = animal(kind);
        let before = a.position;
        assert_eq!(
            step(&mut a, &h, &[[8.5, 1.9, 1.0]], 0.1, true, None),
            Effect::None
        );
        let horizontal = (f64::from(a.velocity[0])).hypot(f64::from(a.velocity[2]));
        assert!(horizontal > kind.speed() * 2.0, "{kind:?}: {horizontal}");
        assert!(distance(before, a.position).sqrt() < kind.speed() * 0.3);
        assert!(
            h.supported_sized(a.position, a.size(), f64::from(a.yaw))
                .is_some()
        );
    }
}
