use super::*;
use crate::{SectionGenerator, streaming::LoadedSection};
use terra_core::WorldConfig;
use terra_voxel::SectionPos;

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
