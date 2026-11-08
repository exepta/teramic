use super::*;
use crate::streaming::{ChunkStore, LoadedSection};
use terra_core::rails::{self, Endpoint};
use terra_voxel::{BlockSection, global_to_section};

pub(super) fn block(chunks: &mut ChunkStore, cell: Cell, material: u16) {
    let (owner, local) = global_to_section(cell);
    chunks
        .loaded
        .entry(owner)
        .or_insert_with(|| LoadedSection {
            data: BlockSection::empty(owner).unwrap(),
            mesh: None,
            mesh_epoch: 0,
            water_epoch: 0,
            water: None,
            props: Vec::new(),
        })
        .data
        .set_block(local, material)
        .unwrap();
}

pub(super) fn prepare(chunks: &mut ChunkStore, piece: Piece) {
    for cell in cells(piece) {
        block(chunks, cell, 0);
    }
    for support in piece.supports().iter() {
        block(
            chunks,
            std::array::from_fn(|axis| piece.anchor[axis] + i64::from(support.support[axis])),
            3,
        );
    }
}

fn install(chunks: &mut ChunkStore, journal: &mut EditJournal, piece: Piece) {
    let layout = plan(
        chunks,
        journal,
        piece.anchor,
        piece.material,
        piece.encoded_facing(),
    )
    .unwrap();
    assert!(commit(chunks, journal, &layout).unwrap());
    assert!(complete(journal, piece));
}

#[test]
fn every_module_owns_all_its_cells_and_breaks_once_from_an_unloaded_secondary() {
    for material in rails::MATERIALS {
        for facing in 0..4 {
            let piece = Piece::new([-17, 16, -17], material, facing).unwrap();
            let mut chunks = ChunkStore::default();
            let mut journal = EditJournal::default();
            prepare(&mut chunks, piece);
            install(&mut chunks, &mut journal, piece);
            assert_eq!(journal.rail_module_count(), 1);
            for cell in cells(piece) {
                assert_eq!(owner_at(&journal, cell), Some(piece));
                assert!(
                    journal
                        .object_state(cell)
                        .unwrap()
                        .valid_for(cell, material)
                );
                assert!(crate::water::can_hold_fluid(material));
                let authored = cell_boxes(cell, material, object_state(piece, cell));
                if authored.is_empty() {
                    assert!(
                        piece.kind().is_some_and(rails::Kind::is_switch),
                        "only historic switch lever reservations may lack solids"
                    );
                    let low = std::array::from_fn::<_, 3, _>(|axis| {
                        (cell[axis] - piece.anchor[axis]) as f64
                    });
                    assert!(
                        piece.local_geometry().iter().all(|beam| {
                            let bounds = beam.bounds();
                            (0..3).any(|axis| {
                                bounds.maximum[axis].min(low[axis] + 1.0)
                                    - bounds.minimum[axis].max(low[axis])
                                    <= 1e-8
                            })
                        }),
                        "an empty legacy slice cannot hide real beam collision"
                    );
                }
                assert!(authored.iter().all(|bounds| {
                    (0..3).all(|axis| bounds.minimum[axis] >= 0.0 && bounds.maximum[axis] <= 1.0)
                }));
            }
            let secondary = cells(piece)
                .into_iter()
                .find(|cell| *cell != piece.anchor)
                .unwrap();
            chunks.loaded.remove(&global_to_section(secondary).0);
            let removal = break_layout(&chunks, &journal, secondary).unwrap();
            assert_eq!(removal.edits.len(), piece.footprint().len());
            assert!(commit(&mut chunks, &mut journal, &removal).unwrap());
            assert!(
                cells(piece)
                    .into_iter()
                    .all(|cell| journal.block(cell) == Some(0)
                        && journal.object_state(cell).is_none())
            );
            assert!(break_layout(&chunks, &journal, secondary).is_err());
            assert_eq!(journal.rail_module_count(), 0);
            assert!(network(&journal).unwrap().is_empty());
            assert!(
                journal.gameplay().drops.is_empty(),
                "the caller owns the single module drop"
            );
        }
    }
}

#[test]
fn missing_support_obstructions_and_changed_cells_never_publish_a_partial_module() {
    let piece = Piece::new([-1, 16, -1], rails::CURVE_LARGE, 0).unwrap();
    let mut chunks = ChunkStore::default();
    let mut journal = EditJournal::default();
    prepare(&mut chunks, piece);
    let support = piece.supports().last().unwrap().support;
    let support = std::array::from_fn(|axis| piece.anchor[axis] + i64::from(support[axis]));
    for point in piece.supports().iter() {
        block(
            &mut chunks,
            std::array::from_fn(|axis| piece.anchor[axis] + i64::from(point.support[axis])),
            0,
        );
    }
    assert!(plan(&chunks, &journal, piece.anchor, piece.material, 0).is_err());
    block(&mut chunks, support, 3);
    let layout = plan(&chunks, &journal, piece.anchor, piece.material, 0).unwrap();
    let changed = layout.edits.last().unwrap().position;
    block(&mut chunks, changed, 3);
    assert!(commit(&mut chunks, &mut journal, &layout).is_err());
    assert_eq!(journal.edited_blocks(), 0);
    assert_eq!(journal.rail_module_count(), 0);
    assert!(
        cells(piece)
            .into_iter()
            .all(|cell| chunks.block_at(cell) == Some(if cell == changed { 3 } else { 0 }))
    );
}

#[test]
fn unsupported_or_obstructed_silhouette_candidates_never_authorize_placement() {
    let piece = Piece::new([0, 16, 0], rails::STRAIGHT, 0).unwrap();
    for occupied in [false, true] {
        let mut chunks = ChunkStore::default();
        let mut journal = EditJournal::default();
        prepare(&mut chunks, piece);
        let obstruction = if occupied {
            *cells(piece).last().unwrap()
        } else {
            let support = piece.supports().last().unwrap().support;
            std::array::from_fn(|axis| piece.anchor[axis] + i64::from(support[axis]))
        };
        if !occupied {
            for point in piece.supports().iter() {
                block(
                    &mut chunks,
                    std::array::from_fn(|axis| piece.anchor[axis] + i64::from(point.support[axis])),
                    0,
                );
            }
        }
        block(&mut chunks, obstruction, if occupied { 3 } else { 0 });
        let hit = terra_voxel::blocks::BlockHit {
            position: [0, 15, 0],
            normal: [0, 1, 0],
            distance: 2.0,
        };
        let ray = ([0.5, 18.0, -2.0], [0.0, -0.5, 0.8660254]);
        let look = [std::f32::consts::PI, -0.5];
        let silhouette = proposal(&chunks, &journal, hit, piece.material, ray, look).unwrap();
        assert_eq!(silhouette.edits.len(), piece.footprint().len());
        assert!(
            silhouette
                .states
                .iter()
                .any(|(cell, state)| *cell == piece.anchor
                    && state.is_some_and(|state| !state.secondary))
        );
        assert!(placement(&chunks, &journal, hit, piece.material, ray, look).is_err());
        assert!(environment(&silhouette, &chunks, &journal).is_err());
        assert!(commit(&mut chunks, &mut journal, &silhouette).is_err());
        assert_eq!(journal.edited_blocks(), 0);
    }
}

#[test]
fn rail_geometry_at_high_altitude_respects_the_callers_bounded_world_policy() {
    let piece = Piece::new([-17, 400, -17], rails::STRAIGHT, 0).unwrap();
    let mut chunks = ChunkStore::default();
    let mut journal = EditJournal::default();
    prepare(&mut chunks, piece);
    journal
        .record([1000, 400, 1000], terra_core::kinetic::WATER_WHEEL)
        .unwrap();
    let layout = plan(&chunks, &journal, piece.anchor, piece.material, 0).unwrap();
    assert!(crate::placement::environment(&layout, &chunks, &journal, true).is_err());
    assert!(crate::placement::environment(&layout, &chunks, &journal, false).is_ok());
    assert!(commit(&mut chunks, &mut journal, &layout).unwrap());
    assert!(complete(&journal, piece));
}

#[test]
fn water_is_retained_around_real_rails_and_survives_module_removal() {
    let piece = Piece::new([15, 16, -17], rails::STRAIGHT, 0).unwrap();
    let mut chunks = ChunkStore::default();
    let mut journal = EditJournal::default();
    prepare(&mut chunks, piece);
    for cell in cells(piece) {
        block(&mut chunks, cell, 27);
    }
    install(&mut chunks, &mut journal, piece);
    for cell in cells(piece) {
        assert_eq!(journal.fluid(cell).unwrap().amount, 8);
        let space =
            crate::water::space::FluidSpace::at(&journal, cell, |cell| chunks.block_at(cell));
        assert_eq!(
            space.solids.as_slice(),
            cell_boxes(cell, piece.material, object_state(piece, cell)).as_ref()
        );
        assert!(space.solids.iter().all(|bounds| bounds.maximum[1] < 0.5));
    }
    let layout = break_layout(&chunks, &journal, piece.anchor).unwrap();
    assert!(commit(&mut chunks, &mut journal, &layout).unwrap());
    assert!(
        cells(piece)
            .into_iter()
            .all(|cell| journal.fluid(cell).unwrap().amount == 8)
    );
}

#[test]
fn protected_hives_cannot_be_overwritten_but_existing_rails_allow_later_slab_floor_changes() {
    let piece = Piece::new([0, 16, 0], rails::STRAIGHT, 0).unwrap();
    let mut chunks = ChunkStore::default();
    let mut journal = EditJournal::default();
    prepare(&mut chunks, piece);
    let support = [0, 15, 0];
    let prop = crate::props::WorldProp {
        id: crate::props::PropId(620),
        kind: crate::props::PropKind::BeeNest,
        position: [0.5, 16.0, 0.5],
        rotation: 0.0,
        scale: 1.0,
        variant: 0,
        section: global_to_section(support).0,
        removed: false,
    };
    assert!(journal.place_prop(prop).unwrap());
    assert!(plan(&chunks, &journal, piece.anchor, piece.material, 0).is_err());
    assert_eq!(journal.placed_props(prop.section).count(), 1);
    journal.remove_prop(prop.section, prop.id).unwrap();
    let slab = terra_core::construction::SLAB_MATERIALS[0];
    block(&mut chunks, support, slab);
    journal.record(support, slab).unwrap();
    journal
        .set_object_state(
            support,
            Some(BlockObjectState {
                facing: 4,
                ..Default::default()
            }),
        )
        .unwrap();
    install(&mut chunks, &mut journal, piece);
    assert!(placement::support_state_allowed(
        &journal,
        support,
        slab,
        Some(BlockObjectState {
            facing: 0,
            ..Default::default()
        })
    ));
}

#[test]
fn rail_section_unloading_does_not_prevent_later_bedding_changes() {
    let piece = Piece::new([-17, 16, -17], rails::SLOPE_ENTRY, 0).unwrap();
    let mut chunks = ChunkStore::default();
    let mut journal = EditJournal::default();
    prepare(&mut chunks, piece);
    install(&mut chunks, &mut journal, piece);
    chunks.loaded.clear();
    for point in piece.supports().iter() {
        let support =
            std::array::from_fn(|axis| piece.anchor[axis] + i64::from(point.support[axis]));
        assert!(support_edit_allowed(&chunks, &journal, support, 0));
        assert!(support_edit_allowed(&chunks, &journal, support, 3));
    }
    assert!(support_edit_allowed(&chunks, &journal, [-500, 0, -500], 0));
}

#[test]
fn real_underlay_edits_leave_floating_rail_ownership_water_geometry_and_routes_intact() {
    for material in [rails::STRAIGHT, rails::SLOPE_ENTRY, rails::SWITCH_RIGHT] {
        let original = Piece::new([-17, 16, -17], material, 0).unwrap();
        let mut chunks = ChunkStore::default();
        let mut journal = EditJournal::default();
        prepare(&mut chunks, original);
        for point in original.supports().iter() {
            block(
                &mut chunks,
                std::array::from_fn(|axis| original.anchor[axis] + i64::from(point.support[axis])),
                206,
            );
        }
        install(&mut chunks, &mut journal, original);
        for cell in cells(original) {
            journal
                .set_fluid(
                    cell,
                    crate::water::FluidCell {
                        amount: 8,
                        source: true,
                        ..Default::default()
                    },
                )
                .unwrap();
        }
        let graph = network(&journal).unwrap();
        let geometry = original.local_geometry();
        let epoch = journal.rail_epoch();
        assert_eq!(
            ballast::speed_multiplier(&chunks, &journal, original.anchor),
            1.5
        );
        for point in original.supports().iter() {
            let support =
                std::array::from_fn(|axis| original.anchor[axis] + i64::from(point.support[axis]));
            assert!(
                crate::streaming::commit_block_edit(
                    &mut chunks,
                    &mut journal,
                    &crate::streaming::BlockEditRequest {
                        position: support,
                        expected_material: 206,
                        material: 0
                    },
                    &Default::default()
                )
                .unwrap()
            );
            assert_eq!(journal.block(support), Some(0));
            assert!(complete(&journal, original));
            assert_eq!(network(&journal).unwrap(), graph);
            assert_eq!(journal.rail_epoch(), epoch);
            assert_eq!(
                ballast::speed_multiplier(&chunks, &journal, original.anchor),
                1.0
            );
            assert!(std::sync::Arc::ptr_eq(
                &geometry,
                &original.local_geometry()
            ));
            for cell in cells(original) {
                assert_eq!(chunks.block_at(cell), Some(material));
                assert_eq!(
                    journal.object_state(cell),
                    Some(object_state(original, cell))
                );
                assert_eq!(journal.fluid(cell).unwrap().amount, 8);
            }
        }
        let sections: Vec<_> = journal.changed_sections().collect();
        let mut replica = EditJournal::default();
        for section in sections {
            replica
                .import_delta(journal.section_delta(section))
                .unwrap();
        }
        assert!(complete(&replica, original));
        assert_eq!(network(&replica).unwrap(), graph);
        for point in original.supports().iter() {
            let support =
                std::array::from_fn(|axis| original.anchor[axis] + i64::from(point.support[axis]));
            assert_eq!(replica.block(support), Some(0));
        }
        let piece = if original.kind().is_some_and(rails::Kind::is_switch) {
            let toggle = toggle_switch_plan(&chunks, &journal, original.anchor).unwrap();
            assert!(commit(&mut chunks, &mut journal, &toggle).unwrap());
            let switched = Piece::new(original.anchor, material, 4).unwrap();
            assert!(complete(&journal, switched));
            switched
        } else {
            original
        };
        let remove = break_layout(&chunks, &journal, piece.anchor).unwrap();
        assert!(commit(&mut chunks, &mut journal, &remove).unwrap());
        assert!(!complete(&journal, piece));
        assert!(
            plan(&chunks, &journal, piece.anchor, piece.material, 0).is_err(),
            "new placement still requires the removed foundation"
        );
    }
}

#[test]
fn switch_changes_all_slices_without_changing_ownership_or_reserved_space() {
    for material in [rails::SWITCH_LEFT, rails::SWITCH_RIGHT] {
        let main = Piece::new([-17, 16, 15], material, 1).unwrap();
        let branch = Piece::new(main.anchor, material, 5).unwrap();
        assert_eq!(main.footprint(), branch.footprint());
        let mut chunks = ChunkStore::default();
        let mut journal = EditJournal::default();
        prepare(&mut chunks, main);
        install(&mut chunks, &mut journal, main);
        let mut cache = NetworkCache::default();
        assert!(cache.refresh(&journal).unwrap());
        assert!(!cache.refresh(&journal).unwrap());
        journal
            .record([main.anchor[0], main.anchor[1] + 8, main.anchor[2]], 3)
            .unwrap();
        assert!(
            !cache.refresh(&journal).unwrap(),
            "unrelated terrain must not rebuild routing"
        );
        let layout = toggle_switch_plan(&chunks, &journal, *cells(main).last().unwrap()).unwrap();
        chunks.loaded.clear();
        assert!(commit(&mut chunks, &mut journal, &layout).unwrap());
        assert!(complete(&journal, branch));
        assert!(cache.refresh(&journal).unwrap());
        assert_eq!(cache.graph().get(main.anchor), Some(&branch));
        let from = Endpoint {
            anchor: main.anchor,
            port: 0,
        };
        let to = Endpoint {
            anchor: main.anchor,
            port: 2,
        };
        assert!(cache.graph().route(from, to, 128).is_ok());
    }
}

#[test]
fn snap_extends_the_actual_free_port_and_rejects_an_incompatible_grade() {
    let first = Piece::new([0, 16, 0], rails::STRAIGHT, 0).unwrap();
    let next = Piece::new([0, 16, 4], rails::STRAIGHT, 0).unwrap();
    let mut chunks = ChunkStore::default();
    let mut journal = EditJournal::default();
    prepare(&mut chunks, first);
    prepare(&mut chunks, next);
    install(&mut chunks, &mut journal, first);
    let hit = terra_voxel::blocks::BlockHit {
        position: [0, 16, 3],
        normal: [0, 1, 0],
        distance: 1.0,
    };
    let ray = ([0.5, 17.0, 4.0], [0.0, -1.0, 0.0]);
    let extension = placement(
        &chunks,
        &journal,
        hit,
        rails::STRAIGHT,
        ray,
        [std::f32::consts::PI, 0.0],
    )
    .unwrap();
    assert_eq!(
        extension
            .states
            .iter()
            .find_map(|(cell, state)| state.filter(|state| !state.secondary).map(|_| *cell)),
        Some(next.anchor)
    );
    assert!(commit(&mut chunks, &mut journal, &extension).unwrap());
    assert_eq!(network(&journal).unwrap().len(), 2);
    assert!(placement(&chunks, &journal, hit, rails::SLOPE, ray, [0.0; 2]).is_err());
}

#[test]
fn partial_section_imports_never_expose_a_partial_graph() {
    let piece = Piece::new([-1, 16, -1], rails::CURVE_LARGE, 0).unwrap();
    let mut chunks = ChunkStore::default();
    let mut journal = EditJournal::default();
    prepare(&mut chunks, piece);
    install(&mut chunks, &mut journal, piece);
    let sections: BTreeMap<_, _> = cells(piece)
        .into_iter()
        .map(|cell| {
            let owner = global_to_section(cell).0;
            (owner, journal.section_delta(owner))
        })
        .collect();
    assert!(sections.len() > 1);
    let mut replica = EditJournal::default();
    for (index, (_, delta)) in sections.iter().enumerate() {
        replica.import_delta(delta.clone()).unwrap();
        assert_eq!(
            network(&replica).unwrap().len(),
            usize::from(index + 1 == sections.len())
        );
    }
    assert_eq!(network(&replica).unwrap(), network(&journal).unwrap());
    let mut cache = NetworkCache::default();
    cache.refresh(&journal).unwrap();
    assert!(
        !cache.refresh(&replica).unwrap(),
        "identical reloads preserve the cached route graph"
    );
    let empty = EditJournal::default();
    assert!(cache.refresh(&empty).unwrap());
    assert!(cache.graph().is_empty());
}

#[test]
fn free_curve_preserves_both_ports_wood_cost_and_unloaded_slice_ownership() {
    let first = Piece::new([-17, 16, -17], rails::STRAIGHT, 0).unwrap();
    let mut chunks = ChunkStore::default();
    let mut journal = EditJournal::default();
    prepare(&mut chunks, first);
    install(&mut chunks, &mut journal, first);
    let start = Endpoint {
        anchor: first.anchor,
        port: 1,
    };
    let hit = terra_voxel::blocks::BlockHit {
        position: [-10, 15, 2],
        normal: [0, 1, 0],
        distance: 2.0,
    };
    let look = [std::f32::consts::PI, -0.5];
    let wood = terra_core::WoodSpecies::Cherry;
    let proposal = proposal_connection(&chunks, &journal, start, hit, look, wood).unwrap();
    let (anchor, state) = proposal
        .states
        .iter()
        .find_map(|(cell, state)| {
            state
                .filter(|state| !state.secondary)
                .map(|state| (*cell, state))
        })
        .unwrap();
    let curve = piece_from_state(anchor, rails::STRAIGHT, state).unwrap();
    assert!(curve.curve.is_some());
    let old_cells: std::collections::BTreeSet<_> = cells(first).into_iter().collect();
    assert!(cells(curve).iter().all(|cell| !old_cells.contains(cell)));
    assert!(first.ports()[1].connects(curve.ports()[0]));
    assert!(
        connection(&chunks, &journal, start, hit, look, wood).is_err(),
        "an unprepared floor cannot authorize the candidate"
    );
    prepare(&mut chunks, curve);
    let plan = connection(&chunks, &journal, start, hit, look, wood).unwrap();
    assert_eq!(cost(&plan), Some(piece_cost(curve)));
    assert!(piece_cost(curve) > 1);
    assert!(commit(&mut chunks, &mut journal, &plan).unwrap());
    assert!(complete(&journal, curve));
    for cell in cells(curve) {
        let state = journal.object_state(cell).unwrap();
        assert_eq!(state.wood, wood);
        assert_eq!(state.rail_curve, curve.curve);
        assert_eq!(owner_at(&journal, cell), Some(curve));
    }
    let graph = network(&journal).unwrap();
    assert!(
        graph
            .route(
                Endpoint {
                    anchor: first.anchor,
                    port: 0
                },
                Endpoint {
                    anchor: curve.anchor,
                    port: 1
                },
                32
            )
            .is_ok()
    );
    let secondary = *cells(curve).last().unwrap();
    chunks.loaded.remove(&global_to_section(curve.anchor).0);
    assert!(
        !cell_boxes(
            secondary,
            curve.material,
            journal.object_state(secondary).unwrap()
        )
        .is_empty()
    );
    let removal = break_layout(&chunks, &journal, secondary).unwrap();
    assert!(commit(&mut chunks, &mut journal, &removal).unwrap());
    assert_eq!(network(&journal).unwrap().len(), 1);
    assert!(complete(&journal, first));
}

#[test]
fn rail_clearance_removes_only_intersecting_generated_and_saved_props_and_reports_foreign_support()
{
    let piece = Piece::new([14, 16, 0], rails::STRAIGHT, 0).unwrap();
    let mut chunks = ChunkStore::default();
    let mut journal = EditJournal::default();
    prepare(&mut chunks, piece);
    let make = |id, position: [f64; 3]| crate::props::WorldProp {
        id: crate::props::PropId(id),
        kind: crate::props::PropKind::Grass,
        position,
        rotation: 0.0,
        scale: 1.0,
        variant: 0,
        section: global_to_section([position[0].floor() as i64, 15, position[2].floor() as i64]).0,
        removed: false,
    };
    let natural = make(20501, [14.5, 16.0, 0.5]);
    let saved = make(20502, [14.5, 16.0, 1.5]);
    let foreign = make(20503, [16.1, 16.0, 0.5]);
    let untouched = make(20504, [18.5, 16.0, 0.5]);
    for prop in [natural, foreign, untouched] {
        let support = crate::props::placement::support(&prop);
        block(&mut chunks, support, 3);
        chunks
            .loaded
            .get_mut(&prop.section)
            .unwrap()
            .props
            .push(prop);
    }
    assert!(journal.place_prop(saved).unwrap());
    let plan = plan(&chunks, &journal, piece.anchor, piece.material, 0).unwrap();
    let cleared = clearance(&plan, &chunks, &journal).unwrap();
    assert_eq!(
        cleared
            .iter()
            .map(|entry| entry.prop.id)
            .collect::<std::collections::BTreeSet<_>>(),
        [natural.id, saved.id, foreign.id].into_iter().collect()
    );
    assert_eq!(
        cleared
            .iter()
            .find(|entry| entry.prop.id == saved.id)
            .unwrap()
            .source,
        PropSource::Saved
    );
    assert!(
        cleared
            .iter()
            .find(|entry| entry.prop.id == foreign.id)
            .unwrap()
            .rights_cells
            .iter()
            .any(|cell| cell[0] == 16)
    );
    assert!(commit(&mut chunks, &mut journal, &plan).unwrap());
    for prop in [natural, saved, foreign] {
        assert!(journal.prop_removed(prop.section, prop.id));
    }
    assert!(!journal.prop_removed(untouched.section, untouched.id));
    assert!(
        journal
            .placed_props(saved.section)
            .all(|prop| prop.id != saved.id)
    );
}

#[test]
fn clearing_a_planted_sapling_retires_its_growth_record_without_touching_other_trees() {
    let piece = Piece::new([0, 16, 0], rails::STRAIGHT, 0).unwrap();
    let mut chunks = ChunkStore::default();
    let mut journal = EditJournal::default();
    prepare(&mut chunks, piece);
    block(&mut chunks, [0, 15, 0], 2);
    let prop = crate::props::WorldProp {
        id: crate::props::PropId(20520),
        kind: crate::props::PropKind::OakSapling,
        position: [0.5, 16.0, 0.5],
        rotation: 0.0,
        scale: 1.0,
        variant: 0,
        section: global_to_section([0, 15, 0]).0,
        removed: false,
    };
    assert!(
        crate::saplings::plant(
            &mut chunks,
            &mut journal,
            prop,
            terra_core::PlayerUuid::from_u128(205)
        )
        .unwrap()
    );
    let tree = journal.gameplay().trees.get(prop.id.0).unwrap().clone();
    let plan = plan(&chunks, &journal, piece.anchor, piece.material, 0).unwrap();
    assert!(
        clearance(&plan, &chunks, &journal)
            .unwrap()
            .iter()
            .any(|entry| entry.prop.id == prop.id)
    );
    assert!(commit(&mut chunks, &mut journal, &plan).unwrap());
    let retired = journal.gameplay().trees.get(prop.id.0).unwrap();
    assert_eq!(retired.stage, terra_core::TreeStage::Felled);
    assert_eq!(retired.revision, tree.revision + 1);
    assert_eq!(retired.root, tree.root);
    assert_eq!(retired.species, tree.species);
    assert!(journal.prop_removed(prop.section, prop.id));
}

#[test]
fn connection_sites_cover_complete_modules_and_a_nearby_curve_hit_can_have_a_distant_terminal_port()
{
    let source = Piece::new([0, 16, -20], rails::STRAIGHT, 0).unwrap();
    let target = Piece::new([0, 16, 0], rails::CURVE_LARGE, 0).unwrap();
    let mut chunks = ChunkStore::default();
    let mut journal = EditJournal::default();
    prepare(&mut chunks, source);
    prepare(&mut chunks, target);
    install(&mut chunks, &mut journal, source);
    install(&mut chunks, &mut journal, target);
    let start = Endpoint {
        anchor: source.anchor,
        port: 1,
    };
    let mut chosen = None;
    'cells: for cell in cells(target) {
        for bounds in cell_boxes(cell, target.material, object_state(target, cell)).iter() {
            let point: [f64; 3] = std::array::from_fn(|axis| {
                cell[axis] as f64 + (bounds.minimum[axis] + bounds.maximum[axis]) * 0.5
            });
            let eye = [point[0] + 0.5, point[1] + 2.0, point[2]];
            let delta: [f64; 3] = std::array::from_fn(|axis| point[axis] - eye[axis]);
            let length = delta.iter().map(|value| value * value).sum::<f64>().sqrt();
            let direction = delta.map(|value| value / length);
            let Some(hit) = crate::interaction_shapes::raycast_solid(
                &chunks,
                &journal,
                &Default::default(),
                eye,
                direction,
                4.5,
            ) else {
                continue;
            };
            if owner_at(&journal, hit.position) != Some(target) {
                continue;
            }
            let look = [
                (-direction[0]).atan2(-direction[2]) as f32,
                direction[1].asin() as f32,
            ];
            let Ok(plan) = proposal_connection(
                &chunks,
                &journal,
                start,
                hit,
                look,
                terra_core::WoodSpecies::Oak,
            ) else {
                continue;
            };
            let port = connection_target(&plan).unwrap();
            let distance = (0..3)
                .map(|axis| (port.position.position()[axis] - eye[axis]).powi(2))
                .sum::<f64>()
                .sqrt();
            if hit.distance <= 4.5 && distance > 4.5 {
                chosen = Some((hit, plan, distance));
                break 'cells;
            }
        }
    }
    let (hit, plan, distance) =
        chosen.expect("a real middle-of-curve hit can be near while its selected port is distant");
    assert!(hit.distance <= 4.5 && distance > 4.5);
    assert!(
        connection_target(&plan)
            .unwrap()
            .connects(target.ports()[0])
            || connection_target(&plan)
                .unwrap()
                .connects(target.ports()[1])
    );
    let sites = connection_sites(&journal, start, hit).unwrap();
    let expected: std::collections::BTreeSet<_> =
        cells(source).into_iter().chain(cells(target)).collect();
    assert_eq!(sites, expected.into_iter().collect::<Vec<_>>());
    let missing = cells(target)
        .into_iter()
        .find(|cell| *cell != target.anchor && *cell != hit.position)
        .unwrap();
    journal.record(missing, 0).unwrap();
    assert!(connection_sites(&journal, start, hit).is_none());
}

#[path = "curve_placement_tests.rs"]
mod curve_placement_tests;

#[path = "context_tests.rs"]
mod context_tests;

#[path = "seed_tests.rs"]
mod seed_tests;

#[path = "build_reach_tests.rs"]
mod build_reach_tests;
