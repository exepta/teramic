use super::*;
use crate::streaming::{LoadedSection, commit_block_edit};
use terra_voxel::{BlockSection, SectionPos, global_to_section};

fn fixture() -> (ChunkStore, EditJournal, BlockPropertiesCatalog) {
    let mut chunks = ChunkStore::default();
    for y in -3..=2 {
        let p = SectionPos::new(-1, y, -1);
        chunks.loaded.insert(
            p,
            LoadedSection {
                data: BlockSection::empty(p).unwrap(),
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            },
        );
    }
    (
        chunks,
        EditJournal::default(),
        BlockPropertiesCatalog::load(&terra_core::asset_directory().join("terra/blocks")).unwrap(),
    )
}
fn put(chunks: &mut ChunkStore, p: [i64; 3], m: u16) {
    let (section, local) = global_to_section(p);
    chunks
        .loaded
        .get_mut(&section)
        .unwrap()
        .data
        .set_block(local, m)
        .unwrap();
}

#[test]
fn crystal_and_seagrass_picking_hits_authored_geometry_but_not_empty_cell_corners() {
    let (mut chunks, _, _) = fixture();
    for material in [50, 51, 52, 53] {
        put(&mut chunks, [-1, -2, -1], 3);
        put(&mut chunks, [-1, -1, -1], material);
        let origin = [-0.5, -0.5, 1.5];
        let hit = crate::segments::raycast(
            &chunks,
            [-1, -1, -1],
            material,
            origin,
            [0.0, 0.0, -1.0],
            5.0,
            crate::props::PropPose::Rest,
        );
        assert_eq!(
            hit.map(|h| h.position),
            Some([-1, -1, -1]),
            "material {material}"
        );
        assert!(
            crate::segments::raycast(
                &chunks,
                [-1, -1, -1],
                material,
                [-0.015, -0.5, 1.5],
                [0.0, 0.0, -1.0],
                5.0,
                crate::props::PropPose::Rest
            )
            .is_none()
        );
    }
}
fn remove(
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
    p: [i64; 3],
    properties: &BlockPropertiesCatalog,
) {
    assert!(
        commit_block_edit(
            chunks,
            journal,
            &BlockEditRequest {
                position: p,
                expected_material: chunks.block_at(p).unwrap(),
                material: 0
            },
            properties
        )
        .unwrap()
    );
}

#[test]
fn seagrass_top_is_independent_and_lower_cut_only_retires_above_across_sections() {
    let (mut chunks, mut journal, properties) = fixture();
    put(&mut chunks, [-1, -19, -1], 3);
    put(&mut chunks, [-2, -19, -1], 3);
    for y in -18..=-11 {
        put(&mut chunks, [-1, y, -1], 50);
        put(&mut chunks, [-2, y, -1], 50);
    }
    remove(&mut chunks, &mut journal, [-1, -11, -1], &properties);
    assert_eq!(chunks.block_at([-1, -12, -1]), Some(50));
    assert!(journal.gameplay().drops.is_empty());
    remove(&mut chunks, &mut journal, [-1, -17, -1], &properties);
    assert_eq!(chunks.block_at([-1, -18, -1]), Some(50));
    for y in -16..=-11 {
        assert_eq!(chunks.block_at([-1, y, -1]), Some(0));
        assert_eq!(chunks.block_at([-2, y, -1]), Some(50));
    }
    assert_eq!(journal.gameplay().drops.len(), 5);
    let mut reloaded = EditJournal::default();
    for y in -2..=0 {
        reloaded
            .import_delta(journal.section_delta(SectionPos::new(-1, y, -1)))
            .unwrap();
    }
    assert_eq!(
        reloaded
            .snapshot(SectionPos::new(-1, -2, -1))
            .unwrap()
            .get(&[-1, -12, -1]),
        Some(&0)
    );
    assert_eq!(
        reloaded
            .snapshot(SectionPos::new(-1, -1, -1))
            .unwrap()
            .get(&[-1, -17, -1]),
        Some(&0)
    );
}

#[test]
fn hanging_mineral_cut_drops_only_detached_part_and_retains_ceiling_root() {
    for material in [51, 52, 53] {
        let (mut chunks, mut journal, properties) = fixture();
        put(&mut chunks, [-1, 2, -1], 3);
        for y in -8..=1 {
            put(&mut chunks, [-1, y, -1], material);
        }
        remove(&mut chunks, &mut journal, [-1, -2, -1], &properties);
        for y in -1..=1 {
            assert_eq!(chunks.block_at([-1, y, -1]), Some(material));
        }
        for y in -8..=-2 {
            assert_eq!(chunks.block_at([-1, y, -1]), Some(0));
        }
        assert_eq!(journal.gameplay().drops.len(), 6);
        assert!(
            journal
                .gameplay()
                .drops
                .iter()
                .all(|d| d.stack.item == ItemKind::Block(material)
                    && d.velocity[1] < 0.0
                    && d.pickup_delay_seconds >= 1.0)
        );
    }
}

#[test]
fn empty_support_section_arriving_last_reconciles_unknown_anchor_without_startup_scans() {
    let (mut chunks, mut journal, properties) = fixture();
    put(&mut chunks, [-1, -1, -1], 51);
    put(&mut chunks, [-1, 0, -1], 51);
    let missing = chunks.loaded.remove(&SectionPos::new(-1, -1, -1)).unwrap();
    assert!(!unsupported(&chunks, [-1, 0, -1], 51));
    loaded(&mut chunks, &journal, SectionPos::new(-1, 0, -1));
    assert!(chunks.support_queue.is_empty());
    journal.record([-1, -2, -1], 0).unwrap();
    chunks.loaded.insert(SectionPos::new(-1, -1, -1), missing);
    loaded(&mut chunks, &journal, SectionPos::new(-1, -1, -1));
    assert!(chunks.support_queue.contains(&[-1, 0, -1]));
    while let Some(p) = chunks.support_queue.pop_front() {
        prune(&mut chunks, &mut journal, p, &properties).unwrap();
    }
    assert_eq!(chunks.block_at([-1, 0, -1]), Some(0));
    assert_eq!(journal.gameplay().drops.len(), 2);
}

#[test]
fn saturated_drop_capacity_defers_segment_removal_without_losing_items() {
    let (mut chunks, mut journal, properties) = fixture();
    for y in 0..=2 {
        put(&mut chunks, [-1, y, -1], 52);
    }
    while journal.gameplay().can_drop() {
        journal
            .gameplay_mut()
            .drop_one([-4.0; 3], ItemKind::Block(3));
    }
    journal.gameplay_mut().drops.pop();
    prune(&mut chunks, &mut journal, [-1, 0, -1], &properties).unwrap();
    assert_eq!(chunks.block_at([-1, 0, -1]), Some(0));
    assert_eq!(chunks.block_at([-1, 1, -1]), Some(52));
    assert!(!chunks.support_queue.is_empty());
    journal.gameplay_mut().drops.clear();
    while let Some(p) = chunks.support_queue.pop_front() {
        prune(&mut chunks, &mut journal, p, &properties).unwrap();
    }
    assert_eq!(chunks.block_at([-1, 2, -1]), Some(0));
    assert_eq!(journal.gameplay().drops.len(), 2);
}
