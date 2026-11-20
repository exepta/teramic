use super::*;
use crate::{
    edits::{BlockObjectState, LiquidState},
    streaming::LoadedSection,
};
use terra_voxel::{BlockSection, SectionPos, global_to_section};

fn fixture() -> (ChunkStore, EditJournal) {
    let p = SectionPos::new(0, 0, 0);
    let mut chunks = ChunkStore::default();
    chunks.loaded.insert(
        p,
        LoadedSection {
            data: BlockSection::empty(p).unwrap(),
            mesh: None,
            mesh_epoch: 0,
            water_epoch: 0,
            water: None,
            props: vec![],
        },
    );
    (chunks, EditJournal::default())
}
fn liquid(
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
    p: [i64; 3],
    material: u16,
    volume: u8,
) {
    let (owner, local) = global_to_section(p);
    chunks
        .loaded
        .get_mut(&owner)
        .unwrap()
        .data
        .set_block(local, material)
        .unwrap();
    journal.record(p, material).unwrap();
    journal
        .set_object_state(
            p,
            Some(BlockObjectState {
                facing: if material == bile::FLUID {
                    bile::facing(volume, bile::FINE)
                } else {
                    roads::tar_state(volume.div_ceil(8), 0)
                },
                liquid: Some(LiquidState {
                    origin: p,
                    volume,
                    age: if material == bile::FLUID {
                        bile::FINE
                    } else {
                        0
                    },
                    source: true,
                }),
                ..Default::default()
            }),
        )
        .unwrap();
}
#[test]
fn viscous_contact_follows_the_wet_front_and_respects_body_height() {
    let (mut chunks, mut journal) = fixture();
    liquid(&mut chunks, &mut journal, [3, 2, 3], bile::FLUID, 64);
    liquid(&mut chunks, &mut journal, [4, 2, 3], bile::FLUID, 8);
    let size = [0.1, 1.8, 0.1];
    let entered = body_contact(&chunks, &journal, [4.05, 2.02, 3.5], size);
    let dry = body_contact(&chunks, &journal, [4.8, 2.02, 3.5], size);
    assert!(entered.movement_multiplier < 0.6);
    assert_eq!(
        dry,
        ViscousContact::default(),
        "The advancing cell's dry half must not be sticky"
    );
    assert_eq!(
        body_contact(&chunks, &journal, [3.5, 3.2, 3.5], size),
        ViscousContact::default()
    );
    assert!(
        body_contact(&chunks, &journal, [3.5, 2.02, 3.5], size).movement_multiplier
            < entered.movement_multiplier
    );
}
#[test]
fn a_thin_wet_tar_coating_slows_feet_but_the_cured_road_does_not() {
    let (mut chunks, mut journal) = fixture();
    let p = [3, 2, 3];
    liquid(&mut chunks, &mut journal, p, roads::WET_TAR, 1);
    let feet = [3.5, 2. + f64::from(roads::PATH_HEIGHT) + 0.002, 3.5];
    assert!(body_contact(&chunks, &journal, feet, [0.6, 1.8, 0.6]).movement_multiplier < 0.5);
    let (owner, local) = global_to_section(p);
    chunks
        .loaded
        .get_mut(&owner)
        .unwrap()
        .data
        .set_block(local, roads::DRIED_TAR)
        .unwrap();
    assert_eq!(
        body_contact(&chunks, &journal, feet, [0.6, 1.8, 0.6]),
        ViscousContact::default()
    );
}
#[test]
fn swimming_uses_galle_columns_and_never_solid_galle_blocks() {
    let (mut chunks, mut journal) = fixture();
    liquid(&mut chunks, &mut journal, [3, 2, 3], bile::FLUID, 64);
    liquid(&mut chunks, &mut journal, [3, 3, 3], bile::FLUID, 32);
    let column = swimming_column(&chunks, &journal, [3.5, 2.4, 3.5]).unwrap();
    assert_eq!(column.bed, 2.);
    assert!((column.level - 3.498).abs() < 0.0001);
    let (owner, local) = global_to_section([3, 2, 3]);
    chunks
        .loaded
        .get_mut(&owner)
        .unwrap()
        .data
        .set_block(local, bile::BLOCK)
        .unwrap();
    assert!(swimming_column(&chunks, &journal, [3.5, 2.4, 3.5]).is_none());
}
