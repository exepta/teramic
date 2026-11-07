use super::*;

#[path = "server_conveyor_ownership_tests.rs"]
mod ownership_tests;

#[test]
fn visible_crusher_rims_can_open_beside_the_real_crank_but_not_through_walls_or_unloaded_sections()
{
    use crate::protocol::inventory::{InventoryAction, InventoryCommand};
    for facing in 0..4 {
        for obstruction in ["wall", "unloaded"] {
            let (mut app, mut peer, _) = fixture(ResourcePropKind::SmallStones, false);
            let anchor = [7, 6, 7];
            let axis = terra_core::kinetic::axis(facing);
            let crank = std::array::from_fn(|i| anchor[i] + axis[i]);
            for (position, material, state) in [
                (
                    anchor,
                    99,
                    terra_world::edits::BlockObjectState {
                        facing,
                        ..Default::default()
                    },
                ),
                (
                    crank,
                    100,
                    terra_world::edits::BlockObjectState {
                        facing,
                        peer: Some(anchor),
                        ..Default::default()
                    },
                ),
            ] {
                let (owner, local) = terra_voxel::global_to_section(position);
                app.world_mut()
                    .resource_mut::<ChunkStore>()
                    .loaded
                    .get_mut(&owner)
                    .unwrap()
                    .data
                    .set_block(local, material)
                    .unwrap();
                let mut journal = app.world_mut().resource_mut::<EditJournal>();
                journal.record(position, material).unwrap();
                journal.set_object_state(position, Some(state)).unwrap();
            }
            app.world_mut()
                .resource_mut::<EditJournal>()
                .set_crusher(anchor, Default::default())
                .unwrap();
            let rotation = Quat::from_rotation_y(f32::from(facing) * std::f32::consts::FRAC_PI_2);
            let body = Vec3::from_array(anchor.map(|v| v as f32))
                + Vec3::new(0.5, 0.0, 0.5)
                + rotation * Vec3::new(2.7, terra_physics::character::PLAYER_HALF_HEIGHT, 0.0);
            app.world_mut()
                .get_mut::<Transform>(peer.body)
                .unwrap()
                .translation = body;
            let mut sender = lightyear::prelude::MessageSender::<ServerControl>::default();
            let mut system = SystemState::<ControlWorld>::new(app.world_mut());
            let mut world = system.get_mut(app.world_mut()).unwrap();
            let eye = body + Vec3::Y * terra_physics::character::PLAYER_EYE_OFFSET;
            let delta = Vec3::from_array(anchor.map(|v| v as f32 + 0.5)) - eye;
            let hit = terra_world::interaction_shapes::raycast(
                &world.chunks,
                &world.journal,
                &world.block_properties,
                eye.to_array().map(f64::from),
                delta.normalize().to_array().map(f64::from),
                f64::from(delta.length()) + 0.1,
            )
            .unwrap();
            assert_eq!(
                hit.position, crank,
                "the exact native operator's center ray is genuinely blocked by100"
            );
            let command = InventoryCommand {
                revision: world.journal.gameplay().inventories[&peer.uuid].revision,
                action: InventoryAction::OpenContainer { position: anchor },
            };
            crate::server::containers::access(command, &mut peer, &mut sender, &mut world);
            assert!(peer.container.as_ref().is_some_and(|s| s.crusher.is_some()));
            if obstruction == "wall" {
                for dy in [0, 1] {
                    let position = [
                        anchor[0] + axis[0] * 2,
                        anchor[1] + dy,
                        anchor[2] + axis[2] * 2,
                    ];
                    let (owner, local) = terra_voxel::global_to_section(position);
                    world
                        .chunks
                        .loaded
                        .get_mut(&owner)
                        .unwrap()
                        .data
                        .set_block(local, 3)
                        .unwrap();
                    world.journal.record(position, 3).unwrap();
                }
            } else {
                world.chunks.loaded.clear();
            }
            crate::server::containers::refresh(&mut peer, &mut sender, &world);
            assert!(
                peer.container.is_none(),
                "real {obstruction} still blocks all authorized Crusher views"
            );
            crate::server::containers::access(command, &mut peer, &mut sender, &mut world);
            assert!(peer.container.is_none());
        }
    }
}

#[test]
fn one_cell_crusher_inventory_opens_without_a_peer_but_a_legacy_pair_requires_both_sections() {
    use crate::protocol::inventory::{InventoryAction, InventoryCommand};
    for legacy in [false, true] {
        let (mut app, mut peer, _) = fixture(ResourcePropKind::SmallStones, false);
        let anchor = [15, 6, 7];
        let other = [16, 6, 7];
        let neighbor = terra_voxel::global_to_section(other).0;
        {
            let mut chunks = app.world_mut().resource_mut::<ChunkStore>();
            chunks
                .loaded
                .entry(neighbor)
                .or_insert_with(|| LoadedSection {
                    data: terra_voxel::BlockSection::empty(neighbor).unwrap(),
                    mesh: None,
                    mesh_epoch: 0,
                    water_epoch: 0,
                    water: None,
                    props: Vec::new(),
                });
        }
        for (position, back, secondary) in
            std::iter::once((anchor, other, false)).chain(legacy.then_some((other, anchor, true)))
        {
            let (owner, local) = terra_voxel::global_to_section(position);
            app.world_mut()
                .resource_mut::<ChunkStore>()
                .loaded
                .get_mut(&owner)
                .unwrap()
                .data
                .set_block(local, 99)
                .unwrap();
            let mut journal = app.world_mut().resource_mut::<EditJournal>();
            journal.record(position, 99).unwrap();
            journal
                .set_object_state(
                    position,
                    Some(terra_world::edits::BlockObjectState {
                        peer: legacy.then_some(back),
                        secondary,
                        ..Default::default()
                    }),
                )
                .unwrap();
        }
        {
            let mut journal = app.world_mut().resource_mut::<EditJournal>();
            journal.set_crusher(anchor, Default::default()).unwrap();
            journal
                .set_container(
                    anchor,
                    Inventory {
                        slots: vec![ItemStack {
                            item: terra_core::crusher::recipes()[0].input,
                            count: 3,
                        }],
                        ..Default::default()
                    },
                )
                .unwrap();
        }
        app.world_mut()
            .get_mut::<Transform>(peer.body)
            .unwrap()
            .translation =
            Vec3::new(15.5, 6.5, 10.5) - Vec3::Y * terra_physics::character::PLAYER_EYE_OFFSET;
        let mut sender = lightyear::prelude::MessageSender::<ServerControl>::default();
        let mut system = SystemState::<ControlWorld>::new(app.world_mut());
        let mut world = system.get_mut(app.world_mut()).unwrap();
        let command = InventoryCommand {
            revision: world.journal.gameplay().inventories[&peer.uuid].revision,
            action: InventoryAction::OpenContainer { position: anchor },
        };
        crate::server::containers::access(command, &mut peer, &mut sender, &mut world);
        assert!(
            peer.container
                .as_ref()
                .is_some_and(|s| s.crusher.is_some() && s.anchor == anchor && s.halves.len() == 1)
        );
        assert_eq!(
            peer.container.as_ref().unwrap().halves[0]
                .inventory
                .slot(0)
                .unwrap()
                .count,
            3
        );
        let unloaded = world.chunks.loaded.remove(&neighbor).unwrap();
        crate::server::containers::refresh(&mut peer, &mut sender, &world);
        assert_eq!(peer.container.is_some(), !legacy);
        assert_eq!(
            world
                .journal
                .container_at(anchor)
                .unwrap()
                .slot(0)
                .unwrap()
                .count,
            3
        );
        world.chunks.loaded.insert(neighbor, unloaded);
        crate::server::containers::access(command, &mut peer, &mut sender, &mut world);
        assert!(peer.container.as_ref().is_some_and(|s| s.crusher.is_some()));
        assert!(world.journal.gameplay().valid());
    }
}

#[test]
fn authoritative_three_shaft_belt_replay_and_removal_preserve_every_other_shaft() {
    for remove_guide in [false, true] {
        let (mut app, mut peer, _) = fixture(ResourcePropKind::SmallStones, false);
        let start = [7, 6, 5];
        let end = [7, 6, 9];
        let guide = [7, 6, 7];
        for position in [start, end, guide] {
            app.world_mut()
                .resource_mut::<ChunkStore>()
                .loaded
                .get_mut(&terra_voxel::SectionPos::default())
                .unwrap()
                .data
                .set_block(position.map(|v| v as usize), 144)
                .unwrap();
            let mut journal = app.world_mut().resource_mut::<EditJournal>();
            journal.record(position, 144).unwrap();
            journal
                .set_object_state(
                    position,
                    Some(terra_world::edits::BlockObjectState {
                        wood: terra_core::WoodSpecies::Spruce,
                        ..Default::default()
                    }),
                )
                .unwrap();
        }
        let item = ItemKind::for_wood(148, terra_core::WoodSpecies::Birch);
        app.world_mut()
            .resource_mut::<EditJournal>()
            .gameplay_mut()
            .inventories
            .get_mut(&peer.uuid)
            .unwrap()
            .creative_set(0, ItemStack { item, count: 1 });
        let eye = Vec3::new(7.5, 6.5, 12.5);
        app.world_mut()
            .get_mut::<Transform>(peer.body)
            .unwrap()
            .translation = eye - Vec3::Y * PLAYER_EYE_OFFSET;
        peer.look = [0.0; 2];
        let journal = app.world().resource::<EditJournal>();
        let command = EditCommand {
            request: 1,
            revision: journal.revision(terra_voxel::SectionPos::default()),
            look: peer.look,
            source: Some(InventorySource {
                slot: 0,
                revision: journal.gameplay().inventories[&peer.uuid].revision,
            }),
            action: EditAction::ConnectConveyor {
                start,
                end,
                start_revision: journal.revision(terra_voxel::SectionPos::default()),
                item,
            },
        };
        let controller;
        {
            let mut system = SystemState::<ControlWorld>::new(app.world_mut());
            let mut world = system.get_mut(app.world_mut()).unwrap();
            assert_eq!(
                apply(command, &peer, &mut world, false),
                EditOutcome::Applied
            );
            assert_eq!(
                world.journal.gameplay().inventories[&peer.uuid].count(item),
                0
            );
            let span = terra_world::conveyors::span_at(&world.journal, guide).unwrap();
            controller = span.controller;
            assert_eq!(world.chunks.block_at(guide), Some(144));
            assert_eq!(
                world.journal.object_state(guide).unwrap().wood,
                terra_core::WoodSpecies::Spruce
            );
            assert_eq!(
                apply(command, &peer, &mut world, false),
                EditOutcome::Conflict
            );
        }
        app.world_mut()
            .resource_mut::<EditJournal>()
            .gameplay_mut()
            .inventories
            .get_mut(&peer.uuid)
            .unwrap()
            .creative_set(
                0,
                ItemStack {
                    item: ItemKind::Tool(terra_core::ToolKind::IronPickaxe),
                    count: 1,
                },
            );
        let selected = if remove_guide { guide } else { controller };
        let eye = if remove_guide {
            Vec3::new(10.5, 6.5, 7.5)
        } else {
            Vec3::new(7.5, 8.5, 6.5)
        };
        peer.look = if remove_guide {
            [std::f32::consts::FRAC_PI_2, 0.0]
        } else {
            [0.0, -1.4]
        };
        app.world_mut()
            .get_mut::<Transform>(peer.body)
            .unwrap()
            .translation = eye - Vec3::Y * PLAYER_EYE_OFFSET;
        let command = EditCommand {
            request: 2,
            revision: app
                .world()
                .resource::<EditJournal>()
                .revision(terra_voxel::SectionPos::default()),
            look: peer.look,
            source: None,
            action: EditAction::Block {
                position: selected,
                expected: if remove_guide { 144 } else { 148 },
                material: 0,
            },
        };
        let mut system = SystemState::<ControlWorld>::new(app.world_mut());
        let mut world = system.get_mut(app.world_mut()).unwrap();
        assert_eq!(
            apply(command, &peer, &mut world, true),
            EditOutcome::Applied
        );
        assert_eq!(
            world
                .journal
                .gameplay()
                .drops
                .iter()
                .filter(|drop| drop.stack.item == item)
                .map(|drop| drop.stack.count)
                .sum::<u16>(),
            1
        );
        for position in [start, end, guide] {
            if remove_guide && position == guide {
                assert_eq!(world.chunks.block_at(position), Some(0));
                continue;
            }
            assert_eq!(world.chunks.block_at(position), Some(144));
            assert_eq!(world.journal.object_state(position).unwrap().peer, None);
            assert_eq!(
                world.journal.object_state(position).unwrap().wood,
                terra_core::WoodSpecies::Spruce
            );
        }
        assert!(terra_world::conveyors::span_at(&world.journal, controller).is_none());
        let count = world.journal.gameplay().drops.len();
        assert_ne!(
            apply(command, &peer, &mut world, true),
            EditOutcome::Applied
        );
        assert_eq!(world.journal.gameplay().drops.len(), count);
    }
}

#[test]
fn authoritative_ports_place_on_an_ordinary_clicked_floor_without_an_inventory_neighbor() {
    for material in [152, 153] {
        let (mut app, mut peer, _) = fixture(ResourcePropKind::SmallStones, false);
        peer.mode = Some(GameMode::Creative);
        let eye = Vec3::new(7.5, 7.5, 8.0);
        app.world_mut()
            .get_mut::<Transform>(peer.body)
            .unwrap()
            .translation = eye - Vec3::Y * PLAYER_EYE_OFFSET;
        peer.look = [0.0, -1.4];
        let position = [7, 4, 7];
        let command = EditCommand {
            request: 1,
            revision: app
                .world()
                .resource::<EditJournal>()
                .revision(terra_voxel::SectionPos::default()),
            look: peer.look,
            source: None,
            action: EditAction::Block {
                position,
                expected: 0,
                material,
            },
        };
        let mut system = SystemState::<ControlWorld>::new(app.world_mut());
        let mut world = system.get_mut(app.world_mut()).unwrap();
        assert_eq!(
            apply(command, &peer, &mut world, false),
            EditOutcome::Applied
        );
        assert_eq!(world.chunks.block_at(position), Some(material));
        assert_eq!(world.journal.object_state(position).unwrap().peer, None);
    }
}

fn belt_fixture() -> (App, Peer, [i64; 3]) {
    let (mut app, peer, _) = fixture(ResourcePropKind::SmallStones, false);
    let position = [7, 6, 7];
    app.world_mut()
        .resource_mut::<ChunkStore>()
        .loaded
        .get_mut(&terra_voxel::SectionPos::default())
        .unwrap()
        .data
        .set_block([7, 6, 7], 148)
        .unwrap();
    {
        let mut journal = app.world_mut().resource_mut::<EditJournal>();
        journal.record(position, 148).unwrap();
        journal
            .set_object_state(position, Some(Default::default()))
            .unwrap();
    }
    (app, peer, position)
}

#[test]
fn explicit_belt_collection_requires_the_reachable_visible_carried_stack_and_preserves_counts() {
    for (material, height) in [(148, 6.755), (99, 7.13)] {
        let (mut app, mut peer, _) = belt_fixture();
        if material == 99 {
            app.world_mut()
                .resource_mut::<ChunkStore>()
                .loaded
                .get_mut(&terra_voxel::SectionPos::default())
                .unwrap()
                .data
                .set_block([7, 6, 7], 99)
                .unwrap();
            let mut journal = app.world_mut().resource_mut::<EditJournal>();
            journal.record([7, 6, 7], 99).unwrap();
            journal
                .set_object_state([7, 6, 7], Some(Default::default()))
                .unwrap();
        }
        let position = [7.5, height, 7.5];
        let id;
        {
            let mut journal = app.world_mut().resource_mut::<EditJournal>();
            assert!(journal.gameplay_mut().drop_stack(
                position,
                ItemStack {
                    item: ItemKind::Block(3),
                    count: 19
                },
                [0.0; 3],
                0.0
            ));
            id = journal.gameplay().drops[0].id;
        }
        let eye = Vec3::new(7.5, height as f32, 10.5);
        app.world_mut()
            .get_mut::<Transform>(peer.body)
            .unwrap()
            .translation = eye - Vec3::Y * PLAYER_EYE_OFFSET;
        peer.look = [0.0; 2];
        let command = EditCommand {
            request: 1,
            revision: 0,
            look: peer.look,
            action: EditAction::CollectConveyor { id },
            source: None,
        };
        {
            let mut system = SystemState::<ControlWorld>::new(app.world_mut());
            let mut world = system.get_mut(app.world_mut()).unwrap();
            assert_eq!(
                check(
                    EditCommand {
                        look: [f32::NAN, 0.0],
                        ..command
                    },
                    &peer,
                    &world
                ),
                EditOutcome::Invalid
            );
            assert_eq!(
                check(
                    EditCommand {
                        action: EditAction::CollectConveyor { id: id + 1 },
                        ..command
                    },
                    &peer,
                    &world
                ),
                EditOutcome::Occluded
            );
            assert_eq!(
                check(
                    EditCommand {
                        look: [1.0, 0.0],
                        ..command
                    },
                    &peer,
                    &world
                ),
                EditOutcome::Occluded
            );
            assert_eq!(check(command, &peer, &world), EditOutcome::Applied);
            assert_eq!(
                apply(command, &peer, &mut world, false),
                EditOutcome::Applied
            );
            assert!(world.journal.gameplay().drops.is_empty());
            assert_eq!(
                world.journal.gameplay().inventories[&peer.uuid].count(ItemKind::Block(3)),
                19
            );
            assert_ne!(
                apply(command, &peer, &mut world, false),
                EditOutcome::Applied
            );
            assert_eq!(
                world.journal.gameplay().inventories[&peer.uuid].count(ItemKind::Block(3)),
                19
            );
        }
    }
}

#[test]
fn belt_collection_rejects_walls_distance_unloaded_support_and_full_inventory() {
    for reason in ["wall", "far", "unloaded", "full", "off_belt"] {
        let (mut app, mut peer, _) = belt_fixture();
        let mut position = [7.5, 6.755, 7.5];
        if reason == "off_belt" {
            position[0] = 8.5;
        }
        let id;
        {
            let mut journal = app.world_mut().resource_mut::<EditJournal>();
            journal.gameplay_mut().drop_stack(
                position,
                ItemStack {
                    item: ItemKind::Block(3),
                    count: 19,
                },
                [0.0; 3],
                0.0,
            );
            id = journal.gameplay().drops[0].id;
            if reason == "full" {
                let bag = journal
                    .gameplay_mut()
                    .inventories
                    .get_mut(&peer.uuid)
                    .unwrap();
                for slot in 0..terra_core::INVENTORY_SLOTS {
                    bag.creative_set(
                        slot,
                        ItemStack {
                            item: ItemKind::Block(4),
                            count: terra_core::STACK_LIMIT,
                        },
                    );
                }
            }
        }
        let eye = Vec3::new(
            position[0] as f32,
            position[1] as f32,
            if reason == "far" { 15.5 } else { 10.5 },
        );
        app.world_mut()
            .get_mut::<Transform>(peer.body)
            .unwrap()
            .translation = eye - Vec3::Y * PLAYER_EYE_OFFSET;
        peer.look = [0.0; 2];
        if reason == "wall" {
            app.world_mut()
                .resource_mut::<ChunkStore>()
                .loaded
                .get_mut(&terra_voxel::SectionPos::default())
                .unwrap()
                .data
                .set_block([7, 6, 9], 3)
                .unwrap();
        }
        if reason == "unloaded" {
            app.world_mut().resource_mut::<ChunkStore>().loaded.clear();
        }
        let command = EditCommand {
            request: 1,
            revision: 0,
            look: peer.look,
            action: EditAction::CollectConveyor { id },
            source: None,
        };
        let mut system = SystemState::<ControlWorld>::new(app.world_mut());
        let mut world = system.get_mut(app.world_mut()).unwrap();
        assert_ne!(
            apply(command, &peer, &mut world, false),
            EditOutcome::Applied,
            "{reason}"
        );
        assert_eq!(
            world.journal.gameplay().drops[0].stack.count,
            19,
            "{reason}"
        );
        assert_eq!(
            world.journal.gameplay().inventories[&peer.uuid].count(ItemKind::Block(3)),
            0,
            "{reason}"
        );
    }
}

#[test]
fn dyeing_a_belt_uses_one_held_dye_and_rejects_replay_and_missing_source() {
    let (mut app, mut peer, position) = belt_fixture();
    let color = terra_core::beds::DyeColor::LightBlue;
    let item = ItemKind::Dye(color);
    {
        let mut journal = app.world_mut().resource_mut::<EditJournal>();
        journal
            .gameplay_mut()
            .inventories
            .get_mut(&peer.uuid)
            .unwrap()
            .creative_set(0, ItemStack { item, count: 2 });
    }
    let eye = Vec3::new(7.5, 8.5, 7.5);
    app.world_mut()
        .get_mut::<Transform>(peer.body)
        .unwrap()
        .translation = eye - Vec3::Y * PLAYER_EYE_OFFSET;
    peer.look = [0.0, -1.4];
    let journal = app.world().resource::<EditJournal>();
    let command = EditCommand {
        request: 1,
        revision: journal.revision(terra_voxel::SectionPos::default()),
        look: peer.look,
        action: EditAction::DyeConveyor { position, color },
        source: Some(InventorySource {
            slot: 0,
            revision: journal.gameplay().inventories[&peer.uuid].revision,
        }),
    };
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
        terra_core::kinetic::belt_color(world.journal.object_state(position).unwrap().facing),
        Some(color)
    );
    assert_eq!(
        terra_core::kinetic::belt_facing(world.journal.object_state(position).unwrap().facing),
        0
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
}

#[test]
fn owned_belt_dye_is_authoritative_from_controller_end_drum_body_and_guide() {
    use terra_core::{WoodSpecies, beds::DyeColor, kinetic};
    use terra_world::{conveyors, edits::BlockObjectState};
    let (mut app, mut peer, _) = fixture(ResourcePropKind::SmallStones, false);
    let start = [7, 6, 5];
    let guide = [7, 6, 6];
    let end = [7, 6, 10];
    let span =
        app.world_mut()
            .resource_scope(|world, mut journal: bevy::prelude::Mut<EditJournal>| {
                let mut chunks = world.resource_mut::<ChunkStore>();
                for p in [start, guide, end] {
                    let (section, local) = global_to_section(p);
                    chunks
                        .loaded
                        .get_mut(&section)
                        .unwrap()
                        .data
                        .set_block(local, 144)
                        .unwrap();
                    journal.record(p, 144).unwrap();
                    journal
                        .set_object_state(
                            p,
                            Some(BlockObjectState {
                                wood: WoodSpecies::Spruce,
                                ..Default::default()
                            }),
                        )
                        .unwrap();
                }
                let layout = conveyors::layout(
                    &chunks,
                    &journal,
                    start,
                    end,
                    ItemKind::for_wood(148, WoodSpecies::Birch),
                )
                .unwrap();
                assert!(conveyors::commit(&mut chunks, &mut journal, &layout).unwrap());
                conveyors::span_at(&journal, guide).unwrap()
            });
    assert_ne!(span.first, span.controller);
    for (position, color) in [
        (span.controller, DyeColor::Red),
        (end, DyeColor::LightBlue),
        ([7, 6, 8], DyeColor::Purple),
        (guide, DyeColor::Lime),
    ] {
        let item = ItemKind::Dye(color);
        app.world_mut()
            .resource_mut::<EditJournal>()
            .gameplay_mut()
            .inventories
            .get_mut(&peer.uuid)
            .unwrap()
            .creative_set(0, ItemStack { item, count: 2 });
        let pitch = -1.4_f32;
        let eye = Vec3::new(
            position[0] as f32 + 0.5,
            position[1] as f32 + 2.5,
            position[2] as f32 + 0.5 + 2.0 / (-pitch).tan(),
        );
        app.world_mut()
            .get_mut::<Transform>(peer.body)
            .unwrap()
            .translation = eye - Vec3::Y * PLAYER_EYE_OFFSET;
        peer.look = [0.0, pitch];
        let journal = app.world().resource::<EditJournal>();
        let shaft_states = [start, guide, end].map(|p| journal.object_state(p).unwrap());
        let command = EditCommand {
            request: 1,
            revision: journal.revision(global_to_section(position).0),
            look: peer.look,
            action: EditAction::DyeConveyor { position, color },
            source: Some(InventorySource {
                slot: 0,
                revision: journal.gameplay().inventories[&peer.uuid].revision,
            }),
        };
        let mut system = SystemState::<ControlWorld>::new(app.world_mut());
        let mut world = system.get_mut(app.world_mut()).unwrap();
        assert_eq!(
            apply(command, &peer, &mut world, false),
            EditOutcome::Applied,
            "{position:?}"
        );
        assert_eq!(
            world.journal.gameplay().inventories[&peer.uuid].count(item),
            1
        );
        for p in span
            .cells()
            .filter(|p| world.journal.block(*p) == Some(148))
        {
            let state = world.journal.object_state(p).unwrap();
            assert_eq!(kinetic::belt_color(state.facing), Some(color));
            assert_eq!(state.wood, WoodSpecies::Birch);
            assert_eq!(conveyors::span_at(&world.journal, p), Some(span));
        }
        assert_eq!(
            [start, guide, end].map(|p| world.journal.object_state(p).unwrap()),
            shaft_states
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
