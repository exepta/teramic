use super::tests::{app, session};
use super::*;
use bevy::ecs::message::Messages;
use terra_core::{InputBinding, WoodSpecies, containers::ChestMerge};
use terra_network::client::ChestPlaceRequest;
use terra_voxel::{SectionPos, blocks::BlockHit};
use terra_world::edits::{BlockObjectState, EditJournal};

fn fixture(side: bool, shift: Option<KeyCode>) -> App {
    let mut app = app();
    session(&mut app, GameMode::Creative, false);
    let mut journal = EditJournal::default();
    journal.record([5, 2, 6], 29).unwrap();
    journal
        .set_object_state(
            [5, 2, 6],
            Some(BlockObjectState {
                facing: 2,
                wood: WoodSpecies::Spruce,
                ..Default::default()
            }),
        )
        .unwrap();
    {
        let mut chunks = app.world_mut().resource_mut::<ChunkStore>();
        let data = &mut chunks.loaded.get_mut(&SectionPos::default()).unwrap().data;
        for x in 4..10 {
            for z in 4..10 {
                data.set_block([x, 1, z], 3).unwrap();
            }
        }
        data.set_block([5, 2, 6], 29).unwrap();
    }
    let hit = BlockHit {
        position: if side { [5, 2, 6] } else { [6, 1, 6] },
        normal: if side { [1, 0, 0] } else { [0, 1, 0] },
        distance: 3.0,
    };
    let mut controls = terra_core::ControlsConfig::default();
    controls
        .rebind(Action::Place, InputBinding::Mouse(MouseButton::Right))
        .unwrap();
    app.insert_resource(journal)
        .insert_resource(controls)
        .insert_resource(terra_core::block_properties::BlockPropertiesCatalog::default())
        .insert_resource(super::super::ActiveItem(Some(ItemKind::Block(29))))
        .insert_resource(TargetBlock(Some(hit)))
        .insert_resource(TargetWorld(Some(WorldTarget::Block(hit))));
    app.world_mut().spawn((
        crate::TerraCamera,
        Transform::from_xyz(6.5, 4.5, 10.5),
        crate::CameraLook::default(),
    ));
    if let Some(shift) = shift {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(shift);
    }
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Right);
    app
}

#[test]
fn actual_shift_input_keeps_floor_placement_single_and_explicitly_pairs_clicked_chest_side() {
    for key in [KeyCode::ShiftLeft, KeyCode::ShiftRight] {
        for side in [false, true] {
            let mut app = fixture(side, Some(key));
            app.update();
            let requests = app
                .world_mut()
                .resource_mut::<Messages<ChestPlaceRequest>>()
                .drain()
                .collect::<Vec<_>>();
            assert_eq!(requests.len(), 1);
            assert_eq!(requests[0].position, [6, 2, 6]);
            assert_eq!(
                requests[0].merge,
                if side {
                    ChestMerge::Target([5, 2, 6])
                } else {
                    ChestMerge::Separate
                }
            );
            let preview = app.world().resource::<PlacementPreview>();
            assert!(preview.valid);
            let state = preview
                .plan
                .as_ref()
                .unwrap()
                .states
                .iter()
                .find(|(p, _)| *p == [6, 2, 6])
                .unwrap()
                .1
                .unwrap();
            assert_eq!(state.peer, side.then_some([5, 2, 6]));
            if side {
                // Placement orientation differs from the existing chest; the pair
                // consistently inherits the old chest's orientation.
                assert_eq!(state.facing, 2);
            }
            assert!(
                app.world()
                    .resource::<Messages<BlockEditRequest>>()
                    .is_empty()
            );
            assert_eq!(
                app.world().resource::<ChunkStore>().block_at([6, 2, 6]),
                Some(0)
            );
            assert_eq!(
                app.world()
                    .resource::<EditJournal>()
                    .object_state([5, 2, 6])
                    .unwrap()
                    .peer,
                None
            );
        }
    }
}

#[test]
fn ordinary_floor_placement_sends_automatic_merge_intent_and_previews_the_same_pair() {
    let mut app = fixture(false, None);
    app.update();
    let requests = app
        .world_mut()
        .resource_mut::<Messages<ChestPlaceRequest>>()
        .drain()
        .collect::<Vec<_>>();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].merge, ChestMerge::Auto);
    assert!(app.world().resource::<PlacementPreview>().valid);
    assert!(
        app.world()
            .resource::<PlacementPreview>()
            .plan
            .as_ref()
            .unwrap()
            .states
            .iter()
            .any(|(p, state)| *p == [6, 2, 6]
                && state.is_some_and(|state| state.peer == Some([5, 2, 6])))
    );
}
