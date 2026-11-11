use super::*;
use terra_core::{PlayerUuid, trains::CouplerEnd};

fn builder_fixture() -> (App, Peer, u64, u16, terra_world::rails::NetworkCache) {
    let (app, mut peer, id, cargo, graph) = super::builder_fixture();
    peer.mode = Some(GameMode::Survival);
    (app, peer, id, cargo, graph)
}

fn freight(app: &mut App, locomotive: u64, couple: bool) -> u64 {
    let mut journal = app.world_mut().resource_mut::<EditJournal>();
    let registry = &mut journal.gameplay_mut().trains;
    let mut wagon = registry.vehicles[&locomotive].clone();
    wagon.kind = VehicleKind::Freight;
    wagon.anchor[2] -= 9;
    wagon.motion = Default::default();
    wagon
        .parts
        .retain(|part| part.material != train_parts::RAIL_BUILDER);
    for part in &mut wagon.parts {
        if part.material == train_parts::CAB {
            part.material = train_parts::FRAME;
        }
    }
    for bag in wagon.cargo.values_mut() {
        *bag = Inventory {
            slots: vec![ItemStack::EMPTY; terra_core::containers::CHEST_SLOTS],
            ..Default::default()
        };
    }
    let owner = wagon.owner;
    let id = registry.insert(wagon).unwrap();
    if couple {
        registry
            .couple(owner, locomotive, CouplerEnd::Rear, id, CouplerEnd::Front)
            .unwrap();
    }
    id
}

fn autoload(app: &mut App, id: u64, active: bool) {
    app.world_mut()
        .resource_mut::<EditJournal>()
        .gameplay_mut()
        .trains
        .vehicles
        .get_mut(&id)
        .unwrap()
        .motion
        .rail_builder_autoload = active;
}

fn obstacle(app: &mut App, position: [i64; 3], material: u16) {
    let (owner, local) = terra_voxel::global_to_section(position);
    app.world_mut()
        .resource_mut::<ChunkStore>()
        .loaded
        .get_mut(&owner)
        .unwrap()
        .data
        .set_block(local, material)
        .unwrap();
}

fn clear(app: &mut App, peer: &Peer, id: u64) -> Result<(), TrainError> {
    let car = app
        .world()
        .resource::<EditJournal>()
        .gameplay()
        .trains
        .vehicles[&id]
        .clone();
    let mut system = SystemState::<ControlWorld>::new(app.world_mut());
    let mut world = system.get_mut(app.world_mut()).unwrap();
    crate::server::train_builder::clear_front(&car, peer, &mut world)
}

fn stored(app: &App, id: u64, item: ItemKind) -> u16 {
    app.world()
        .resource::<EditJournal>()
        .gameplay()
        .trains
        .vehicles[&id]
        .cargo
        .values()
        .map(|bag| bag.count(item))
        .sum()
}

#[test]
fn autoload_toggle_requires_the_parked_owner_driver_and_is_replicated_without_inventory_changes() {
    let (mut app, mut peer, id, _, _) = builder_fixture();
    app.world_mut()
        .get_mut::<Transform>(peer.body)
        .unwrap()
        .translation = Vec3::new(6.5, 7.0, 6.5);
    let car = app
        .world()
        .resource::<EditJournal>()
        .gameplay()
        .trains
        .vehicles[&id]
        .clone();
    let seat = car
        .parts
        .iter()
        .position(|part| part.material == train_parts::CAB)
        .unwrap() as u16;
    let mut seats = crate::server::trains::SeatedPlayers::default();
    let command = TrainAction::RailBuilderAutoload {
        vehicle: id,
        active: true,
    };
    assert_eq!(
        run(&mut app, &peer, command.clone(), &mut seats),
        Err(TrainError::OutOfReach)
    );
    run(
        &mut app,
        &peer,
        TrainAction::Seat {
            vehicle: id,
            part: seat,
        },
        &mut seats,
    )
    .unwrap();
    let owner = peer.uuid;
    peer.uuid = PlayerUuid::from_u128(9999);
    assert_eq!(
        run(&mut app, &peer, command.clone(), &mut seats),
        Err(TrainError::NotOwner)
    );
    peer.uuid = owner;
    run(&mut app, &peer, command, &mut seats).unwrap();
    let journal = app.world().resource::<EditJournal>();
    let selected = &journal.gameplay().trains.vehicles[&id];
    assert!(selected.motion.rail_builder_autoload);
    assert_eq!(selected.cargo, car.cargo);
    assert!(
        journal
            .gameplay()
            .trains
            .snapshot(vec![])
            .vehicles
            .iter()
            .find(|v| v.id == id)
            .unwrap()
            .motion
            .rail_builder_autoload
    );
    app.world_mut()
        .resource_mut::<EditJournal>()
        .gameplay_mut()
        .trains
        .vehicles
        .get_mut(&id)
        .unwrap()
        .motion
        .speed_mps = 1.0;
    assert_eq!(
        run(
            &mut app,
            &peer,
            TrainAction::RailBuilderAutoload {
                vehicle: id,
                active: false
            },
            &mut seats
        ),
        Err(TrainError::Busy)
    );
    assert!(
        app.world()
            .resource::<EditJournal>()
            .gameplay()
            .trains
            .vehicles[&id]
            .motion
            .rail_builder_autoload
    );
}

#[test]
fn excavation_loads_only_the_coupled_freight_preserves_item_identity_and_never_duplicates() {
    let (mut app, peer, id, _, _) = builder_fixture();
    let wagon = freight(&mut app, id, true);
    autoload(&mut app, id, true);
    let stone = [4, 8, 12];
    let plank = [5, 8, 12];
    obstacle(&mut app, stone, 3);
    obstacle(&mut app, plank, 28);
    app.world_mut()
        .resource_mut::<EditJournal>()
        .record(plank, 28)
        .unwrap();
    app.world_mut()
        .resource_mut::<EditJournal>()
        .set_object_state(
            plank,
            Some(terra_world::edits::BlockObjectState {
                wood: WoodSpecies::Cherry,
                ..Default::default()
            }),
        )
        .unwrap();
    let loco = app
        .world()
        .resource::<EditJournal>()
        .gameplay()
        .trains
        .vehicles[&id]
        .cargo
        .clone();
    clear(&mut app, &peer, id).unwrap();
    assert_eq!(stored(&app, wagon, ItemKind::Block(3)), 1);
    assert_eq!(
        stored(&app, wagon, ItemKind::for_wood(28, WoodSpecies::Cherry)),
        1
    );
    assert_eq!(
        app.world().resource::<ChunkStore>().block_at(stone),
        Some(0)
    );
    assert_eq!(
        app.world()
            .resource::<EditJournal>()
            .gameplay()
            .trains
            .vehicles[&id]
            .cargo,
        loco
    );
    assert!(
        app.world()
            .resource::<EditJournal>()
            .gameplay()
            .drops
            .is_empty()
    );
    clear(&mut app, &peer, id).unwrap();
    assert_eq!(stored(&app, wagon, ItemKind::Block(3)), 1);
    assert!(
        app.world()
            .resource::<EditJournal>()
            .gameplay()
            .trains
            .vehicles[&wagon]
            .cargo
            .values()
            .all(terra_core::containers::valid_chest)
    );
}

#[test]
fn disabled_uncoupled_or_full_freight_leaves_real_overflow_drops_instead_of_destroying_them() {
    for (active, coupled, full) in [
        (false, true, false),
        (true, false, false),
        (true, true, true),
    ] {
        let (mut app, peer, id, _, _) = builder_fixture();
        let wagon = freight(&mut app, id, coupled);
        autoload(&mut app, id, active);
        if full {
            for bag in app
                .world_mut()
                .resource_mut::<EditJournal>()
                .gameplay_mut()
                .trains
                .vehicles
                .get_mut(&wagon)
                .unwrap()
                .cargo
                .values_mut()
            {
                bag.slots.fill(ItemStack {
                    item: ItemKind::Block(4),
                    count: ItemKind::Block(4).max_stack(),
                });
            }
        }
        obstacle(&mut app, [4, 8, 12], 3);
        clear(&mut app, &peer, id).unwrap();
        assert_eq!(stored(&app, wagon, ItemKind::Block(3)), 0);
        let gameplay = app.world().resource::<EditJournal>().gameplay();
        assert_eq!(
            gameplay
                .drops
                .iter()
                .filter(|drop| drop.stack.item == ItemKind::Block(3))
                .map(|drop| drop.stack.count)
                .sum::<u16>(),
            1
        );
    }
}

#[test]
fn partial_freight_capacity_stows_only_the_accepted_amount_and_drops_the_rest() {
    let (mut app, peer, id, _, _) = builder_fixture();
    let wagon = freight(&mut app, id, true);
    autoload(&mut app, id, true);
    let item = ItemKind::Block(3);
    {
        let mut journal = app.world_mut().resource_mut::<EditJournal>();
        let bag = journal
            .gameplay_mut()
            .trains
            .vehicles
            .get_mut(&wagon)
            .unwrap()
            .cargo
            .values_mut()
            .next()
            .unwrap();
        bag.slots.fill(ItemStack {
            item: ItemKind::Block(4),
            count: ItemKind::Block(4).max_stack(),
        });
        bag.slots[0] = ItemStack {
            item,
            count: item.max_stack() - 1,
        };
    }
    obstacle(&mut app, [4, 8, 12], 3);
    obstacle(&mut app, [5, 8, 12], 3);
    clear(&mut app, &peer, id).unwrap();
    assert_eq!(stored(&app, wagon, item), item.max_stack());
    let gameplay = app.world().resource::<EditJournal>().gameplay();
    assert_eq!(
        gameplay
            .drops
            .iter()
            .filter(|drop| drop.stack.item == item)
            .map(|drop| drop.stack.count)
            .sum::<u16>(),
        1
    );
    assert!(
        gameplay.trains.vehicles[&wagon]
            .cargo
            .values()
            .all(terra_core::containers::valid_chest)
    );
}

#[test]
fn builder_supplies_use_captured_wagon_chests_and_change_each_inventory_once_per_item() {
    let (mut app, peer, id, cargo, graph) = builder_fixture();
    let wagon = freight(&mut app, id, true);
    let chest = {
        let mut journal = app.world_mut().resource_mut::<EditJournal>();
        journal
            .gameplay_mut()
            .trains
            .vehicles
            .get_mut(&id)
            .unwrap()
            .cargo
            .get_mut(&cargo)
            .unwrap()
            .slots
            .clear();
        let wagon = journal
            .gameplay_mut()
            .trains
            .vehicles
            .get_mut(&wagon)
            .unwrap();
        let chest = wagon.parts.len() as u16;
        wagon.parts.push(TrainPart {
            offset: [0, 2, 2],
            material: 29,
            facing: 0,
            wood: WoodSpecies::Oak,
        });
        wagon.blocks.insert(
            chest,
            terra_core::trains::TrainBlockView {
                boxes: vec![terra_core::block_shapes::ShapeBox {
                    minimum: [0.0; 3],
                    maximum: [1.0; 3],
                    hinged: false,
                }],
                ..Default::default()
            },
        );
        wagon.cargo.insert(
            chest,
            Inventory {
                slots: vec![
                    ItemStack {
                        item: ItemKind::Block(196),
                        count: 2,
                    },
                    ItemStack {
                        item: ItemKind::Block(206),
                        count: 64,
                    },
                ],
                revision: 7,
                ..Default::default()
            },
        );
        assert!(wagon.valid());
        chest
    };
    build_step(&mut app, &peer, id, &graph).unwrap();
    let journal = app.world().resource::<EditJournal>();
    let bag = &journal.gameplay().trains.vehicles[&wagon].cargo[&chest];
    assert_eq!(bag.count(ItemKind::Block(196)), 1);
    assert!(bag.count(ItemKind::Block(206)) < 64);
    assert_eq!(bag.revision, 9);
}

#[test]
fn autoload_extension_and_failed_material_preflight_keep_excavation_and_cargo_atomic() {
    for materials in [false, true] {
        let (mut app, peer, id, cargo, graph) = builder_fixture();
        let wagon = freight(&mut app, id, true);
        autoload(&mut app, id, true);
        if !materials {
            app.world_mut()
                .resource_mut::<EditJournal>()
                .gameplay_mut()
                .trains
                .vehicles
                .get_mut(&id)
                .unwrap()
                .cargo
                .get_mut(&cargo)
                .unwrap()
                .slots
                .clear();
        }
        obstacle(&mut app, [4, 8, 12], 3);
        let before = app
            .world()
            .resource::<EditJournal>()
            .gameplay()
            .trains
            .clone();
        if materials {
            build_step(&mut app, &peer, id, &graph).unwrap();
            assert!(stored(&app, wagon, ItemKind::Block(3)) > 0);
            assert!(
                app.world()
                    .resource::<EditJournal>()
                    .gameplay()
                    .drops
                    .is_empty()
            );
        } else {
            assert_eq!(
                build_step(&mut app, &peer, id, &graph),
                Err(TrainError::MissingMaterials)
            );
            assert_eq!(
                app.world().resource::<EditJournal>().gameplay().trains,
                before
            );
            assert_eq!(
                app.world().resource::<ChunkStore>().block_at([4, 8, 12]),
                Some(3)
            );
        }
    }
}

#[test]
fn creative_excavation_does_not_manufacture_cargo_and_revision_overflow_never_clears_the_wall() {
    let (mut app, mut peer, id, _, _) = builder_fixture();
    let wagon = freight(&mut app, id, true);
    autoload(&mut app, id, true);
    obstacle(&mut app, [4, 8, 12], 3);
    peer.mode = Some(GameMode::Creative);
    clear(&mut app, &peer, id).unwrap();
    assert_eq!(stored(&app, wagon, ItemKind::Block(3)), 0);
    assert!(
        app.world()
            .resource::<EditJournal>()
            .gameplay()
            .drops
            .is_empty()
    );
    peer.mode = Some(GameMode::Survival);
    obstacle(&mut app, [4, 8, 12], 3);
    app.world_mut()
        .resource_mut::<EditJournal>()
        .gameplay_mut()
        .trains
        .vehicles
        .get_mut(&wagon)
        .unwrap()
        .revision = u64::MAX;
    assert_eq!(clear(&mut app, &peer, id), Err(TrainError::Overflow));
    assert_eq!(
        app.world().resource::<ChunkStore>().block_at([4, 8, 12]),
        Some(3)
    );
    assert_eq!(stored(&app, wagon, ItemKind::Block(3)), 0);
}

#[test]
fn deep_foundations_consume_exact_cargo_ballast_and_insufficient_supplies_change_nothing() {
    for supply in [0, 1, 2] {
        let (mut app, peer, id, cargo, graph) = builder_fixture();
        let car = app
            .world()
            .resource::<EditJournal>()
            .gameplay()
            .trains
            .vehicles[&id]
            .clone();
        let port = graph.graph().get([4, 5, 3]).unwrap().ports()[1];
        let original = terra_world::rails::builder::plan_for_vehicle(
            app.world().resource::<ChunkStore>(),
            app.world().resource::<EditJournal>(),
            port,
            WoodSpecies::Cherry,
            &car,
        )
        .unwrap();
        let columns: Vec<_> = original
            .layout()
            .edits
            .iter()
            .filter(|edit| edit.material == 206)
            .map(|edit| edit.position)
            .collect();
        assert!(!columns.is_empty());
        for &[x, y, z] in &columns {
            obstacle(&mut app, [x, y, z], 0);
            obstacle(&mut app, [x, y - 1, z], 0);
            obstacle(&mut app, [x, y - 2, z], 3);
        }
        let prepared = terra_world::rails::builder::plan_for_vehicle(
            app.world().resource::<ChunkStore>(),
            app.world().resource::<EditJournal>(),
            port,
            WoodSpecies::Cherry,
            &car,
        )
        .unwrap();
        let required = prepared.ballast();
        assert_eq!(usize::from(required), columns.len() * 2);
        let provided = match supply {
            0 => 0,
            1 => required - 1,
            _ => required,
        };
        {
            let mut journal = app.world_mut().resource_mut::<EditJournal>();
            let bag = journal
                .gameplay_mut()
                .trains
                .vehicles
                .get_mut(&id)
                .unwrap()
                .cargo
                .get_mut(&cargo)
                .unwrap();
            bag.slots.truncate(1);
            let mut remaining = provided;
            while remaining > 0 {
                let count = remaining.min(ItemKind::Block(206).max_stack());
                bag.slots.push(ItemStack {
                    item: ItemKind::Block(206),
                    count,
                });
                remaining -= count;
            }
            assert!(terra_core::containers::valid_chest(bag));
        }
        let before = app
            .world()
            .resource::<EditJournal>()
            .gameplay()
            .trains
            .clone();
        let epoch = app.world().resource::<EditJournal>().rail_epoch();
        if provided < required {
            assert_eq!(
                build_step(&mut app, &peer, id, &graph),
                Err(TrainError::MissingMaterials)
            );
            assert_eq!(
                app.world().resource::<EditJournal>().gameplay().trains,
                before
            );
            assert_eq!(app.world().resource::<EditJournal>().rail_epoch(), epoch);
            for &[x, y, z] in &columns {
                assert_eq!(
                    app.world().resource::<ChunkStore>().block_at([x, y, z]),
                    Some(0)
                );
                assert_eq!(
                    app.world().resource::<ChunkStore>().block_at([x, y - 1, z]),
                    Some(0)
                );
            }
        } else {
            build_step(&mut app, &peer, id, &graph).unwrap();
            assert_eq!(stored(&app, id, ItemKind::Block(206)), 0);
            assert_eq!(
                stored(&app, id, ItemKind::for_wood(196, WoodSpecies::Cherry)),
                3
            );
            for &[x, y, z] in &columns {
                assert_eq!(
                    app.world().resource::<ChunkStore>().block_at([x, y, z]),
                    Some(206)
                );
                assert_eq!(
                    app.world().resource::<ChunkStore>().block_at([x, y - 1, z]),
                    Some(206)
                );
                assert_eq!(
                    app.world().resource::<ChunkStore>().block_at([x, y - 2, z]),
                    Some(3)
                );
            }
        }
        assert!(
            app.world()
                .resource::<EditJournal>()
                .gameplay()
                .drops
                .is_empty()
        );
    }
}
