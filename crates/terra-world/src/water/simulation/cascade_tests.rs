use super::*;
use crate::{SectionGenerator, streaming::LoadedSection};
use terra_core::WorldConfig;
use terra_voxel::SectionPos;

#[test]
fn a_loaded_tall_waterfall_reaches_its_real_floor_with_progressive_bounded_steps() {
    let terrain = TerraGenerator::new(WorldConfig {
        base_height: -20.0,
        sea_level: -30.0,
        hill_amplitude: 0.0,
        mountain_amplitude: 0.0,
        caves: false,
        biomes: false,
        trees: false,
        rivers: false,
        water: true,
        ..Default::default()
    })
    .unwrap();
    let mut chunks = ChunkStore::default();
    for y in 0..=2 {
        let p = SectionPos::new(0, y, 0);
        chunks.loaded.insert(
            p,
            LoadedSection {
                data: terra_voxel::BlockSection::empty(p).unwrap(),
                mesh: None,
                water: None,
                props: vec![],
                mesh_epoch: 0,
                water_epoch: 0,
            },
        );
    }
    chunks
        .loaded
        .get_mut(&SectionPos::default())
        .unwrap()
        .data
        .set_block([8, 0, 8], 3)
        .unwrap();
    let mut journal = EditJournal::default();
    journal
        .set_fluid(
            [8, 40, 8],
            FluidCell {
                amount: 8,
                source: true,
                flow: [0; 2],
            },
        )
        .unwrap();
    journal.wake_fluid_neighbors([8, 40, 8]);
    let step = simulate_fluid_step(&terrain, &chunks, &mut journal, 64).unwrap();
    assert!(step.visited <= 64);
    assert!(
        journal.fluid([8, 1, 8]).is_none(),
        "a forty-cell fall needs travel time"
    );
    for _ in 0..400 {
        let step = simulate_fluid_step(&terrain, &chunks, &mut journal, 64).unwrap();
        assert!(step.visited <= 64);
    }
    for y in 1..40 {
        assert!(
            journal
                .fluid([8, y, 8])
                .is_some_and(|c| c.amount > 0 && !c.source),
            "a loaded gravity column must meet its floor, missing y={y}"
        );
    }
    assert!(journal.fluid([8, 0, 8]).is_none());
    assert_eq!(journal.fluid_cells().filter(|(_, c)| c.source).count(), 1);

    // Removing a middle section must suspend work at its real boundary. When
    // it arrives, the same priority path joins both vertical seams to the floor.
    let middle = SectionPos::new(0, 1, 0);
    let middle_data = chunks.loaded.remove(&middle).unwrap();
    let mut journal = EditJournal::default();
    journal
        .set_fluid(
            [8, 40, 8],
            FluidCell {
                amount: 8,
                source: true,
                flow: [0; 2],
            },
        )
        .unwrap();
    journal.wake_fluid_neighbors([8, 40, 8]);
    for _ in 0..100 {
        let step = simulate_fluid_step(&terrain, &chunks, &mut journal, 64).unwrap();
        assert!(step.visited <= 64);
    }
    assert!(journal.fluid_cells().all(|(p, _)| p[1] >= 32));
    assert!(journal.fluid_work_counts().sleeping > 0);
    chunks.loaded.insert(middle, middle_data);
    journal.resume_fluids(middle);
    for _ in 0..400 {
        let step = simulate_fluid_step(&terrain, &chunks, &mut journal, 64).unwrap();
        assert!(step.visited <= 64);
    }
    assert!(journal.fluid([8, 1, 8]).is_some_and(|c| c.amount > 0));
}

#[test]
fn stepped_waterfall_crosses_horizontal_and_vertical_sections_partial_outlet_and_recedes_after_closing()
 {
    let terrain = TerraGenerator::new(WorldConfig {
        base_height: -20.0,
        sea_level: -30.0,
        hill_amplitude: 0.0,
        mountain_amplitude: 0.0,
        caves: false,
        biomes: false,
        trees: false,
        rivers: false,
        water: true,
        ..Default::default()
    })
    .unwrap();
    let mut chunks = ChunkStore::default();
    for x in 0..=1 {
        for y in 0..=1 {
            let p = SectionPos::new(x, y, 0);
            chunks.loaded.insert(
                p,
                LoadedSection {
                    data: terra_voxel::BlockSection::empty(p).unwrap(),
                    mesh: None,
                    water: None,
                    props: vec![],
                    mesh_epoch: 0,
                    water_epoch: 0,
                },
            );
        }
    }
    let mut put = |p: [i64; 3], material| {
        let (owner, local) = global_to_section(p);
        chunks
            .loaded
            .get_mut(&owner)
            .unwrap()
            .data
            .set_block(local, material)
            .unwrap();
    };
    for x in 12..=23 {
        for z in 4..=10 {
            put([x, 0, z], 3);
            for y in 1..=23 {
                if x == 12 || x == 23 || z == 4 || z == 10 {
                    put([x, y, z], 3);
                }
            }
        }
    }
    for x in 13..=16 {
        for z in 5..=9 {
            put([x, 20, z], 3);
        }
    }
    for z in 5..=9 {
        for y in 21..=23 {
            if z != 7 || y != 21 {
                put([16, y, z], 3);
            }
        }
    }
    for x in 17..=19 {
        for z in 6..=8 {
            put([x, 10, z], 3);
        }
    }
    // Back the middle terrace with an actual rim. A raised half-block outlet
    // needs real upstream depth; thin flow on an unconfined ledge can correctly
    // bypass it instead of manufacturing a full-height water shelf.
    for x in 16..=20 {
        for z in 5..=9 {
            if z == 5 || z == 9 || x == 16 || x == 20 && z != 7 {
                put([x, 11, z], 3);
            }
        }
    }
    put([20, 11, 7], 82);
    put([18, 11, 6], 74);
    put([15, 21, 7], 27);
    let mut journal = EditJournal::default();
    journal
        .set_fluid(
            [15, 21, 7],
            FluidCell {
                amount: 8,
                source: true,
                flow: [0; 2],
            },
        )
        .unwrap();
    journal.wake_fluid_neighbors([15, 21, 7]);
    for _ in 0..480 {
        let step = simulate_fluid_step(&terrain, &chunks, &mut journal, 256).unwrap();
        assert!(step.visited <= 256);
    }
    assert!(
        journal
            .fluid([17, 16, 7])
            .is_some_and(|cell| cell.amount > 0 && !cell.source),
        "the upper fall must physically cross the vertical section plane"
    );
    assert!(
        journal
            .fluid([20, 11, 7])
            .is_some_and(|cell| cell.amount > 4 && !cell.source),
        "the middle terrace must feed the real open half of the slab"
    );
    assert!(
        journal
            .fluid_cells()
            .any(|(p, c)| p[0] >= 21 && p[1] < 5 && c.amount > 0 && !c.source),
        "water must leave the stepped terrace and reach the lower basin"
    );
    assert_eq!(journal.fluid_cells().filter(|(_, c)| c.source).count(), 1);
    let below = journal
        .fluid_cells()
        .filter(|(p, _)| p[0] >= 17)
        .map(|(_, c)| u64::from(c.amount))
        .sum::<u64>();
    assert!(below > 100);
    let (owner, local) = global_to_section([16, 21, 7]);
    chunks
        .loaded
        .get_mut(&owner)
        .unwrap()
        .data
        .set_block(local, 3)
        .unwrap();
    journal.record([16, 21, 7], 3).unwrap();
    journal.wake_fluid_neighbors([16, 21, 7]);
    let retained = journal
        .fluid_cells()
        .map(|(_, c)| u64::from(c.amount))
        .sum::<u64>();
    let (mut injected, mut removed) = (0, 0);
    for _ in 0..240 {
        let step = simulate_fluid_step(&terrain, &chunks, &mut journal, 256).unwrap();
        injected += step.sourced + step.replenished;
        removed += step.drained + step.absorbed;
        assert_eq!(
            journal
                .fluid_cells()
                .map(|(_, c)| u64::from(c.amount))
                .sum::<u64>()
                + removed,
            retained + injected
        );
    }
    let supported_middle = |p: [i64; 3]| {
        p[1] == 11 && (((17..=19).contains(&p[0]) && (6..=8).contains(&p[2])) || p == [20, 11, 7])
    };
    let supported_above = |p: [i64; 3]| {
        let below = [p[0], p[1] - 1, p[2]];
        let Some(material) = chunks.block_at(below) else {
            return false;
        };
        let full_water = chunks
            .fluid_cell_at(&terrain, &journal, below, material)
            .amount
            == 8;
        let floor = super::super::space::FluidSpace::at(&journal, below, |q| chunks.block_at(q));
        let solid_top = !floor
            .wet_boxes(1.)
            .iter()
            .any(|void| void.maximum[1] > 1. - 1e-8);
        full_water || solid_top
    };
    let unsupported: Vec<_> = journal
        .fluid_cells()
        .filter(|(p, c)| {
            p[0] >= 17 && p[1] > 1 && c.amount > 0 && !supported_middle(*p) && !supported_above(*p)
        })
        .collect();
    assert!(
        unsupported.is_empty(),
        "source cutoff leaves actual unsupported jets: {unsupported:?}"
    );
    assert!(
        journal
            .fluid_cells()
            .all(|(p, c)| p[0] < 17 || c.amount == 0),
        "Neither terraces nor lower basin may sustain the disconnected stream"
    );
    let bottom: Vec<_> = journal.fluid_cells().filter(|(p, _)| p[1] == 1).collect();
    assert!(bottom.iter().all(|(_, c)| c.amount == 0));
    assert!(bottom.iter().all(|(_, c)| !c.source && c.flow == [0; 2]));
    // The fifty-cell enclosed bottom basin must stay dry and quiet after the
    // waterfall's supply is cut, even though its floor could hold a puddle.
    let epochs: Vec<_> = [SectionPos::new(0, 0, 0), SectionPos::new(1, 0, 0)]
        .into_iter()
        .map(|p| (p, journal.fluid_epoch(p), journal.water_mesh_epoch(p)))
        .collect();
    let snapshot = |journal: &EditJournal| {
        journal
            .fluid_cells()
            .collect::<std::collections::BTreeMap<_, _>>()
    };
    let mut previous = snapshot(&journal);
    let mut quiet_diagnostics = Vec::new();
    for index in 0..64 {
        let step = simulate_fluid_step(&terrain, &chunks, &mut journal, 256).unwrap();
        if index < 2 {
            let after = snapshot(&journal);
            let positions: std::collections::BTreeSet<_> =
                previous.keys().chain(after.keys()).copied().collect();
            let changes: Vec<_> = positions
                .into_iter()
                .filter_map(|p| {
                    let before = previous.get(&p).copied();
                    let next = after.get(&p).copied();
                    (before != next).then_some((p, before, next))
                })
                .collect();
            quiet_diagnostics.push((
                index,
                (
                    step.visited,
                    step.sourced,
                    step.drained,
                    step.absorbed,
                    step.transferred,
                    step.changed,
                ),
                changes,
            ));
            previous = after;
        }
    }
    assert_eq!(
        journal
            .fluid_cells()
            .filter(|(p, _)| p[1] == 1)
            .collect::<Vec<_>>(),
        bottom
    );
    for (p, fluid, water) in epochs {
        assert_eq!(
            journal.fluid_epoch(p),
            fluid,
            "quiet steps(index,stats[visited,sourced,drained,absorbed,transferred,changed],state changes)={quiet_diagnostics:?}"
        );
        assert_eq!(
            journal.water_mesh_epoch(p),
            water,
            "quiet steps(index,stats[visited,sourced,drained,absorbed,transferred,changed],state changes)={quiet_diagnostics:?}"
        );
    }
    assert_eq!(journal.fluid([15, 21, 7]).unwrap().amount, 8);
    assert_eq!(journal.fluid_cells().filter(|(_, c)| c.source).count(), 1);
}

#[test]
fn generated_cascade_uses_real_gravity_cells_and_survives_delta_reload() {
    let mut config = WorldConfig::default();
    config.cave_biomes.distribution = terra_core::config::CaveDistribution {
        normal_weight: 0.0,
        salt_weight: 0.0,
        lush_weight: 0.0,
        cascade_weight: 1.0,
    };
    let terrain = TerraGenerator::new(config).unwrap();
    let mut outlet = None;
    'find: for z in -4..4 {
        for x in -4..4 {
            let Some(center) =
                terrain.cave_chamber_center(x as f64 * 192.0 + 96.0, z as f64 * 192.0 + 96.0)
            else {
                continue;
            };
            for dz in -24..24 {
                for dx in -60..60 {
                    let (px, pz) = (
                        (center[0] + dx as f64).floor() + 0.5,
                        (center[2] + dz as f64).floor() + 0.5,
                    );
                    let room = terrain.cave_structure_at(px, pz);
                    if let Some(p) = room.fluid_outlet(px, pz) {
                        outlet = Some(p);
                        break 'find;
                    }
                }
            }
        }
    }
    let outlet = outlet.expect("cascade needs a real source lip");
    let owner = global_to_section(outlet).0;
    let generator = SectionGenerator::new(terrain.clone());
    let mut chunks = ChunkStore::default();
    let mut journal = EditJournal::default();
    for z in owner.z - 1..=owner.z + 1 {
        for x in owner.x - 1..=owner.x + 1 {
            let prepared = generator.prepare_column([x, z], false).unwrap();
            for y in owner.y - 2..=owner.y + 1 {
                let p = SectionPos::new(x, y, z);
                let generated = generator
                    .build_prepared(p, 0, &Default::default(), &prepared)
                    .unwrap();
                for cell in generated.fluid_wake {
                    journal.wake_fluid_neighbors(cell);
                }
                chunks.loaded.insert(
                    p,
                    LoadedSection {
                        data: generated.data,
                        mesh: None,
                        water: None,
                        props: generated.props,
                        mesh_epoch: 0,
                        water_epoch: 0,
                    },
                );
                journal.resume_fluids(p);
            }
        }
    }
    assert_eq!(chunks.block_at(outlet), Some(0));
    assert_eq!(
        super::super::fluid_cell_at(&terrain, &journal, outlet, 0).amount,
        0
    );
    for _ in 0..300 {
        let step = simulate_fluid_step(&terrain, &chunks, &mut journal, 256).unwrap();
        assert!(step.visited <= 256);
    }
    let wet: Vec<_> = journal
        .fluid_cells()
        .filter(|(_, c)| c.amount > 0)
        .collect();
    assert!(
        wet.iter().any(|(p, c)| p[1] < outlet[1] - 4 && !c.source),
        "no falling cells: {outlet:?}"
    );
    assert!(
        wet.iter().all(|(_, c)| !c.source),
        "flow created a new source"
    );
    assert!(wet.iter().all(|(p, _)| {
        chunks
            .block_at(*p)
            .is_some_and(super::super::can_hold_fluid)
    }));
    let mut reloaded = EditJournal::default();
    for delta in journal.changed_sections().map(|s| journal.section_delta(s)) {
        reloaded.import_delta(delta).unwrap();
    }
    for (p, cell) in wet {
        assert_eq!(reloaded.fluid(p), Some(cell));
    }
}
