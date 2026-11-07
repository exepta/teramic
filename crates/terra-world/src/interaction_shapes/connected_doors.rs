use super::*;
use crate::{edits::BlockObjectState, streaming::LoadedSection};
use terra_voxel::{BlockSection, SectionPos, global_to_section};

#[test]
fn double_door_picking_targets_the_struck_leaf_and_open_collision_has_no_middle_frame() {
    let geometry = crate::objects::windows::Group {
        anchor: [-1, 15, -1],
        material: 65,
        facing: 0,
        width: 2,
        height: 2,
    };
    let mut chunks = ChunkStore::default();
    let mut journal = EditJournal::default();
    for x in -1..=0 {
        for y in 0..=1 {
            for z in -1..=0 {
                let section = SectionPos::new(x, y, z);
                chunks.loaded.insert(
                    section,
                    LoadedSection {
                        data: BlockSection::empty(section).unwrap(),
                        mesh: None,
                        mesh_epoch: 0,
                        water_epoch: 0,
                        water: None,
                        props: Vec::new(),
                    },
                );
            }
        }
    }
    for p in geometry.cells() {
        chunks
            .loaded
            .get_mut(&global_to_section(p).0)
            .unwrap()
            .data
            .set_block(global_to_section(p).1, 65)
            .unwrap();
        journal.record(p, 65).unwrap();
        let top = p[1] > geometry.anchor[1];
        journal
            .set_object_state(
                p,
                Some(BlockObjectState {
                    peer: Some([p[0], p[1] + if top { -1 } else { 1 }, p[2]]),
                    secondary: top,
                    ..Default::default()
                }),
            )
            .unwrap();
    }
    let properties = BlockPropertiesCatalog::default();
    for p in geometry.cells() {
        let hit = raycast(
            &chunks,
            &journal,
            &properties,
            [p[0] as f64 + 0.4, p[1] as f64 + 0.4, -3.0],
            [0.0, 0.0, 1.0],
            4.0,
        )
        .unwrap();
        assert_eq!(
            hit.position, p,
            "mining must select the actual struck half/leaf"
        );
    }
    // Restore the closed pose for a collision baseline, then open all4 together.
    for open in [false, true] {
        for p in geometry.cells() {
            let state = journal.object_state(p).unwrap();
            journal
                .set_object_state(
                    p,
                    Some(BlockObjectState {
                        facing: if open { 4 } else { 0 },
                        ..state
                    }),
                )
                .unwrap();
        }
        let mut intersects = false;
        assert!(visit_collision_boxes(
            &chunks,
            &journal,
            &properties,
            geometry.anchor,
            |bounds| intersects |= bounds.intersects([-0.6, 15.2, -0.65], [0.6, 16.8, -0.35])
        ));
        assert_eq!(intersects, !open);
    }
}
