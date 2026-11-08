use super::*;
use terra_world::{edits::BlockObjectState, streaming::LoadedSection};

#[test]
fn native_wide_bogie_overhang_keeps_exact_collision_across_negative_section_seams() {
    let position = [-16, 4, -16];
    let material = terra_core::train_parts::BOGIE;
    let owner = global_to_section(position).0;
    for facing in 0..4 {
        let neighbor = owner
            .offset(if facing % 2 == 0 {
                [-1, 0, 0]
            } else {
                [0, 0, -1]
            })
            .unwrap();
        let mut chunks = ChunkStore::default();
        for section in [owner, neighbor] {
            chunks.loaded.insert(
                section,
                LoadedSection {
                    data: BlockSection::empty(section).unwrap(),
                    mesh: None,
                    mesh_epoch: 0,
                    water_epoch: 0,
                    water: None,
                    props: vec![],
                },
            );
        }
        chunks
            .loaded
            .get_mut(&owner)
            .unwrap()
            .data
            .set_block(global_to_section(position).1, material)
            .unwrap();
        let mut journal = EditJournal::default();
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
        let origin = neighbor.origin().unwrap();
        let expected: Vec<_> = terra_core::train_parts::boxes(material, facing)
            .into_iter()
            .filter_map(|mut bounds| {
                for axis in 0..3 {
                    bounds.minimum[axis] = (bounds.minimum[axis] + position[axis] as f64)
                        .max(origin[axis] as f64)
                        - origin[axis] as f64;
                    bounds.maximum[axis] = (bounds.maximum[axis] + position[axis] as f64)
                        .min(origin[axis] as f64 + 16.0)
                        - origin[axis] as f64;
                }
                (0..3)
                    .all(|axis| bounds.minimum[axis] < bounds.maximum[axis])
                    .then_some(bounds)
            })
            .collect();
        assert!(!expected.is_empty());
        let actual = model_geometry(
            &chunks.loaded[&neighbor].data,
            &chunks,
            &journal,
            &Default::default(),
        )
        .unwrap();
        assert_eq!(actual.boxes.len(), expected.len());
        // The world picker rotates about the cell center before translating;
        // Core rotates interval endpoints directly. Negative-world addition
        // differs by at most a few f64 ULPs, not by a physical profile change.
        assert!(
            expected
                .iter()
                .all(|bounds| actual.boxes.iter().any(|actual| {
                    bounds.hinged == actual.hinged
                        && (0..3).all(|axis| {
                            (bounds.minimum[axis] - actual.minimum[axis]).abs() <= 1e-12
                                && (bounds.maximum[axis] - actual.maximum[axis]).abs() <= 1e-12
                        })
                }))
        );
        assert!(
            build_collider_with_geometry(
                &chunks.loaded[&neighbor].data,
                &actual.boxes,
                &actual.triangles,
                &[material]
            )
            .unwrap()
            .is_some()
        );
    }
}
