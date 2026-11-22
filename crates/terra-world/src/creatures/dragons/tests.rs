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
    let mut a = adult(K::WaterDragon);
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
    let id = PlayerUuid::from_u128(1);
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
    let id = PlayerUuid::from_u128(1);
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
    let owner = PlayerUuid::from_u128(1);
    let bystander = PlayerUuid::from_u128(2);
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
        let id = PlayerUuid::from_u128(3);
        let hit = encounter.strikes(
            &a,
            &h,
            &[
                (id, [8.5, 2.5, -8.5]),
                (PlayerUuid::nil(), [15.5, 2.5, -8.5]),
            ],
        );
        assert_eq!(hit.len(), 1, "{kind:?} muzzle {:?}", pose::muzzle(&a));
        assert_eq!(hit[0].target, id);
    }
}

#[test]
fn all_three_guardians_use_the_walkable_entrance_fly_out_and_return_to_their_nest() {
    use crate::generation::sections::SectionGenerator;
    use crate::streaming::{ChunkStore, LoadedSection};
    use crate::{TerraGenerator, edits::EditJournal};
    use terra_voxel::{SectionPos, global_to_section};

    let terrain = TerraGenerator::new(Default::default()).unwrap();
    let sections = SectionGenerator::new(terrain.clone());
    for region in [[-1, 3], [-1, -5], [-6, 5]] {
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
        // For one real nest cover the longest possible route including the leash.
        // Pick an age/identity whose first deterministic excursion targets the rim.
        if kind == K::EarthDragon {
            a.age = 1280.;
            a.id = (1..10_000)
                .find(|id| crate::generation::seed::layer_seed(id + 10, 0).is_multiple_of(353))
                .unwrap();
        }
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
            // Follow the same moving 5x3x5 collision neighborhood as server
            // streaming. Exercise the real trees and cave roofs along the sortie.
            let focus = global_to_section(a.position.map(|v| v.floor() as i64)).0;
            if previous != Some(focus) {
                previous = Some(focus);
                for z in focus.z - 2..=focus.z + 2 {
                    for x in focus.x - 2..=focus.x + 2 {
                        for y in focus.y - 1..=focus.y + 1 {
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
            "{kind:?}: maximum horizontal distance {furthest:.1}, clearance {max_clearance:.1} blocks, returned {returned}"
        );
        assert!(
            max_clearance >= 35.,
            "{kind:?} cruising only {max_clearance:.1} above terrain"
        );
        if kind == K::EarthDragon {
            assert!(furthest > 495., "long patrol stopped at {furthest}");
        }
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
    let mut a = adult(K::EarthDragon);
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
    let target = PlayerUuid::from_u128(5);
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
    lineages::resolve(&mut a, &h, pose::Lineage::Tidal, false, &[]);
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
            (PlayerUuid::from_u128(1), [8.5, 2., 0.5]),
            (PlayerUuid::from_u128(2), [14.5, 2., 0.5]),
        ],
    );
    assert_eq!(hit.len(), 1);
    assert!(hit[0].impulse[1] > 8.);
    a.kind = K::DarkDragon;
    let before = a.position;
    lineages::resolve(&mut a, &h, pose::Lineage::Shadow, false, &[]);
    assert_ne!(a.position, before);
    assert!(h.clear_rotated(a.position, a.size(), false, f64::from(a.yaw)));
    a.needs.goal.as_mut().unwrap().position = [1000, 2, -600];
    let before = a.position;
    lineages::resolve(&mut a, &h, pose::Lineage::Shadow, false, &[]);
    assert_eq!(a.position, before);
}
