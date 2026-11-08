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
fn a_legacy_crossing_item_never_offers_or_sends_new_crossing_construction() {
    let (mut app, _, _, _) = connection_fixture();
    app.insert_resource(super::super::ActiveItem(Some(ItemKind::Block(
        rails::CROSSING,
    ))));
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyB);
    app.update();
    let preview = app.world().resource::<PlacementPreview>();
    assert!(!preview.valid && preview.plan.is_none());
    assert!(
        preview
            .rail_error
            .as_ref()
            .is_some_and(|error| error.contains("Kreuzungen"))
    );
    assert!(
        app.world()
            .resource::<Messages<BlockEditRequest>>()
            .is_empty()
    );
    assert!(
        app.world()
            .resource::<Messages<terra_network::client::RailBuildRequest>>()
            .is_empty()
    );
    assert!(
        app.world()
            .resource::<Messages<terra_network::client::RailConnectRequest>>()
            .is_empty()
    );
}

fn floor_fixture(wood: WoodSpecies) -> (App, Entity, ItemKind) {
    let mut app = app();
    session(&mut app, GameMode::Creative, false);
    app.world_mut()
        .resource_mut::<terra_network::client::ClientSession>()
        .uuid = Some(PlayerUuid::from_u128(1));
    {
        let mut chunks = app.world_mut().resource_mut::<ChunkStore>();
        for x in -3..=15 {
            for z in -4..=31 {
                block(&mut chunks, [x, 0, z], 3);
            }
        }
    }
    let item = ItemKind::for_wood(rails::STRAIGHT, wood);
    app.insert_resource(EditJournal::default())
        .insert_resource(terra_core::block_properties::BlockPropertiesCatalog::default())
        .insert_resource(super::super::ActiveItem(Some(item)))
        .init_resource::<super::super::RailBuildContext>()
        .add_message::<bevy::input::mouse::MouseWheel>()
        .add_systems(
            Update,
            super::super::target_block.before(super::super::rail_wheel::prepare_intent),
        )
        .add_systems(
            Update,
            super::super::rail_wheel::update
                .after(super::super::rail_sequence::confirm)
                .before(edit_target_block),
        );
    let camera = app
        .world_mut()
        .spawn((
            crate::TerraCamera,
            Transform::default(),
            crate::CameraLook::default(),
        ))
        .id();
    aim(
        &mut app,
        camera,
        Vec3::new(4.5, 3.0, 1.5),
        Vec3::new(4.5, 1.0, 4.5),
    );
    (app, camera, item)
}
fn aim(app: &mut App, camera: Entity, eye: Vec3, point: Vec3) {
    let transform = Transform::from_translation(eye).looking_at(point, Vec3::Y);
    *app.world_mut().get_mut::<Transform>(camera).unwrap() = transform;
    app.world_mut()
        .get_mut::<crate::CameraLook>(camera)
        .unwrap()
        .sync_external(transform.rotation);
}
fn press(app: &mut App, key: KeyCode) {
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .reset_all();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(key);
}
fn release(app: &mut App) {
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .reset_all();
}
fn take_placement(app: &mut App) -> terra_network::client::RailPlacementRequest {
    let requests = app
        .world_mut()
        .resource_mut::<Messages<terra_network::client::RailPlacementRequest>>()
        .drain()
        .collect::<Vec<_>>();
    assert_eq!(requests.len(), 1);
    requests[0]
}
fn commit_replica(app: &mut App, plan: &terra_world::placement::Layout) {
    app.world_mut()
        .resource_scope(|world, mut chunks: Mut<ChunkStore>| {
            let mut journal = world.resource_mut::<EditJournal>();
            assert!(terra_world::rails::commit(&mut chunks, &mut journal, plan).unwrap());
        });
}
fn acknowledge(
    app: &mut App,
    request: terra_network::client::RailPlacementRequest,
    wire: u64,
    outcome: terra_network::protocol::session::EditOutcome,
) {
    app.world_mut()
        .write_message(terra_network::client::RailPlacementSent {
            token: request.token,
            request: wire,
            action: request.action.edit_action(),
        });
    app.world_mut()
        .write_message(terra_network::client::ConfirmedEdit {
            request: wire,
            action: request.action.edit_action(),
            outcome,
        });
}
fn assert_no_legacy_requests(app: &App) {
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
    assert!(
        app.world()
            .resource::<Messages<terra_network::client::RailBuildRequest>>()
            .is_empty()
    );
}

#[test]
fn first_place_is_a_one_by_three_seed_in_every_wood_and_waits_for_its_exact_ack_and_replica() {
    use terra_network::{client::RailPlacementAction, protocol::session::EditOutcome};
    for wood in [
        WoodSpecies::Oak,
        WoodSpecies::Birch,
        WoodSpecies::Spruce,
        WoodSpecies::Cherry,
    ] {
        let (mut app, _, _) = floor_fixture(wood);
        app.update();
        let hit = app.world().resource::<TargetBlock>().0.unwrap();
        assert_eq!(hit.position, [4, 0, 4]);
        let preview = app.world().resource::<PlacementPreview>();
        assert!(preview.valid, "{wood:?}: {:?}", preview.rail_error);
        let plan = preview.plan.clone().unwrap();
        let next = terra_world::rails::next_endpoint(&plan).unwrap();
        let state = plan
            .states
            .iter()
            .find(|(cell, _)| *cell == next.anchor)
            .unwrap()
            .1
            .unwrap();
        let piece =
            terra_world::rails::piece_from_state(next.anchor, rails::STRAIGHT, state).unwrap();
        let ports = piece.ports();
        let delta: [i64; 3] =
            std::array::from_fn(|axis| ports[1].position.0[axis] - ports[0].position.0[axis]);
        assert_eq!(delta, [0, 0, 16]);
        assert_eq!(piece.footprint().len(), 3);
        assert!(plan.states.iter().all(|(_, state)| {
            state.is_some_and(|state| state.wood == wood && state.rail_curve.is_some())
        }));
        press(&mut app, KeyCode::KeyB);
        app.update();
        let request = take_placement(&mut app);
        assert_eq!(
            request.action,
            RailPlacementAction::Seed {
                position: hit.position,
                expected: 3,
                signature: terra_world::rails::seed_signature(&plan)
            }
        );
        let preview = app.world().resource::<PlacementPreview>();
        assert!(preview.rail_waiting && preview.rail_start.is_none());
        assert_eq!(app.world().resource::<EditJournal>().rail_module_count(), 0);
        assert!(
            terra_world::rails::cells(piece).iter().all(|cell| app
                .world()
                .resource::<ChunkStore>()
                .block_at(*cell)
                == Some(0))
        );
        // A second click cannot enqueue a duplicate starter while waiting.
        press(&mut app, KeyCode::KeyB);
        app.update();
        assert!(
            app.world()
                .resource::<Messages<terra_network::client::RailPlacementRequest>>()
                .is_empty()
        );
        release(&mut app);
        acknowledge(&mut app, request, 51, EditOutcome::Applied);
        app.update();
        assert!(app.world().resource::<PlacementPreview>().rail_waiting);
        assert!(
            app.world()
                .resource::<PlacementPreview>()
                .rail_start
                .is_none(),
            "ACK alone cannot invent a module"
        );
        commit_replica(&mut app, &plan);
        app.update();
        let preview = app.world().resource::<PlacementPreview>();
        assert!(!preview.rail_waiting);
        assert_eq!(preview.rail_start, Some(next));
        assert_eq!(preview.rail_origin, Some((piece, wood)));
        assert_no_legacy_requests(&app);
    }
}

#[test]
fn a_replicated_seed_without_its_own_applied_ack_cannot_start_the_chain() {
    let (mut app, _, _) = floor_fixture(WoodSpecies::Birch);
    press(&mut app, KeyCode::KeyB);
    app.update();
    let request = take_placement(&mut app);
    let plan = app
        .world()
        .resource::<PlacementPreview>()
        .plan
        .clone()
        .unwrap();
    release(&mut app);
    commit_replica(&mut app, &plan);
    app.world_mut()
        .write_message(terra_network::client::RailPlacementSent {
            token: request.token + 1,
            request: 70,
            action: request.action.edit_action(),
        });
    app.world_mut()
        .write_message(terra_network::client::ConfirmedEdit {
            request: 70,
            action: request.action.edit_action(),
            outcome: terra_network::protocol::session::EditOutcome::Applied,
        });
    app.update();
    let preview = app.world().resource::<PlacementPreview>();
    assert!(preview.rail_waiting && preview.rail_start.is_none());
    acknowledge(
        &mut app,
        request,
        71,
        terra_network::protocol::session::EditOutcome::Applied,
    );
    app.update();
    assert_eq!(
        app.world().resource::<PlacementPreview>().rail_start,
        terra_world::rails::next_endpoint(&plan)
    );
}

#[test]
fn confirmed_seed_stretches_continuously_then_advances_only_after_accepted_replication() {
    use terra_network::{client::RailPlacementAction, protocol::session::EditOutcome};
    let (mut app, camera, _) = floor_fixture(WoodSpecies::Spruce);
    press(&mut app, KeyCode::KeyB);
    app.update();
    let seed_request = take_placement(&mut app);
    let seed = app
        .world()
        .resource::<PlacementPreview>()
        .plan
        .clone()
        .unwrap();
    release(&mut app);
    commit_replica(&mut app, &seed);
    acknowledge(&mut app, seed_request, 91, EditOutcome::Applied);
    app.update();
    let start = app
        .world()
        .resource::<PlacementPreview>()
        .rail_start
        .unwrap();
    aim(
        &mut app,
        camera,
        Vec3::new(4.5, 3.0, 11.5),
        Vec3::new(4.5, 1.0, 14.5),
    );
    app.update();
    let preview = app.world().resource::<PlacementPreview>();
    assert!(preview.valid, "{:?}", preview.rail_error);
    assert_eq!(preview.rail_start, Some(start));
    assert_eq!(preview.rail_choice, Some(rails::BuildChoice::Straight));
    let first_plan = preview.plan.clone().unwrap();
    // Moving the target recomputes the same connected preview without another
    // click or any fixed four-block automatic extension.
    aim(
        &mut app,
        camera,
        Vec3::new(4.5, 3.0, 15.5),
        Vec3::new(4.5, 1.0, 18.5),
    );
    app.update();
    let preview = app.world().resource::<PlacementPreview>();
    assert!(preview.valid, "{:?}", preview.rail_error);
    let plan = preview.plan.clone().unwrap();
    assert_ne!(
        terra_world::rails::seed_signature(&plan),
        terra_world::rails::seed_signature(&first_plan)
    );
    let destination = terra_world::rails::next_endpoint(&plan).unwrap();
    let hit = app.world().resource::<TargetBlock>().0.unwrap();
    press(&mut app, KeyCode::KeyB);
    app.update();
    let request = take_placement(&mut app);
    assert_eq!(
        request.action,
        RailPlacementAction::Stretch {
            start,
            position: hit.position,
            expected: 3,
            signature: terra_world::rails::connection_signature(&plan, start)
        }
    );
    assert_eq!(app.world().resource::<EditJournal>().rail_module_count(), 1);
    assert_eq!(
        app.world().resource::<PlacementPreview>().rail_start,
        Some(start)
    );
    release(&mut app);
    acknowledge(&mut app, request, 92, EditOutcome::Applied);
    app.update();
    assert_eq!(
        app.world().resource::<PlacementPreview>().rail_start,
        Some(start)
    );
    commit_replica(&mut app, &plan);
    app.update();
    let preview = app.world().resource::<PlacementPreview>();
    assert_eq!(preview.rail_start, Some(destination));
    assert!(!preview.rail_waiting);
    assert_eq!(app.world().resource::<EditJournal>().rail_module_count(), 2);
    assert_no_legacy_requests(&app);
}

#[test]
fn existing_free_end_click_selects_it_without_any_automatic_extension_and_shift_cancels() {
    let (mut app, _, start, _) = connection_fixture();
    press(&mut app, KeyCode::KeyB);
    app.update();
    assert_eq!(
        app.world().resource::<PlacementPreview>().rail_start,
        Some(start)
    );
    assert!(app.world().resource::<PlacementPreview>().plan.is_none());
    assert!(
        app.world()
            .resource::<Messages<terra_network::client::RailPlacementRequest>>()
            .is_empty()
    );
    press(&mut app, KeyCode::ShiftLeft);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyB);
    app.update();
    let preview = app.world().resource::<PlacementPreview>();
    assert!(preview.rail_start.is_none() && !preview.rail_waiting && preview.plan.is_none());
    assert_no_legacy_requests(&app);
}

#[test]
fn cancel_and_retry_with_the_same_action_cannot_be_rearmed_by_an_old_ack() {
    use terra_network::protocol::session::EditOutcome;
    let (mut app, _, _) = floor_fixture(WoodSpecies::Oak);
    press(&mut app, KeyCode::KeyB);
    app.update();
    let old = take_placement(&mut app);
    press(&mut app, KeyCode::ShiftLeft);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyB);
    app.update();
    assert!(!app.world().resource::<PlacementPreview>().rail_waiting);
    press(&mut app, KeyCode::KeyB);
    app.update();
    let fresh = take_placement(&mut app);
    assert_eq!(old.action, fresh.action);
    assert_ne!(old.token, fresh.token);
    let plan = app
        .world()
        .resource::<PlacementPreview>()
        .plan
        .clone()
        .unwrap();
    release(&mut app);
    acknowledge(&mut app, old, 101, EditOutcome::Applied);
    commit_replica(&mut app, &plan);
    app.update();
    let preview = app.world().resource::<PlacementPreview>();
    assert!(preview.rail_waiting && preview.rail_start.is_none());
    acknowledge(&mut app, fresh, 102, EditOutcome::Applied);
    app.update();
    assert_eq!(
        app.world().resource::<PlacementPreview>().rail_start,
        terra_world::rails::next_endpoint(&plan)
    );
}

#[test]
fn rejected_extension_keeps_confirmed_start_and_r_does_not_erase_it() {
    use terra_network::protocol::session::EditOutcome;
    let (mut app, camera, start, _) = connection_fixture();
    press(&mut app, KeyCode::KeyB);
    app.update();
    release(&mut app);
    let hit = terra_voxel::blocks::BlockHit {
        position: [4, 0, 15],
        normal: [0, 1, 0],
        distance: 3.0,
    };
    app.insert_resource(TargetBlock(Some(hit)))
        .insert_resource(TargetWorld(Some(WorldTarget::Block(hit))));
    aim(
        &mut app,
        camera,
        Vec3::new(4.5, 2.5, 12.5),
        Vec3::new(4.5, 1.0, 15.5),
    );
    press(&mut app, KeyCode::KeyB);
    app.update();
    let request = take_placement(&mut app);
    release(&mut app);
    acknowledge(&mut app, request, 111, EditOutcome::Conflict);
    app.update();
    assert_eq!(
        app.world().resource::<PlacementPreview>().rail_start,
        Some(start)
    );
    assert!(!app.world().resource::<PlacementPreview>().rail_waiting);
    press(&mut app, KeyCode::KeyR);
    app.update();
    let preview = app.world().resource::<PlacementPreview>();
    assert_eq!(preview.rail_mode, 1);
    assert_eq!(preview.rail_start, Some(start));
    assert_eq!(
        preview.rail_choice,
        Some(rails::BuildChoice::Straight),
        "the collinear plan cannot be mislabeled as a curve"
    );
}

#[test]
fn item_or_input_changes_cancel_pending_before_a_late_receipt_can_select_an_endpoint() {
    for blocked in [false, true] {
        let (mut app, _, _) = floor_fixture(WoodSpecies::Oak);
        press(&mut app, KeyCode::KeyB);
        app.update();
        let request = take_placement(&mut app);
        let plan = app
            .world()
            .resource::<PlacementPreview>()
            .plan
            .clone()
            .unwrap();
        release(&mut app);
        if blocked {
            app.world_mut().resource_mut::<crate::PlayerInputBlock>().0 = true;
        } else {
            app.world_mut().resource_mut::<super::super::ActiveItem>().0 =
                Some(ItemKind::for_wood(rails::STRAIGHT, WoodSpecies::Cherry));
        }
        acknowledge(
            &mut app,
            request,
            121,
            terra_network::protocol::session::EditOutcome::Applied,
        );
        commit_replica(&mut app, &plan);
        app.update();
        let preview = app.world().resource::<PlacementPreview>();
        assert!(!preview.rail_waiting && preview.rail_start.is_none());
    }
}

#[test]
fn continuous_preview_uses_real_curved_and_rising_geometry_from_the_confirmed_seed() {
    use terra_network::protocol::session::EditOutcome;
    for (destination, expected_curve, expected_ramp) in
        [([8, 0, 13], true, false), ([4, 1, 9], false, true)]
    {
        let (mut app, camera, item) = floor_fixture(WoodSpecies::Birch);
        press(&mut app, KeyCode::KeyB);
        app.update();
        let request = take_placement(&mut app);
        let seed = app
            .world()
            .resource::<PlacementPreview>()
            .plan
            .clone()
            .unwrap();
        release(&mut app);
        commit_replica(&mut app, &seed);
        acknowledge(&mut app, request, 141, EditOutcome::Applied);
        app.update();
        let start = app
            .world()
            .resource::<PlacementPreview>()
            .rail_start
            .unwrap();
        let hit = terra_voxel::blocks::BlockHit {
            position: destination,
            normal: [0, 1, 0],
            distance: 3.0,
        };
        block(
            &mut app.world_mut().resource_mut::<ChunkStore>(),
            destination,
            3,
        );
        // Build the real load-bearing foundation of the proposed rise. The
        // preview still comes from a fresh actual camera ray and shared planner.
        let proposal = terra_world::rails::proposal_connection(
            app.world().resource::<ChunkStore>(),
            app.world().resource::<EditJournal>(),
            start,
            hit,
            [std::f32::consts::PI, 0.0],
            item.wood_species().unwrap(),
        )
        .unwrap();
        let endpoint = terra_world::rails::next_endpoint(&proposal).unwrap();
        let state = proposal
            .states
            .iter()
            .find(|(cell, _)| *cell == endpoint.anchor)
            .unwrap()
            .1
            .unwrap();
        let piece =
            terra_world::rails::piece_from_state(endpoint.anchor, rails::STRAIGHT, state).unwrap();
        support(&mut app.world_mut().resource_mut::<ChunkStore>(), piece);
        let point = Vec3::new(
            destination[0] as f32 + 0.5,
            destination[1] as f32 + 1.0,
            destination[2] as f32 + 0.5,
        );
        aim(&mut app, camera, point + Vec3::new(0.0, 2.0, -3.0), point);
        app.update();
        let preview = app.world().resource::<PlacementPreview>();
        assert!(preview.valid, "{destination:?}: {:?}", preview.rail_error);
        assert_eq!(preview.rail_start, Some(start));
        assert_eq!(preview.rail_ramp, expected_ramp);
        assert_eq!(
            preview.rail_choice,
            Some(if expected_curve {
                rails::BuildChoice::Curve
            } else {
                rails::BuildChoice::Straight
            })
        );
        assert!(preview.plan.as_ref().is_some_and(|plan| {
            plan.states
                .iter()
                .any(|(_, state)| state.is_some_and(|state| state.rail_curve.is_some()))
        }));
        press(&mut app, KeyCode::KeyB);
        app.update();
        let extension = take_placement(&mut app);
        assert!(
            matches!(extension.action, terra_network::client::RailPlacementAction::Stretch {start: actual, ..} if actual == start)
        );
        assert_eq!(app.world().resource::<EditJournal>().rail_module_count(), 1);
        assert_no_legacy_requests(&app);
    }
}

#[test]
fn a_confirmed_seed_previews_and_signs_a_real_target_beyond_ordinary_block_reach() {
    use terra_network::{client::RailPlacementAction, protocol::session::EditOutcome};
    let (mut app, camera, _) = floor_fixture(WoodSpecies::Spruce);
    press(&mut app, KeyCode::KeyB);
    app.update();
    let request = take_placement(&mut app);
    let seed = app
        .world()
        .resource::<PlacementPreview>()
        .plan
        .clone()
        .unwrap();
    release(&mut app);
    commit_replica(&mut app, &seed);
    acknowledge(&mut app, request, 2101, EditOutcome::Applied);
    app.update();
    let start = app
        .world()
        .resource::<PlacementPreview>()
        .rail_start
        .unwrap();
    aim(
        &mut app,
        camera,
        Vec3::new(4.5, 3.0, 8.5),
        Vec3::new(4.5, 1.0, 24.5),
    );
    app.update();
    let hit = app
        .world()
        .resource::<TargetBlock>()
        .0
        .expect("the rail-specific picker reaches the actual distant floor");
    assert!(hit.distance > 8.0 && hit.distance <= rails::RAIL_BUILD_REACH);
    let preview = app.world().resource::<PlacementPreview>();
    assert!(preview.valid, "{:?}", preview.rail_error);
    let plan = preview.plan.clone().unwrap();
    let signature = terra_world::rails::connection_signature(&plan, start);
    press(&mut app, KeyCode::KeyB);
    app.update();
    let request = take_placement(&mut app);
    assert!(
        matches!(request.action,RailPlacementAction::Stretch{start:actual,position,signature:actual_signature,..} if actual==start && position==hit.position && actual_signature==signature)
    );
    assert_eq!(
        app.world().resource::<EditJournal>().rail_module_count(),
        1,
        "the signed request never speculatively publishes the extension"
    );
}

#[test]
fn real_rail_preview_accepts_its_builder_inside_seed_curve_and_ramp_but_keeps_remote_bodies_protected()
 {
    use terra_network::{
        client::{RemoteTrack, ReplicatedPlayers},
        protocol::session::PlayerState,
    };
    for kind in ["seed", "curve", "ramp"] {
        let (mut app, camera, _) = floor_fixture(WoodSpecies::Birch);
        app.update();
        let source = Piece::new([4, 1, 1], rails::STRAIGHT, 0).unwrap();
        let start = rails::Endpoint {
            anchor: source.anchor,
            port: 1,
        };
        let floor = match kind {
            "curve" => [10, 0, 12],
            "ramp" => [4, 1, 12],
            _ => [4, 0, 4],
        };
        if kind != "seed" {
            app.world_mut()
                .resource_scope(|world, mut journal: Mut<EditJournal>| {
                    let mut chunks = world.resource_mut::<ChunkStore>();
                    let plan = terra_world::rails::plan(
                        &chunks,
                        &journal,
                        source.anchor,
                        source.material,
                        source.encoded_facing(),
                    )
                    .unwrap();
                    terra_world::rails::commit(&mut chunks, &mut journal, &plan).unwrap();
                });
            app.world_mut()
                .resource_mut::<PlacementPreview>()
                .rail_start = Some(start);
        }
        block(&mut app.world_mut().resource_mut::<ChunkStore>(), floor, 3);
        let point = Vec3::new(
            floor[0] as f32 + 0.5,
            (floor[1] + 1) as f32,
            floor[2] as f32 + if kind == "seed" { 0.9 } else { 0.5 },
        );
        let center = if kind == "seed" {
            Vec3::new(4.5, 1.0 + terra_physics::character::PLAYER_HALF_HEIGHT, 4.5)
        } else {
            let candidate = terra_world::rails::proposal_connection(
                app.world().resource::<ChunkStore>(),
                app.world().resource::<EditJournal>(),
                start,
                terra_voxel::blocks::BlockHit {
                    position: floor,
                    normal: [0, 1, 0],
                    distance: 1.0,
                },
                [0.0; 2],
                WoodSpecies::Birch,
            )
            .unwrap();
            let endpoint = terra_world::rails::next_endpoint(&candidate).unwrap();
            let state = candidate
                .states
                .iter()
                .find(|(cell, _)| *cell == endpoint.anchor)
                .unwrap()
                .1
                .unwrap();
            let piece =
                terra_world::rails::piece_from_state(endpoint.anchor, rails::STRAIGHT, state)
                    .unwrap();
            let mid = piece.paths()[0].sample(0.5).position;
            Vec3::new(
                mid[0] as f32,
                1.0 + terra_physics::character::PLAYER_HALF_HEIGHT,
                mid[2] as f32,
            )
        };
        app.world_mut()
            .spawn((crate::PlayerBody, Transform::from_translation(center)));
        aim(
            &mut app,
            camera,
            center + Vec3::Y * terra_physics::character::PLAYER_EYE_OFFSET,
            point,
        );
        let local = terra_core::PlayerId(1);
        app.world_mut()
            .resource_mut::<terra_network::client::ClientSession>()
            .player = Some(local);
        let track = |player| {
            let state = PlayerState {
                player,
                position: center.to_array(),
                ..default()
            };
            RemoteTrack {
                previous: state,
                current: state,
                received: std::time::Instant::now(),
                interval: 0.05,
            }
        };
        let mut remote = ReplicatedPlayers::default();
        remote.players.insert(local, track(local));
        app.insert_resource(remote);
        app.update();
        let preview = app.world().resource::<PlacementPreview>();
        assert!(preview.valid, "{kind}: {:?}", preview.rail_error);
        let half = Vec3::new(
            terra_physics::character::PLAYER_RADIUS,
            terra_physics::character::PLAYER_HALF_HEIGHT,
            terra_physics::character::PLAYER_RADIUS,
        );
        assert!(
            terra_world::placement::boxes(
                preview.plan.as_ref().unwrap(),
                app.world().resource::<ChunkStore>(),
                app.world().resource::<EditJournal>(),
                app.world()
                    .resource::<terra_core::block_properties::BlockPropertiesCatalog>()
            )
            .iter()
            .any(|shape| shape.intersects(
                (center - half).to_array().map(f64::from),
                (center + half).to_array().map(f64::from)
            )),
            "{kind}: actual actor/rail overlap is required"
        );
        let other = terra_core::PlayerId(9);
        app.world_mut()
            .resource_mut::<ReplicatedPlayers>()
            .players
            .insert(other, track(other));
        app.update();
        assert!(
            !app.world().resource::<PlacementPreview>().valid,
            "{kind}: another player at the same position still blocks"
        );
        assert_eq!(
            app.world()
                .resource::<PlacementPreview>()
                .rail_error
                .as_deref(),
            Some("Ein Spieler steht im Gleisbereich")
        );
        assert!(
            app.world()
                .resource::<Messages<terra_network::client::RailPlacementRequest>>()
                .is_empty()
        );
        app.world_mut()
            .resource_mut::<ReplicatedPlayers>()
            .players
            .remove(&other);
        press(&mut app, KeyCode::KeyB);
        app.update();
        let placement = take_placement(&mut app);
        assert!(matches!(
            (kind, placement.action),
            (
                "seed",
                terra_network::client::RailPlacementAction::Seed { .. }
            ) | (
                "curve" | "ramp",
                terra_network::client::RailPlacementAction::Stretch { .. }
            )
        ));
        assert!(app.world().resource::<PlacementPreview>().rail_waiting);
        assert!(
            app.world()
                .resource::<EditJournal>()
                .block([floor[0], floor[1] + 1, floor[2]])
                .is_none(),
            "preview must never publish speculative rail cells"
        );
    }
}
