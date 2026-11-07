use super::*;
use crate::{
    rails,
    streaming::LoadedSection,
    targeting::{self, WorldTarget},
};
use terra_core::rails::{Piece, STRAIGHT, SWITCH_RIGHT};
use terra_voxel::{BlockSection, global_to_section};

fn block(chunks: &mut ChunkStore, cell: [i64; 3], material: u16) {
    let (section, local) = global_to_section(cell);
    chunks
        .loaded
        .entry(section)
        .or_insert_with(|| LoadedSection {
            data: BlockSection::empty(section).unwrap(),
            mesh: None,
            mesh_epoch: 0,
            water_epoch: 0,
            water: None,
            props: vec![],
        })
        .data
        .set_block(local, material)
        .unwrap();
}

#[test]
fn rail_rays_keep_the_visible_secondary_slice_even_when_the_anchor_section_is_unloaded() {
    for material in [STRAIGHT, SWITCH_RIGHT] {
        for unload_anchor in [false, true] {
            let piece = Piece::new([-1, 16, -17], material, 0).unwrap();
            let secondary = [0, 16, -17];
            let mut chunks = ChunkStore::default();
            let mut journal = EditJournal::default();
            for cell in rails::cells(piece) {
                block(&mut chunks, cell, 0);
            }
            for support in piece.supports() {
                block(
                    &mut chunks,
                    std::array::from_fn(|axis| {
                        piece.anchor[axis] + i64::from(support.support[axis])
                    }),
                    3,
                );
            }
            let plan = rails::plan(&chunks, &journal, piece.anchor, material, 0).unwrap();
            assert!(rails::commit(&mut chunks, &mut journal, &plan).unwrap());
            assert!(journal.object_state(secondary).unwrap().secondary);
            assert_ne!(
                global_to_section(piece.anchor).0,
                global_to_section(secondary).0
            );
            if unload_anchor {
                chunks.loaded.remove(&global_to_section(piece.anchor).0);
            }
            let properties = BlockPropertiesCatalog::default();
            let origin = [0.5, 18.0, -16.5];
            let direction = [0.0, -1.0, 0.0];
            let results = [
                targeting::pick_world_with_context(
                    &chunks,
                    &journal,
                    &properties,
                    origin,
                    direction,
                    crate::props::PropPose::Rest,
                ),
                targeting::pick_world_for_mining(
                    &chunks,
                    &journal,
                    &properties,
                    origin,
                    direction,
                    crate::props::PropPose::Rest,
                ),
                targeting::pick_world_for_placement(
                    &chunks,
                    &journal,
                    &properties,
                    origin,
                    direction,
                    crate::props::PropPose::Rest,
                ),
            ];
            for (block, selected) in results {
                let hit = block.expect("the visible rail head has a real collision shape");
                assert_eq!(
                    hit.position, secondary,
                    "material {material}, unloaded {unload_anchor}"
                );
                assert_eq!(hit.normal, [0, 1, 0]);
                assert!(
                    (hit.distance
                        - (2.0 - terra_core::rails::DECK_HEIGHT - terra_core::rails::RAIL_HEIGHT))
                        .abs()
                        < 1.0e-8
                );
                assert!(
                    matches!(selected, Some(WorldTarget::Block(hit)) if hit.position == secondary)
                );
                assert_eq!(rails::owner_at(&journal, hit.position), Some(piece));
            }
            assert!(
                rails::complete(&journal, piece),
                "picking cannot alter module ownership"
            );
        }
    }
}
