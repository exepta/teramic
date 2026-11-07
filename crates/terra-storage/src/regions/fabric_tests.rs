use super::*;
use terra_core::{ItemKind, WoodSpecies, beds::DyeColor, logistics::FilterConfig};
use terra_world::edits::BlockObjectState;

#[test]
fn rga7_preserves_rgb_fabrics_ghost_filters_and_section_revisions_on_repeated_reload() {
    let root = std::env::temp_dir().join(format!("terra-fabrics-{}", PlayerUuid::new_v4()));
    let world = WorldId(1580);
    let rgb = [37, 181, 204];
    let mut journal = EditJournal::default();
    let cells = [[-17, 15, -1], [-16, 16, -1], [-15, 17, -1], [-14, 18, -1]];
    for (position, material) in cells.into_iter().zip([103, 106, 148, 71]) {
        journal.record(position, material).unwrap();
        journal.set_tint(position, Some(rgb)).unwrap();
        if material == 71 {
            let peer = [position[0], position[1], position[2] + 1];
            journal.record(peer, material).unwrap();
            journal
                .set_object_state(
                    position,
                    Some(BlockObjectState {
                        peer: Some(peer),
                        ..Default::default()
                    }),
                )
                .unwrap();
            journal
                .set_object_state(
                    peer,
                    Some(BlockObjectState {
                        peer: Some(position),
                        secondary: true,
                        ..Default::default()
                    }),
                )
                .unwrap();
            journal.set_tint(peer, Some(rgb)).unwrap();
        }
    }
    let filter = [-13, 18, -1];
    journal.record(filter, 157).unwrap();
    journal
        .set_object_state(filter, Some(BlockObjectState::default()))
        .unwrap();
    let selection = FilterConfig {
        item: Some(ItemKind::TintedBlock {
            material: 148,
            wood: WoodSpecies::Cherry,
            color: rgb,
        }),
        allowed: false,
    };
    journal.set_logistics_filter(filter, selection).unwrap();
    let other_filters = [([-12, 18, -1], 159), ([-11, 18, -1], 157)];
    for (cell, material) in other_filters {
        journal.record(cell, material).unwrap();
        journal
            .set_object_state(cell, Some(BlockObjectState::default()))
            .unwrap();
        journal.set_logistics_filter(cell, selection).unwrap();
    }
    // A prior-format mixer region and ordinary region must coexist with RGA7.
    let mixer = [512, 10, 0];
    journal.record(mixer, 156).unwrap();
    journal
        .set_object_state(mixer, Some(BlockObjectState::default()))
        .unwrap();
    journal.set_mixer(mixer, Default::default()).unwrap();
    journal.record([1024, 10, 0], 1).unwrap();
    let owners: BTreeSet<_> = cells
        .into_iter()
        .map(|p| {
            let s = terra_voxel::global_to_section(p).0;
            (s.x, s.y, s.z)
        })
        .collect();
    let expected: Vec<_> = owners
        .into_iter()
        .map(|(x, y, z)| journal.section_delta(terra_voxel::SectionPos::new(x, y, z)))
        .collect();
    for _ in 0..3 {
        save_edits(&root, world, &journal).unwrap();
        let loaded = load_edits(&root, world).unwrap();
        for delta in &expected {
            assert_eq!(loaded.section_delta(delta.position), *delta);
        }
        for cell in cells {
            assert_eq!(loaded.tint_at(cell), Some(rgb));
        }
        assert_eq!(loaded.tint_at([-14, 18, 0]), Some(rgb));
        assert_eq!(loaded.logistics_filter_at(filter), Some(&selection));
        for (cell, material) in other_filters {
            assert_eq!(loaded.block(cell), Some(material));
            assert_eq!(loaded.logistics_filter_at(cell), Some(&selection));
        }
        assert!(loaded.mixer_at(mixer).is_some());
        assert_eq!(loaded.block([1024, 10, 0]), Some(1));
        journal = loaded;
    }
    assert_eq!(
        read(&root.join(region_name([-1, -1]))).unwrap().get(12..16),
        Some(b"RGA7".as_slice())
    );
    assert_eq!(
        read(&root.join(region_name([1, 0]))).unwrap().get(12..16),
        Some(b"RGA6".as_slice())
    );
    assert_eq!(
        ItemKind::DyedBed {
            material: 71,
            color: DyeColor::Rgb(rgb)
        }
        .block_material(),
        Some(71)
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn previous_section_binary_layout_is_frozen_and_rgb_is_not_silently_inserted_into_it() {
    let mut journal = EditJournal::default();
    let position = [-1, 17, 15];
    journal.record(position, 103).unwrap();
    let owner = terra_voxel::global_to_section(position).0;
    let legacy: StoredSectionDelta = journal.section_delta(owner).into();
    let bytes = encode(b"RGN3", &legacy).unwrap();
    let decoded: StoredSectionDelta = decode(b"RGN3", &bytes).unwrap();
    let mut restored = EditJournal::default();
    restored.import_delta(decoded.into()).unwrap();
    assert_eq!(restored.section_delta(owner), journal.section_delta(owner));
    journal.set_tint(position, Some([7, 8, 9])).unwrap();
    let current = journal.section_delta(owner);
    assert!(!current.tints.is_empty());
    let frozen: StoredSectionDelta = current.into();
    let restored: SectionDelta =
        decode::<StoredSectionDelta>(b"RGN3", &encode(b"RGN3", &frozen).unwrap())
            .unwrap()
            .into();
    assert!(restored.tints.is_empty());
}

#[test]
fn rga7_roundtrips_rgb_wool_shapes_and_stone_owned_mixed_halves_without_new_ids() {
    let root = std::env::temp_dir().join(format!("terra-shape-rgb-{}", PlayerUuid::new_v4()));
    let world = WorldId(1590);
    let mut journal = EditJournal::default();
    let rgb = [13, 177, 211];
    for (position, material, facing) in [
        ([-17, 15, -1], 104, 7),
        ([-16, 16, -1], 105, 4),
        (
            [-15, 16, -1],
            85,
            terra_core::construction::merged_slab(85, 0, 105).unwrap(),
        ),
    ] {
        journal.record(position, material).unwrap();
        journal
            .set_object_state(
                position,
                Some(BlockObjectState {
                    facing,
                    ..Default::default()
                }),
            )
            .unwrap();
        journal.set_tint(position, Some(rgb)).unwrap();
    }
    for _ in 0..3 {
        save_edits(&root, world, &journal).unwrap();
        let loaded = load_edits(&root, world).unwrap();
        for position in [[-17, 15, -1], [-16, 16, -1], [-15, 16, -1]] {
            let owner = terra_voxel::global_to_section(position).0;
            assert_eq!(loaded.section_delta(owner), journal.section_delta(owner));
        }
        journal = loaded;
    }
    let mixed = [-15, 16, -1];
    journal
        .set_object_state(mixed, Some(Default::default()))
        .unwrap();
    assert_eq!(journal.tint_at(mixed), None);
    save_edits(&root, world, &journal).unwrap();
    let loaded = load_edits(&root, world).unwrap();
    let owner = terra_voxel::global_to_section(mixed).0;
    assert_eq!(loaded.section_delta(owner), journal.section_delta(owner));
    assert_eq!(loaded.tint_at([-16, 16, -1]), Some(rgb));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn legacy_filters_on_ordinary_ports_are_discarded_without_losing_filtered_variants() {
    let root = std::env::temp_dir().join(format!(
        "terra-legacy-port-filters-{}",
        PlayerUuid::new_v4()
    ));
    let world = WorldId(1731);
    let mut journal = EditJournal::default();
    let ports = [
        ([-17, 4, -1], 152),
        ([-18, 4, -1], 153),
        ([-19, 4, -1], 157),
        ([-20, 4, -1], 159),
    ];
    let filter = FilterConfig {
        item: Some(ItemKind::Block(3)),
        allowed: false,
    };
    for (p, material) in ports {
        journal.record(p, material).unwrap();
        journal
            .set_object_state(p, Some(BlockObjectState::default()))
            .unwrap();
        if terra_core::logistics::supports_filter(material) {
            journal.set_logistics_filter(p, filter).unwrap();
        }
    }
    save_edits(&root, world, &journal).unwrap();
    let path = root.join(region_name([-1, -1]));
    let mut legacy: FabricRegion = decode(b"RGA7", &read(&path).unwrap()).unwrap();
    for (p, material) in ports {
        if !terra_core::logistics::supports_filter(material) {
            legacy.filters.push((p, filter));
        }
    }
    fs::write(&path, encode(b"RGA7", &legacy).unwrap()).unwrap();
    for _ in 0..2 {
        let loaded = load_edits(&root, world).unwrap();
        for (p, material) in ports {
            assert_eq!(loaded.block(p), Some(material));
            assert_eq!(
                loaded.logistics_filter_at(p),
                terra_core::logistics::supports_filter(material).then_some(&filter)
            );
            assert!(loaded.container_at(p).is_none());
        }
        assert_eq!(loaded.logistics_filters().count(), 2);
        save_edits(&root, world, &loaded).unwrap();
    }
    let cleaned: FabricRegion = decode(b"RGA7", &read(&path).unwrap()).unwrap();
    assert_eq!(cleaned.filters.len(), 2);
    fs::remove_dir_all(root).unwrap();
}
