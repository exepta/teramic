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

fn fixture(kind: ResourcePropKind, wet: bool) -> (App, Peer, [i64; 3]) {
    let support = [4, 3, 4];
    let mut config = GameConfig::default();
    config.server.game_mode = GameMode::Survival;
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
        .init_resource::<crate::server::ConnectedProfiles>();
    let owner = terra_voxel::global_to_section(support).0;
    let mut data = terra_voxel::BlockSection::empty(owner).unwrap();
    for x in 0..16 {
        for z in 0..16 {
            data.set_block([x, 3, z], 3).unwrap();
        }
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
    let uuid = PlayerUuid::new_v4();
    {
        let mut journal = app.world_mut().resource_mut::<EditJournal>();
        let mut bag = Inventory::default();
        bag.creative_set(
            0,
            ItemStack {
                item: ItemKind::Prop(kind),
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
        chest_cursor: None,
        creature_action_at: now,
        container: None,
        held_slot: Some(0),
        mode: Some(GameMode::Survival),
        generation: 0,
        chat: Default::default(),
        uuid,
        gender: Default::default(),
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
        last_movement: Default::default(),
        vitals: Default::default(),
        survival: Default::default(),
        stats_sent: None,
    };
    (app, peer, support)
}

fn aim(app: &App, peer: &Peer, target: [f64; 3]) -> [f32; 2] {
    let eye =
        app.world().get::<Transform>(peer.body).unwrap().translation + Vec3::Y * PLAYER_EYE_OFFSET;
    let direction = (Vec3::from_array(target.map(|v| v as f32)) - eye).normalize();
    [(-direction.x).atan2(-direction.z), direction.y.asin()]
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
fn marine_placement_checks_real_source_cells_depth_and_loaded_ground() {
    for kind in ResourcePropKind::ALL {
        let (mut app, _, support) = fixture(kind, true);
        let valid = |app: &App| {
            prop_environment_valid(
                app.world().resource::<ChunkStore>(),
                &app.world().resource::<WorldTerrain>().0,
                app.world().resource::<EditJournal>(),
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
            support,
            kind
        ),
        "one shallow water layer cannot submerge seagrass"
    );
}

#[test]
fn every_prop_uses_authority_placement_accurate_picking_harvest_drop_and_reload_tombstone() {
    for kind in ResourcePropKind::ALL {
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
            assert_eq!(world.journal.gameplay().drops.len(), 1);
            assert_eq!(
                world.journal.gameplay().drops[0].stack.item,
                ItemKind::Prop(kind)
            );
            assert_eq!(
                apply(command, &peer, &mut world, true),
                EditOutcome::Conflict
            );
            assert_eq!(world.journal.gameplay().drops.len(), 1);
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
