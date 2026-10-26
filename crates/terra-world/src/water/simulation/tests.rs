use super::*;
use crate::{
    streaming::{BlockEditRequest, LoadedSection, commit_block_edit as commit_with_properties},
    water::fluid_cell_at,
};
use terra_core::WorldConfig;
use terra_voxel::{BlockSection, SectionPos};

fn commit_block_edit(
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
    request: &BlockEditRequest,
) -> Result<bool, String> {
    commit_with_properties(chunks, journal, request, &Default::default())
}

fn terrain() -> TerraGenerator {
    TerraGenerator::new(WorldConfig {
        base_height: 5.0,
        sea_level: 8.0,
        hill_amplitude: 0.0,
        mountain_amplitude: 0.0,
        caves: false,
        trees: false,
        biomes: false,
        rivers: false,
        ..Default::default()
    })
    .unwrap()
}
fn section(p: SectionPos, fill: u16) -> LoadedSection {
    LoadedSection {
        data: BlockSection::from_blocks(p, 0, vec![fill; 4096]).unwrap(),
        mesh: None,
        mesh_epoch: 0,
        water_epoch: 0,
        water: None,
        props: Vec::new(),
    }
}
fn block(chunks: &mut ChunkStore, p: [i64; 3], id: u16) {
    let (owner, local) = global_to_section(p);
    chunks
        .loaded
        .get_mut(&owner)
        .unwrap()
        .data
        .set_block(local, id)
        .unwrap();
}
fn amount(journal: &EditJournal) -> u64 {
    journal
        .fluid_cells()
        .map(|(_, c)| u64::from(c.amount))
        .sum()
}

#[test]
fn placed_fluid_water_source_flows_and_remains_removable_after_reload() {
    use terra_voxel::MaterialId;
    let terrain = terrain();
    let mut chunks = ChunkStore::default();
    let owner = SectionPos::new(-1, 1, -1);
    chunks.loaded.insert(owner, section(owner, 0));
    for x in -16..0 {
        for z in -16..0 {
            block(&mut chunks, [x, 16, z], 3);
        }
    }
    let p = [-8, 19, -8];
    let mut journal = EditJournal::default();
    assert!(
        commit_block_edit(
            &mut chunks,
            &mut journal,
            &BlockEditRequest {
                position: p,
                expected_material: 0,
                material: MaterialId::FluidWater as u16
            }
        )
        .unwrap()
    );
    assert_eq!(journal.fluid(p).unwrap().amount, FLUID_CAPACITY);
    assert!(journal.fluid(p).unwrap().source);
    for _ in 0..40 {
        simulate_fluid_step(&terrain, &chunks, &mut journal, 64).unwrap();
    }
    assert!(
        journal
            .fluid_cells()
            .any(|(q, c)| q != p && c.amount > 0 && !c.source)
    );
    let source = chunks.water_column_at(&terrain, &journal, p).unwrap();
    assert!(source.level >= 20.0);
    let mut reloaded = EditJournal::default();
    for delta in journal.changed_sections().map(|s| journal.section_delta(s)) {
        reloaded.import_delta(delta).unwrap();
    }
    assert!(reloaded.fluid(p).unwrap().source);
    assert!(
        commit_block_edit(
            &mut chunks,
            &mut reloaded,
            &BlockEditRequest {
                position: p,
                expected_material: MaterialId::FluidWater as u16,
                material: 0
            }
        )
        .unwrap()
    );
    assert!(
        reloaded
            .fluid(p)
            .is_some_and(|c| c.amount == 0 && !c.source)
    );
    assert!(reloaded.fluid_cells().all(|(_, c)| !c.source));
    let solid = [-4, 18, -4];
    commit_block_edit(
        &mut chunks,
        &mut reloaded,
        &BlockEditRequest {
            position: solid,
            expected_material: 0,
            material: MaterialId::SolidWater as u16,
        },
    )
    .unwrap();
    assert_eq!(
        super::super::fluid_cell_at(&terrain, &reloaded, solid, MaterialId::SolidWater as u16)
            .amount,
        0
    );
}

#[test]
fn a_solid_block_displaces_the_source_and_persists_without_hidden_liquid() {
    let terrain = terrain();
    let owner = SectionPos::new(-1, 1, -1);
    let p = [-8, 19, -8];
    let mut chunks = ChunkStore::default();
    chunks.loaded.insert(owner, section(owner, 0));
    let mut journal = EditJournal::default();
    for (expected_material, material) in [(0, 27), (27, 3)] {
        assert!(
            commit_block_edit(
                &mut chunks,
                &mut journal,
                &BlockEditRequest {
                    position: p,
                    expected_material,
                    material
                }
            )
            .unwrap()
        );
    }
    for _ in 0..12 {
        simulate_fluid_step(&terrain, &chunks, &mut journal, 64).unwrap();
    }
    assert_eq!(chunks.block_at(p), Some(3));
    assert!(journal.fluid(p).is_none());
    assert!(journal.fluid_cells().all(|(_, cell)| !cell.source));
    let mut reloaded = EditJournal::default();
    reloaded.import_delta(journal.section_delta(owner)).unwrap();
    assert!(reloaded.fluid(p).is_none());
    assert!(
        reloaded
            .section_delta(owner)
            .blocks
            .contains(&([8, 3, 8], 3))
    );
}

#[test]
fn a_high_source_has_a_continuous_column_without_floating_shelves_and_then_drains() {
    let terrain = terrain();
    let owner = SectionPos::new(-1, 1, -1);
    let mut chunks = ChunkStore::default();
    chunks.loaded.insert(owner, section(owner, 0));
    for x in -16..0 {
        for z in -16..0 {
            block(&mut chunks, [x, 16, z], 3);
        }
    }
    let source = [-8, 30, -8];
    let mut journal = EditJournal::default();
    commit_block_edit(
        &mut chunks,
        &mut journal,
        &BlockEditRequest {
            position: source,
            expected_material: 0,
            material: 27,
        },
    )
    .unwrap();
    for _ in 0..100 {
        simulate_fluid_step(&terrain, &chunks, &mut journal, 256).unwrap();
    }
    assert!(journal.fluid([-8, 17, -8]).unwrap().amount > 0);
    for y in 18..=30 {
        assert_eq!(
            journal.fluid([-8, y, -8]).unwrap().amount,
            8,
            "gap at y={y}"
        );
    }
    assert!(
        journal
            .fluid_cells()
            .all(|(p, _)| p[1] == 17 || (p[0] == -8 && p[2] == -8))
    );
    assert!(journal.fluid_cells().any(|(p, _)| p[0] != -8));
    let before = amount(&journal);
    commit_block_edit(
        &mut chunks,
        &mut journal,
        &BlockEditRequest {
            position: source,
            expected_material: 27,
            material: 3,
        },
    )
    .unwrap();
    assert!(
        amount(&journal) > 0,
        "removal must recede, not instantly erase downstream water"
    );
    let mut drained = 0;
    for _ in 0..400 {
        let step = simulate_fluid_step(&terrain, &chunks, &mut journal, 256).unwrap();
        assert_eq!(step.sourced, 0);
        drained += step.drained + step.absorbed;
    }
    assert_eq!(amount(&journal), 0);
    assert_eq!(drained, before - 8);
    let mut restored = EditJournal::default();
    restored.import_delta(journal.section_delta(owner)).unwrap();
    assert_eq!(amount(&restored), 0);
}

#[test]
fn flowing_water_beside_partial_blocks_drains_when_its_source_is_blocked_across_a_section_edge() {
    use crate::edits::BlockObjectState;

    let terrain = terrain();
    for owner in [SectionPos::new(0, 1, 0), SectionPos::new(-1, 1, -1)] {
        for (material, facing) in [(74, 0), (74, 4), (82, 0), (82, 5), (82, 10)] {
            let origin = owner.origin().unwrap();
            let mut chunks = ChunkStore::default();
            for dx in 0..=1 {
                for dz in -1..=1 {
                    let section_position = owner.offset([dx, 0, dz]).unwrap();
                    chunks
                        .loaded
                        .insert(section_position, section(section_position, 0));
                    let base = section_position.origin().unwrap();
                    for x in 0..16 {
                        for z in 0..16 {
                            block(&mut chunks, [base[0] + x, base[1], base[2] + z], 3);
                        }
                    }
                }
            }
            let source = [origin[0] + 15, origin[1] + 7, origin[2] + 8];
            let beside = [source[0] + 1, origin[1] + 2, source[2]];
            let mut journal = EditJournal::default();
            assert!(
                commit_block_edit(
                    &mut chunks,
                    &mut journal,
                    &BlockEditRequest {
                        position: beside,
                        expected_material: 0,
                        material,
                    }
                )
                .unwrap()
            );
            journal
                .set_object_state(
                    beside,
                    Some(BlockObjectState {
                        facing,
                        ..Default::default()
                    }),
                )
                .unwrap();
            assert!(
                commit_block_edit(
                    &mut chunks,
                    &mut journal,
                    &BlockEditRequest {
                        position: source,
                        expected_material: 0,
                        material: 27,
                    }
                )
                .unwrap()
            );
            for _ in 0..100 {
                simulate_fluid_step(&terrain, &chunks, &mut journal, 512).unwrap();
            }
            let curtain = [source[0], beside[1], source[2]];
            let cell = journal
                .fluid(curtain)
                .expect("gravity flow must remain next to the shape");
            assert_eq!(cell.amount, 8);
            assert!(!cell.source);
            assert_eq!(
                fluid_cell_at(&terrain, &journal, beside, material).amount,
                0,
                "fix must not put hidden water inside the solid construction"
            );
            assert!(
                commit_block_edit(
                    &mut chunks,
                    &mut journal,
                    &BlockEditRequest {
                        position: source,
                        expected_material: 27,
                        material,
                    }
                )
                .unwrap()
            );
            for _ in 0..400 {
                simulate_fluid_step(&terrain, &chunks, &mut journal, 512).unwrap();
            }
            assert_eq!(
                amount(&journal),
                0,
                "remaining fluid beside material={material} facing={facing} owner={owner:?}"
            );
            let mut reloaded = EditJournal::default();
            for position in journal.changed_sections().collect::<Vec<_>>() {
                reloaded
                    .import_delta(journal.section_delta(position))
                    .unwrap();
            }
            assert_eq!(amount(&reloaded), 0);
            assert_eq!(fluid_cell_at(&terrain, &reloaded, curtain, 0).amount, 0);
        }
    }
}

#[test]
fn flowing_water_removes_only_contacted_props_and_persists_the_removal() {
    use crate::props::{PropId, PropKind, WorldProp};
    let terrain = terrain();
    let owner = SectionPos::new(-1, 1, -1);
    let mut chunks = ChunkStore::default();
    chunks.loaded.insert(owner, section(owner, 0));
    let make_prop = |id, x| WorldProp {
        id: PropId(id),
        kind: PropKind::Grass,
        position: [x, 17.0, -7.5],
        rotation: 0.0,
        scale: 1.0,
        variant: 0,
        section: owner,
        removed: false,
    };
    chunks.loaded.get_mut(&owner).unwrap().props = vec![make_prop(1, -7.5), make_prop(2, -3.5)];
    for x in -16..0 {
        for z in -16..0 {
            block(&mut chunks, [x, 16, z], 3);
        }
    }
    let mut journal = EditJournal::default();
    journal
        .set_fluid(
            [-8, 17, -8],
            FluidCell {
                amount: 8,
                flow: [1, 0],
                source: false,
            },
        )
        .unwrap();
    let step = simulate_fluid_step(&terrain, &chunks, &mut journal, 1).unwrap();
    assert_eq!(step.removed_props, 1);
    assert!(journal.prop_removed(owner, PropId(1)));
    assert!(!journal.prop_removed(owner, PropId(2)));
    assert_eq!(journal.revision(owner), 0);
    assert_eq!(journal.mesh_epoch(owner), 0);
    let delta = journal.section_delta(owner);
    let mut reloaded = EditJournal::default();
    reloaded.import_delta(delta).unwrap();
    let mut plants = chunks.loaded.get(&owner).unwrap().props.clone();
    reloaded.apply_prop_removals(owner, &mut plants);
    assert!(plants[0].removed);
    assert!(!plants[1].removed);
    assert_eq!(
        simulate_fluid_step(&terrain, &chunks, &mut journal, 0)
            .unwrap()
            .removed_props,
        0
    );
}

#[test]
fn flowing_water_keeps_every_aquatic_prop_and_existing_reeds_but_removes_land_plants() {
    use crate::props::{PropId, PropKind, WorldProp};
    let terrain = terrain();
    let owner = SectionPos::new(-1, 1, -1);
    let mut chunks = ChunkStore::default();
    chunks.loaded.insert(owner, section(owner, 0));
    for x in -16..0 {
        for z in -16..0 {
            block(&mut chunks, [x, 16, z], 3);
        }
    }
    let original: Vec<_> = PropKind::ALL
        .into_iter()
        .enumerate()
        .map(|(index, kind)| {
            let id = PropId(index as u64 + 100);
            if kind == PropKind::SpiderWeb {
                return crate::props::webs::attached([-8, 16, -8], [0, 1, 0], 5, id).unwrap();
            }
            WorldProp {
                id,
                kind,
                position: [-7.5, 17.0, -7.5],
                rotation: 0.0,
                scale: 1.0,
                variant: 0,
                section: owner,
                removed: false,
            }
        })
        .collect();
    assert!(original.iter().all(WorldProp::valid));
    let web = original
        .iter()
        .find(|p| p.kind == PropKind::SpiderWeb)
        .unwrap();
    assert!(crate::props::webs::fits(web, |p| chunks.block_at(p)));
    chunks.loaded.get_mut(&owner).unwrap().props = original.clone();
    let mut journal = EditJournal::default();
    journal
        .set_fluid(
            [-8, 17, -8],
            FluidCell {
                amount: 8,
                flow: [1, 0],
                source: false,
            },
        )
        .unwrap();
    let step = simulate_fluid_step(&terrain, &chunks, &mut journal, 1).unwrap();
    let land_count = original.iter().filter(|p| !p.water_tolerant()).count();
    assert_eq!(step.removed_props, land_count);
    assert!(
        journal.prop_removed(owner, web.id),
        "flow must remove the anchored floor web"
    );
    for prop in &original {
        assert_eq!(
            journal.prop_removed(owner, prop.id),
            !prop.water_tolerant(),
            "{:?}",
            prop.kind
        );
    }
    let mut reloaded = EditJournal::default();
    reloaded.import_delta(journal.section_delta(owner)).unwrap();
    let mut restored = original;
    reloaded.apply_prop_removals(owner, &mut restored);
    for prop in restored {
        assert_eq!(prop.removed, !prop.water_tolerant(), "{:?}", prop.kind);
    }
}

#[test]
fn flood_contact_reaches_prop_owner_below_a_negative_section_boundary() {
    use crate::props::{PropId, PropKind, WorldProp};
    let terrain = terrain();
    let owner = SectionPos::new(-1, 0, -1);
    let wet_owner = SectionPos::new(-1, 1, -1);
    let mut chunks = ChunkStore::default();
    chunks.loaded.insert(owner, section(owner, 0));
    chunks.loaded.insert(wet_owner, section(wet_owner, 0));
    block(&mut chunks, [-1, 15, -1], 3);
    chunks
        .loaded
        .get_mut(&owner)
        .unwrap()
        .props
        .push(WorldProp {
            id: PropId(77),
            kind: PropKind::Fern,
            position: [-0.5, 16.0, -0.5],
            rotation: 0.0,
            scale: 1.0,
            variant: 0,
            section: owner,
            removed: false,
        });
    let mut journal = EditJournal::default();
    journal
        .set_fluid(
            [-1, 16, -1],
            FluidCell {
                amount: 8,
                flow: [1, 0],
                source: false,
            },
        )
        .unwrap();
    let step = simulate_fluid_step(&terrain, &chunks, &mut journal, 1).unwrap();
    assert_eq!(step.removed_props, 1);
    assert!(journal.prop_removed(owner, PropId(77)));
    assert_eq!(journal.prop_epoch(wet_owner), 0);
    assert_eq!(journal.prop_epoch(owner), 1);
}

#[test]
fn disconnected_cells_recede_with_accounted_mass_across_negative_boundaries() {
    let terrain = terrain();
    let mut chunks = ChunkStore::default();
    for x in -1..=0 {
        for z in -1..=0 {
            let p = SectionPos::new(x, 1, z);
            chunks.loaded.insert(p, section(p, 0));
            for lx in 0..16 {
                for lz in 0..16 {
                    block(&mut chunks, [x * 16 + lx, 16, z * 16 + lz], 3);
                }
            }
        }
    }
    let run = || {
        let mut journal = EditJournal::default();
        for p in [[-1, 20, -1], [0, 20, 0]] {
            journal
                .set_fluid(
                    p,
                    FluidCell {
                        amount: 8,
                        ..Default::default()
                    },
                )
                .unwrap();
        }
        let mut drained = 0;
        let mut replenished = 0;
        let mut crossed = false;
        for _ in 0..120 {
            let step = simulate_fluid_step(&terrain, &chunks, &mut journal, 64).unwrap();
            assert!(step.visited <= 64);
            assert_eq!(step.sourced, 0);
            drained += step.drained + step.absorbed;
            replenished += step.replenished;
            assert_eq!(amount(&journal) + drained, 16 + replenished);
            crossed |= journal.fluid_cells().any(|(p, _)| p[0] >= 0 && p[1] < 20);
        }
        assert!(crossed);
        assert_eq!(amount(&journal), 0);
        let mut deltas: Vec<_> = journal
            .changed_sections()
            .map(|p| journal.section_delta(p))
            .collect();
        deltas.sort_unstable_by_key(|d| (d.position.x, d.position.y, d.position.z));
        deltas
    };
    assert_eq!(run(), run());
}

#[test]
fn mining_bed_wakes_natural_source_and_replacing_barrier_stops_its_debit() {
    let terrain = terrain();
    let mut chunks = ChunkStore::default();
    chunks
        .loaded
        .insert(SectionPos::default(), section(SectionPos::default(), 3));
    block(&mut chunks, [4, 5, 4], 0);
    for x in 5..9 {
        block(&mut chunks, [x, 4, 4], 0);
    }
    let mut journal = EditJournal::default();
    assert!(fluid_cell_at(&terrain, &journal, [4, 5, 4], 0).source);
    assert_eq!(amount(&journal), 0);
    assert!(
        commit_block_edit(
            &mut chunks,
            &mut journal,
            &BlockEditRequest {
                position: [4, 4, 4],
                expected_material: 3,
                material: 0,
            }
        )
        .unwrap()
    );
    let mut sourced = 0;
    let mut drained = 0;
    for _ in 0..100 {
        let step = simulate_fluid_step(&terrain, &chunks, &mut journal, 64).unwrap();
        sourced += step.sourced + step.replenished;
        drained += step.drained + step.absorbed;
        assert_eq!(amount(&journal) + drained, sourced);
    }
    assert!(journal.fluid([8, 4, 4]).is_some());
    assert!(journal.fluid_cells().all(|(_, c)| !c.source));
    let displaced = journal.fluid([4, 4, 4]).unwrap().amount;
    assert!(
        commit_block_edit(
            &mut chunks,
            &mut journal,
            &BlockEditRequest {
                position: [4, 4, 4],
                expected_material: 0,
                material: 3,
            }
        )
        .unwrap()
    );
    assert_eq!(amount(&journal) + drained, sourced - u64::from(displaced));
    for _ in 0..100 {
        let step = simulate_fluid_step(&terrain, &chunks, &mut journal, 64).unwrap();
        assert_eq!(step.sourced, 0);
        sourced += step.replenished;
        drained += step.drained + step.absorbed;
        assert_eq!(amount(&journal) + drained, sourced - u64::from(displaced));
    }
    assert_eq!(amount(&journal), 0);
    assert_eq!(journal.revision(SectionPos::default()), 2);
    assert!(journal.fluid_epoch(SectionPos::default()) > 2);
    assert_eq!(journal.mesh_epoch(SectionPos::default()), 2);
}

#[test]
fn unloaded_floors_pause_without_discarding_water_and_resume_on_reload() {
    let terrain = terrain();
    let mut chunks = ChunkStore::default();
    chunks.loaded.insert(
        SectionPos::new(0, 1, 0),
        section(SectionPos::new(0, 1, 0), 0),
    );
    let mut journal = EditJournal::default();
    journal
        .set_fluid(
            [7, 16, 7],
            FluidCell {
                amount: 8,
                ..Default::default()
            },
        )
        .unwrap();
    for _ in 0..8 {
        simulate_fluid_step(&terrain, &chunks, &mut journal, 64).unwrap();
    }
    assert_eq!(journal.fluid([7, 16, 7]).unwrap().amount, 8);
    assert_eq!(amount(&journal), 8);
    let lower = SectionPos::default();
    chunks.loaded.insert(lower, section(lower, 3));
    block(&mut chunks, [7, 15, 7], 0);
    journal.resume_fluids(lower);
    let step = simulate_fluid_step(&terrain, &chunks, &mut journal, 64).unwrap();
    assert_eq!(journal.fluid([7, 15, 7]).unwrap().amount, 8);
    assert_eq!(amount(&journal) + step.drained, 8 + step.replenished);
    let delta = journal.section_delta(lower);
    let mut restored = EditJournal::default();
    restored.import_delta(delta.clone()).unwrap();
    assert_eq!(restored.section_delta(lower), delta);
}

#[test]
fn physical_queries_report_contiguous_liquid_and_solid_barriers() {
    let terrain = terrain();
    let mut chunks = ChunkStore::default();
    chunks
        .loaded
        .insert(SectionPos::default(), section(SectionPos::default(), 0));
    let mut journal = EditJournal::default();
    let natural = chunks
        .water_column_at(&terrain, &journal, [2, 6, 2])
        .unwrap();
    assert_eq!((natural.bed, natural.level), (5.0, 8.0));
    for material in [3, 14, 15] {
        block(&mut chunks, [2, 6, 2], material);
        assert!(
            chunks
                .water_column_at(&terrain, &journal, [2, 6, 2])
                .is_none()
        );
        assert_eq!(
            chunks
                .water_column_at(&terrain, &journal, [2, 7, 2])
                .unwrap()
                .bed,
            7.0
        );
    }
    for (y, amount) in [(10, 8), (11, 8), (12, 3)] {
        journal
            .set_fluid(
                [3, y, 3],
                FluidCell {
                    amount,
                    ..Default::default()
                },
            )
            .unwrap();
    }
    let dynamic = chunks
        .water_column_at(&terrain, &journal, [3, 11, 3])
        .unwrap();
    assert_eq!((dynamic.bed, dynamic.level), (10.0, 12.375));
    assert!(
        chunks
            .water_column_at(&terrain, &journal, [3, 13, 3])
            .is_none()
    );
}

#[test]
fn replica_system_never_simulates_local_liquid() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(WorldAccess::Replica { ready: true })
        .insert_resource(WorldTerrain(terrain()))
        .init_resource::<ChunkStore>()
        .init_resource::<EditJournal>()
        .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            std::time::Duration::from_millis(100),
        ))
        .add_systems(Update, tick);
    let p = SectionPos::new(0, 1, 0);
    app.world_mut()
        .resource_mut::<ChunkStore>()
        .loaded
        .insert(p, section(p, 0));
    app.world_mut()
        .resource_mut::<EditJournal>()
        .set_fluid(
            [2, 20, 2],
            FluidCell {
                amount: 8,
                ..Default::default()
            },
        )
        .unwrap();
    let before = app.world().resource::<EditJournal>().section_delta(p);
    for _ in 0..10 {
        app.update();
    }
    assert_eq!(
        app.world().resource::<EditJournal>().section_delta(p),
        before
    );
}

#[test]
fn a_large_disconnected_pool_cannot_self_feed_and_recedes_after_mid_drain_reload() {
    let terrain = terrain();
    let mut chunks = ChunkStore::default();
    for x in 0..2 {
        for z in 0..2 {
            let owner = SectionPos::new(x, 1, z);
            chunks.loaded.insert(owner, section(owner, 0));
            for lx in 0..16 {
                for lz in 0..16 {
                    block(&mut chunks, [x * 16 + lx, 16, z * 16 + lz], 3);
                }
            }
        }
    }
    let mut journal = EditJournal::default();
    for x in 2..30 {
        for z in 2..30 {
            journal
                .set_fluid(
                    [x, 17, z],
                    FluidCell {
                        amount: 8,
                        ..Default::default()
                    },
                )
                .unwrap();
        }
    }
    for _ in 0..10 {
        assert!(
            simulate_fluid_step(&terrain, &chunks, &mut journal, 256)
                .unwrap()
                .visited
                <= 256
        );
    }
    assert!(amount(&journal) > 0 && amount(&journal) < 28 * 28 * 8);
    let mut restored = EditJournal::default();
    let mut positions: Vec<_> = journal.changed_sections().collect();
    positions.sort_unstable_by_key(|p| (p.x, p.y, p.z));
    for p in positions {
        restored.import_delta(journal.section_delta(p)).unwrap();
        restored.resume_fluids(p);
    }
    for _ in 0..250 {
        assert!(
            simulate_fluid_step(&terrain, &chunks, &mut restored, 256)
                .unwrap()
                .visited
                <= 256
        );
    }
    assert_eq!(amount(&restored), 0);
}

#[test]
fn a_falling_source_joins_a_natural_reservoir_without_an_air_gap_or_extra_surface_layer() {
    let terrain = terrain();
    let owner = SectionPos::default();
    let mut chunks = ChunkStore::default();
    chunks.loaded.insert(owner, section(owner, 0));
    let mut journal = EditJournal::default();
    commit_block_edit(
        &mut chunks,
        &mut journal,
        &BlockEditRequest {
            position: [7, 13, 7],
            expected_material: 0,
            material: 27,
        },
    )
    .unwrap();
    for _ in 0..60 {
        simulate_fluid_step(&terrain, &chunks, &mut journal, 256).unwrap();
    }
    for y in 8..=13 {
        assert_eq!(
            journal.fluid([7, y, 7]).unwrap().amount,
            8,
            "air gap at reservoir y={y}"
        );
    }
    assert!(journal.fluid_cells().all(|(p, _)| p[0] == 7 && p[2] == 7));
    assert!(fluid_cell_at(&terrain, &journal, [7, 7, 7], 0).source);
}

#[test]
fn fractional_sources_feed_a_dry_bank_and_reservoir_return_is_accounted() {
    let wet_terrain = TerraGenerator::new(WorldConfig {
        sea_level: 8.05,
        ..terrain().config().clone()
    })
    .unwrap();
    let mut chunks = ChunkStore::default();
    chunks
        .loaded
        .insert(SectionPos::default(), section(SectionPos::default(), 3));
    block(&mut chunks, [4, 8, 4], 0);
    block(&mut chunks, [4, 9, 4], 0);
    let mut journal = EditJournal::default();
    // A manufactured finite cell is a non-source even in the source-height band.
    journal
        .set_fluid(
            [5, 8, 4],
            FluidCell {
                amount: 1,
                ..Default::default()
            },
        )
        .unwrap();
    block(&mut chunks, [5, 8, 4], 0);
    journal.wake_fluid_neighbors([4, 8, 4]);
    let source = fluid_cell_at(&wet_terrain, &journal, [4, 8, 4], 0);
    assert_eq!(source.amount, 1);
    // Test the one-unit pressure branch independently against a dry adjacent cell.
    let dry = terrain();
    let mut spill = EditJournal::default();
    spill
        .set_fluid(
            [4, 8, 4],
            FluidCell {
                amount: 1,
                source: true,
                flow: [0, 0],
            },
        )
        .unwrap();
    let result = simulate_fluid_step(&dry, &chunks, &mut spill, 64).unwrap();
    assert_eq!(result.sourced, 1);
    assert_eq!(spill.fluid([5, 8, 4]).unwrap().amount, 1);
    journal
        .set_fluid(
            [4, 9, 4],
            FluidCell {
                amount: 8,
                ..Default::default()
            },
        )
        .unwrap();
    let before = amount(&journal);
    let result = simulate_fluid_step(&wet_terrain, &chunks, &mut journal, 64).unwrap();
    assert_eq!(result.absorbed, 8);
    assert_eq!(amount(&journal), before + result.sourced - result.absorbed);
}

fn enclosed_cross_section_basin() -> (TerraGenerator, ChunkStore, EditJournal, [i64; 3]) {
    let terrain = terrain();
    let mut chunks = ChunkStore::default();
    for x in -1..=0 {
        let owner = SectionPos::new(x, 1, 0);
        chunks.loaded.insert(owner, section(owner, 3));
    }
    for x in -4..=4 {
        for y in 20..=23 {
            for z in 5..=11 {
                block(&mut chunks, [x, y, z], 0);
            }
        }
    }
    let source = [-1, 22, 8];
    let mut journal = EditJournal::default();
    assert!(
        commit_block_edit(
            &mut chunks,
            &mut journal,
            &BlockEditRequest {
                position: source,
                expected_material: 0,
                material: terra_voxel::MaterialId::FluidWater as u16,
            },
        )
        .unwrap()
    );
    (terrain, chunks, journal, source)
}

fn settle_basin(terrain: &TerraGenerator, chunks: &ChunkStore, journal: &mut EditJournal) {
    for _ in 0..1000 {
        let step = simulate_fluid_step(terrain, chunks, journal, 256).unwrap();
        if step.visited == 0 {
            return;
        }
    }
    panic!("enclosed basin did not settle within 100 seconds of bounded simulation");
}

fn saved_fluid_state(journal: &EditJournal) -> Vec<crate::edits::SectionDelta> {
    let mut sections: Vec<_> = journal
        .changed_sections()
        .map(|position| journal.section_delta(position))
        .collect();
    sections.sort_unstable_by_key(|section| {
        (section.position.x, section.position.y, section.position.z)
    });
    sections
}

#[test]
fn settled_source_and_basin_do_no_work_or_revision_growth_for_fifteen_simulated_minutes() {
    let (terrain, chunks, mut journal, _) = enclosed_cross_section_basin();
    settle_basin(&terrain, &chunks, &mut journal);
    let counts = journal.fluid_work_counts();
    let saved = saved_fluid_state(&journal);
    assert!(counts.recorded_cells > 1);
    assert_eq!(counts.sources, 1);
    assert_eq!(counts.active, 0);
    assert_eq!(counts.queued, 0);
    assert_eq!(counts.sleeping, 0);
    for _ in 0..9000 {
        let step = simulate_fluid_step(&terrain, &chunks, &mut journal, 256).unwrap();
        assert_eq!(step.visited, 0);
        assert!(!step.changed);
    }
    assert_eq!(journal.fluid_work_counts(), counts);
    assert_eq!(saved_fluid_state(&journal), saved);
}

#[test]
fn repeated_wakes_are_deduplicated_and_basin_reload_does_not_accumulate_work() {
    let (terrain, mut chunks, mut journal, source) = enclosed_cross_section_basin();
    settle_basin(&terrain, &chunks, &mut journal);
    let stable = journal.fluid_work_counts();
    let saved = saved_fluid_state(&journal);
    for _ in 0..10_000 {
        journal.wake_fluid_neighbors(source);
    }
    let awake = journal.fluid_work_counts();
    assert_eq!(awake.active, 7);
    assert_eq!(awake.active, awake.queued);
    settle_basin(&terrain, &chunks, &mut journal);
    assert_eq!(journal.fluid_work_counts(), stable);

    let owner = global_to_section(source).0;
    for _ in 0..64 {
        let unloaded = chunks.loaded.remove(&owner).unwrap();
        journal.wake_fluid_neighbors(source);
        settle_basin(&terrain, &chunks, &mut journal);
        let sleeping = journal.fluid_work_counts();
        assert_eq!(sleeping.active, 0);
        assert_eq!(sleeping.queued, 0);
        assert_eq!(sleeping.sleeping_sections, 1);
        assert!(sleeping.sleeping <= 7);
        assert_eq!(sleeping.recorded_cells, stable.recorded_cells);
        chunks.loaded.insert(owner, unloaded);
        journal.resume_fluids(owner);
        settle_basin(&terrain, &chunks, &mut journal);
        assert_eq!(journal.fluid_work_counts(), stable);
        assert_eq!(saved_fluid_state(&journal), saved);
    }
}
