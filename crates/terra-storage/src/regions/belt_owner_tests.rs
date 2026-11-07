use super::*;
use terra_core::{ItemKind, WoodSpecies};
use terra_world::edits::BlockObjectState;

const START: [i64; 3] = [-1, 15, -514];
const OWNER: [i64; 3] = [-1, 15, -513];
const GUIDE: [i64; 3] = [-1, 15, -512];
const END: [i64; 3] = [-1, 15, -511];

fn fixture() -> (EditJournal, BeltOwner) {
    let mut journal = EditJournal::default();
    for (position, material, peer) in [
        (START, 144, OWNER),
        (OWNER, 144, END),
        (GUIDE, 145, OWNER),
        (END, 146, START),
    ] {
        journal.record(position, material).unwrap();
        journal
            .set_object_state(
                position,
                Some(BlockObjectState {
                    peer: Some(peer),
                    wood: WoodSpecies::Spruce,
                    ..Default::default()
                }),
            )
            .unwrap();
    }
    let owner = BeltOwner {
        start: START,
        end: END,
        item: ItemKind::TintedBlock {
            material: 148,
            wood: WoodSpecies::Cherry,
            color: [27, 171, 91],
        },
    };
    journal.set_belt_owner(OWNER, Some(owner)).unwrap();
    (journal, owner)
}

#[test]
fn rga8_preserves_all_shaft_belt_frame_rgb_and_revisions_across_negative_region_boundary() {
    let root = std::env::temp_dir().join(format!("terra-belt-owner-{}", PlayerUuid::new_v4()));
    let world = WorldId(1600);
    let (mut journal, owner) = fixture();
    // Adjacent ordinary RGB data must still use the frozen RGA7 envelope.
    journal.record([512, 17, 0], 103).unwrap();
    journal.set_tint([512, 17, 0], Some([77, 81, 99])).unwrap();
    let rgb_cell = [-8, 12, -517];
    let filter_cell = [-7, 12, -517];
    let mixer_cell = [-6, 12, -517];
    let filter = terra_core::logistics::FilterConfig {
        item: Some(owner.item),
        allowed: false,
    };
    for (cell, material) in [(rgb_cell, 103), (filter_cell, 157), (mixer_cell, 156)] {
        journal.record(cell, material).unwrap();
        if material != 103 {
            journal
                .set_object_state(cell, Some(BlockObjectState::default()))
                .unwrap();
        }
    }
    journal.set_tint(rgb_cell, Some([14, 121, 207])).unwrap();
    journal.set_logistics_filter(filter_cell, filter).unwrap();
    journal.set_mixer(mixer_cell, Default::default()).unwrap();
    let expected: Vec<_> = journal
        .changed_sections()
        .map(|p| journal.section_delta(p))
        .collect();
    for _ in 0..3 {
        save_edits(&root, world, &journal).unwrap();
        let loaded = load_edits(&root, world).unwrap();
        for delta in &expected {
            assert_eq!(loaded.section_delta(delta.position), *delta);
        }
        assert_eq!(loaded.belt_owner(OWNER), Some(owner));
        for cell in [START, OWNER, GUIDE, END] {
            assert_eq!(loaded.block(cell), journal.block(cell));
            assert_eq!(loaded.object_state(cell), journal.object_state(cell));
            assert_eq!(loaded.object_state(cell).unwrap().wood, WoodSpecies::Spruce);
        }
        assert_eq!(loaded.tint_at(OWNER), None);
        assert_eq!(loaded.tint_at([512, 17, 0]), Some([77, 81, 99]));
        assert_eq!(loaded.tint_at(rgb_cell), Some([14, 121, 207]));
        assert_eq!(loaded.logistics_filter_at(filter_cell), Some(&filter));
        assert_eq!(loaded.mixer_at(mixer_cell), journal.mixer_at(mixer_cell));
        journal = loaded;
    }
    assert_eq!(
        read(&root.join(region_name([-1, -2]))).unwrap().get(12..16),
        Some(b"RGA8".as_slice())
    );
    assert_eq!(
        read(&root.join(region_name([1, 0]))).unwrap().get(12..16),
        Some(b"RGA7".as_slice())
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn removing_owner_does_not_leave_belt_metadata_or_upgrade_unrelated_legacy_regions() {
    let root = std::env::temp_dir().join(format!("terra-belt-remove-{}", PlayerUuid::new_v4()));
    let world = WorldId(1601);
    let (mut journal, _) = fixture();
    save_edits(&root, world, &journal).unwrap();
    journal.set_belt_owner(OWNER, None).unwrap();
    save_edits(&root, world, &journal).unwrap();
    let loaded = load_edits(&root, world).unwrap();
    assert_eq!(loaded.belt_owner(OWNER), None);
    assert!(
        loaded
            .section_delta(terra_voxel::global_to_section(OWNER).0)
            .belts
            .is_empty()
    );
    assert_ne!(
        read(&root.join(region_name([-1, -2]))).unwrap().get(12..16),
        Some(b"RGA8".as_slice())
    );
    assert_eq!(loaded.block(OWNER), Some(144));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn frozen_section_payload_does_not_absorb_appended_belt_owner_metadata() {
    let (journal, _) = fixture();
    let delta = journal.section_delta(terra_voxel::global_to_section(OWNER).0);
    assert_eq!(delta.belts.len(), 1);
    let frozen: StoredSectionDelta = delta.into();
    let restored: SectionDelta =
        decode::<StoredSectionDelta>(b"RGN3", &encode(b"RGN3", &frozen).unwrap())
            .unwrap()
            .into();
    assert!(restored.belts.is_empty());
    assert_eq!(restored.objects.len(), 2);
    assert_eq!(restored.blocks.len(), 2);
}

#[test]
fn rga8_rejects_duplicate_misplaced_invalid_and_orphaned_owner_records() {
    for corruption in 0..5 {
        let root =
            std::env::temp_dir().join(format!("terra-belt-invalid-{}", PlayerUuid::new_v4()));
        let world = WorldId(1602);
        let (journal, _) = fixture();
        save_edits(&root, world, &journal).unwrap();
        let path = root.join(region_name([-1, -2]));
        let mut saved: BeltRegion = decode(b"RGA8", &read(&path).unwrap()).unwrap();
        match corruption {
            0 => saved.belts.push(saved.belts[0]),
            1 => saved.belts[0].0[0] += 512,
            2 => saved.belts[0].1.item = ItemKind::Block(3),
            3 => saved
                .prior
                .prior
                .prior
                .prior
                .prior
                .prior
                .prior
                .prior
                .prior
                .region
                .sections
                .clear(),
            4 => {
                let owner_section = terra_voxel::global_to_section(OWNER).0;
                let section = saved
                    .prior
                    .prior
                    .prior
                    .prior
                    .prior
                    .prior
                    .prior
                    .prior
                    .prior
                    .region
                    .sections
                    .iter_mut()
                    .find(|section| section.position == owner_section)
                    .unwrap();
                section.objects.clear();
            }
            _ => unreachable!(),
        }
        fs::write(&path, encode(b"RGA8", &saved).unwrap()).unwrap();
        assert!(
            load_edits(&root, world).is_err(),
            "accepted corruption {corruption}"
        );
        fs::remove_dir_all(root).unwrap();
    }
}
