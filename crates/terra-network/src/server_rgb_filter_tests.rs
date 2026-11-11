use super::*;
use crate::protocol::inventory::{ContainerCommand, InventoryAction, InventoryCommand};
use bevy::ecs::system::SystemState;
use terra_core::{
    GameMode, Inventory, ItemKind, ItemStack, ToolKind, WoodSpecies, logistics::FilterConfig,
};
use terra_world::{
    edits::BlockObjectState,
    streaming::{LoadedSection, WorldTerrain},
};

pub(super) fn fixture() -> (App, Peer) {
    let config = GameConfig::default();
    let terrain = terra_world::TerraGenerator::new(config.world.clone()).unwrap();
    let mut app = App::new();
    app.insert_resource(terra_core::WorldClock::new(config.world.time))
        .init_resource::<terra_core::WorldWeather>()
        .insert_resource(config)
        .insert_resource(WorldTerrain(terrain))
        .init_resource::<terra_core::crafting::RecipeBook>()
        .insert_resource(
            terra_core::block_properties::BlockPropertiesCatalog::load(
                &terra_core::asset_directory().join("terra/blocks"),
            )
            .unwrap(),
        )
        .init_resource::<ChunkStore>()
        .init_resource::<EditJournal>()
        .init_resource::<ConnectedProfiles>()
        .init_resource::<riding::Mounts>();
    let body = app
        .world_mut()
        .spawn((
            Transform::default(),
            ServerCharacter {
                link: Entity::PLACEHOLDER,
            },
        ))
        .id();
    let now = Instant::now();
    let peer = Peer {
        farming: default(),
        crank: default(),
        chest_cursor: None,
        creature_action_at: now,
        container: None,
        held_slot: Some(0),
        mode: Some(GameMode::Survival),
        generation: 0,
        chat: default(),
        uuid: terra_core::PlayerUuid::new_v4(),
        gender: default(),
        appearance: Default::default(),
        mining: None,
        consumption: None,
        gameplay_sent: None,
        permissions: terra_core::PlayerPermissions {
            build: true,
            game_mode: false,
            flight: false,
            environment: false,
        },
        id: PlayerId(1),
        transport_id: 1,
        body,
        accepted: true,
        synced: true,
        view: 1,
        known: default(),
        inputs: default(),
        received: 0,
        ack: 0,
        look: [0.0; 2],
        last_input: now,
        joined: now,
        last_request: 0,
        edits_left: 20.0,
        rate_time: now,
        spawned: true,
        restored_spawn: false,
        restore_location: None,
        last_movement: default(),
        vitals: default(),
        equipment: default(),
        survival: default(),
        stats_sent: None,
    };
    app.world_mut()
        .resource_mut::<EditJournal>()
        .gameplay_mut()
        .inventories
        .insert(peer.uuid, Inventory::default());
    (app, peer)
}

pub(super) fn place(
    app: &mut App,
    position: [i64; 3],
    material: u16,
    state: Option<BlockObjectState>,
) {
    let (owner, local) = terra_voxel::global_to_section(position);
    let mut chunks = app.world_mut().resource_mut::<ChunkStore>();
    let section = chunks.loaded.entry(owner).or_insert_with(|| LoadedSection {
        data: terra_voxel::BlockSection::empty(owner).unwrap(),
        mesh: None,
        mesh_epoch: 0,
        water_epoch: 0,
        water: None,
        props: Vec::new(),
    });
    section.data.set_block(local, material).unwrap();
    let mut journal = app.world_mut().resource_mut::<EditJournal>();
    journal.record(position, material).unwrap();
    if let Some(state) = state {
        journal.set_object_state(position, Some(state)).unwrap();
    }
}

pub(super) fn look(app: &mut App, peer: &mut Peer, eye: Vec3, target: Vec3) {
    let owner = terra_voxel::global_to_section(eye.to_array().map(|v| v.floor() as i64)).0;
    app.world_mut()
        .resource_mut::<ChunkStore>()
        .loaded
        .entry(owner)
        .or_insert_with(|| LoadedSection {
            data: terra_voxel::BlockSection::empty(owner).unwrap(),
            mesh: None,
            mesh_epoch: 0,
            water_epoch: 0,
            water: None,
            props: Vec::new(),
        });
    app.world_mut()
        .get_mut::<Transform>(peer.body)
        .unwrap()
        .translation = eye - Vec3::Y * terra_physics::character::PLAYER_EYE_OFFSET;
    let direction = (target - eye).normalize();
    peer.look = [(-direction.x).atan2(-direction.z), direction.y.asin()];
}

fn command(
    app: &App,
    peer: &Peer,
    position: [i64; 3],
    expected: u16,
    material: u16,
) -> EditCommand {
    EditCommand {
        request: 1,
        revision: app
            .world()
            .resource::<EditJournal>()
            .revision(terra_voxel::global_to_section(position).0),
        look: peer.look,
        source: (material != 0).then(|| InventorySource {
            slot: 0,
            revision: app.world().resource::<EditJournal>().gameplay().inventories[&peer.uuid]
                .revision,
        }),
        action: EditAction::Block {
            position,
            expected,
            material,
        },
    }
}

pub(super) fn apply(app: &mut App, peer: &Peer, command: EditCommand, timed: bool) -> EditOutcome {
    let mut system = SystemState::<ControlWorld>::new(app.world_mut());
    validation::apply(
        command,
        peer,
        &mut system.get_mut(app.world_mut()).unwrap(),
        timed,
    )
}

#[test]
fn survival_rgb_fabrics_beds_and_cherry_belts_place_and_drop_exact_identity_across_negative_sections()
 {
    let rgb = [71, 93, 184];
    for material in [103, 106, 71, 72, 73, 148] {
        for remove_secondary in
            [false, true]
                .into_iter()
                .take(if terra_core::beds::is_bed(material) {
                    2
                } else {
                    1
                })
        {
            let (mut app, mut peer) = fixture();
            let position = [-1, 1, -1];
            for x in -3..=2 {
                for z in -3..=2 {
                    place(&mut app, [x, 0, z], 3, None);
                }
            }
            // Include air sections touched by the authoritative placement and return ray.
            for cell in [[-1, 1, -1], [0, 1, 0], [-1, 1, 1], [0, 1, -1]] {
                place(&mut app, cell, 0, None);
            }
            let item = if terra_core::beds::is_bed(material) {
                ItemKind::DyedBed {
                    material,
                    color: terra_core::beds::DyeColor::Rgb(rgb),
                }
            } else {
                ItemKind::TintedBlock {
                    material,
                    wood: if material == 148 {
                        WoodSpecies::Cherry
                    } else {
                        WoodSpecies::Oak
                    },
                    color: rgb,
                }
            };
            assert!(item.valid());
            app.world_mut()
                .resource_mut::<EditJournal>()
                .gameplay_mut()
                .inventories
                .get_mut(&peer.uuid)
                .unwrap()
                .creative_set(0, ItemStack { item, count: 2 });
            look(
                &mut app,
                &mut peer,
                Vec3::new(-0.5, 3.5, 2.5),
                Vec3::new(-0.5, 1.0, -0.5),
            );
            let request = command(&app, &peer, position, 0, material);
            assert_eq!(
                apply(&mut app, &peer, request, false),
                EditOutcome::Applied,
                "material={material}"
            );
            let journal = app.world().resource::<EditJournal>();
            assert_eq!(journal.tint_at(position), Some(rgb));
            assert_eq!(journal.gameplay().inventories[&peer.uuid].count(item), 1);
            let primary = journal.object_state(position);
            let other = primary.and_then(|state| state.peer);
            if terra_core::beds::is_bed(material) {
                assert_eq!(journal.tint_at(other.unwrap()), Some(rgb));
            }
            if material == 148 {
                assert_eq!(primary.unwrap().wood, WoodSpecies::Cherry);
            }
            let removed = if remove_secondary {
                other.unwrap()
            } else {
                position
            };
            let center = Vec3::from_array(removed.map(|v| v as f32))
                + Vec3::new(
                    0.5,
                    if material == 106 {
                        0.06
                    } else if terra_core::beds::is_bed(material) {
                        0.3
                    } else {
                        0.5
                    },
                    0.5,
                );
            let outward = other.map_or(Vec3::Z, |other| {
                let interior = if removed == position { other } else { position };
                Vec3::from_array(std::array::from_fn(|axis| {
                    (removed[axis] - interior[axis]) as f32
                }))
            });
            let eye = center + Vec3::Y * 1.8 + outward * 3.0;
            look(&mut app, &mut peer, eye, center);
            let direction =
                Quat::from_euler(EulerRot::YXZ, peer.look[0], peer.look[1], 0.0) * Vec3::NEG_Z;
            let (_, target) = terra_world::targeting::pick_world_with_context(
                app.world().resource::<ChunkStore>(),
                app.world().resource::<EditJournal>(),
                app.world()
                    .resource::<terra_core::block_properties::BlockPropertiesCatalog>(),
                eye.to_array().map(f64::from),
                direction.to_array().map(f64::from),
                terra_world::props::PropPose::Rest,
            );
            assert!(
                matches!(target, Some(terra_world::targeting::WorldTarget::Block(hit)) if hit.position == removed),
                "the outward ray must select material={material} secondary={remove_secondary}"
            );
            app.world_mut()
                .resource_mut::<EditJournal>()
                .gameplay_mut()
                .inventories
                .get_mut(&peer.uuid)
                .unwrap()
                .creative_set(
                    1,
                    ItemStack {
                        item: ItemKind::Tool(ToolKind::IronPickaxe),
                        count: 1,
                    },
                );
            peer.held_slot = Some(1);
            let removal = command(&app, &peer, removed, material, 0);
            assert_eq!(
                apply(&mut app, &peer, removal, true),
                EditOutcome::Applied,
                "material={material} secondary={remove_secondary}"
            );
            let journal = app.world().resource::<EditJournal>();
            assert_eq!(
                app.world().resource::<ChunkStore>().block_at(position),
                Some(0)
            );
            assert_eq!(journal.tint_at(position), None);
            if let Some(other) = other {
                assert_eq!(
                    app.world().resource::<ChunkStore>().block_at(other),
                    Some(0)
                );
                assert_eq!(journal.tint_at(other), None);
            }
            assert_eq!(
                journal
                    .gameplay()
                    .drops
                    .iter()
                    .map(|drop| drop.stack)
                    .collect::<Vec<_>>(),
                vec![ItemStack { item, count: 1 }]
            );
            assert!(journal.gameplay().valid());
            assert_eq!(apply(&mut app, &peer, removal, true), EditOutcome::Conflict);
        }
    }
}

fn access(app: &mut App, peer: &mut Peer, action: InventoryAction) {
    peer.edits_left = 20.0;
    let revision =
        app.world().resource::<EditJournal>().gameplay().inventories[&peer.uuid].revision;
    let mut sender = MessageSender::<ServerControl>::default();
    let mut system = SystemState::<ControlWorld>::new(app.world_mut());
    containers::access(
        InventoryCommand { revision, action },
        peer,
        &mut sender,
        &mut system.get_mut(app.world_mut()).unwrap(),
    );
}

#[test]
fn distributor_ghost_filters_require_an_open_visible_owner_and_never_transfer_real_items() {
    let (mut app, mut peer) = fixture();
    let station = [-1, 4, -1];
    place(
        &mut app,
        station,
        terra_core::logistics::BELT_DISTRIBUTOR,
        Some(BlockObjectState::default()),
    );
    look(
        &mut app,
        &mut peer,
        Vec3::new(-0.5, 4.5, 2.5),
        Vec3::new(-0.5, 4.5, -0.5),
    );
    let filter = FilterConfig {
        item: Some(ItemKind::Dye(terra_core::beds::DyeColor::Rgb([
            71, 93, 184,
        ]))),
        allowed: false,
    };
    access(
        &mut app,
        &mut peer,
        InventoryAction::SetDistributorFilter {
            station,
            output: 0,
            filter,
        },
    );
    assert!(
        app.world()
            .resource::<EditJournal>()
            .distributor_at(station)
            .is_none()
    );
    access(
        &mut app,
        &mut peer,
        InventoryAction::OpenContainer { position: station },
    );
    let snapshot = peer
        .container
        .as_ref()
        .expect("real distributor frame opens");
    assert!(snapshot.valid() && snapshot.halves.is_empty() && snapshot.filter.is_none());
    assert_eq!(snapshot.distributor, Some(Default::default()));
    let before = app.world().resource::<EditJournal>().gameplay().clone();
    for output in 0..3 {
        access(
            &mut app,
            &mut peer,
            InventoryAction::SetDistributorFilter {
                station,
                output,
                filter: FilterConfig {
                    allowed: output != 1,
                    ..filter
                },
            },
        );
    }
    let configured = *app
        .world()
        .resource::<EditJournal>()
        .distributor_at(station)
        .unwrap();
    assert_eq!(
        configured.filters.map(|filter| filter.allowed),
        [true, false, true]
    );
    assert_eq!(
        peer.container.as_ref().unwrap().distributor,
        Some(configured)
    );
    access(
        &mut app,
        &mut peer,
        InventoryAction::SetDistributorFilter {
            station,
            output: 3,
            filter: Default::default(),
        },
    );
    assert_eq!(
        app.world()
            .resource::<EditJournal>()
            .distributor_at(station),
        Some(&configured)
    );
    let command = ContainerCommand {
        view: crate::protocol::inventory::ContainerView::Primary,
        anchor: station,
        inventory_revision: before.inventories[&peer.uuid].revision,
        revisions: Vec::new(),
        reserved: [0; terra_core::INVENTORY_SLOTS],
        action: terra_core::containers::ContainerGesture::Click {
            slot: terra_core::INVENTORY_SLOTS as u8,
            secondary: false,
        },
    };
    let mut sender = MessageSender::<ServerControl>::default();
    let mut system = SystemState::<ControlWorld>::new(app.world_mut());
    containers::transfer(
        command,
        &mut peer,
        &mut sender,
        &mut system.get_mut(app.world_mut()).unwrap(),
    );
    assert!(
        app.world()
            .resource::<EditJournal>()
            .container_at(station)
            .is_none()
    );
    assert_eq!(
        app.world().resource::<EditJournal>().gameplay().inventories,
        before.inventories
    );
    assert!(
        app.world()
            .resource::<EditJournal>()
            .gameplay()
            .drops
            .is_empty()
    );
    access(&mut app, &mut peer, InventoryAction::CloseContainer);
    access(
        &mut app,
        &mut peer,
        InventoryAction::SetDistributorFilter {
            station,
            output: 1,
            filter: Default::default(),
        },
    );
    assert_eq!(
        app.world()
            .resource::<EditJournal>()
            .distributor_at(station),
        Some(&configured)
    );
    access(
        &mut app,
        &mut peer,
        InventoryAction::OpenContainer { position: station },
    );
    look(
        &mut app,
        &mut peer,
        Vec3::new(-0.5, 4.5, 20.5),
        Vec3::new(-0.5, 4.5, -0.5),
    );
    access(
        &mut app,
        &mut peer,
        InventoryAction::SetDistributorFilter {
            station,
            output: 1,
            filter: Default::default(),
        },
    );
    assert_eq!(
        app.world()
            .resource::<EditJournal>()
            .distributor_at(station),
        Some(&configured)
    );
}

#[test]
fn filter_configuration_requires_a_real_open_snapshot_and_never_exposes_ghost_inventory() {
    for material in [
        terra_core::logistics::FILTERED_FUNNEL,
        terra_core::logistics::FILTERED_TUNNEL,
    ] {
        let (mut app, mut peer) = fixture();
        let station = [-1, 4, -1];
        place(
            &mut app,
            station,
            material,
            Some(BlockObjectState::default()),
        );
        look(
            &mut app,
            &mut peer,
            Vec3::new(-0.5, 4.5, 2.5),
            Vec3::new(-0.5, 4.5, -0.5),
        );
        let selected = ItemKind::TintedBlock {
            material: 103,
            wood: WoodSpecies::Oak,
            color: [71, 93, 184],
        };
        let filter = FilterConfig {
            item: Some(selected),
            allowed: true,
        };
        access(
            &mut app,
            &mut peer,
            InventoryAction::SetFunnelFilter { station, filter },
        );
        assert!(
            app.world()
                .resource::<EditJournal>()
                .logistics_filter_at(station)
                .is_none()
        );
        access(
            &mut app,
            &mut peer,
            InventoryAction::OpenContainer { position: station },
        );
        let snapshot = peer
            .container
            .as_ref()
            .expect("the real authored funnel is visible and opens");
        assert!(snapshot.valid());
        assert_eq!(snapshot.filter, Some(FilterConfig::default()));
        assert!(snapshot.halves.is_empty());
        let before = app.world().resource::<EditJournal>().gameplay().clone();
        access(
            &mut app,
            &mut peer,
            InventoryAction::SetFunnelFilter { station, filter },
        );
        assert_eq!(peer.container.as_ref().unwrap().filter, Some(filter));
        assert_eq!(
            app.world()
                .resource::<EditJournal>()
                .logistics_filter_at(station),
            Some(&filter)
        );
        assert_eq!(
            app.world().resource::<EditJournal>().gameplay().inventories,
            before.inventories
        );
        assert!(
            app.world()
                .resource::<EditJournal>()
                .container_at(station)
                .is_none()
        );
        assert!(
            app.world()
                .resource::<EditJournal>()
                .gameplay()
                .drops
                .is_empty()
        );
        for action in [
            terra_core::containers::ContainerGesture::Click {
                slot: terra_core::INVENTORY_SLOTS as u8,
                secondary: false,
            },
            terra_core::containers::ContainerGesture::QuickMove {
                slot: terra_core::INVENTORY_SLOTS as u8,
            },
            terra_core::containers::ContainerGesture::Collect {
                slot: terra_core::INVENTORY_SLOTS as u8,
            },
        ] {
            let mut sender = MessageSender::<ServerControl>::default();
            let mut system = SystemState::<ControlWorld>::new(app.world_mut());
            let mut world = system.get_mut(app.world_mut()).unwrap();
            peer.edits_left = 20.0;
            let command = ContainerCommand {
                view: crate::protocol::inventory::ContainerView::Primary,
                inventory_revision: world.journal.gameplay().inventories[&peer.uuid].revision,
                anchor: station,
                revisions: Vec::new(),
                reserved: [0; terra_core::INVENTORY_SLOTS],
                action,
            };
            containers::transfer(command, &mut peer, &mut sender, &mut world);
            assert_eq!(world.journal.gameplay().inventories, before.inventories);
            assert!(world.journal.container_at(station).is_none());
            assert_eq!(peer.container.as_ref().unwrap().filter, Some(filter));
        }
        for allowed in [false, true] {
            let mode = FilterConfig { allowed, ..filter };
            access(
                &mut app,
                &mut peer,
                InventoryAction::SetFunnelFilter {
                    station,
                    filter: mode,
                },
            );
            assert_eq!(peer.container.as_ref().unwrap().filter, Some(mode));
            assert_eq!(
                app.world()
                    .resource::<EditJournal>()
                    .logistics_filter_at(station),
                Some(&mode)
            );
            assert_eq!(
                app.world().resource::<EditJournal>().gameplay().inventories,
                before.inventories
            );
        }
        let wall = [-1, 4, 0];
        place(&mut app, wall, 3, None);
        access(
            &mut app,
            &mut peer,
            InventoryAction::SetFunnelFilter {
                station,
                filter: FilterConfig {
                    allowed: false,
                    ..filter
                },
            },
        );
        assert_eq!(
            app.world()
                .resource::<EditJournal>()
                .logistics_filter_at(station),
            Some(&filter)
        );
        access(
            &mut app,
            &mut peer,
            InventoryAction::OpenContainer { position: station },
        );
        assert!(
            peer.container.is_none(),
            "even the open-frame sight targets cannot open a port through a wall"
        );
        place(&mut app, wall, 0, None);
        access(
            &mut app,
            &mut peer,
            InventoryAction::OpenContainer { position: station },
        );
        assert!(peer.container.is_some());
        access(&mut app, &mut peer, InventoryAction::CloseContainer);
        access(
            &mut app,
            &mut peer,
            InventoryAction::SetFunnelFilter {
                station,
                filter: FilterConfig {
                    allowed: false,
                    ..filter
                },
            },
        );
        assert_eq!(
            app.world()
                .resource::<EditJournal>()
                .logistics_filter_at(station),
            Some(&filter)
        );
        assert!(peer.container.is_none());
        access(
            &mut app,
            &mut peer,
            InventoryAction::OpenContainer { position: station },
        );
        assert!(peer.container.is_some());
        app.world_mut()
            .get_mut::<Transform>(peer.body)
            .unwrap()
            .translation += Vec3::X * 20.0;
        access(
            &mut app,
            &mut peer,
            InventoryAction::SetFunnelFilter {
                station,
                filter: FilterConfig {
                    allowed: false,
                    ..filter
                },
            },
        );
        assert_eq!(
            app.world()
                .resource::<EditJournal>()
                .logistics_filter_at(station),
            Some(&filter)
        );
        access(
            &mut app,
            &mut peer,
            InventoryAction::OpenContainer { position: station },
        );
        assert!(peer.container.is_none());
        assert!(app.world().resource::<EditJournal>().gameplay().valid());
    }
}

#[test]
fn stale_rgb_source_revision_cannot_publish_a_block_or_spend_any_pigmented_stack() {
    let (mut app, mut peer) = fixture();
    let position = [-1, 1, -1];
    place(&mut app, [-1, 0, -1], 3, None);
    let item = ItemKind::TintedBlock {
        material: 103,
        wood: WoodSpecies::Oak,
        color: [71, 93, 184],
    };
    app.world_mut()
        .resource_mut::<EditJournal>()
        .gameplay_mut()
        .inventories
        .get_mut(&peer.uuid)
        .unwrap()
        .creative_set(0, ItemStack { item, count: 2 });
    look(
        &mut app,
        &mut peer,
        Vec3::new(-0.5, 3.5, 2.5),
        Vec3::new(-0.5, 1.0, -0.5),
    );
    let mut request = command(&app, &peer, position, 0, 103);
    request.source.as_mut().unwrap().revision += 1;
    let before = app.world().resource::<EditJournal>().gameplay().clone();
    let epoch = app
        .world()
        .resource::<EditJournal>()
        .revision(terra_voxel::global_to_section(position).0);
    assert_eq!(
        apply(&mut app, &peer, request, false),
        EditOutcome::Conflict
    );
    let journal = app.world().resource::<EditJournal>();
    assert_eq!(
        app.world().resource::<ChunkStore>().block_at(position),
        Some(0)
    );
    assert_eq!(journal.tint_at(position), None);
    assert_eq!(
        journal.revision(terra_voxel::global_to_section(position).0),
        epoch
    );
    assert_eq!(journal.gameplay(), &before);
}

#[test]
fn creative_rgb_sources_cannot_partially_commit_an_unrelated_stone_placement() {
    for item in [
        ItemKind::Dye(terra_core::beds::DyeColor::Rgb([71, 93, 184])),
        ItemKind::TintedBlock {
            material: 103,
            wood: WoodSpecies::Oak,
            color: [71, 93, 184],
        },
    ] {
        let (mut app, mut peer) = fixture();
        peer.mode = Some(GameMode::Creative);
        let position = [-1, 1, -1];
        place(&mut app, [-1, 0, -1], 3, None);
        app.world_mut()
            .resource_mut::<EditJournal>()
            .gameplay_mut()
            .inventories
            .get_mut(&peer.uuid)
            .unwrap()
            .creative_set(0, ItemStack { item, count: 2 });
        look(
            &mut app,
            &mut peer,
            Vec3::new(-0.5, 3.5, 2.5),
            Vec3::new(-0.5, 1.0, -0.5),
        );
        let owner = terra_voxel::global_to_section(position).0;
        let request = command(&app, &peer, position, 0, 3);
        let journal = app.world().resource::<EditJournal>();
        let before = journal.section_delta(owner);
        let gameplay = journal.gameplay().clone();
        let mesh_epoch = journal.mesh_epoch(owner);
        let block_revision = app.world().resource::<ChunkStore>().loaded[&owner]
            .data
            .revision();
        assert_eq!(apply(&mut app, &peer, request, false), EditOutcome::Invalid);
        let journal = app.world().resource::<EditJournal>();
        assert_eq!(journal.section_delta(owner), before);
        assert_eq!(journal.mesh_epoch(owner), mesh_epoch);
        assert_eq!(journal.gameplay(), &gameplay);
        assert_eq!(journal.tint_at(position), None);
        let chunks = app.world().resource::<ChunkStore>();
        assert_eq!(chunks.block_at(position), Some(0));
        assert_eq!(chunks.loaded[&owner].data.revision(), block_revision);
    }
}

#[test]
fn exhausted_rgb_metadata_epochs_reject_placement_before_blocks_or_inventory_change() {
    for material in [103, 71, 148] {
        for appearance in [false, true] {
            let (mut app, mut peer) = fixture();
            let position = [-1, 1, -1];
            for x in -3..=2 {
                for z in -3..=2 {
                    place(&mut app, [x, 0, z], 3, None);
                }
            }
            for cell in [[-1, 1, -1], [0, 1, 0], [-1, 1, 1], [0, 1, -1]] {
                place(&mut app, cell, 0, None);
            }
            let rgb = [71, 93, 184];
            let item = if terra_core::beds::is_bed(material) {
                ItemKind::DyedBed {
                    material,
                    color: terra_core::beds::DyeColor::Rgb(rgb),
                }
            } else {
                ItemKind::TintedBlock {
                    material,
                    wood: WoodSpecies::Oak,
                    color: rgb,
                }
            };
            app.world_mut()
                .resource_mut::<EditJournal>()
                .gameplay_mut()
                .inventories
                .get_mut(&peer.uuid)
                .unwrap()
                .creative_set(0, ItemStack { item, count: 2 });
            look(
                &mut app,
                &mut peer,
                Vec3::new(-0.5, 3.5, 2.5),
                Vec3::new(-0.5, 1.0, -0.5),
            );
            let owner = terra_voxel::global_to_section(position).0;
            let mut delta = app.world().resource::<EditJournal>().section_delta(owner);
            if appearance {
                delta.appearance_epoch = u64::MAX;
            } else {
                delta.revision = u64::MAX;
            }
            app.world_mut()
                .resource_mut::<EditJournal>()
                .import_delta(delta)
                .unwrap();
            let request = command(&app, &peer, position, 0, material);
            let journal = app.world().resource::<EditJournal>();
            let gameplay = journal.gameplay().clone();
            let deltas: Vec<_> = app
                .world()
                .resource::<ChunkStore>()
                .loaded
                .iter()
                .map(|(section, loaded)| {
                    (
                        *section,
                        journal.section_delta(*section),
                        loaded.data.revision(),
                    )
                })
                .collect();
            assert_eq!(
                apply(&mut app, &peer, request, false),
                EditOutcome::Invalid,
                "material={material} appearance={appearance}"
            );
            let journal = app.world().resource::<EditJournal>();
            assert_eq!(journal.gameplay(), &gameplay);
            assert_eq!(journal.tint_at(position), None);
            let chunks = app.world().resource::<ChunkStore>();
            assert_eq!(chunks.block_at(position), Some(0));
            for (section, before, block_revision) in deltas {
                assert_eq!(journal.section_delta(section), before);
                assert_eq!(chunks.loaded[&section].data.revision(), block_revision);
            }
        }
    }
}

#[test]
fn held_funnel_rgb_stacks_require_explicit_visible_loaded_collection_without_duplication() {
    for obstruction in ["none", "wall", "unloaded", "inventory"] {
        let (mut app, mut peer) = fixture();
        let port = [-1, 1, -1];
        place(
            &mut app,
            port,
            terra_core::logistics::FILTERED_FUNNEL,
            Some(BlockObjectState::default()),
        );
        let stack = ItemStack {
            item: ItemKind::Dye(terra_core::beds::DyeColor::Rgb([71, 93, 184])),
            count: 11,
        };
        {
            let mut journal = app.world_mut().resource_mut::<EditJournal>();
            assert!(
                journal
                    .gameplay_mut()
                    .drop_stack([-0.5, 1.63, -0.5], stack, [0.0; 3], 0.0)
            );
            terra_world::funnels::stabilize(&mut journal.gameplay_mut().drops[0], port);
            if obstruction == "inventory" {
                let bag = journal
                    .gameplay_mut()
                    .inventories
                    .get_mut(&peer.uuid)
                    .unwrap();
                for slot in 0..terra_core::INVENTORY_SLOTS {
                    bag.creative_set(
                        slot,
                        ItemStack {
                            item: ItemKind::Block(3),
                            count: ItemKind::Block(3).max_stack(),
                        },
                    );
                }
            }
        }
        look(
            &mut app,
            &mut peer,
            Vec3::new(-0.5, 3.8, 0.5),
            Vec3::new(-0.5, 1.63, -0.5),
        );
        if obstruction == "wall" {
            place(&mut app, [-1, 2, -1], 3, None);
        } else if obstruction == "unloaded" {
            app.world_mut().resource_mut::<ChunkStore>().loaded.clear();
        }
        let id = app.world().resource::<EditJournal>().gameplay().drops[0].id;
        let request = EditCommand {
            request: 1,
            revision: 0,
            look: peer.look,
            source: None,
            action: EditAction::CollectConveyor { id },
        };
        let outcome = apply(&mut app, &peer, request, false);
        if obstruction == "none" {
            assert_eq!(outcome, EditOutcome::Applied);
            let journal = app.world().resource::<EditJournal>();
            assert!(journal.gameplay().drops.is_empty());
            assert_eq!(
                journal.gameplay().inventories[&peer.uuid].count(stack.item),
                11
            );
            assert_ne!(apply(&mut app, &peer, request, false), EditOutcome::Applied);
            assert_eq!(
                app.world().resource::<EditJournal>().gameplay().inventories[&peer.uuid]
                    .count(stack.item),
                11
            );
        } else {
            assert_ne!(outcome, EditOutcome::Applied, "{obstruction}");
            let journal = app.world().resource::<EditJournal>();
            assert_eq!(journal.gameplay().drops.len(), 1);
            assert_eq!(journal.gameplay().drops[0].id, id);
            assert_eq!(journal.gameplay().drops[0].stack, stack);
            assert_eq!(
                journal.gameplay().inventories[&peer.uuid].count(stack.item),
                0
            );
        }
        assert!(app.world().resource::<EditJournal>().gameplay().valid());
    }
}

#[test]
fn passive_pickup_does_not_attract_or_consume_a_held_funnel_stack() {
    let (mut app, peer) = fixture();
    let port = [-1, 1, -1];
    place(
        &mut app,
        port,
        terra_core::logistics::FILTERED_FUNNEL,
        Some(BlockObjectState::default()),
    );
    let stack = ItemStack {
        item: ItemKind::Dye(terra_core::beds::DyeColor::Rgb([71, 93, 184])),
        count: 11,
    };
    {
        let mut journal = app.world_mut().resource_mut::<EditJournal>();
        assert!(
            journal
                .gameplay_mut()
                .drop_stack([-0.5, 1.63, -0.5], stack, [0.0; 3], 0.0)
        );
        terra_world::funnels::stabilize(&mut journal.gameplay_mut().drops[0], port);
    }
    app.world_mut()
        .get_mut::<Transform>(peer.body)
        .unwrap()
        .translation = Vec3::new(-0.5, 1.63, -0.45);
    let before = app.world().resource::<EditJournal>().gameplay().clone();
    app.init_resource::<Time>()
        .init_resource::<terra_world::tree_felling::Queue>()
        .add_systems(Update, gameplay::tick);
    let connection = app
        .world_mut()
        .spawn((
            peer,
            MessageSender::<ServerControl>::default(),
            RemoteId(PeerId::Netcode(1)),
        ))
        .id();
    app.world_mut().entity_mut(connection).insert(Connected);
    app.update();
    assert_eq!(app.world().resource::<EditJournal>().gameplay(), &before);
}

#[test]
fn authoritative_logistics_merges_three_rgb_streams_without_changing_identity_or_quantity() {
    for facing in 0..4 {
        let (mut app, _) = fixture();
        let target = [-1, 1, -1];
        let merger = [-1, 2, -1];
        place(
            &mut app,
            merger,
            terra_core::logistics::BELT_MERGER,
            Some(BlockObjectState {
                facing,
                ..default()
            }),
        );
        let forward = terra_core::kinetic::forward(facing);
        let mut powers = std::collections::BTreeMap::new();
        for step in 0..=5 {
            let cell = std::array::from_fn(|axis| target[axis] + forward[axis] * step);
            place(
                &mut app,
                cell,
                148,
                Some(BlockObjectState {
                    facing,
                    ..default()
                }),
            );
            powers.insert(
                cell,
                terra_core::kinetic::KineticState {
                    rpm: 30.0,
                    ..default()
                },
            );
        }
        let items = [
            ItemKind::TintedBlock {
                material: 103,
                wood: WoodSpecies::Oak,
                color: [1, 2, 3],
            },
            ItemKind::TintedBlock {
                material: 106,
                wood: WoodSpecies::Oak,
                color: [71, 93, 184],
            },
            ItemKind::TintedBlock {
                material: 148,
                wood: WoodSpecies::Cherry,
                color: [200, 150, 40],
            },
        ];
        for (input, item) in items.into_iter().enumerate() {
            let side_facing = (facing + [1, 2, 3][input]) % 4;
            let side = terra_core::kinetic::forward(side_facing);
            let cell = std::array::from_fn(|axis| target[axis] + side[axis]);
            place(
                &mut app,
                cell,
                148,
                Some(BlockObjectState {
                    facing: (side_facing + 2) % 4,
                    ..default()
                }),
            );
            powers.insert(
                cell,
                terra_core::kinetic::KineticState {
                    rpm: 30.0,
                    ..default()
                },
            );
            let center =
                terra_world::conveyors::segment_at(app.world().resource::<EditJournal>(), cell)
                    .unwrap()
                    .drop_center()
                    .unwrap();
            assert!(
                app.world_mut()
                    .resource_mut::<EditJournal>()
                    .gameplay_mut()
                    .drop_stack(
                        center,
                        ItemStack {
                            item,
                            count: input as u16 + 2
                        },
                        [0.0; 3],
                        0.0
                    )
            );
        }
        app.world_mut()
            .resource_mut::<EditJournal>()
            .replace_kinetic_states(powers);
        let before = app
            .world()
            .resource::<EditJournal>()
            .gameplay()
            .drops
            .iter()
            .map(|drop| (drop.id, drop.stack))
            .collect::<Vec<_>>();
        let mut runtime = logistics::Runtime::default();
        let mut transport = terra_world::conveyors::Transport::default();
        let mut system = SystemState::<ControlWorld>::new(app.world_mut());
        let mut world = system.get_mut(app.world_mut()).unwrap();
        let initial = world.journal.gameplay().drops.clone();
        logistics::advance(
            &mut runtime,
            &world.chunks,
            &mut world.journal,
            &world.block_properties,
            0.5,
        );
        assert_eq!(
            world.journal.gameplay().drops,
            initial,
            "inventory logistics may not teleport junction stacks"
        );
        let mut remaining = before
            .iter()
            .map(|(_, stack)| stack.count)
            .collect::<Vec<_>>();
        let mut delivered = [0_u16; 3];
        let mut turn = 0;
        for _ in 0..9 {
            let input = (0..3)
                .map(|offset| (turn + offset) % 3)
                .find(|input| remaining[*input] > 0)
                .unwrap();
            turn = (input + 1) % 3;
            let mut reached = None;
            for _ in 0..100 {
                let positions = world
                    .journal
                    .gameplay()
                    .drops
                    .iter()
                    .map(|drop| (drop.id, (drop.position, drop.stack.item)))
                    .collect::<std::collections::BTreeMap<_, _>>();
                terra_world::conveyors::transport(
                    &mut transport,
                    &world.chunks,
                    &mut world.journal,
                    &world.block_properties,
                    0.02,
                );
                logistics::advance(
                    &mut runtime,
                    &world.chunks,
                    &mut world.journal,
                    &world.block_properties,
                    0.02,
                );
                for drop in &world.journal.gameplay().drops {
                    let previous = positions
                        .get(&drop.id)
                        .map(|(position, _)| *position)
                        .unwrap_or_else(|| {
                            positions
                                .values()
                                .find(|(_, item)| *item == drop.stack.item)
                                .unwrap()
                                .0
                        });
                    let distance = (0..3)
                        .map(|axis| (drop.position[axis] - previous[axis]).powi(2))
                        .sum::<f64>()
                        .sqrt();
                    assert!(
                        distance <= 0.02001,
                        "facing={facing}: real authority movement may not teleport ({distance})"
                    );
                    let along = (0..3)
                        .map(|axis| {
                            (drop.position[axis] - (target[axis] as f64 + 0.5))
                                * forward[axis] as f64
                        })
                        .sum::<f64>();
                    if along >= 0.75 {
                        reached = Some(drop.clone());
                    }
                }
                assert!(world.journal.gameplay().valid());
                if reached.is_some() {
                    break;
                }
            }
            let drop = reached.unwrap_or_else(|| {
                panic!("facing={facing}: input={input} must physically leave the merger")
            });
            assert_eq!(
                drop.stack,
                ItemStack {
                    item: items[input],
                    count: 1
                }
            );
            assert_eq!(
                drop.id == before[input].0,
                remaining[input] == 1,
                "the original ID belongs to the remainder until its final item"
            );
            assert!(terra_world::conveyors::carrying_segment(&drop, &world.journal).is_some());
            remaining[input] -= 1;
            delivered[input] += 1;
            world
                .journal
                .gameplay_mut()
                .drops
                .retain(|current| current.id != drop.id);
            for (index, (_, original)) in before.iter().enumerate() {
                let pending = world
                    .journal
                    .gameplay()
                    .drops
                    .iter()
                    .filter(|drop| drop.stack.item == original.item)
                    .map(|drop| drop.stack.count)
                    .sum::<u16>();
                assert_eq!(pending, remaining[index]);
                assert_eq!(pending + delivered[index], original.count);
            }
        }
        assert!(world.journal.gameplay().drops.is_empty());
        assert_eq!(delivered, [2, 3, 4]);
        assert!(world.journal.gameplay().valid());
        assert!(world.journal.container_at(merger).is_none());
    }
}

#[test]
fn ordinary_tunnel_keeps_random_controls_without_any_filter_or_item_inventory() {
    for material in [terra_core::logistics::TUNNEL, terra_core::logistics::FUNNEL] {
        let (mut app, mut peer) = fixture();
        let station = [-1, 4, -1];
        place(
            &mut app,
            station,
            material,
            Some(BlockObjectState::default()),
        );
        look(
            &mut app,
            &mut peer,
            Vec3::new(-0.5, 4.5, 2.5),
            Vec3::new(-0.5, 4.5, -0.5),
        );
        access(
            &mut app,
            &mut peer,
            InventoryAction::OpenContainer { position: station },
        );
        if material == terra_core::logistics::TUNNEL {
            let snapshot = peer
                .container
                .as_ref()
                .expect("ordinary tunnel configuration opens");
            assert!(snapshot.valid());
            assert_eq!(snapshot.tunnel_random, Some(false));
            assert!(snapshot.filter.is_none() && snapshot.halves.is_empty());
            access(
                &mut app,
                &mut peer,
                InventoryAction::SetTunnelRandom {
                    station,
                    random: true,
                },
            );
            assert!(
                app.world()
                    .resource::<EditJournal>()
                    .tunnel_random_at(station)
            );
            let snapshot = peer.container.as_ref().unwrap();
            assert_eq!(snapshot.tunnel_random, Some(true));
            assert!(snapshot.filter.is_none());
        } else {
            assert!(
                peer.container.is_none(),
                "ordinary funnel has no filter configuration"
            );
        }
        let before = app.world().resource::<EditJournal>().gameplay().clone();
        access(
            &mut app,
            &mut peer,
            InventoryAction::SetFunnelFilter {
                station,
                filter: FilterConfig {
                    item: Some(ItemKind::Block(3)),
                    allowed: false,
                },
            },
        );
        let journal = app.world().resource::<EditJournal>();
        assert!(journal.logistics_filter_at(station).is_none());
        assert!(journal.container_at(station).is_none());
        assert_eq!(journal.gameplay(), &before);
    }
}
