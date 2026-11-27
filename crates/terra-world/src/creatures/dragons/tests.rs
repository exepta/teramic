use super::*;
use crate::creatures::perception::tests::fixture;
use terra_core::creatures::{CreatureNeeds, creature_id};

fn adult(kind: K) -> Creature {
    Creature {
        id: creature_id([0, 0], 0).unwrap(),
        kind,
        home: [0, 0],
        position: [8.5, 1.01, 12.5],
        velocity: [0.0; 3],
        yaw: 0.0,
        health: kind.health(),
        behavior: B::Rest,
        timer: 0.0,
        age: 1000.0,
        variant: 0,
        needs: CreatureNeeds::full(kind),
    }
}

#[test]
fn hatchling_drinks_at_the_bank_sleeps_in_cover_and_only_launches_after_three_months() {
    use terra_core::creatures::{breeding, dragon_life::DragonLife};
    use terra_voxel::MaterialId;
    let (dry, chunks, mut journal) = fixture();
    let terrain = crate::TerraGenerator::new(terra_core::WorldConfig {
        water: true,
        sea_level: -20.,
        ..dry.config().clone()
    })
    .unwrap();
    let mut a = adult(K::FireDragon);
    a.id = breeding::next_birth_id(&Default::default(), 0).unwrap();
    a.age = 0.;
    let nest = [8, 1, 12];
    a.needs.dragon = Some(DragonLife {
        nest: Some(nest),
        ..Default::default()
    });
    a.needs.water = 3.;
    a.needs.search_in = 0.;
    journal
        .record([8, 0, 12], MaterialId::StrawBlock as u16)
        .unwrap();
    journal
        .set_fluid(
            [10, 1, 12],
            crate::water::FluidCell {
                amount: 8,
                flow: [0; 2],
                source: true,
            },
        )
        .unwrap();
    let h = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    let mut encounter = Encounter::new(&a, None);
    encounter.set_keeper(Some([8., 1., 10.]), false);
    encounter.step(&mut a, &h, 0.25, &[], false, true);
    assert_eq!(a.behavior, B::Drink);
    assert!(a.needs.water > 3.);
    assert!(a.needs.dragon.unwrap().affection > 0.);
    assert!(a.needs.dragon.unwrap().sheltered);
    a.needs.water = a.kind.water();
    a.behavior = B::Rest;
    encounter.step(&mut a, &h, 0.25, &[], true, true);
    assert_eq!(a.behavior, B::Sleep);
    assert!(!flight::can_launch(&a, &h));
    a.needs.dragon.as_mut().unwrap().months = 3.;
    assert!(flight::can_launch(&a, &h));
}

#[test]
fn encounter_roars_at_a_visible_intruder_and_drops_aggro_behind_cover() {
    let (terrain, chunks, mut journal) = fixture();
    let mut a = adult(K::DarkDragon);
    let mut encounter = Encounter::new(&a, None);
    let id = Target::Player(PlayerUuid::from_u128(1));
    let players = [(id, [8.5, 2.1, -10.5])];
    {
        let h = Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        };
        encounter.step(&mut a, &h, 0.1, &players, false, false);
        assert_eq!(encounter.target, None, "daytime resting radius");
        encounter.step(&mut a, &h, 0.1, &players, true, true);
        assert_eq!(encounter.target, None, "owned dragons are friendly");
        encounter.step(&mut a, &h, 0.1, &players, true, false);
        assert_eq!(encounter.target, Some(id));
        assert_eq!(a.behavior, B::Emerge);
    }
    for y in 1..16 {
        journal.record([8, y, 0], 3).unwrap();
    }
    let h = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    for _ in 0..30 {
        assert!(
            encounter
                .step(&mut a, &h, 0.1, &players, true, false)
                .is_empty()
        );
    }
    assert_eq!(encounter.target, None);
}

#[test]
fn breath_leads_a_running_target_then_keeps_its_warned_impact_point() {
    let (terrain, chunks, journal) = fixture();
    let h = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    let mut a = adult(K::WaterDragon);
    let mut encounter = Encounter::new(&a, None);
    let id = Target::Player(PlayerUuid::from_u128(1));
    let mut point = [5.5, 2.1, -8.5];
    encounter.step(&mut a, &h, 0.1, &[(id, point)], true, false);
    assert_eq!(a.behavior, B::Emerge);
    for _ in 0..40 {
        point[0] += 0.32;
        encounter.step(&mut a, &h, 0.1, &[(id, point)], true, false);
        if a.behavior == B::Spit {
            break;
        }
    }
    assert_eq!(a.behavior, B::Spit);
    let warned = a.needs.goal.unwrap();
    assert!(warned.position[0] > point[0].floor() as i64);
    point[0] -= 6.;
    encounter.step(&mut a, &h, 0.1, &[(id, point)], true, false);
    assert_eq!(a.needs.goal, Some(warned));
}

#[test]
fn water_vortex_pulls_bystanders_towards_locked_aim_but_spares_players_who_dodge() {
    let (terrain, chunks, journal) = fixture();
    let h = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    let mut a = adult(K::WaterDragon);
    let owner = Target::Player(PlayerUuid::from_u128(1));
    let bystander = Target::Player(PlayerUuid::from_u128(2));
    let mut encounter = Encounter::new(&a, None);
    encounter.target = Some(owner);
    a.behavior = B::Ink;
    a.timer = pose::duration(B::Ink) * 0.45 + 0.05;
    a.needs.goal = Some(NeedGoal {
        kind: NeedKind::AttackAim,
        position: [8, 2, 1],
    });
    let players = [(owner, [18.5, 2.0, 1.5]), (bystander, [11.5, 2.0, 1.5])];
    let hit = encounter.step(&mut a, &h, 0.1, &players, true, false);
    assert_eq!(hit.len(), 1);
    assert_eq!(hit[0].target, bystander);
    assert_eq!(hit[0].effect, Some(terra_core::EffectKind::Slow));
    assert!(hit[0].impulse[0] < 0.0 && hit[0].damage > 0.0);
    assert!(
        encounter
            .step(&mut a, &h, 0.1, &players, true, false)
            .is_empty()
    );
}

#[test]
fn each_species_has_distinct_combat_moves_and_breath_uses_its_animated_mouth() {
    let (terrain, chunks, journal) = fixture();
    let h = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    for kind in [K::DarkDragon, K::EarthDragon, K::WaterDragon] {
        let mut a = adult(kind);
        let mut encounter = Encounter::new(&a, None);
        assert_eq!(encounter.choose(kind, 24.0), B::Spit);
        encounter.attack_number = 2;
        assert_eq!(
            encounter.choose(kind, 8.0),
            if kind == K::EarthDragon {
                B::Attack
            } else {
                B::Ink
            }
        );
        encounter.attack_number = 1;
        assert_eq!(encounter.choose(kind, 8.0), B::Tremor);
        a.behavior = B::Spit;
        a.timer = pose::duration(B::Spit) * 0.45;
        a.needs.goal = Some(NeedGoal {
            kind: NeedKind::AttackAim,
            position: [8, 2, -9],
        });
        let id = Target::Player(PlayerUuid::from_u128(3));
        let hit = encounter.strikes(
            &a,
            &h,
            &[
                (id, [8.5, 2.5, -8.5]),
                (Target::Player(PlayerUuid::nil()), [15.5, 2.5, -8.5]),
            ],
        );
        assert_eq!(hit.len(), 1, "{kind:?} muzzle {:?}", pose::muzzle(&a));
        assert_eq!(hit[0].target, id);
    }
}

#[test]
fn flying_guardians_use_the_walkable_entrance_fly_out_and_return_to_their_nest() {
    use crate::generation::sections::SectionGenerator;
    use crate::streaming::{ChunkStore, LoadedSection};
    use crate::{TerraGenerator, edits::EditJournal};
    use terra_voxel::{SectionPos, global_to_section};

    let terrain = TerraGenerator::new(terra_core::WorldConfig {
        ecology_revision: 19,
        ..Default::default()
    })
    .unwrap();
    let sections = SectionGenerator::new(terrain.clone());
    for (region, variant) in [[-1, -5], [-6, 18]]
        .into_iter()
        .flat_map(|region| [(region, 0), (region, 3)])
    {
        let site = terrain.structure_site(region).unwrap();
        let kind = site.kind.dragon().unwrap();
        let center = site.center;
        let [fx, fz] = site.dragon_entrance().unwrap();
        let at = |u, dy, v| {
            [
                center[0] + u * fz + v * fx,
                center[1] + dy,
                center[2] - u * fx + v * fz,
            ]
        };
        let corners = [
            at(-26, -14, -8),
            at(26, 48, 92),
            at(26, 48, -8),
            at(-26, 48, 92),
        ];
        let lo = global_to_section(std::array::from_fn(|i| {
            corners.iter().map(|p| p[i]).min().unwrap()
        }))
        .0;
        let hi = global_to_section(std::array::from_fn(|i| {
            corners.iter().map(|p| p[i]).max().unwrap()
        }))
        .0;
        let mut chunks = ChunkStore::default();
        let journal = EditJournal::default();
        for x in lo.x..=hi.x {
            for z in lo.z..=hi.z {
                let column = sections.prepare_column([x, z], true).unwrap();
                for y in lo.y..=hi.y {
                    let pos = SectionPos::new(x, y, z);
                    let part = sections
                        .build_prepared(pos, 0, &Default::default(), &column)
                        .unwrap();
                    chunks.loaded.insert(
                        pos,
                        LoadedSection {
                            data: part.data,
                            mesh: None,
                            mesh_epoch: 0,
                            water_epoch: 0,
                            water: part.water,
                            props: vec![],
                        },
                    );
                }
            }
        }
        let h = Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        };
        let mut a = adult(kind);
        a.variant = variant;
        a.position = [
            center[0] as f64 + 0.5,
            center[1] as f64,
            center[2] as f64 + 0.5,
        ];
        let mut player = a.position;
        for v in 1..=65 {
            let point = at(0, 0, v);
            player = h
                .ground_sized(
                    [point[0] as f64 + 0.5, player[1], point[2] as f64 + 0.5],
                    [0.6, 1.8, 0.6],
                    0.,
                )
                .unwrap_or_else(|| panic!("{kind:?} player path blocked at {v} {player:?}"));
        }
        let mut encounter = Encounter::new(&a, Some((center, [fx, fz])));
        let mut launch = false;
        let mut fly = false;
        let mut land = false;
        let mut returned = false;
        let mut furthest = 0f64;
        let mut max_clearance = 0f64;
        let mut prepared = std::collections::BTreeMap::new();
        let mut previous = None;
        for _ in 0..1850 {
            // Follow the same moving 5x4x5 collision neighborhood as server
            // streaming. Exercise the real trees and cave roofs along the sortie.
            let focus = global_to_section(a.position.map(|v| v.floor() as i64)).0;
            if previous != Some(focus) {
                previous = Some(focus);
                for z in focus.z - 2..=focus.z + 2 {
                    for x in focus.x - 2..=focus.x + 2 {
                        for y in focus.y - 1..=focus.y + 2 {
                            let pos = SectionPos::new(x, y, z);
                            if chunks.loaded.contains_key(&pos) {
                                continue;
                            }
                            let column = prepared
                                .entry([x, z])
                                .or_insert_with(|| sections.prepare_column([x, z], true).unwrap());
                            let part = sections
                                .build_prepared(pos, 0, &Default::default(), column)
                                .unwrap();
                            chunks.loaded.insert(
                                pos,
                                LoadedSection {
                                    data: part.data,
                                    mesh: None,
                                    mesh_epoch: 0,
                                    water_epoch: 0,
                                    water: part.water,
                                    props: vec![],
                                },
                            );
                        }
                    }
                }
            }
            let h = Habitat {
                terrain: &terrain,
                chunks: &chunks,
                journal: &journal,
            };
            assert!(encounter.step(&mut a, &h, 0.2, &[], true, false).is_empty());
            let offset = DVec3::from_array(a.position) - encounter.home;
            furthest = furthest.max(offset.with_y(0.).length());
            if a.behavior == B::Fly {
                let column = terrain.surface_column(a.position[0], a.position[2]);
                max_clearance = max_clearance.max(
                    a.position[1]
                        - column
                            .height
                            .max(column.water_level.unwrap_or(column.height)),
                );
            }
            launch |= a.behavior == B::Leap && furthest > 40.;
            fly |= launch && a.behavior == B::Fly && offset.y > 5.;
            land |= fly && a.behavior == B::Land;
            if land && offset.length() < 3.0 && a.behavior == B::Rest {
                returned = true;
                break;
            }
        }
        eprintln!(
            "{kind:?} variant {variant}: maximum horizontal distance {furthest:.1}, clearance {max_clearance:.1} blocks, returned {returned}"
        );
        assert!(
            max_clearance >= 35.,
            "{kind:?} variant {variant} cruising only {max_clearance:.1} above terrain"
        );

        assert!(
            returned && furthest > 140. && furthest <= ROAM_RADIUS,
            "{kind:?} launch={launch} fly={fly} land={land} far={furthest:.1} end={:?} position={:?} home={:?} patrol={:?} course={:?}",
            a.behavior,
            a.position,
            center,
            encounter.patrol,
            encounter.course
        );
    }
}

#[test]
fn launch_sweeps_the_cave_roof_and_wingtips_and_rejects_leaf_landings() {
    use crate::streaming::LoadedSection;
    use terra_voxel::{BlockSection, MaterialId, SectionPos, global_to_section};
    let (terrain, mut chunks, journal) = fixture();
    for x in -2..=2 {
        for z in -2..=2 {
            for y in 0..=2 {
                let p = SectionPos::new(x, y, z);
                chunks.loaded.entry(p).or_insert_with(|| LoadedSection {
                    data: BlockSection::empty(p).unwrap(),
                    mesh: None,
                    mesh_epoch: 0,
                    water_epoch: 0,
                    water: None,
                    props: Vec::new(),
                });
            }
        }
    }
    let a = adult(K::DarkDragon);
    let set = |chunks: &mut crate::streaming::ChunkStore, p, id| {
        let (sec, local) = global_to_section(p);
        chunks
            .loaded
            .get_mut(&sec)
            .unwrap()
            .data
            .set_block(local, id)
            .unwrap();
    };
    let clear = |chunks: &crate::streaming::ChunkStore| {
        flight::can_launch(
            &a,
            &Habitat {
                terrain: &terrain,
                chunks,
                journal: &journal,
            },
        )
    };
    assert!(clear(&chunks));
    let roof = [8, 18, 9];
    set(&mut chunks, roof, MaterialId::Stone as u16);
    assert!(!clear(&chunks));
    set(&mut chunks, roof, 0);
    // Well outside the walking box but inside the right wing's launch sweep.
    let canopy = [24, 9, 12];
    set(&mut chunks, canopy, MaterialId::BloodLeaves as u16);
    assert!(!clear(&chunks));
    set(&mut chunks, canopy, 0);
    assert!(clear(&chunks));
    // Excavating the ground below one foot must invalidate a previously
    // usable landing, even though the centre and the remaining feet are solid.
    let paw = terra_core::creatures::dragon_pose::Joint::Paw {
        side: 1,
        hind: false,
    }
    .pivot();
    let scale = f64::from(pose::model_scale(a.kind, a.variant));
    let cell = [
        (a.position[0] + scale * f64::from(paw.x)).floor() as i64,
        0,
        (a.position[2] + scale * f64::from(paw.z)).floor() as i64,
    ];
    let ground = |chunks: &crate::streaming::ChunkStore| {
        flight::clean_landing(
            &a,
            &Habitat {
                terrain: &terrain,
                chunks,
                journal: &journal,
            },
            a.position,
        )
    };
    assert!(ground(&chunks));
    set(&mut chunks, cell, 0);
    assert!(!ground(&chunks));
    set(&mut chunks, cell, MaterialId::Stone as u16);
    set(&mut chunks, [8, 0, 12], MaterialId::Leaves as u16);
    assert!(!flight::clean_landing(
        &a,
        &Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal
        },
        a.position
    ));
}

#[test]
fn flight_steers_over_canopy_on_the_approach_and_reaches_the_far_side() {
    use crate::streaming::LoadedSection;
    use terra_voxel::{BlockSection, MaterialId, SectionPos, global_to_section};
    let (terrain, mut chunks, journal) = fixture();
    for x in -4..=4 {
        for z in -6..=4 {
            for y in -1..=4 {
                let p = SectionPos::new(x, y, z);
                chunks.loaded.entry(p).or_insert_with(|| LoadedSection {
                    data: BlockSection::empty(p).unwrap(),
                    mesh: None,
                    mesh_epoch: 0,
                    water_epoch: 0,
                    water: None,
                    props: Vec::new(),
                });
            }
        }
    }
    for x in -8..=24 {
        for z in -16..=-10 {
            for y in 6..=17 {
                let (sec, local) = global_to_section([x, y, z]);
                chunks
                    .loaded
                    .get_mut(&sec)
                    .unwrap()
                    .data
                    .set_block(local, MaterialId::BloodLeaves as u16)
                    .unwrap();
            }
        }
    }
    let h = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    let mut a = adult(K::FireDragon);
    a.position = [8.5, 8., 13.5];
    a.behavior = B::Fly;
    a.yaw = 0.;
    let mut encounter = Encounter::new(&a, None);
    let start = DVec3::from_array(a.position);
    encounter.travel(&mut a, &h, start + DVec3::Z * 9., 0.1, 13.);
    assert!(
        a.position[2] <= start.z + 0.001,
        "reversal cannot slide backwards while still facing forward"
    );
    assert!(a.yaw > 0.01);
    let goal = DVec3::new(8.5, 9., -45.);
    let mut crest = 8f64;
    for _ in 0..850 {
        let from = DVec3::from_array(a.position);
        encounter.travel(&mut a, &h, goal, 0.1, 13.);
        crest = crest.max(a.position[1]);
        let to = DVec3::from_array(a.position);
        assert!(flight::passage(&h, &a, to, to, f64::from(a.yaw)));
        assert!(from.distance(to) <= 1.31);
        if to.distance(goal) < 2. {
            break;
        }
    }
    assert!(
        DVec3::from_array(a.position).distance(goal) < 2.,
        "failed to clear the canopy: {:?} crest {crest}",
        a.position
    );
}

#[test]
fn obstructed_apron_holds_in_flight_then_uses_a_new_strip_beyond_the_old_pad() {
    use crate::streaming::LoadedSection;
    use terra_voxel::{BlockSection, MaterialId, SectionPos};
    let (terrain, mut chunks, journal) = fixture();
    let mut a = adult(K::FireDragon);
    let center = DVec3::new(8.5, 1., 12.5);
    a.position = (center + DVec3::Y * 15.).to_array();
    a.behavior = B::Fly;
    a.yaw = std::f32::consts::PI;
    let mut encounter = Encounter::new(&a, None);
    for x in -4..=4 {
        for z in -4..=5 {
            for y in -1..=6 {
                let pos = SectionPos::new(x, y, z);
                chunks.loaded.insert(
                    pos,
                    LoadedSection {
                        data: BlockSection::empty(pos).unwrap(),
                        mesh: None,
                        mesh_epoch: 0,
                        water_epoch: 0,
                        water: None,
                        props: Vec::new(),
                    },
                );
            }
        }
    }
    let mut path_length = 0.;
    for index in 0..800 {
        if index == 170 {
            // New footing appears well outside the original seven pad probes.
            for x in -7..=24 {
                for z in 33..=49 {
                    let (sec, block) = terra_voxel::global_to_section([x, 0, z]);
                    chunks
                        .loaded
                        .get_mut(&sec)
                        .unwrap()
                        .data
                        .set_block(block, MaterialId::Stone as u16)
                        .unwrap();
                }
            }
        }
        let h = Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        };
        a.age += 0.2;
        let from = DVec3::from_array(a.position);
        encounter.approach_pad(&mut a, &h, center, DVec3::Z, 0.2);
        if index < 170 {
            assert_eq!(a.behavior, B::Fly);
            path_length += (DVec3::from_array(a.position) - from).with_y(0.).length();
        }
        if a.behavior == B::Land {
            assert!(index >= 170 && a.position[2] >= 33.);
            assert!(path_length > 40., "a moving hold, traveled {path_length}");
            for _ in 0..35 {
                encounter.step(&mut a, &h, 0.2, &[], true, false);
                if !encounter.airborne {
                    break;
                }
            }
            assert!(!encounter.airborne && (a.position[1] - 1.).abs() < 0.01);
            return;
        }
    }
    panic!("never found new strip, last pose {:?}", a.position);
}

#[test]
fn walking_dragon_turns_before_descending_and_requires_a_reachable_front_tread() {
    let (terrain, chunks, mut journal) = fixture();
    let mut a = adult(K::EarthDragon);
    a.yaw = std::f32::consts::PI;
    let mut encounter = Encounter::new(&a, None);
    let goal = DVec3::from_array(a.position) - DVec3::Z * 9.;
    let start = a.position;
    let habitat = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    encounter.travel(&mut a, &habitat, goal, 0.1, 4.);
    assert_eq!(a.position, start);
    // A full terrace at the hips, four steps down at the foreclaws.
    for z in 2..=17 {
        let height = ((z - 4) / 2).clamp(0, 5);
        for x in 3..=14 {
            for y in 1..=height {
                journal.record([x, y, z], 3).unwrap();
            }
        }
    }
    let p = [8.5, 6., 12.5];
    let h = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    assert!(walkable_treads(&h, p, 0., 1.));
    for x in 3..=14 {
        for z in 5..=9 {
            for y in 0..=6 {
                journal.record([x, y, z], 0).unwrap();
            }
        }
    }
    let h = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    assert!(!walkable_treads(&h, p, 0., 1.));
}

#[test]
fn greater_dragon_volleys_hit_once_and_stop_at_a_new_wall() {
    let (terrain, chunks, mut journal) = fixture();
    let target = Target::Player(PlayerUuid::from_u128(5));
    let players = [(target, [8.5, 2.5, -7.5])];
    for expected in [true, false] {
        if !expected {
            for x in 0..16 {
                for y in 1..16 {
                    journal.record([x, y, -4], 3).unwrap();
                }
            }
        }
        let h = Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        };
        for (kind, variant) in [(K::DarkDragon, 3), (K::WaterDragon, 4), (K::EarthDragon, 3)] {
            let mut a = adult(kind);
            a.variant = variant;
            a.behavior = B::DragonVolley;
            a.needs.goal = Some(NeedGoal {
                kind: NeedKind::AttackAim,
                position: [8, 2, -8],
            });
            let mut e = Encounter::new(&a, None);
            let form = pose::lineage(kind, variant).unwrap();
            let duration = pose::duration(a.behavior);
            let mut hits = vec![];
            for i in 1..=25 {
                let prev = duration * (1. - (i - 1) as f32 / 25.);
                a.timer = duration * (1. - i as f32 / 25.);
                hits.extend(e.volley(&a, &h, form, &players, prev));
            }
            assert_eq!(hits.len(), usize::from(expected), "{kind:?}");
            assert!(
                hits.iter()
                    .all(|s| s.target == target && s.damage > 0. && s.effect.is_some())
            );
        }
    }
}

#[test]
fn great_bloodlines_use_personal_defenses_a_safe_blink_and_raised_geysers() {
    let (terrain, chunks, journal) = fixture();
    let h = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    let mut a = adult(K::WaterDragon);
    a.variant = 3;
    a.health *= 0.51;
    a.behavior = B::DragonWard;
    a.timer = pose::duration(a.behavior);
    assert_eq!(pose::incoming_damage(&a, 10.), 10.);
    a.timer *= 0.45;
    assert_eq!(pose::incoming_damage(&a, 10.), 3.);
    let prior = a.health;
    lineages::resolve(&mut a, &h, pose::Lineage::Tidal, false, &[], None);
    assert!(a.health > prior && a.health <= a.kind.health());
    a.needs.goal = Some(NeedGoal {
        kind: NeedKind::AttackAim,
        position: [8, 2, 0],
    });
    a.behavior = B::DragonSurge;
    let hit = Encounter::new(&a, None).strikes(
        &a,
        &h,
        &[
            (Target::Player(PlayerUuid::from_u128(1)), [8.5, 2., 0.5]),
            (Target::Player(PlayerUuid::from_u128(2)), [14.5, 2., 0.5]),
        ],
    );
    assert_eq!(hit.len(), 1);
    assert!(hit[0].impulse[1] > 8.);
    a.kind = K::DarkDragon;
    let before = a.position;
    lineages::resolve(&mut a, &h, pose::Lineage::Shadow, false, &[], None);
    assert_ne!(a.position, before);
    assert!(h.clear_rotated(a.position, a.size(), false, f64::from(a.yaw)));
    a.needs.goal.as_mut().unwrap().position = [1000, 2, -600];
    let before = a.position;
    lineages::resolve(&mut a, &h, pose::Lineage::Shadow, false, &[], None);
    assert_eq!(a.position, before);
}

#[test]
fn changing_to_creative_releases_the_player_and_the_roost_guardian_walks_home() {
    let (terrain, chunks, journal) = fixture();
    let h = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    let mut a = adult(K::FireDragon);
    let mut encounter = Encounter::new(&a, Some(([8, 1, 5], [0, 1])));
    let target = Target::Player(PlayerUuid::from_u128(4));
    encounter.step(&mut a, &h, 0.1, &[(target, [8.5, 2., 14.5])], false, false);
    assert_eq!(encounter.target, Some(target));
    // Aim away from the nest to exercise the pivot while returning to the roost.
    a.yaw = std::f32::consts::PI;
    for _ in 0..65 {
        encounter.step(&mut a, &h, 0.1, &[], false, false);
    }
    assert_eq!(encounter.target, None);
    assert!((a.position[2] - 5.5).abs() < 2.4, "{:?}", a.position);
    assert!(matches!(a.behavior, B::Rest | B::Sleep));
}

#[test]
fn a_low_flying_dragon_must_land_before_it_can_tremor_against_a_mob() {
    let (terrain, chunks, journal) = fixture();
    let h = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    let mut a = adult(K::FireDragon);
    a.position[1] = 3.;
    a.behavior = B::Fly;
    let mut encounter = Encounter::new(&a, None);
    let target = Target::Mob(7);
    let prey = [(target, [8.5, 2., 2.5])];
    encounter.step(&mut a, &h, 0.1, &prey, false, false);
    assert_eq!(encounter.target, Some(target));
    a.timer = 0.;
    encounter.cooldown = 0.;
    encounter.attack_number = 1;
    assert_eq!(encounter.choose(a.kind, 10.), B::Tremor);
    encounter.step(&mut a, &h, 0.1, &prey, false, false);
    assert!(encounter.attack_pad.is_some());
    assert!(matches!(a.behavior, B::Fly | B::Land));
    let mut landed = false;
    for _ in 0..220 {
        encounter.step(&mut a, &h, 0.10, &prey, false, false);
        landed |= a.behavior == B::Land;
        if a.behavior == B::Tremor {
            assert!(landed && on_ground(&a, &h));
            return;
        }
    }
    panic!(
        "failed to touch down and stomp: {:?} {:?}",
        a.behavior, a.position
    );
}

#[test]
fn a_tidal_guardian_spawns_in_the_ocean_and_dives_without_leaving_its_reef() {
    use crate::streaming::{ChunkStore, LoadedSection};
    use terra_voxel::{SectionPos, global_to_section};
    let config = terra_core::WorldConfig {
        ecology_revision: 19,
        structures_revision: 16,
        ..Default::default()
    };
    let terrain = crate::TerraGenerator::new(config).unwrap();
    let site = terrain.structure_site([-6, 5]).unwrap();
    assert_eq!(site.kind.dragon(), Some(K::WaterDragon));
    let [x, y, z] = site.center;
    let home = [x.div_euclid(32), z.div_euclid(32)];
    let water = terrain
        .water_columns(x as f64 + 0.5, z as f64 + 0.5)
        .at(y)
        .unwrap();
    assert!(water.level > y as f64 + 14.);
    let sections = crate::SectionGenerator::new(terrain.clone());
    let (anchor, _) = global_to_section(site.center);
    let mut chunks = ChunkStore::default();
    for sx in anchor.x - 1..=anchor.x + 1 {
        for sz in anchor.z - 1..=anchor.z + 1 {
            let col = sections.prepare_column([sx, sz], true).unwrap();
            for sy in anchor.y - 1..=anchor.y + 1 {
                let pos = SectionPos::new(sx, sy, sz);
                let section = sections
                    .build_prepared(pos, 0, &Default::default(), &col)
                    .unwrap();
                chunks.loaded.insert(
                    pos,
                    LoadedSection {
                        data: section.data,
                        water: section.water,
                        props: vec![],
                        mesh: None,
                        mesh_epoch: 0,
                        water_epoch: 0,
                    },
                );
            }
        }
    }
    let journal = crate::edits::EditJournal::default();
    let h = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    let mut population = terra_core::creatures::CreaturePopulation::default();
    assert!(crate::creatures::spawn_cell_away_from_players(
        &mut population,
        &h,
        home,
        &[],
        1
    ));
    let a = population.animals.values_mut().next().unwrap();
    assert!(!flight::can_launch(a, &h));
    let mut enc = Encounter::new(a, Some((site.center, site.dragon_entrance().unwrap())));
    let base = DVec3::from_array(a.position);
    let goal = base + DVec3::new(2., 3., 2.);
    for _ in 0..14 {
        enc.travel(a, &h, goal, 0.15, 5.);
    }
    assert_eq!(a.behavior, B::Swim);
    let high = a.position[1];
    assert!(high > base.y + 0.9);
    for _ in 0..14 {
        enc.travel(a, &h, base, 0.15, 5.);
    }
    assert!(a.position[1] < high - 0.8 && h.wet(a.position));
    // Exercise the actual untargeted update, including entrance/patrol handling.
    let mut travelled = 0.;
    let mut excursion = 0_f64;
    for _ in 0..370 {
        let before = DVec3::from_array(a.position);
        enc.step(a, &h, 0.1, &[], false, false);
        let after = DVec3::from_array(a.position);
        travelled += after.distance(before);
        excursion = excursion.max(after.distance(base));
        assert!(h.wet(a.position) && h.clear_swimmer_rotated(a.position, a.size(), a.yaw.into()));
    }
    assert!(
        travelled > 14. && excursion > 7.,
        "reef patrol travelled {travelled}, excursion {excursion}"
    );
}

#[test]
fn tidal_guardian_swims_around_an_edited_pillar_without_crossing_dry_or_unloaded_space() {
    use crate::water::{FLUID_CAPACITY, FluidCell};
    use terra_voxel::{BlockSection, MaterialId, SectionPos};
    let (dry, mut chunks, mut journal) = fixture();
    for x in -1..=1 {
        for z in -1..=1 {
            let pos = SectionPos::new(x, 1, z);
            chunks.loaded.insert(
                pos,
                crate::streaming::LoadedSection {
                    data: BlockSection::empty(pos).unwrap(),
                    mesh: None,
                    mesh_epoch: 0,
                    water_epoch: 0,
                    water: None,
                    props: vec![],
                },
            );
        }
    }
    let terrain = crate::TerraGenerator::new(terra_core::WorldConfig {
        water: true,
        sea_level: -60.,
        ..dry.config().clone()
    })
    .unwrap();
    let water = FluidCell {
        amount: FLUID_CAPACITY,
        source: true,
        flow: [0, 0],
    };
    for x in -11..=28 {
        for z in -10..=24 {
            for y in 1..=14 {
                journal.set_fluid([x, y, z], water).unwrap();
            }
        }
    }
    // A solid wall from bed to waterline forces a lateral, not a vertical detour.
    for y in 1..=15 {
        for z in 4..=8 {
            journal.record([9, y, z], MaterialId::Stone as u16).unwrap();
        }
    }
    let h = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    let mut a = crate::creatures::spawning::near(
        &Default::default(),
        &h,
        terra_core::creatures::spawning::SpawnMob {
            kind: K::WaterDragon,
            variant: 0,
        },
        [-14.8, 3., 6.2],
        std::f32::consts::PI * 1.5,
    )
    .unwrap();
    assert_eq!(a.behavior, B::Swim);
    let mut enc = Encounter::new(&a, None);
    let submerged = DVec3::from_array(a.position);
    a.position[1] = 13.5;
    assert!(h.wet(a.position));
    assert!(!h.wet([
        a.position[0],
        a.position[1] + a.size()[1] - 0.14,
        a.position[2]
    ]));
    assert!(h.clear_swimmer_rotated(a.position, a.size(), a.yaw.into()));
    for _ in 0..35 {
        enc.travel(&mut a, &h, submerged, 0.1, 5.);
    }
    assert!(
        a.position[1] < 6.,
        "stranded at the waterline {:?}",
        a.position
    );
    let goal = DVec3::new(19., 3., 6.);
    assert!(h.clear_swimmer_rotated(a.position, a.size(), a.yaw.into()));
    for _ in 0..135 {
        enc.travel(&mut a, &h, goal, 0.1, 5.);
        assert!(h.clear_swimmer_rotated(a.position, a.size(), a.yaw.into()) && h.wet(a.position));
    }
    assert!(
        DVec3::from_array(a.position).distance(goal) < 2.,
        "stopped at {:?}",
        a.position
    );
}

#[test]
fn an_earth_dragon_dives_below_a_broad_natural_bank_and_reemerges() {
    use crate::streaming::LoadedSection;
    use terra_voxel::{BlockSection, MaterialId, SectionPos};
    let (terrain, mut chunks, journal) = fixture();
    for z in -1..=1 {
        for x in -1..=1 {
            let sec = SectionPos::new(x, -1, z);
            let mut data = BlockSection::empty(sec).unwrap();
            for nx in 0..16 {
                for nz in 0..16 {
                    data.set_block([nx, 15, nz], MaterialId::Stone as u16)
                        .unwrap();
                }
            }
            chunks.loaded.insert(
                sec,
                LoadedSection {
                    data,
                    water: None,
                    props: vec![],
                    mesh: None,
                    mesh_epoch: 0,
                    water_epoch: 0,
                },
            );
        }
    }
    let h = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    let mut a = adult(K::EarthDragon);
    let from = DVec3::from_array(a.position);
    let mut e = Encounter::new(&a, None);
    assert!(!flight::can_launch(&a, &h));
    assert!(e.begin_tunnel(&mut a, &h, from - DVec3::Z * 15.));
    let mut buried = false;
    for _ in 0..46 {
        a.timer = (a.timer - 0.1).max(0.);
        if a.behavior != B::DragonBurrow {
            break;
        }
        buried |= terra_core::creatures::dragon_pose::Pose::of(&a, 0.).submergence() > 0.95;
        e.tunnel(&mut a, &h, 0.1);
    }
    assert!(buried && a.position[2] < from.z - 9. && a.behavior == B::Hunt);
}

#[test]
fn shadow_gates_open_behind_the_victims_look_and_reject_an_unloaded_exit() {
    let (terrain, chunks, journal) = fixture();
    let h = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    let mut a = adult(K::DarkDragon);
    a.behavior = B::DragonSurge;
    a.needs.goal = Some(NeedGoal {
        kind: NeedKind::AttackAim,
        position: [8, 1, 0],
    });
    let target = [8.5, 2.0, 0.5];
    let prey = [(Target::Player(PlayerUuid::from_u128(2)), target)];
    for yaw in [0_f32, std::f32::consts::FRAC_PI_2] {
        a.position = [8.5, 1., 12.5];
        lineages::resolve(&mut a, &h, pose::Lineage::Shadow, false, &prey, Some(yaw));
        let delta = DVec3::from_array(a.position) - DVec3::from_array(target);
        let (s, c) = f64::from(yaw).sin_cos();
        assert!(delta.dot(DVec3::new(s, 0., c)) > 7. && facing(&a, DVec3::from_array(target)));
    }
    let before = a.position;
    a.needs.goal.as_mut().unwrap().position = [500, 1, 0];
    lineages::resolve(&mut a, &h, pose::Lineage::Shadow, false, &prey, Some(0.));
    assert_eq!(a.position, before);
}

#[test]
fn grounded_fire_and_dark_guardians_make_combat_passes_and_land_to_hit() {
    use crate::streaming::LoadedSection;
    use terra_voxel::{BlockSection, SectionPos};
    let (terrain, mut chunks, journal) = fixture();
    for x in -3..=3 {
        for z in -3..=3 {
            for y in 0..=2 {
                let p = SectionPos::new(x, y, z);
                chunks.loaded.entry(p).or_insert_with(|| {
                    let mut data = BlockSection::empty(p).unwrap();
                    if y == 0 {
                        for x in 0..16 {
                            for z in 0..16 {
                                data.set_block([x, 0, z], 3).unwrap();
                            }
                        }
                    }
                    LoadedSection {
                        data,
                        mesh: None,
                        mesh_epoch: 0,
                        water_epoch: 0,
                        water: None,
                        props: vec![],
                    }
                });
            }
        }
    }
    let h = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    for (kind, variant, range) in [
        (K::FireDragon, 0, 4.),
        (K::DarkDragon, 0, 6.),
        (K::FireDragon, 3, 22.),
        (K::DarkDragon, 5, 18.),
    ] {
        let mut a = adult(kind);
        a.variant = variant;
        let mut encounter = Encounter::new(&a, None);
        let target = [(
            Target::Player(PlayerUuid::from_u128(112)),
            [8.5, 1.95, 12.5 - range],
        )];
        let mut last = a.behavior;
        let mut history = vec![];
        let mut airborne_hit = false;
        let mut landed = false;
        let mut touchdown_hit = false;
        for _ in 0..1100 {
            let hits = encounter.step(&mut a, &h, 0.1, &target, true, false);
            if last != a.behavior {
                history.push(a.behavior);
                last = a.behavior;
            }
            if hits.is_empty() {
                continue;
            }
            if a.behavior == B::Spit
                && a.needs
                    .goal
                    .is_some_and(|g| g.kind == NeedKind::AirborneAttackAim)
            {
                assert!(a.position[1] > 5.);
                airborne_hit = true;
            }
            landed |= airborne_hit && history.contains(&B::Land);
            if landed && a.behavior == B::Tremor {
                assert!(on_ground(&a, &h));
                touchdown_hit = true;
                break;
            }
        }
        assert!(
            touchdown_hit,
            "{kind:?}/{variant}, range {range}: flight hit {airborne_hit}, landed {landed}, {history:?}, final {:?}",
            a.position
        );
    }
}
