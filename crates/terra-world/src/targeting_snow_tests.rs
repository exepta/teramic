use super::*;
use crate::{
    edits::{BlockObjectState, EditJournal},
    streaming::LoadedSection,
};

fn place(chunks: &mut ChunkStore, position: [i64; 3], material: u16) {
    let (section, local) = terra_voxel::global_to_section(position);
    chunks
        .loaded
        .entry(section)
        .or_insert_with(|| LoadedSection {
            data: terra_voxel::BlockSection::empty(section).unwrap(),
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

#[test]
fn construction_replaces_thin_snow_but_uses_complete_snow_as_solid_support() {
    let position = [-1, 4, -1];
    let hit = BlockHit {
        position,
        normal: [0, 1, 0],
        distance: 2.0,
    };
    let above = [-1, 5, -1];
    let mut chunks = ChunkStore::default();
    place(&mut chunks, position, 98);
    for material in [3, 77, 78, 85, 86, 94, 102, 104, 105, 29, 71, 156] {
        assert_eq!(item_placement(&chunks, hit, material), Some((position, 98)));
    }
    assert_eq!(
        item_placement(&chunks, hit, 98),
        Some((above, 0)),
        "manual snow-layer placement keeps its own placement path"
    );
    let floor_hit = BlockHit {
        position: [-1, 3, -1],
        ..hit
    };
    place(&mut chunks, floor_hit.position, 3);
    assert_eq!(item_placement(&chunks, floor_hit, 3), Some((position, 98)));
    place(&mut chunks, position, 5);
    assert_eq!(item_placement(&chunks, hit, 3), Some((above, 0)));
    assert_eq!(
        item_placement(&chunks, hit, 5),
        Some((above, 0)),
        "manual full snow blocks remain stackable"
    );
    place(&mut chunks, above, 5);
    assert!(
        item_placement(&chunks, hit, 3).is_none(),
        "solid neighboring snow cannot be replaced either"
    );
}

#[test]
fn paired_bed_layout_replaces_both_thin_caps_across_negative_section_boundary() {
    let position = [-16, 4, -16];
    let peer = [-16, 4, -17];
    let mut chunks = ChunkStore::default();
    let mut journal = EditJournal::default();
    for cell in [position, peer] {
        place(&mut chunks, cell, 98);
        place(&mut chunks, [cell[0], cell[1] - 1, cell[2]], 3);
        journal.record(cell, 98).unwrap();
        journal
            .set_object_state(
                cell,
                Some(BlockObjectState {
                    facing: 3,
                    ..Default::default()
                }),
            )
            .unwrap();
    }
    let plan =
        crate::placement::layout(&chunks, &journal, position, 98, 71, [0.0; 2], None).unwrap();
    assert_eq!(plan.edits.len(), 2);
    assert!(
        plan.edits
            .iter()
            .all(|edit| edit.expected_material == 98 && edit.material == 71)
    );
    assert!(plan.edits.iter().any(|edit| edit.position == peer));
    crate::placement::environment(&plan, &chunks, &journal, true).unwrap();
}
