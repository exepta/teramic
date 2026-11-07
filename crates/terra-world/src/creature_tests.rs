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
        landscape_revision: 3,
        sea_level: 8.0,
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
fn walking_sweeps_reject_intermediate_gaps_walls_and_unknown_cells() {
    let (terrain, chunks, mut journal) = fixture();
    let start = [7.5, 3.0, 8.5];
    let end = [12.5, 3.0, 8.5];
    for x in 9..=10 {
        for z in 6..=10 {
            for y in 0..=2 {
                journal.record([x, y, z], 0).unwrap();
            }
        }
    }
    let habitat = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    assert!(habitat.ground(end, CreatureKind::Pig).is_some());
    assert!(
        habitat
            .walk_step(start, end, CreatureKind::Pig, 0.0)
            .is_none()
    );
    let mut journal = EditJournal::default();
    for z in 6..=10 {
        for y in 3..=6 {
            journal.record([9, y, z], 3).unwrap();
        }
    }
    let habitat = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    assert!(habitat.ground(end, CreatureKind::Pig).is_some());
    assert!(
        habitat
            .walk_step(start, end, CreatureKind::Pig, 0.0)
            .is_none()
    );
    assert!(
        habitat
            .walk_step(start, [f64::NAN, 3.0, 8.5], CreatureKind::Pig, 0.0)
            .is_none()
    );
    assert!(
        habitat
            .walk_step([63.0, 3.0, 8.0], [65.0, 3.0, 8.0], CreatureKind::Pig, 0.0)
            .is_none()
    );
}

#[test]
fn mammals_walk_down_consecutive_full_block_steps_without_balancing_on_the_edge() {
    let (terrain, chunks, mut journal) = fixture();
    for x in 0..=9 {
        let height = if x < 8 { 6 } else { 13 - x };
        for z in 2..=13 {
            for y in 3..height {
                journal.record([x, y, z], 3).unwrap();
            }
        }
    }
    let habitat = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    // A 1.5m body spans three stair levels here. Route validation remains
    // conservative; real movement must retain its two genuine rear contacts.
    let edge = [8.5, 6.0, 8.0];
    let yaw = -std::f64::consts::FRAC_PI_2;
    assert!(
        habitat
            .ground_sized(edge, CreatureKind::Cow.size(), yaw)
            .is_none()
    );
    assert_eq!(
        habitat.supported_sized(edge, CreatureKind::Cow.size(), yaw),
        Some(edge)
    );
    for kind in [CreatureKind::Cow, CreatureKind::Sheep, CreatureKind::Pig] {
        let mut a = animal(kind);
        a.position = [6.5, 6.0, 8.0];
        a.yaw = yaw as f32;
        a.timer = 50.0;
        for _ in 0..600 {
            let before = a.position;
            step_budgeted(&mut a, &habitat, &[], 1.0 / 60.0, false);
            assert!(habitat.clear_swimmer_rotated(a.position, a.size(), f64::from(a.yaw)));
            assert!(
                a.position[0] >= before[0] - 0.00001,
                "{kind:?} turned away at {a:?}"
            );
            assert!(
                a.velocity[1].abs() <= 16.0,
                "a support step is not a vertical impulse"
            );
            assert!(a.valid());
            if a.position[0] >= 12.0 {
                break;
            }
        }
        assert!(
            a.position[0] >= 12.0,
            "{kind:?} stopped on the stairs at {a:?}"
        );
        assert!((a.position[1] - 3.0).abs() < 0.001);
        assert_eq!(a.velocity[1], 0.0);
    }
}

#[test]
fn real_partial_hoof_contact_never_treats_missing_sections_or_removed_floor_as_support() {
    let (terrain, mut chunks, mut journal) = fixture();
    let mut a = animal(CreatureKind::Cow);
    a.position = [8.0, 6.0, 8.0];
    a.yaw = -std::f32::consts::FRAC_PI_2;
    a.behavior = CreatureBehavior::Rest;
    a.timer = 50.0;
    for z in 6..=10 {
        journal.record([7, 5, z], 3).unwrap();
    }
    {
        let habitat = Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        };
        assert_eq!(
            habitat.supported_sized(a.position, a.size(), f64::from(a.yaw)),
            Some(a.position)
        );
        for _ in 0..10 {
            step_budgeted(&mut a, &habitat, &[], 1.0 / 60.0, false);
        }
        assert_eq!(a.position[1], 6.0);
    }
    for z in 6..=10 {
        journal.record([7, 5, z], 0).unwrap();
    }
    {
        let habitat = Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        };
        assert!(
            habitat
                .supported_sized(a.position, a.size(), f64::from(a.yaw))
                .is_none()
        );
        step_budgeted(&mut a, &habitat, &[], 0.1, false);
        assert!(a.position[1] < 6.0 && a.velocity[1] < 0.0);
        for _ in 0..30 {
            step_budgeted(&mut a, &habitat, &[], 0.1, false);
        }
        assert!((a.position[1] - 3.0).abs() < 0.001);
        assert!(a.valid());
    }
    chunks.loaded.remove(&SectionPos::new(0, 0, 0));
    let habitat = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    assert!(
        habitat
            .supported_sized(a.position, a.size(), f64::from(a.yaw))
            .is_none()
    );
    assert!(
        habitat
            .travel_step(a.position, [9.0, 3.0, 8.0], a.kind, f64::from(a.yaw))
            .is_none()
    );
}

#[test]
fn genuine_land_animal_overlaps_resolve_locally_onto_real_floor_or_partial_shapes() {
    for (position, material) in [([8.25, 3.0, 8.5], 3), ([8.5, 3.35, 8.5], 82)] {
        let (terrain, chunks, mut journal) = fixture();
        journal.record([8, 3, 8], material).unwrap();
        if material == 82 {
            journal
                .set_object_state([8, 3, 8], Some(crate::edits::BlockObjectState::default()))
                .unwrap();
        }
        let habitat = Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        };
        let mut a = animal(CreatureKind::Pig);
        a.position = position;
        a.behavior = CreatureBehavior::Rest;
        assert!(!habitat.clear_swimmer_rotated(a.position, a.size(), f64::from(a.yaw)));
        step_budgeted(&mut a, &habitat, &[], 1.0 / 60.0, false);
        assert!(
            habitat.clear_swimmer_rotated(a.position, a.size(), f64::from(a.yaw)),
            "{a:?}"
        );
        assert_eq!(
            a.velocity, [0.0; 3],
            "a recovered pose is not a ballistic impulse"
        );
        assert!(
            habitat
                .supported_sized(a.position, a.size(), f64::from(a.yaw))
                .is_some()
        );
        assert!(distance(position, a.position) <= 1.001_f64.powi(2));
        assert!(a.valid());
        if material == 82 {
            assert_eq!(
                a.position[1], 3.5,
                "use the slab surface, not the full block above it"
            );
        } else {
            assert!(a.position[0] < 8.0 && a.position[1] == 3.0);
        }
    }
    let (terrain, chunks, journal) = fixture();
    let habitat = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    let mut a = animal(CreatureKind::Pig);
    a.position[1] = 2.8;
    a.behavior = CreatureBehavior::Rest;
    step_budgeted(&mut a, &habitat, &[], 0.1, false);
    assert_eq!(a.position[1], 3.0);
    assert!(a.valid() && habitat.clear_swimmer_rotated(a.position, a.size(), f64::from(a.yaw)));
}

#[test]
fn overlap_recovery_cannot_cross_a_thick_wall_roof_or_unknown_section() {
    let (terrain, mut chunks, mut journal) = fixture();
    for x in 6..=10 {
        for z in 6..=10 {
            journal.record([x, 4, z], 3).unwrap();
        }
    }
    for x in 7..=9 {
        journal.record([x, 3, 8], 3).unwrap();
    }
    let mut a = animal(CreatureKind::Pig);
    a.position = [8.5, 3.0, 8.5];
    a.behavior = CreatureBehavior::Rest;
    let origin = a.position;
    {
        let habitat = Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        };
        for _ in 0..5 {
            step_budgeted(&mut a, &habitat, &[], 0.1, false);
            assert_eq!(
                a.position, origin,
                "cannot escape through newly encountered solids"
            );
        }
    }
    chunks.loaded.remove(&SectionPos::new(0, 0, 0));
    step_budgeted(
        &mut a,
        &Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        },
        &[],
        0.1,
        false,
    );
    assert_eq!(a.position, origin);
    assert!(a.valid());
}

#[test]
fn owned_animals_ignore_legacy_follow_and_keep_waiting_across_loaded_region_changes() {
    use terra_core::creatures::taming::CompanionMode;
    let (terrain, mut chunks, mut journal) = fixture();
    for z in 6..=10 {
        for y in 3..=6 {
            journal.record([11, y, z], 3).unwrap();
        }
    }
    for kind in [
        CreatureKind::Cow,
        CreatureKind::Sheep,
        CreatureKind::Pig,
        CreatureKind::Horse,
        CreatureKind::SmallSpider,
        CreatureKind::MediumSpider,
    ] {
        let mut a = animal(kind);
        if kind.is_spider() {
            a.position[1] += 0.015;
        }
        let mut navigation = navigation::Navigation::default();
        let owner = Some([22.0, 3.0, 8.0]);
        navigation::testing::reset_path_calls();
        {
            let habitat = Habitat {
                terrain: &terrain,
                chunks: &chunks,
                journal: &journal,
            };
            let origin = a.position;
            for i in 0..50 {
                let before = a.position;
                companions::step(
                    &mut a,
                    &habitat,
                    owner,
                    CompanionMode::Follow,
                    0.1,
                    i % 5 == 0,
                    Some(&mut navigation),
                );
                assert_eq!(a.position[0], before[0]);
                assert_eq!(a.position[2], before[2]);
                assert!(habitat.clear_rotated(a.position, a.size(), false, f64::from(a.yaw)));
            }
            assert_eq!(a.position[0], origin[0]);
            assert_eq!(a.position[2], origin[2]);
            assert_eq!(navigation::testing::path_calls(), 0);
            let intent = navigation.inspection(&a);
            assert_eq!(
                intent.reason,
                terra_core::creatures::intentions::IntentReason::WaitForOwner
            );
            assert!(intent.destination.is_none());
            assert!(intent.path.is_empty());
            let waiting = a.position;
            for _ in 0..20 {
                companions::step(
                    &mut a,
                    &habitat,
                    owner,
                    CompanionMode::Wait,
                    0.1,
                    false,
                    Some(&mut navigation),
                );
            }
            assert_eq!(a.position, waiting);
        }
        let section = terra_voxel::global_to_section(a.position.map(|v| v.floor() as i64)).0;
        let loaded = chunks.loaded.remove(&section).unwrap();
        let before = a.position;
        companions::step(
            &mut a,
            &Habitat {
                terrain: &terrain,
                chunks: &chunks,
                journal: &journal,
            },
            Some([30.0, 3.0, 8.0]),
            CompanionMode::Follow,
            0.1,
            true,
            Some(&mut navigation),
        );
        assert_eq!(a.position, before);
        chunks.loaded.insert(section, loaded);
        companions::step(
            &mut a,
            &Habitat {
                terrain: &terrain,
                chunks: &chunks,
                journal: &journal,
            },
            Some([30.0, 3.0, 8.0]),
            CompanionMode::Follow,
            0.1,
            true,
            Some(&mut navigation),
        );
        assert_eq!(a.position, before);
    }
}

#[test]
fn tamed_spiders_have_hunger_but_no_thirst_damage_or_owner_attacks_while_waiting() {
    let (terrain, chunks, journal) = fixture();
    let habitat = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    for kind in [CreatureKind::SmallSpider, CreatureKind::MediumSpider] {
        let mut a = animal(kind);
        a.position[1] += 0.015;
        let health = a.health;
        for _ in 0..100 {
            companions::step(
                &mut a,
                &habitat,
                Some([8.0, 3.0, 8.0]),
                terra_core::creatures::taming::CompanionMode::Wait,
                0.1,
                false,
                None,
            );
        }
        assert_eq!(kind.food(), 50.0);
        assert_eq!(a.needs.water, 0.0);
        assert!(a.needs.food < 50.0 && a.needs.food > 49.0);
        assert_eq!(a.health, health);
        assert_eq!(a.behavior, CreatureBehavior::Rest);
    }
}

#[test]
fn companion_starvation_and_spider_submersion_keep_dead_entities_valid_in_wait_and_food_modes() {
    let (terrain, chunks, mut journal) = fixture();
    let mut a = animal(CreatureKind::Cow);
    a.health = 0.001;
    a.needs.food = 0.0;
    companions::step(
        &mut a,
        &Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        },
        None,
        terra_core::creatures::taming::CompanionMode::Wait,
        0.1,
        false,
        None,
    );
    assert_eq!(a.health, 0.0);
    assert_eq!(a.behavior, CreatureBehavior::Dead);
    assert!(a.valid());
    let terrain = TerraGenerator::new(WorldConfig {
        water: true,
        sea_level: -30.0,
        ..terrain.config().clone()
    })
    .unwrap();
    for x in 5..=11 {
        for z in 5..=11 {
            for y in 3..=5 {
                journal
                    .set_fluid(
                        [x, y, z],
                        crate::water::FluidCell {
                            amount: 8,
                            source: true,
                            flow: [0, 0],
                        },
                    )
                    .unwrap();
            }
        }
    }
    let habitat = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    let mut spider = animal(CreatureKind::MediumSpider);
    spider.id = terra_core::creatures::spawning::next_command_id(&Default::default()).unwrap();
    spider.health = 0.04;
    let mut hunter = spider.clone();
    companions::step(
        &mut spider,
        &habitat,
        None,
        terra_core::creatures::taming::CompanionMode::Wait,
        0.1,
        false,
        None,
    );
    assert_eq!(spider.health, 0.0);
    assert_eq!(spider.behavior, CreatureBehavior::Dead);
    assert_eq!(spider.velocity, [0.0; 3]);
    assert!(spider.valid());
    hunter.needs.food = 20.0;
    let mut food = terra_core::GameplayState::default();
    food.drop_one(
        [8.5, 3.4, 8.0],
        terra_core::ItemKind::Animal(AnimalProduct::Beef),
    );
    let index = spider_hunting::FoodIndex::new(std::iter::empty(), food.drops.iter());
    let result = spider_hunting::step(
        &mut hunter,
        &habitat,
        &index,
        [8, 2, 8],
        0.1,
        true,
        &mut navigation::Navigation::default(),
    );
    assert_eq!(result, Some(None));
    assert_eq!(hunter.behavior, CreatureBehavior::Dead);
    assert!(hunter.valid());
}

#[test]
fn spider_den_requests_are_stable_and_hungry_search_consumes_real_food_only_inside_twenty_meters() {
    use terra_core::{
        GameplayState, ItemKind,
        creatures::{AnimalProduct, taming::food_gain},
    };
    let (terrain, chunks, journal) = fixture();
    let habitat = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    let mut a = animal(CreatureKind::MediumSpider);
    a.id = terra_core::creatures::spawning::next_command_id(&Default::default()).unwrap();
    a.position[1] += 0.015;
    a.needs.food = 20.0;
    a.timer = 0.0;
    let den = crate::creatures::spiders::den_request(&a, &habitat).unwrap();
    a.age = 300.0;
    assert_eq!(
        crate::creatures::spiders::den_request(&a, &habitat)
            .unwrap()
            .id,
        den.id
    );
    let mut gameplay = GameplayState::default();
    assert!(gameplay.drop_stack(
        [8.8, 3.3, 8.0],
        terra_core::ItemStack {
            item: ItemKind::Animal(AnimalProduct::Beef),
            count: 2
        },
        [0.0; 3],
        0.0
    ));
    let mut population = CreaturePopulation::default();
    population.animals.insert(a.id, a.clone());
    let index = spider_hunting::FoodIndex::new(std::iter::empty(), gameplay.drops.iter());
    let mut navigation = navigation::Navigation::default();
    let meal = spider_hunting::step(
        &mut a,
        &habitat,
        &index,
        den.anchor,
        0.1,
        true,
        &mut navigation,
    )
    .flatten()
    .unwrap();
    population.animals.insert(a.id, a.clone());
    assert!(spider_hunting::eat(
        &mut population,
        &mut gameplay,
        &habitat,
        a.id,
        meal
    ));
    assert_eq!(gameplay.drops[0].stack.count, 1);
    assert_eq!(population.animals[&a.id].needs.food, a.needs.food + 15.0);
    assert_eq!(
        spider_hunting::step(
            &mut a,
            &habitat,
            &index,
            den.anchor,
            0.1,
            true,
            &mut navigation
        ),
        Some(None),
        "a second bite must respect the existing combat cooldown"
    );
    assert!(food_gain(CreatureKind::MediumSpider, ItemKind::Stick).is_none());
    let mut far = gameplay.drops[0].clone();
    far.position = [30.0, 3.3, 8.0];
    let far_index = spider_hunting::FoodIndex::new(std::iter::empty(), std::iter::once(&far));
    a.needs.goal = None;
    assert!(
        spider_hunting::step(
            &mut a,
            &habitat,
            &far_index,
            den.anchor,
            0.1,
            true,
            &mut navigation
        )
        .is_none()
    );
    assert!(!spider_hunting::eat(
        &mut population,
        &mut gameplay,
        &habitat,
        a.id,
        spider_hunting::Meal::Drop(u64::MAX)
    ));
}

#[test]
fn idle_animals_fall_to_real_loaded_support_after_their_floor_is_removed() {
    let (terrain, mut chunks, mut journal) = fixture();
    let p = SectionPos::new(0, -1, 0);
    let mut lower = BlockSection::empty(p).unwrap();
    for x in 0..16 {
        for z in 0..16 {
            lower.set_block([x, 12, z], 3).unwrap();
        }
    }
    chunks.loaded.insert(
        p,
        LoadedSection {
            data: lower,
            mesh: None,
            mesh_epoch: 0,
            water_epoch: 0,
            water: None,
            props: vec![],
        },
    );
    for x in 5..=11 {
        for z in 5..=11 {
            for y in 0..=2 {
                journal.record([x, y, z], 0).unwrap();
            }
        }
    }
    let habitat = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    for kind in [
        CreatureKind::Cow,
        CreatureKind::Pig,
        CreatureKind::Sheep,
        CreatureKind::Horse,
        CreatureKind::Bear,
    ] {
        let mut a = animal(kind);
        a.behavior = CreatureBehavior::Rest;
        a.timer = 5.0;
        for _ in 0..30 {
            step_budgeted(&mut a, &habitat, &[], 0.1, false);
        }
        assert!(
            (a.position[1] + 3.0).abs() < 0.02,
            "{kind:?}: {:?}",
            a.position
        );
        assert!(
            habitat
                .ground_rotated(a.position, kind, f64::from(a.yaw))
                .is_some()
        );
    }
}

#[test]
fn shaped_support_uses_slab_and_stair_surfaces_and_releases_removed_support() {
    for (material, height) in [(82, 3.5), (74, 4.0)] {
        let (terrain, chunks, mut journal) = fixture();
        for x in 6..=10 {
            for z in 6..=10 {
                journal.record([x, 3, z], material).unwrap();
                journal
                    .set_object_state([x, 3, z], Some(crate::edits::BlockObjectState::default()))
                    .unwrap();
            }
        }
        let mut a = animal(CreatureKind::Pig);
        a.position = [8.5, height, 8.5];
        a.behavior = CreatureBehavior::Rest;
        {
            let habitat = Habitat {
                terrain: &terrain,
                chunks: &chunks,
                journal: &journal,
            };
            // An unapplied journal edit must select the same sub-cell profile
            // as the effective material used by collision and support checks.
            assert_eq!(chunks.block_at([8, 3, 8]), Some(0));
            assert_eq!(habitat.block([8, 3, 8]), Some(material));
            assert_eq!(habitat.ground(a.position, a.kind), Some(a.position));
            assert!(habitat.clear(a.position, a.kind.size(), false));
            step_budgeted(&mut a, &habitat, &[], 0.1, false);
            assert_eq!(a.position[1], height);
        }
        for x in 6..=10 {
            for z in 6..=10 {
                journal.record([x, 3, z], 0).unwrap();
            }
        }
        let habitat = Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        };
        for _ in 0..15 {
            step_budgeted(&mut a, &habitat, &[], 0.1, false);
        }
        assert_eq!(a.position[1], 3.0, "material={material}");
    }
}

#[test]
fn walking_mammals_enter_real_water_and_buoyancy_immerses_them() {
    let (terrain, chunks, mut journal) = fixture();
    let terrain = TerraGenerator::new(WorldConfig {
        water: true,
        sea_level: -30.0,
        ..terrain.config().clone()
    })
    .unwrap();
    for x in 8..=24 {
        for z in 3..=12 {
            for y in 0..=2 {
                journal.record([x, y, z], 0).unwrap();
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
    let habitat = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    assert!(
        habitat
            .walk_step([6.5, 3.0, 8.0], [9.5, 3.0, 8.0], CreatureKind::Pig, 0.0)
            .is_none(),
        "dry-land paths must not invent a floor over water"
    );
    for kind in [
        CreatureKind::Cow,
        CreatureKind::Sheep,
        CreatureKind::Pig,
        CreatureKind::Horse,
        CreatureKind::Bear,
    ] {
        let mut a = animal(kind);
        a.position = [6.5, 3.0, 8.0];
        a.yaw = -std::f32::consts::FRAC_PI_2;
        a.timer = 10.0;
        for _ in 0..60 {
            step_budgeted(&mut a, &habitat, &[], 0.1, false);
            assert!(habitat.clear_swimmer_rotated(a.position, a.size(), f64::from(a.yaw)));
        }
        assert!(a.position[0] > 8.6, "{kind:?}: {:?}", a.position);
        assert!(habitat.wet(a.position), "{kind:?}: {:?}", a.position);
        assert!(a.position[1] < 2.8, "{kind:?}: {:?}", a.position);
        assert_eq!(a.behavior, CreatureBehavior::Swim);
    }
}

#[test]
fn starvation_keeps_wild_and_companion_mammals_valid_and_dead_on_land_and_in_loaded_water() {
    let (terrain, chunks, dry_journal) = fixture();
    let terrain = TerraGenerator::new(WorldConfig {
        water: true,
        sea_level: -30.0,
        ..terrain.config().clone()
    })
    .unwrap();
    let mut wet_journal = dry_journal.clone();
    for x in 5..=11 {
        for z in 5..=11 {
            for y in 3..=5 {
                wet_journal
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
    for kind in [
        CreatureKind::Cow,
        CreatureKind::Sheep,
        CreatureKind::Pig,
        CreatureKind::Horse,
        CreatureKind::Bear,
    ] {
        for wet in [false, true] {
            let habitat = Habitat {
                terrain: &terrain,
                chunks: &chunks,
                journal: if wet { &wet_journal } else { &dry_journal },
            };
            for companion in [false, true] {
                let mut a = animal(kind);
                if wet {
                    a.position[1] += 0.2;
                    a.behavior = CreatureBehavior::Swim;
                }
                a.health = 0.001;
                a.needs.food = 0.0;
                a.needs.goal = Some(NeedGoal {
                    kind: NeedKind::Food,
                    position: [8, 2, 8],
                });
                a.velocity = [1.0, 0.25, -0.75];
                assert!(a.valid());
                assert_eq!(habitat.wet(a.position), wet);
                let before = a.position;
                let update = |a: &mut Creature| {
                    if companion {
                        companions::step(
                            a,
                            &habitat,
                            Some([12.0, 3.0, 8.0]),
                            taming::CompanionMode::Wait,
                            0.1,
                            false,
                            None,
                        );
                    } else {
                        step_budgeted(a, &habitat, &[], 0.1, false);
                    }
                };
                update(&mut a);
                assert_eq!(a.health, 0.0, "{kind:?}, wet={wet}, companion={companion}");
                assert_eq!(a.behavior, CreatureBehavior::Dead);
                assert_eq!(a.velocity, [0.0; 3]);
                assert!(a.needs.goal.is_none());
                assert_eq!(
                    a.position, before,
                    "a lethal need update must not resume movement"
                );
                let dead = a.clone();
                for _ in 0..20 {
                    update(&mut a);
                    assert_eq!(a, dead);
                }
                let mut gameplay = terra_core::GameplayState::default();
                gameplay.creatures.explored.insert(a.home);
                if companion {
                    gameplay
                        .creatures
                        .owners
                        .insert(a.id, terra_core::PlayerUuid::from_u128(1));
                }
                gameplay.creatures.animals.insert(a.id, a);
                assert!(
                    gameplay.valid(),
                    "{kind:?}, wet={wet}, companion={companion}"
                );
            }
        }
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
        for _ in 0..5 {
            step_budgeted(&mut a, &h, &[], 0.1, false);
            assert!(
                a.position[1] <= before + 0.001,
                "{kind:?} lifted onto ledge later: {:?}",
                a.position
            );
        }
        if kind != CreatureKind::Bear {
            assert!(
                a.position[1] < before - 0.1,
                "{kind:?} must slide clear and continue real gravity"
            );
            assert!(h.clear_swimmer_rotated(a.position, a.size(), f64::from(a.yaw)));
        }
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
                assert!(!a.juvenile());
                assert_eq!(a.size(), kind.size());
                assert_eq!(
                    crate::water::fluid_motion_at(&t, &c, &j, [start, 4.5, 8.0]).velocity[1],
                    0.0,
                    "a calm full pool cell below partial surface water is not a falling shaft"
                );
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
                assert!(
                    (a.position[1] - (5.5 - (kind.size()[1] * 0.68).min(0.9))).abs() < 0.12,
                    "{kind:?} adult at seam {start}: {:?}, size {:?}",
                    a.position,
                    a.size()
                );
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
fn natural_land_spawns_validate_their_saved_heading_instead_of_an_unrotated_body() {
    let (terrain, chunks, _) = fixture();
    let mut exercised_corner = false;
    'seeds: for seed in 0..16 {
        let mut config = terrain.config().clone();
        config.seed = terra_core::WorldSeed(seed);
        let terrain = TerraGenerator::new(config).unwrap();
        let journal = EditJournal::default();
        let mut baseline = CreaturePopulation::default();
        assert!(spawn_cell(
            &mut baseline,
            &Habitat {
                terrain: &terrain,
                chunks: &chunks,
                journal: &journal
            },
            [0, 0]
        ));
        for candidate in baseline.animals.values() {
            for x in
                candidate.position[0].floor() as i64 - 2..=candidate.position[0].floor() as i64 + 2
            {
                for z in candidate.position[2].floor() as i64 - 2
                    ..=candidate.position[2].floor() as i64 + 2
                {
                    let mut journal = EditJournal::default();
                    journal.record([x, 3, z], 3).unwrap();
                    let habitat = Habitat {
                        terrain: &terrain,
                        chunks: &chunks,
                        journal: &journal,
                    };
                    if !habitat.clear_rotated(candidate.position, candidate.size(), false, 0.0)
                        || habitat.clear_rotated(
                            candidate.position,
                            candidate.size(),
                            false,
                            f64::from(candidate.yaw),
                        )
                    {
                        continue;
                    }
                    let mut population = CreaturePopulation::default();
                    assert!(spawn_cell(&mut population, &habitat, [0, 0]));
                    assert!(!population.animals.contains_key(&candidate.id));
                    for actual in population.animals.values() {
                        assert!(habitat.clear_rotated(
                            actual.position,
                            actual.size(),
                            false,
                            f64::from(actual.yaw)
                        ));
                    }
                    exercised_corner = true;
                    break 'seeds;
                }
            }
        }
    }
    assert!(
        exercised_corner,
        "fixture must exercise a real orientation-only wall collision"
    );
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
    // The former z=8 start overlapped this wall by 0.75m, which is a
    // penetration-recovery fixture rather than a free navigation start.
    a.position[2] = 8.75001;
    let before = a.position;
    assert!(h.clear_swimmer_rotated(a.position, a.size(), f64::from(a.yaw)));
    assert!(h.walk_step(before, [8.0, 3.0, 6.0], a.kind, 0.0).is_none());
    step(&mut a, &h, &[], 0.1);
    let (sin, cos) = f64::from(a.yaw).sin_cos();
    let front = a.position[2] - (a.size()[0] * sin.abs() + a.size()[2] * cos.abs()) * 0.5;
    assert!(
        front >= 8.0 - 0.000001,
        "navigation cannot cut through the front wall"
    );
    assert!(h.clear_swimmer_rotated(a.position, a.size(), f64::from(a.yaw)));
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
    a.position = before;
    a.yaw = 0.0;
    a.behavior = CreatureBehavior::Rest;
    a.timer = 10.0;
    assert!(h.walk_step(before, [8.0, 3.0, 5.0], a.kind, 0.0).is_none());
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
fn juvenile_horse_routes_and_walks_under_a_real_one_block_passage_using_its_own_size() {
    let (terrain, chunks, mut journal) = fixture();
    for x in 9..=13 {
        for z in 4..=12 {
            journal.record([x, 4, z], 3).unwrap();
        }
    }
    let habitat = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    let mut horse = animal(CreatureKind::Horse);
    horse.id = terra_core::creatures::breeding::next_birth_id(&Default::default(), 0).unwrap();
    horse.position = [7.5, 3.0, 8.5];
    horse.yaw = -std::f32::consts::FRAC_PI_2;
    horse.timer = 50.0;
    let target = [12.5, 3.0, 8.5];
    assert!(
        habitat
            .walk_step(horse.position, target, horse.kind, f64::from(horse.yaw))
            .is_none()
    );
    assert!(
        habitat
            .walk_step_sized(horse.position, target, horse.size(), f64::from(horse.yaw))
            .is_some()
    );
    for _ in 0..25 {
        step_budgeted(&mut horse, &habitat, &[], 0.1, false);
        assert!(horse.valid());
        assert!(habitat.clear_swimmer_rotated(horse.position, horse.size(), f64::from(horse.yaw)));
    }
    assert!(
        horse.position[0] > 10.0,
        "juvenile must actually pass the entrance: {:?}",
        horse.position
    );
}

#[test]
fn authoritative_picking_ignores_horse_side_air_and_keeps_actual_body_and_baby_hits() {
    let mut horse = animal(CreatureKind::Horse);
    assert!(ray_distance(&horse, [8.8, 4.0, 5.0], [0.0, 0.0, 1.0], 6.0).is_none());
    assert!(ray_distance(&horse, [8.0, 4.0, 5.0], [0.0, 0.0, 1.0], 6.0).is_some());
    horse.id = terra_core::creatures::breeding::next_birth_id(&Default::default(), 0).unwrap();
    assert!(ray_distance(&horse, [8.0, 4.0, 5.0], [0.0, 0.0, 1.0], 6.0).is_none());
    assert!(ray_distance(&horse, [8.0, 3.46, 5.0], [0.0, 0.0, 1.0], 6.0).is_some());
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
    let denied = apply_forage(
        Forage::Prop {
            section: owner,
            id: grass.id,
        },
        &mut cow,
        &mut chunks,
        &mut journal,
        terra_core::GameRules {
            mob_griefing: false,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(!denied);
    assert!(!journal.prop_removed(owner, grass.id));
    assert_eq!(cow.needs.food, 10.0);
    assert_eq!(journal.forage_regrowth().count(), 0);
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
        apply_forage(
            effect,
            &mut cow,
            &mut chunks,
            &mut journal,
            Default::default(),
        )
        .unwrap();
    }
    assert!(cow.needs.food > 15.0);
    assert!(journal.prop_removed(owner, grass.id));
    assert!(
        journal
            .forage_regrowth()
            .any(|state| state.prop.id == grass.id)
    );
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
        apply_forage(
            effect,
            &mut pig,
            &mut chunks,
            &mut journal,
            Default::default(),
        )
        .unwrap();
    }
    assert!(pig.needs.food > 5.0 && journal.prop_removed(owner, mushroom.id));
}

#[test]
fn actual_hungry_animals_find_ripe_crops_and_graze_without_drops_or_duplicate_plants() {
    use crate::props::{PropId, WorldProp};
    use terra_core::{CropKind, block_properties::BlockPropertiesCatalog};
    for (kind, eater) in [
        (CropKind::Wheat, CreatureKind::Cow),
        (CropKind::Wheat, CreatureKind::Horse),
        (CropKind::Corn, CreatureKind::Pig),
        (CropKind::Lettuce, CreatureKind::Pig),
    ] {
        let (terrain, mut chunks, mut journal) = fixture();
        let support = [8, 2, 8];
        let owner = SectionPos::new(0, 0, 0);
        let prop = WorldProp {
            id: PropId(919),
            kind: kind.resource_prop().into(),
            position: [8.5, 3.0, 8.5],
            rotation: 0.0,
            scale: 1.0,
            variant: 0,
            section: owner,
            removed: false,
        };
        assert!(
            crate::farming::plant_crop(
                &mut chunks,
                &mut journal,
                &BlockPropertiesCatalog::default(),
                prop
            )
            .unwrap()
        );
        let id = PropId(journal.crop_at(support).unwrap().prop_id);
        let mut state = *journal.crop_at(support).unwrap();
        state.progress_seconds = state.growth_duration_seconds;
        crate::farming::commit_crop_state(&mut chunks, &mut journal, support, state).unwrap();
        let mut animal = animal(eater);
        animal.needs.food = 1.0;
        for _ in 0..65 {
            let effect = step(
                &mut animal,
                &Habitat {
                    terrain: &terrain,
                    chunks: &chunks,
                    journal: &journal,
                },
                &[],
                0.1,
            );
            apply_forage(
                effect,
                &mut animal,
                &mut chunks,
                &mut journal,
                Default::default(),
            )
            .unwrap();
        }
        assert!(animal.needs.food > 5.0, "{eater:?} must eat ripe {kind:?}");
        assert_eq!(journal.crop_at(support).unwrap().progress_seconds, 0.0);
        assert!(!journal.prop_removed(owner, id));
        assert_eq!(
            chunks.loaded[&owner]
                .props
                .iter()
                .filter(|plant| !plant.removed && plant.id == id && plant.variant == 0)
                .count(),
            1
        );
        assert!(journal.gameplay().drops.is_empty());
        assert_eq!(journal.forage_regrowth().count(), 0);
    }
}
