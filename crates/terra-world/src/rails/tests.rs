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
    for support in piece.supports() {
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
                assert!(!authored.is_empty());
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
    block(&mut chunks, support, 0);
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
            cell_boxes(cell, piece.material, object_state(piece, cell))
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
fn placed_props_cannot_be_overwritten_and_a_saved_slab_floor_cannot_rotate_out_of_support() {
    let piece = Piece::new([0, 16, 0], rails::STRAIGHT, 0).unwrap();
    let mut chunks = ChunkStore::default();
    let mut journal = EditJournal::default();
    prepare(&mut chunks, piece);
    let support = [0, 15, 0];
    let prop = crate::props::WorldProp {
        id: crate::props::PropId(620),
        kind: crate::props::PropKind::SmallStones,
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
    assert!(!placement::support_state_allowed(
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
fn sleeper_support_is_protected_even_when_its_rail_section_is_unloaded() {
    let piece = Piece::new([-17, 16, -17], rails::SLOPE_ENTRY, 0).unwrap();
    let mut chunks = ChunkStore::default();
    let mut journal = EditJournal::default();
    prepare(&mut chunks, piece);
    install(&mut chunks, &mut journal, piece);
    chunks.loaded.clear();
    for point in piece.supports() {
        let support =
            std::array::from_fn(|axis| piece.anchor[axis] + i64::from(point.support[axis]));
        assert!(!support_edit_allowed(&chunks, &journal, support, 0));
        assert!(support_edit_allowed(&chunks, &journal, support, 3));
    }
    assert!(support_edit_allowed(&chunks, &journal, [-500, 0, -500], 0));
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
