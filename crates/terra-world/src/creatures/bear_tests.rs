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
use terra_voxel::{BlockSection, SectionPos};

fn fixture() -> (TerraGenerator, ChunkStore, EditJournal) {
    let terrain = TerraGenerator::new(WorldConfig {
        base_height: 3.0,
        hill_amplitude: 0.0,
        mountain_amplitude: 0.0,
        caves: false,
        trees: false,
        biomes: false,
        rivers: false,
        water: false,
        ..Default::default()
    })
    .unwrap();
    let mut chunks = ChunkStore::default();
    for z in -1..=1 {
        for x in -1..=1 {
            let p = SectionPos::new(x, 0, z);
            let mut data = BlockSection::empty(p).unwrap();
            for x in 0..16 {
                for z in 0..16 {
                    for y in 0..3 {
                        data.set_block([x, y, z], 1).unwrap();
                    }
                }
            }
            chunks.loaded.insert(
                p,
                LoadedSection {
                    data,
                    mesh: None,
                    mesh_epoch: 0,
                    water_epoch: 0,
                    water: None,
                    props: vec![],
                },
            );
        }
    }
    (terrain, chunks, EditJournal::default())
}
fn bear() -> Creature {
    Creature {
        id: creature_id([0, 0], 0).unwrap(),
        home: [0, 0],
        kind: Kind::Bear,
        position: [8.5, 3.0, 8.5],
        velocity: [0.0; 3],
        yaw: 0.0,
        health: Kind::Bear.health(),
        behavior: Behavior::Rest,
        timer: 0.0,
        age: 0.0,
        variant: 0,
        needs: CreatureNeeds::full(Kind::Bear),
    }
}

#[test]
fn bear_attacks_require_proximity_clear_voxels_and_a_real_cooldown() {
    let (t, c, mut j) = fixture();
    let mut a = bear();
    let player = [8.5, 3.8, 7.2];
    let mut hits = 0;
    for _ in 0..35 {
        let h = Habitat {
            terrain: &t,
            chunks: &c,
            journal: &j,
        };
        if matches!(
            step(
                &mut a,
                &h,
                &[player],
                &PreyIndex::default(),
                0.1,
                false,
                false
            ),
            Effect::StrikePlayer {
                index: 0,
                damage: 12.0
            }
        ) {
            hits += 1;
        }
        assert!(a.valid());
    }
    assert!((2..=3).contains(&hits));
    for y in 3..=5 {
        for x in 6..=10 {
            j.record([x, y, 7], 3).unwrap();
        }
    }
    let h = Habitat {
        terrain: &t,
        chunks: &c,
        journal: &j,
    };
    for _ in 0..40 {
        assert_eq!(
            step(
                &mut a,
                &h,
                &[player],
                &PreyIndex::default(),
                0.1,
                false,
                false
            ),
            Effect::None
        );
    }
    assert_eq!(
        step(
            &mut a,
            &h,
            &[[100.0, 3.0, 100.0]],
            &PreyIndex::default(),
            0.1,
            false,
            false
        ),
        Effect::None
    );
}

#[test]
fn hunting_starts_at_fifteen_and_consumes_only_one_live_reachable_fish() {
    let (t, c, j) = fixture();
    let h = Habitat {
        terrain: &t,
        chunks: &c,
        journal: &j,
    };
    let mut a = bear();
    let mut fish = a.clone();
    fish.id = creature_id([0, 0], 1).unwrap();
    fish.kind = Kind::Fish;
    fish.health = Kind::Fish.health();
    fish.needs = CreatureNeeds::full(Kind::Fish);
    fish.position = [8.5, 3.0, 7.1];
    fish.behavior = Behavior::Swim;
    let prey = PreyIndex::new(std::iter::once(&fish));
    a.needs.food = 16.0;
    assert_eq!(
        step(&mut a, &h, &[], &prey, 0.1, false, false),
        Effect::None
    );
    assert_ne!(a.behavior, Behavior::Hunt);
    a.needs.food = 15.0;
    let mut caught = false;
    for _ in 0..10 {
        caught |=
            step(&mut a, &h, &[], &prey, 0.1, false, false) == Effect::CatchFish { id: fish.id };
    }
    assert!(caught);
    let mut population = CreaturePopulation::default();
    population.explored.insert([0, 0]);
    population.animals.insert(a.id, a.clone());
    population.animals.insert(fish.id, fish.clone());
    assert!(catch_fish(&mut population, a.id, fish.id, &h));
    assert!(!catch_fish(&mut population, a.id, fish.id, &h));
    assert_eq!(population.animals[&fish.id].behavior, Behavior::Dead);
    assert!(population.animals[&a.id].needs.food > 32.0);
    assert!(population.valid());
}

#[test]
fn supported_tree_climb_ascends_and_returns_to_ground_without_teleporting() {
    let (t, c, mut j) = fixture();
    for y in 3..=10 {
        j.record([8, y, 8], 6).unwrap();
    }
    let h = Habitat {
        terrain: &t,
        chunks: &c,
        journal: &j,
    };
    let mut a = bear();
    a.position = [
        8.5 + Kind::Bear.size()[0].max(Kind::Bear.size()[2]) * 0.5 + 0.53,
        3.0,
        8.5,
    ];
    a.needs.goal = Some(NeedGoal {
        kind: NeedKind::Tree,
        position: [8, 3, 8],
    });
    a.behavior = Behavior::Climb;
    a.timer = 16.0;
    let mut highest: f64 = 3.0;
    for _ in 0..190 {
        let before = a.position;
        step(&mut a, &h, &[], &PreyIndex::default(), 0.1, false, false);
        highest = highest.max(a.position[1]);
        assert!((a.position[1] - before[1]).abs() <= 0.121);
        assert!(h.clear(a.position, Kind::Bear.size(), false));
        assert!(a.valid());
    }
    assert!(highest > 7.0 && highest <= 9.01);
    assert!((a.position[1] - 3.0).abs() < 0.001);
    assert_ne!(a.behavior, Behavior::Climb);
    assert!(a.needs.goal.is_none());
}

#[test]
fn removed_tree_support_forces_descent_and_unloaded_space_is_never_entered() {
    let (t, c, j) = fixture();
    let h = Habitat {
        terrain: &t,
        chunks: &c,
        journal: &j,
    };
    let mut a = bear();
    a.position = [9.88, 7.0, 8.5];
    a.needs.goal = Some(NeedGoal {
        kind: NeedKind::Tree,
        position: [8, 3, 8],
    });
    a.behavior = Behavior::Climb;
    a.timer = 16.0;
    step(&mut a, &h, &[], &PreyIndex::default(), 0.1, false, false);
    assert!(a.position[1] < 7.0);
    for _ in 0..60 {
        step(&mut a, &h, &[], &PreyIndex::default(), 0.1, false, false);
    }
    assert_eq!(a.position[1], 3.0);
    a.position = [31.0, 3.0, 8.5];
    assert!(!travel(
        &mut a,
        &h,
        [40.0, 3.0, 8.5],
        0.25,
        6.0,
        false,
        None
    ));
    assert!(a.position[0] <= 31.0);
}

#[test]
fn broad_bear_body_cannot_hide_a_trunk_between_its_corner_samples() {
    let (t, c, mut j) = fixture();
    j.record([8, 3, 8], 6).unwrap();
    j.record([7, 3, 6], 6).unwrap();
    let h = Habitat {
        terrain: &t,
        chunks: &c,
        journal: &j,
    };
    assert!(!h.clear([8.5, 3.0, 8.5], Kind::Bear.size(), false));
    assert!(!h.clear_swimmer([8.5, 3.0, 8.5], Kind::Bear.size()));
    assert!(h.ground([8.5, 3.0, 8.5], Kind::Bear).is_none());
    // At integer offsets a three-point probe samples x=6,8,9 and misses x=7.
    assert!(!h.clear([8.0, 3.0, 6.0], Kind::Bear.size(), false));
    assert!(!h.clear_swimmer([8.0, 3.0, 6.0], Kind::Bear.size()));
}

#[test]
fn sleep_and_drinking_use_existing_authoritative_needs() {
    let (t, c, mut j) = fixture();
    let mut a = bear();
    let h = Habitat {
        terrain: &t,
        chunks: &c,
        journal: &j,
    };
    step(&mut a, &h, &[], &PreyIndex::default(), 0.1, true, true);
    assert_eq!(a.behavior, Behavior::Sleep);
    a.needs.water = 3.0;
    a.needs.search_in = 0.0;
    a.timer = 0.0;
    let mut config = t.config().clone();
    config.water = true;
    config.sea_level = -30.0;
    let t = TerraGenerator::new(config).unwrap();
    j.set_fluid(
        [10, 3, 8],
        crate::water::FluidCell {
            amount: 8,
            source: true,
            flow: [0; 2],
        },
    )
    .unwrap();
    let h = Habitat {
        terrain: &t,
        chunks: &c,
        journal: &j,
    };
    for _ in 0..100 {
        step(&mut a, &h, &[], &PreyIndex::default(), 0.1, true, false);
    }
    assert!(a.needs.water > 8.0);
    assert!(a.valid());
}

#[test]
fn water_habitat_requires_loaded_water_not_only_a_procedural_height() {
    let (t, c, mut j) = fixture();
    let p = [8.5, 3.0, 8.5];
    assert!(!near_water(
        &Habitat {
            terrain: &t,
            chunks: &c,
            journal: &j
        },
        p
    ));
    let mut config = t.config().clone();
    config.water = true;
    config.sea_level = 5.5;
    let t = TerraGenerator::new(config).unwrap();
    assert!(near_water(
        &Habitat {
            terrain: &t,
            chunks: &c,
            journal: &j
        },
        p
    ));
    for x in -16..32 {
        for z in -16..32 {
            for y in 3..=5 {
                // Zero fluid removes only the dynamic override. Generated
                // reservoirs are displaced by real placed solid blocks.
                j.record([x, y, z], 3).unwrap();
            }
        }
    }
    assert!(!near_water(
        &Habitat {
            terrain: &t,
            chunks: &c,
            journal: &j
        },
        p
    ));
}

#[test]
fn natural_shore_bear_is_solitary_deterministic_and_keeps_its_spawn_tombstone() {
    let (t, c, mut j) = fixture();
    let home = [0, 0];
    let id = creature_id(home, 0).unwrap();
    let seed = (0..512)
        .find(|seed| {
            let group = random(id, *seed);
            (group >> 40).is_multiple_of(7)
                && !(group >> 32).is_multiple_of(3)
                // Regression: both the bear and butterfly gates must accept
                // this exact cell, so priority is exercised rather than avoided.
                && (group >> 48).is_multiple_of(6)
        })
        .unwrap();
    let group = random(id, seed);
    let x = 8 + (group % 8) as i64;
    let z = 8 + ((group >> 8) % 8) as i64;
    let mut config = t.config().clone();
    config.seed = terra_core::WorldSeed(seed);
    config.water = true;
    config.sea_level = 4.5;
    let t = TerraGenerator::new(config).unwrap();
    // A loaded dry bank inside the otherwise wet flat habitat.
    for dx in -2..=2 {
        for dz in -2..=2 {
            for y in 3..=4 {
                j.record([x + dx, y, z + dz], 1).unwrap();
            }
        }
    }
    let h = Habitat {
        terrain: &t,
        chunks: &c,
        journal: &j,
    };
    let mut a = CreaturePopulation::default();
    let mut b = CreaturePopulation::default();
    assert!(super::super::spawn_cell(&mut a, &h, home));
    assert!(super::super::spawn_cell(&mut b, &h, home));
    assert_eq!(a, b);
    assert_eq!(a.animals.len(), 1);
    assert_eq!(a.animals[&id].kind, Kind::Bear);
    let animal = a.animals.get_mut(&id).unwrap();
    animal.health = 0.0;
    animal.behavior = Behavior::Dead;
    assert!(!super::super::spawn_cell(&mut a, &h, home));
    assert_eq!(a.animals[&id].health, 0.0);
    assert!(a.valid());
}

#[test]
fn cached_ground_detours_work_even_with_an_existing_needs_goal() {
    let (t, c, mut j) = fixture();
    for x in 7..=9 {
        for y in 3..=5 {
            j.record([x, y, 6], 3).unwrap();
        }
    }
    let h = Habitat {
        terrain: &t,
        chunks: &c,
        journal: &j,
    };
    let mut a = bear();
    a.needs.goal = Some(NeedGoal {
        kind: NeedKind::Water,
        position: [8, 3, 3],
    });
    let target = [8.5, 3.0, 3.5];
    let mut navigation = navigation::Navigation::default();
    let mut searches = 0;
    for _ in 0..140 {
        a.age += 0.1;
        let search = navigation.needs_search(a.id, a.age);
        searches += usize::from(search);
        travel(&mut a, &h, target, 0.1, 1.5, search, Some(&mut navigation));
        assert!(h.clear(a.position, Kind::Bear.size(), false));
    }
    assert!(
        distance(a.position, target) < 0.5,
        "position={:?}, searches={searches}",
        a.position
    );
    assert!(searches < 30, "requests={searches}");
    assert!(!navigation.needs_search(a.id, a.age + 10.0));
}

#[test]
fn waiting_attack_does_not_hover_after_its_ground_is_removed() {
    let (t, c, mut j) = fixture();
    for x in 7..=9 {
        for z in 7..=9 {
            j.record([x, 2, z], 0).unwrap();
        }
    }
    let h = Habitat {
        terrain: &t,
        chunks: &c,
        journal: &j,
    };
    let mut a = bear();
    step(
        &mut a,
        &h,
        &[[8.5, 3.0, 7.1]],
        &PreyIndex::default(),
        0.1,
        false,
        false,
    );
    assert!(a.position[1] < 3.0);
}

#[test]
fn a_ready_strike_rechecks_reach_after_removed_support_causes_a_drop() {
    let (t, c, mut j) = fixture();
    for x in 7..=9 {
        for z in 7..=9 {
            j.record([x, 2, z], 0).unwrap();
        }
    }
    let h = Habitat {
        terrain: &t,
        chunks: &c,
        journal: &j,
    };
    let mut a = bear();
    a.behavior = Behavior::Attack;
    a.timer = 0.0;
    let player = [8.5, 3.9, 7.2];
    assert!(distance(a.position, player) < 1.9_f64.powi(2));
    let effect = step(
        &mut a,
        &h,
        &[player],
        &PreyIndex::default(),
        0.1,
        false,
        false,
    );
    assert_eq!(effect, Effect::None);
    assert_eq!(a.position[1], 2.0);
    assert!(distance(a.position, player) > 1.9_f64.powi(2));
}
