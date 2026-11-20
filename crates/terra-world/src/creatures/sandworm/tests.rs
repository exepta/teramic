use super::*;
use crate::{
    SectionGenerator, TerraGenerator,
    edits::EditJournal,
    streaming::{ChunkStore, LoadedSection},
};
use terra_core::{
    WorldConfig,
    creatures::{CreatureNeeds, CreaturePopulation, creature_id},
};
use terra_voxel::{MaterialId, SectionPos};

fn fixture() -> (TerraGenerator, ChunkStore, EditJournal) {
    let terrain = TerraGenerator::new(WorldConfig {
        biomes: false,
        trees: false,
        caves: false,
        water: false,
        ..Default::default()
    })
    .unwrap();
    let mut chunks = ChunkStore::default();
    for x in -1..=1 {
        for z in -1..=1 {
            for y in -1..=0 {
                let position = terra_voxel::SectionPos::new(x, y, z);
                let mut data = terra_voxel::BlockSection::empty(position).unwrap();
                for lx in 0..16 {
                    for lz in 0..16 {
                        for ly in 0..16 {
                            if y < 0 || ly == 0 {
                                data.set_block([lx, ly, lz], 4).unwrap();
                            }
                        }
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
    }
    (terrain, chunks, EditJournal::default())
}
fn worm() -> Creature {
    Creature {
        id: creature_id([0, 0], 0).unwrap(),
        kind: Kind::SandWorm,
        home: [0, 0],
        position: [8.5, 1.0, 8.5],
        velocity: [0.0; 3],
        yaw: 0.0,
        health: Kind::SandWorm.health(),
        behavior: B::Burrowed,
        timer: 0.0,
        age: 0.0,
        variant: 0,
        needs: CreatureNeeds::full(Kind::SandWorm),
    }
}

#[test]
fn desert_selection_is_sparse_deterministic_and_excludes_other_biomes() {
    for biome in ["desert", "cliffted_desert"] {
        let count = (0..50_u64)
            .filter(|n| surface_species(biome, n << 40) == Some(Kind::SandWorm))
            .count();
        assert_eq!(count, 10);
    }
    for biome in [
        "forest",
        "ocean",
        "plains",
        "blood_forest",
        "spirit_blossom",
    ] {
        assert!(surface_species(biome, 0).is_none());
    }
}

#[test]
fn stepped_dunes_check_each_sand_column_instead_of_requiring_a_flat_slab() {
    let (terrain, mut chunks, journal) = fixture();
    for z in 6..=10 {
        chunks
            .loaded
            .get_mut(&SectionPos::default())
            .unwrap()
            .data
            .set_block([9, 1, z], 4)
            .unwrap();
    }
    let habitat = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    let p =
        surface(&habitat, [8.5, 1.0, 8.5]).expect("one-block dune steps are valid burrowing sand");
    assert_eq!(p[1], 2.0);
}

#[test]
fn explicit_command_worm_is_exposed_and_pickable_on_ordinary_dry_ground() {
    let (terrain, chunks, mut journal) = fixture();
    for x in 0..16 {
        for z in 0..16 {
            journal.record([x, 0, z], 3).unwrap();
        }
    }
    let habitat = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    assert!(
        surface(&habitat, [8.5, 1.0, 8.5]).is_none(),
        "stone is not natural burrowing sand"
    );
    let mut a = super::super::spawning::near(
        &CreaturePopulation::default(),
        &habitat,
        terra_core::creatures::spawning::SpawnMob {
            kind: Kind::SandWorm,
            variant: 0,
        },
        [8.5, 1.0, 8.5],
        0.0,
    )
    .unwrap();
    assert_eq!(a.behavior, B::Attack);
    assert_eq!(a.timer, EXPOSED_SECONDS);
    assert!(terra_core::creatures::spawning::is_command_id(a.id));
    let mut spits = 0;
    for _ in 0..100 {
        match step(&mut a, &habitat, &[], 0.1) {
            Effect::SpitBile { .. } => spits += 1,
            Effect::None => {}
            other => panic!("inspection without vulnerable players cannot bite: {other:?}"),
        }
    }
    assert_eq!(spits, 0, "no enemy means no spit");
    assert_eq!(
        a.behavior,
        B::Burrowed,
        "a command worm also finishes its exposed phase"
    );
    assert_eq!(
        terra_core::creatures::hitboxes::for_creature(&a)
            .iter()
            .count(),
        0
    );
    assert!(a.valid());
}

#[test]
fn a_command_worm_on_desert_sand_faces_the_summoner_and_remains_visible_for_inspection() {
    let (terrain, chunks, journal) = fixture();
    let habitat = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    let origin = [8.5, 1.0, 8.5];
    let mut a = super::super::spawning::near(
        &CreaturePopulation::default(),
        &habitat,
        terra_core::creatures::spawning::SpawnMob {
            kind: Kind::SandWorm,
            variant: 0,
        },
        origin,
        0.0,
    )
    .unwrap();
    let dx = origin[0] - a.position[0];
    let dz = origin[2] - a.position[2];
    assert!((-f64::from(a.yaw.sin()) * dx - f64::from(a.yaw.cos()) * dz) / dx.hypot(dz) > 0.99);
    let mut spits = 0;
    for _ in 0..30 {
        match step(&mut a, &habitat, &[], 0.1) {
            Effect::SpitBile { .. } => spits += 1,
            Effect::None => {}
            other => panic!("no survival target for a bite: {other:?}"),
        }
        assert_eq!(a.behavior, B::Attack);
        assert_eq!(terra_core::creatures::sandworm::emergence(&a), 1.0);
    }
    assert_eq!(spits, 0, "creative observers must never trigger a spit");
}

#[test]
fn stationary_worms_wait_underground_without_movement_and_never_invent_a_bite() {
    let (terrain, chunks, journal) = fixture();
    let habitat = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    let mut a = worm();
    let mut exposed = false;
    let mut spits = 0;
    for _ in 0..100 {
        match step(&mut a, &habitat, &[], 0.1) {
            Effect::SpitBile { origin, velocity } => {
                spits += 1;
                assert!(origin.iter().chain(&velocity).all(|v| v.is_finite()));
            }
            Effect::None => {}
            other => panic!("observers cannot be bitten: {other:?}"),
        }
        exposed |= a.behavior == B::Attack;
    }
    assert_eq!(spits, 0, "empty observer cycles never spit");
    assert!(!exposed, "no footfalls means no eruption");
}

#[test]
fn a_real_sand_bed_is_required_and_stone_floors_roofs_unknown_data_block_emergence() {
    let (t, c, mut j) = fixture();
    assert_eq!(
        surface(
            &Habitat {
                terrain: &t,
                chunks: &c,
                journal: &j
            },
            [8.5, 1.0, 8.5]
        ),
        Some([8.5, 1.0, 8.5])
    );
    for (p, id) in [([8, 0, 8], 3), ([8, -2, 8], 3), ([8, 3, 8], 3)] {
        j.record(p, id).unwrap();
        assert!(
            surface(
                &Habitat {
                    terrain: &t,
                    chunks: &c,
                    journal: &j
                },
                [8.5, 1.0, 8.5]
            )
            .is_none()
        );
        j.record(p, if p[1] <= 0 { 4 } else { 0 }).unwrap();
    }
    let empty = ChunkStore::default();
    assert!(
        surface(
            &Habitat {
                terrain: &t,
                chunks: &empty,
                journal: &j
            },
            [8.5, 1.0, 8.5]
        )
        .is_none()
    );
}

#[test]
fn generated_sandstone_under_a_sand_cap_is_burrowable_but_a_built_floor_is_not() {
    let (t, mut c, mut j) = fixture();
    for section in c.loaded.values_mut() {
        if section.data.position().y != -1 {
            continue;
        }
        for x in 0..16 {
            for z in 0..16 {
                for y in 13..16 {
                    section
                        .data
                        .set_block([x, y, z], MaterialId::Sandstone as u16)
                        .unwrap();
                }
            }
        }
    }
    let valid = |j: &EditJournal| {
        surface(
            &Habitat {
                terrain: &t,
                chunks: &c,
                journal: j,
            },
            [8.5, 1.0, 8.5],
        )
        .is_some()
    };
    assert!(
        valid(&j),
        "natural deserts have a thin sand cap over weathered sandstone"
    );
    j.record([8, -1, 8], MaterialId::Sandstone as u16).unwrap();
    assert!(
        !valid(&j),
        "player-built sandstone is not natural burrowing substrate"
    );
    j.record([8, -1, 8], MaterialId::Sand as u16).unwrap();
    assert!(valid(&j));
    j.record([8, 7, 8], MaterialId::Stone as u16).unwrap();
    assert!(
        !valid(&j),
        "a high roof must also prevent an eruption inside a building"
    );
}

fn generated_desert(terrain: &TerraGenerator) -> ([i64; 2], ChunkStore) {
    let generator = SectionGenerator::new(terrain.clone());
    // Filter inexpensive deterministic identity choices before sampling terrain.
    // A real generated floor, not an injected biome or selector, must pass.
    for z in (-8192_i64..=8192).step_by(128) {
        for x in (-8192_i64..=8192).step_by(128) {
            let home = [x / 32, z / 32];
            let id = creature_id(home, 0).unwrap();
            let seed = super::super::random(id, terrain.config().seed.0);
            if (seed >> 32).is_multiple_of(3)
                || (seed >> 40).is_multiple_of(7)
                || surface_species("desert", seed).is_none()
            {
                continue;
            }
            let px = (x + 8 + (seed % 8) as i64) as f64 + 0.5;
            let pz = (z + 8 + ((seed >> 8) % 8) as i64) as f64 + 0.5;
            let surface = terrain.surface_column(px, pz);
            let biome = super::super::biome_slug(terrain.surface_biome(&surface));
            if surface_species(biome, seed).is_none()
                || surface.surface_material != MaterialId::Sand
                || surface.slope > 0.25
                || terrain.water_column(px, pz).is_some()
            {
                continue;
            }
            let mut chunks = ChunkStore::default();
            for dz in 0..2 {
                for dx in 0..2 {
                    let column = [home[0] * 2 + dx, home[1] * 2 + dz];
                    let prepared =
                        std::sync::Arc::new(generator.prepare_column(column, false).unwrap());
                    let mut low = i64::MAX;
                    let mut high = i64::MIN;
                    for z in 0..16 {
                        for x in 0..16 {
                            let height =
                                prepared.local_surface(x, z).unwrap().height.floor() as i64;
                            low = low.min((height - 4).div_euclid(16));
                            high = high.max((height + 10).div_euclid(16));
                        }
                    }
                    for y in low..=high {
                        let position = SectionPos::new(column[0], y, column[1]);
                        let generated = generator
                            .build_prepared(position, 0, &Default::default(), &prepared)
                            .unwrap();
                        chunks.loaded.insert(
                            position,
                            LoadedSection {
                                data: generated.data,
                                mesh: None,
                                mesh_epoch: 0,
                                water_epoch: 0,
                                water: generated.water,
                                props: generated.props,
                            },
                        );
                    }
                    chunks.set_prepared_column_for_test(column, Some(prepared));
                }
            }
            let journal = EditJournal::default();
            let h = Habitat {
                terrain,
                chunks: &chunks,
                journal: &journal,
            };
            if spawn_surface(&h, [px, surface.height.floor() + 1.0, pz]).is_some() {
                return (home, chunks);
            }
        }
    }
    panic!("seed 42042 must provide an eligible loaded natural desert habitat");
}

#[test]
fn natural_spawner_creates_a_worm_in_generated_desert_and_repairs_old_empty_habitats_once() {
    let terrain = TerraGenerator::new(WorldConfig {
        landscape_revision: 6,
        structures_revision: 0,
        caves: false,
        trees: false,
        ..Default::default()
    })
    .unwrap();
    let (home, chunks) = generated_desert(&terrain);
    let journal = EditJournal::default();
    let habitat = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    let mut population = CreaturePopulation::default();
    let near = [
        (home[0] * 32 + 16) as f64,
        100.0,
        (home[1] * 32 + 16) as f64,
    ];
    assert!(!super::super::spawn_cell_away_from_players(
        &mut population,
        &habitat,
        home,
        &[near],
        1
    ));
    assert!(
        !population.explored.contains(&home),
        "defer until the player leaves instead of permanently consuming this habitat"
    );
    let far = [near[0] + 64.0, near[1], near[2]];
    assert!(super::super::spawn_cell_away_from_players(
        &mut population,
        &habitat,
        home,
        &[far],
        1
    ));
    assert_eq!(
        population.animals.len(),
        1,
        "one remaining population slot permits the solitary worm"
    );
    let id = creature_id(home, 0).unwrap();
    let a = population
        .animals
        .get(&id)
        .expect("the normal natural spawn path must create its deterministic identity");
    assert_eq!(a.kind, Kind::SandWorm);
    assert_eq!(a.behavior, B::Burrowed);
    assert!(a.valid());
    assert_eq!(surface(&habitat, a.position), Some(a.position));
    assert_eq!(
        habitat.block([
            a.position[0].floor() as i64,
            a.position[1] as i64 - 1,
            a.position[2].floor() as i64
        ]),
        Some(MaterialId::Sand as u16)
    );
    assert!(
        chunks
            .loaded
            .values()
            .any(|s| s.data.blocks().contains(&(MaterialId::Sandstone as u16)))
    );
    let saved = population.clone();
    assert!(!super::super::spawn_cell(&mut population, &habitat, home));
    assert_eq!(population, saved);
    let mut old = CreaturePopulation::default();
    old.explored.insert(home);
    assert!(
        super::super::spawn_cell(&mut old, &habitat, home),
        "a formerly rejected explored desert must recover without retrogen"
    );
    assert_eq!(old.animals.get(&id), population.animals.get(&id));
    old.animals.get_mut(&id).unwrap().health = 0.0;
    assert!(
        !super::super::spawn_cell(&mut old, &habitat, home),
        "recorded deaths must never respawn or reroll"
    );
    assert_eq!(old.animals[&id].health, 0.0);
    let unloaded = Habitat {
        terrain: &terrain,
        chunks: &ChunkStore::default(),
        journal: &journal,
    };
    let mut old_empty = CreaturePopulation::default();
    old_empty.explored.insert(home);
    assert!(!super::super::spawn_cell(&mut old_empty, &unloaded, home));
    assert!(
        old_empty.animals.is_empty(),
        "unknown terrain must remain unavailable"
    );
}

#[test]
fn an_emerged_bite_chooses_a_visible_victim_over_a_nearer_hidden_seismic_target() {
    let (terrain, chunks, mut journal) = fixture();
    for y in 1..=3 {
        journal.record([8, y, 6], MaterialId::Stone as u16).unwrap();
    }
    let habitat = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    let hidden = [8.5, 1.9, 5.8];
    let visible = [11.25, 1.9, 8.5];
    let mut a = worm();
    a.behavior = B::Attack;
    a.timer = EXPOSED_SECONDS * (1.0 - BITE_PHASE) + 0.05;
    assert!(distance(hidden, a.position) < distance(visible, a.position));
    assert_eq!(
        step(&mut a, &habitat, &[hidden, visible], 0.1),
        Effect::StrikePlayer {
            index: 1,
            damage: 6.0
        }
    );
    a = worm();
    a.behavior = B::Attack;
    a.timer = EXPOSED_SECONDS * (1.0 - BITE_PHASE) + 0.05;
    assert_eq!(
        step(&mut a, &habitat, &[hidden], 0.1),
        Effect::None,
        "seismic awareness must never permit a through-wall bite"
    );
}

#[test]
fn full_cycle_warns_before_one_strike_exposes_then_buries_with_a_long_rearm() {
    let (t, c, j) = fixture();
    let h = Habitat {
        terrain: &t,
        chunks: &c,
        journal: &j,
    };
    let mut a = worm();
    let mut phases = std::collections::BTreeSet::new();
    let mut hits = Vec::new();
    let mut hidden = 0;
    for tick in 0..300 {
        let target = [[a.position[0] + 1.0, a.position[1] + 0.9, a.position[2]]];
        let effect = step(&mut a, &h, &target, 0.1);
        phases.insert(a.behavior as u8);
        if matches!(a.behavior, B::Burrowed | B::Tremor) {
            hidden += 1;
        }
        if let Effect::StrikePlayer { index, damage } = effect {
            assert_eq!(index, 0);
            if damage == KNOCKUP_DAMAGE {
                assert_eq!(a.behavior, B::Emerge);
                continue;
            }
            assert_eq!(damage, 6.0);
            assert!(
                tick >= 26,
                "no strike before the complete ground warning and emergence"
            );
            hits.push(tick);
        }
        assert!(a.valid(), "{a:?}");
    }
    assert!(!hits.is_empty());
    assert!(hits.windows(2).all(|p| p[1] - p[0] >= 120));
    assert!(hidden > 65, "rearming is spent underground");
    for phase in [B::Burrowed, B::Tremor, B::Emerge, B::Attack, B::Retreat] {
        assert!(phases.contains(&(phase as u8)));
    }
    assert!(
        !phases.contains(&(B::Leap as u8)),
        "a close bite erupts in place rather than taking a travelling detour"
    );
    let saved = serde_json::to_vec(&a).unwrap();
    assert_eq!(a, serde_json::from_slice::<Creature>(&saved).unwrap());
}

#[test]
fn warning_site_stays_fixed_so_a_player_can_dodge_and_walls_stop_a_strike() {
    let (t, c, mut j) = fixture();
    let mut a = worm();
    step(
        &mut a,
        &Habitat {
            terrain: &t,
            chunks: &c,
            journal: &j,
        },
        &[[9.5, 1.9, 8.5]],
        0.1,
    );
    assert_eq!(a.behavior, B::Tremor);
    let locked = a.position;
    for _ in 0..95 {
        if a.behavior == B::Attack {
            break;
        }
        assert_eq!(
            step(
                &mut a,
                &Habitat {
                    terrain: &t,
                    chunks: &c,
                    journal: &j
                },
                &[[14.5, 1.9, 8.5]],
                0.1,
            ),
            Effect::None,
        );
        if matches!(a.behavior, B::Tremor | B::Leap) {
            assert_eq!(a.position, locked, "the announced arc never chases a dodge");
        }
    }
    assert_eq!(a.behavior, B::Attack);
    for _ in 0..10 {
        assert_eq!(
            step(
                &mut a,
                &Habitat {
                    terrain: &t,
                    chunks: &c,
                    journal: &j
                },
                &[[14.5, 1.9, 8.5]],
                0.1
            ),
            Effect::None
        );
    }
    for z in 7..=10 {
        for y in 1..=4 {
            j.record([10, y, z], 3).unwrap();
        }
    }
    a = worm();
    a.behavior = B::Attack;
    a.timer = EXPOSED_SECONDS * (1.0 - BITE_PHASE) + 0.05;
    assert_eq!(
        step(
            &mut a,
            &Habitat {
                terrain: &t,
                chunks: &c,
                journal: &j
            },
            &[[11.2, 1.9, 8.5]],
            0.1
        ),
        Effect::None
    );
}

#[test]
fn invalid_timesteps_do_not_advance_or_damage_and_burrowing_cannot_cross_stone() {
    let (t, c, mut j) = fixture();
    let mut a = worm();
    let original = a.clone();
    for dt in [0.0, -0.1, f32::NAN, 0.3] {
        assert_eq!(
            step(
                &mut a,
                &Habitat {
                    terrain: &t,
                    chunks: &c,
                    journal: &j
                },
                &[[9.5, 1.9, 8.5]],
                dt
            ),
            Effect::None
        );
        assert_eq!(a, original);
    }
    for z in 0..16 {
        j.record([10, 0, z], 3).unwrap();
    }
    for _ in 0..80 {
        step(
            &mut a,
            &Habitat {
                terrain: &t,
                chunks: &c,
                journal: &j,
            },
            &[[14.5, 1.9, 8.5]],
            0.1,
        );
    }
    assert!(a.position[0] < 10.0);
}

#[test]
fn buried_observer_motion_and_exposed_crawling_leave_the_stationary_mound_behind() {
    let (terrain, chunks, journal) = fixture();
    let h = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    let mut a = worm();
    a.timer = BURROW_SECONDS;
    let initial = a.position;
    let mut travelled = false;
    for _ in 0..20 {
        assert_eq!(
            step_observed(&mut a, &h, &[], &[[14.0, 1.9, 8.5]], 0.1),
            Effect::None
        );
        travelled |= a.velocity.iter().any(|v| v.abs() > 0.1);
    }
    assert_eq!(a.behavior, B::Burrowed);
    assert!(
        distance(initial, a.position) > 0.5,
        "observed movement leaves a moving sand trail"
    );
    assert!(
        travelled,
        "pursuit must produce physical movement before reaching the disturbance"
    );
    a = worm();
    a.behavior = B::Attack;
    a.timer = EXPOSED_SECONDS * 0.70;
    let initial = a.position;
    for _ in 0..8 {
        assert_eq!(step(&mut a, &h, &[[14.0, 1.9, 8.5]], 0.1), Effect::None);
    }
    assert_eq!(a.behavior, B::Attack);
    assert!(
        a.position[0] - initial[0] > 0.25,
        "the exposed body must crawl forward after its bite"
    );
    assert!(surface(&h, a.position).is_some());
}

#[test]
fn exposed_bite_has_a_windup_and_bile_releases_once_after_the_bite() {
    let (terrain, chunks, journal) = fixture();
    let h = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    let mut a = worm();
    a.behavior = B::Emerge;
    a.timer = 0.05;
    let target = [[9.5, 1.9, 8.5]];
    assert_eq!(step(&mut a, &h, &target, 0.1), Effect::None);
    assert_eq!(a.behavior, B::Attack);
    let mut bites = 0;
    let mut spits = 0;
    for tick in 0..45 {
        let targets = if tick < 15 {
            target
        } else {
            [[14.0, 1.9, 8.5]]
        };
        match step(&mut a, &h, &targets, 0.1) {
            Effect::StrikePlayer { index, damage } => {
                assert_eq!(index, 0);
                assert_eq!(damage, 6.0);
                assert!(
                    (7..=9).contains(&tick),
                    "bite must coincide with the closing mouth"
                );
                bites += 1;
            }
            Effect::SpitBile { origin, velocity } => {
                assert!(tick > 25 && bites == 1);
                assert!(origin[1] > a.position[1] + 1.0);
                assert!(velocity[0].hypot(velocity[2]) > 7.9);
                spits += 1;
            }
            Effect::None => {}
            other => panic!("unexpected worm effect {other:?}"),
        }
    }
    assert_eq!((bites, spits), (1, 1));
}

#[test]
fn only_a_visible_enemy_in_ranged_reach_can_trigger_a_locked_aim_spit() {
    let (terrain, chunks, mut journal) = fixture();
    let release = || {
        let mut a = worm();
        a.behavior = B::Attack;
        a.timer = EXPOSED_SECONDS * (1.0 - SPIT_PHASE) + 0.05;
        a
    };
    let target = [[14.5, 1.9, 8.5]];
    let h = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    let mut empty = release();
    assert_eq!(step(&mut empty, &h, &[], 0.1), Effect::None);
    assert!(empty.needs.goal.is_none());
    let mut close = release();
    assert_eq!(step(&mut close, &h, &[[9.5, 1.9, 8.5]], 0.1), Effect::None);
    let mut aimed = release();
    let Effect::SpitBile { origin, velocity } = step(&mut aimed, &h, &target, 0.1) else {
        panic!("visible ranged enemy must be aimed at");
    };
    assert_eq!(aimed.behavior, B::Spit);
    let aim = aimed.needs.goal.unwrap().position.map(|v| v as f64 + 0.5);
    let time = (aim[0] - origin[0]).hypot(aim[2] - origin[2]) / velocity[0].hypot(velocity[2]);
    let at = std::array::from_fn::<_, 3, _>(|i| {
        origin[i] + velocity[i] * time
            - if i == 1 {
                0.5 * f64::from(SPIT_GRAVITY) * time * time
            } else {
                0.0
            }
    });
    assert!(at.into_iter().zip(aim).all(|(a, b)| (a - b).abs() < 1e-6));
    for _ in 0..10 {
        assert_eq!(step(&mut aimed, &h, &[], 0.1), Effect::None);
    }
    for y in 1..=9 {
        journal.record([11, y, 8], 3).unwrap();
    }
    let h = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    let mut blocked = release();
    assert_eq!(step(&mut blocked, &h, &target, 0.1), Effect::None);
    assert!(blocked.needs.goal.is_none());
}

#[test]
fn a_visible_eye_sightline_is_insufficient_when_the_real_lob_hits_a_ceiling() {
    let (terrain, chunks, mut journal) = fixture();
    for x in 0..=5 {
        journal.record([x, 7, 8], 3).unwrap();
    }
    let h = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    let mut a = worm();
    a.behavior = B::Attack;
    a.timer = EXPOSED_SECONDS * (1.0 - SPIT_PHASE) + 0.05;
    let targets = [[-5.5, 1.9, 8.5]];
    assert!(
        super::super::perception::nearest_visible(
            &h,
            [8.5, 7.85, 8.5],
            a.position,
            &targets,
            16.0,
            |_| true
        )
        .is_some()
    );
    assert_eq!(step(&mut a, &h, &targets, 0.1), Effect::None);
    assert!(a.needs.goal.is_none());
}

#[test]
fn a_two_block_cliff_is_not_burrowable_and_swept_motion_stops_before_its_edge() {
    let (terrain, chunks, mut journal) = fixture();
    // Extend the ledge to both loaded boundaries. A short notch can now
    // legitimately be skirted by the worm's lateral pursuit.
    for x in 9..=15 {
        for z in -16..32 {
            for y in -1..=0 {
                journal.record([x, y, z], 0).unwrap();
            }
        }
    }
    let h = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    assert!(surface(&h, [8.5, 1.0, 8.5]).is_none());
    let mut a = worm();
    a.position[0] = 6.5;
    a.timer = BURROW_SECONDS;
    for _ in 0..25 {
        step(&mut a, &h, &[[15.0, 1.9, 8.5]], 0.1);
    }
    assert!(
        a.position[0] < 8.1,
        "underground anchor may not cross an unsupported ledge: {:?}",
        a.position
    );
    assert!(surface(&h, a.position).is_some());
}

#[test]
fn an_exposed_distant_worm_completes_its_cycle_without_loaded_terrain() {
    let (terrain, _, journal) = fixture();
    let chunks = ChunkStore::default();
    let h = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    for phase in [
        B::Tremor,
        B::Leap,
        B::Emerge,
        B::Attack,
        B::Spit,
        B::Retreat,
    ] {
        let mut a = worm();
        a.position = [180.0, 1.0, 8.5];
        a.behavior = phase;
        a.timer = 0.2;
        for _ in 0..100 {
            assert_eq!(step_observed(&mut a, &h, &[], &[], 0.1), Effect::None);
        }
        assert_eq!(
            a.behavior,
            B::Burrowed,
            "{phase:?} must not freeze at the streaming boundary"
        );
        assert!(
            terra_core::creatures::hitboxes::for_creature(&a)
                .iter()
                .next()
                .is_none()
        );
    }
}

#[test]
fn a_stationary_nearby_target_does_not_wake_a_worm_but_an_observed_footfall_does() {
    let (terrain, chunks, journal) = fixture();
    let h = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    let mut a = worm();
    let target = [[9.5, 1.9, 8.5]];
    for _ in 0..100 {
        step_observed(&mut a, &h, &target, &[], 0.1);
    }
    assert_eq!(a.behavior, B::Burrowed);
    step_observed(&mut a, &h, &[], &target, 0.1);
    assert_eq!(
        a.behavior,
        B::Tremor,
        "creative movement can announce a breach without becoming prey"
    );
}

#[test]
fn leap_root_only_moves_after_the_last_annulus_reenters_the_sand() {
    let (terrain, chunks, journal) = fixture();
    let h = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    let mut a = worm();
    a.behavior = B::Leap;
    a.timer = 0.2;
    let origin = a.position;
    let landing = leap_destination(&a);
    step_observed(&mut a, &h, &[], &[], 0.1);
    assert_eq!(a.position, origin);
    assert!((0..=HEAD_SEGMENT).all(|i| !anatomy_visible(B::Leap, pose(&a, i), i)));
    step_observed(&mut a, &h, &[], &[], 0.1);
    assert_eq!(a.position, landing);
    assert_eq!(a.behavior, B::Emerge);
}

#[test]
fn a_close_attack_erupts_at_its_current_burrow_position_and_reuses_its_actual_dive_hole() {
    let (terrain, chunks, journal) = fixture();
    let h = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    let mut a = worm();
    a.timer = BURROW_SECONDS;
    let initial = a.position;
    let moving = [[14.5, 1.9, 8.5]];
    for _ in 0..4 {
        step_observed(&mut a, &h, &moving, &moving, 0.1);
    }
    assert_ne!(
        a.position, initial,
        "first use real swept underground pursuit"
    );
    let actual_hole = a.position;
    let close = [[actual_hole[0] + 1., actual_hole[1] + 0.9, actual_hole[2]]];
    for _ in 0..2 {
        a.timer = 0.;
        step_observed(&mut a, &h, &close, &close, 0.1);
        assert_eq!(a.behavior, B::Tremor);
        for _ in 0..30 {
            if a.behavior != B::Tremor {
                break;
            }
            step_observed(&mut a, &h, &close, &close, 0.1);
        }
        assert_eq!(
            a.behavior,
            B::Emerge,
            "near prey must not force a fourteen-metre arc"
        );
        assert_eq!(a.position, actual_hole);
        let mut saw_retreat = false;
        for _ in 0..90 {
            if a.behavior == B::Burrowed {
                break;
            }
            assert_eq!(a.position, actual_hole);
            if a.behavior == B::Emerge && phase_progress(a.behavior, a.timer) < 0.68 {
                let head = pose(&a, HEAD_SEGMENT);
                assert!(head.center[0].abs() < 0.00001 && head.center[2].abs() < 0.00001);
            }
            saw_retreat |= a.behavior == B::Retreat;
            step_observed(&mut a, &h, &close, &close, 0.1);
        }
        assert!(saw_retreat);
        assert_eq!(a.behavior, B::Burrowed);
        assert_eq!(
            a.position, actual_hole,
            "the next eruption cannot snap to a stale origin"
        );
    }
}

#[test]
fn a_long_range_travelling_leap_remains_separate_from_a_close_attack_eruption() {
    let (terrain, chunks, journal) = fixture();
    let h = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    let mut a = worm();
    a.yaw = 0.;
    a.behavior = B::Tremor;
    a.timer = 0.05;
    let far = [[8.5, 1.9, -2.0]];
    step_observed(&mut a, &h, &far, &far, 0.1);
    assert_eq!(a.behavior, B::Leap);
    assert_eq!(a.position, [8.5, 1., 8.5]);
}

#[test]
fn healthy_worm_patrols_beneath_quiet_observers_without_a_combat_aim_or_breach() {
    let (terrain, chunks, journal) = fixture();
    let h = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    let mut a = worm();
    assert_eq!(a.health, 120.0);
    let start = a.position;
    let mut path = 0.0;
    for _ in 0..100 {
        let before = a.position;
        let quiet = [[a.position[0] + 1.0, a.position[1] + 1.5, a.position[2]]];
        assert_eq!(step_observed(&mut a, &h, &quiet, &[], 0.1), Effect::None);
        assert_eq!(a.behavior, B::Burrowed);
        assert!(a.needs.goal.is_none());
        assert_eq!(
            terra_core::creatures::hitboxes::for_creature(&a)
                .iter()
                .count(),
            0
        );
        path += distance(a.position, before).sqrt();
    }
    assert!(path > 5.0 && distance(start, a.position) > 1.0);
}

#[test]
fn warned_charge_runs_fast_on_sand_strikes_once_and_does_not_chase_a_dodge() {
    let (terrain, chunks, journal) = fixture();
    let h = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    let target = [[19.5, 1.9, 8.5]];
    let mut a = worm();
    assert_eq!(
        step_observed(&mut a, &h, &target, &target, 0.1),
        Effect::None
    );
    assert_eq!(a.behavior, B::Tremor);
    let locked = a.needs.goal.unwrap();
    for _ in 0..30 {
        step_observed(&mut a, &h, &target, &[], 0.1);
        if a.behavior == B::Hunt {
            break;
        }
    }
    assert_eq!(a.behavior, B::Hunt);
    let origin = a.position;
    let mut hit = None;
    for _ in 0..20 {
        let effect = step_observed(&mut a, &h, &target, &[], 0.1);
        if let Effect::StrikePlayer { index, damage } = effect {
            hit = Some((index, damage));
            break;
        }
        assert_eq!(effect, Effect::None);
        assert!(
            terra_core::creatures::hitboxes::for_creature(&a)
                .iter()
                .next()
                .is_none()
        );
    }
    assert_eq!(hit, Some((0, CHARGE_DAMAGE)));
    assert!(distance(origin, a.position).sqrt() >= 8.0);
    assert_eq!(a.behavior, B::Emerge);
    assert_eq!(
        a.timer, EMERGE_SECONDS,
        "a charge hit starts the complete vertical rise"
    );
    assert_eq!(
        a.needs.goal,
        Some(locked),
        "the consumed charge aim suppresses a second contact hit"
    );
    let underfoot = [[a.position[0], a.position[1] + 1.5, a.position[2]]];
    for _ in 0..13 {
        assert_eq!(
            step_observed(&mut a, &h, &underfoot, &[], 0.1),
            Effect::None,
            "the charge already hit; its complete rise must not apply knockup damage again"
        );
    }
    assert_eq!(a.behavior, B::Emerge);
    assert_eq!(
        step_observed(&mut a, &h, &underfoot, &[], 0.1),
        Effect::None
    );
    assert_eq!(a.behavior, B::Attack);
    assert!(
        a.needs.goal.is_none(),
        "the consumed charge marker ends with the rise"
    );

    let mut dodged = worm();
    dodged.behavior = B::Hunt;
    dodged.timer = CHARGE_SECONDS;
    dodged.needs.goal = Some(locked);
    let dodge = [[19.5, 1.9, 13.5]];
    for _ in 0..20 {
        assert_eq!(
            step_observed(&mut dodged, &h, &dodge, &[], 0.1),
            Effect::None
        );
        if dodged.behavior != B::Hunt {
            break;
        }
    }
    assert_eq!(dodged.behavior, B::Emerge);
    assert!(
        (dodged.position[2] - 8.5).abs() < 1e-6,
        "the warned line stays locked"
    );
}

#[test]
fn emergence_knocks_up_only_once_underfoot_and_a_sideways_dodge_avoids_it() {
    let (terrain, chunks, journal) = fixture();
    let h = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    let mut a = worm();
    a.behavior = B::Emerge;
    a.timer = EMERGE_SECONDS;
    let targets = [[8.5, 2.5, 8.5], [11.0, 2.5, 8.5]];
    let mut hits = Vec::new();
    for _ in 0..13 {
        let effect = step_observed(&mut a, &h, &targets, &[], 0.1);
        if effect != Effect::None {
            hits.push(effect);
        }
    }
    assert_eq!(
        hits,
        [Effect::StrikePlayer {
            index: 0,
            damage: KNOCKUP_DAMAGE
        }]
    );
    a = worm();
    a.behavior = B::Emerge;
    a.timer = EMERGE_SECONDS * (1.0 - KNOCKUP_PHASE) + 0.05;
    assert_eq!(
        step_observed(&mut a, &h, &targets[1..], &[], 0.1),
        Effect::None
    );
}

#[test]
fn ranged_spit_reaches_a_real_visible_enemy_beyond_the_old_eighteen_metre_limit() {
    let (terrain, chunks, journal) = fixture();
    let h = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    let mut a = worm();
    a.behavior = B::Attack;
    a.timer = EXPOSED_SECONDS * (1.0 - SPIT_PHASE) + 0.05;
    let target = [[30.5, 1.9, 30.5]];
    let Effect::SpitBile { origin, velocity } = step_observed(&mut a, &h, &target, &[], 0.1) else {
        panic!("clear distant target must receive an aimed spit");
    };
    let aim = a.needs.goal.unwrap().position.map(|v| v as f64 + 0.5);
    let horizontal = (aim[0] - origin[0]).hypot(aim[2] - origin[2]);
    assert!(horizontal > 28.0);
    let time = horizontal / velocity[0].hypot(velocity[2]);
    assert!(time < f64::from(SPIT_SECONDS));
    let at = std::array::from_fn::<_, 3, _>(|i| {
        origin[i] + velocity[i] * time
            - if i == 1 {
                0.5 * f64::from(SPIT_GRAVITY) * time * time
            } else {
                0.0
            }
    });
    assert!(at.into_iter().zip(aim).all(|(a, b)| (a - b).abs() < 1e-6));
}

#[test]
fn a_running_victim_is_intercepted_on_sand_and_can_reverse_after_the_warning() {
    let (terrain, chunks, mut journal) = fixture();
    let signal = Footfall {
        position: [19.5, 1.9, 8.5],
        velocity: [0.0, 5.0],
        prey: true,
    };
    let nearer_observer = Footfall {
        position: [12.5, 1.9, 8.5],
        prey: false,
        ..signal
    };
    let targets = [signal.position];
    let mut a = worm();
    {
        let h = Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        };
        step_sensed(
            &mut a,
            &h,
            &targets,
            [nearer_observer, signal].into_iter(),
            0.1,
        );
        assert_eq!(a.behavior, B::Tremor);
        let intercept = a.needs.goal.unwrap().position;
        assert_eq!(intercept, [19, 1, 11]);
        let reverse = Footfall {
            position: [19.5, 1.9, 5.5],
            velocity: [0.0, -6.0],
            ..signal
        };
        for _ in 0..44 {
            assert_eq!(
                step_sensed(&mut a, &h, &[reverse.position], [reverse].into_iter(), 0.1),
                Effect::None
            );
            if a.behavior == B::Emerge {
                break;
            }
        }
        assert_eq!(a.behavior, B::Emerge);
        assert!((a.position[0] - 19.5).hypot(a.position[2] - 11.5) < 1.8);
        let mut observer = worm();
        step_sensed(
            &mut observer,
            &h,
            &targets,
            [nearer_observer].into_iter(),
            0.1,
        );
        assert!(
            observer.needs.goal.is_none(),
            "quiet vulnerable bystanders must not turn creative footsteps into an aimed charge"
        );
    }
    journal
        .record([19, 0, 11], MaterialId::Stone as u16)
        .unwrap();
    let h = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    let mut blocked = worm();
    step_sensed(&mut blocked, &h, &targets, [signal].into_iter(), 0.1);
    assert_eq!(blocked.needs.goal.unwrap().position, [19, 1, 8]);
}

#[test]
fn pursuit_skirts_a_stone_obstacle_on_verified_sand() {
    let (terrain, chunks, mut journal) = fixture();
    for z in 7..=10 {
        journal
            .record([12, 0, z], MaterialId::Stone as u16)
            .unwrap();
    }
    let h = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    let target = [18.5, 1.9, 8.5];
    let mut a = worm();
    a.timer = BURROW_SECONDS;
    let mut detour = 0.0_f64;
    for _ in 0..55 {
        step_observed(&mut a, &h, &[target], &[target], 0.1);
        assert!(surface(&h, a.position).is_some());
        detour = detour.max((a.position[2] - 8.5).abs());
    }
    assert!(
        detour >= 2.0 && a.position[0] > 15.0,
        "expected a sand-only route around stone, ended at {:?}",
        a.position
    );
}

#[test]
fn wounded_worm_flanks_and_rearms_earlier_while_a_hidden_enemy_breaks_off_the_attack() {
    let (terrain, chunks, mut journal) = fixture();
    let target = [[16.5, 1.9, 8.5]];
    let mut healthy = worm();
    let mut injured = worm();
    injured.health = 24.;
    {
        let h = Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        };
        for a in [&mut healthy, &mut injured] {
            a.behavior = B::Retreat;
            a.timer = 0.05;
            step_observed(a, &h, &target, &target, 0.1);
            assert_eq!(a.behavior, B::Burrowed);
        }
        assert!(injured.timer + 1.5 < healthy.timer);
        for _ in 0..5 {
            for a in [&mut healthy, &mut injured] {
                step_observed(a, &h, &target, &target, 0.1);
            }
        }
        assert!((healthy.position[2] - 8.5).abs() < 1e-6);
        assert!((injured.position[2] - 8.5).abs() > 0.4);
    }
    for y in 1..=9 {
        for z in 5..=12 {
            journal
                .record([14, y, z], MaterialId::Stone as u16)
                .unwrap();
        }
    }
    let h = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    healthy.behavior = B::Attack;
    healthy.timer = EXPOSED_SECONDS * 0.55;
    assert_eq!(
        step_observed(&mut healthy, &h, &target, &[], 0.1),
        Effect::None
    );
    assert_eq!(healthy.behavior, B::Retreat);
    assert_eq!(healthy.timer, RETREAT_SECONDS);
}

#[test]
fn a_worm_emerging_at_range_opens_with_an_aimed_spit_while_close_prey_get_a_bite_windup() {
    let (terrain, chunks, journal) = fixture();
    let h = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    let mut a = worm();
    a.behavior = B::Emerge;
    a.timer = 0.05;
    let mut close = a.clone();
    let ranged = [[25.5, 1.9, 8.5]];
    step_observed(&mut a, &h, &ranged, &[], 0.1);
    step_observed(&mut close, &h, &[[10.0, 1.9, 8.5]], &[], 0.1);
    assert_eq!(a.behavior, B::Attack);
    assert_eq!(close.timer, EXPOSED_SECONDS);
    assert_eq!(bite_closure(phase_progress(B::Attack, a.timer)), 0.0);
    let mut elapsed = 0.0;
    while elapsed < 2.0 {
        elapsed += 0.1;
        match step_observed(&mut a, &h, &ranged, &[], 0.1) {
            Effect::None => {}
            Effect::SpitBile { .. } => break,
            other => panic!("unexpected ranged opener {other:?}"),
        }
    }
    assert!(a.behavior == B::Spit && (1.2..1.5).contains(&elapsed));
}
