use super::*;
use crate::streaming::LoadedSection;
use terra_core::{ItemKind, WoodSpecies, beds::DyeColor};
use terra_voxel::{BlockSection, global_to_section};

fn place(
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
    p: [i64; 3],
    material: u16,
    state: BlockObjectState,
) {
    let (section, local) = global_to_section(p);
    chunks
        .loaded
        .entry(section)
        .or_insert_with(|| LoadedSection {
            data: BlockSection::empty(section).unwrap(),
            mesh: None,
            mesh_epoch: 0,
            water_epoch: 0,
            water: None,
            props: Vec::new(),
        })
        .data
        .set_block(local, material)
        .unwrap();
    journal.record(p, material).unwrap();
    if material == 148 || terra_core::beds::is_bed(material) {
        journal.set_object_state(p, Some(state)).unwrap();
    }
}

#[test]
fn creative_copy_uses_replicated_rgb_and_bed_anchor_at_negative_chunk_boundaries() {
    let mut chunks = ChunkStore::default();
    let mut journal = EditJournal::default();
    let rgb = [71, 93, 184];
    for (offset, material) in [103, 106, 148].into_iter().enumerate() {
        let p = [-17 + offset as i64, 4, -1];
        let state = BlockObjectState {
            wood: if material == 148 {
                WoodSpecies::Birch
            } else {
                WoodSpecies::Oak
            },
            ..Default::default()
        };
        place(&mut chunks, &mut journal, p, material, state);
        journal.set_tint(p, Some(rgb)).unwrap();
        assert_eq!(
            picked_item(&chunks, &journal, p),
            Some(ItemKind::TintedBlock {
                material,
                wood: state.wood,
                color: rgb
            })
        );
    }
    let bed = [-17, 5, -17];
    let other = [-17, 5, -16];
    place(
        &mut chunks,
        &mut journal,
        bed,
        71,
        BlockObjectState {
            peer: Some(other),
            ..Default::default()
        },
    );
    place(
        &mut chunks,
        &mut journal,
        other,
        71,
        BlockObjectState {
            peer: Some(bed),
            secondary: true,
            ..Default::default()
        },
    );
    journal.set_tint(bed, Some(rgb)).unwrap();
    let expected = Some(ItemKind::DyedBed {
        material: 71,
        color: DyeColor::Rgb(rgb),
    });
    assert_eq!(picked_item(&chunks, &journal, bed), expected);
    assert_eq!(picked_item(&chunks, &journal, other), expected);
    assert_eq!(picked_item(&chunks, &journal, [-16, 5, -16]), None);
}

#[test]
fn creative_copy_keeps_raw_textile_palette_colors_and_has_no_rgb_registry_expansion() {
    let mut chunks = ChunkStore::default();
    let mut journal = EditJournal::default();
    let p = [1, 1, 1];
    let material = terra_core::textiles::dyed(103, DyeColor::Red).unwrap();
    place(
        &mut chunks,
        &mut journal,
        p,
        material,
        BlockObjectState::default(),
    );
    assert_eq!(
        picked_item(&chunks, &journal, p),
        Some(ItemKind::Block(material))
    );
}

#[test]
fn creative_copy_selects_only_the_clicked_rgb_wool_half_even_when_stone_owns_the_cell() {
    let mut chunks = ChunkStore::default();
    let mut journal = EditJournal::default();
    let p = [-17, 4, 2];
    place(&mut chunks, &mut journal, p, 85, Default::default());
    journal
        .set_object_state(
            p,
            Some(BlockObjectState {
                facing: terra_core::construction::merged_slab(85, 0, 105).unwrap(),
                ..Default::default()
            }),
        )
        .unwrap();
    journal.set_tint(p, Some([17, 95, 213])).unwrap();
    assert_eq!(
        picked_item_for_ray(&chunks, &journal, p, ([-16.5, 7.0, 2.5], [0.0, -1.0, 0.0])),
        crate::slabs::targeted_half(&chunks, &journal, p, ([-16.5, 7.0, 2.5], [0.0, -1.0, 0.0]))
            .map(|part| if part.removed == 105 {
                terra_core::tints::recolored(ItemKind::Block(105), DyeColor::Rgb([17, 95, 213]))
                    .unwrap()
            } else {
                ItemKind::Block(85)
            })
    );
    let lower =
        picked_item_for_ray(&chunks, &journal, p, ([-16.5, 1.0, 2.5], [0.0, 1.0, 0.0])).unwrap();
    let upper =
        picked_item_for_ray(&chunks, &journal, p, ([-16.5, 7.0, 2.5], [0.0, -1.0, 0.0])).unwrap();
    assert_ne!(lower, upper);
    assert!([lower, upper].contains(&ItemKind::Block(85)));
    assert!(
        [lower, upper]
            .iter()
            .any(|item| terra_core::tints::color(*item) == Some([17, 95, 213]))
    );
}
