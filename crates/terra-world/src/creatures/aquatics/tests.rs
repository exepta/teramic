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

fn spawn_kind(seed: u64, water: SpawnWater, depth: f64, biome: &str) -> Option<Kind> {
    select_spawn_kind(
        seed,
        water,
        depth,
        biome,
        terra_core::creatures::definitions::get,
    )
}

fn fixture() -> (TerraGenerator, ChunkStore, EditJournal) {
    let terrain = TerraGenerator::new(WorldConfig {
        base_height: 3.0,
        hill_amplitude: 0.0,
        mountain_amplitude: 0.0,
        caves: false,
        trees: false,
        biomes: false,
        rivers: false,
        water: true,
        sea_level: 13.0,
        ..Default::default()
    })
    .unwrap();
    let mut chunks = ChunkStore::default();
    for x in -1..=1 {
        for z in -1..=1 {
            let section = SectionPos::new(x, 0, z);
            let mut data = BlockSection::empty(section).unwrap();
            for x in 0..16 {
                for z in 0..16 {
                    for y in 0..3 {
                        data.set_block([x, y, z], 3).unwrap();
                    }
                }
            }
            chunks.loaded.insert(
                section,
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
fn animal(kind: Kind, slot: u8) -> Creature {
    Creature {
        id: creature_id([0, 0], slot).unwrap(),
        kind,
        home: [0, 0],
        position: [8.0, 5.0, 8.0],
        velocity: [0.0; 3],
        yaw: 0.0,
        health: kind.health(),
        behavior: Behavior::Swim,
        timer: 2.0,
        age: 0.0,
        variant: 0,
        needs: CreatureNeeds::full(kind),
    }
}

#[test]
fn new_species_are_ocean_only_except_rare_lake_squid_and_do_not_replace_fish() {
    let mut kinds = std::collections::BTreeSet::new();
    let mut lake_squid = 0;
    for n in 0..2400_u64 {
        let seed = n << 16;
        assert_eq!(
            spawn_kind(seed, SpawnWater::River, 20.0, "forest"),
            Some(Kind::Fish)
        );
        let lake = spawn_kind(seed, SpawnWater::Lake, 20.0, "plains");
        assert!(matches!(lake, Some(Kind::Fish | Kind::Squid)));
        lake_squid += usize::from(lake == Some(Kind::Squid));
        let ocean = spawn_kind(seed, SpawnWater::Ocean, 20.0, "ocean");
        assert_eq!(ocean, spawn_kind(seed, SpawnWater::Ocean, 20.0, "ocean"));
        kinds.insert(ocean.unwrap());
    }
    assert_eq!(kinds.len(), 10);
    assert_eq!(lake_squid, 100);
}

fn configured_definitions() -> Vec<MobDefinition> {
    Kind::ALL
        .into_iter()
        .map(|kind| {
            let mut definition = terra_core::creatures::definitions::get(kind).clone();
            definition.spawn = Some(terra_core::creatures::definitions::MobSpawnSettings {
                aquatic: Some(Default::default()),
                ..Default::default()
            });
            definition
        })
        .collect()
}

#[test]
fn marine_weighted_selection_preserves_all_legacy_seed_decisions() {
    for seed in (0..10_000).map(|n| random(n, 0x8217)) {
        let expected = match (seed >> 16) % 100 {
            0..=12 => Kind::Squid,
            13..=18 => Kind::MoonJelly,
            19..=24 => Kind::FireJelly,
            25..=29 => Kind::VoidJelly,
            30..=35 => Kind::SunJelly,
            36..=40 => Kind::CrystalJelly,
            41..=46 => Kind::ReefShark,
            47..=50 => Kind::HammerheadShark,
            51..=52 => Kind::WhaleShark,
            _ => Kind::Fish,
        };
        assert_eq!(
            spawn_kind(seed, SpawnWater::Ocean, 20.0, "ocean"),
            Some(expected)
        );
        let lake = if (seed >> 16).is_multiple_of(24) {
            Kind::Squid
        } else {
            Kind::Fish
        };
        assert_eq!(
            spawn_kind(seed, SpawnWater::Lake, 20.0, "forest"),
            Some(lake)
        );
    }
}

#[test]
fn configured_weights_and_habitat_depth_biome_filters_drive_selection() {
    let mut definitions = configured_definitions();
    let squid = definitions[Kind::Squid as usize].spawn.as_mut().unwrap();
    squid.biomes = vec!["forest".into()];
    squid.aquatic = Some(terra_core::creatures::definitions::AquaticSpawn {
        lake_weight: 3.0,
        water_depth: [4.0, 12.0],
        ..Default::default()
    });
    let shark = definitions[Kind::ReefShark as usize]
        .spawn
        .as_mut()
        .unwrap();
    shark.biomes = vec!["ocean".into()];
    shark.aquatic = Some(terra_core::creatures::definitions::AquaticSpawn {
        ocean_weight: 1.0,
        water_depth: [8.0, 20.0],
        ..Default::default()
    });
    let choose = |seed, water, depth, biome| {
        select_spawn_kind(seed, water, depth, biome, |kind| {
            &definitions[kind as usize]
        })
    };
    for seed in 0..100 {
        assert_eq!(
            choose(seed, SpawnWater::Ocean, 10.0, "ocean"),
            Some(Kind::ReefShark)
        );
        assert_eq!(
            choose(seed, SpawnWater::Lake, 6.0, "forest"),
            Some(Kind::Squid)
        );
        for depth in [3.99, 12.01, f64::NAN, -1.0] {
            assert_eq!(choose(seed, SpawnWater::Lake, depth, "forest"), None);
        }
        assert_eq!(choose(seed, SpawnWater::Lake, 6.0, "desert"), None);
        assert_eq!(choose(seed, SpawnWater::Ocean, 7.99, "ocean"), None);
        assert_eq!(choose(seed, SpawnWater::River, 10.0, "forest"), None);
    }
}

#[test]
fn fractional_spawn_weights_below_one_are_sampled_proportionally() {
    let mut definitions = configured_definitions();
    for (kind, weight) in [(Kind::Squid, 0.025), (Kind::Fish, 0.075)] {
        definitions[kind as usize]
            .spawn
            .as_mut()
            .unwrap()
            .aquatic
            .as_mut()
            .unwrap()
            .ocean_weight = weight;
    }
    let mut squid = 0;
    for seed in 0..10_000 {
        let choose = || {
            select_spawn_kind(seed, SpawnWater::Ocean, 10.0, "ocean", |kind| {
                &definitions[kind as usize]
            })
        };
        assert_eq!(choose(), choose());
        squid += usize::from(choose() == Some(Kind::Squid));
    }
    assert!((2200..=2800).contains(&squid), "squid={squid}");
}

#[test]
fn configured_spawn_depth_is_clamped_to_water_and_body_clearance() {
    let mut squid = terra_core::creatures::definitions::get(Kind::Squid).clone();
    squid
        .spawn
        .as_mut()
        .unwrap()
        .aquatic
        .as_mut()
        .unwrap()
        .depth_below_surface = Some([2.0, 6.0]);
    for seed in 0..600 {
        let depth = spawn_depth_for(&squid, seed, 4.0).unwrap();
        assert!((2.0..=3.8).contains(&depth));
    }
    assert_eq!(spawn_depth_for(&squid, 0, 1.0), None);
    squid
        .spawn
        .as_mut()
        .unwrap()
        .aquatic
        .as_mut()
        .unwrap()
        .depth_below_surface = Some([0.0, 0.1]);
    assert_eq!(spawn_depth_for(&squid, 0, 4.0), None);
}

#[test]
fn custom_spawn_depth_waits_for_the_whole_body_then_retries_without_changing_old_ids() {
    for home in [[0, 0], [-1, -1]] {
        for depth in [25.0, 29.1] {
            let seed = (0..100)
                .find(|&seed| {
                    !(random(creature_id(home, 0).unwrap(), seed) >> 32).is_multiple_of(3)
                })
                .unwrap();
            let terrain = TerraGenerator::new(WorldConfig {
                seed: terra_core::WorldSeed(seed),
                base_height: 3.0,
                hill_amplitude: 0.0,
                mountain_amplitude: 0.0,
                caves: false,
                trees: false,
                biomes: false,
                rivers: false,
                water: true,
                sea_level: 45.0,
                ..Default::default()
            })
            .unwrap();
            let mut definitions = configured_definitions();
            let fish = definitions[Kind::Fish as usize]
                .spawn
                .as_mut()
                .unwrap()
                .aquatic
                .as_mut()
                .unwrap();
            fish.ocean_weight = 1.0;
            fish.lake_weight = 1.0;
            fish.river_weight = 1.0;
            fish.depth_below_surface = Some([depth, depth]);
            assert!(definitions.iter().all(MobDefinition::valid));
            let mut chunks = ChunkStore::default();
            let load = |chunks: &mut ChunkStore, y| {
                for x in home[0] * 2..=home[0] * 2 + 1 {
                    for z in home[1] * 2..=home[1] * 2 + 1 {
                        let position = SectionPos::new(x, y, z);
                        chunks.loaded.insert(
                            position,
                            LoadedSection {
                                data: BlockSection::empty(position).unwrap(),
                                mesh: None,
                                mesh_epoch: 0,
                                water_epoch: 0,
                                water: None,
                                props: Vec::new(),
                            },
                        );
                    }
                }
            };
            load(&mut chunks, 2);
            if depth > 29.0 {
                // Feet are loaded, but the fish's upper body crosses into y=1.
                load(&mut chunks, 0);
            }
            let journal = EditJournal::default();
            let mut population = CreaturePopulation::default();
            population.explored.insert([7, -9]);
            let before = population.clone();
            assert!(!super::super::spawn_cell_using(
                &mut population,
                &Habitat {
                    terrain: &terrain,
                    chunks: &chunks,
                    journal: &journal,
                },
                home,
                |kind| &definitions[kind as usize]
            ));
            assert_eq!(population, before, "home={home:?}, depth={depth}");
            load(&mut chunks, 1);
            assert!(super::super::spawn_cell_using(
                &mut population,
                &Habitat {
                    terrain: &terrain,
                    chunks: &chunks,
                    journal: &journal,
                },
                home,
                |kind| &definitions[kind as usize]
            ));
            assert!(population.explored.contains(&[7, -9]));
            assert!(population.explored.contains(&home));
            assert!(!population.animals.is_empty());
            for animal in population.animals.values() {
                assert_eq!(animal.kind, Kind::Fish);
                assert!((animal.position[1] - (45.0 - depth)).abs() < 1e-6);
                assert!((0..4).any(|slot| creature_id(home, slot) == Some(animal.id)));
            }
            let after = population.clone();
            assert!(!super::super::spawn_cell_using(
                &mut population,
                &Habitat {
                    terrain: &terrain,
                    chunks: &chunks,
                    journal: &journal,
                },
                home,
                |kind| &definitions[kind as usize]
            ));
            assert_eq!(population, after);
        }
    }
}

#[test]
fn swimming_stays_submerged_and_never_enters_unloaded_voxels_or_an_edited_wall() {
    let (t, c, mut j) = fixture();
    for x in 4..=12 {
        for y in 3..=12 {
            j.record([x, y, 6], 3).unwrap();
        }
    }
    let h = Habitat {
        terrain: &t,
        chunks: &c,
        journal: &j,
    };
    for kind in Kind::ALL
        .into_iter()
        .filter(|k| k.is_aquatic() && *k != Kind::Fish)
    {
        let mut a = animal(kind, 0);
        a.position[2] = 10.0;
        for _ in 0..80 {
            step(&mut a, &h, &[], &bears::PreyIndex::default(), 0.1);
            assert!(
                h.clear(a.position, a.size(), true),
                "{kind:?}: {:?}",
                a.position
            );
            assert!(a.valid(), "{kind:?}: {a:?}");
        }
    }
}

#[test]
fn squid_inks_a_close_submerged_threat_then_has_a_real_cooldown() {
    let (t, c, j) = fixture();
    let h = Habitat {
        terrain: &t,
        chunks: &c,
        journal: &j,
    };
    let mut squid = animal(Kind::Squid, 0);
    let threat = [[8.0, 5.4, 7.0]];
    step(&mut squid, &h, &threat, &bears::PreyIndex::default(), 0.1);
    assert_eq!(squid.behavior, Behavior::Ink);
    assert_eq!(squid.needs.search_in, 9.0);
    for _ in 0..9 {
        step(&mut squid, &h, &threat, &bears::PreyIndex::default(), 0.1);
    }
    assert_ne!(squid.behavior, Behavior::Ink);
    assert!(squid.needs.search_in > 8.0);
    let mut dry_threat = animal(Kind::Squid, 1);
    step(
        &mut dry_threat,
        &h,
        &[[8.0, 14.0, 8.0]],
        &bears::PreyIndex::default(),
        0.1,
    );
    assert_ne!(dry_threat.behavior, Behavior::Ink);
}

#[test]
fn hostile_sharks_bite_with_cooldown_but_hammerheads_and_jellies_do_not_attack_players() {
    let (t, c, j) = fixture();
    let h = Habitat {
        terrain: &t,
        chunks: &c,
        journal: &j,
    };
    for kind in [
        Kind::ReefShark,
        Kind::WhaleShark,
        Kind::HammerheadShark,
        Kind::MoonJelly,
    ] {
        let mut a = animal(kind, 0);
        let mut hits = 0;
        for _ in 0..30 {
            if let bears::Effect::StrikePlayer { index, damage } = step(
                &mut a,
                &h,
                &[[8.0, 5.5, 7.2]],
                &bears::PreyIndex::default(),
                0.1,
            ) {
                assert_eq!(index, 0);
                assert!(damage > 0.0);
                hits += 1;
            }
        }
        if kind.hostile_to_players() {
            assert!((1..=2).contains(&hits), "{kind:?}: {hits}");
        } else {
            assert_eq!(hits, 0);
        }
    }
}

#[test]
fn shark_fish_catches_are_revalidated_and_cannot_consume_one_fish_twice() {
    let (t, c, j) = fixture();
    let h = Habitat {
        terrain: &t,
        chunks: &c,
        journal: &j,
    };
    for variant in [0, 2] {
        let mut predator = animal(Kind::HammerheadShark, 0);
        predator.needs.food = 5.0;
        let mut fish = animal(Kind::Fish, 1);
        fish.position = center(&predator);
        fish.variant = variant;
        let mut population = CreaturePopulation::default();
        population.explored.insert([0, 0]);
        population.animals.insert(predator.id, predator.clone());
        population.animals.insert(fish.id, fish.clone());
        assert!(catch_fish(&mut population, predator.id, fish.id, &h));
        assert!(!catch_fish(&mut population, predator.id, fish.id, &h));
        assert_eq!(population.animals[&fish.id].behavior, Behavior::Dead);
        assert_eq!(population.animals[&predator.id].needs.food, 17.0);
        assert!(population.valid());
    }
}

#[test]
fn jelly_can_rest_on_the_actual_floor_and_squid_can_hold_an_adjacent_wall() {
    let (t, c, mut j) = fixture();
    let mut jelly = animal(Kind::MoonJelly, 0);
    jelly.position[1] = 3.02;
    jelly.behavior = Behavior::Rest;
    jelly.timer = 4.0;
    let h = Habitat {
        terrain: &t,
        chunks: &c,
        journal: &j,
    };
    let before = jelly.position;
    step(&mut jelly, &h, &[], &bears::PreyIndex::default(), 0.1);
    assert_eq!(jelly.position, before);
    assert_eq!(jelly.velocity, [0.0; 3]);
    j.record([9, 5, 8], 3).unwrap();
    let h = Habitat {
        terrain: &t,
        chunks: &c,
        journal: &j,
    };
    let mut squid = animal(Kind::Squid, 1);
    squid.behavior = Behavior::Hold;
    let before = squid.position;
    step(&mut squid, &h, &[], &bears::PreyIndex::default(), 0.1);
    assert_eq!(squid.position, before);
    assert_eq!(squid.velocity, [0.0; 3]);
}
