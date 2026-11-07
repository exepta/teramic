use super::*;
use crate::protocol::inventory::{ContainerCommand, ContainerView};
use crate::server::rgb_filter_tests::{fixture, look, place};
use bevy::ecs::system::SystemState;
use std::collections::BTreeMap;
use terra_core::{ItemStack, ToolKind, brewing, campfire, containers::ContainerGesture};
use terra_world::edits::BlockObjectState;

const HOST: [i64; 3] = [-1, 16, -1];

fn aim(app: &mut App, peer: &mut Peer) {
    let mut points = Vec::new();
    terra_world::interaction_shapes::visit_boxes(
        app.world().resource::<ChunkStore>(),
        app.world().resource::<EditJournal>(),
        app.world()
            .resource::<terra_core::block_properties::BlockPropertiesCatalog>(),
        HOST,
        |bounds| {
            points.push(Vec3::from_array(std::array::from_fn(|axis| {
                ((bounds.minimum[axis] + bounds.maximum[axis]) * 0.5) as f32
            })))
        },
    );
    look(app, peer, Vec3::new(-0.5, 17.8, 2.5), points[0]);
}

fn edit(app: &App, peer: &Peer, expected: u16, material: u16) -> EditCommand {
    EditCommand {
        request: 1,
        revision: app
            .world()
            .resource::<EditJournal>()
            .revision(terra_voxel::global_to_section(HOST).0),
        look: peer.look,
        source: (material != 0).then(|| InventorySource {
            slot: 0,
            revision: app.world().resource::<EditJournal>().gameplay().inventories[&peer.uuid]
                .revision,
        }),
        action: EditAction::Block {
            position: HOST,
            expected,
            material,
        },
    }
}

fn fixture_with(first: u16) -> (App, Peer) {
    let (mut app, mut peer) = fixture();
    for offset in [
        [0, -1, 0],
        [0, 1, 0],
        [-1, 0, 0],
        [1, 0, 0],
        [0, 0, -1],
        [0, 0, 1],
    ] {
        let position = std::array::from_fn(|axis| HOST[axis] + offset[axis]);
        place(
            &mut app,
            position,
            if offset == [0, -1, 0] { 3 } else { 0 },
            None,
        );
    }
    place(&mut app, HOST, first, Some(BlockObjectState::default()));
    if first == 162 {
        let food = &campfire::recipes()[0];
        let mut inventory = Inventory {
            slots: vec![ItemStack::EMPTY; 3],
            ..Default::default()
        };
        inventory.slots[0] = ItemStack {
            item: food.input,
            count: 2,
        };
        inventory.slots[1] = ItemStack {
            item: ItemKind::Stick,
            count: 3,
        };
        inventory.slots[2] = food.output;
        app.world_mut()
            .resource_mut::<EditJournal>()
            .commit_campfire(
                HOST,
                campfire::CampfireState {
                    lit: true,
                    burn_remaining: 5.0,
                    burn_total: 8.0,
                    recipe: Some(food.id),
                    progress_seconds: 1.0,
                    completed: 2,
                },
                inventory,
            )
            .unwrap();
    } else {
        let mut inventory = Inventory {
            slots: vec![ItemStack::EMPTY; 7],
            ..Default::default()
        };
        inventory.slots[0] = brewing::recipe(7).unwrap().inputs[0];
        inventory.slots[4] = ItemStack {
            item: ItemKind::GlassBottle,
            count: 2,
        };
        inventory.slots[5] = ItemStack {
            item: ItemKind::Tool(ToolKind::GoldSpoon),
            count: 1,
        };
        app.world_mut()
            .resource_mut::<EditJournal>()
            .commit_cauldron(
                HOST,
                brewing::CauldronState {
                    recipe: Some(7),
                    water: 2,
                    progress_seconds: 1.5,
                },
                inventory,
            )
            .unwrap();
    }
    let mut inventory = Inventory::default();
    inventory.insert(ItemKind::Block(if first == 161 { 162 } else { 161 }), 1);
    app.world_mut()
        .resource_mut::<EditJournal>()
        .gameplay_mut()
        .inventories
        .insert(peer.uuid, inventory);
    aim(&mut app, &mut peer);
    (app, peer)
}

fn apply(app: &mut App, peer: &Peer, command: EditCommand) -> EditOutcome {
    let mut system = SystemState::<ControlWorld>::new(app.world_mut());
    crate::server::validation::apply(
        command,
        peer,
        &mut system.get_mut(app.world_mut()).unwrap(),
        true,
    )
}

fn access(app: &mut App, peer: &mut Peer, action: InventoryAction) {
    let revision =
        app.world().resource::<EditJournal>().gameplay().inventories[&peer.uuid].revision;
    let mut system = SystemState::<ControlWorld>::new(app.world_mut());
    super::super::access(
        InventoryCommand { revision, action },
        peer,
        &mut MessageSender::default(),
        &mut system.get_mut(app.world_mut()).unwrap(),
    );
}

fn stacks(inventories: impl IntoIterator<Item = Inventory>) -> BTreeMap<ItemKind, u32> {
    let mut values = BTreeMap::new();
    for inventory in inventories {
        for stack in inventory.slots.into_iter().filter(|stack| stack.valid()) {
            *values.entry(stack.item).or_default() += u32::from(stack.count);
        }
    }
    values
}

#[test]
fn survival_compound_upgrades_both_orders_in_one_cell_and_mines_both_blocks_and_contents_once() {
    for first in [161, 162] {
        let (mut app, mut peer) = fixture_with(first);
        let before_inventory = app
            .world()
            .resource::<EditJournal>()
            .container_at(HOST)
            .unwrap()
            .clone();
        let before_fire = app
            .world()
            .resource::<EditJournal>()
            .campfire_at(HOST)
            .cloned();
        let before_kettle = app
            .world()
            .resource::<EditJournal>()
            .cauldron_at(HOST)
            .cloned();
        let material = if first == 161 { 162 } else { 161 };
        let command = edit(&app, &peer, first, material);
        assert_eq!(
            apply(&mut app, &peer, command),
            EditOutcome::Applied,
            "first {first}"
        );
        let journal = app.world().resource::<EditJournal>();
        assert_eq!(
            app.world().resource::<ChunkStore>().block_at(HOST),
            Some(161)
        );
        assert!(journal.has_attached_campfire(HOST));
        assert!(journal.gameplay().drops.is_empty());
        assert_eq!(
            journal.gameplay().inventories[&peer.uuid].count(ItemKind::Block(material)),
            0
        );
        if first == 162 {
            assert_eq!(
                journal.attached_campfire_at(HOST).unwrap().state,
                before_fire.unwrap()
            );
            assert_eq!(
                journal.attached_campfire_at(HOST).unwrap().inventory,
                before_inventory
            );
            assert!(journal.campfire_at(HOST).is_none());
        } else {
            assert_eq!(journal.cauldron_at(HOST), before_kettle.as_ref());
            assert_eq!(journal.container_at(HOST), Some(&before_inventory));
        }
        let mut expected = stacks([
            journal.container_at(HOST).unwrap().clone(),
            journal
                .attached_campfire_at(HOST)
                .unwrap()
                .inventory
                .clone(),
        ]);
        *expected.entry(ItemKind::Block(161)).or_default() += 1;
        *expected.entry(ItemKind::Block(162)).or_default() += 1;
        let mut inventory = Inventory::default();
        inventory.insert(ItemKind::Tool(ToolKind::IronPickaxe), 1);
        app.world_mut()
            .resource_mut::<EditJournal>()
            .gameplay_mut()
            .inventories
            .insert(peer.uuid, inventory);
        aim(&mut app, &mut peer);
        let removal = edit(&app, &peer, 161, 0);
        let next_drop = app.world().resource::<EditJournal>().gameplay().next_drop;
        app.world_mut()
            .resource_mut::<EditJournal>()
            .gameplay_mut()
            .next_drop = u64::MAX;
        assert_eq!(apply(&mut app, &peer, removal), EditOutcome::Invalid);
        assert!(
            app.world()
                .resource::<EditJournal>()
                .has_attached_campfire(HOST)
        );
        assert!(
            app.world()
                .resource::<EditJournal>()
                .gameplay()
                .drops
                .is_empty()
        );
        app.world_mut()
            .resource_mut::<EditJournal>()
            .gameplay_mut()
            .next_drop = next_drop;
        assert_eq!(apply(&mut app, &peer, removal), EditOutcome::Applied);
        let journal = app.world().resource::<EditJournal>();
        assert!(
            journal.attached_campfire_at(HOST).is_none() && journal.cauldron_at(HOST).is_none()
        );
        let mut actual = BTreeMap::new();
        for drop in &journal.gameplay().drops {
            *actual.entry(drop.stack.item).or_default() += u32::from(drop.stack.count);
        }
        assert_eq!(actual, expected);
        let drop_count = journal.gameplay().drops.len();
        assert_ne!(apply(&mut app, &peer, removal), EditOutcome::Applied);
        assert_eq!(
            app.world().resource::<EditJournal>().gameplay().drops.len(),
            drop_count
        );
    }
}

#[test]
fn same_anchor_equal_revision_stale_gestures_cannot_cross_kettle_and_attached_fire_views() {
    let (mut app, mut peer) = fixture_with(162);
    let upgrade = edit(&app, &peer, 162, 161);
    assert_eq!(apply(&mut app, &peer, upgrade), EditOutcome::Applied);
    aim(&mut app, &mut peer);
    access(
        &mut app,
        &mut peer,
        InventoryAction::OpenContainer { position: HOST },
    );
    assert_eq!(
        peer.container.as_ref().unwrap().view,
        ContainerView::Primary
    );
    assert!(peer.container.as_ref().unwrap().attached_campfire);
    let mut delayed = ContainerCommand {
        inventory_revision: app.world().resource::<EditJournal>().gameplay().inventories
            [&peer.uuid]
            .revision,
        anchor: HOST,
        view: ContainerView::Primary,
        revisions: vec![0],
        reserved: [0; terra_core::INVENTORY_SLOTS],
        action: ContainerGesture::QuickMove {
            slot: terra_core::INVENTORY_SLOTS as u8,
        },
    };
    access(
        &mut app,
        &mut peer,
        InventoryAction::OpenCampfire { position: HOST },
    );
    assert_eq!(
        peer.container.as_ref().unwrap().view,
        ContainerView::AttachedCampfire
    );
    assert_eq!(
        peer.container.as_ref().unwrap().halves[0]
            .inventory
            .revision,
        0
    );
    let before = app.world().resource::<EditJournal>().gameplay().inventories[&peer.uuid].clone();
    let fire = app
        .world()
        .resource::<EditJournal>()
        .attached_campfire_at(HOST)
        .unwrap()
        .clone();
    let kettle = app
        .world()
        .resource::<EditJournal>()
        .container_at(HOST)
        .unwrap()
        .clone();
    {
        let mut system = SystemState::<ControlWorld>::new(app.world_mut());
        super::super::transfer(
            delayed.clone(),
            &mut peer,
            &mut MessageSender::default(),
            &mut system.get_mut(app.world_mut()).unwrap(),
        );
    }
    assert_eq!(
        app.world().resource::<EditJournal>().gameplay().inventories[&peer.uuid],
        before
    );
    assert_eq!(
        app.world()
            .resource::<EditJournal>()
            .attached_campfire_at(HOST),
        Some(&fire)
    );
    delayed.view = ContainerView::AttachedCampfire;
    {
        let mut system = SystemState::<ControlWorld>::new(app.world_mut());
        super::super::transfer(
            delayed,
            &mut peer,
            &mut MessageSender::default(),
            &mut system.get_mut(app.world_mut()).unwrap(),
        );
    }
    assert!(
        app.world()
            .resource::<EditJournal>()
            .attached_campfire_at(HOST)
            .unwrap()
            .inventory
            .slot(campfire::INPUT)
            .is_none()
    );
    assert_eq!(
        app.world().resource::<EditJournal>().gameplay().inventories[&peer.uuid]
            .count(fire.inventory.slot(0).unwrap().item),
        2
    );
    assert_eq!(
        app.world().resource::<EditJournal>().container_at(HOST),
        Some(&kettle)
    );
    access(
        &mut app,
        &mut peer,
        InventoryAction::OpenContainer { position: HOST },
    );
    assert_eq!(
        peer.container.as_ref().unwrap().view,
        ContainerView::Primary
    );
}
