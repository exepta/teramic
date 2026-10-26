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

fn fixture() -> (TerraGenerator, ChunkStore, EditJournal, Creature) {
    let terrain = TerraGenerator::new(WorldConfig {
        caves: false,
        trees: false,
        biomes: false,
        water: false,
        ..Default::default()
    })
    .unwrap();
    let mut chunks = ChunkStore::default();
    for sx in -1..=1 {
        for sz in -1..=1 {
            let position = SectionPos::new(sx, 0, sz);
            let mut data = BlockSection::empty(position).unwrap();
            for x in 0..16 {
                for z in 0..16 {
                    data.set_block([x, 2, z], 3).unwrap();
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
    let horse = Creature {
        id: creature_id([0, 0], 0).unwrap(),
        home: [0, 0],
        kind: Kind::Horse,
        position: [8.0, 3.0, 8.0],
        velocity: [0.0; 3],
        yaw: 0.0,
        health: Kind::Horse.health(),
        behavior: Behavior::Rest,
        timer: 0.0,
        age: 0.0,
        variant: 0,
        needs: CreatureNeeds::full(Kind::Horse),
    };
    (terrain, chunks, EditJournal::default(), horse)
}

#[test]
fn mounted_horse_uses_real_central_hoof_support_without_rest_leap_oscillation() {
    let (terrain, chunks, mut journal, mut horse) = fixture();
    horse.position = [8.5, 3.0, 8.5];
    for x in 6..=10 {
        for z in 6..=10 {
            if [x, z] != [8, 8] {
                journal.record([x, 2, z], 0).unwrap();
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
            .ground_sized(horse.position, horse.size(), 0.0)
            .is_none()
    );
    assert!(
        grounded(&horse, &habitat),
        "the authored four hooves all stand on the center block"
    );
    let mut stamina = f32::from(horse.horse_traits().unwrap().stamina);
    for _ in 0..60 {
        assert!(step(
            &mut horse,
            &mut stamina,
            &habitat,
            RideInput::default(),
            1.0 / 60.0
        ));
        assert_eq!(horse.behavior, Behavior::Rest);
        assert_eq!(horse.velocity[1], 0.0);
        assert_eq!(horse.position[1], 3.0);
    }
    journal.record([8, 2, 8], 0).unwrap();
    let habitat = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    assert!(!step(
        &mut horse,
        &mut stamina,
        &habitat,
        RideInput::default(),
        0.1
    ));
    assert!(horse.position[1] < 3.0 && horse.velocity[1] < 0.0);
    assert_eq!(horse.behavior, Behavior::Leap);
    assert!(horse.valid());
}

#[test]
fn identical_mount_inputs_predict_identically_without_diagonal_boost_and_update_needs() {
    let (terrain, chunks, journal, horse) = fixture();
    let habitat = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    let mut a = horse.clone();
    let mut b = horse.clone();
    let mut straight = horse.clone();
    let mut sa = f32::from(horse.horse_traits().unwrap().stamina);
    let mut sb = sa;
    let mut ss = sa;
    for _ in 0..20 {
        let input = RideInput {
            direction: [1.0, 1.0],
            sprint: true,
            jump: false,
        };
        step(&mut a, &mut sa, &habitat, input, 0.1);
        step(&mut b, &mut sb, &habitat, input, 0.1);
        step(
            &mut straight,
            &mut ss,
            &habitat,
            RideInput {
                direction: [1.0, 0.0],
                ..input
            },
            0.1,
        );
        assert_eq!(a, b);
        assert_eq!(sa, sb);
        assert!(a.valid());
        assert!(clear(&a, &habitat));
    }
    assert!(
        (super::super::distance(a.position, horse.position).sqrt()
            - super::super::distance(straight.position, horse.position).sqrt())
        .abs()
            < 0.001
    );
    assert!(sa < f32::from(horse.horse_traits().unwrap().stamina));
    assert!(a.needs.food < horse.needs.food && a.needs.water < horse.needs.water);
    assert!((a.age - 2.0).abs() < 0.001);
}

#[test]
fn riding_accelerates_turns_and_brakes_continuously_and_preserves_airborne_momentum() {
    let (terrain, chunks, journal, mut horse) = fixture();
    let habitat = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    let mut stamina = f32::from(horse.horse_traits().unwrap().stamina);
    let dt = 1.0 / 60.0;
    let input = RideInput {
        direction: [1.0, 0.0],
        sprint: true,
        jump: false,
    };
    let mut previous = 0.0;
    for _ in 0..30 {
        let yaw = horse.yaw;
        step(&mut horse, &mut stamina, &habitat, input, dt);
        let speed = horse.velocity[0].hypot(horse.velocity[2]);
        assert!(speed >= previous - 0.001 && speed - previous <= 24.0 * dt + 0.001);
        let turn = (horse.yaw - yaw + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
            - std::f32::consts::PI;
        assert!(turn.abs() <= 6.0 * dt + 0.001);
        previous = speed;
    }
    let before = horse.velocity[0];
    step(
        &mut horse,
        &mut stamina,
        &habitat,
        RideInput {
            jump: true,
            ..input
        },
        dt,
    );
    assert!(horse.velocity[1] > 0.0);
    step(&mut horse, &mut stamina, &habitat, RideInput::default(), dt);
    assert!(
        horse.velocity[0] > before - 6.0 * dt * 2.0 - 0.001,
        "release must not stop a horse in midair"
    );
    for _ in 0..150 {
        step(&mut horse, &mut stamina, &habitat, RideInput::default(), dt);
    }
    assert!(horse.velocity[0].hypot(horse.velocity[2]) < 0.001);
    assert_eq!(horse.behavior, Behavior::Rest);
}

#[test]
fn riding_jump_uses_fixed_traits_at_sixty_and_ten_hz_and_has_no_timer_dependency() {
    let (terrain, chunks, journal, template) = fixture();
    let habitat = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    for dt in [1.0 / 60.0, 0.1] {
        for timer in [0.0, 40.0] {
            let mut horse = template.clone();
            horse.timer = timer;
            let traits = horse.horse_traits().unwrap();
            let mut stamina = f32::from(traits.stamina);
            assert!(!step(
                &mut horse,
                &mut stamina,
                &habitat,
                RideInput {
                    jump: true,
                    ..Default::default()
                },
                dt
            ));
            let mut peak = horse.position[1];
            for _ in 0..180 {
                let landed = step(&mut horse, &mut stamina, &habitat, RideInput::default(), dt);
                peak = peak.max(horse.position[1]);
                assert!(horse.valid());
                assert!(clear(&horse, &habitat));
                if landed {
                    break;
                }
            }
            assert!(peak > 3.0 + f64::from(traits.jump_height) - 0.03);
            assert!(peak <= 3.0 + f64::from(traits.jump_height) + 0.001);
            assert!((horse.position[1] - 3.0).abs() < 0.03);
            assert_eq!(horse.velocity[1], 0.0);
        }
    }
    let mut horse = template;
    let mut stamina = 0.0;
    assert!(step(
        &mut horse,
        &mut stamina,
        &habitat,
        RideInput {
            jump: true,
            ..Default::default()
        },
        0.1
    ));
    assert_eq!(horse.velocity[1], 0.0);
}

#[test]
fn rider_headroom_blocks_paths_and_jump_before_horse_or_player_passes_through_roofs() {
    let (terrain, chunks, mut journal, mut horse) = fixture();
    for x in 10..=14 {
        for z in 5..=11 {
            journal.record([x, 6, z], 3).unwrap();
        }
    }
    let habitat = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    let mut stamina = 100.0_f32.min(f32::from(horse.horse_traits().unwrap().stamina));
    for _ in 0..30 {
        step(
            &mut horse,
            &mut stamina,
            &habitat,
            RideInput {
                direction: [1.0, 0.0],
                ..Default::default()
            },
            0.1,
        );
        assert!(clear(&horse, &habitat));
    }
    assert!(horse.position[0] + RIDER_SIZE[0] * 0.5 <= 10.001);
    horse.position = [8.0, 3.0, 8.0];
    for x in 5..=9 {
        for z in 5..=11 {
            journal.record([x, 7, z], 3).unwrap();
        }
    }
    let habitat = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    let mut peak = 3.0_f64;
    for tick in 0..90 {
        step(
            &mut horse,
            &mut stamina,
            &habitat,
            RideInput {
                jump: tick == 0,
                ..Default::default()
            },
            1.0 / 60.0,
        );
        peak = peak.max(horse.position[1]);
        assert!(clear(&horse, &habitat));
    }
    assert!(peak < 3.81);
    assert!((horse.position[1] - 3.0).abs() < 0.03);
}

#[test]
fn removed_support_falls_and_missing_sections_are_never_a_floor_or_a_forward_path() {
    let (terrain, chunks, mut journal, mut horse) = fixture();
    for x in 5..=11 {
        for z in 5..=11 {
            journal.record([x, 2, z], 0).unwrap();
        }
    }
    let habitat = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    let mut stamina = f32::from(horse.horse_traits().unwrap().stamina);
    assert!(!step(
        &mut horse,
        &mut stamina,
        &habitat,
        RideInput::default(),
        0.1
    ));
    assert!(horse.position[1] < 3.0);
    assert!(horse.velocity[1] < 0.0);
    let empty = ChunkStore::default();
    let habitat = Habitat {
        terrain: &terrain,
        chunks: &empty,
        journal: &journal,
    };
    let original = horse.clone();
    let original_stamina = stamina;
    assert!(!step(
        &mut horse,
        &mut stamina,
        &habitat,
        RideInput {
            direction: [1.0, 0.0],
            jump: true,
            sprint: true
        },
        0.1
    ));
    assert_eq!(horse, original);
    assert_eq!(stamina, original_stamina);
}

#[test]
fn dismount_requires_real_clear_ground_and_a_clear_route_including_negative_coordinates() {
    let (terrain, chunks, mut journal, mut horse) = fixture();
    horse.position = [-8.0, 3.0, -8.0];
    let habitat = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    let feet = dismount(&horse, &habitat).unwrap();
    assert!(habitat.clear(feet, RIDER_SIZE, false));
    assert_eq!(feet[1], 3.0);
    assert!(super::super::distance(feet, horse.position) > 0.8);
    for x in -12..=-4 {
        for z in -12..=-4 {
            if (x as f64 + 0.5 - horse.position[0]).abs() > 0.6
                || (z as f64 + 0.5 - horse.position[2]).abs() > 1.2
            {
                for y in 3..=7 {
                    journal.record([x, y, z], 3).unwrap();
                }
            }
        }
    }
    let habitat = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    assert!(dismount(&horse, &habitat).is_none());
}

#[test]
fn malformed_riding_inputs_cannot_mutate_motion_stamina_or_simulation_age() {
    let (terrain, chunks, journal, original) = fixture();
    let habitat = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    for dt in [0.0, -0.1, 0.101, f32::NAN, f32::INFINITY] {
        let mut horse = original.clone();
        let mut stamina = 20.0;
        assert!(!step(
            &mut horse,
            &mut stamina,
            &habitat,
            RideInput::default(),
            dt
        ));
        assert_eq!(horse, original);
        assert_eq!(stamina, 20.0);
    }
    for direction in [[f32::NAN, 0.0], [0.0, f32::INFINITY], [2.0, 0.0]] {
        let mut horse = original.clone();
        let mut stamina = 20.0;
        assert!(!step(
            &mut horse,
            &mut stamina,
            &habitat,
            RideInput {
                direction,
                ..Default::default()
            },
            0.1
        ));
        assert_eq!(horse, original);
        assert_eq!(stamina, 20.0);
    }
}

#[test]
fn mounted_horse_enters_real_water_and_swims_below_its_surface_instead_of_standing_on_it() {
    let (terrain, chunks, mut journal, mut horse) = fixture();
    let terrain = TerraGenerator::new(WorldConfig {
        water: true,
        sea_level: -30.0,
        ..terrain.config().clone()
    })
    .unwrap();
    for x in 9..=15 {
        for z in 4..=12 {
            journal
                .record([x, 3, z], terra_voxel::MaterialId::FluidWater as u16)
                .unwrap();
        }
    }
    let habitat = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    assert!(!habitat.wet([9.0, 2.92, 8.0]));
    assert!(habitat.wet([9.0, 3.0, 8.0]));
    assert!(
        habitat
            .travel_step([8.65, 3.0, 8.0], [8.75, 3.0, 8.0], Kind::Horse, 0.0,)
            .is_some(),
        "the front hooves must enter shallow water above solid ground"
    );
    let mut stamina = f32::from(horse.horse_traits().unwrap().stamina);
    let mut swam = false;
    for _ in 0..70 {
        step(
            &mut horse,
            &mut stamina,
            &habitat,
            RideInput {
                direction: [1.0, 0.0],
                ..Default::default()
            },
            1.0 / 60.0,
        );
        assert!(clear(&horse, &habitat));
        if horse.behavior == Behavior::Swim {
            swam = true;
            assert!(
                horse.position[1] < 3.5,
                "water top is4.0, feet must stay immersed: {:?}",
                horse.position
            );
        }
    }
    assert!(swam && horse.position[0] > 10.0);
    for kind in [Kind::Cow, Kind::Horse] {
        let mut animal = horse.clone();
        animal.kind = kind;
        animal.health = kind.health();
        animal.needs = CreatureNeeds::full(kind);
        animal.position = [8.0, 3.0, 8.0];
        animal.velocity = [0.0; 3];
        animal.yaw = -std::f32::consts::FRAC_PI_2;
        animal.behavior = Behavior::Wander;
        animal.timer = 10.0;
        let mut swam = false;
        for _ in 0..180 {
            super::super::step_budgeted(&mut animal, &habitat, &[], 1.0 / 60.0, false);
            swam |= animal.behavior == Behavior::Swim;
            assert!(animal.valid());
            assert!(habitat.clear_swimmer_rotated(
                animal.position,
                animal.size(),
                f64::from(animal.yaw)
            ));
        }
        assert!(swam && animal.position[0] > 10.0, "{kind:?}: {animal:?}");
        assert!(animal.position[1] < 3.5);
    }
}

#[test]
fn mounted_ground_tracks_actual_half_slab_height_without_an_invisible_cube_floor() {
    let (terrain, chunks, mut journal, mut horse) = fixture();
    for x in 3..=13 {
        for z in 3..=13 {
            journal.record([x, 2, z], 82).unwrap();
        }
    }
    let habitat = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    horse.position[1] = 2.5;
    let mut stamina = f32::from(horse.horse_traits().unwrap().stamina);
    for _ in 0..20 {
        assert!(step(
            &mut horse,
            &mut stamina,
            &habitat,
            RideInput {
                direction: [0.0, -1.0],
                ..Default::default()
            },
            1.0 / 60.0
        ));
        assert!((horse.position[1] - 2.5).abs() < 0.001);
        assert!(clear(&horse, &habitat));
    }
}

#[test]
fn sixty_hz_shore_steps_are_ground_support_not_invalid_fifty_meter_vertical_impulses() {
    let (terrain, chunks, mut journal, mut horse) = fixture();
    let terrain = TerraGenerator::new(WorldConfig {
        water: true,
        sea_level: -30.0,
        ..terrain.config().clone()
    })
    .unwrap();
    for x in 5..=8 {
        for z in 4..=12 {
            journal
                .record([x, 3, z], terra_voxel::MaterialId::FluidWater as u16)
                .unwrap();
        }
    }
    for x in 9..=13 {
        for z in 4..=12 {
            journal.record([x, 3, z], 3).unwrap();
        }
    }
    let habitat = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    horse.position = [8.68, 3.1, 8.0];
    assert!(clear(&horse, &habitat));
    assert!(support::swimming(&horse, &habitat));
    let mut swimmer = horse.clone();
    let destination = [10.0, 3.1, 8.0];
    let dt = 1.0 / 60.0;
    let speed = swimmer.kind.speed();
    support::swim_toward(&mut swimmer, &habitat, destination, dt, speed);
    assert!((swimmer.position[1] - 4.0).abs() < 0.001);
    assert!(
        (swimmer.position[1] - horse.position[1]) / f64::from(dt) > 32.0,
        "fixture must exercise the original invalid base-step velocity"
    );
    assert_eq!(swimmer.velocity[1], 0.0);
    assert!(swimmer.valid());
    let mut stamina = f32::from(horse.horse_traits().unwrap().stamina);
    let input = RideInput {
        direction: [1.0, 0.0],
        ..Default::default()
    };
    let start = horse.position;
    assert!(
        !step(&mut horse, &mut stamina, &habitat, input, dt),
        "a stationary swimmer must accelerate before reaching the bank"
    );
    assert!(horse.position[0] > start[0]);
    assert!(horse.position[0] - start[0] <= 6.0 * f64::from(dt).powi(2) + 0.000001);
    assert!(horse.position[1] < 4.0);
    assert!(horse.valid() && clear(&horse, &habitat));
    let mut bank_rise = None;
    for _ in 0..120 {
        let before = horse.position;
        let ashore = step(&mut horse, &mut stamina, &habitat, input, dt);
        assert!(horse.valid(), "invalid mounted swim state: {horse:?}");
        assert!(
            clear(&horse, &habitat),
            "mounted shore collision: {horse:?}"
        );
        if ashore {
            bank_rise = Some((horse.position[1] - before[1]) / f64::from(dt));
            break;
        }
    }
    assert!(
        bank_rise.is_some_and(|rise| rise > 32.0),
        "the accelerating mount must actually step onto this bank: {horse:?}"
    );
    assert!((horse.position[1] - 4.0).abs() < 0.001);
    assert_eq!(horse.velocity[1], 0.0);
    assert!(horse.valid());
    let mut state = terra_core::GameplayState::default();
    state.creatures.explored.insert(horse.home);
    state.creatures.animals.insert(horse.id, horse);
    assert!(state.valid());
}
