use super::*;
use crate::{edits::BlockObjectState, objects::windows::Group, streaming::LoadedSection};
use terra_voxel::{BlockSection, SectionPos, global_to_section};

#[test]
fn merged_window_collision_picking_and_loaded_owner_halo_match_each_real_cell() {
    for material in 68..=70 {
        for (width, height) in [(2, 1), (1, 2), (2, 2)] {
            let group = Group {
                anchor: [8, 4, 8],
                material,
                facing: 0,
                width,
                height,
            };
            let mut chunks = ChunkStore::default();
            let mut data = BlockSection::empty(SectionPos::new(0, 0, 0)).unwrap();
            let mut journal = EditJournal::default();
            for p in group.cells() {
                data.set_block(global_to_section(p).1, material).unwrap();
                journal.record(p, material).unwrap();
                journal
                    .set_object_state(p, Some(BlockObjectState::default()))
                    .unwrap();
            }
            chunks.loaded.insert(
                data.position(),
                LoadedSection {
                    data,
                    mesh: None,
                    mesh_epoch: 0,
                    water_epoch: 0,
                    water: None,
                    props: Vec::new(),
                },
            );
            let properties = BlockPropertiesCatalog::default();
            for p in group.cells() {
                let origin = [p[0] as f64 + 0.45, p[1] as f64 + 0.35, 6.0];
                let hit =
                    raycast(&chunks, &journal, &properties, origin, [0.0, 0.0, 1.0], 4.0).unwrap();
                assert_eq!(
                    hit.position, p,
                    "pick must target the struck cell rather than whichever group owner was visited first"
                );
            }
            for p in group.cells() {
                journal
                    .set_object_state(
                        p,
                        Some(BlockObjectState {
                            facing: 4,
                            ..Default::default()
                        }),
                    )
                    .unwrap();
            }
            let min = [8.30, 4.20, 8.40];
            let max = [8.70, 4.70, 8.60];
            for p in group.cells() {
                let mut boxes = Vec::new();
                assert!(visit_boxes(&chunks, &journal, &properties, p, |b| boxes.push(b)));
                assert!(
                    boxes.iter().all(|b| !b.intersects(min, max)),
                    "open glass is not a full invisible wall"
                );
                assert!(boxes.len() < 32);
            }
        }
    }
}
