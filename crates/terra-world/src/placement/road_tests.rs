use super::*;
use crate::streaming::LoadedSection;
use terra_core::{ItemKind, roads};
use terra_voxel::{BlockSection, global_to_section};

fn fixture(position: [i64; 3]) -> (ChunkStore, EditJournal) {
    let owner = global_to_section(position).0;
    let mut chunks = ChunkStore::default();
    for dx in -1..=1 {
        for dz in -1..=1 {
            let section = owner.offset([dx, 0, dz]).unwrap();
            let mut data = BlockSection::empty(section).unwrap();
            for x in 0..16 {
                for z in 0..16 {
                    data.set_block([x, 1, z], 3).unwrap();
                }
            }
            chunks.loaded.insert(
                section,
                LoadedSection {
                    data,
                    mesh: None,
                    mesh_epoch: 0,
                    water_epoch: 0,
                    water: None,
                    props: Vec::new(),
                },
            );
        }
    }
    (chunks, EditJournal::default())
}

fn place(position: [i64; 3], material: u16, heading: [f64; 3]) -> (ChunkStore, EditJournal, u8) {
    let (mut chunks, mut journal) = fixture(position);
    let properties = BlockPropertiesCatalog::default();
    let direction = [heading[0] * 0.8, -0.6, heading[2] * 0.8];
    let point = [
        position[0] as f64 + 0.5,
        position[1] as f64,
        position[2] as f64 + 0.5,
    ];
    let origin = std::array::from_fn(|i| point[i] - direction[i] * 3.0);
    let hit =
        crate::interaction_shapes::raycast(&chunks, &journal, &properties, origin, direction, 6.0)
            .unwrap();
    assert_eq!(hit.position, [position[0], position[1] - 1, position[2]]);
    let look = [(-heading[0]).atan2(-heading[2]) as f32, (-0.6_f32).asin()];
    let plan = propose(
        &chunks,
        &journal,
        hit,
        ItemKind::Block(material),
        (origin, direction),
        look,
    )
    .unwrap();
    environment(&plan, &chunks, &journal, false).unwrap();
    assert_eq!(plan.edits[0].position, position);
    let facing = plan.states[0].1.unwrap().facing;
    let direct = layout(&chunks, &journal, position, 0, material, look, None).unwrap();
    assert_eq!(
        direct.states[0].1.unwrap().facing,
        facing,
        "direct placement and player-ray placement disagree"
    );
    for edit in &plan.edits {
        crate::streaming::commit_block_edit(&mut chunks, &mut journal, edit, &properties).unwrap();
    }
    for &(cell, state) in &plan.states {
        crate::streaming::commit_object_state(&mut chunks, &mut journal, cell, state).unwrap();
    }
    (chunks, journal, facing)
}

#[test]
fn actual_road_placement_rises_away_from_player_and_curbs_follow_the_road_in_all_directions() {
    for position in [[7, 2, 7], [-17, 2, -1]] {
        for heading in [
            [0.0, 0.0, -1.0],
            [1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
            [-1.0, 0.0, 0.0],
        ] {
            for material in [roads::SLOPE, roads::ENTRY, roads::EXIT, roads::CURB] {
                let (chunks, journal, facing) = place(position, material, heading);
                if material == roads::CURB {
                    let shape = roads::boxes(material)[0].rotated_y(facing);
                    let axis = if heading[0] == 0.0 { 2 } else { 0 };
                    assert_eq!(shape.maximum[axis] - shape.minimum[axis], 1.0);
                    assert_eq!(shape.maximum[2 - axis] - shape.minimum[2 - axis], 0.1875);
                    continue;
                }
                let height = |along| {
                    let origin = [
                        position[0] as f64 + 0.5 + heading[0] * along,
                        position[1] as f64 + 2.0,
                        position[2] as f64 + 0.5 + heading[2] * along,
                    ];
                    let hit = crate::interaction_shapes::raycast(
                        &chunks,
                        &journal,
                        &BlockPropertiesCatalog::default(),
                        origin,
                        [0.0, -1.0, 0.0],
                        4.0,
                    )
                    .unwrap();
                    assert_eq!(hit.position, position);
                    origin[1] - hit.distance
                };
                let near = height(-0.4);
                let far = height(0.4);
                assert!(
                    far > near + 0.2,
                    "ramp {material} faces the player: {heading:?} at {position:?}, {near}->{far}"
                );
            }
        }
    }
}

#[test]
fn placed_road_forms_can_be_wrenched_four_times_and_reversed_without_changing_items() {
    for material in [
        roads::COVER,
        roads::SLOPE,
        roads::ENTRY,
        roads::EXIT,
        roads::CURB,
    ] {
        let position = [-17, 2, -1];
        let (mut chunks, mut journal, original) = place(position, material, [0.0, 0.0, -1.0]);
        for step in 1..=4 {
            let plan = crate::wrench::propose(&chunks, &journal, position, false).unwrap();
            crate::wrench::commit(&mut chunks, &mut journal, &plan).unwrap();
            assert_eq!(
                journal.object_state(position).unwrap().facing,
                (original + step) % 4
            );
            assert_eq!(chunks.block_at(position), Some(material));
            assert!(journal.gameplay().drops.is_empty());
        }
        let plan = crate::wrench::propose(&chunks, &journal, position, true).unwrap();
        crate::wrench::commit(&mut chunks, &mut journal, &plan).unwrap();
        assert_eq!(
            journal.object_state(position).unwrap().facing,
            (original + 3) % 4
        );
        assert_eq!(
            picked_item(&chunks, &journal, position),
            Some(ItemKind::Block(material))
        );
    }
}
