use super::*;
use bevy::ecs::system::SystemState;
use terra_core::{
    GameConfig, GameMode, Inventory, ItemKind, ItemStack, PlayerId, PlayerUuid, ResourcePropKind,
};
use terra_world::{
    edits::EditJournal,
    props::{PropKind, PropPose},
    streaming::{ChunkStore, LoadedSection, WorldTerrain},
};

#[path = "server_building_jobs_tests.rs"]
mod building_jobs_tests;
#[path = "server_building_sight_tests.rs"]
mod building_sight_tests;
#[path = "server_chest_authority_tests.rs"]
mod chest_authority_tests;
#[path = "server_conveyor_interaction_tests.rs"]
mod conveyor_interaction_tests;
#[path = "server_npc_work_tests.rs"]
mod npc_work_tests;
#[path = "server_validation_pot_tests.rs"]
mod pot_tests;
#[path = "server_rail_tests.rs"]
mod rail_tests;
#[path = "server_sapling_placement_tests.rs"]
mod sapling_placement_tests;
#[path = "server_tar_tests.rs"]
mod tar_tests;
#[path = "server_train_authority_tests.rs"]
mod train_authority_tests;
#[path = "server_train_protection_tests.rs"]
mod train_protection_tests;
#[path = "server_villager_defense_authority_tests.rs"]
mod villager_defense_authority_tests;
#[path = "server_wrench_tests.rs"]
mod wrench_tests;

fn fixture(kind: ResourcePropKind, wet: bool) -> (App, Peer, [i64; 3]) {
    let support = [4, 3, 4];
    let mut config = GameConfig::default();
    config.server.game_mode = GameMode::Survival;
    config.world.landscape_revision = 3;
    config.world.base_height = 3.0;
    config.world.sea_level = 0.0;
    config.world.hill_amplitude = 0.0;
    config.world.mountain_amplitude = 0.0;
    config.world.biomes = false;
    config.world.caves = false;
    config.world.rivers = false;
    config.world.trees = false;
    config.world.water = true;
    let terrain = terra_world::TerraGenerator::new(config.world.clone()).unwrap();
    let mut app = App::new();
    app.insert_resource(terra_core::WorldClock::new(config.world.time))
        .init_resource::<terra_core::WorldWeather>()
        .insert_resource(config)
        .insert_resource(WorldTerrain(terrain))
        .init_resource::<terra_core::crafting::RecipeBook>()
        .init_resource::<terra_core::block_properties::BlockPropertiesCatalog>()
        .init_resource::<ChunkStore>()
        .init_resource::<EditJournal>()
        .init_resource::<crate::server::ConnectedProfiles>()
        .init_resource::<crate::server::riding::Mounts>();
    let owner = terra_voxel::global_to_section(support).0;
    let mut data = terra_voxel::BlockSection::empty(owner).unwrap();
    for x in 0..16 {
        for z in 0..16 {
            data.set_block([x, 3, z], 3).unwrap();
        }
    }
    if terra_core::TreeSpecies::from_sapling(kind).is_some() {
        data.set_block([4, 3, 4], MaterialId::Dirt as u16).unwrap();
    }
    if wet {
        data.set_block([4, 4, 4], MaterialId::FluidWater as u16)
            .unwrap();
    }
    app.world_mut().resource_mut::<ChunkStore>().loaded.insert(
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
    if terra_core::TreeSpecies::from_sapling(kind).is_some() {
        let upper = owner.offset([0, 1, 0]).unwrap();
        app.world_mut().resource_mut::<ChunkStore>().loaded.insert(
            upper,
            LoadedSection {
                data: terra_voxel::BlockSection::empty(upper).unwrap(),
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: vec![],
            },
        );
    }
    let uuid = PlayerUuid::new_v4();
    {
        let mut journal = app.world_mut().resource_mut::<EditJournal>();
        let mut bag = Inventory::default();
        bag.creative_set(
            0,
            ItemStack {
                item: prop_placement_item(kind),
                count: 2,
            },
        );
        journal.gameplay_mut().inventories.insert(uuid, bag);
        if wet {
            for x in 2..=6 {
                for z in 2..=6 {
                    for y in 4..=8 {
                        journal
                            .set_fluid(
                                [x, y, z],
                                terra_world::water::FluidCell {
                                    amount: 8,
                                    flow: [0; 2],
                                    source: true,
                                },
                            )
                            .unwrap();
                    }
                }
            }
        }
    }
    let body_position = Vec3::new(4.5, 6.0, 8.5);
    let body = app
        .world_mut()
        .spawn((
            Transform::from_translation(body_position),
            crate::server::ServerCharacter {
                link: Entity::PLACEHOLDER,
            },
        ))
        .id();
    let now = std::time::Instant::now();
    let peer = Peer {
        farming: Default::default(),
        crank: Default::default(),
        chest_cursor: None,
        creature_action_at: now,
        container: None,
        held_slot: Some(0),
        mode: Some(GameMode::Survival),
        generation: 0,
        chat: Default::default(),
        uuid,
        gender: Default::default(),
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
        known: Default::default(),
        inputs: Default::default(),
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
        last_movement: Default::default(),
        vitals: Default::default(),
        equipment: Default::default(),
        survival: Default::default(),
        stats_sent: None,
    };
    (app, peer, support)
}

#[test]
fn conveyor_connection_consumes_one_connector_and_middle_removal_returns_one_without_removing_shafts()
 {
    let (mut app, mut peer, _) = fixture(ResourcePropKind::SmallStones, false);
    let owner = terra_voxel::SectionPos::default();
    let start = [7, 6, 5];
    let end = [7, 6, 9];
    let item = ItemKind::for_wood(148, terra_core::WoodSpecies::Birch);
    for p in [start, end] {
        app.world_mut()
            .resource_mut::<ChunkStore>()
            .loaded
            .get_mut(&owner)
            .unwrap()
            .data
            .set_block(p.map(|v| v as usize), 144)
            .unwrap();
        let mut journal = app.world_mut().resource_mut::<EditJournal>();
        journal.record(p, 144).unwrap();
        journal
            .set_object_state(p, Some(Default::default()))
            .unwrap();
    }
    app.world_mut()
        .resource_mut::<EditJournal>()
        .gameplay_mut()
        .inventories
        .get_mut(&peer.uuid)
        .unwrap()
        .creative_set(0, ItemStack { item, count: 2 });
    let eye = Vec3::new(7.5, 6.5, 12.5);
    app.world_mut()
        .get_mut::<Transform>(peer.body)
        .unwrap()
        .translation = eye - Vec3::Y * PLAYER_EYE_OFFSET;
    let journal = app.world().resource::<EditJournal>();
    let command = EditCommand {
        request: 1,
        revision: journal.revision(owner),
        look: [0.0; 2],
        source: Some(InventorySource {
            slot: 0,
            revision: journal.gameplay().inventories[&peer.uuid].revision,
        }),
        action: EditAction::ConnectConveyor {
            start,
            end,
            start_revision: journal.revision(owner),
            item,
        },
    };
    {
        let mut system = SystemState::<ControlWorld>::new(app.world_mut());
        let mut world = system.get_mut(app.world_mut()).unwrap();
        assert_eq!(
            check(
                EditCommand {
                    source: None,
                    ..command
                },
                &peer,
                &world
            ),
            EditOutcome::Invalid
        );
        assert_eq!(
            apply(command, &peer, &mut world, false),
            EditOutcome::Applied
        );
        assert_eq!(
            world.journal.gameplay().inventories[&peer.uuid].count(item),
            1
        );
        assert_eq!(
            apply(command, &peer, &mut world, false),
            EditOutcome::Conflict
        );
        assert_eq!(
            world.journal.gameplay().inventories[&peer.uuid].count(item),
            1
        );
        for z in 6..=8 {
            assert_eq!(world.chunks.block_at([7, 6, z]), Some(148));
        }
    }
    {
        let mut journal = app.world_mut().resource_mut::<EditJournal>();
        let bag = journal
            .gameplay_mut()
            .inventories
            .get_mut(&peer.uuid)
            .unwrap();
        bag.creative_set(1, ItemStack { item, count: 1 });
        bag.creative_set(
            0,
            ItemStack {
                item: ItemKind::Tool(terra_core::ToolKind::IronPickaxe),
                count: 1,
            },
        );
    }
    let eye = Vec3::new(7.5, 8.5, 7.5);
    app.world_mut()
        .get_mut::<Transform>(peer.body)
        .unwrap()
        .translation = eye - Vec3::Y * PLAYER_EYE_OFFSET;
    peer.look = [0.0, -1.4];
    let remove = EditCommand {
        request: 2,
        revision: app.world().resource::<EditJournal>().revision(owner),
        look: peer.look,
        source: None,
        action: EditAction::Block {
            position: [7, 6, 7],
            expected: 148,
            material: 0,
        },
    };
    let mut system = SystemState::<ControlWorld>::new(app.world_mut());
    let mut world = system.get_mut(app.world_mut()).unwrap();
    assert_eq!(apply(remove, &peer, &mut world, true), EditOutcome::Applied);
    for z in 6..=8 {
        assert_eq!(world.chunks.block_at([7, 6, z]), Some(0));
    }
    for p in [start, end] {
        assert_eq!(world.chunks.block_at(p), Some(144));
    }
    let drops = &world.journal.gameplay().drops;
    assert_eq!(drops.len(), 1);
    assert_eq!(drops[0].stack, ItemStack { item, count: 1 });
}

#[test]
fn side_clicked_shaft_places_the_gear_at_the_selected_face_and_rejects_the_old_off_face_snap() {
    let (mut app, mut peer, _) = fixture(ResourcePropKind::SmallStones, false);
    peer.mode = Some(GameMode::Creative);
    let owner = terra_voxel::SectionPos::default();
    let shaft = [7, 6, 7];
    let target = [7, 6, 8];
    app.world_mut()
        .resource_mut::<ChunkStore>()
        .loaded
        .get_mut(&owner)
        .unwrap()
        .data
        .set_block(shaft.map(|v| v as usize), 146)
        .unwrap();
    {
        let mut journal = app.world_mut().resource_mut::<EditJournal>();
        journal.record(shaft, 146).unwrap();
        journal
            .set_object_state(shaft, Some(Default::default()))
            .unwrap();
    }
    let eye = Vec3::new(7.5, 6.5, 10.5);
    app.world_mut()
        .get_mut::<Transform>(peer.body)
        .unwrap()
        .translation = eye - Vec3::Y * PLAYER_EYE_OFFSET;
    let command = EditCommand {
        request: 1,
        revision: app.world().resource::<EditJournal>().revision(owner),
        look: [0.0; 2],
        source: None,
        action: EditAction::Block {
            position: target,
            expected: 0,
            material: 143,
        },
    };
    let mut system = SystemState::<ControlWorld>::new(app.world_mut());
    let mut world = system.get_mut(app.world_mut()).unwrap();
    let (hit, _) = pick_world_with_context(
        &world.chunks,
        &world.journal,
        &world.block_properties,
        eye.to_array().map(f64::from),
        [0.0, 0.0, -1.0],
        PropPose::Rest,
    );
    let hit = hit.unwrap();
    assert_eq!((hit.position, hit.normal), (shaft, [0, 0, 1]));
    let forged = EditCommand {
        action: EditAction::Block {
            position: [8, 6, 7],
            expected: 0,
            material: 143,
        },
        ..command
    };
    assert_eq!(check(forged, &peer, &world), EditOutcome::Occluded);
    assert_eq!(check(command, &peer, &world), EditOutcome::Applied);
    assert_eq!(
        apply(command, &peer, &mut world, false),
        EditOutcome::Applied
    );
    assert_eq!(world.chunks.block_at(target), Some(143));
    assert_eq!(world.chunks.block_at([8, 5, 7]), Some(0));
    assert_eq!(
        terra_core::kinetic::axis(world.journal.object_state(target).unwrap().facing).map(i64::abs),
        [0, 0, 1]
    );
}

#[test]
fn ordinary_block_commit_cannot_bypass_a_failed_mechanical_overhang_preview() {
    let (mut app, mut peer, _) = fixture(ResourcePropKind::SmallStones, false);
    peer.mode = Some(GameMode::Creative);
    let owner = terra_voxel::SectionPos::default();
    let gear = [7, 5, 7];
    let support = [8, 4, 7];
    let target = [8, 5, 7];
    {
        let mut chunks = app.world_mut().resource_mut::<ChunkStore>();
        let data = &mut chunks.loaded.get_mut(&owner).unwrap().data;
        data.set_block(gear.map(|v| v as usize), 151).unwrap();
        data.set_block(support.map(|v| v as usize), 3).unwrap();
    }
    {
        let mut journal = app.world_mut().resource_mut::<EditJournal>();
        journal.record(gear, 151).unwrap();
        journal
            .set_object_state(
                gear,
                Some(terra_world::edits::BlockObjectState {
                    facing: 4,
                    ..Default::default()
                }),
            )
            .unwrap();
    }
    let eye = Vec3::new(8.85, 6.0, 10.5);
    app.world_mut()
        .get_mut::<Transform>(peer.body)
        .unwrap()
        .translation = eye - Vec3::Y * PLAYER_EYE_OFFSET;
    let direction = (Vec3::new(8.85, 5.0, 7.85) - eye).normalize();
    let look = [(-direction.x).atan2(-direction.z), direction.y.asin()];
    peer.look = look;
    let revision = app.world().resource::<EditJournal>().revision(owner);
    let mut command = EditCommand {
        request: 1,
        revision,
        look,
        source: None,
        action: EditAction::Block {
            position: target,
            expected: 0,
            material: 3,
        },
    };
    {
        let mut system = SystemState::<ControlWorld>::new(app.world_mut());
        let mut world = system.get_mut(app.world_mut()).unwrap();
        let (hit, _) = pick_world_with_context(
            &world.chunks,
            &world.journal,
            &world.block_properties,
            eye.to_array().map(f64::from),
            direction.to_array().map(f64::from),
            PropPose::Rest,
        );
        assert_eq!(hit.unwrap().position, support);
        assert_eq!(check(command, &peer, &world), EditOutcome::Invalid);
        assert_eq!(
            apply(command, &peer, &mut world, false),
            EditOutcome::Invalid
        );
        assert_eq!(world.chunks.block_at(target), Some(0));
        assert_eq!(world.journal.revision(owner), revision);
    }
    app.world_mut()
        .resource_mut::<ChunkStore>()
        .loaded
        .get_mut(&owner)
        .unwrap()
        .data
        .set_block(gear.map(|v| v as usize), 0)
        .unwrap();
    app.world_mut()
        .resource_mut::<EditJournal>()
        .record(gear, 0)
        .unwrap();
    command.revision = app.world().resource::<EditJournal>().revision(owner);
    let mut system = SystemState::<ControlWorld>::new(app.world_mut());
    let mut world = system.get_mut(app.world_mut()).unwrap();
    assert_eq!(check(command, &peer, &world), EditOutcome::Applied);
    assert_eq!(
        apply(command, &peer, &mut world, false),
        EditOutcome::Applied
    );
    assert_eq!(world.chunks.block_at(target), Some(3));
}

fn aim(app: &App, peer: &Peer, target: [f64; 3]) -> [f32; 2] {
    let eye =
        app.world().get::<Transform>(peer.body).unwrap().translation + Vec3::Y * PLAYER_EYE_OFFSET;
    let direction = (Vec3::from_array(target.map(|v| v as f32)) - eye).normalize();
    [(-direction.x).atan2(-direction.z), direction.y.asin()]
}

fn equip_farm_tool(app: &mut App, peer: &Peer, tool: terra_core::ToolKind) {
    app.world_mut()
        .resource_mut::<EditJournal>()
        .gameplay_mut()
        .inventories
        .get_mut(&peer.uuid)
        .unwrap()
        .creative_set(
            0,
            ItemStack {
                item: ItemKind::Tool(tool),
                count: 1,
            },
        );
}

fn farm_command(
    app: &App,
    peer: &Peer,
    position: [i64; 3],
    action: terra_core::farming::FarmingAction,
    look: [f32; 2],
) -> EditCommand {
    use terra_core::farming::FarmingAction;
    let journal = app.world().resource::<EditJournal>();
    let section = terra_voxel::global_to_section(position).0;
    EditCommand {
        request: 1,
        look,
        action: EditAction::Farm { position, action },
        revision: if matches!(
            action,
            FarmingAction::FillBucket
                | FarmingAction::EmptyBucket
                | FarmingAction::Till
                | FarmingAction::MakePath
                | FarmingAction::PourTar
                | FarmingAction::FillBileBucket
                | FarmingAction::EmptyBileBucket
                | FarmingAction::Refill
        ) {
            journal.revision(section)
        } else {
            journal.prop_epoch(section)
        },
        source: Some(InventorySource {
            slot: 0,
            revision: journal.gameplay().inventories[&peer.uuid].revision,
        }),
    }
}

#[test]
fn buckets_exchange_the_held_item_and_real_source_without_duplication_or_remote_use() {
    use terra_core::{ToolKind, farming::FarmingAction};
    let (mut app, mut peer, _) = fixture(ResourcePropKind::Wheat, false);
    let p = [6, 4, 4];
    app.world_mut()
        .resource_mut::<ChunkStore>()
        .loaded
        .get_mut(&terra_voxel::SectionPos::default())
        .unwrap()
        .data
        .set_block([6, 4, 4], 27)
        .unwrap();
    app.world_mut()
        .resource_mut::<EditJournal>()
        .set_fluid(
            p,
            terra_world::water::FluidCell {
                amount: 8,
                source: true,
                flow: [0; 2],
            },
        )
        .unwrap();
    equip_farm_tool(&mut app, &peer, ToolKind::Bucket);
    let fill = farm_command(
        &app,
        &peer,
        p,
        FarmingAction::FillBucket,
        aim(&app, &peer, [6.5, 4.5, 4.5]),
    );
    let mut sender = lightyear::prelude::MessageSender::<ServerControl>::default();
    let mut system = SystemState::<ControlWorld>::new(app.world_mut());
    {
        let mut world = system.get_mut(app.world_mut()).unwrap();
        let apply = super::super::farm_tools::apply;
        assert_eq!(
            apply(fill, &mut peer, &mut sender, &mut world),
            EditOutcome::Applied
        );
        assert_eq!(
            world.journal.gameplay().inventories[&peer.uuid]
                .slot(0)
                .unwrap()
                .item,
            ItemKind::Tool(ToolKind::WaterBucket)
        );
        assert_eq!(world.journal.fluid(p).unwrap().amount, 0);
        assert_eq!(world.chunks.block_at(p), Some(0));
        peer.farming.reset_cooldown();
        assert_eq!(
            apply(fill, &mut peer, &mut sender, &mut world),
            EditOutcome::Conflict
        );
    }
    let pour = farm_command(
        &app,
        &peer,
        p,
        FarmingAction::EmptyBucket,
        aim(&app, &peer, [6.5, 3.99, 4.5]),
    );
    {
        let mut world = system.get_mut(app.world_mut()).unwrap();
        peer.permissions.build = false;
        assert_eq!(
            super::super::farm_tools::apply(pour, &mut peer, &mut sender, &mut world),
            EditOutcome::Invalid
        );
        peer.permissions.build = true;
        assert_eq!(
            super::super::farm_tools::apply(pour, &mut peer, &mut sender, &mut world),
            EditOutcome::Applied
        );
        assert!(world.journal.fluid(p).unwrap().source);
        assert_eq!(
            world.journal.gameplay().inventories[&peer.uuid]
                .slot(0)
                .unwrap()
                .item,
            ItemKind::Tool(ToolKind::Bucket)
        );
        let delta = world
            .journal
            .section_delta(terra_voxel::SectionPos::default());
        let mut replica = EditJournal::default();
        replica.import_delta(delta).unwrap();
        assert_eq!(replica.fluid(p), world.journal.fluid(p));
    }
}

fn plant_test_crop(app: &mut App, support: [i64; 3], ripe: bool) {
    let owner = terra_voxel::global_to_section(support).0;
    app.world_mut()
        .resource_mut::<ChunkStore>()
        .loaded
        .get_mut(&owner)
        .unwrap()
        .data
        .set_block(support.map(|value| value.rem_euclid(16) as usize), 2)
        .unwrap();
    let prop = placed_prop(support, ResourcePropKind::Wheat, 0);
    let mut system = SystemState::<ControlWorld>::new(app.world_mut());
    let mut world = system.get_mut(app.world_mut()).unwrap();
    assert!(
        terra_world::farming::plant_crop(
            &mut world.chunks,
            &mut world.journal,
            &world.block_properties,
            prop
        )
        .unwrap()
    );
    let mut crop = *world.journal.crop_at(support).unwrap();
    crop.moisture = 0.1;
    if ripe {
        crop.progress_seconds = crop.growth_duration_seconds;
    }
    terra_world::farming::commit_crop_state(&mut world.chunks, &mut world.journal, support, crop)
        .unwrap();
}

#[test]
fn hoe_requires_selected_tool_current_inventory_and_uncovered_top_soil() {
    use terra_core::{ToolKind, farming::FarmingAction};
    let (mut app, mut peer, support) = fixture(ResourcePropKind::Wheat, false);
    app.world_mut()
        .resource_mut::<ChunkStore>()
        .loaded
        .get_mut(&terra_voxel::SectionPos::default())
        .unwrap()
        .data
        .set_block([4, 3, 4], 2)
        .unwrap();
    equip_farm_tool(&mut app, &peer, ToolKind::WoodHoe);
    let command = farm_command(
        &app,
        &peer,
        support,
        FarmingAction::Till,
        aim(&app, &peer, [4.5, 4.0, 4.5]),
    );
    let mut sender = lightyear::prelude::MessageSender::<ServerControl>::default();
    let mut system = SystemState::<ControlWorld>::new(app.world_mut());
    let mut world = system.get_mut(app.world_mut()).unwrap();
    let mut forged = command;
    forged.source.as_mut().unwrap().revision += 1;
    assert_eq!(
        super::super::farm_tools::apply(forged, &mut peer, &mut sender, &mut world),
        EditOutcome::Conflict
    );
    assert_eq!(world.chunks.block_at(support), Some(2));
    assert_eq!(
        super::super::farm_tools::apply(command, &mut peer, &mut sender, &mut world),
        EditOutcome::Applied
    );
    assert_eq!(
        world.chunks.block_at(support),
        Some(terra_core::farming::FARMLAND)
    );
    peer.farming.reset_cooldown();
    assert_eq!(
        super::super::farm_tools::apply(command, &mut peer, &mut sender, &mut world),
        EditOutcome::Conflict
    );
    assert_eq!(
        world.journal.gameplay().inventories[&peer.uuid].count(ItemKind::Tool(ToolKind::WoodHoe)),
        1
    );
}

#[test]
fn watering_refills_real_water_and_persists_crop_moisture_without_terrain_rebuild() {
    use terra_core::{ToolKind, farming::FarmingAction};
    let (mut app, mut peer, support) = fixture(ResourcePropKind::Wheat, false);
    plant_test_crop(&mut app, support, false);
    equip_farm_tool(&mut app, &peer, ToolKind::IronWateringCan);
    app.world_mut()
        .resource_mut::<ChunkStore>()
        .loaded
        .get_mut(&terra_voxel::SectionPos::default())
        .unwrap()
        .data
        .set_block([6, 4, 4], 27)
        .unwrap();
    let refill = farm_command(
        &app,
        &peer,
        [6, 4, 4],
        FarmingAction::Refill,
        aim(&app, &peer, [6.5, 4.5, 4.5]),
    );
    let mut water = farm_command(
        &app,
        &peer,
        support,
        FarmingAction::Water,
        aim(&app, &peer, [4.5, 4.0, 4.5]),
    );
    let mut sender = lightyear::prelude::MessageSender::<ServerControl>::default();
    let mut system = SystemState::<ControlWorld>::new(app.world_mut());
    let mut world = system.get_mut(app.world_mut()).unwrap();
    let mesh_epoch = world.chunks.loaded[&terra_voxel::SectionPos::default()].mesh_epoch;
    assert_eq!(
        super::super::farm_tools::apply(water, &mut peer, &mut sender, &mut world),
        EditOutcome::Invalid
    );
    assert_eq!(
        super::super::farm_tools::apply(refill, &mut peer, &mut sender, &mut world),
        EditOutcome::Applied
    );
    assert_eq!(peer.farming.water(), 12);
    assert_eq!(
        world.journal.gameplay().inventories[&peer.uuid]
            .slot(0)
            .unwrap()
            .item
            .watering_units(),
        12
    );
    assert_eq!(
        super::super::farm_tools::apply(water, &mut peer, &mut sender, &mut world),
        EditOutcome::Conflict,
        "refilling changes the actual vessel inventory revision"
    );
    water.source.as_mut().unwrap().revision =
        world.journal.gameplay().inventories[&peer.uuid].revision;
    assert_eq!(
        super::super::farm_tools::apply(water, &mut peer, &mut sender, &mut world),
        EditOutcome::RateLimited
    );
    peer.farming.reset_cooldown();
    assert_eq!(
        super::super::farm_tools::apply(water, &mut peer, &mut sender, &mut world),
        EditOutcome::Applied
    );
    assert_eq!(peer.farming.water(), 11);
    assert_eq!(world.journal.crop_at(support).unwrap().moisture, 1.0);
    assert_eq!(
        world.chunks.loaded[&terra_voxel::SectionPos::default()].mesh_epoch,
        mesh_epoch
    );
    peer.farming.reset_cooldown();
    water.source.as_mut().unwrap().revision =
        world.journal.gameplay().inventories[&peer.uuid].revision;
    assert_eq!(
        super::super::farm_tools::apply(water, &mut peer, &mut sender, &mut world),
        EditOutcome::Invalid
    );
    assert_eq!(peer.farming.water(), 11);
}

#[test]
fn scythe_harvests_only_ripe_visible_crops_and_reserves_all_drop_slots_atomically() {
    use terra_core::{ToolKind, farming::FarmingAction};
    let (mut app, mut peer, _) = fixture(ResourcePropKind::Wheat, false);
    for x in 4..=6 {
        plant_test_crop(&mut app, [x, 3, 4], true);
    }
    plant_test_crop(&mut app, [5, 3, 5], false);
    equip_farm_tool(&mut app, &peer, ToolKind::WoodScythe);
    let prop = app.world().resource::<ChunkStore>().loaded[&terra_voxel::SectionPos::default()]
        .props
        .iter()
        .find(|prop| terra_world::props::placement::support(prop) == [5, 3, 4] && !prop.removed)
        .copied()
        .unwrap();
    let command = farm_command(
        &app,
        &peer,
        [5, 3, 4],
        FarmingAction::Harvest,
        aim_crop(&app, &peer, prop),
    );
    let mut sender = lightyear::prelude::MessageSender::<ServerControl>::default();
    let mut system = SystemState::<ControlWorld>::new(app.world_mut());
    let mut world = system.get_mut(app.world_mut()).unwrap();
    world.journal.gameplay_mut().next_drop = u64::MAX - 2;
    assert_eq!(
        super::super::farm_tools::apply(command, &mut peer, &mut sender, &mut world),
        EditOutcome::RateLimited
    );
    assert!((4..=6).all(|x| world.journal.crop_at([x, 3, 4]).is_some()));
    world.journal.gameplay_mut().next_drop = 0;
    assert_eq!(
        super::super::farm_tools::apply(command, &mut peer, &mut sender, &mut world),
        EditOutcome::Applied
    );
    assert!((4..=6).all(|x| world.journal.crop_at([x, 3, 4]).is_none()));
    assert!(world.journal.crop_at([5, 3, 5]).is_some());
    assert_eq!(world.journal.gameplay().drops.len(), 6);
    peer.farming.reset_cooldown();
    assert_eq!(
        super::super::farm_tools::apply(command, &mut peer, &mut sender, &mut world),
        EditOutcome::Conflict
    );
    assert_eq!(world.journal.gameplay().drops.len(), 6);
}

#[test]
fn slab_merge_is_authoritative_consumes_one_half_and_rejects_a_third() {
    for (facing, added) in [0, 4, 5, 6, 7, 8]
        .into_iter()
        .flat_map(|f| [82, 85].map(|id| (f, id)))
    {
        let (mut app, mut peer, _) = fixture(ResourcePropKind::Seagrass, false);
        peer.mode = Some(GameMode::Survival);
        let position = [4, 4, 4];
        if facing == 4 {
            let mut chunks = app.world_mut().resource_mut::<ChunkStore>();
            for x in 3..=6 {
                for z in 3..=6 {
                    chunks
                        .loaded
                        .get_mut(&terra_voxel::SectionPos::default())
                        .unwrap()
                        .data
                        .set_block([x, 3, z], 0)
                        .unwrap();
                }
            }
        }
        let source = terra_core::construction::slab_bounds(facing);
        let axis = (0..3)
            .find(|&i| source.minimum[i] == 0.5 || source.maximum[i] == 0.5)
            .unwrap();
        let sign = if source.maximum[axis] == 0.5 {
            1.0
        } else {
            -1.0
        };
        let center = Vec3::from_array(position.map(|v| v as f32)) + Vec3::splat(0.5);
        let mut eye = center;
        eye[axis] += sign * 3.0;
        app.world_mut()
            .get_mut::<Transform>(peer.body)
            .unwrap()
            .translation = eye - Vec3::Y * PLAYER_EYE_OFFSET;
        app.world_mut()
            .resource_mut::<ChunkStore>()
            .loaded
            .get_mut(&terra_voxel::SectionPos::default())
            .unwrap()
            .data
            .set_block([4, 4, 4], 82)
            .unwrap();
        {
            let mut journal = app.world_mut().resource_mut::<EditJournal>();
            journal.record(position, 82).unwrap();
            journal
                .set_object_state(
                    position,
                    Some(terra_world::edits::BlockObjectState {
                        facing,
                        ..Default::default()
                    }),
                )
                .unwrap();
            journal
                .gameplay_mut()
                .inventories
                .get_mut(&peer.uuid)
                .unwrap()
                .creative_set(
                    0,
                    ItemStack {
                        item: ItemKind::Block(added),
                        count: 2,
                    },
                );
        }
        let look = aim(&app, &peer, center.to_array().map(f64::from));
        // Vertical rays exceed the player's allowed camera pitch; retain a slight horizontal offset.
        if axis == 1 {
            app.world_mut()
                .get_mut::<Transform>(peer.body)
                .unwrap()
                .translation
                .x += 0.5;
        }
        let look = if axis == 1 {
            aim(&app, &peer, center.to_array().map(f64::from))
        } else {
            look
        };
        let mut system = SystemState::<ControlWorld>::new(app.world_mut());
        let mut world = system.get_mut(app.world_mut()).unwrap();
        let command = EditCommand {
            request: 1,
            revision: world.journal.revision(terra_voxel::SectionPos::default()),
            look,
            source: None,
            action: EditAction::Block {
                position,
                expected: 82,
                material: added,
            },
        };
        assert_eq!(
            check(command, &peer, &world),
            EditOutcome::Applied,
            "facing {facing}"
        );
        assert_eq!(
            apply(command, &peer, &mut world, false),
            EditOutcome::Applied
        );
        assert_eq!(
            world.journal.object_state(position).unwrap().facing,
            terra_core::construction::merged_slab(82, facing, added).unwrap()
        );
        assert_eq!(
            world.journal.gameplay().inventories[&peer.uuid].count(ItemKind::Block(added)),
            1
        );
        let third = EditCommand {
            revision: world.journal.revision(terra_voxel::SectionPos::default()),
            ..command
        };
        assert_eq!(check(third, &peer, &world), EditOutcome::Occluded);
        let restored: terra_world::edits::BlockObjectState = serde_json::from_str(
            &serde_json::to_string(&world.journal.object_state(position).unwrap()).unwrap(),
        )
        .unwrap();
        assert!(restored.valid_for(position, 82));
        assert_eq!(
            restored.facing,
            terra_core::construction::merged_slab(82, facing, added).unwrap()
        );
        assert_eq!(world.chunks.block_at(position), Some(82));
    }
}

#[test]
fn mixed_slabs_are_mined_one_aimed_half_at_a_time_and_keep_the_other_material_and_pose() {
    for facing in [0, 4, 5, 6, 7, 8] {
        for selected in 0..2 {
            for mode in [GameMode::Survival, GameMode::Creative] {
                let (mut app, mut peer, _) = fixture(ResourcePropKind::Seagrass, false);
                peer.mode = Some(mode);
                let position = [4, 4, 4];
                let merged = terra_core::construction::merged_slab(82, facing, 85).unwrap();
                let halves = terra_core::construction::slab_halves(82, merged).unwrap();
                let bounds = terra_core::construction::slab_bounds(halves[selected].1);
                let center = std::array::from_fn(|axis| {
                    position[axis] as f64 + (bounds.minimum[axis] + bounds.maximum[axis]) * 0.5
                });
                let split_axis = (0..3)
                    .find(|&axis| bounds.maximum[axis] - bounds.minimum[axis] == 0.5)
                    .unwrap();
                let mut eye = Vec3::from_array(center.map(|v| v as f32));
                eye[if split_axis == 2 { 0 } else { 2 }] -= 3.0;
                app.world_mut()
                    .get_mut::<Transform>(peer.body)
                    .unwrap()
                    .translation = eye - Vec3::Y * PLAYER_EYE_OFFSET;
                app.world_mut()
                    .resource_mut::<ChunkStore>()
                    .loaded
                    .get_mut(&terra_voxel::SectionPos::default())
                    .unwrap()
                    .data
                    .set_block([4, 4, 4], 82)
                    .unwrap();
                {
                    let mut journal = app.world_mut().resource_mut::<EditJournal>();
                    journal.record(position, 82).unwrap();
                    journal
                        .set_object_state(
                            position,
                            Some(terra_world::edits::BlockObjectState {
                                facing: merged,
                                wood: terra_core::WoodSpecies::Birch,
                                ..Default::default()
                            }),
                        )
                        .unwrap();
                }
                equip_farm_tool(&mut app, &peer, terra_core::ToolKind::WoodPickaxe);
                let look = aim(&app, &peer, center);
                let mut system = SystemState::<ControlWorld>::new(app.world_mut());
                let mut world = system.get_mut(app.world_mut()).unwrap();
                let command = EditCommand {
                    request: 1,
                    revision: world.journal.revision(terra_voxel::SectionPos::default()),
                    look,
                    source: None,
                    action: EditAction::Block {
                        position,
                        expected: 82,
                        material: 0,
                    },
                };
                assert_eq!(
                    check(command, &peer, &world),
                    EditOutcome::Applied,
                    "{facing}/{selected}/{mode:?}"
                );
                if mode == GameMode::Survival {
                    assert_eq!(
                        apply(command, &peer, &mut world, false),
                        EditOutcome::Invalid
                    );
                    assert_eq!(world.journal.object_state(position).unwrap().facing, merged);
                }
                assert_eq!(
                    apply(command, &peer, &mut world, true),
                    EditOutcome::Applied,
                    "{facing}/{selected}/{mode:?}"
                );
                assert_eq!(
                    world.chunks.block_at(position),
                    Some(halves[1 - selected].0)
                );
                let state = world.journal.object_state(position).unwrap();
                assert_eq!(state.facing, halves[1 - selected].1);
                assert_eq!(state.wood, terra_core::WoodSpecies::Birch);
                assert!(state.valid_for(position, halves[1 - selected].0));
                let restored: terra_world::edits::BlockObjectState =
                    serde_json::from_str(&serde_json::to_string(&state).unwrap()).unwrap();
                assert_eq!(restored, state);
                let drops = &world.journal.gameplay().drops;
                assert_eq!(drops.len(), usize::from(mode == GameMode::Survival));
                if mode == GameMode::Survival {
                    assert_eq!(
                        drops[0].stack,
                        ItemStack {
                            item: ItemKind::Block(halves[selected].0),
                            count: 1
                        }
                    );
                    assert_eq!(drops[0].position, center);
                }
                let stale = command;
                assert_eq!(apply(stale, &peer, &mut world, true), EditOutcome::Conflict);
            }
        }
    }
}

#[test]
fn identical_and_legacy_double_slabs_keep_whole_mining_and_two_item_drops() {
    for facing in [
        terra_core::construction::DOUBLE_SLAB,
        terra_core::construction::merged_slab(82, 5, 82).unwrap(),
    ] {
        let (mut app, mut peer, _) = fixture(ResourcePropKind::Seagrass, false);
        peer.mode = Some(GameMode::Survival);
        let position = [4, 4, 4];
        app.world_mut()
            .resource_mut::<ChunkStore>()
            .loaded
            .get_mut(&terra_voxel::SectionPos::default())
            .unwrap()
            .data
            .set_block([4, 4, 4], 82)
            .unwrap();
        {
            let mut journal = app.world_mut().resource_mut::<EditJournal>();
            journal.record(position, 82).unwrap();
            journal
                .set_object_state(
                    position,
                    Some(terra_world::edits::BlockObjectState {
                        facing,
                        ..Default::default()
                    }),
                )
                .unwrap();
        }
        let look = aim(&app, &peer, [4.5, 4.5, 4.5]);
        let mut system = SystemState::<ControlWorld>::new(app.world_mut());
        let mut world = system.get_mut(app.world_mut()).unwrap();
        let command = EditCommand {
            request: 1,
            revision: world.journal.revision(terra_voxel::SectionPos::default()),
            look,
            source: None,
            action: EditAction::Block {
                position,
                expected: 82,
                material: 0,
            },
        };
        assert!(mined_slab(command, &peer, &world).is_none());
        assert_eq!(
            apply(command, &peer, &mut world, true),
            EditOutcome::Applied
        );
        assert_eq!(world.chunks.block_at(position), Some(0));
        assert!(world.journal.object_state(position).is_none());
        assert_eq!(
            world
                .journal
                .gameplay()
                .drops
                .iter()
                .filter(|drop| drop.stack.item == ItemKind::Block(82))
                .map(|drop| drop.stack.count)
                .sum::<u16>(),
            2
        );
    }
}

#[test]
fn floor_slab_placement_is_vertical_in_the_center_and_horizontal_near_the_edge_on_authority() {
    for (x, facing) in [(4.5, 8), (4.05, 0)] {
        let (mut app, peer, _) = fixture(ResourcePropKind::Seagrass, false);
        app.world_mut()
            .get_mut::<Transform>(peer.body)
            .unwrap()
            .translation = Vec3::new(4.5, 6.0 - PLAYER_EYE_OFFSET, 7.5);
        {
            let mut journal = app.world_mut().resource_mut::<EditJournal>();
            journal
                .gameplay_mut()
                .inventories
                .get_mut(&peer.uuid)
                .unwrap()
                .creative_set(
                    0,
                    ItemStack {
                        item: ItemKind::Block(82),
                        count: 2,
                    },
                );
        }
        let look = aim(&app, &peer, [x, 4.0, 4.5]);
        let mut system = SystemState::<ControlWorld>::new(app.world_mut());
        let mut world = system.get_mut(app.world_mut()).unwrap();
        let command = EditCommand {
            request: 1,
            revision: world.journal.revision(terra_voxel::SectionPos::default()),
            look,
            source: None,
            action: EditAction::Block {
                position: [4, 4, 4],
                expected: 0,
                material: 82,
            },
        };
        assert_eq!(check(command, &peer, &world), EditOutcome::Applied);
        assert_eq!(
            apply(command, &peer, &mut world, false),
            EditOutcome::Applied
        );
        assert_eq!(
            world.journal.object_state([4, 4, 4]).unwrap().facing,
            facing
        );
        assert_eq!(
            world.journal.gameplay().inventories[&peer.uuid].count(ItemKind::Block(82)),
            1
        );
    }
}

#[test]
fn crystal_extension_consumes_one_resource_and_rejects_forged_endpoints_and_spectators() {
    for material in [51, 52, 53] {
        let (mut app, mut peer, _) = fixture(ResourcePropKind::Seagrass, false);
        let item = if material == 51 {
            ItemKind::Mineral(terra_core::MineralItem::SaltCrystal)
        } else {
            ItemKind::Block(material)
        };
        app.world_mut()
            .resource_mut::<EditJournal>()
            .gameplay_mut()
            .inventories
            .get_mut(&peer.uuid)
            .unwrap()
            .creative_set(0, ItemStack { item, count: 2 });
        {
            let mut chunks = app.world_mut().resource_mut::<ChunkStore>();
            let section = chunks
                .loaded
                .get_mut(&terra_voxel::SectionPos::default())
                .unwrap();
            for y in 4..14 {
                section.data.set_block([4, y, 4], material).unwrap();
            }
        }
        // Aim at the side, not the top; the free tip is beyond normal reach.
        let look = aim(&app, &peer, [4.5, 5.5, 4.5]);
        let command = EditCommand {
            request: 1,
            revision: 0,
            look,
            source: None,
            action: EditAction::Block {
                position: [4, 14, 4],
                expected: 0,
                material,
            },
        };
        let mut system = SystemState::<ControlWorld>::new(app.world_mut());
        let mut world = system.get_mut(app.world_mut()).unwrap();
        let forged = EditCommand {
            action: EditAction::Block {
                position: [5, 14, 4],
                expected: 0,
                material,
            },
            ..command
        };
        assert_eq!(check(forged, &peer, &world), EditOutcome::Occluded);
        peer.mode = Some(GameMode::Spectator);
        assert_eq!(check(command, &peer, &world), EditOutcome::Invalid);
        peer.mode = Some(GameMode::Survival);
        assert_eq!(
            apply(command, &peer, &mut world, false),
            EditOutcome::Applied
        );
        assert_eq!(world.chunks.block_at([4, 14, 4]), Some(material));
        assert_eq!(
            world.journal.gameplay().inventories[&peer.uuid].count(item),
            1
        );
        assert_eq!(
            apply(command, &peer, &mut world, false),
            EditOutcome::Conflict
        );
        assert_eq!(
            world.journal.gameplay().inventories[&peer.uuid].count(item),
            1
        );
    }
}

#[test]
fn segmented_seagrass_uses_authority_water_support_inventory_and_the_existing_source() {
    for wet in [false, true] {
        let (mut app, peer, support) = fixture(ResourcePropKind::Seagrass, wet);
        let look = aim(&app, &peer, [4.5, 3.9, 4.5]);
        let command = EditCommand {
            request: 1,
            revision: 0,
            look,
            source: None,
            action: EditAction::Block {
                position: [4, 4, 4],
                expected: if wet { 27 } else { 0 },
                material: 50,
            },
        };
        let mut system = SystemState::<ControlWorld>::new(app.world_mut());
        let mut world = system.get_mut(app.world_mut()).unwrap();
        assert_eq!(
            apply(command, &peer, &mut world, false),
            if wet {
                EditOutcome::Applied
            } else {
                EditOutcome::Invalid
            }
        );
        if wet {
            assert_eq!(world.chunks.block_at([4, 4, 4]), Some(50));
            assert_eq!(world.journal.fluid([4, 4, 4]).unwrap().amount, 8);
            assert!(
                world
                    .chunks
                    .loaded
                    .values()
                    .all(|section| section.props.is_empty())
            );
            assert_eq!(
                world.journal.gameplay().inventories[&peer.uuid]
                    .count(ItemKind::Prop(ResourcePropKind::Seagrass)),
                1
            );
            assert_eq!(support, [4, 3, 4]);
        }
    }
}

#[test]
fn scarecrow_requires_a_full_cube_and_two_dry_free_cells() {
    let (mut app, _, support) = fixture(ResourcePropKind::Scarecrow, false);
    let owner = terra_voxel::global_to_section(support).0;
    let valid = |app: &App| {
        prop_environment_valid(
            app.world().resource::<ChunkStore>(),
            &app.world().resource::<WorldTerrain>().0,
            app.world().resource::<EditJournal>(),
            app.world()
                .resource::<terra_core::block_properties::BlockPropertiesCatalog>(),
            support,
            ResourcePropKind::Scarecrow,
        )
    };
    for (material, allowed) in [
        (2, true),
        (3, true),
        (101, true),
        (82, false),
        (74, false),
        (29, false),
        (98, false),
    ] {
        app.world_mut()
            .resource_mut::<ChunkStore>()
            .loaded
            .get_mut(&owner)
            .unwrap()
            .data
            .set_block([4, 3, 4], material)
            .unwrap();
        assert_eq!(valid(&app), allowed, "support material {material}");
    }
    app.world_mut()
        .resource_mut::<ChunkStore>()
        .loaded
        .get_mut(&owner)
        .unwrap()
        .data
        .set_block([4, 3, 4], 3)
        .unwrap();
    for dy in 1..=2 {
        let position = [support[0], support[1] + dy, support[2]];
        app.world_mut()
            .resource_mut::<EditJournal>()
            .set_fluid(
                position,
                terra_world::water::FluidCell {
                    amount: 4,
                    flow: [0; 2],
                    source: false,
                },
            )
            .unwrap();
        assert!(!valid(&app), "flowing water in air cell {position:?}");
        app.world_mut()
            .resource_mut::<EditJournal>()
            .set_fluid(position, terra_world::water::FluidCell::default())
            .unwrap();
        assert!(valid(&app));
    }
    app.world_mut()
        .resource_mut::<terra_core::block_properties::BlockPropertiesCatalog>()
        .1
        .insert(
            3,
            std::sync::Arc::from([terra_core::block_shapes::ShapeBox {
                minimum: [0.0; 3],
                maximum: [1.0, 0.5, 1.0],
                hinged: false,
            }]),
        );
    assert!(
        !valid(&app),
        "data-defined partial shapes also cannot leave a floating puppet"
    );
}

#[test]
fn marine_placement_checks_real_source_cells_depth_and_loaded_ground() {
    for kind in ResourcePropKind::ALL
        .into_iter()
        .filter(|kind| ItemKind::Prop(*kind).valid())
        .filter(|kind| terra_core::CropKind::from_prop(*kind).is_none())
    {
        let (mut app, _, support) = fixture(kind, true);
        let valid = |app: &App| {
            prop_environment_valid(
                app.world().resource::<ChunkStore>(),
                &app.world().resource::<WorldTerrain>().0,
                app.world().resource::<EditJournal>(),
                app.world()
                    .resource::<terra_core::block_properties::BlockPropertiesCatalog>(),
                support,
                kind,
            )
        };
        assert_eq!(valid(&app), PropKind::from(kind).aquatic(), "{kind:?}");
        app.world_mut()
            .resource_mut::<ChunkStore>()
            .loaded
            .get_mut(&terra_voxel::global_to_section(support).0)
            .unwrap()
            .data
            .set_block([4, 4, 4], MaterialId::FluidWater as u16)
            .unwrap();
        assert_eq!(
            valid(&app),
            PropKind::from(kind).aquatic(),
            "source {kind:?}"
        );
        app.world_mut()
            .resource_mut::<ChunkStore>()
            .loaded
            .get_mut(&terra_voxel::global_to_section(support).0)
            .unwrap()
            .data
            .set_block([4, 4, 4], MaterialId::Stone as u16)
            .unwrap();
        assert!(!valid(&app), "solid obstruction {kind:?}");
        let (dry, _, support) = fixture(kind, false);
        assert_eq!(
            prop_environment_valid(
                dry.world().resource::<ChunkStore>(),
                &dry.world().resource::<WorldTerrain>().0,
                dry.world().resource::<EditJournal>(),
                dry.world()
                    .resource::<terra_core::block_properties::BlockPropertiesCatalog>(),
                support,
                kind
            ),
            !PropKind::from(kind).aquatic() || kind == ResourcePropKind::SmallStones
        );
    }
    let kind = ResourcePropKind::Seagrass;
    let (mut app, _, support) = fixture(kind, true);
    for x in 2..=6 {
        for z in 2..=6 {
            for y in 4..=8 {
                app.world_mut()
                    .resource_mut::<EditJournal>()
                    .set_fluid([x, y, z], terra_world::water::FluidCell::default())
                    .unwrap();
            }
        }
    }
    app.world_mut()
        .resource_mut::<EditJournal>()
        .set_fluid(
            [4, 4, 4],
            terra_world::water::FluidCell {
                amount: 1,
                flow: [0; 2],
                source: false,
            },
        )
        .unwrap();
    assert!(
        !prop_environment_valid(
            app.world().resource::<ChunkStore>(),
            &app.world().resource::<WorldTerrain>().0,
            app.world().resource::<EditJournal>(),
            app.world()
                .resource::<terra_core::block_properties::BlockPropertiesCatalog>(),
            support,
            kind
        ),
        "one shallow water layer cannot submerge seagrass"
    );
}

#[test]
fn every_prop_uses_authority_placement_accurate_picking_harvest_drop_and_reload_tombstone() {
    for kind in ResourcePropKind::ALL
        .into_iter()
        .filter(|kind| ItemKind::Prop(*kind).valid())
        .filter(|kind| terra_core::CropKind::from_prop(*kind).is_none())
    {
        let (mut app, peer, support) = fixture(kind, PropKind::from(kind).aquatic());
        let owner = terra_voxel::global_to_section(support).0;
        let look = aim(&app, &peer, [4.5, 4.0, 4.5]);
        let command = EditCommand {
            request: 1,
            revision: 0,
            look,
            source: None,
            action: EditAction::PlaceProp { support, kind },
        };
        {
            let mut system = SystemState::<ControlWorld>::new(app.world_mut());
            let mut world = system.get_mut(app.world_mut()).unwrap();
            assert_eq!(
                apply(command, &peer, &mut world, false),
                EditOutcome::Applied,
                "{kind:?}"
            );
            assert_eq!(
                world.journal.gameplay().inventories[&peer.uuid].count(ItemKind::Prop(kind)),
                1
            );
        }
        let prop = app
            .world()
            .resource::<EditJournal>()
            .section_delta(owner)
            .placed_props[0];
        let [low, high] = prop.bounds();
        let mut removal_look = None;
        'search: for y in 1..=8 {
            for z in 1..=8 {
                for x in 1..=8 {
                    let target = [
                        low[0] + (high[0] - low[0]) * x as f64 / 9.0,
                        low[1] + (high[1] - low[1]) * y as f64 / 9.0,
                        low[2] + (high[2] - low[2]) * z as f64 / 9.0,
                    ];
                    let look = aim(&app, &peer, target);
                    let eye = app.world().get::<Transform>(peer.body).unwrap().translation
                        + Vec3::Y * PLAYER_EYE_OFFSET;
                    let direction =
                        Quat::from_euler(EulerRot::YXZ, look[0], look[1], 0.0) * Vec3::NEG_Z;
                    let (_, selected) = pick_world_with_context(
                        app.world().resource::<ChunkStore>(),
                        app.world().resource::<EditJournal>(),
                        app.world()
                            .resource::<terra_core::block_properties::BlockPropertiesCatalog>(),
                        eye.to_array().map(f64::from),
                        direction.to_array().map(f64::from),
                        PropPose::Rest,
                    );
                    if matches!(selected,Some(WorldTarget::Prop(hit)) if hit.prop.id == prop.id) {
                        removal_look = Some(look);
                        break 'search;
                    }
                }
            }
        }
        let command = EditCommand {
            request: 2,
            revision: app.world().resource::<EditJournal>().prop_epoch(owner),
            look: removal_look.unwrap_or_else(|| panic!("model can be targeted: {kind:?}")),
            source: None,
            action: EditAction::Prop {
                section: owner,
                id: prop.id,
            },
        };
        {
            let mut system = SystemState::<ControlWorld>::new(app.world_mut());
            let mut world = system.get_mut(app.world_mut()).unwrap();
            assert_eq!(
                apply(command, &peer, &mut world, true),
                EditOutcome::Applied,
                "{kind:?}"
            );
            let expected_drops = world.journal.gameplay().drops.clone();
            assert!((1..=2).contains(&expected_drops.len()));
            assert_eq!(expected_drops[0].stack.count, 1);
            assert_eq!(
                world.journal.gameplay().drops[0].stack.item,
                if kind == ResourcePropKind::Flower {
                    ItemKind::Flower(terra_core::crusher::flower_color(prop.variant))
                } else {
                    ItemKind::Prop(kind)
                }
            );
            if let Some(seed_drop) = expected_drops.get(1) {
                assert!(matches!(
                    kind,
                    ResourcePropKind::Grass
                        | ResourcePropKind::Flower
                        | ResourcePropKind::SmallPlant
                ));
                assert_eq!(seed_drop.stack.count, 1);
                assert!(seed_drop.stack.valid());
                let ItemKind::Crop(seed) = seed_drop.stack.item else {
                    panic!("Only a crop seed may supplement a wild prop harvest");
                };
                assert!(seed.crop_kind().is_some());
            }
            if let ItemKind::Flower(color) = world.journal.gameplay().drops[0].stack.item {
                assert_eq!(color, terra_core::crusher::flower_color(prop.variant));
                assert_eq!(
                    terra_core::crusher::recipe_for(ItemKind::Flower(color))
                        .unwrap()
                        .output
                        .item,
                    ItemKind::Dye(color)
                );
            }
            assert_eq!(
                apply(command, &peer, &mut world, true),
                EditOutcome::Conflict
            );
            assert_eq!(world.journal.gameplay().drops, expected_drops);
        }
        let delta = app.world().resource::<EditJournal>().section_delta(owner);
        let mut loaded = EditJournal::default();
        loaded.import_delta(delta).unwrap();
        let mut props = vec![prop];
        loaded.apply_prop_removals(owner, &mut props);
        assert!(
            props[0].removed,
            "{kind:?} must not return after section reload"
        );
    }
}

fn aim_crop(app: &App, peer: &Peer, prop: terra_world::props::WorldProp) -> [f32; 2] {
    let [low, high] = prop.bounds();
    for y in 1..=8 {
        for z in 1..=8 {
            for x in 1..=8 {
                let target = [
                    low[0] + (high[0] - low[0]) * x as f64 / 9.0,
                    low[1] + (high[1] - low[1]) * y as f64 / 9.0,
                    low[2] + (high[2] - low[2]) * z as f64 / 9.0,
                ];
                let look = aim(app, peer, target);
                let eye = app.world().get::<Transform>(peer.body).unwrap().translation
                    + Vec3::Y * PLAYER_EYE_OFFSET;
                let direction =
                    Quat::from_euler(EulerRot::YXZ, look[0], look[1], 0.0) * Vec3::NEG_Z;
                let (_, target) = pick_world_with_context(
                    app.world().resource::<ChunkStore>(),
                    app.world().resource::<EditJournal>(),
                    app.world()
                        .resource::<terra_core::block_properties::BlockPropertiesCatalog>(),
                    eye.to_array().map(f64::from),
                    direction.to_array().map(f64::from),
                    PropPose::Rest,
                );
                if matches!(target, Some(WorldTarget::Prop(hit)) if hit.prop.id == prop.id) {
                    return look;
                }
            }
        }
    }
    panic!(
        "Crop model must be pickable: {:?} stage {}",
        prop.kind, prop.variant
    )
}

#[test]
fn lettuce_seeds_plant_prepared_farmland_at_negative_section_edges_and_replant_authoritatively() {
    use terra_core::CropKind;
    let kind = CropKind::Lettuce;
    for support in [[4, 3, 4], [-1, 15, -1]] {
        let (mut app, peer, _) = fixture(kind.resource_prop(), false);
        let (owner, local) = terra_voxel::global_to_section(support);
        let mut data = terra_voxel::BlockSection::empty(owner).unwrap();
        data.set_block(local, terra_core::farming::FARMLAND)
            .unwrap();
        {
            let mut chunks = app.world_mut().resource_mut::<ChunkStore>();
            chunks.loaded.clear();
            for dx in -1..=1 {
                for dz in -1..=1 {
                    for dy in 0..=1 {
                        let position = owner.offset([dx, dy, dz]).unwrap();
                        chunks.loaded.insert(
                            position,
                            LoadedSection {
                                data: terra_voxel::BlockSection::empty(position).unwrap(),
                                mesh: None,
                                mesh_epoch: 0,
                                water_epoch: 0,
                                water: None,
                                props: vec![],
                            },
                        );
                    }
                }
            }
            chunks.loaded.insert(
                owner,
                LoadedSection {
                    data,
                    mesh: None,
                    mesh_epoch: 0,
                    water_epoch: 0,
                    water: None,
                    props: vec![],
                },
            );
            let above = terra_voxel::global_to_section([support[0], support[1] + 1, support[2]]).0;
            if above != owner {
                chunks.loaded.insert(
                    above,
                    LoadedSection {
                        data: terra_voxel::BlockSection::empty(above).unwrap(),
                        mesh: None,
                        mesh_epoch: 0,
                        water_epoch: 0,
                        water: None,
                        props: vec![],
                    },
                );
            }
        }
        app.world_mut()
            .get_mut::<Transform>(peer.body)
            .unwrap()
            .translation = Vec3::new(
            support[0] as f32 + 0.5,
            support[1] as f32 + 3.0,
            support[2] as f32 + 4.5,
        );
        let look = aim(
            &app,
            &peer,
            [
                support[0] as f64 + 0.5,
                support[1] as f64 + 1.0,
                support[2] as f64 + 0.5,
            ],
        );
        let source = crate::protocol::session::InventorySource {
            slot: 0,
            revision: app.world().resource::<EditJournal>().gameplay().inventories[&peer.uuid]
                .revision,
        };
        let place = EditCommand {
            request: 1,
            revision: 0,
            look,
            source: Some(source),
            action: EditAction::PlaceProp {
                support,
                kind: kind.resource_prop(),
            },
        };
        {
            let mut system = SystemState::<ControlWorld>::new(app.world_mut());
            let mut world = system.get_mut(app.world_mut()).unwrap();
            assert_eq!(world.journal.block(support), None);
            assert_eq!(apply(place, &peer, &mut world, false), EditOutcome::Applied);
            assert_eq!(
                world.journal.gameplay().inventories[&peer.uuid].count(ItemKind::Crop(kind.seed())),
                1
            );
            assert_eq!(
                world.journal.block(support),
                Some(terra_core::farming::FARMLAND)
            );
            assert_eq!(
                apply(place, &peer, &mut world, false),
                EditOutcome::Conflict
            );
            let mut ripe = *world.journal.crop_at(support).unwrap();
            ripe.progress_seconds = kind.definition().growth_seconds;
            terra_world::farming::commit_crop_state(
                &mut world.chunks,
                &mut world.journal,
                support,
                ripe,
            )
            .unwrap();
        }
        let prop = app
            .world()
            .resource::<EditJournal>()
            .section_delta(owner)
            .placed_props[0];
        let harvest = EditCommand {
            request: 2,
            revision: app.world().resource::<EditJournal>().prop_epoch(owner),
            look: aim_crop(&app, &peer, prop),
            source: None,
            action: EditAction::Prop {
                section: owner,
                id: prop.id,
            },
        };
        let mut system = SystemState::<ControlWorld>::new(app.world_mut());
        let mut world = system.get_mut(app.world_mut()).unwrap();
        assert_eq!(
            apply(harvest, &peer, &mut world, true),
            EditOutcome::Applied
        );
        let mesh = world.journal.mesh_epoch(owner);
        let source = crate::protocol::session::InventorySource {
            slot: 0,
            revision: world.journal.gameplay().inventories[&peer.uuid].revision,
        };
        let replant = EditCommand {
            request: 3,
            revision: world.journal.prop_epoch(owner),
            source: Some(source),
            ..place
        };
        assert_eq!(
            apply(replant, &peer, &mut world, false),
            EditOutcome::Applied
        );
        assert_eq!(world.journal.mesh_epoch(owner), mesh);
        assert_eq!(world.journal.crop_at(support).unwrap().kind, kind);
        assert_eq!(
            world.journal.gameplay().inventories[&peer.uuid].count(ItemKind::Crop(kind.seed())),
            0
        );
    }
}

#[test]
fn lettuce_seeds_do_not_replace_existing_decoration_or_consume_items_on_rejection() {
    let kind = terra_core::CropKind::Lettuce;
    for blocker in [PropKind::Grass, PropKind::Flower, PropKind::Mushroom] {
        let (mut app, peer, support) = fixture(kind.resource_prop(), false);
        let owner = terra_voxel::global_to_section(support).0;
        let mut decoration = placed_prop(support, ResourcePropKind::Grass, 0);
        decoration.kind = blocker;
        app.world_mut()
            .resource_mut::<ChunkStore>()
            .loaded
            .get_mut(&owner)
            .unwrap()
            .props
            .push(decoration);
        let source = crate::protocol::session::InventorySource {
            slot: 0,
            revision: app.world().resource::<EditJournal>().gameplay().inventories[&peer.uuid]
                .revision,
        };
        let command = EditCommand {
            request: 1,
            revision: 0,
            look: aim(&app, &peer, [4.5, 4.0, 4.5]),
            source: Some(source),
            action: EditAction::PlaceProp {
                support,
                kind: kind.resource_prop(),
            },
        };
        let mut system = SystemState::<ControlWorld>::new(app.world_mut());
        let mut world = system.get_mut(app.world_mut()).unwrap();
        assert_ne!(
            apply(command, &peer, &mut world, false),
            EditOutcome::Applied
        );
        assert_eq!(
            world.journal.gameplay().inventories[&peer.uuid].count(ItemKind::Crop(kind.seed())),
            2
        );
        assert!(world.journal.crop_at(support).is_none());
        assert_eq!(world.chunks.loaded[&owner].props, vec![decoration]);
    }
}

#[test]
fn crops_sow_seed_only_till_harvest_atomically_replant_and_reject_stale_replays() {
    use terra_core::{CropKind, CropState};
    for kind in CropKind::ALL {
        let (mut app, peer, support) = fixture(kind.resource_prop(), false);
        let owner = terra_voxel::global_to_section(support).0;
        app.world_mut()
            .resource_mut::<ChunkStore>()
            .loaded
            .get_mut(&owner)
            .unwrap()
            .data
            .set_block([4, 3, 4], MaterialId::Dirt as u16)
            .unwrap();
        let look = aim(&app, &peer, [4.5, 4.0, 4.5]);
        let place = EditCommand {
            request: 1,
            revision: 0,
            look,
            source: None,
            action: EditAction::PlaceProp {
                support,
                kind: kind.resource_prop(),
            },
        };
        {
            let mut system = SystemState::<ControlWorld>::new(app.world_mut());
            let mut world = system.get_mut(app.world_mut()).unwrap();
            assert_eq!(apply(place, &peer, &mut world, false), EditOutcome::Applied);
            assert_eq!(
                world.journal.gameplay().inventories[&peer.uuid].count(ItemKind::Crop(kind.seed())),
                1
            );
            assert_eq!(
                world.chunks.block_at(support),
                Some(terra_core::farming::FARMLAND)
            );
            assert_eq!(world.journal.crop_at(support).unwrap().kind, kind);
            let mut ripe = *world.journal.crop_at(support).unwrap();
            ripe.progress_seconds = kind.definition().growth_seconds;
            terra_world::farming::commit_crop_state(
                &mut world.chunks,
                &mut world.journal,
                support,
                ripe,
            )
            .unwrap();
        }
        let prop = app
            .world()
            .resource::<EditJournal>()
            .section_delta(owner)
            .placed_props[0];
        let harvest = EditCommand {
            request: 2,
            revision: app.world().resource::<EditJournal>().prop_epoch(owner),
            look: aim_crop(&app, &peer, prop),
            source: None,
            action: EditAction::Prop {
                section: owner,
                id: prop.id,
            },
        };
        {
            let mut system = SystemState::<ControlWorld>::new(app.world_mut());
            let mut world = system.get_mut(app.world_mut()).unwrap();
            world.journal.gameplay_mut().next_drop = u64::MAX - 1;
            assert_eq!(
                apply(harvest, &peer, &mut world, true),
                EditOutcome::RateLimited
            );
            assert!(world.journal.crop_at(support).unwrap().mature());
            assert!(!world.journal.prop_removed(owner, prop.id));
            world.journal.gameplay_mut().next_drop = 0;
            let expected = CropState {
                kind,
                prop_id: prop.id.0,
                progress_seconds: kind.definition().growth_seconds,
                growth_duration_seconds: kind.definition().growth_seconds,
                moisture: 0.5,
            }
            .harvest();
            assert_eq!(
                apply(harvest, &peer, &mut world, true),
                EditOutcome::Applied
            );
            assert_eq!(
                world
                    .journal
                    .gameplay()
                    .drops
                    .iter()
                    .map(|drop| drop.stack)
                    .collect::<Vec<_>>(),
                expected
            );
            assert!(world.journal.crop_at(support).is_none());
            assert_eq!(
                apply(harvest, &peer, &mut world, true),
                EditOutcome::Conflict
            );
            assert_eq!(world.journal.gameplay().drops.len(), 2);
            let mut replant = place;
            replant.request = 3;
            replant.revision = world.journal.prop_epoch(owner);
            assert_eq!(
                apply(replant, &peer, &mut world, false),
                EditOutcome::Applied
            );
            assert_eq!(
                world.journal.gameplay().inventories[&peer.uuid].count(ItemKind::Crop(kind.seed())),
                0
            );
            assert_eq!(
                world.journal.crop_at(support).unwrap().progress_seconds,
                0.0
            );
            assert_eq!(world.journal.crop_at(support).unwrap().prop_id, prop.id.0);
            assert_eq!(
                apply(harvest, &peer, &mut world, true),
                EditOutcome::Conflict
            );
        }
    }
}

#[test]
fn legacy_prop_items_cannot_forge_seeds_and_wild_seed_drops_are_stable_and_obtainable() {
    for kind in terra_core::CropKind::ALL {
        let (mut app, peer, support) = fixture(kind.resource_prop(), false);
        let mut bag = Inventory::default();
        bag.creative_set(
            0,
            ItemStack {
                item: ItemKind::Prop(kind.resource_prop()),
                count: 2,
            },
        );
        let revision = bag.revision;
        app.world_mut()
            .resource_mut::<EditJournal>()
            .gameplay_mut()
            .inventories
            .insert(peer.uuid, bag);
        let command = EditCommand {
            request: 1,
            revision: 0,
            look: aim(&app, &peer, [4.5, 4.0, 4.5]),
            source: Some(crate::protocol::session::InventorySource { slot: 0, revision }),
            action: EditAction::PlaceProp {
                support,
                kind: kind.resource_prop(),
            },
        };
        let mut system = SystemState::<ControlWorld>::new(app.world_mut());
        let world = system.get_mut(app.world_mut()).unwrap();
        assert_eq!(check(command, &peer, &world), EditOutcome::Invalid);
    }
    let owner = terra_voxel::SectionPos::new(0, 0, 0);
    let mut chunks = ChunkStore::default();
    chunks.loaded.insert(
        owner,
        LoadedSection {
            data: terra_voxel::BlockSection::empty(owner).unwrap(),
            mesh: None,
            mesh_epoch: 0,
            water_epoch: 0,
            water: None,
            props: Vec::new(),
        },
    );
    let journal = EditJournal::default();
    let mut found = std::collections::HashSet::new();
    for id in 1..=1024 {
        let prop = terra_world::props::WorldProp {
            id: terra_world::props::PropId(id),
            kind: PropKind::Grass,
            position: [4.5, 4.0, 4.5],
            rotation: 0.0,
            scale: 1.0,
            variant: 0,
            section: owner,
            removed: false,
        };
        chunks.loaded.get_mut(&owner).unwrap().props = vec![prop];
        let action = EditAction::Prop {
            section: owner,
            id: prop.id,
        };
        let drops = resource_stacks(action, &chunks, &journal);
        assert_eq!(drops, resource_stacks(action, &chunks, &journal));
        for (_, stack) in drops {
            if let ItemKind::Crop(seed) = stack.item {
                found.insert(seed);
            }
        }
    }
    assert_eq!(found.len(), 3);
}

#[path = "server_water_interaction_tests.rs"]
mod water_interaction_tests;

#[test]
fn derived_apple_fruit_is_rejected_by_the_actual_prop_placement_transaction() {
    let (mut app, peer, support) = fixture(ResourcePropKind::AppleFruit, false);
    let look = aim(&app, &peer, [4.5, 4., 4.5]);
    let command = EditCommand {
        request: 1,
        revision: 0,
        look,
        source: None,
        action: EditAction::PlaceProp {
            support,
            kind: ResourcePropKind::AppleFruit,
        },
    };
    let mut system = SystemState::<ControlWorld>::new(app.world_mut());
    let mut world = system.get_mut(app.world_mut()).unwrap();
    assert_eq!(
        apply(command, &peer, &mut world, false),
        EditOutcome::Invalid
    );
    assert!(
        world
            .journal
            .section_delta(terra_voxel::global_to_section(support).0)
            .placed_props
            .is_empty()
    );
}
