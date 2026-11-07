use super::*;
use crate::streaming::LoadedSection;
use terra_core::block_properties::BlockPropertiesCatalog;
use terra_voxel::{BlockSection, SectionPos};

fn world(material: u16) -> (ChunkStore, EditJournal, [i64; 3]) {
    let position = [-1, 4, -1];
    let owner = SectionPos::new(-1, 0, -1);
    let mut data = BlockSection::empty(owner).unwrap();
    data.set_block([15, 3, 15], 3).unwrap();
    data.set_block([15, 4, 15], material).unwrap();
    let mut chunks = ChunkStore::default();
    chunks.loaded.insert(
        owner,
        LoadedSection {
            data,
            mesh: None,
            mesh_epoch: 0,
            water_epoch: 0,
            water: None,
            props: Vec::new(),
        },
    );
    (chunks, EditJournal::default(), position)
}

#[test]
fn player_layers_stack_in_place_to_a_full_block_and_keep_disjoint_saved_poses() {
    let (mut chunks, mut journal, position) = world(0);
    let properties = BlockPropertiesCatalog::default();
    for layers in 1..=8 {
        let expected = if layers == 1 { 0 } else { 98 };
        assert!(commit(&mut chunks, &mut journal, position, expected, &properties).unwrap());
        let material = if layers == 8 { 5 } else { 98 };
        assert_eq!(chunks.block_at(position), Some(material));
        assert_eq!(
            journal.object_state(position).map(|state| state.facing),
            terra_core::construction::manual_snow_facing(layers)
        );
        let shape = bounds(
            position,
            SnowPlacement::Layer {
                material,
                facing: terra_core::construction::manual_snow_facing(layers),
            },
        )
        .unwrap();
        assert_eq!(shape.maximum[1], 4.0 + f64::from(layers) / 8.0);
        let delta = journal.section_delta(global_to_section(position).0);
        let mut restored = EditJournal::default();
        restored.import_delta(delta.clone()).unwrap();
        assert_eq!(restored.section_delta(delta.position), delta);
        assert_eq!(
            restored.object_state(position),
            journal.object_state(position)
        );
        assert_eq!(
            crate::placement::picked_item(&chunks, &journal, position),
            Some(terra_core::ItemKind::Block(material))
        );
    }
    assert!(plan(&chunks, &journal, position, 5).is_none());
    assert!(plan(&chunks, &journal, [-1, 5, -1], 0).is_some());
}

#[test]
fn plates_coat_real_construction_shapes_without_replacing_occupancy_pose_or_clearance() {
    let properties = BlockPropertiesCatalog::default();
    for material in [
        74, 78, 82, 86, 90, 94, 101, 102, 104, 105, 106, 136, 137, 138,
    ] {
        let (mut chunks, mut journal, position) = world(material);
        journal.record(position, material).unwrap();
        let state =
            terra_core::construction::is_shape(material).then_some(BlockObjectState::default());
        journal.set_object_state(position, state).unwrap();
        let hit = BlockHit {
            position,
            normal: [0, 1, 0],
            distance: 2.0,
        };
        assert_eq!(target(&chunks, &journal, hit), Some((position, material)));
        assert!(commit(&mut chunks, &mut journal, position, material, &properties).unwrap());
        assert_eq!(chunks.block_at(position), Some(material));
        assert_eq!(journal.object_state(position), state);
        assert_eq!(chunks.block_at([-1, 5, -1]), Some(0));
        assert_eq!(journal.surface_condition(position).snow, 1.0);
        assert!(!commit(&mut chunks, &mut journal, position, material, &properties).unwrap());
        let mut restored = EditJournal::default();
        restored
            .import_delta(journal.section_delta(global_to_section(position).0))
            .unwrap();
        assert_eq!(restored.surface_condition(position).snow, 1.0);
        assert_eq!(restored.object_state(position), state);
    }
}

#[test]
fn plates_reject_unsupported_air_water_and_furniture_instead_of_creating_floating_caps() {
    let (mut chunks, journal, position) = world(0);
    chunks
        .loaded
        .get_mut(&global_to_section(position).0)
        .unwrap()
        .data
        .set_block([15, 3, 15], 0)
        .unwrap();
    assert_eq!(plan(&chunks, &journal, position, 0), None);
    for material in [5, 27, 28, 29, 59, 71, 99, 148, 156, 157, 158, 159, 160] {
        assert!(!coatable(material));
    }
}
