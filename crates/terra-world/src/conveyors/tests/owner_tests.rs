use super::*;

#[test]
fn four_adjacent_shafts_use_the_selected_endpoints_and_an_independent_belt_owner() {
    let (mut chunks, mut journal) = fixture();
    let cells = [[-1, 4, 4], [-1, 4, 5], [-1, 4, 6], [-1, 4, 7]];
    for cell in cells {
        shaft_at(&mut chunks, &mut journal, cell, 0);
    }
    let belt_item = ItemKind::TintedBlock {
        material: 148,
        wood: WoodSpecies::Cherry,
        color: [17, 83, 201],
    };
    let hit = crate::interaction_shapes::raycast(
        &chunks,
        &journal,
        &BlockPropertiesCatalog::default(),
        [-3.0, 4.5, 7.5],
        [1.0, 0.0, 0.0],
        6.0,
    )
    .unwrap();
    assert_eq!(
        hit.position, cells[3],
        "the last actual shaft is picked, not the second shaft"
    );
    let plan = layout(&chunks, &journal, cells[0], hit.position, belt_item).unwrap();
    let span = proposed_span(&plan).unwrap();
    assert_eq!((span.start, span.end, span.length), (cells[0], cells[3], 2));
    assert!(commit(&mut chunks, &mut journal, &plan).unwrap());
    for cell in cells {
        assert_eq!(chunks.block_at(cell), Some(144));
        assert_eq!(span_at(&journal, cell), Some(span));
        assert_eq!(journal.object_state(cell).unwrap().wood, WoodSpecies::Birch);
    }
    assert_eq!(item(span, &journal), belt_item);
    power(span, &mut journal);
    let point = segment_at(&journal, cells[1])
        .unwrap()
        .drop_center()
        .unwrap();
    assert!(journal.gameplay_mut().drop_stack(
        point,
        ItemStack {
            item: ItemKind::Block(3),
            count: 9
        },
        [0.0; 3],
        0.0
    ));
    transport(
        &mut Transport::default(),
        &chunks,
        &mut journal,
        &BlockPropertiesCatalog::default(),
        0.1,
    );
    assert!(journal.gameplay().drops[0].position[2] > point[2]);
    let dye = dye_layout(
        &chunks,
        &journal,
        cells[2],
        terra_core::beds::DyeColor::Rgb([3, 12, 254]),
    )
    .unwrap();
    assert!(commit(&mut chunks, &mut journal, &dye).unwrap());
    let dyed = item(span, &journal);
    assert_eq!(terra_core::tints::color(dyed), Some([3, 12, 254]));
    assert_eq!(dyed.wood_species(), Some(WoodSpecies::Cherry));
    let mut replica = EditJournal::default();
    for section in journal.changed_sections() {
        replica
            .import_delta(journal.section_delta(section))
            .unwrap();
    }
    assert_eq!(span_at(&replica, cells[2]), Some(span));
    assert_eq!(item(span, &replica), dyed);
    let removal = removal_layout(&chunks, &journal, span, Some(cells[1])).unwrap();
    assert!(commit(&mut chunks, &mut journal, &removal).unwrap());
    assert!(journal.belt_owner(span.controller).is_none());
    for cell in cells {
        assert_eq!(
            chunks.block_at(cell),
            Some(if cell == cells[1] { 0 } else { 144 })
        );
        assert!(span_at(&journal, cell).is_none());
        if cell != cells[1] {
            assert_eq!(journal.object_state(cell).unwrap().peer, None);
        }
    }
}

#[test]
fn shafts_can_replace_every_belt_cell_including_legacy_controller_in_all_span_planes() {
    for (start, end, facing) in [
        ([4, 4, 4], [4, 4, 7], 0),
        ([4, 4, 4], [4, 7, 7], 0),
        ([4, 4, 4], [7, 4, 7], 4),
    ] {
        let (mut chunks, mut journal) = fixture();
        let span = connect(&mut chunks, &mut journal, start, end, facing);
        // Existing saves have only the original 148 controller and peer chain.
        journal.set_belt_owner(span.controller, None).unwrap();
        journal
            .set_tint(span.controller, Some([71, 93, 184]))
            .unwrap();
        let belt_item = item(span, &journal);
        for position in span.cells() {
            let hit = terra_voxel::blocks::BlockHit {
                position,
                normal: [0, 1, 0],
                distance: 1.0,
            };
            assert_eq!(
                crate::targeting::item_placement_with_state(&chunks, &journal, hit, 144),
                Some((position, 148))
            );
            let shaft_item = ItemKind::for_wood(144, WoodSpecies::Spruce);
            let plan = crate::placement::propose(
                &chunks,
                &journal,
                hit,
                shaft_item,
                ([4.5, 10.0, 4.5], [0.0, -1.0, 0.0]),
                [0.0, -1.4],
            )
            .unwrap();
            assert!(commit(&mut chunks, &mut journal, &plan).unwrap());
            assert_eq!(span_at(&journal, position), Some(span));
            assert_eq!(
                journal.object_state(position).unwrap().wood,
                WoodSpecies::Spruce
            );
            assert_eq!(journal.object_state(position).unwrap().facing, facing);
            assert_eq!(item(span, &journal), belt_item);
        }
        assert!(span.cells().all(|p| chunks.block_at(p) == Some(144)));
        assert!(span.loaded(&chunks, &journal));
        let section = global_to_section(span.controller).0;
        let loaded = chunks.loaded.remove(&section).unwrap();
        assert!(!span.loaded(&chunks, &journal));
        assert!(
            insert_shaft_layout(&chunks, &journal, span.controller, ItemKind::Block(145)).is_err()
        );
        chunks.loaded.insert(section, loaded);
        let mut replica = EditJournal::default();
        for owner in journal.changed_sections() {
            replica.import_delta(journal.section_delta(owner)).unwrap();
        }
        assert_eq!(span_at(&replica, span.controller), Some(span));
        assert_eq!(item(span, &replica), belt_item);
    }
}

#[test]
fn malformed_owner_deltas_and_late_insertion_overflow_are_atomic() {
    let (mut chunks, mut journal) = fixture();
    let span = connect(&mut chunks, &mut journal, [4, 4, 4], [4, 4, 7], 0);
    let section = global_to_section(span.controller).0;
    let before = journal.section_delta(section);
    for invalid in [
        BeltOwner {
            item: ItemKind::Block(3),
            ..before.belts[0].1
        },
        BeltOwner {
            end: [4, 5, 7],
            ..before.belts[0].1
        },
        BeltOwner {
            start: span.controller,
            ..before.belts[0].1
        },
    ] {
        let mut delta = before.clone();
        delta.belts[0].1 = invalid;
        assert!(EditJournal::default().import_delta(delta).is_err());
    }
    let mut duplicate = before.clone();
    duplicate.belts.push(duplicate.belts[0]);
    assert!(EditJournal::default().import_delta(duplicate).is_err());
    let plan =
        insert_shaft_layout(&chunks, &journal, span.controller, ItemKind::Block(144)).unwrap();
    let mut exhausted = before.clone();
    exhausted.revision = u64::MAX;
    journal.import_delta(exhausted.clone()).unwrap();
    assert!(commit(&mut chunks, &mut journal, &plan).is_err());
    assert_eq!(chunks.block_at(span.controller), Some(148));
    assert_eq!(journal.section_delta(section), exhausted);
}
