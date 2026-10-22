use super::creatures::spawning;
use super::creatures::*;
use crate::{
    TerraGenerator,
    edits::EditJournal,
    streaming::{ChunkStore, LoadedSection},
};
use terra_core::{WorldConfig, creatures::*};
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
    for z in -3..=3 {
        for x in -3..=3 {
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
fn animal(kind: CreatureKind) -> Creature {
    Creature {
        id: creature_id([0, 0], 0).unwrap(),
        home: [0, 0],
        kind,
        position: [8.0, 3.0, 8.0],
        velocity: [0.0; 3],
        yaw: 0.0,
        health: kind.health(),
        behavior: CreatureBehavior::Wander,
        timer: 5.0,
        age: 0.0,
        variant: 0,
        needs: CreatureNeeds::full(kind),
    }
}

#[test]
fn mammals_stand_on_actual_snow_layers_without_hovering_or_crossing_the_cap() {
    let (terrain, mut chunks, mut journal) = fixture();
    for state in 1..=8 {
        let material = if state == 8 { 5 } else { 98 };
        for x in 7..=9 {
            for z in 7..=9 {
                chunks
                    .loaded
                    .get_mut(&SectionPos::default())
                    .unwrap()
                    .data
                    .set_block([x, 3, z], material)
                    .unwrap();
                journal.record([x as i64, 3, z as i64], material).unwrap();
                journal
                    .set_object_state(
                        [x as i64, 3, z as i64],
                        Some(crate::edits::BlockObjectState {
                            facing: state,
                            ..Default::default()
                        }),
                    )
                    .unwrap();
            }
        }
        let habitat = Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        };
        for kind in [
            CreatureKind::Cow,
            CreatureKind::Sheep,
            CreatureKind::Pig,
            CreatureKind::Horse,
        ] {
            let feet = [8.5, 3.0 + f64::from(state) / 8.0, 8.5];
            assert_eq!(habitat.ground(feet, kind), Some(feet));
            assert!(habitat.clear(feet, kind.size(), false));
            assert!(habitat.clear_swimmer(feet, kind.size()));
            let penetrating = [feet[0], feet[1] - 0.09, feet[2]];
            assert!(!habitat.clear(penetrating, kind.size(), false));
            assert!(!habitat.clear_swimmer(penetrating, kind.size()));
        }
    }
}

fn blocked_navigation_fixture() -> (TerraGenerator, ChunkStore, EditJournal) {
    let (terrain, chunks, mut journal) = fixture();
    for x in 5..=11 {
        for y in 3..=6 {
            journal.record([x, y, 7], 1).unwrap();
        }
    }
    (terrain, chunks, journal)
}

fn creature_at_wall(kind: CreatureKind) -> Creature {
    let mut creature = animal(kind);
    let radius = kind.size()[0].max(kind.size()[2]) * 0.5;
    creature.position = [8.5, 3.0, 8.03 + radius];
    creature
}

#[test]
fn blocked_land_creatures_do_not_run_astar_without_the_server_search_budget() {
    let (terrain, chunks, journal) = blocked_navigation_fixture();
    let habitat = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    let mut ecology = terra_core::creatures::ecology::Ecology::default();
    let mut navigation = navigation::Navigation::default();
    navigation::testing::reset_path_calls();
    for kind in [CreatureKind::Cow, CreatureKind::Sheep, CreatureKind::Pig] {
        for id in 1..=64 {
            let mut creature = creature_at_wall(kind);
            creature.id = id;
            creature.needs.search_in = 100.0;
            assert!(habitat.ground(creature.position, kind).is_some());
            crate::creatures::ecology::step_with_navigation(
                &mut creature,
                &mut ecology,
                &habitat,
                &[],
                0.25,
                false,
                false,
                Some(&mut navigation),
            );
        }
    }
    assert_eq!(navigation::testing::path_calls(), 0);
}

#[test]
fn blocked_land_group_shares_the_same_eight_search_budget_as_other_creatures() {
    let (terrain, chunks, journal) = blocked_navigation_fixture();
    let habitat = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    let mut ecology = terra_core::creatures::ecology::Ecology::default();
    let mut navigation = navigation::Navigation::default();
    navigation::testing::reset_path_calls();
    for id in 1..=64 {
        let mut creature = creature_at_wall(CreatureKind::Cow);
        creature.id = id;
        creature.needs.search_in = 100.0;
        crate::creatures::ecology::step_with_navigation(
            &mut creature,
            &mut ecology,
            &habitat,
            &[],
            0.25,
            id <= 8,
            false,
            Some(&mut navigation),
        );
    }
    assert_eq!(navigation::testing::path_calls(), 8);
}

#[test]
fn blocked_land_creatures_reuse_their_route_between_budgeted_searches() {
    let (terrain, chunks, mut journal) = blocked_navigation_fixture();
    let owner = SectionPos::default();
    let food = crate::props::WorldProp {
        id: crate::props::PropId(197),
        kind: crate::props::PropKind::Grass,
        position: [8.5, 3.0, 5.5],
        rotation: 0.0,
        scale: 1.0,
        variant: 0,
        section: owner,
        removed: false,
    };
    journal.place_prop(food).unwrap();
    let mut chunks = chunks;
    chunks.loaded.get_mut(&owner).unwrap().props.push(food);
    let habitat = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    let mut ecology = terra_core::creatures::ecology::Ecology::default();
    let mut navigation = navigation::Navigation::default();
    let mut creature = creature_at_wall(CreatureKind::Cow);
    creature.needs.goal = Some(NeedGoal {
        kind: NeedKind::Food,
        position: [8, 2, 5],
    });
    creature.needs.search_in = 15.0;
    navigation::testing::reset_path_calls();
    for tick in 0..8 {
        crate::creatures::ecology::step_with_navigation(
            &mut creature,
            &mut ecology,
            &habitat,
            &[],
            0.1,
            tick == 0,
            false,
            Some(&mut navigation),
        );
    }
    assert_eq!(navigation::testing::path_calls(), 1);
    assert!(creature.valid());
    assert!(habitat.ground(creature.position, creature.kind).is_some());
}

#[test]
fn command_spawns_require_loaded_habitat_and_keep_repeated_identities_distinct() {
    let (terrain, chunks, journal) = fixture();
    let mut population = CreaturePopulation::default();
    let habitat = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    let cow = terra_core::creatures::spawning::lookup("teramic::cow").unwrap();
    let fish = terra_core::creatures::spawning::lookup("teramic::clown_fish").unwrap();
    for _ in 0..8 {
        let spawned = spawning::near(&population, &habitat, cow, [8.0, 3.0, 8.0], 0.0)
            .expect("nearby dry ground");
        assert_eq!(spawned.position[1], 3.0);
        assert!(!population.animals.contains_key(&spawned.id));
        population.animals.insert(spawned.id, spawned);
    }
    assert!(population.valid());
    assert!(population.explored.is_empty());
    assert!(spawning::near(&population, &habitat, fish, [8.0, 3.0, 8.0], 0.0).is_err());
    assert!(spawning::near(&population, &habitat, cow, [1000.0, 3.0, 1000.0], 0.0).is_err());
    assert!(spawning::near(&population, &habitat, cow, [f64::NAN, 3.0, 8.0], 0.0).is_err());
}

#[test]
fn command_clownfish_spawns_inside_edited_water_and_not_inside_a_block() {
    let (terrain, chunks, mut journal) = fixture();
    let terrain = TerraGenerator::new(WorldConfig {
        water: true,
        sea_level: -40.0,
        ..terrain.config().clone()
    })
    .unwrap();
    for x in 3..=12 {
        for z in 0..=8 {
            for y in 3..=6 {
                journal
                    .set_fluid(
                        [x, y, z],
                        crate::water::FluidCell {
                            amount: 8,
                            flow: [0, 0],
                            source: true,
                        },
                    )
                    .unwrap();
            }
        }
    }
    journal.record([7, 4, 4], 1).unwrap();
    let habitat = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    let mob = terra_core::creatures::spawning::lookup("teramic::clown_fish").unwrap();
    let fish = spawning::near(
        &CreaturePopulation::default(),
        &habitat,
        mob,
        [8.0, 7.0, 8.0],
        0.0,
    )
    .unwrap();
    assert_eq!(fish.product(), Some(AnimalProduct::Clownfish));
    assert_eq!(fish.behavior, CreatureBehavior::Swim);
    assert!(habitat.clear(fish.position, fish.size(), true));
    assert!(fish.valid());
}

#[test]
fn animals_land_on_partial_support_without_hovering_or_crossing_unknown_sections() {
    let (t, c, mut j) = fixture();
    for x in 5..=11 {
        for z in 5..=11 {
            for y in 0..3 {
                if [x, z] != [8, 8] {
                    j.record([x, y, z], 0).unwrap();
                }
            }
        }
    }
    let h = Habitat {
        terrain: &t,
        chunks: &c,
        journal: &j,
    };
    for kind in [
        CreatureKind::Sheep,
        CreatureKind::Cow,
        CreatureKind::Pig,
        CreatureKind::Bear,
    ] {
        let mut a = animal(kind);
        a.position = [8.9, 6.37, 8.9];
        a.behavior = CreatureBehavior::Rest;
        a.timer = 100.0;
        for _ in 0..20 {
            step_budgeted(&mut a, &h, &[], 0.1, false);
        }
        assert!(
            (a.position[1] - 3.0).abs() < 0.025,
            "{kind:?}: {:?}",
            a.position
        );
        assert!(a.position[1] >= 2.975);
    }
    let mut a = animal(CreatureKind::Cow);
    a.position = [46.9, 6.37, 46.9];
    a.behavior = CreatureBehavior::Rest;
    for _ in 0..50 {
        step_budgeted(&mut a, &h, &[], 0.1, false);
    }
    assert!(a.position[1] >= 2.975);
}

#[test]
fn sleeping_animals_resolve_changed_ground_before_becoming_stationary() {
    let (t, c, mut j) = fixture();
    for x in 6..=10 {
        for z in 6..=10 {
            j.record([x, 2, z], 0).unwrap();
        }
    }
    let h = Habitat {
        terrain: &t,
        chunks: &c,
        journal: &j,
    };
    let mut a = animal(CreatureKind::Sheep);
    a.behavior = CreatureBehavior::Sleep;
    let mut ecology = terra_core::creatures::ecology::Ecology::default();
    crate::creatures::ecology::step(&mut a, &mut ecology, &h, &[], 0.1, false, true);
    assert_eq!(a.behavior, CreatureBehavior::Sleep);
    assert_eq!(a.position[1], 2.0);
}

#[test]
fn falling_mammals_do_not_snap_upward_onto_a_nearby_ledge() {
    let (t, c, mut j) = fixture();
    for x in 6..=11 {
        for z in 6..=11 {
            for y in 0..3 {
                j.record([x, y, z], 0).unwrap();
            }
        }
    }
    for x in 7..=9 {
        for z in 7..=9 {
            j.record([x, 2, z], 1).unwrap();
        }
    }
    let h = Habitat {
        terrain: &t,
        chunks: &c,
        journal: &j,
    };
    for kind in [
        CreatureKind::Sheep,
        CreatureKind::Cow,
        CreatureKind::Pig,
        CreatureKind::Bear,
    ] {
        let mut a = animal(kind);
        a.position = [8.5, 2.7, 10.1];
        a.velocity[1] = -2.0;
        a.behavior = CreatureBehavior::Rest;
        a.timer = 10.0;
        let before = a.position[1];
        step_budgeted(&mut a, &h, &[], 0.1, false);
        assert!(
            a.position[1] <= before + 0.001,
            "{kind:?}: {:?}",
            a.position
        );
    }
}

#[test]
fn mammals_enter_the_water_at_its_exact_surface_and_float_with_immersed_bodies() {
    let (t, c, mut j) = fixture();
    let mut config = t.config().clone();
    config.water = true;
    config.sea_level = -30.0;
    let t = TerraGenerator::new(config).unwrap();
    for x in -4..=16 {
        for z in -8..=16 {
            for y in 3..6 {
                j.set_fluid(
                    [x, y, z],
                    crate::water::FluidCell {
                        amount: 8,
                        flow: [0, 0],
                        source: false,
                    },
                )
                .unwrap();
            }
        }
    }
    let h = Habitat {
        terrain: &t,
        chunks: &c,
        journal: &j,
    };
    for kind in [
        CreatureKind::Sheep,
        CreatureKind::Cow,
        CreatureKind::Pig,
        CreatureKind::Bear,
    ] {
        let mut a = animal(kind);
        a.position = [8.0, 6.0, 8.0];
        a.behavior = CreatureBehavior::Rest;
        a.timer = 10.0;
        let mut eco = terra_core::creatures::ecology::Ecology::default();
        for _ in 0..40 {
            crate::creatures::ecology::step(&mut a, &mut eco, &h, &[], 0.1, false, false);
        }
        assert!(h.wet(a.position), "{kind:?}: {:?}", a.position);
        assert!(!h.wet([a.position[0], a.position[1] + kind.size()[1], a.position[2]]));
        assert!(
            (a.position[1] - (6.0 - (kind.size()[1] * 0.68).min(0.9))).abs() < 0.12,
            "{kind:?}: {:?}",
            a.position
        );
        assert_eq!(a.behavior, CreatureBehavior::Swim);
        assert!(a.velocity[1].abs() < 0.1);
    }
}

#[test]
fn calm_animals_watch_nearby_players_but_flee_from_intrusion_and_damage() {
    let (t, c, j) = fixture();
    let h = Habitat {
        terrain: &t,
        chunks: &c,
        journal: &j,
    };
    for kind in [CreatureKind::Sheep, CreatureKind::Cow, CreatureKind::Pig] {
        let mut a = animal(kind);
        a.behavior = CreatureBehavior::Rest;
        a.timer = 10.0;
        let player = [8.0, 3.0, 10.0];
        step_budgeted(&mut a, &h, &[player], 0.1, false);
        assert_eq!(a.behavior, CreatureBehavior::Rest);
        let forward = [-f64::from(a.yaw.sin()), -f64::from(a.yaw.cos())];
        assert!(
            forward[1] > 0.99,
            "{kind:?} is looking away from the player"
        );
        let before = distance(a.position, player);
        a.behavior = CreatureBehavior::Flee;
        a.timer = 2.0;
        step_budgeted(&mut a, &h, &[player], 0.1, false);
        assert!(distance(a.position, player) > before);
        assert_eq!(a.behavior, CreatureBehavior::Flee);
        a.behavior = CreatureBehavior::Rest;
        let player = [a.position[0] + 0.7, a.position[1], a.position[2]];
        step_budgeted(&mut a, &h, &[player], 0.1, false);
        assert_eq!(a.behavior, CreatureBehavior::Flee);
    }
}

#[test]
fn mammals_can_climb_a_dry_bank_without_treating_open_water_as_ground() {
    let (t, c, mut j) = fixture();
    let mut config = t.config().clone();
    config.water = true;
    config.sea_level = -30.0;
    let t = TerraGenerator::new(config).unwrap();
    for x in 2..=13 {
        for z in -8..=13 {
            for y in 3..6 {
                if z < 5 {
                    j.record([x, y, z], 1).unwrap();
                } else {
                    j.set_fluid(
                        [x, y, z],
                        crate::water::FluidCell {
                            amount: 8,
                            flow: [0, 0],
                            source: false,
                        },
                    )
                    .unwrap();
                }
            }
        }
    }
    let h = Habitat {
        terrain: &t,
        chunks: &c,
        journal: &j,
    };
    for kind in [
        CreatureKind::Sheep,
        CreatureKind::Cow,
        CreatureKind::Pig,
        CreatureKind::Bear,
    ] {
        assert!(h.ground([8.0, 6.0, 8.0], kind).is_none());
        let mut a = animal(kind);
        a.position = [8.0, 6.0 - (kind.size()[1] * 0.68).min(0.9), 6.3];
        a.behavior = CreatureBehavior::Swim;
        a.timer = 10.0;
        let mut eco = terra_core::creatures::ecology::Ecology::default();
        for _ in 0..30 {
            crate::creatures::ecology::step(&mut a, &mut eco, &h, &[], 0.1, false, false);
        }
        assert!(!h.wet(a.position), "{kind:?}: {:?}", a.position);
        assert!(
            (a.position[1] - 6.0).abs() < 0.01,
            "{kind:?}: {:?}",
            a.position
        );
        assert!(a.position[2] < 5.0, "{kind:?}: {:?}", a.position);
    }
}

#[test]
fn land_animals_respect_both_section_loading_edges_and_resume_after_reload() {
    for (start, missing, sign) in [(14.0, 1, 1.0), (-14.0, -2, -1.0)] {
        let (t, mut c, j) = fixture();
        let key = SectionPos::new(missing, 0, 0);
        let section = c.loaded.remove(&key).unwrap();
        let yaw = if sign > 0.0 {
            std::f32::consts::TAU - std::f32::consts::FRAC_PI_2
        } else {
            std::f32::consts::FRAC_PI_2
        };
        let mut animals = Vec::new();
        {
            let h = Habitat {
                terrain: &t,
                chunks: &c,
                journal: &j,
            };
            for kind in [
                CreatureKind::Sheep,
                CreatureKind::Cow,
                CreatureKind::Pig,
                CreatureKind::Bear,
                CreatureKind::Horse,
            ] {
                let mut a = animal(kind);
                a.position = [start, 3.0, 8.0];
                a.yaw = yaw;
                a.timer = 10.0;
                for _ in 0..25 {
                    step_budgeted(&mut a, &h, &[], 0.1, false);
                    assert!(
                        h.clear_rotated(a.position, kind.size(), false, f64::from(a.yaw)),
                        "{kind:?}: {:?}",
                        a.position
                    );
                    assert_eq!(a.position[1], 3.0);
                    assert!(a.valid());
                }
                let (sin, cos) = f64::from(a.yaw).sin_cos();
                let extent = (kind.size()[0] * cos.abs() + kind.size()[2] * sin.abs()) * 0.5;
                assert!(sign * a.position[0] + extent <= 16.000001);
                animals.push(a);
            }
        }
        c.loaded.insert(key, section);
        let h = Habitat {
            terrain: &t,
            chunks: &c,
            journal: &j,
        };
        for mut a in animals {
            a.yaw = yaw;
            a.behavior = CreatureBehavior::Wander;
            a.timer = 10.0;
            for _ in 0..50 {
                step_budgeted(&mut a, &h, &[], 0.1, false);
                assert!(h.clear_rotated(a.position, a.kind.size(), false, f64::from(a.yaw)));
                assert_eq!(a.position[1], 3.0);
            }
            assert!(
                sign * a.position[0] > 16.0,
                "{:?}: {:?}",
                a.kind,
                a.position
            );
        }
    }
}

#[test]
fn mammals_cross_partial_water_section_seams_and_fall_after_drain() {
    let (t, c, mut j) = fixture();
    let mut config = t.config().clone();
    config.water = true;
    config.sea_level = -30.0;
    let t = TerraGenerator::new(config).unwrap();
    for x in -5..=24 {
        for z in 4..=11 {
            for y in 3..6 {
                j.set_fluid(
                    [x, y, z],
                    crate::water::FluidCell {
                        amount: if y == 5 { 4 } else { 8 },
                        flow: [0, 0],
                        source: false,
                    },
                )
                .unwrap();
            }
        }
    }
    let mut animals = Vec::new();
    {
        let h = Habitat {
            terrain: &t,
            chunks: &c,
            journal: &j,
        };
        for start in [-0.1, 15.9] {
            for kind in [
                CreatureKind::Sheep,
                CreatureKind::Cow,
                CreatureKind::Pig,
                CreatureKind::Bear,
            ] {
                let mut a = animal(kind);
                a.position = [start, 5.5, 8.0];
                a.yaw = std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
                a.timer = 10.0;
                for _ in 0..30 {
                    step_budgeted(&mut a, &h, &[], 0.1, false);
                    assert!(h.clear_swimmer(a.position, kind.size()));
                    assert!(a.position[1] <= 5.5);
                    assert!(a.valid());
                }
                assert!(a.position[0] > start + 1.0);
                assert!(h.wet(a.position));
                assert!((a.position[1] - (5.5 - (kind.size()[1] * 0.68).min(0.9))).abs() < 0.12);
                animals.push(a);
            }
        }
    }
    for x in -5..=24 {
        for z in 4..=11 {
            for y in 3..6 {
                j.set_fluid([x, y, z], crate::water::FluidCell::default())
                    .unwrap();
            }
        }
    }
    let h = Habitat {
        terrain: &t,
        chunks: &c,
        journal: &j,
    };
    for mut a in animals {
        a.behavior = CreatureBehavior::Rest;
        a.timer = 10.0;
        for _ in 0..30 {
            step_budgeted(&mut a, &h, &[], 0.1, false);
        }
        assert_eq!(a.position[1], 3.0, "{:?}: {:?}", a.kind, a.position);
        assert!(h.clear(a.position, a.kind.size(), false));
    }
}
#[test]
fn spawn_is_order_independent_and_reload_does_not_duplicate_dead_animals() {
    let (t, c, j) = fixture();
    let h = Habitat {
        terrain: &t,
        chunks: &c,
        journal: &j,
    };
    let mut a = CreaturePopulation::default();
    let mut b = a.clone();
    for home in [[-1, -1], [0, 0], [0, -1], [-1, 0]] {
        assert!(spawn_cell(&mut a, &h, home));
    }
    for home in [[-1, 0], [0, -1], [0, 0], [-1, -1]] {
        assert!(spawn_cell(&mut b, &h, home));
    }
    assert_eq!(a, b);
    assert!(!a.animals.is_empty());
    for home in &a.explored {
        let group: Vec<_> = a.animals.values().filter(|c| c.home == *home).collect();
        assert!(group.is_empty() || (3..=4).contains(&group.len()));
        if let Some(first) = group.first() {
            assert!(
                group
                    .iter()
                    .all(|c| c.kind == first.kind && distance(c.position, first.position) <= 25.0)
            );
        }
    }
    assert!(a.valid());
    for a in a.animals.values_mut() {
        a.health = 0.0;
        a.behavior = CreatureBehavior::Dead;
    }
    let before = a.clone();
    assert!(!spawn_cell(&mut a, &h, [0, 0]));
    assert_eq!(a, before);
}

#[test]
fn herd_occupancy_does_not_exclude_any_land_species() {
    let (terrain, chunks, journal) = fixture();
    let mut kinds = std::collections::BTreeSet::new();
    for seed in 0..16 {
        let mut config = terrain.config().clone();
        config.seed = terra_core::WorldSeed(seed);
        let terrain = TerraGenerator::new(config).unwrap();
        let habitat = Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        };
        let mut population = CreaturePopulation::default();
        for home in [[-1, -1], [-1, 0], [0, -1], [0, 0]] {
            assert!(spawn_cell(&mut population, &habitat, home));
        }
        for animal in population.animals.values() {
            kinds.insert(animal.kind);
        }
        assert!(population.valid());
    }
    assert_eq!(
        kinds,
        std::collections::BTreeSet::from([
            CreatureKind::Sheep,
            CreatureKind::Cow,
            CreatureKind::Pig
        ])
    );
}
#[test]
fn navigation_avoids_walls_cliffs_and_unloaded_cells_after_edits() {
    let (t, c, mut j) = fixture();
    for x in 6..=10 {
        for y in 3..=5 {
            j.record([x, y, 7], 3).unwrap();
        }
    }
    let h = Habitat {
        terrain: &t,
        chunks: &c,
        journal: &j,
    };
    let mut a = animal(CreatureKind::Cow);
    let before = a.position;
    step(&mut a, &h, &[], 0.1);
    assert_eq!(a.position, before);
    assert!(a.valid());
    assert!(h.ground([1000.0, 3.0, 1000.0], CreatureKind::Cow).is_none());
    for x in 6..=10 {
        for y in 0..=5 {
            for z in 6..=7 {
                j.record([x, y, z], 0).unwrap();
            }
        }
    }
    let h = Habitat {
        terrain: &t,
        chunks: &c,
        journal: &j,
    };
    a.yaw = 0.0;
    step(&mut a, &h, &[], 0.1);
    assert_eq!(a.position, before);
}
#[test]
fn fish_respect_real_liquid_barriers_and_suffocate_after_drain() {
    let (mut t, c, mut j) = fixture();
    let mut config = t.config().clone();
    config.water = true;
    config.sea_level = -30.0;
    t = TerraGenerator::new(config).unwrap();
    for x in 6..=10 {
        for y in 3..=5 {
            for z in 6..=10 {
                j.set_fluid(
                    [x, y, z],
                    crate::water::FluidCell {
                        amount: 8,
                        flow: [1, 0],
                        source: false,
                    },
                )
                .unwrap();
            }
        }
    }
    let mut a = animal(CreatureKind::Fish);
    a.position[1] = 4.0;
    a.behavior = CreatureBehavior::Swim;
    let h = Habitat {
        terrain: &t,
        chunks: &c,
        journal: &j,
    };
    step(&mut a, &h, &[], 0.1);
    assert!(a.position[0] > 8.0 && h.wet(a.position));
    for x in 6..=10 {
        for y in 3..=5 {
            for z in 6..=10 {
                j.set_fluid([x, y, z], crate::water::FluidCell::default())
                    .unwrap();
            }
        }
    }
    let h = Habitat {
        terrain: &t,
        chunks: &c,
        journal: &j,
    };
    for _ in 0..110 {
        step(&mut a, &h, &[], 0.1);
    }
    assert_eq!(a.behavior, CreatureBehavior::Dead);
    assert_eq!(a.health, 0.0);
    assert!(a.valid());
}
#[test]
fn sight_ray_checks_range_and_zero_direction_components() {
    let a = animal(CreatureKind::Sheep);
    assert!(ray_distance(&a, [8.0, 3.5, 5.0], [0.0, 0.0, 1.0], 4.5).is_some());
    assert!(ray_distance(&a, [8.0, 3.5, 0.0], [0.0, 0.0, 1.0], 4.5).is_none());
    assert!(ray_distance(&a, [10.0, 3.5, 5.0], [0.0, 0.0, 1.0], 4.5).is_none());
}

#[test]
fn bounded_herd_navigation_stays_valid_under_many_updates() {
    let (t, c, j) = fixture();
    let h = Habitat {
        terrain: &t,
        chunks: &c,
        journal: &j,
    };
    let mut herd: Vec<_> = (0..128)
        .map(|i| {
            let mut a = animal(CreatureKind::ALL[i % 3]);
            a.home = [i as i64, 0];
            a.id = creature_id(a.home, 0).unwrap();
            a.position = [
                -20.0 + (i % 16) as f64 * 2.5,
                3.0,
                -15.0 + (i / 16) as f64 * 3.0,
            ];
            a
        })
        .collect();
    let start = std::time::Instant::now();
    let mut samples = Vec::new();
    for _ in 0..100 {
        let tick = std::time::Instant::now();
        for a in &mut herd {
            step(a, &h, &[[0.0, 3.0, 0.0], [20.0, 3.0, 10.0]], 0.1);
        }
        samples.push(tick.elapsed().as_secs_f64() * 1000.0);
    }
    assert!(herd.iter().all(Creature::valid));
    samples.sort_by(f64::total_cmp);
    println!(
        "herd_benchmark animals=128 players=2 steps=100 wall_seconds={:.6} tick_p95_ms={:.6} tick_p99_ms={:.6}",
        start.elapsed().as_secs_f64(),
        samples[94],
        samples[98]
    );
}

#[test]
fn thirsty_cow_walks_to_real_water_and_drinks_from_dry_land() {
    let (mut terrain, chunks, mut journal) = fixture();
    let mut config = terrain.config().clone();
    config.water = true;
    config.sea_level = -30.0;
    terrain = TerraGenerator::new(config).unwrap();
    for z in 6..=10 {
        for x in 12..=14 {
            for y in 1..=2 {
                journal.record([x, y, z], 0).unwrap();
            }
            journal
                .set_fluid(
                    [x, 1, z],
                    crate::water::FluidCell {
                        amount: 8,
                        source: true,
                        flow: [0, 0],
                    },
                )
                .unwrap();
        }
    }
    let h = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    let mut cow = animal(CreatureKind::Cow);
    cow.needs.water = 1.0;
    let before = journal.edited_blocks();
    let mut drank = false;
    for _ in 0..200 {
        step(&mut cow, &h, &[], 0.1);
        drank |= cow.behavior == CreatureBehavior::Drink;
        if cow.behavior == CreatureBehavior::Drink {
            assert!(cow.position[0] > 9.0);
        }
        assert!(!h.wet(cow.position));
        assert!(cow.valid());
    }
    assert!(drank && cow.needs.water > 5.0);
    assert_eq!(journal.edited_blocks(), before);
}

#[test]
fn grazing_commits_one_prop_without_removing_neighbors_and_pigs_prefer_plants() {
    use crate::props::{PropId, PropKind, WorldProp};
    let (terrain, mut chunks, mut journal) = fixture();
    let owner = SectionPos::new(0, 0, 0);
    let grass = WorldProp {
        id: PropId(901),
        kind: PropKind::Grass,
        position: [8.5, 3.0, 8.5],
        rotation: 0.0,
        scale: 0.7,
        variant: 0,
        section: owner,
        removed: false,
    };
    let mushroom = WorldProp {
        id: PropId(902),
        kind: PropKind::Mushroom,
        position: [9.5, 3.0, 8.5],
        ..grass
    };
    chunks.loaded.get_mut(&owner).unwrap().props = vec![grass, mushroom];
    let mut cow = animal(CreatureKind::Cow);
    cow.needs.food = 10.0;
    for _ in 0..40 {
        let effect = step(
            &mut cow,
            &Habitat {
                terrain: &terrain,
                chunks: &chunks,
                journal: &journal,
            },
            &[],
            0.1,
        );
        apply_forage(effect, &mut cow, &mut chunks, &mut journal).unwrap();
    }
    assert!(cow.needs.food > 15.0);
    assert!(journal.prop_removed(owner, grass.id));
    assert!(!journal.prop_removed(owner, mushroom.id));
    let mut pig = animal(CreatureKind::Pig);
    pig.needs.food = 1.0;
    for _ in 0..65 {
        let effect = step(
            &mut pig,
            &Habitat {
                terrain: &terrain,
                chunks: &chunks,
                journal: &journal,
            },
            &[],
            0.1,
        );
        apply_forage(effect, &mut pig, &mut chunks, &mut journal).unwrap();
    }
    assert!(pig.needs.food > 5.0 && journal.prop_removed(owner, mushroom.id));
}
