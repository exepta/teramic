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

#[test]
fn skeleton_cannot_chase_or_hit_until_the_dig_out_is_complete() {
    let (terrain, chunks, journal) = fixture();
    let habitat = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    let mut a = animal(Kind::Skeleton);
    a.behavior = Behavior::Emerge;
    a.timer = terra_core::creatures::SKELETON_RISE_SECONDS;
    let start = a.position;
    let target = [a.position[0], a.position[1] + 0.9, a.position[2] - 0.9];
    for _ in 0..32 {
        assert_eq!(
            step(&mut a, &habitat, &[target], 0.10, true, None),
            Effect::None
        );
        assert_eq!(a.position, start);
        assert_eq!(a.behavior, Behavior::Emerge);
    }
    assert!(terra_core::creatures::skeleton_rise(a.timer) > 0.94);
    for _ in 0..5 {
        step(&mut a, &habitat, &[target], 0.1, true, None);
    }
    assert_eq!(a.behavior, Behavior::Attack);
}

#[test]
fn stepping_behind_a_committed_guardian_forces_a_fresh_windup() {
    let (terrain, chunks, journal) = fixture();
    let h = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    for kind in [
        Kind::Skeleton,
        Kind::BloodStalker,
        Kind::BloodWisp,
        Kind::ThornBrute,
    ] {
        let mut a = animal(kind);
        let front = [8.5, 1.9, 7.5];
        let behind = [8.5, 1.9, 9.5];
        for _ in 0..3 {
            assert_eq!(step(&mut a, &h, &[front], 0.1, true, None), Effect::None);
        }
        assert_eq!(a.behavior, Behavior::Attack);
        assert_eq!(step(&mut a, &h, &[behind], 0.1, true, None), Effect::None);
        assert_eq!(a.behavior, Behavior::Hunt);
        assert_eq!(step(&mut a, &h, &[behind], 0.1, true, None), Effect::None);
        assert_eq!(a.timer, definitions::get(kind).combat.unwrap().cooldown);
    }
}

#[test]
fn wisp_flies_round_a_pillar_under_a_low_roof() {
    let (terrain, chunks, mut journal) = fixture();
    for x in 4..=12 {
        for z in 2..=12 {
            journal.record([x, 4, z], 3).unwrap();
        }
    }
    for y in 1..4 {
        journal.record([8, y, 6], 3).unwrap();
    }
    let h = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    let mut a = animal(Kind::BloodWisp);
    a.position[1] = 2.8;
    a.behavior = Behavior::Hunt;
    let target = [8.5, 2.8, 3.5];
    let mut sidestepped = false;
    for _ in 0..70 {
        fly_toward(&mut a, &h, target, 0.1);
        assert!(h.clear(a.position, a.size(), false));
        sidestepped |= (a.position[0] - 8.5).abs() > 0.6;
    }
    assert!(
        sidestepped && distance(a.position, target) < 0.25,
        "wisp ended at {:?}",
        a.position
    );
}

#[test]
fn stone_guardian_wakes_then_strikes_visible_intruders_and_returns_to_its_perch() {
    use crate::creatures::golem;
    let (t, c, j) = fixture();
    let h = Habitat {
        terrain: &t,
        chunks: &c,
        journal: &j,
    };
    let mut a = animal(Kind::StoneGolem);
    a.behavior = Behavior::Sleep;
    let close = [8.5, 1.9, 6.4];
    let away = [30.5, 1.9, 8.5];
    let mut hits = Vec::new();
    golem::step(&mut a, &h, &[away], 0.1, true, None, |i, _| hits.push(i));
    assert_eq!(a.behavior, Behavior::Sleep);
    golem::step(&mut a, &h, &[close], 0.1, true, None, |i, _| hits.push(i));
    assert_eq!(a.behavior, Behavior::Emerge);
    for _ in 0..18 {
        golem::step(&mut a, &h, &[close], 0.1, true, None, |i, _| hits.push(i));
    }
    assert_eq!(a.behavior, Behavior::Emerge);
    assert!(hits.is_empty());
    for _ in 0..140 {
        golem::step(&mut a, &h, &[close], 0.1, true, None, |i, _| hits.push(i));
    }
    assert!((3..=6).contains(&hits.len()));
    assert!(a.valid());
    let anchor = a.needs.goal.unwrap();
    a.position[0] += 5.;
    a.behavior = Behavior::Hunt;
    for _ in 0..140 {
        golem::step(&mut a, &h, &[away], 0.1, true, None, |_, _| {
            panic!("outside territory")
        });
    }
    assert_eq!(a.needs.goal, Some(anchor));
    assert_eq!(a.behavior, Behavior::Sleep);
    assert!((a.position[0] - 8.5).abs() < 1.);
}
#[test]
fn stone_guardian_slam_has_one_visible_area_impact_and_can_be_dodged() {
    use crate::creatures::golem;
    let (t, mut c, j) = fixture();
    let section = terra_voxel::SectionPos::new(0, 0, 0);
    for y in 1..=5 {
        for x in 5..=11 {
            c.loaded
                .get_mut(&section)
                .unwrap()
                .data
                .set_block([x, y, 11], 3)
                .unwrap();
        }
    }
    let h = Habitat {
        terrain: &t,
        chunks: &c,
        journal: &j,
    };
    let mut a = animal(Kind::StoneGolem);
    a.behavior = Behavior::Tremor;
    a.timer = terra_core::creatures::golem::SLAM_SECONDS;
    let mut hits = Vec::new();
    let points = [
        [8.5, 1.9, 6.5],
        [11., 1.9, 8.5],
        [8.5, 1.9, 12.5],
        [8.5, 1.9, 1.5],
    ];
    for _ in 0..31 {
        golem::step(&mut a, &h, &points, 0.1, true, None, |i, _| hits.push(i));
    }
    assert_eq!(hits, [0, 1]);
}

#[test]
fn stone_guardian_appears_singly_on_loaded_fossil_soil_and_keeps_its_identity() {
    use crate::{
        SectionGenerator,
        creatures::{spawn_cell, test_habitat::load_surface},
    };
    use terra_core::creatures::CreaturePopulation;
    let terrain = TerraGenerator::new(WorldConfig::default()).unwrap();
    let generator = SectionGenerator::new(terrain.clone());
    let journal = EditJournal::default();
    for x in -30..-9 {
        for z in 10..35 {
            let home = [x, z];
            if !crate::creatures::golem::habitat_cell(home, terrain.config().seed.0) {
                continue;
            }
            let [cx, cz] = home.map(|n| n * 32 + 12);
            let col = terrain.surface_column(cx as f64, cz as f64);
            if col.biome.weights.petrified_forest < 0.72 {
                continue;
            }
            let chunks = load_surface(&generator, home);
            let h = Habitat {
                terrain: &terrain,
                chunks: &chunks,
                journal: &journal,
            };
            let mut population = CreaturePopulation::default();
            population.explored.insert(home); // A region first visited before guardians existed.
            let player = [cx as f64 + 19., col.height + 2., cz as f64];
            if !crate::creatures::spawn_cell_away_from_players(
                &mut population,
                &h,
                home,
                &[player],
                1,
            ) {
                continue;
            }
            let mut guardians = population
                .animals
                .values()
                .filter(|a| a.kind == Kind::StoneGolem);
            if let Some(a) = guardians.next() {
                assert!(a.valid() && a.behavior == Behavior::Sleep);
                assert!(guardians.next().is_none());
                let id = a.id;
                let old = population.clone();
                spawn_cell(&mut population, &h, home);
                assert_eq!(old, population);
                let a = population.animals.get_mut(&id).unwrap();
                let perch = a.position;
                let mut roaming = 0_f64;
                let mut nav = crate::creatures::navigation::Navigation::default();
                for n in 0..330 {
                    crate::creatures::golem::step(
                        a,
                        &h,
                        &[],
                        0.10,
                        n % 6 == 0,
                        Some(&mut nav),
                        |_, _| unreachable!(),
                    );
                    roaming =
                        roaming.max((a.position[0] - perch[0]).hypot(a.position[2] - perch[2]));
                }
                assert!(
                    (1.5..10.).contains(&roaming),
                    "golem patrol radius {roaming} at {perch:?}"
                );
                return;
            }
        }
    }
    panic!("no natural guardian on loaded petrified soil");
}

#[test]
fn committed_melee_keeps_the_exposed_front_target_when_a_closer_enemy_flanks_it() {
    let (terrain, chunks, journal) = fixture();
    let habitat = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    for kind in [
        Kind::Skeleton,
        Kind::BloodStalker,
        Kind::BloodWisp,
        Kind::ThornBrute,
    ] {
        let mut a = animal(kind);
        let front = [8.5, 1.9, 7.2];
        let behind = [8.5, 1.9, 9.3];
        assert_eq!(
            step(&mut a, &habitat, &[front], 0.05, true, None),
            Effect::None
        );
        assert_eq!(a.behavior, Behavior::Attack);
        let duration = definitions::get(kind).combat.unwrap().cooldown;
        let mut hits = 0;
        for _ in 0..(duration / 0.05).floor() as usize {
            let effect = step(&mut a, &habitat, &[behind, front], 0.05, false, None);
            assert_eq!(
                a.behavior,
                Behavior::Attack,
                "{kind:?} cancelled its committed swing"
            );
            if let Effect::StrikePlayer { index, .. } = effect {
                assert_eq!(index, 1);
                hits += 1;
            }
        }
        assert_eq!(hits, 1, "{kind:?}");
    }
}
