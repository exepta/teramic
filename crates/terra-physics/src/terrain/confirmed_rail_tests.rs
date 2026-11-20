use super::*;
use std::collections::{BTreeMap, BTreeSet};
use terra_core::{
    WoodSpecies,
    block_properties::BlockPropertiesCatalog,
    block_shapes::ShapeBox,
    rails::{CurveSpec, Piece, STRAIGHT},
};
use terra_world::{edits::BlockObjectState, streaming::LoadedSection};

fn key(bounds: &ShapeBox) -> [u64; 6] {
    std::array::from_fn(|axis| {
        if axis < 3 {
            bounds.minimum[axis].to_bits()
        } else {
            bounds.maximum[axis - 3].to_bits()
        }
    })
}

fn fixture(piece: Piece) -> (ChunkStore, EditJournal, BTreeMap<SectionPos, Vec<[i64; 3]>>) {
    let mut chunks = ChunkStore::default();
    let mut journal = EditJournal::default();
    let mut sections = BTreeMap::<SectionPos, Vec<_>>::new();
    for offset in piece.footprint().iter() {
        let cell = std::array::from_fn(|axis| piece.anchor[axis] + i64::from(offset[axis]));
        let (owner, local) = global_to_section(cell);
        chunks
            .loaded
            .entry(owner)
            .or_insert_with(|| LoadedSection {
                data: BlockSection::empty(owner).unwrap(),
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            })
            .data
            .set_block(local, piece.material)
            .unwrap();
        journal.record(cell, piece.material).unwrap();
        journal
            .set_object_state(
                cell,
                Some(BlockObjectState {
                    facing: piece.encoded_facing(),
                    peer: Some(piece.anchor),
                    secondary: cell != piece.anchor,
                    wood: WoodSpecies::Spruce,
                    rail_curve: piece.curve,
                    liquid: None,
                }),
            )
            .unwrap();
        sections.entry(owner).or_default().push(cell);
    }
    (chunks, journal, sections)
}

#[test]
fn every_confirmed_rail_slice_keeps_its_collider_across_negative_seams_and_an_unloaded_anchor() {
    let properties =
        BlockPropertiesCatalog::load(&terra_core::asset_directory().join("terra/blocks")).unwrap();
    assert!(!properties.shape(STRAIGHT).is_empty());
    let ramp = CurveSpec::new([[8, 2, 0], [8, 2, 26], [8, 18, 38], [8, 18, 64]]).unwrap();
    for piece in [
        Piece::new([-17, 15, -17], STRAIGHT, 0).unwrap(),
        Piece::new_curve([-17, 15, -17], ramp).unwrap(),
    ] {
        for unloaded_anchor in [false, true] {
            let (mut chunks, journal, sections) = fixture(piece);
            let anchor_owner = global_to_section(piece.anchor).0;
            assert!(sections.len() > 1);
            assert!(sections.values().any(|cells| cells.len() > 1));
            if unloaded_anchor {
                chunks.loaded.remove(&anchor_owner).unwrap();
            }
            let mut checked = 0;
            for (owner, cells) in &sections {
                let Some(loaded) = chunks.loaded.get(owner) else {
                    assert!(unloaded_anchor && *owner == anchor_owner);
                    continue;
                };
                let origin = owner.origin().unwrap();
                let mut expected = BTreeSet::new();
                for &cell in cells {
                    let offset =
                        std::array::from_fn(|axis| (cell[axis] - piece.anchor[axis]) as i32);
                    assert!(!piece.cell_boxes(offset).is_empty());
                    for mut bounds in piece.cell_boxes(offset).iter().copied() {
                        for axis in 0..3 {
                            bounds.minimum[axis] += cell[axis] as f64;
                            bounds.maximum[axis] += cell[axis] as f64;
                            bounds.minimum[axis] =
                                bounds.minimum[axis].max(origin[axis] as f64) - origin[axis] as f64;
                            bounds.maximum[axis] = bounds.maximum[axis]
                                .min(origin[axis] as f64 + 16.0)
                                - origin[axis] as f64;
                        }
                        expected.insert(key(&bounds));
                    }
                }
                let geometry =
                    model_geometry(&loaded.data, &chunks, &journal, &properties).unwrap();
                assert!(geometry.triangles.is_empty());
                let actual: BTreeSet<_> = geometry.boxes.iter().map(key).collect();
                assert_eq!(
                    actual,
                    expected,
                    "all {} rail cells in {owner:?} retain their exact physical slice, anchor unloaded: {unloaded_anchor}",
                    cells.len()
                );
                assert!(
                    build_collider_with_geometry(
                        &loaded.data,
                        &geometry.boxes,
                        &geometry.triangles,
                        &[STRAIGHT]
                    )
                    .unwrap()
                    .is_some()
                );
                checked += cells.len();
            }
            assert!(checked > 1);
            if unloaded_anchor {
                assert!(
                    chunks
                        .loaded
                        .values()
                        .any(|section| section.data.position() != anchor_owner)
                );
            }
        }
    }
}
