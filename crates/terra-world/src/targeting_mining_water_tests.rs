use super::*;
use crate::{
    edits::{BlockObjectState, EditJournal},
    streaming::LoadedSection,
};
use terra_core::{block_properties::BlockPropertiesCatalog, plumbing};
use terra_voxel::{BlockSection, global_to_section};

#[test]
fn break_rays_reach_underwater_beds_and_keep_actual_machine_occlusion() {
    for p in [[4, 3, 4], [-4, 3, -4]] {
        let owner = global_to_section(p).0;
        let mut data = BlockSection::empty(owner).unwrap();
        data.set_block(global_to_section(p).1, 3).unwrap();
        for dy in 1..=3 {
            data.set_block(global_to_section([p[0], p[1] + dy, p[2]]).1, 27)
                .unwrap();
        }
        let mut chunks = ChunkStore::default();
        chunks.loaded.insert(
            owner,
            LoadedSection {
                data,
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: vec![],
            },
        );
        let mut journal = EditJournal::default();
        let properties = BlockPropertiesCatalog::default();
        let direction = [0., -1., 0.];
        let origin = [p[0] as f64 + 0.5, p[1] as f64 + 4.5, p[2] as f64 + 0.5];
        assert_eq!(
            pick_world(&chunks, origin, direction).0.unwrap().position,
            [p[0], p[1] + 3, p[2]]
        );
        assert_eq!(
            pick_world_for_mining(
                &chunks,
                &journal,
                &properties,
                origin,
                direction,
                PropPose::Rest
            )
            .0
            .unwrap()
            .position,
            p
        );
        let pipe = [p[0], p[1] + 2, p[2]];
        chunks
            .loaded
            .get_mut(&owner)
            .unwrap()
            .data
            .set_block(global_to_section(pipe).1, plumbing::WOOD_PIPE)
            .unwrap();
        journal.record(pipe, plumbing::WOOD_PIPE).unwrap();
        journal
            .set_object_state(
                pipe,
                Some(BlockObjectState {
                    facing: 4,
                    ..Default::default()
                }),
            )
            .unwrap();
        assert_eq!(
            pick_world_for_mining(
                &chunks,
                &journal,
                &properties,
                origin,
                direction,
                PropPose::Rest
            )
            .0
            .unwrap()
            .position,
            pipe
        );
        let beside = [p[0] as f64 + 0.05, origin[1], p[2] as f64 + 0.05];
        assert_eq!(
            pick_world_for_mining(
                &chunks,
                &journal,
                &properties,
                beside,
                direction,
                PropPose::Rest
            )
            .0
            .unwrap()
            .position,
            p
        );
        chunks.loaded.clear();
        assert_eq!(
            pick_world_for_mining(
                &chunks,
                &journal,
                &properties,
                origin,
                direction,
                PropPose::Rest
            ),
            (None, None)
        );
    }
}

#[test]
fn submerged_construction_targets_the_bed_without_replacing_the_eyes_water_cell() {
    for p in [[4, 3, 4], [-4, 3, -4]] {
        let owner = global_to_section(p).0;
        let mut data = BlockSection::empty(owner).unwrap();
        data.set_block(global_to_section(p).1, 3).unwrap();
        for dy in 1..=3 {
            data.set_block(global_to_section([p[0], p[1] + dy, p[2]]).1, 27)
                .unwrap();
        }
        let mut chunks = ChunkStore::default();
        chunks.loaded.insert(
            owner,
            LoadedSection {
                data,
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: vec![],
            },
        );
        let journal = EditJournal::default();
        let properties = BlockPropertiesCatalog::default();
        let eye = [p[0] as f64 + 0.5, p[1] as f64 + 3.5, p[2] as f64 + 0.5];
        let direction = [0., -1., 0.];
        let old = pick_world_with_context(
            &chunks,
            &journal,
            &properties,
            eye,
            direction,
            PropPose::Rest,
        )
        .0
        .unwrap();
        assert_eq!(old.position, [p[0], p[1] + 3, p[2]]);
        assert_eq!(old.distance, 0.);
        let (hit, target) = pick_world_for_placement(
            &chunks,
            &journal,
            &properties,
            eye,
            direction,
            PropPose::Rest,
        );
        let hit = hit.unwrap();
        assert_eq!(hit.position, p);
        assert_eq!(hit.normal, [0, 1, 0]);
        assert!(matches!(target, Some(WorldTarget::Block(selected)) if selected == hit));
        for material in [3, plumbing::IRON_PIPE] {
            assert_eq!(
                item_placement_with_state(&chunks, &journal, hit, material),
                Some(([p[0], p[1] + 1, p[2]], 27))
            );
        }
        // A real solid along the same ray still occludes the farther bed.
        let wall = [p[0], p[1] + 2, p[2]];
        chunks
            .loaded
            .get_mut(&owner)
            .unwrap()
            .data
            .set_block(global_to_section(wall).1, 1)
            .unwrap();
        assert_eq!(
            pick_world_for_placement(
                &chunks,
                &journal,
                &properties,
                eye,
                direction,
                PropPose::Rest
            )
            .0
            .unwrap()
            .position,
            wall
        );
        chunks.loaded.clear();
        assert_eq!(
            pick_world_for_placement(
                &chunks,
                &journal,
                &properties,
                eye,
                direction,
                PropPose::Rest
            ),
            (None, None)
        );
    }
}
