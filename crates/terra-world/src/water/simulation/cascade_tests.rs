use super::*;
use crate::{SectionGenerator, streaming::LoadedSection};
use terra_core::WorldConfig;
use terra_voxel::SectionPos;

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
    for _ in 0..240 {
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
    for _ in 0..240 {
        simulate_fluid_step(&terrain, &chunks, &mut journal, 256).unwrap();
    }
    assert!(
        journal
            .fluid_cells()
            .filter(|(p, _)| p[0] >= 17 && p[1] > 1)
            .all(|(_, c)| c.amount == 0),
        "an unsupported waterfall cannot remain after its actual upstream gate closes"
    );
    let bottom: Vec<_> = journal.fluid_cells().filter(|(p, _)| p[1] == 1).collect();
    assert!(bottom.iter().any(|(_, c)| c.amount > 0));
    assert!(bottom.iter().all(|(_, c)| !c.source && c.flow == [0; 2]));
    // The fifty-cell bottom basin has a complete floor and four enclosing
    // walls; closing the waterfall must retain its finite settled water.
    let epochs: Vec<_> = [SectionPos::new(0, 0, 0), SectionPos::new(1, 0, 0)]
        .into_iter()
        .map(|p| (p, journal.fluid_epoch(p), journal.water_mesh_epoch(p)))
        .collect();
    for _ in 0..64 {
        simulate_fluid_step(&terrain, &chunks, &mut journal, 256).unwrap();
    }
    assert_eq!(
        journal
            .fluid_cells()
            .filter(|(p, _)| p[1] == 1)
            .collect::<Vec<_>>(),
        bottom
    );
    for (p, fluid, water) in epochs {
        assert_eq!(journal.fluid_epoch(p), fluid);
        assert_eq!(journal.water_mesh_epoch(p), water);
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
