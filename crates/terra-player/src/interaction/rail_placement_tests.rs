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
fn shift_remembers_a_long_connection_start_even_when_one_click_docking_is_available() {
    let (mut app, _, start, item) = connection_fixture();
    let hit = app.world().resource::<TargetBlock>().0.unwrap();
    let eye = [4.5, 2.0, 9.5];
    let choices = terra_world::rails::context_choices(
        app.world().resource::<ChunkStore>(),
        app.world().resource::<EditJournal>(),
        hit,
        (eye, [0.0, 0.0, -1.0]),
        [0.0, 0.0],
        item.wood_species().unwrap(),
    );
    assert!(choices.iter().any(|choice| choice.layout.is_some()));
    app.insert_resource(super::super::RailBuildContext {
        choices,
        selected: rails::BuildChoice::Straight,
        active: true,
        hit: Some(hit),
        item: Some(item),
        ..Default::default()
    });
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::ShiftLeft);
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
            .resource::<Messages<terra_network::client::RailBuildRequest>>()
            .is_empty()
    );
    assert!(
        app.world()
            .resource::<Messages<BlockEditRequest>>()
            .is_empty()
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
    assert!(
        app.world()
            .resource::<Messages<terra_network::client::RailBuildRequest>>()
            .is_empty()
    );
}

#[test]
fn contextual_step_preview_sends_one_signed_build_intent_without_speculative_replacement() {
    let mut app = app();
    session(&mut app, GameMode::Creative, false);
    app.world_mut()
        .resource_mut::<terra_network::client::ClientSession>()
        .uuid = Some(PlayerUuid::from_u128(1));
    let old = Piece::new([4, 1, 4], rails::STRAIGHT, 0).unwrap();
    let behind = Piece::new([4, 1, 0], rails::STRAIGHT, 0).unwrap();
    let upper = Piece::new([4, 2, 8], rails::STRAIGHT, 0).unwrap();
    let mut journal = EditJournal::default();
    {
        let mut chunks = app.world_mut().resource_mut::<ChunkStore>();
        for x in 2..=6 {
            for z in -1..=12 {
                block(&mut chunks, [x, 0, z], 3);
                if z >= 8 {
                    block(&mut chunks, [x, 1, z], 3);
                }
            }
        }
        for piece in [behind, old, upper] {
            support(&mut chunks, piece);
            let plan = terra_world::rails::plan(&chunks, &journal, piece.anchor, piece.material, 0)
                .unwrap();
            terra_world::rails::commit(&mut chunks, &mut journal, &plan).unwrap();
        }
    }
    let eye = Vec3::new(4.5, 3.0, 6.0);
    let direction = (Vec3::new(4.5, 1.75, 8.0) - eye).normalize();
    let look = [(-direction.x).atan2(-direction.z), direction.y.asin()];
    let ray = (
        eye.to_array().map(f64::from),
        direction.to_array().map(f64::from),
    );
    let chunks = app.world().resource::<ChunkStore>();
    let hit = terra_world::interaction_shapes::raycast_solid(
        chunks,
        &journal,
        &Default::default(),
        ray.0,
        ray.1,
        4.5,
    )
    .unwrap();
    assert_eq!(hit.position, [4, 1, 8]);
    let choices =
        terra_world::rails::context_choices(chunks, &journal, hit, ray, look, WoodSpecies::Oak);
    let choice = choices
        .iter()
        .find(|choice| choice.choice == rails::BuildChoice::Straight)
        .unwrap();
    assert!(choice.valid, "{:?}", choice.error);
    assert_eq!(choice.replaced, Some(old));
    let signature = terra_world::rails::context_signature(choice);
    let item = ItemKind::Block(rails::STRAIGHT);
    app.insert_resource(super::super::RailBuildContext {
        choices,
        selected: rails::BuildChoice::Straight,
        active: true,
        hit: Some(hit),
        item: Some(item),
        ..Default::default()
    });
    let mut camera = crate::CameraLook::default();
    camera.sync_external(Quat::from_euler(EulerRot::YXZ, look[0], look[1], 0.0));
    app.world_mut()
        .spawn((crate::TerraCamera, Transform::from_translation(eye), camera));
    app.insert_resource(journal)
        .insert_resource(terra_core::block_properties::BlockPropertiesCatalog::default())
        .insert_resource(super::super::ActiveItem(Some(item)))
        .insert_resource(TargetBlock(Some(hit)))
        .insert_resource(TargetWorld(Some(WorldTarget::Block(hit))));
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyB);
    app.update();
    let preview = app.world().resource::<PlacementPreview>();
    assert!(preview.valid, "{:?}", preview.rail_error);
    assert!(preview.rail_replacing && preview.rail_ramp);
    assert_eq!(preview.rail_cost, 1);
    let requests: Vec<_> = app
        .world_mut()
        .resource_mut::<Messages<terra_network::client::RailBuildRequest>>()
        .drain()
        .collect();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].signature, signature);
    assert_eq!(requests[0].position, hit.position);
    assert!(
        app.world()
            .resource::<Messages<BlockEditRequest>>()
            .is_empty()
    );
    assert!(
        app.world()
            .resource::<Messages<terra_network::client::RailConnectRequest>>()
            .is_empty()
    );
    let journal = app.world().resource::<EditJournal>();
    assert_eq!(journal.rail_module_count(), 3);
    assert!(terra_world::rails::complete(journal, old));
}

#[test]
fn each_wood_switch_is_placed_as_its_own_handed_block_and_never_as_a_track_mode() {
    for material in [rails::SWITCH_LEFT, rails::SWITCH_RIGHT] {
        for wood in [
            WoodSpecies::Oak,
            WoodSpecies::Birch,
            WoodSpecies::Spruce,
            WoodSpecies::Cherry,
        ] {
            let mut app = app();
            session(&mut app, GameMode::Creative, false);
            app.world_mut()
                .resource_mut::<terra_network::client::ClientSession>()
                .uuid = Some(PlayerUuid::from_u128(1));
            let piece = Piece::new([4, 1, 4], material, 0).unwrap();
            support(&mut app.world_mut().resource_mut::<ChunkStore>(), piece);
            let item = ItemKind::for_wood(material, wood);
            app.insert_resource(EditJournal::default())
                .insert_resource(terra_core::block_properties::BlockPropertiesCatalog::default())
                .insert_resource(super::super::ActiveItem(Some(item)));
            let mut look = crate::CameraLook::default();
            look.sync_external(Quat::from_rotation_y(std::f32::consts::PI));
            app.world_mut()
                .spawn((crate::TerraCamera, Transform::from_xyz(4.5, 3., 1.5), look));
            let hit = terra_voxel::blocks::BlockHit {
                position: [4, 0, 4],
                normal: [0, 1, 0],
                distance: 3.0,
            };
            app.insert_resource(TargetBlock(Some(hit)))
                .insert_resource(TargetWorld(Some(WorldTarget::Block(hit))));
            app.update();
            let preview = app.world().resource::<PlacementPreview>();
            assert!(
                preview.valid,
                "{material} {wood:?}: {:?}",
                preview.rail_error
            );
            assert_eq!((preview.material, preview.rail_cost), (material, 1));
            assert!(
                preview
                    .plan
                    .as_ref()
                    .unwrap()
                    .states
                    .iter()
                    .all(|(_, state)| state.unwrap().wood == wood)
            );
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(KeyCode::KeyR);
            app.update();
            assert_eq!(app.world().resource::<PlacementPreview>().rail_mode, 0);
            assert!(
                app.world()
                    .resource::<Messages<BlockEditRequest>>()
                    .is_empty()
            );
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .reset_all();
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(KeyCode::KeyB);
            app.update();
            let requests: Vec<_> = app
                .world_mut()
                .resource_mut::<Messages<BlockEditRequest>>()
                .drain()
                .collect();
            assert_eq!(requests.len(), 1);
            assert_eq!(requests[0].material, material);
            assert!(
                app.world()
                    .resource::<PlacementPreview>()
                    .rail_start
                    .is_none()
            );
            assert_eq!(app.world().resource::<EditJournal>().rail_module_count(), 0);
        }
    }
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
    for mode in 1..=3 {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset_all();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyR);
        app.update();
        assert_eq!(
            app.world().resource::<PlacementPreview>().rail_mode,
            mode % 3
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
