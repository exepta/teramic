use super::rgb_filter_tests::{fixture, look, place};
use super::*;
use crate::protocol::inventory::{InventoryAction, InventoryCommand};
use bevy::ecs::system::SystemState;
use terra_core::{Inventory, ItemKind, ToolKind, farming::FarmingAction, plumbing::*};
use terra_world::{
    edits::BlockObjectState,
    plumbing::{PlumbingState, Runtime, advance},
    water::FluidCell,
};

#[path = "server_valve_placement_tests.rs"]
mod valve_placement;

const TAP: Cell = [7, 4, 4];
fn setup() -> (App, Peer) {
    let (mut app, mut peer) = fixture();
    for x in 3..=4 {
        for y in 5..=6 {
            for z in 3..=4 {
                let cell = [x, y, z];
                place(&mut app, cell, BASIN, Some(Default::default()));
                app.world_mut()
                    .resource_mut::<EditJournal>()
                    .set_plumbing(
                        cell,
                        PlumbingState {
                            water: FluidCell {
                                amount: 8,
                                ..Default::default()
                            },
                            ..Default::default()
                        },
                    )
                    .unwrap();
            }
        }
    }
    for (cell, material, facing) in [
        ([4, 4, 4], WOOD_PIPE, 4),
        ([5, 4, 4], IRON_PIPE, 0),
        ([6, 4, 4], VALVE, 0),
        (TAP, FAUCET, 1),
    ] {
        place(
            &mut app,
            cell,
            material,
            Some(BlockObjectState {
                facing,
                ..Default::default()
            }),
        );
        app.world_mut()
            .resource_mut::<EditJournal>()
            .set_plumbing(cell, Default::default())
            .unwrap();
    }
    let mut runtime = Runtime::default();
    let mut system = SystemState::<ControlWorld>::new(app.world_mut());
    {
        let mut world = system.get_mut(app.world_mut()).unwrap();
        for _ in 0..5 {
            advance(
                &mut runtime,
                &mut world.chunks,
                &mut world.journal,
                &world.terrain.0,
                0.2,
            )
            .unwrap();
        }
    }
    look(
        &mut app,
        &mut peer,
        Vec3::new(7.5, 5.4, 8.5),
        Vec3::new(7.5, 4.4, 4.5),
    );
    assert!(terra_world::plumbing::tap_available(
        app.world().resource::<ChunkStore>(),
        app.world().resource::<EditJournal>(),
        TAP,
        8
    ));
    (app, peer)
}
fn quantity(app: &App) -> u32 {
    let journal = app.world().resource::<EditJournal>();
    journal
        .plumbing()
        .map(|(_, state)| u32::from(state.water.amount))
        .sum::<u32>()
        + journal
            .fluid_cells()
            .map(|(_, state)| u32::from(state.amount))
            .sum::<u32>()
}
fn bag(app: &App, peer: &Peer) -> Inventory {
    app.world().resource::<EditJournal>().gameplay().inventories[&peer.uuid].clone()
}
fn held(app: &mut App, peer: &mut Peer, item: ItemKind, count: u16) {
    let mut inventory = Inventory::default();
    inventory.insert(item, count);
    app.world_mut()
        .resource_mut::<EditJournal>()
        .gameplay_mut()
        .inventories
        .insert(peer.uuid, inventory);
    peer.held_slot = Some(0);
    peer.farming.reset_cooldown();
}
fn bottle(app: &mut App, peer: &mut Peer, revision: u64) {
    let mut system = SystemState::<ControlWorld>::new(app.world_mut());
    containers::access(
        InventoryCommand {
            revision,
            action: InventoryAction::FillBottle {
                slot: 0,
                position: TAP,
            },
        },
        peer,
        &mut MessageSender::default(),
        &mut system.get_mut(app.world_mut()).unwrap(),
    );
}
fn farm_command(app: &App, peer: &Peer, position: Cell, action: FarmingAction) -> EditCommand {
    let journal = app.world().resource::<EditJournal>();
    let section = terra_voxel::global_to_section(position).0;
    let command = EditCommand {
        request: 1,
        revision: journal.revision(section),
        look: peer.look,
        source: Some(InventorySource {
            slot: 0,
            revision: bag(app, peer).revision,
        }),
        action: EditAction::Farm { position, action },
    };
    #[cfg(feature = "client")]
    assert_eq!(
        crate::client::farming_revision(journal, position, action),
        command.revision
    );
    command
}
fn apply_farm(app: &mut App, peer: &mut Peer, command: EditCommand) -> EditOutcome {
    let mut system = SystemState::<ControlWorld>::new(app.world_mut());
    farm_tools::apply(
        command,
        peer,
        &mut MessageSender::default(),
        &mut system.get_mut(app.world_mut()).unwrap(),
    )
}
fn farm(app: &mut App, peer: &mut Peer, action: FarmingAction) -> EditOutcome {
    let command = farm_command(app, peer, TAP, action);
    apply_farm(app, peer, command)
}

#[test]
fn bottle_bucket_and_can_debit_actual_shared_finite_water_and_reject_stale_or_occluded_use() {
    let (mut app, mut peer) = setup();
    held(&mut app, &mut peer, ItemKind::GlassBottle, 2);
    let before = quantity(&app);
    let revision = bag(&app, &peer).revision;
    bottle(&mut app, &mut peer, revision);
    assert_eq!(bag(&app, &peer).count(ItemKind::GlassBottle), 1);
    assert_eq!(bag(&app, &peer).count(ItemKind::WaterBottle), 1);
    assert_eq!(quantity(&app) + 1, before);
    let saved = bag(&app, &peer);
    let water = quantity(&app);
    bottle(&mut app, &mut peer, revision);
    assert_eq!(bag(&app, &peer), saved);
    assert_eq!(quantity(&app), water);
    place(&mut app, [7, 4, 6], 3, None);
    let revision = bag(&app, &peer).revision;
    bottle(&mut app, &mut peer, revision);
    assert_eq!(bag(&app, &peer), saved);
    assert_eq!(quantity(&app), water);
    place(&mut app, [7, 4, 6], 0, None);
    held(&mut app, &mut peer, ItemKind::Tool(ToolKind::Bucket), 1);
    let before = quantity(&app);
    assert_eq!(
        farm(&mut app, &mut peer, FarmingAction::FillBucket),
        EditOutcome::Applied
    );
    assert_eq!(
        bag(&app, &peer).count(ItemKind::Tool(ToolKind::WaterBucket)),
        1
    );
    assert_eq!(quantity(&app) + 8, before);
    held(
        &mut app,
        &mut peer,
        ItemKind::Tool(ToolKind::WoodWateringCan),
        1,
    );
    let before = quantity(&app);
    assert_eq!(
        farm(&mut app, &mut peer, FarmingAction::Refill),
        EditOutcome::Applied
    );
    assert_eq!(peer.farming.water(), 8);
    assert_eq!(quantity(&app) + 8, before);
    peer.farming.reset_cooldown();
    let before = quantity(&app);
    assert_eq!(
        farm(&mut app, &mut peer, FarmingAction::Refill),
        EditOutcome::Conflict
    );
    assert_eq!(quantity(&app), before);
    assert_eq!(peer.farming.water(), 8);
}

#[test]
fn legacy_can_stack_refill_splits_one_and_preflights_remainder_capacity_before_drawing_water() {
    for packed in [false, true] {
        let (mut app, mut peer) = setup();
        let empty = ItemKind::Tool(ToolKind::WoodWateringCan);
        let full = empty.with_watering_units(8).unwrap();
        held(&mut app, &mut peer, empty, terra_core::STACK_LIMIT);
        if packed {
            let mut journal = app.world_mut().resource_mut::<EditJournal>();
            let inventory = journal
                .gameplay_mut()
                .inventories
                .get_mut(&peer.uuid)
                .unwrap();
            for slot in 1..terra_core::INVENTORY_SLOTS {
                inventory.creative_set(
                    slot,
                    terra_core::ItemStack {
                        item: ItemKind::Block(3),
                        count: terra_core::STACK_LIMIT,
                    },
                );
            }
        }
        let before = bag(&app, &peer);
        let water = quantity(&app);
        let result = farm(&mut app, &mut peer, FarmingAction::Refill);
        if packed {
            assert_eq!(result, EditOutcome::Invalid);
            assert_eq!(bag(&app, &peer), before);
            assert_eq!(quantity(&app), water);
        } else {
            assert_eq!(result, EditOutcome::Applied);
            let after = bag(&app, &peer);
            assert_eq!(after.slot(0).unwrap().item, full);
            assert_eq!(after.count(empty), terra_core::STACK_LIMIT - 1);
            assert_eq!(after.count(full), 1);
            assert_eq!(quantity(&app) + 8, water);
            assert!(after.valid());
        }
    }
}

#[test]
fn valve_toggle_uses_real_build_permission_revision_and_los_and_closed_supply_cannot_fill() {
    let (mut app, mut peer) = setup();
    let valve = [6, 4, 4];
    look(
        &mut app,
        &mut peer,
        Vec3::new(6.5, 5.4, 8.5),
        Vec3::new(6.5, 4.5, 4.5),
    );
    let command = EditCommand {
        request: 1,
        revision: app
            .world()
            .resource::<EditJournal>()
            .revision(terra_voxel::global_to_section(valve).0),
        look: peer.look,
        source: None,
        action: EditAction::ToggleObject {
            position: valve,
            expected: VALVE,
            facing: 0,
            closing: false,
        },
    };
    let call = |app: &mut App, peer: &Peer, command| {
        let mut system = SystemState::<ControlWorld>::new(app.world_mut());
        validation::apply(
            command,
            peer,
            &mut system.get_mut(app.world_mut()).unwrap(),
            false,
        )
    };
    peer.permissions.build = false;
    assert_eq!(call(&mut app, &peer, command), EditOutcome::Invalid);
    peer.permissions.build = true;
    assert_eq!(call(&mut app, &peer, command), EditOutcome::Applied);
    assert_eq!(
        app.world()
            .resource::<EditJournal>()
            .object_state(valve)
            .unwrap()
            .facing,
        8
    );
    assert_eq!(call(&mut app, &peer, command), EditOutcome::Conflict);
    held(&mut app, &mut peer, ItemKind::GlassBottle, 1);
    look(
        &mut app,
        &mut peer,
        Vec3::new(7.5, 5.4, 8.5),
        Vec3::new(7.5, 4.4, 4.5),
    );
    let water = quantity(&app);
    let revision = bag(&app, &peer).revision;
    bottle(&mut app, &mut peer, revision);
    assert_eq!(bag(&app, &peer).count(ItemKind::WaterBottle), 0);
    assert_eq!(bag(&app, &peer).count(ItemKind::GlassBottle), 1);
    assert_eq!(quantity(&app), water);
}

#[test]
fn delayed_tap_bucket_fill_ignores_live_outlet_mesh_updates_but_not_changed_object_pose() {
    let (mut app, mut peer) = setup();
    held(&mut app, &mut peer, ItemKind::Tool(ToolKind::Bucket), 1);
    let command = farm_command(&app, &peer, TAP, FarmingAction::FillBucket);
    let section = terra_voxel::global_to_section(TAP).0;
    let water_epoch = app
        .world()
        .resource::<EditJournal>()
        .water_mesh_epoch(section);
    {
        let mut system = SystemState::<ControlWorld>::new(app.world_mut());
        let mut world = system.get_mut(app.world_mut()).unwrap();
        advance(
            &mut Runtime::default(),
            &mut world.chunks,
            &mut world.journal,
            &world.terrain.0,
            0.2,
        )
        .unwrap();
    }
    assert!(
        app.world()
            .resource::<EditJournal>()
            .water_mesh_epoch(section)
            > water_epoch,
        "real running faucet must change its water mesh while the request is in flight"
    );
    assert_eq!(
        app.world().resource::<EditJournal>().revision(section),
        command.revision
    );
    let before = quantity(&app);
    assert_eq!(
        apply_farm(&mut app, &mut peer, command),
        EditOutcome::Applied
    );
    assert_eq!(quantity(&app) + 8, before);
    assert_eq!(
        bag(&app, &peer).count(ItemKind::Tool(ToolKind::WaterBucket)),
        1
    );

    let (mut app, mut peer) = setup();
    held(&mut app, &mut peer, ItemKind::Tool(ToolKind::Bucket), 1);
    let command = farm_command(&app, &peer, TAP, FarmingAction::FillBucket);
    let mut pose = app
        .world()
        .resource::<EditJournal>()
        .object_state(TAP)
        .unwrap();
    pose.facing ^= 4;
    app.world_mut()
        .resource_mut::<EditJournal>()
        .set_object_state(TAP, Some(pose))
        .unwrap();
    let before = quantity(&app);
    let inventory = bag(&app, &peer);
    assert_eq!(
        apply_farm(&mut app, &mut peer, command),
        EditOutcome::Conflict
    );
    assert_eq!(quantity(&app), before);
    assert_eq!(bag(&app, &peer), inventory);
}

#[test]
fn delayed_basin_bucket_pour_uses_block_revision_and_full_basin_still_rejects_atomically() {
    let (mut app, mut peer) = setup();
    let basin = [10, 4, 4];
    place(&mut app, basin, BASIN, Some(Default::default()));
    app.world_mut()
        .resource_mut::<EditJournal>()
        .set_plumbing(
            basin,
            PlumbingState {
                basin: Some(BasinShape {
                    origin: basin,
                    size: [1; 3],
                }),
                ..Default::default()
            },
        )
        .unwrap();
    held(
        &mut app,
        &mut peer,
        ItemKind::Tool(ToolKind::WaterBucket),
        1,
    );
    look(
        &mut app,
        &mut peer,
        Vec3::new(10.5, 5.4, 8.5),
        Vec3::new(10.5, 4.4, 4.5),
    );
    let command = farm_command(&app, &peer, basin, FarmingAction::EmptyBucket);
    let section = terra_voxel::global_to_section(basin).0;
    let water_epoch = app
        .world()
        .resource::<EditJournal>()
        .water_mesh_epoch(section);
    app.world_mut()
        .resource_mut::<EditJournal>()
        .set_fluid(
            [8, 3, 4],
            FluidCell {
                amount: 1,
                ..Default::default()
            },
        )
        .unwrap();
    assert!(
        app.world()
            .resource::<EditJournal>()
            .water_mesh_epoch(section)
            > water_epoch
    );
    assert_eq!(
        app.world().resource::<EditJournal>().revision(section),
        command.revision
    );
    let before = quantity(&app);
    assert_eq!(
        apply_farm(&mut app, &mut peer, command),
        EditOutcome::Applied
    );
    assert_eq!(quantity(&app), before + 8);
    assert_eq!(bag(&app, &peer).count(ItemKind::Tool(ToolKind::Bucket)), 1);
    held(
        &mut app,
        &mut peer,
        ItemKind::Tool(ToolKind::WaterBucket),
        1,
    );
    let command = farm_command(&app, &peer, basin, FarmingAction::EmptyBucket);
    let before = quantity(&app);
    let inventory = bag(&app, &peer);
    assert_eq!(
        apply_farm(&mut app, &mut peer, command),
        EditOutcome::Conflict
    );
    assert_eq!(quantity(&app), before);
    assert_eq!(bag(&app, &peer), inventory);
}

#[test]
fn removed_natural_source_and_removed_tap_never_use_old_fill_admission() {
    let (mut app, mut peer) = setup();
    let source = [9, 4, 7];
    place(&mut app, source, 27, None);
    app.world_mut()
        .resource_mut::<EditJournal>()
        .set_fluid(
            source,
            FluidCell {
                amount: 8,
                source: true,
                ..Default::default()
            },
        )
        .unwrap();
    held(&mut app, &mut peer, ItemKind::Tool(ToolKind::Bucket), 1);
    look(
        &mut app,
        &mut peer,
        Vec3::new(9.5, 5.4, 11.5),
        Vec3::new(9.5, 4.4, 7.5),
    );
    let command = farm_command(&app, &peer, source, FarmingAction::FillBucket);
    app.world_mut()
        .resource_mut::<EditJournal>()
        .set_fluid(
            source,
            FluidCell {
                amount: 0,
                ..Default::default()
            },
        )
        .unwrap();
    let before = quantity(&app);
    let inventory = bag(&app, &peer);
    assert_ne!(
        apply_farm(&mut app, &mut peer, command),
        EditOutcome::Applied
    );
    assert_eq!(quantity(&app), before);
    assert_eq!(bag(&app, &peer), inventory);

    let (mut app, mut peer) = setup();
    held(&mut app, &mut peer, ItemKind::Tool(ToolKind::Bucket), 1);
    let command = farm_command(&app, &peer, TAP, FarmingAction::FillBucket);
    place(&mut app, TAP, 3, None);
    let before = quantity(&app);
    let inventory = bag(&app, &peer);
    assert_ne!(
        apply_farm(&mut app, &mut peer, command),
        EditOutcome::Applied
    );
    assert_eq!(quantity(&app), before);
    assert_eq!(bag(&app, &peer), inventory);
}

#[test]
fn faucet_steps_are_authoritative_bounded_and_preserve_facing_and_water() {
    let (mut app, mut peer) = setup();
    let initial = quantity(&app);
    for (closing, expected_level) in [
        (false, 2),
        (false, 3),
        (false, 3),
        (true, 2),
        (true, 1),
        (true, 0),
        (true, 0),
    ] {
        let journal = app.world().resource::<EditJournal>();
        let facing = journal.object_state(TAP).unwrap().facing;
        let command = EditCommand {
            request: 1,
            revision: journal.revision(terra_voxel::global_to_section(TAP).0),
            look: peer.look,
            source: None,
            action: EditAction::ToggleObject {
                position: TAP,
                expected: FAUCET,
                facing,
                closing,
            },
        };
        let call = |app: &mut App, peer: &Peer, command| {
            let mut system = SystemState::<ControlWorld>::new(app.world_mut());
            validation::apply(
                command,
                peer,
                &mut system.get_mut(app.world_mut()).unwrap(),
                false,
            )
        };
        peer.permissions.build = false;
        assert_eq!(call(&mut app, &peer, command), EditOutcome::Invalid);
        peer.permissions.build = true;
        assert_eq!(call(&mut app, &peer, command), EditOutcome::Applied);
        let state = app
            .world()
            .resource::<EditJournal>()
            .object_state(TAP)
            .unwrap();
        assert_eq!(faucet_level(state.facing), expected_level);
        assert_eq!(orientation(FAUCET, state.facing), 1);
        assert_eq!(quantity(&app), initial);
        if state.facing != facing {
            assert_eq!(call(&mut app, &peer, command), EditOutcome::Conflict);
        }
    }
}
