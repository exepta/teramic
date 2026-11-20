use super::*;
use terra_voxel::{BlockSection, global_to_section};
use terra_world::{
    TerraGenerator,
    streaming::{ChunkStore, LoadedSection},
    water::{FluidCell, simulate_fluid_step},
};

#[test]
fn natural_ice_provenance_survives_real_region_save_and_restores_exact_source_height() {
    let root = std::env::temp_dir().join(format!("terra-seasonal-ice-{}", PlayerUuid::new_v4()));
    let world = WorldId(17231);
    let mut journal = EditJournal::default();
    for (p, amount) in [([-1, 7, -1], 8), ([16, 7, 16], 5)] {
        journal.record(p, 49).unwrap();
        journal
            .set_object_state(
                p,
                Some(terra_world::edits::BlockObjectState {
                    facing: amount,
                    ..Default::default()
                }),
            )
            .unwrap();
    }
    let built = [1, 7, 1];
    journal.record(built, 49).unwrap();
    save_edits(&root, world, &journal).unwrap();
    let mut restored = load_edits(&root, world).unwrap();
    for (p, amount) in [([-1, 7, -1], 8), ([16, 7, 16], 5)] {
        assert_eq!(restored.block(p), Some(49));
        assert_eq!(restored.object_state(p).unwrap().facing, amount);
        restored.record(p, 0).unwrap();
        assert_eq!(
            restored.fluid(p),
            Some(FluidCell {
                amount,
                source: true,
                flow: [0, 0]
            })
        );
    }
    assert_eq!(restored.block(built), Some(49));
    assert!(restored.object_state(built).is_none());
    save_edits(&root, world, &restored).unwrap();
    let thawed = load_edits(&root, world).unwrap();
    assert_eq!(thawed.fluid([-1, 7, -1]).unwrap().amount, 8);
    assert_eq!(thawed.fluid([16, 7, 16]).unwrap().amount, 5);
    assert_eq!(thawed.block(built), Some(49));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn placed_sources_and_receding_pool_water_survive_real_cross_region_save_reload() {
    let root = std::env::temp_dir().join(format!("terra-pool-sources-{}", PlayerUuid::new_v4()));
    let world = WorldId(17229);
    let terrain = TerraGenerator::new(terra_core::WorldConfig {
        id: world,
        base_height: -50.,
        sea_level: -100.,
        rivers: false,
        caves: false,
        trees: false,
        biomes: false,
        ..Default::default()
    })
    .unwrap();
    let mut chunks = ChunkStore::default();
    let mut journal = EditJournal::default();
    for x in -2..=1 {
        for y in 17..=19 {
            for z in -2..=1 {
                let p = [x, y, z];
                let (section, local) = global_to_section(p);
                let section = chunks
                    .loaded
                    .entry(section)
                    .or_insert_with(|| LoadedSection {
                        data: BlockSection::from_blocks(section, 0, vec![3; 4096]).unwrap(),
                        mesh: None,
                        mesh_epoch: 0,
                        water_epoch: 0,
                        water: None,
                        props: vec![],
                    });
                let material = if (-1..=0).contains(&x) && (-1..=0).contains(&z) && y >= 18 {
                    0
                } else {
                    3
                };
                section.data.set_block(local, material).unwrap();
                journal.record(p, material).unwrap();
            }
        }
    }
    let positions = [[-1, 18, -1], [0, 18, -1], [-1, 18, 0], [0, 18, 0]];
    for p in [positions[0], positions[3]] {
        let (section, local) = global_to_section(p);
        chunks
            .loaded
            .get_mut(&section)
            .unwrap()
            .data
            .set_block(local, 27)
            .unwrap();
        journal.record(p, 27).unwrap();
    }
    for _ in 0..32 {
        simulate_fluid_step(&terrain, &chunks, &mut journal, 256).unwrap();
    }
    save_edits(&root, world, &journal).unwrap();
    journal = load_edits(&root, world).unwrap();
    for (index, &p) in positions.iter().enumerate() {
        assert_eq!(
            journal.fluid(p),
            Some(FluidCell {
                amount: 8,
                flow: [0; 2],
                source: index == 0 || index == 3,
            })
        );
    }
    for _ in 0..32 {
        assert!(
            !simulate_fluid_step(&terrain, &chunks, &mut journal, 256)
                .unwrap()
                .changed
        );
    }
    for p in [positions[0], positions[3]] {
        let (section, local) = global_to_section(p);
        chunks
            .loaded
            .get_mut(&section)
            .unwrap()
            .data
            .set_block(local, 0)
            .unwrap();
        journal.record(p, 0).unwrap();
    }
    let first = simulate_fluid_step(&terrain, &chunks, &mut journal, 256).unwrap();
    let snapshot = positions.map(|p| journal.fluid(p).unwrap_or_default());
    let remaining: u64 = snapshot.iter().map(|c| u64::from(c.amount)).sum();
    assert!(remaining > 0 && first.drained > 0);
    assert_eq!(remaining + first.drained, 16);
    save_edits(&root, world, &journal).unwrap();
    journal = load_edits(&root, world).unwrap();
    assert_eq!(
        positions.map(|p| journal.fluid(p).unwrap_or_default()),
        snapshot
    );
    let mut drained = 0;
    for _ in 0..32 {
        let step = simulate_fluid_step(&terrain, &chunks, &mut journal, 256).unwrap();
        assert_eq!(step.sourced + step.replenished, 0);
        drained += step.drained;
    }
    assert_eq!(drained, remaining);
    assert_eq!(
        positions.map(|p| journal.fluid(p).unwrap_or_default()),
        [FluidCell::default(); 4]
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn source_free_pools_of_other_sizes_finish_draining_and_stay_quiet_after_real_save_reload() {
    for (size, total) in [([1_i64, 1], 5_u16), ([3, 3], 21), ([4, 3], 37)] {
        let root = std::env::temp_dir().join(format!("terra-finite-rest-{}", PlayerUuid::new_v4()));
        let world = WorldId(17230);
        let terrain = TerraGenerator::new(terra_core::WorldConfig {
            id: world,
            base_height: -50.,
            sea_level: -100.,
            rivers: false,
            caves: false,
            trees: false,
            biomes: false,
            ..Default::default()
        })
        .unwrap();
        let origin = [-1, 18, 15];
        let mut chunks = ChunkStore::default();
        let mut journal = EditJournal::default();
        for x in origin[0] - 1..=origin[0] + size[0] {
            for y in 17..=19 {
                for z in origin[2] - 1..=origin[2] + size[1] {
                    let p = [x, y, z];
                    let (section, local) = global_to_section(p);
                    let section = chunks
                        .loaded
                        .entry(section)
                        .or_insert_with(|| LoadedSection {
                            data: BlockSection::from_blocks(section, 0, vec![3; 4096]).unwrap(),
                            mesh: None,
                            mesh_epoch: 0,
                            water_epoch: 0,
                            water: None,
                            props: vec![],
                        });
                    let inside = x >= origin[0]
                        && x < origin[0] + size[0]
                        && z >= origin[2]
                        && z < origin[2] + size[1];
                    let material = if inside && y >= 18 { 0 } else { 3 };
                    section.data.set_block(local, material).unwrap();
                    journal.record(p, material).unwrap();
                }
            }
        }
        let mut positions = vec![];
        let mut left = total;
        for z in 0..size[1] {
            for x in 0..size[0] {
                let p = [origin[0] + x, 18, origin[2] + z];
                positions.push(p);
                let amount = left.min(8) as u8;
                left -= u16::from(amount);
                if amount > 0 {
                    journal
                        .set_fluid(
                            p,
                            FluidCell {
                                amount,
                                flow: [1, 0],
                                source: false,
                            },
                        )
                        .unwrap();
                }
            }
        }
        let initial = simulate_fluid_step(&terrain, &chunks, &mut journal, 256).unwrap();
        let snapshot: Vec<_> = positions.iter().map(|&p| journal.fluid(p)).collect();
        save_edits(&root, world, &journal).unwrap();
        journal = load_edits(&root, world).unwrap();
        assert_eq!(
            positions
                .iter()
                .map(|&p| journal.fluid(p))
                .collect::<Vec<_>>(),
            snapshot
        );
        let mut drained = initial.drained;
        for _ in 0..32 {
            let step = simulate_fluid_step(&terrain, &chunks, &mut journal, 256).unwrap();
            assert_eq!(step.sourced + step.replenished, 0);
            drained += step.drained;
        }
        assert_eq!(drained, u64::from(total));
        let epochs: Vec<_> = journal
            .changed_sections()
            .map(|section| {
                (
                    section,
                    journal.fluid_epoch(section),
                    journal.water_mesh_epoch(section),
                )
            })
            .collect();
        for _ in 0..64 {
            let step = simulate_fluid_step(&terrain, &chunks, &mut journal, 256).unwrap();
            assert!(!step.changed);
            assert_eq!(step.sourced + step.replenished + step.drained, 0);
        }
        for p in positions {
            assert_eq!(journal.fluid(p).unwrap_or_default(), FluidCell::default());
        }
        for (section, fluid, mesh) in epochs {
            assert_eq!(journal.fluid_epoch(section), fluid);
            assert_eq!(journal.water_mesh_epoch(section), mesh);
        }
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn waterlogged_pipe_sources_and_finite_pipe_exteriors_keep_exact_cells_on_real_save_reload() {
    use terra_core::plumbing;
    use terra_world::edits::BlockObjectState;
    let root =
        std::env::temp_dir().join(format!("terra-waterlogged-pipes-{}", PlayerUuid::new_v4()));
    let world = WorldId(17329);
    let pipes = [
        plumbing::WOOD_PIPE,
        plumbing::STONE_PIPE,
        plumbing::IRON_PIPE,
        plumbing::GLASS_PIPE,
    ];
    let positions = [[-1, 18, -1], [0, 18, -1], [-1, 18, 0], [0, 18, 0]];
    let terrain = TerraGenerator::new(terra_core::WorldConfig {
        id: world,
        base_height: -100.,
        sea_level: -200.,
        rivers: false,
        caves: false,
        trees: false,
        biomes: false,
        ..Default::default()
    })
    .unwrap();
    let mut chunks = ChunkStore::default();
    let mut journal = EditJournal::default();
    for (p, material) in positions.into_iter().zip(pipes) {
        let (owner, local) = global_to_section(p);
        chunks
            .loaded
            .entry(owner)
            .or_insert_with(|| LoadedSection {
                data: BlockSection::from_blocks(owner, 0, vec![3; 4096]).unwrap(),
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: vec![],
            })
            .data
            .set_block(local, material)
            .unwrap();
        journal.record(p, 27).unwrap();
        journal.record(p, material).unwrap();
        journal
            .set_object_state(
                p,
                Some(BlockObjectState {
                    facing: 4,
                    ..Default::default()
                }),
            )
            .unwrap();
    }
    for source in [true, false] {
        if !source {
            for p in positions {
                journal
                    .set_fluid(
                        p,
                        FluidCell {
                            amount: 6,
                            source: false,
                            flow: [0; 2],
                        },
                    )
                    .unwrap();
            }
        }
        save_edits(&root, world, &journal).unwrap();
        journal = load_edits(&root, world).unwrap();
        for (p, material) in positions.into_iter().zip(pipes) {
            assert_eq!(journal.block(p), Some(material));
            let water = FluidCell {
                amount: if source { 8 } else { 6 },
                source,
                flow: [0; 2],
            };
            assert_eq!(journal.fluid(p), Some(water));
            assert_eq!(chunks.fluid_cell_at(&terrain, &journal, p, material), water);
            assert_eq!(journal.object_state(p).unwrap().facing, 4);
            assert!(
                journal.plumbing_at(p).is_none(),
                "external water never becomes internal pipe storage"
            );
        }
        if source {
            for _ in 0..32 {
                assert!(
                    !simulate_fluid_step(&terrain, &chunks, &mut journal, 256)
                        .unwrap()
                        .changed
                );
            }
        }
    }
    fs::remove_dir_all(root).unwrap();
}
