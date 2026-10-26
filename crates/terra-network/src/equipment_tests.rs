use super::*;
use crate::{
    client::{ClientSession, InventoryRequest, ReplicatedPlayers},
    protocol::inventory::{InventoryAction, InventoryCommand, InventoryOutcome},
};
use terra_core::{
    AccessoryKind, ArmorItem, ArmorMaterial, ArmorPiece, EquipmentSlot, EquipmentSlots, GameMode,
    ItemKind, ItemStack, PlayerProfile, PlayerStats, SurvivalVitals,
};
use terra_world::edits::EditJournal;

fn helmet(material: ArmorMaterial) -> ItemKind {
    ItemKind::Armor(ArmorItem {
        material,
        piece: ArmorPiece::Helmet,
    })
}

fn fixture() -> (App, App, App, PlayerProfile) {
    let mut host = server();
    host.world_mut()
        .resource_mut::<GameConfig>()
        .server
        .game_mode = GameMode::Survival;
    let profile = PlayerProfile::default();
    {
        let mut journal = host.world_mut().resource_mut::<EditJournal>();
        journal.update_player_stats(
            profile.uuid,
            PlayerStats {
                survival: SurvivalVitals {
                    health: 60.0,
                    hunger: 50.0,
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        let bag = journal
            .gameplay_mut()
            .inventories
            .entry(profile.uuid)
            .or_default();
        for item in [
            helmet(ArmorMaterial::Iron),
            helmet(ArmorMaterial::Leather),
            ItemKind::Accessory(AccessoryKind::Necklace),
            ItemKind::Accessory(AccessoryKind::Belt),
            ItemKind::Block(3),
        ] {
            assert_eq!(bag.insert(item, 1), 0);
        }
    }
    let address = host
        .world()
        .resource::<server::ServerRuntime>()
        .address
        .unwrap();
    let mut alice = profile_client(address, profile.clone());
    let mut bob = profile_client(address, PlayerProfile::default());
    wait(&mut host, &mut [&mut alice, &mut bob], |_, clients| {
        clients
            .iter()
            .all(|client| client.world().resource::<ClientSession>().ready)
            && clients[0]
                .world()
                .resource::<ClientSession>()
                .inventory
                .count(helmet(ArmorMaterial::Iron))
                == 1
    });
    (host, alice, bob, profile)
}

fn action(
    host: &mut App,
    alice: &mut App,
    bob: &mut App,
    action: InventoryAction,
) -> InventoryOutcome {
    let session = alice.world().resource::<ClientSession>();
    let revision = session.inventory.revision;
    let response = session.inventory_response;
    let uuid = session.uuid.unwrap();
    alice
        .world_mut()
        .write_message(InventoryRequest(InventoryCommand { revision, action }));
    wait(host, &mut [alice, bob], |host, clients| {
        clients[0]
            .world()
            .resource::<ClientSession>()
            .inventory_response
            > response
            && clients[0]
                .world()
                .resource::<ClientSession>()
                .stats
                .equipment
                == host
                    .world()
                    .resource::<EditJournal>()
                    .gameplay()
                    .player_stats[&uuid]
                    .equipment
    });
    alice
        .world()
        .resource::<ClientSession>()
        .last_inventory
        .unwrap()
}

#[test]
fn two_udp_clients_share_atomic_equipment_cursor_moves_and_derived_caps_without_free_healing() {
    let (mut host, mut alice, mut bob, profile) = fixture();
    let player = alice.world().resource::<ClientSession>().player.unwrap();
    assert_eq!(
        action(
            &mut host,
            &mut alice,
            &mut bob,
            InventoryAction::QuickMove { slot: 0 }
        ),
        InventoryOutcome::Applied
    );
    wait(&mut host, &mut [&mut alice, &mut bob], |host, clients| {
        let saved = host
            .world()
            .resource::<EditJournal>()
            .gameplay()
            .player_stats[&profile.uuid];
        saved.equipment.get(EquipmentSlot::Helmet) == Some(helmet(ArmorMaterial::Iron))
            && clients[0]
                .world()
                .resource::<ClientSession>()
                .stats
                .equipment
                == saved.equipment
            && clients[1]
                .world()
                .resource::<ReplicatedPlayers>()
                .players
                .get(&player)
                .is_some_and(|track| {
                    track.current.equipment == saved.equipment
                        && track.current.survival.armor_points == saved.survival.armor_points
                })
    });
    let session = alice.world().resource::<ClientSession>();
    assert_eq!(session.inventory.count(helmet(ArmorMaterial::Iron)), 0);
    assert_eq!(session.stats.survival.health, 60.0);
    assert!(session.stats.survival.armor_points > 0.0);
    for slot in [2, 3] {
        assert_eq!(
            action(
                &mut host,
                &mut alice,
                &mut bob,
                InventoryAction::QuickMove { slot }
            ),
            InventoryOutcome::Applied
        );
    }
    let before = alice.world().resource::<ClientSession>().stats;
    assert!(before.survival.max_health > 100.0 && before.survival.max_stamina > 100.0);
    assert_eq!(before.survival.health, 60.0);
    assert_eq!(
        action(
            &mut host,
            &mut alice,
            &mut bob,
            InventoryAction::EquipmentClick {
                slot: 0,
                secondary: true
            }
        ),
        InventoryOutcome::Applied
    );
    assert_eq!(
        alice.world().resource::<ClientSession>().inventory.cursor,
        Some(ItemStack {
            item: helmet(ArmorMaterial::Iron),
            count: 1
        })
    );
    let inventory = alice.world().resource::<ClientSession>().inventory.clone();
    let equipment = alice.world().resource::<ClientSession>().stats.equipment;
    assert_eq!(
        action(
            &mut host,
            &mut alice,
            &mut bob,
            InventoryAction::EquipmentClick {
                slot: EquipmentSlot::Boots.index() as u8,
                secondary: false
            }
        ),
        InventoryOutcome::Invalid
    );
    assert_eq!(
        alice.world().resource::<ClientSession>().inventory,
        inventory
    );
    assert_eq!(
        alice.world().resource::<ClientSession>().stats.equipment,
        equipment
    );
    assert_eq!(
        action(
            &mut host,
            &mut alice,
            &mut bob,
            InventoryAction::EquipmentClick {
                slot: 0,
                secondary: true
            }
        ),
        InventoryOutcome::Applied
    );
    assert!(
        alice
            .world()
            .resource::<ClientSession>()
            .inventory
            .cursor
            .is_none()
    );
    assert_eq!(
        action(
            &mut host,
            &mut alice,
            &mut bob,
            InventoryAction::EquipmentQuickMove {
                slot: EquipmentSlot::Necklace.index() as u8
            }
        ),
        InventoryOutcome::Applied
    );
    let session = alice.world().resource::<ClientSession>();
    assert_eq!(
        session
            .inventory
            .count(ItemKind::Accessory(AccessoryKind::Necklace)),
        1
    );
    assert_eq!(session.stats.equipment.get(EquipmentSlot::Necklace), None);
    assert_eq!(session.stats.survival.health, 60.0);
    assert!(session.stats.survival.max_health < before.survival.max_health);
    assert_eq!(
        host.world()
            .resource::<EditJournal>()
            .gameplay()
            .player_stats[&profile.uuid]
            .equipment,
        session.stats.equipment
    );
}

#[test]
fn equipment_inventory_rejects_replays_bad_slots_and_forged_creative_overstacks() {
    let (mut host, mut alice, mut bob, profile) = fixture();
    let stale_revision = alice.world().resource::<ClientSession>().inventory.revision;
    assert_eq!(
        action(
            &mut host,
            &mut alice,
            &mut bob,
            InventoryAction::QuickMove { slot: 0 }
        ),
        InventoryOutcome::Applied
    );
    let equipment = alice.world().resource::<ClientSession>().stats.equipment;
    let inventory = alice.world().resource::<ClientSession>().inventory.clone();
    let response = alice.world().resource::<ClientSession>().inventory_response;
    alice
        .world_mut()
        .write_message(InventoryRequest(InventoryCommand {
            revision: stale_revision,
            action: InventoryAction::EquipmentQuickMove { slot: 0 },
        }));
    wait(&mut host, &mut [&mut alice, &mut bob], |_, clients| {
        clients[0]
            .world()
            .resource::<ClientSession>()
            .inventory_response
            > response
    });
    assert_eq!(
        alice.world().resource::<ClientSession>().last_inventory,
        Some(InventoryOutcome::Conflict)
    );
    assert_eq!(
        alice.world().resource::<ClientSession>().inventory,
        inventory
    );
    assert_eq!(
        alice.world().resource::<ClientSession>().stats.equipment,
        equipment
    );
    assert_eq!(
        action(
            &mut host,
            &mut alice,
            &mut bob,
            InventoryAction::EquipmentClick {
                slot: 250,
                secondary: false
            }
        ),
        InventoryOutcome::Invalid
    );
    assert_eq!(
        action(
            &mut host,
            &mut alice,
            &mut bob,
            InventoryAction::CreativeCursor {
                item: helmet(ArmorMaterial::Iron),
                count: 128
            }
        ),
        InventoryOutcome::Denied
    );
    host.world_mut()
        .resource_mut::<GameConfig>()
        .server
        .game_mode = GameMode::Creative;
    assert_eq!(
        action(
            &mut host,
            &mut alice,
            &mut bob,
            InventoryAction::CreativeCursor {
                item: helmet(ArmorMaterial::Iron),
                count: 128
            }
        ),
        InventoryOutcome::Invalid
    );
    assert_eq!(
        alice.world().resource::<ClientSession>().inventory,
        inventory
    );
    assert_eq!(
        host.world()
            .resource::<EditJournal>()
            .gameplay()
            .player_stats[&profile.uuid]
            .equipment,
        equipment
    );
}

#[test]
fn snapshot_equipment_validation_rejects_forged_caps_and_wrong_slot_loadouts() {
    let mut equipment = EquipmentSlots::default();
    equipment.set(EquipmentSlot::Helmet, Some(helmet(ArmorMaterial::Iron)));
    let mut vitals = SurvivalVitals::default();
    assert!(!client::valid_equipment_vitals(vitals, equipment));
    vitals.apply_equipment(equipment);
    assert!(client::valid_equipment_vitals(vitals, equipment));
    equipment.slots[EquipmentSlot::Boots.index()] = equipment.slots[EquipmentSlot::Helmet.index()];
    assert!(!client::valid_equipment_vitals(vitals, equipment));
}

#[test]
fn equipment_without_stats_capacity_is_rejected_before_removing_the_backpack_item() {
    let (mut host, mut alice, mut bob, profile) = fixture();
    {
        let mut journal = host.world_mut().resource_mut::<EditJournal>();
        let stats = &mut journal.gameplay_mut().player_stats;
        stats.clear();
        while stats.len() < 4096 {
            let id = terra_core::PlayerUuid::new_v4();
            if id != profile.uuid {
                stats.insert(id, PlayerStats::default());
            }
        }
    }
    let inventory = alice.world().resource::<ClientSession>().inventory.clone();
    let equipment = alice.world().resource::<ClientSession>().stats.equipment;
    let response = alice.world().resource::<ClientSession>().inventory_response;
    alice
        .world_mut()
        .write_message(InventoryRequest(InventoryCommand {
            revision: inventory.revision,
            action: InventoryAction::QuickMove { slot: 0 },
        }));
    wait(&mut host, &mut [&mut alice, &mut bob], |_, clients| {
        clients[0]
            .world()
            .resource::<ClientSession>()
            .inventory_response
            > response
    });
    let session = alice.world().resource::<ClientSession>();
    assert_eq!(session.last_inventory, Some(InventoryOutcome::Invalid));
    assert_eq!(session.inventory, inventory);
    assert_eq!(session.stats.equipment, equipment);
    let journal = host.world().resource::<EditJournal>();
    assert_eq!(journal.gameplay().inventories[&profile.uuid], inventory);
    assert!(!journal.gameplay().player_stats.contains_key(&profile.uuid));
}
