use super::tests::{app, session};
use super::*;
use bevy::ecs::message::Messages;
use terra_core::{
    PlayerUuid, WoodSpecies,
    rails::{self, Piece},
};
use terra_voxel::{BlockSection, global_to_section};
use terra_world::{edits::EditJournal, streaming::LoadedSection};

fn block(chunks: &mut ChunkStore, cell: [i64; 3], material: u16) {
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
        .set_block(local, material)
        .unwrap();
}
fn support(chunks: &mut ChunkStore, piece: Piece) {
    for cell in terra_world::rails::cells(piece) {
        block(chunks, cell, 0);
    }
    for point in piece.supports().iter() {
        block(
            chunks,
            std::array::from_fn(|axis| piece.anchor[axis] + i64::from(point.support[axis])),
            3,
        );
    }
}
fn connection_fixture() -> (App, Entity, terra_core::rails::Endpoint, ItemKind) {
    let mut app = app();
    session(&mut app, GameMode::Creative, false);
    app.world_mut()
        .resource_mut::<terra_network::client::ClientSession>()
        .uuid = Some(PlayerUuid::from_u128(1));
    let source = Piece::new([4, 1, 4], rails::STRAIGHT, 0).unwrap();
    let mut journal = EditJournal::default();
    {
        let mut chunks = app.world_mut().resource_mut::<ChunkStore>();
        support(&mut chunks, source);
        let plan =
            terra_world::rails::plan(&chunks, &journal, source.anchor, source.material, 0).unwrap();
        terra_world::rails::commit(&mut chunks, &mut journal, &plan).unwrap();
        let start = rails::Endpoint {
            anchor: source.anchor,
            port: 1,
        };
        let hit = terra_voxel::blocks::BlockHit {
            position: [4, 0, 15],
            normal: [0, 1, 0],
            distance: 3.,
        };
        let plan = terra_world::rails::proposal_connection(
            &chunks,
            &journal,
            start,
            hit,
            [std::f32::consts::PI, 0.],
            WoodSpecies::Birch,
        )
        .unwrap();
        let (anchor, state) = plan
            .states
            .iter()
            .find(|(_, state)| state.is_some_and(|state| !state.secondary))
            .unwrap();
        let piece =
            terra_world::rails::piece_from_state(*anchor, rails::STRAIGHT, state.unwrap()).unwrap();
        support(&mut chunks, piece);
    }
    let item = ItemKind::for_wood(rails::STRAIGHT, WoodSpecies::Birch);
    app.world_mut()
        .resource_mut::<terra_network::client::ClientSession>()
        .inventory
        .insert(item, 4);
    app.insert_resource(journal)
        .insert_resource(terra_core::block_properties::BlockPropertiesCatalog::default())
        .insert_resource(super::super::ActiveItem(Some(item)));
    let camera = app
        .world_mut()
        .spawn((
            crate::TerraCamera,
            Transform::from_xyz(4.5, 2., 9.5),
            crate::CameraLook::default(),
        ))
        .id();
    let hit = terra_voxel::blocks::BlockHit {
        position: [4, 1, 7],
        normal: [0, 1, 0],
        distance: 2.,
    };
    app.insert_resource(TargetBlock(Some(hit)))
        .insert_resource(TargetWorld(Some(WorldTarget::Block(hit))));
    (
        app,
        camera,
        rails::Endpoint {
            anchor: source.anchor,
            port: 1,
        },
        item,
    )
}

#[test]
fn two_click_preview_sends_one_real_connection_without_speculative_changes_or_wood_loss() {
    let (mut app, camera, start, item) = connection_fixture();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyB);
    app.update();
    assert_eq!(
        app.world().resource::<PlacementPreview>().rail_start,
        Some(start)
    );
    assert!(
        app.world()
            .resource::<Messages<BlockEditRequest>>()
            .is_empty()
    );
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .reset_all();
    let hit = terra_voxel::blocks::BlockHit {
        position: [4, 0, 15],
        normal: [0, 1, 0],
        distance: 3.,
    };
    app.insert_resource(TargetBlock(Some(hit)))
        .insert_resource(TargetWorld(Some(WorldTarget::Block(hit))));
    app.world_mut()
        .get_mut::<Transform>(camera)
        .unwrap()
        .translation = Vec3::new(4.5, 2.5, 12.5);
    app.world_mut()
        .get_mut::<crate::CameraLook>(camera)
        .unwrap()
        .sync_external(Quat::from_rotation_y(std::f32::consts::PI));
    app.update();
    let preview = app.world().resource::<PlacementPreview>();
    assert!(preview.valid);
    assert_eq!(preview.rail_cost, 2);
    assert!(
        preview
            .plan
            .as_ref()
            .unwrap()
            .states
            .iter()
            .all(|(_, state)| {
                state.is_some_and(|state| {
                    state.wood == WoodSpecies::Birch && state.rail_curve.is_some()
                })
            })
    );
    assert!(
        app.world()
            .resource::<Messages<terra_network::client::RailConnectRequest>>()
            .is_empty()
    );
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyB);
    app.update();
    let requests = app
        .world_mut()
        .resource_mut::<Messages<terra_network::client::RailConnectRequest>>()
        .drain()
        .collect::<Vec<_>>();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].start, start);
    assert_eq!(requests[0].position, hit.position);
    assert!(
        app.world()
            .resource::<PlacementPreview>()
            .rail_start
            .is_none()
    );
    assert_eq!(app.world().resource::<EditJournal>().rail_module_count(), 1);
    assert_eq!(
        app.world()
            .resource::<terra_network::client::ClientSession>()
            .inventory
            .count(item),
        4
    );
    assert!(
        app.world()
            .resource::<Messages<BlockEditRequest>>()
            .is_empty()
    );
}

#[test]
fn cancel_item_changes_and_shared_item_modes_never_place_an_unconfirmed_connection() {
    let (mut app, _, start, _) = connection_fixture();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyB);
    app.update();
    assert_eq!(
        app.world().resource::<PlacementPreview>().rail_start,
        Some(start)
    );
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .reset_all();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::ShiftLeft);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyB);
    app.update();
    assert!(
        app.world()
            .resource::<PlacementPreview>()
            .rail_start
            .is_none()
    );
    for mode in 1..=5 {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset_all();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyR);
        app.update();
        assert_eq!(
            app.world().resource::<PlacementPreview>().rail_mode,
            mode % 5
        );
    }
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .reset_all();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyB);
    app.update();
    assert!(
        app.world()
            .resource::<PlacementPreview>()
            .rail_start
            .is_some()
    );
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .reset_all();
    app.insert_resource(super::super::ActiveItem(Some(ItemKind::for_wood(
        rails::STRAIGHT,
        WoodSpecies::Spruce,
    ))));
    app.update();
    assert!(
        app.world()
            .resource::<PlacementPreview>()
            .rail_start
            .is_none()
    );
    assert!(
        app.world()
            .resource::<Messages<terra_network::client::RailConnectRequest>>()
            .is_empty()
    );
    assert!(
        app.world()
            .resource::<Messages<BlockEditRequest>>()
            .is_empty()
    );
}
