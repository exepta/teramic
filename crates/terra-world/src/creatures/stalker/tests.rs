use super::*;
use crate::{
    TerraGenerator,
    edits::EditJournal,
    streaming::{ChunkStore, LoadedSection},
};
use terra_core::{
    WorldConfig,
    creatures::{
        spawning::{SpawnMob, next_command_id},
        stalker_id,
    },
};

fn fixture() -> (TerraGenerator, ChunkStore, EditJournal) {
    let config = WorldConfig {
        caves: false,
        trees: false,
        biomes: false,
        water: false,
        base_height: 8.0,
        hill_amplitude: 0.0,
        mountain_amplitude: 0.0,
        ..Default::default()
    };
    let terrain = TerraGenerator::new(config).unwrap();
    let mut chunks = ChunkStore::default();
    for x in -2..=0 {
        for z in -2..=2 {
            let section = terra_voxel::SectionPos::new(x, 0, z);
            let mut data = terra_voxel::BlockSection::empty(section).unwrap();
            for x in 0..16 {
                for z in 0..16 {
                    for y in [0, 5, 6, 7] {
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
                    props: Vec::new(),
                },
            );
        }
    }
    (terrain, chunks, EditJournal::default())
}
fn animal() -> Creature {
    Creature {
        id: stalker_id([-1, -1]).unwrap(),
        kind: Kind::Stalker,
        home: [-1, -1],
        position: [-16.5, 1.0, -16.5],
        velocity: [0.0; 3],
        yaw: 0.0,
        health: 45.0,
        behavior: Behavior::Rest,
        timer: 0.0,
        age: 0.0,
        variant: 0,
        needs: CreatureNeeds::full(Kind::Stalker),
    }
}
fn player() -> Player {
    Player {
        id: PlayerUuid::from_u128(5),
        position: [-16.5, 1.9, -10.5],
        look: [std::f32::consts::PI, 0.0],
        held_light: false,
        targetable: true,
    }
}

#[test]
fn invalid_observer_pose_releases_only_its_target_and_revalidates_the_next_live_observer() {
    let (terrain, chunks, journal) = fixture();
    let habitat = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    let mut a = animal();
    let p = player();
    let mut minds = Stalkers::default();
    let mut nav = navigation::Navigation::default();
    minds.step(&mut a, &habitat, &[p], 0.1, true, &mut nav);
    assert_eq!(minds.minds[&a.id].target, Some(p.id));
    let mut invalid = p;
    invalid.position[0] = f64::NAN;
    let result = minds.step(&mut a, &habitat, &[invalid], 0.1, false, &mut nav);
    assert!(result.pull.is_none() && result.strike.is_none());
    assert_eq!(minds.minds[&a.id].target, None);
    assert_eq!(a.velocity, [0.0; 3]);
    assert!(a.valid());
    let snapshot = a.clone();
    for dt in [0.0, f32::NAN, f32::INFINITY] {
        let result = minds.step(&mut a, &habitat, &[p], dt, true, &mut nav);
        assert!(result.pull.is_none() && result.strike.is_none());
        assert_eq!(a, snapshot);
    }
    for _ in 0..12 {
        minds.step(&mut a, &habitat, &[p], 0.1, true, &mut nav);
    }
    assert_eq!(minds.minds[&a.id].target, Some(p.id));
    assert!(a.valid());
    invalid = p;
    invalid.look[1] = f32::NAN;
    minds.step(&mut a, &habitat, &[invalid], 0.1, false, &mut nav);
    assert_eq!(minds.minds[&a.id].target, None);
    assert!(a.valid());
}

#[test]
fn stalker_ground_step_at_sixty_hz_is_support_motion_not_an_invalid_vertical_impulse() {
    let (terrain, chunks, mut journal) = fixture();
    for z in -18..=-15 {
        journal.record([-16, 1, z], 3).unwrap();
    }
    let habitat = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    let mut a = animal();
    a.position[0] = -16.54;
    assert!(habitat.clear_swimmer_rotated(a.position, a.size(), 0.0));
    let target = [a.position[0] + 0.02, a.position[1], a.position[2]];
    assert!(move_to(&mut a, &habitat, target, 1.0 / 60.0));
    assert_eq!(a.position[1], 2.0);
    assert_eq!(a.velocity[1], 0.0);
    assert!(
        a.valid(),
        "a support-height change must never poison the next AI tick"
    );
    let mut minds = Stalkers::default();
    minds.step(
        &mut a,
        &habitat,
        &[player()],
        1.0 / 60.0,
        true,
        &mut navigation::Navigation::default(),
    );
    assert!(a.valid());
    assert!(minds.minds.contains_key(&a.id));
}

#[test]
fn cave_only_natural_stalker_spawn_does_not_require_or_claim_surface_habitat_history() {
    let (_, chunks, journal) = fixture();
    let home = [-1, 0];
    let id = stalker_id(home).unwrap();
    let seed = (0..128)
        .find(|seed| random(id, *seed).is_multiple_of(11))
        .unwrap();
    let terrain = TerraGenerator::new(WorldConfig {
        seed: terra_core::WorldSeed(seed),
        base_height: 80.0,
        hill_amplitude: 0.0,
        mountain_amplitude: 0.0,
        caves: false,
        trees: false,
        biomes: false,
        water: false,
        ..Default::default()
    })
    .unwrap();
    let habitat = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    let mut population = CreaturePopulation::default();
    assert!(!super::super::spawn_cell(&mut population, &habitat, home));
    assert!(
        !population.explored.contains(&home),
        "surface sections are not loaded"
    );
    assert_eq!(spawn(&mut population, &habitat, home, 1.0, 1), 1);
    assert!(population.animals[&id].valid());
    assert!(
        !population.explored.contains(&home),
        "cave spawn must not suppress later surface herds"
    );
    assert!(population.valid());
    let state = terra_core::GameplayState {
        creatures: population,
        ..Default::default()
    };
    assert!(
        state.valid(),
        "natural cave spawn must never poison every subsequent autosave"
    );
}
#[test]
fn cave_sensing_is_negative_coordinate_safe_bounded_and_cannot_cross_a_sealed_wall_or_daylight() {
    let (terrain, chunks, mut journal) = fixture();
    let a = animal();
    let target = [a.position[0], 1.0, a.position[2] + 43.0];
    let route = navigation::caves::route(
        &Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        },
        a.position,
        target,
    )
    .unwrap();
    assert!((40.0..=50.0).contains(&route.length));
    assert!(route.expanded <= navigation::caves::NODE_LIMIT);
    assert!(
        navigation::caves::route(
            &Habitat {
                terrain: &terrain,
                chunks: &chunks,
                journal: &journal
            },
            a.position,
            [a.position[0], 1.0, a.position[2] + 51.0]
        )
        .is_none()
    );
    for x in -32..16 {
        for y in 1..=4 {
            journal.record([x, y, 0], 3).unwrap();
        }
    }
    assert!(
        navigation::caves::route(
            &Habitat {
                terrain: &terrain,
                chunks: &chunks,
                journal: &journal
            },
            a.position,
            target
        )
        .is_none()
    );
    for x in -32..16 {
        for y in 1..=4 {
            journal.record([x, y, 0], 0).unwrap();
        }
        for z in 4..32 {
            for y in 5..=7 {
                journal.record([x, y, z], 0).unwrap();
            }
        }
    }
    assert!(
        navigation::caves::route(
            &Habitat {
                terrain: &terrain,
                chunks: &chunks,
                journal: &journal
            },
            a.position,
            target
        )
        .is_none()
    );
}
#[test]
fn seen_stalker_recoils_then_chasing_player_causes_anger_and_held_torch_cancels_it() {
    let (terrain, chunks, journal) = fixture();
    let h = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    let mut a = animal();
    let mut p = player();
    p.look = [0.0, 0.0];
    let mut minds = Stalkers::default();
    let mut nav = navigation::Navigation::default();
    minds.step(&mut a, &h, &[p], 0.1, true, &mut nav);
    assert_eq!(a.behavior, Behavior::Flee);
    p.position[2] -= 0.8;
    minds.step(&mut a, &h, &[p], 0.1, false, &mut nav);
    assert_eq!(a.behavior, Behavior::Attack);
    p.held_light = true;
    let outcome = minds.step(&mut a, &h, &[p], 0.1, false, &mut nav);
    assert_eq!(a.behavior, Behavior::Flee);
    assert!(outcome.strike.is_none() && outcome.pull.is_none());
    assert!(a.valid());
}
#[test]
fn far_approach_resets_the_peek_timer_once_when_entering_fifteen_meters() {
    let (terrain, chunks, journal) = fixture();
    let h = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    let mut a = animal();
    let mut p = player();
    p.position[2] = a.position[2] + 20.0;
    let mut minds = Stalkers::default();
    let mut nav = navigation::Navigation::default();
    let mut approached_seventeen = false;
    let mut entered = false;
    for _ in 0..100 {
        minds.step(&mut a, &h, &[p], 0.1, true, &mut nav);
        let mind = &minds.minds[&a.id];
        if mind.last_distance <= 17.0 && mind.last_distance > 15.0 {
            assert!(mind.ignored > 3.0);
            approached_seventeen = true;
        }
        if mind.last_distance <= 15.0 {
            assert!(approached_seventeen);
            assert!(mind.ignored < 0.2);
            assert_eq!(a.behavior, Behavior::Hunt);
            assert_eq!(a.velocity, [0.0; 3]);
            entered = true;
            break;
        }
    }
    assert!(entered, "the real cave route must reach the peek radius");
    let peeking_position = a.position;
    for _ in 0..20 {
        minds.step(&mut a, &h, &[p], 0.1, true, &mut nav);
        assert_eq!(a.position, peeking_position);
        assert_eq!(a.velocity, [0.0; 3]);
        assert_eq!(a.behavior, Behavior::Hunt);
    }
    assert!(minds.minds[&a.id].ignored > 2.0);
    for _ in 0..15 {
        minds.step(&mut a, &h, &[p], 0.1, true, &mut nav);
    }
    assert!(distance(a.position, peeking_position) > 0.01);
    assert!(minds.minds[&a.id].ignored > 3.0);
}
#[test]
fn occluded_l_shaped_cave_uses_a_real_supported_corner_pause_and_reveal() {
    let (terrain, mut chunks, journal) = fixture();
    for (section, loaded) in &mut chunks.loaded {
        for x in 0..16 {
            for z in 0..16 {
                let gx = section.x * 16 + x as i64;
                let gz = section.z * 16 + z as i64;
                let corridor = ((-18..=-14).contains(&gx) && (-25..=-9).contains(&gz))
                    || ((-18..=-3).contains(&gx) && (-13..=-9).contains(&gz));
                if !corridor {
                    for y in 1..=4 {
                        loaded.data.set_block([x, y, z], 3).unwrap();
                    }
                }
            }
        }
    }
    let h = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    let mut a = animal();
    a.position = [-16.5, 1.0, -19.5];
    let mut p = player();
    p.position = [-5.5, 1.9, -10.5];
    p.look = [-std::f32::consts::FRAC_PI_2, 0.0];
    let player_eye = [p.position[0], p.position[1] + 0.72, p.position[2]];
    assert!(!super::super::bears::visible(
        &h,
        eye(a.position),
        player_eye
    ));
    let route = navigation::caves::route(&h, a.position, [-5.5, 1.0, -10.5])
        .expect("the two corridors must connect around their real rock corner");
    assert!(route.length < 30.0);
    let mut minds = Stalkers::default();
    let mut nav = navigation::Navigation::default();
    let mut stopped_at_corner = false;
    let mut stepped_around_corner = false;
    let mut inspected_visible_point = false;
    let mut transitions = Vec::new();
    for _ in 0..300 {
        let before = a.position;
        let phase = minds
            .minds
            .get(&a.id)
            .and_then(|m| m.corner.as_ref())
            .map(|c| c.phase);
        minds.step(&mut a, &h, &[p], 0.1, true, &mut nav);
        assert!(h.clear(a.position, a.size(), false));
        assert!(h.ground(a.position, Kind::Stalker).is_some());
        assert!(navigation::caves::covered(&h, a.position, a.size()[1]));
        let after = minds.minds[&a.id].corner.as_ref().map(|c| c.phase);
        if phase != after {
            transitions.push((
                a.position,
                after,
                minds.minds[&a.id].corner.as_ref().and_then(|c| c.reveal),
            ));
        }
        if let Some(corner) = minds.minds[&a.id].corner.as_ref() {
            if corner.phase == PeekPhase::Pause {
                stopped_at_corner = true;
                assert!(!super::super::bears::visible(
                    &h,
                    eye(corner.anchor),
                    player_eye
                ));
                assert_eq!(a.velocity, [0.0; 3]);
            }
            if phase == Some(PeekPhase::Reveal) && distance(before, a.position) > 0.0001 {
                stepped_around_corner = true;
            }
            if corner.phase == PeekPhase::Observe {
                assert!(super::super::bears::visible(
                    &h,
                    eye(a.position),
                    player_eye
                ));
                inspected_visible_point = true;
                break;
            }
        }
    }
    assert!(
        stopped_at_corner,
        "must pause at the actual occluded route turn"
    );
    assert!(
        stepped_around_corner,
        "the peek must move on the existing physical route: {transitions:?}"
    );
    assert!(
        inspected_visible_point,
        "must reach a genuinely visible point around the corner: {transitions:?}"
    );
}
#[test]
fn ignored_stalker_only_grabs_with_a_real_forty_meter_route_and_releases_at_new_obstacles() {
    let (terrain, chunks, mut journal) = fixture();
    let mut a = animal();
    let mut p = player();
    p.position[2] = a.position[2] + 1.1;
    let mut minds = Stalkers::default();
    let mut nav = navigation::Navigation::default();
    let mut grab = None;
    for _ in 0..90 {
        let outcome = minds.step(
            &mut a,
            &Habitat {
                terrain: &terrain,
                chunks: &chunks,
                journal: &journal,
            },
            &[p],
            0.1,
            true,
            &mut nav,
        );
        if outcome.pull.is_some() {
            grab = outcome.pull;
            break;
        }
    }
    let grab = grab.expect("ignored close target and connected deep cave should permit a grab");
    assert_eq!(grab.player, p.id);
    assert_eq!(grab.remaining_meters, 40.0);
    assert_eq!(a.behavior, Behavior::Hold);
    let z = a.position[2].floor() as i64 + 1;
    for x in -32..16 {
        for y in 1..=4 {
            journal.record([x, y, z], 3).unwrap();
        }
    }
    let outcome = minds.step(
        &mut a,
        &Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        },
        &[p],
        0.1,
        false,
        &mut nav,
    );
    assert!(
        outcome.pull.is_none(),
        "a changed voxel must interrupt the grab, never pull through a wall"
    );
    assert_eq!(a.behavior, Behavior::Rest);
    assert_eq!(a.velocity, [0.0; 3]);
    assert!(a.timer >= 1.9);
    assert_eq!(minds.minds[&a.id].ignored, 0.0);
    assert!(minds.minds[&a.id].path.is_empty());
    // Removing the obstruction immediately is not permission to grab again
    // with the old six-second ignored timer or stale walking velocity.
    for x in -32..16 {
        for y in 1..=4 {
            journal.record([x, y, z], 0).unwrap();
        }
    }
    for _ in 0..10 {
        let outcome = minds.step(
            &mut a,
            &Habitat {
                terrain: &terrain,
                chunks: &chunks,
                journal: &journal,
            },
            &[p],
            0.1,
            true,
            &mut nav,
        );
        assert!(outcome.pull.is_none());
        assert_eq!(a.velocity, [0.0; 3]);
    }
}
#[test]
fn cave_searches_share_one_hard_node_quota_and_cache_roof_probes_across_target_and_escape_queries()
{
    let (terrain, chunks, journal) = fixture();
    let h = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    let start = animal().position;
    let mut budget = navigation::caves::SearchBudget::default();
    let began = std::time::Instant::now();
    for z in [20.5, 25.5, 30.5, 35.5] {
        let _ = navigation::caves::route_budgeted(&h, start, [start[0], 1.0, z], &mut budget);
    }
    let _ = navigation::caves::deeper_budgeted(&h, start, &mut budget);
    assert!(budget.nodes <= navigation::caves::NODE_LIMIT);
    assert!(budget.roof_probes <= navigation::caves::NODE_LIMIT * 4 + 8);
    eprintln!(
        "stalker cave batch: nodes={} roofs={} elapsed_us={}",
        budget.nodes,
        budget.roof_probes,
        began.elapsed().as_micros()
    );
}
#[test]
fn removed_ground_uses_existing_swept_gravity_and_cancels_stalking_instead_of_hovering() {
    let (terrain, chunks, mut journal) = fixture();
    let mut a = animal();
    for x in -19..=-15 {
        for z in -19..=-15 {
            journal.record([x, 0, z], 0).unwrap();
        }
    }
    let before = a.position[1];
    let mut minds = Stalkers::default();
    let mut nav = navigation::Navigation::default();
    let outcome = minds.step(
        &mut a,
        &Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        },
        &[player()],
        0.1,
        false,
        &mut nav,
    );
    assert!(a.position[1] < before && a.velocity[1] < 0.0);
    assert!(outcome.pull.is_none() && outcome.strike.is_none());
}
#[test]
fn bounded_stalker_cache_evicts_inactive_lru_entries_with_stable_id_ties() {
    let (terrain, chunks, journal) = fixture();
    let h = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    let mut minds = Stalkers::default();
    let mut nav = navigation::Navigation::default();
    let mut previous = Vec::new();
    for x in 0..MAX_MINDS {
        let mut a = animal();
        a.home = [x as i64, -1];
        a.id = stalker_id(a.home).unwrap();
        minds.step(&mut a, &h, &[], 0.1, false, &mut nav);
        previous.push(a);
    }
    assert_eq!(minds.minds.len(), MAX_MINDS);
    minds.step(&mut previous[0], &h, &[], 0.1, false, &mut nav);
    let mut active = animal();
    active.home = [1000, -1];
    active.id = stalker_id(active.home).unwrap();
    minds.step(&mut active, &h, &[player()], 0.1, true, &mut nav);
    assert_eq!(minds.minds.len(), MAX_MINDS);
    assert!(minds.minds.contains_key(&previous[0].id));
    assert!(!minds.minds.contains_key(&previous[1].id));
    assert_eq!(minds.minds[&active.id].target, Some(player().id));

    for mind in minds.minds.values_mut() {
        mind.touched = 7;
    }
    let oldest_id = *minds.minds.keys().min().unwrap();
    active.home = [1001, -1];
    active.id = stalker_id(active.home).unwrap();
    minds.step(&mut active, &h, &[], 0.1, false, &mut nav);
    assert_eq!(minds.minds.len(), MAX_MINDS);
    assert!(!minds.minds.contains_key(&oldest_id));
    assert!(minds.minds.contains_key(&active.id));
}
#[test]
fn pull_distance_budget_counts_actual_player_motion_and_clamps_last_destination() {
    let (terrain, chunks, journal) = fixture();
    let h = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    let mut a = animal();
    let mut p = player();
    p.position[2] = a.position[2] + 1.1;
    let mut minds = Stalkers::default();
    let mut nav = navigation::Navigation::default();
    let mut grabbed = false;
    for _ in 0..90 {
        if minds
            .step(&mut a, &h, &[p], 0.1, true, &mut nav)
            .pull
            .is_some()
        {
            grabbed = true;
            break;
        }
    }
    assert!(grabbed);
    minds.minds.get_mut(&a.id).unwrap().remaining = 0.15;
    p.position[2] += 0.10;
    let result = minds
        .step(&mut a, &h, &[p], 0.1, false, &mut nav)
        .pull
        .unwrap();
    assert!((result.remaining_meters - 0.05).abs() < 0.001);
    assert!(distance(result.destination, p.position).sqrt() <= 0.051);
}
#[test]
fn placed_light_uses_loaded_emitters_and_real_occlusion_not_a_global_light_toggle() {
    let (terrain, chunks, mut journal) = fixture();
    let a = animal();
    let lamp = [-16, 1, -12];
    journal.record(lamp, 40).unwrap();
    journal
        .set_object_state(lamp, Some(Default::default()))
        .unwrap();
    assert!(near_light(
        &Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal
        },
        a.position
    ));
    for x in -32..16 {
        for y in 1..=4 {
            journal.record([x, y, -15], 3).unwrap();
        }
    }
    assert!(!near_light(
        &Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal
        },
        a.position
    ));
}
#[test]
fn a_nearby_rescuers_held_torch_deters_even_when_the_unlit_victim_remains_the_target() {
    let (terrain, chunks, mut journal) = fixture();
    let mut victim = player();
    victim.id = PlayerUuid::from_u128(5);
    let rescuer = Player {
        id: PlayerUuid::from_u128(6),
        position: [-8.5, 1.9, -16.5],
        held_light: true,
        ..victim
    };
    let mut a = animal();
    let mut minds = Stalkers::default();
    minds.step(
        &mut a,
        &Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        },
        &[victim, rescuer],
        0.1,
        true,
        &mut navigation::Navigation::default(),
    );
    assert_eq!(minds.minds[&a.id].target, Some(victim.id));
    assert_eq!(a.behavior, Behavior::Flee);

    for z in -32..48 {
        for y in 1..=4 {
            journal.record([-13, y, z], 3).unwrap();
        }
    }
    let mut a = animal();
    let mut minds = Stalkers::default();
    minds.step(
        &mut a,
        &Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        },
        &[victim, rescuer],
        0.1,
        true,
        &mut navigation::Navigation::default(),
    );
    assert_eq!(minds.minds[&a.id].target, Some(victim.id));
    assert_eq!(a.behavior, Behavior::Hunt);
    assert_eq!(minds.minds[&a.id].anger, 0.0);
}
#[test]
fn invulnerable_torch_rescuer_deters_without_becoming_a_target_or_receiving_damage_or_a_grab() {
    let (terrain, chunks, journal) = fixture();
    let h = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    let victim = player();
    let mut rescuer = Player {
        id: PlayerUuid::from_u128(6),
        position: [-13.5, 1.9, -16.5],
        held_light: true,
        targetable: false,
        ..victim
    };
    let mut a = animal();
    let mut minds = Stalkers::default();
    let mut nav = navigation::Navigation::default();
    let result = minds.step(&mut a, &h, &[victim, rescuer], 0.1, true, &mut nav);
    assert_eq!(minds.minds[&a.id].target, Some(victim.id));
    assert_eq!(a.behavior, Behavior::Flee);
    assert!(result.strike.is_none() && result.pull.is_none());

    rescuer.held_light = false;
    let result = minds.step(&mut a, &h, &[rescuer], 0.1, false, &mut nav);
    assert!(result.strike.is_none() && result.pull.is_none());
    assert!(minds.minds[&a.id].target.is_none());
    for _ in 0..90 {
        let result = minds.step(&mut a, &h, &[rescuer], 0.1, true, &mut nav);
        assert!(result.strike.is_none() && result.pull.is_none());
        assert!(minds.minds[&a.id].target.is_none());
        assert_eq!(a.behavior, Behavior::Rest);
    }
}
#[test]
fn an_actual_grab_releases_immediately_when_the_victim_becomes_invulnerable() {
    let (terrain, chunks, journal) = fixture();
    let h = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    let mut a = animal();
    let mut p = player();
    p.position[2] = a.position[2] + 1.1;
    let mut minds = Stalkers::default();
    let mut nav = navigation::Navigation::default();
    let mut grabbed = false;
    for _ in 0..90 {
        if minds
            .step(&mut a, &h, &[p], 0.1, true, &mut nav)
            .pull
            .is_some()
        {
            grabbed = true;
            break;
        }
    }
    assert!(
        grabbed,
        "the vulnerable victim must first enter a real grab"
    );
    p.targetable = false;
    let result = minds.step(&mut a, &h, &[p], 0.1, false, &mut nav);
    assert!(result.pull.is_none() && result.strike.is_none());
    assert!(minds.minds[&a.id].target.is_none());
    assert_eq!(minds.minds[&a.id].remaining, 0.0);
    assert_eq!(a.behavior, Behavior::Rest);
}
#[test]
fn naturally_generated_cave_mushroom_light_respects_real_occlusion_and_edited_removal() {
    let (terrain, mut chunks, mut journal) = fixture();
    let source = [-15, 1, -15];
    let (owner, local) = terra_voxel::global_to_section(source);
    chunks
        .loaded
        .get_mut(&owner)
        .unwrap()
        .data
        .set_block(local, 56)
        .unwrap();
    assert!(journal.object_state(source).is_none());
    let position = animal().position;
    assert!(near_light(
        &Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal
        },
        position
    ));
    journal.record(source, 0).unwrap();
    assert!(!near_light(
        &Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal
        },
        position
    ));
    journal.record(source, 56).unwrap();
    for z in -32..48 {
        for y in 1..=4 {
            journal.record([-16, y, z], 3).unwrap();
        }
    }
    assert!(!near_light(
        &Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal
        },
        position
    ));
}
#[test]
fn burning_furnace_front_deters_in_all_facings_but_its_own_shell_and_real_walls_are_distinct() {
    for facing in 0..4 {
        let (terrain, chunks, mut journal) = fixture();
        let furnace = [-16, 1, -12];
        let upper = [-16, 2, -12];
        journal.record(furnace, 59).unwrap();
        journal.record(upper, 59).unwrap();
        journal
            .set_object_state(
                furnace,
                Some(crate::edits::BlockObjectState {
                    facing,
                    peer: Some(upper),
                    ..Default::default()
                }),
            )
            .unwrap();
        journal
            .set_object_state(
                upper,
                Some(crate::edits::BlockObjectState {
                    facing,
                    peer: Some(furnace),
                    secondary: true,
                    ..Default::default()
                }),
            )
            .unwrap();
        let yaw = f64::from(facing) * std::f64::consts::FRAC_PI_2;
        let front = [-yaw.sin(), -yaw.cos()];
        let position = [-15.5 + front[0] * 3.0, 1.0, -11.5 + front[1] * 3.0];
        assert!(!near_light(
            &Habitat {
                terrain: &terrain,
                chunks: &chunks,
                journal: &journal
            },
            position
        ));
        journal
            .set_furnace(
                furnace,
                terra_core::furnace::FurnaceState {
                    burn_remaining: 5.0,
                    burn_total: 5.0,
                    ..Default::default()
                },
            )
            .unwrap();
        let h = Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        };
        assert!(
            near_light(&h, position),
            "lit furnace facing {facing} must not occlude its own front flame"
        );
        let mut a = animal();
        a.position = position;
        let mut p = player();
        p.position = [position[0], 1.9, position[2] + 6.0];
        let mut minds = Stalkers::default();
        minds.step(
            &mut a,
            &h,
            &[p],
            0.1,
            true,
            &mut navigation::Navigation::default(),
        );
        assert_eq!(a.behavior, Behavior::Flee);

        if front[0].abs() > 0.5 {
            let x = (-15.5 + front[0] * 1.5).floor() as i64;
            for z in -32..48 {
                for y in 1..=4 {
                    journal.record([x, y, z], 3).unwrap();
                }
            }
        } else {
            let z = (-11.5 + front[1] * 1.5).floor() as i64;
            for x in -32..16 {
                for y in 1..=4 {
                    journal.record([x, y, z], 3).unwrap();
                }
            }
        }
        assert!(
            !near_light(
                &Habitat {
                    terrain: &terrain,
                    chunks: &chunks,
                    journal: &journal
                },
                position
            ),
            "a real intervening wall must still block the furnace light"
        );
    }
}
#[test]
fn administrator_stalker_spawn_remains_cave_only_and_preserves_appended_identity_namespaces() {
    let (terrain, chunks, mut journal) = fixture();
    let population = CreaturePopulation::default();
    let mob = SpawnMob {
        kind: Kind::Stalker,
        variant: 0,
    };
    let a = super::super::spawning::near(
        &population,
        &Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        },
        mob,
        [-16.5, 1.9, -16.5],
        0.0,
    )
    .unwrap();
    assert_eq!(Some(a.id), next_command_id(&population));
    assert!(a.valid());
    assert_ne!(
        stalker_id([-1, -1]),
        terra_core::creatures::spider_id([-1, -1], 0)
    );
    for x in -32..16 {
        for z in -32..48 {
            for y in 5..=7 {
                journal.record([x, y, z], 0).unwrap();
            }
        }
    }
    assert!(
        super::super::spawning::near(
            &population,
            &Habitat {
                terrain: &terrain,
                chunks: &chunks,
                journal: &journal
            },
            mob,
            [-16.5, 1.9, -16.5],
            0.0
        )
        .is_err()
    );
}
