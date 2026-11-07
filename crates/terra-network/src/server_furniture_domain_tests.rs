use super::*;
use bevy::ecs::system::SystemState;
use terra_core::{GameMode, Inventory, ItemKind, ItemStack, MineralItem, ToolKind};

fn place_port(app: &mut App, position: [i64; 3], material: u16, peer: [i64; 3]) {
    let mut system = SystemState::<ControlWorld>::new(app.world_mut());
    let mut world = system.get_mut(app.world_mut()).unwrap();
    let edit = BlockEditRequest {
        position,
        expected_material: 0,
        material,
    };
    commit_block_edit(
        &mut world.chunks,
        &mut world.journal,
        &edit,
        &world.block_properties,
    )
    .unwrap();
    commit_object_state(
        &mut world.chunks,
        &mut world.journal,
        position,
        Some(BlockObjectState {
            peer: Some(peer),
            ..Default::default()
        }),
    )
    .unwrap();
}

fn removal_peer(app: &mut App, position: [i64; 3], mode: GameMode, tool: Option<ItemKind>) -> Peer {
    let uuid = terra_core::PlayerUuid::new_v4();
    let mut inventory = Inventory::default();
    if let Some(tool) = tool {
        assert_eq!(inventory.insert(tool, 1), 0);
    }
    app.world_mut()
        .resource_mut::<EditJournal>()
        .gameplay_mut()
        .inventories
        .insert(uuid, inventory);
    let target = {
        let chunks = app.world().resource::<ChunkStore>();
        let journal = app.world().resource::<EditJournal>();
        let properties = app
            .world()
            .resource::<terra_core::block_properties::BlockPropertiesCatalog>();
        let mut bounds = Vec::new();
        terra_world::interaction_shapes::visit_boxes(chunks, journal, properties, position, |b| {
            bounds.push(b)
        });
        let bounds = bounds.first().unwrap();
        Vec3::from_array(std::array::from_fn(|axis| {
            ((bounds.minimum[axis] + bounds.maximum[axis]) * 0.5) as f32
        }))
    };
    let body_position = Vec3::from_array(position.map(|v| v as f32)) + Vec3::new(0.5, 1.0, 3.5);
    let body = app
        .world_mut()
        .spawn((
            ServerCharacter {
                link: Entity::PLACEHOLDER,
            },
            Transform::from_translation(body_position),
        ))
        .id();
    let direction =
        (target - body_position - Vec3::Y * terra_physics::character::PLAYER_EYE_OFFSET)
            .normalize();
    let look = [(-direction.x).atan2(-direction.z), direction.y.asin()];
    let now = Instant::now();
    Peer {
        farming: Default::default(),
        crank: Default::default(),
        chest_cursor: None,
        creature_action_at: now,
        container: None,
        held_slot: tool.map(|_| 0),
        mode: Some(mode),
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
        look,
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
    }
}

#[test]
fn survival_and_creative_can_remove_ports_without_touching_attached_inventories_or_progress() {
    for mode in [GameMode::Survival, GameMode::Creative] {
        for (material, attached) in [(152, 59), (152, 99), (153, 29)] {
            let mut app = super::tests::app();
            app.insert_resource(
                terra_core::block_properties::BlockPropertiesCatalog::load(
                    &terra_core::asset_directory().join("terra/blocks"),
                )
                .unwrap(),
            );
            let source = [4, 1, 8];
            let position = [4, 1, 9];
            super::tests::edit(&mut app, source, 0, attached, 0.0).unwrap();
            let inventory = if attached == 59 {
                Inventory {
                    slots: vec![
                        ItemStack {
                            item: ItemKind::Mineral(MineralItem::RawIron),
                            count: 3,
                        },
                        ItemStack {
                            item: ItemKind::Mineral(MineralItem::Coal),
                            count: 2,
                        },
                        ItemStack {
                            item: ItemKind::Mineral(MineralItem::IronIngot),
                            count: 5,
                        },
                    ],
                    ..Default::default()
                }
            } else if attached == 99 {
                let recipe = &terra_core::crusher::recipes()[0];
                Inventory {
                    slots: vec![ItemStack {
                        item: recipe.input,
                        count: 3,
                    }],
                    ..Default::default()
                }
            } else {
                Inventory {
                    slots: vec![ItemStack {
                        item: ItemKind::Block(3),
                        count: 12,
                    }],
                    ..Default::default()
                }
            };
            app.world_mut()
                .resource_mut::<EditJournal>()
                .set_container(source, inventory.clone())
                .unwrap();
            if attached == 59 {
                app.world_mut()
                    .resource_mut::<EditJournal>()
                    .set_furnace(
                        source,
                        terra_core::furnace::FurnaceState {
                            progress_seconds: 0.125,
                            recipe: Some(
                                terra_core::furnace::recipe_for(ItemKind::Mineral(
                                    MineralItem::RawIron,
                                ))
                                .unwrap()
                                .id,
                            ),
                            burn_remaining: 3.0,
                            burn_total: 8.0,
                            completed: 7,
                            ..Default::default()
                        },
                    )
                    .unwrap();
            } else if attached == 99 {
                app.world_mut()
                    .resource_mut::<EditJournal>()
                    .set_crusher(
                        source,
                        terra_core::crusher::CrusherState {
                            progress_seconds: 0.125,
                            recipe: Some(terra_core::crusher::recipes()[0].id),
                            completed: 7,
                        },
                    )
                    .unwrap();
            }
            place_port(&mut app, position, material, source);
            let source_state = app.world().resource::<EditJournal>().object_state(source);
            let furnace = app
                .world()
                .resource::<EditJournal>()
                .furnace_at(source)
                .cloned();
            let crusher = app
                .world()
                .resource::<EditJournal>()
                .crusher_at(source)
                .cloned();
            let peer = removal_peer(
                &mut app,
                position,
                mode,
                Some(ItemKind::Tool(ToolKind::IronPickaxe)),
            );
            let mut system = SystemState::<ControlWorld>::new(app.world_mut());
            let mut world = system.get_mut(app.world_mut()).unwrap();
            let command = EditCommand {
                request: 1,
                revision: world
                    .journal
                    .revision(terra_voxel::global_to_section(position).0),
                look: peer.look,
                source: None,
                action: EditAction::Block {
                    position,
                    expected: material,
                    material: 0,
                },
            };
            if mode == GameMode::Survival {
                let tool = world.journal.gameplay().inventories[&peer.uuid].slots[0];
                world
                    .journal
                    .gameplay_mut()
                    .inventories
                    .get_mut(&peer.uuid)
                    .unwrap()
                    .slots[0]
                    .item = ItemKind::Tool(ToolKind::StoneAxe);
                assert_eq!(
                    super::super::validation::check(command, &peer, &world),
                    EditOutcome::Invalid
                );
                world
                    .journal
                    .gameplay_mut()
                    .inventories
                    .get_mut(&peer.uuid)
                    .unwrap()
                    .slots[0] = tool;
                assert_eq!(
                    super::super::validation::apply(command, &peer, &mut world, false),
                    EditOutcome::Invalid
                );
            }
            assert_eq!(
                super::super::validation::check(command, &peer, &world),
                EditOutcome::Applied
            );
            assert_eq!(
                super::super::validation::apply(command, &peer, &mut world, true),
                EditOutcome::Applied
            );
            assert_eq!(world.chunks.block_at(position), Some(0));
            assert!(world.journal.object_state(position).is_none());
            assert_eq!(world.chunks.block_at(source), Some(attached));
            assert_eq!(world.journal.object_state(source), source_state);
            assert_eq!(world.journal.container_at(source), Some(&inventory));
            assert_eq!(world.journal.furnace_at(source), furnace.as_ref());
            assert_eq!(world.journal.crusher_at(source), crusher.as_ref());
            let drops = &world.journal.gameplay().drops;
            assert_eq!(drops.len(), usize::from(mode == GameMode::Survival));
            if mode == GameMode::Survival {
                assert_eq!(
                    drops[0].stack,
                    ItemStack {
                        item: ItemKind::Block(material),
                        count: 1
                    }
                );
            }
            let gameplay = world.journal.gameplay().clone();
            assert_ne!(
                super::super::validation::apply(command, &peer, &mut world, true),
                EditOutcome::Applied
            );
            assert_eq!(world.journal.gameplay(), &gameplay);
        }
    }
}

#[test]
fn side_chests_align_to_the_existing_half_and_keep_contents_and_importer_when_split() {
    let mut app = super::tests::app();
    let first = [-1, 1, 8];
    let second = [0, 1, 8];
    let port = [-1, 1, 9];
    super::tests::wooden_edit(&mut app, first, 29, terra_core::WoodSpecies::Birch);
    let contents = Inventory {
        slots: vec![ItemStack {
            item: ItemKind::Block(3),
            count: 12,
        }],
        ..Default::default()
    };
    app.world_mut()
        .resource_mut::<EditJournal>()
        .set_container(first, contents.clone())
        .unwrap();
    place_port(&mut app, port, 153, first);
    super::tests::edit(&mut app, second, 0, 29, std::f32::consts::FRAC_PI_2).unwrap();
    let journal = app.world().resource::<EditJournal>();
    let primary = journal.object_state(first).unwrap();
    let secondary = journal.object_state(second).unwrap();
    assert_eq!(primary.peer, Some(second));
    assert_eq!(secondary.peer, Some(first));
    assert!(!primary.secondary && secondary.secondary);
    assert_eq!(primary.facing, secondary.facing);
    assert_eq!(primary.wood, terra_core::WoodSpecies::Birch);
    assert_eq!(secondary.wood, terra_core::WoodSpecies::Oak);
    assert_eq!(journal.container_at(first), Some(&contents));
    assert_eq!(journal.object_state(port).unwrap().peer, Some(first));
    super::tests::edit(&mut app, second, 29, 0, 0.0).unwrap();
    let journal = app.world().resource::<EditJournal>();
    assert_eq!(
        journal.object_state(first),
        Some(BlockObjectState {
            peer: None,
            secondary: false,
            ..primary
        })
    );
    assert_eq!(journal.container_at(first), Some(&contents));
    assert_eq!(journal.object_state(port).unwrap().peer, Some(first));
    super::tests::edit(&mut app, port, 153, 0, 0.0).unwrap();
    assert_eq!(
        app.world().resource::<EditJournal>().container_at(first),
        Some(&contents)
    );
}

#[test]
fn filled_chest_halves_drop_their_own_items_in_both_modes_and_invalidate_open_views() {
    use crate::protocol::inventory::{ContainerCommand, InventoryAction, InventoryCommand};
    for mode in [GameMode::Creative, GameMode::Survival] {
        for removed_index in [0, 1] {
            let mut app = super::tests::app();
            app.insert_resource(
                terra_core::block_properties::BlockPropertiesCatalog::load(
                    &terra_core::asset_directory().join("terra/blocks"),
                )
                .unwrap(),
            );
            let positions = [[4, 1, 8], [5, 1, 8]];
            let contents = [
                Inventory {
                    slots: vec![ItemStack {
                        item: ItemKind::Block(3),
                        count: 12,
                    }],
                    ..Default::default()
                },
                Inventory {
                    slots: vec![ItemStack {
                        item: ItemKind::Stick,
                        count: 9,
                    }],
                    ..Default::default()
                },
            ];
            for (position, inventory) in positions.into_iter().zip(&contents) {
                super::tests::edit(&mut app, position, 0, 29, 0.0).unwrap();
                app.world_mut()
                    .resource_mut::<EditJournal>()
                    .set_container(position, inventory.clone())
                    .unwrap();
            }
            let removed = positions[removed_index];
            let surviving = positions[1 - removed_index];
            let mut peer = removal_peer(
                &mut app,
                removed,
                mode,
                Some(ItemKind::Tool(ToolKind::IronAxe)),
            );
            let cursor = Some(ItemStack {
                item: ItemKind::Stick,
                count: 5,
            });
            app.world_mut()
                .resource_mut::<EditJournal>()
                .gameplay_mut()
                .inventories
                .get_mut(&peer.uuid)
                .unwrap()
                .cursor = cursor;
            let mut sender = lightyear::prelude::MessageSender::<ServerControl>::default();
            let mut system = SystemState::<ControlWorld>::new(app.world_mut());
            let mut world = system.get_mut(app.world_mut()).unwrap();
            super::super::containers::access(
                InventoryCommand {
                    revision: world.journal.gameplay().inventories[&peer.uuid].revision,
                    action: InventoryAction::OpenContainer { position: removed },
                },
                &mut peer,
                &mut sender,
                &mut world,
            );
            let opened = peer.container.clone().expect("filled double chest opens");
            assert_eq!(opened.halves.len(), 2);
            let stale = ContainerCommand {
                inventory_revision: world.journal.gameplay().inventories[&peer.uuid].revision,
                anchor: opened.anchor,
                view: opened.view,
                revisions: opened
                    .halves
                    .iter()
                    .map(|half| half.inventory.revision)
                    .collect(),
                reserved: [0; terra_core::INVENTORY_SLOTS],
                action: terra_core::containers::ContainerGesture::QuickMove {
                    slot: terra_core::INVENTORY_SLOTS as u8,
                },
            };
            let command = EditCommand {
                request: 1,
                revision: world
                    .journal
                    .revision(terra_voxel::global_to_section(removed).0),
                look: peer.look,
                source: None,
                action: EditAction::Block {
                    position: removed,
                    expected: 29,
                    material: 0,
                },
            };
            assert_eq!(
                super::super::validation::apply(command, &peer, &mut world, true),
                EditOutcome::Applied
            );
            assert_eq!(world.chunks.block_at(removed), Some(0));
            assert!(world.journal.container_at(removed).is_none());
            assert_eq!(
                world.journal.container_at(surviving),
                Some(&contents[1 - removed_index])
            );
            let remaining = world.journal.object_state(surviving).unwrap();
            assert!(!remaining.secondary && remaining.peer.is_none());
            let game = world.journal.gameplay();
            assert_eq!(game.inventories[&peer.uuid].cursor, cursor);
            let stacks: Vec<_> = game.drops.iter().map(|drop| drop.stack).collect();
            let mut expected = contents[removed_index].slots.clone();
            if mode == GameMode::Survival {
                expected.push(ItemStack {
                    item: ItemKind::Block(29),
                    count: 1,
                });
            }
            assert_eq!(stacks, expected);
            assert!(
                game.drops
                    .iter()
                    .all(|drop| drop.position == removed.map(|value| value as f64 + 0.5))
            );
            super::super::containers::refresh(&mut peer, &mut sender, &world);
            assert!(peer.container.is_none());
            let before = world.journal.gameplay().clone();
            super::super::containers::transfer(stale, &mut peer, &mut sender, &mut world);
            assert_eq!(
                world.journal.gameplay(),
                &before,
                "stale transfer cannot recreate or collect destroyed inventory"
            );
            assert_eq!(
                world.journal.container_at(surviving),
                Some(&contents[1 - removed_index])
            );
        }
    }
}

#[test]
fn filled_double_chest_removal_preflights_capacity_ids_and_both_section_revisions() {
    for failure in ["capacity", "drop_ids", "gameplay_revision", "peer_revision"] {
        let mut app = super::tests::app();
        let positions = [[15, 1, 8], [16, 1, 8]];
        let contents = Inventory {
            slots: vec![ItemStack {
                item: ItemKind::Stick,
                count: 17,
            }],
            ..Default::default()
        };
        for position in positions {
            super::tests::edit(&mut app, position, 0, 29, 0.0).unwrap();
            app.world_mut()
                .resource_mut::<EditJournal>()
                .set_container(position, contents.clone())
                .unwrap();
        }
        {
            let mut journal = app.world_mut().resource_mut::<EditJournal>();
            match failure {
                "capacity" => {
                    for _ in 0..terra_core::MAX_WORLD_DROPS - 1 {
                        assert!(
                            journal
                                .gameplay_mut()
                                .drop_one([0.5, 3.5, 0.5], ItemKind::Stick)
                        );
                    }
                }
                "drop_ids" => journal.gameplay_mut().next_drop = u64::MAX - 1,
                "gameplay_revision" => journal.gameplay_mut().revision = u64::MAX - 1,
                "peer_revision" => {
                    let mut delta =
                        journal.section_delta(terra_voxel::global_to_section(positions[1]).0);
                    delta.revision = u64::MAX;
                    journal.import_delta(delta).unwrap();
                }
                _ => unreachable!(),
            }
        }
        let before = positions.map(|position| super::tests::furnace_snapshot(&app, position));
        assert!(
            super::tests::edit(&mut app, positions[0], 29, 0, 0.0).is_err(),
            "{failure}"
        );
        assert_eq!(
            positions.map(|position| super::tests::furnace_snapshot(&app, position)),
            before,
            "{failure}"
        );
        let journal = app.world().resource::<EditJournal>();
        for position in positions {
            assert_eq!(journal.container_at(position), Some(&contents));
            assert!(journal.object_state(position).unwrap().peer.is_some());
        }
    }
}

#[test]
fn creative_chest_contents_can_fill_last_drop_slot_while_survival_reserves_the_chest_item() {
    for mode in [GameMode::Creative, GameMode::Survival] {
        for free in [1, 2] {
            let mut app = super::tests::app();
            app.insert_resource(
                terra_core::block_properties::BlockPropertiesCatalog::load(
                    &terra_core::asset_directory().join("terra/blocks"),
                )
                .unwrap(),
            );
            let position = [4, 1, 8];
            super::tests::edit(&mut app, position, 0, 29, 0.0).unwrap();
            let contents = Inventory {
                slots: vec![ItemStack {
                    item: ItemKind::Stick,
                    count: 17,
                }],
                ..Default::default()
            };
            app.world_mut()
                .resource_mut::<EditJournal>()
                .set_container(position, contents.clone())
                .unwrap();
            let peer = removal_peer(
                &mut app,
                position,
                mode,
                Some(ItemKind::Tool(ToolKind::IronAxe)),
            );
            {
                let mut journal = app.world_mut().resource_mut::<EditJournal>();
                for _ in 0..terra_core::MAX_WORLD_DROPS - free {
                    assert!(
                        journal
                            .gameplay_mut()
                            .drop_one([0.5, 3.5, 0.5], ItemKind::Block(3))
                    );
                }
            }
            let before = super::tests::furnace_snapshot(&app, position);
            let mut system = SystemState::<ControlWorld>::new(app.world_mut());
            let mut world = system.get_mut(app.world_mut()).unwrap();
            let command = EditCommand {
                request: 1,
                revision: world
                    .journal
                    .revision(terra_voxel::global_to_section(position).0),
                look: peer.look,
                source: None,
                action: EditAction::Block {
                    position,
                    expected: 29,
                    material: 0,
                },
            };
            let result = super::super::validation::apply(command, &peer, &mut world, true);
            if mode == GameMode::Survival && free == 1 {
                assert_ne!(result, EditOutcome::Applied);
                assert_eq!(world.journal.gameplay(), &before.2);
                assert_eq!(world.journal.container_at(position), Some(&contents));
                assert_eq!(
                    world
                        .journal
                        .section_delta(terra_voxel::global_to_section(position).0),
                    before.1
                );
                assert_eq!(world.chunks.block_at(position), Some(29));
            } else {
                assert_eq!(result, EditOutcome::Applied);
                assert!(world.journal.container_at(position).is_none());
                let drops = &world.journal.gameplay().drops;
                let extra = 1 + usize::from(mode == GameMode::Survival);
                assert_eq!(drops.len(), terra_core::MAX_WORLD_DROPS - free + extra);
                assert_eq!(drops[drops.len() - extra].stack, contents.slots[0]);
                if mode == GameMode::Survival {
                    assert_eq!(
                        drops.last().unwrap().stack,
                        ItemStack {
                            item: ItemKind::Block(29),
                            count: 1
                        }
                    );
                }
            }
        }
    }
}

#[test]
fn realm_territory_keeps_terrain_open_but_container_removal_needs_inventory_access() {
    use crate::protocol::inventory::InventoryAction;
    use terra_core::kingdom::{ClaimActor, ClaimKind, KingdomRole};
    for material in [29, 59, 99] {
        let mut app = super::tests::app();
        app.insert_resource(
            terra_core::block_properties::BlockPropertiesCatalog::load(
                &terra_core::asset_directory().join("terra/blocks"),
            )
            .unwrap(),
        );
        let anchor = [4, 1, 8];
        super::tests::edit(&mut app, anchor, 0, material, 0.0).unwrap();
        if material == 29 {
            super::tests::edit(&mut app, [anchor[0] + 1, anchor[1], anchor[2]], 0, 29, 0.0)
                .unwrap();
        }
        let contents = Inventory {
            slots: vec![ItemStack {
                item: match material {
                    59 => ItemKind::Mineral(MineralItem::RawIron),
                    99 => terra_core::crusher::recipes()[0].input,
                    _ => ItemKind::Block(3),
                },
                count: 3,
            }],
            ..Default::default()
        };
        app.world_mut()
            .resource_mut::<EditJournal>()
            .set_container(anchor, contents.clone())
            .unwrap();
        let mut peer = removal_peer(&mut app, anchor, GameMode::Creative, None);
        let owner = terra_core::PlayerUuid::from_u128(199);
        {
            let mut journal = app.world_mut().resource_mut::<EditJournal>();
            let r = &mut journal.gameplay_mut().kingdoms;
            let realm = r.found(owner, [0, 10, -16], "Territory storage").unwrap();
            r.claim(
                realm,
                [0, 0],
                ClaimKind::RealmTerritory,
                ClaimActor::Player(owner),
            )
            .unwrap();
            r.join(peer.uuid, realm).unwrap();
        }
        let mut system = SystemState::<ControlWorld>::new(app.world_mut());
        let mut world = system.get_mut(app.world_mut()).unwrap();
        assert!(
            super::super::kingdoms::edit_allowed(
                EditAction::Block {
                    position: [9, 0, 9],
                    expected: 3,
                    material: 0
                },
                &peer,
                &world
            ),
            "ordinary terrain remains open in Realm Territory"
        );
        for action in [
            InventoryAction::DropSlot { slot: 0, all: true },
            InventoryAction::DropCursor { all: true },
        ] {
            assert!(
                super::super::kingdoms::inventory_allowed(action, &peer, &world),
                "personal bag drops remain usable"
            );
        }
        for target in std::iter::once(anchor).chain(
            world
                .journal
                .object_state(anchor)
                .and_then(|state| state.peer),
        ) {
            assert!(
                !super::super::kingdoms::edit_allowed(
                    EditAction::Block {
                        position: target,
                        expected: material,
                        material: 0
                    },
                    &peer,
                    &world
                ),
                "both machine halves require inventory access"
            );
        }
        let command = EditCommand {
            request: 1,
            revision: world
                .journal
                .revision(terra_voxel::global_to_section(anchor).0),
            look: peer.look,
            source: None,
            action: EditAction::Block {
                position: anchor,
                expected: material,
                material: 0,
            },
        };
        let before = world.journal.gameplay().drops.clone();
        assert_eq!(
            super::super::validation::apply(command, &peer, &mut world, true),
            EditOutcome::Invalid
        );
        assert_eq!(world.chunks.block_at(anchor), Some(material));
        assert_eq!(world.journal.container_at(anchor), Some(&contents));
        assert_eq!(world.journal.gameplay().drops, before);
        let mut sender = lightyear::prelude::MessageSender::<ServerControl>::default();
        {
            let bag = world
                .journal
                .gameplay_mut()
                .inventories
                .get_mut(&peer.uuid)
                .unwrap();
            bag.insert(ItemKind::Stick, 5);
            bag.cursor = Some(ItemStack {
                item: ItemKind::Mineral(MineralItem::Coal),
                count: 2,
            });
        }
        for action in [
            InventoryAction::DropSlot { slot: 0, all: true },
            InventoryAction::DropCursor { all: true },
        ] {
            let revision = world.journal.gameplay().inventories[&peer.uuid].revision;
            let drops = world.journal.gameplay().drops.len();
            super::super::gameplay::inventory(
                crate::protocol::inventory::InventoryCommand { revision, action },
                &mut peer,
                &mut sender,
                &mut world,
            );
            assert_eq!(
                world.journal.gameplay().drops.len(),
                drops + 1,
                "Visitor can actually drop owned slot/cursor contents"
            );
        }
        assert_eq!(
            world.journal.gameplay().inventories[&peer.uuid].count(ItemKind::Stick),
            0
        );
        assert!(
            world.journal.gameplay().inventories[&peer.uuid]
                .cursor
                .is_none()
        );
        assert_eq!(world.journal.container_at(anchor), Some(&contents));
        let realm = world
            .journal
            .gameplay()
            .kingdoms
            .active_for_member(peer.uuid)
            .unwrap()
            .id;
        world
            .journal
            .gameplay_mut()
            .kingdoms
            .set_role(owner, realm, peer.uuid, KingdomRole::Member)
            .unwrap();
        assert_eq!(
            super::super::validation::apply(command, &peer, &mut world, true),
            EditOutcome::Applied
        );
        assert!(world.journal.container_at(anchor).is_none());
        assert!(
            world
                .journal
                .gameplay()
                .drops
                .iter()
                .any(|drop| drop.stack == contents.slots[0])
        );
    }
}
