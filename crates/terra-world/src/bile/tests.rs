use super::*;
use crate::streaming::LoadedSection;
use terra_voxel::{BlockSection, SectionPos};
fn fixture() -> (ChunkStore, EditJournal) {
    let mut chunks = ChunkStore::default();
    for z in -1..=1 {
        for x in -1..=1 {
            let p = SectionPos::new(x, 0, z);
            let mut data = BlockSection::empty(p).unwrap();
            for z in 0..16 {
                for x in 0..16 {
                    data.set_block([x, 1, z], 3).unwrap();
                }
            }
            chunks.loaded.insert(
                p,
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
    (chunks, EditJournal::default())
}
#[test]
fn source_spreads_four_cells_without_generating_chunks_and_drains_after_collection() {
    let (mut chunks, mut journal) = fixture();
    let origin = [0, 2, 0];
    assert!(deposit_from_bucket(&mut chunks, &mut journal, origin).unwrap());
    for _ in 0..160 {
        advance(&mut chunks, &mut journal).unwrap();
    }
    let cells: Vec<_> = journal
        .object_states()
        .filter(|(_, id, _)| *id == bile::FLUID)
        .collect();
    assert_eq!(cells.len(), 41);
    for (p, _, s) in &cells {
        assert!(in_range(*p, origin));
        assert!(s.valid_for(*p, bile::FLUID));
    }
    assert!(
        cells
            .iter()
            .any(|(p, _, s)| *p == [4, 2, 0] && s.liquid.unwrap().volume > 0)
    );
    assert_eq!(chunks.loaded.len(), 9);
    assert!(take_source(&mut chunks, &mut journal, origin).unwrap());
    for _ in 0..16 {
        advance(&mut chunks, &mut journal).unwrap();
    }
    assert!(!journal.object_states().any(|(_, id, _)| id == bile::FLUID));
}
#[test]
fn liquid_replica_preserves_source_and_unloaded_origin_freezes_the_flow() {
    let (mut chunks, mut journal) = fixture();
    let origin = [-1, 2, -1];
    deposit_from_bucket(&mut chunks, &mut journal, origin).unwrap();
    advance(&mut chunks, &mut journal).unwrap();
    let mut replica = EditJournal::default();
    for p in journal.changed_sections() {
        let bytes = serde_json::to_vec(&journal.section_delta(p)).unwrap();
        let delta = serde_json::from_slice(&bytes).unwrap();
        replica.import_delta(delta).unwrap();
    }
    assert_eq!(replica.object_state(origin), journal.object_state(origin));
    let section = global_to_section(origin).0;
    chunks.loaded.remove(&section);
    assert_eq!(advance(&mut chunks, &mut journal).unwrap().removed, 0);
    assert!(!deposit_from_bucket(&mut chunks, &mut journal, [100, 2, 100]).unwrap());
}
#[test]
fn acid_contact_follows_thin_surface_instead_of_whole_voxel() {
    let (mut chunks, mut journal) = fixture();
    let p = [0, 2, 0];
    set(
        &mut chunks,
        &mut journal,
        p,
        Some(LiquidState {
            origin: p,
            volume: 1,
            age: 0,
            source: true,
        }),
    )
    .unwrap();
    let body = terra_core::creatures::hitboxes::Hitbox {
        center: [0.5, 2.3, 0.5],
        size: [0.2; 3],
        axes: [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]],
    };
    assert!(!hitbox_contact(body, &chunks, &journal));
    assert!(hitbox_contact(
        terra_core::creatures::hitboxes::Hitbox {
            center: [0.5, 2.08, 0.5],
            ..body
        },
        &chunks,
        &journal
    ));
    assert_eq!(
        bile::FLOW_SECONDS,
        2. * terra_core::fluid_surface::WATER_STEP_SECONDS
    );
}

#[test]
fn unloaded_persistent_acid_counts_toward_the_global_growth_budget() {
    let (mut chunks, mut journal) = fixture();
    let p = [0, 2, 0];
    deposit_from_bucket(&mut chunks, &mut journal, p).unwrap();
    for i in 1..MAX_CELLS {
        let p = [1_000 + i as i64 * 5, 2, 1_000];
        journal.record(p, bile::FLUID).unwrap();
        journal
            .set_object_state(
                p,
                Some(BlockObjectState {
                    facing: 7,
                    liquid: Some(LiquidState {
                        origin: p,
                        volume: 8,
                        source: true,
                        age: 0,
                    }),
                    ..Default::default()
                }),
            )
            .unwrap();
    }
    let step = advance(&mut chunks, &mut journal).unwrap();
    assert_eq!(step.visited, 1);
    assert_eq!(step.spread, 0);
    assert_eq!(
        journal
            .object_states()
            .filter(|(_, id, _)| *id == bile::FLUID)
            .count(),
        MAX_CELLS
    );
}

#[test]
fn spit_sources_expire_after_five_seconds_even_after_unload_but_collected_buckets_persist() {
    let (mut chunks, mut journal) = fixture();
    let p = [0, 2, 0];
    deposit_from_spit(&mut chunks, &mut journal, p).unwrap();
    for _ in 0..10 {
        advance(&mut chunks, &mut journal).unwrap();
    }
    assert!(liquid(&chunks, &journal, p).unwrap().source);
    assert!(take_source(&mut chunks, &mut journal, p).unwrap());
    deposit_from_bucket(&mut chunks, &mut journal, p).unwrap();
    for _ in 0..30 {
        advance(&mut chunks, &mut journal).unwrap();
    }
    assert!(liquid(&chunks, &journal, p).unwrap().source);
    take_source(&mut chunks, &mut journal, p).unwrap();
    deposit_from_spit(&mut chunks, &mut journal, p).unwrap();
    chunks.loaded.remove(&global_to_section(p).0);
    let cells = bile_cells(&journal);
    advance_from(
        &mut chunks,
        &mut journal,
        &mut 0,
        bile::SPIT_STEPS - 1,
        cells,
    )
    .unwrap();
    assert!(journal.object_state(p).is_some());
    advance(&mut chunks, &mut journal).unwrap();
    assert_eq!(journal.block(p), Some(0));
    assert!(journal.object_state(p).is_none());
}
#[test]
fn first_acid_front_receives_only_paid_partial_volume_and_stays_valid_on_the_wire() {
    let (mut chunks, mut journal) = fixture();
    let p = [0, 2, 0];
    deposit_from_bucket(&mut chunks, &mut journal, p).unwrap();
    advance(&mut chunks, &mut journal).unwrap();
    let q = [1, 2, 0];
    let state = journal.object_state(q).unwrap();
    let fluid = state.liquid.unwrap();
    assert!(fluid.volume > 0 && fluid.volume < bile::CAPACITY / 2);
    assert!(state.valid_for(q, bile::FLUID));
    let bytes = serde_json::to_vec(&state).unwrap();
    assert_eq!(
        serde_json::from_slice::<BlockObjectState>(&bytes).unwrap(),
        state
    );
}

#[test]
fn children_recede_after_an_unloaded_spit_source_expires_across_a_section_edge() {
    let (mut chunks, mut journal) = fixture();
    let p = [-1, 2, 0];
    deposit_from_spit(&mut chunks, &mut journal, p).unwrap();
    advance(&mut chunks, &mut journal).unwrap();
    let child = [0, 2, 0];
    assert!(liquid(&chunks, &journal, child).is_some());
    chunks.loaded.remove(&global_to_section(p).0);
    for _ in 0..bile::SPIT_STEPS + 8 {
        advance(&mut chunks, &mut journal).unwrap();
    }
    assert_eq!(journal.block(p), Some(0));
    assert!(liquid(&chunks, &journal, child).is_none());
}

#[test]
fn galle_uses_the_water_surface_across_a_section_edge_at_half_the_clock_rate() {
    let (mut chunks, mut journal) = fixture();
    let source = [-1, 2, 5];
    deposit_from_bucket(&mut chunks, &mut journal, source).unwrap();
    advance(&mut chunks, &mut journal).unwrap();
    let pos = SectionPos::default();
    let snapshot =
        BileSurface::capture(&chunks.loaded[&pos].data, &journal, |p| chunks.block_at(p)).unwrap();
    let galle = snapshot.mesh().unwrap();
    let mut fluid = crate::water::FluidSnapshot::new();
    let mut padded = Vec::new();
    for z in -1..=16 {
        for y in -1..=16 {
            for x in -1..=16 {
                let p = [x, y, z];
                padded.push(match chunks.block_at(p) {
                    Some(0 | bile::FLUID) => 0,
                    _ => 3,
                });
                if let Some(s) = liquid(&chunks, &journal, p) {
                    fluid.insert(
                        p,
                        crate::water::FluidCell {
                            amount: s.volume / 8,
                            source: s.source,
                            flow: if s.source {
                                [0; 2]
                            } else {
                                [(x - source[0]) as i8, (z - source[2]) as i8]
                            },
                        },
                    );
                }
            }
        }
    }
    let mut water = None;
    crate::water::append_fluid_patch(&mut water, pos, &[], &padded, &fluid, false);
    let water = water.unwrap();
    assert_eq!(water.positions, galle.positions);
    assert_eq!(water.normals, galle.normals);
    assert_eq!(water.indices, galle.indices);
    assert_eq!(water.flow, galle.flow);
    assert!(
        galle
            .surface_effects
            .iter()
            .any(|v| v[3] == crate::water::POUR_SURFACE)
    );
    let tip = galle.positions.iter().map(|p| p[0]).fold(0., f32::max);
    assert!(tip > 0.2 && tip < 0.5);
    assert!(height_at(&chunks, &journal, [0.1, 2.02, 5.5]).is_some());
    assert!(height_at(&chunks, &journal, [0.8, 2.02, 5.5]).is_none());
    assert!(!hitbox_contact(
        terra_core::creatures::hitboxes::Hitbox {
            center: [0.8, 2.04, 5.5],
            size: [0.12; 3],
            axes: [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]]
        },
        &chunks,
        &journal
    ));
    assert_eq!(
        bile::FLOW_SECONDS,
        2. * terra_core::fluid_surface::WATER_STEP_SECONDS
    );
}

#[test]
fn old_spit_timers_keep_their_remaining_seconds_on_the_faster_clock() {
    let (mut chunks, mut journal) = fixture();
    let p = [2, 2, 2];
    set(
        &mut chunks,
        &mut journal,
        p,
        Some(LiquidState {
            origin: p,
            volume: 64,
            source: true,
            age: bile::FINE | 10,
        }),
    )
    .unwrap();
    for _ in 0..15 {
        advance(&mut chunks, &mut journal).unwrap();
    }
    assert!(liquid(&chunks, &journal, p).is_some_and(|s| s.source));
    advance(&mut chunks, &mut journal).unwrap();
    assert!(!liquid(&chunks, &journal, p).is_some_and(|s| s.source));
}

#[test]
fn blocking_the_only_channel_drains_the_isolated_flow_while_the_source_survives() {
    let (mut chunks, mut journal) = fixture();
    for x in -1..=5 {
        for z in [2, 4] {
            let (section, local) = global_to_section([x, 2, z]);
            chunks
                .loaded
                .get_mut(&section)
                .unwrap()
                .data
                .set_block(local, 3)
                .unwrap();
        }
    }
    let origin = [0, 2, 3];
    deposit_from_bucket(&mut chunks, &mut journal, origin).unwrap();
    for _ in 0..32 {
        advance(&mut chunks, &mut journal).unwrap();
    }
    assert!(liquid(&chunks, &journal, [3, 2, 3]).is_some());
    commit_block_edit(
        &mut chunks,
        &mut journal,
        &BlockEditRequest {
            position: [1, 2, 3],
            expected_material: bile::FLUID,
            material: 3,
        },
        &BlockPropertiesCatalog::default(),
    )
    .unwrap();
    for _ in 0..12 {
        advance(&mut chunks, &mut journal).unwrap();
    }
    assert!(liquid(&chunks, &journal, origin).is_some_and(|s| s.source));
    assert!(liquid(&chunks, &journal, [3, 2, 3]).is_none());
}

#[test]
fn a_falling_jet_returns_to_a_lower_reservoir_without_spilling_sideways_in_midair() {
    let (mut chunks, mut journal) = fixture();
    for y in [2, 4] {
        deposit_from_bucket(&mut chunks, &mut journal, [2, y, 2]).unwrap();
        for [dx, _, dz] in SIDES {
            let (section, local) = global_to_section([2 + dx, y, 2 + dz]);
            chunks
                .loaded
                .get_mut(&section)
                .unwrap()
                .data
                .set_block(local, 3)
                .unwrap();
        }
    }
    for _ in 0..24 {
        advance(&mut chunks, &mut journal).unwrap();
    }
    assert!(liquid(&chunks, &journal, [2, 3, 2]).is_some_and(|cell| cell.volume > 0));
    for [dx, _, dz] in SIDES {
        assert!(liquid(&chunks, &journal, [2 + dx, 3, 2 + dz]).is_none());
    }
}

#[test]
fn idle_acid_and_tar_ticks_leave_terrain_and_journal_change_ticks_untouched() {
    let mut app = App::new();
    app.init_resource::<Time>()
        .init_resource::<WorldAccess>()
        .init_resource::<ChunkStore>()
        .init_resource::<EditJournal>()
        .add_systems(Update, (tick, crate::roads::tick));
    app.update();
    let before = (
        app.world().resource_ref::<ChunkStore>().last_changed(),
        app.world().resource_ref::<EditJournal>().last_changed(),
    );
    for _ in 0..3 {
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs(1));
        app.update();
        assert_eq!(
            app.world().resource_ref::<ChunkStore>().last_changed(),
            before.0
        );
        assert_eq!(
            app.world().resource_ref::<EditJournal>().last_changed(),
            before.1
        );
    }
}
