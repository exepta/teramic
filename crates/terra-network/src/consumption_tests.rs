use super::*;
use client::{ClientSession, SelectedInventorySlot};
use lightyear::prelude::{MessageSender, client::Client};
use protocol::session::{ClientControl, ControlChannel};
use terra_core::{GameMode, Inventory, ItemKind, PlayerProfile, PlayerStats, ResourcePropKind};
use terra_world::edits::EditJournal;

const FOOD: ItemKind = ItemKind::Prop(ResourcePropKind::Mushroom);

fn fixture(mode: GameMode) -> (App, App, PlayerProfile) {
    fixture_with_hunger(mode, 20.0)
}

fn fixture_with_hunger(mode: GameMode, hunger: f32) -> (App, App, PlayerProfile) {
    let mut host = server();
    {
        let mut config = host.world_mut().resource_mut::<GameConfig>();
        config.server.game_mode = mode;
        config.server.natural_mob_limit = Some(0);
    }
    let profile = PlayerProfile::default();
    {
        let mut journal = host.world_mut().resource_mut::<EditJournal>();
        journal.update_player_stats(
            profile.uuid,
            PlayerStats {
                survival: terra_core::SurvivalVitals {
                    health: 70.0,
                    hunger,
                    ..default()
                },
                game_mode: Some(mode),
                ..default()
            },
        );
        journal
            .gameplay_mut()
            .inventories
            .entry(profile.uuid)
            .or_default()
            .insert(FOOD, 3);
    }
    let address = host
        .world()
        .resource::<server::ServerRuntime>()
        .address
        .unwrap();
    let mut client = profile_client(address, profile.clone());
    client.world_mut().resource_mut::<SelectedInventorySlot>().0 = Some(0);
    wait(&mut host, &mut [&mut client], |_, clients| {
        let session = clients[0].world().resource::<ClientSession>();
        session.ready
            && session.inventory.count(FOOD) == 3
            && session
                .confirmed_state()
                .is_some_and(|state| state.grounded || mode == GameMode::Spectator)
    });
    (host, client, profile)
}

fn send(client: &mut App, packet: ClientControl) {
    let world = client.world_mut();
    world
        .query_filtered::<&mut MessageSender<ClientControl>, With<Client>>()
        .single_mut(world)
        .unwrap()
        .send::<ControlChannel>(packet);
}

fn pulse(client: &mut App, slot: Option<u8>, revision: u64) {
    send(client, ClientControl::Consumption { slot, revision });
}

fn start(host: &mut App, client: &mut App) -> u64 {
    let revision = client
        .world()
        .resource::<ClientSession>()
        .inventory
        .revision;
    client.world_mut().resource_mut::<SelectedInventorySlot>().0 = Some(0);
    send(client, ClientControl::HeldSlot(Some(0)));
    pulse(client, Some(0), revision);
    wait(host, &mut [client], |_, clients| {
        clients[0]
            .world()
            .resource::<ClientSession>()
            .consumption
            .is_some()
    });
    revision
}

fn pump(host: &mut App, client: &mut App, duration: Duration, held: Option<u64>) {
    let deadline = Instant::now() + duration;
    let mut next_pulse = Instant::now();
    while Instant::now() < deadline {
        if let Some(revision) = held
            && Instant::now() >= next_pulse
        {
            pulse(client, Some(0), revision);
            next_pulse = Instant::now() + Duration::from_millis(60);
        }
        host.update();
        client.update();
        assert!(client.world().resource::<ClientSession>().error.is_none());
        std::thread::sleep(Duration::from_millis(4));
    }
}

fn bag(host: &App, profile: &PlayerProfile) -> Inventory {
    host.world()
        .resource::<EditJournal>()
        .gameplay()
        .inventories[&profile.uuid]
        .clone()
}

fn cancelled(host: &mut App, client: &mut App) {
    // An explicit interruption must arrive before the 400 ms missing-pulse timeout.
    let deadline = Instant::now() + Duration::from_millis(300);
    while client
        .world()
        .resource::<ClientSession>()
        .consumption
        .is_some()
    {
        host.update();
        client.update();
        assert!(
            Instant::now() < deadline,
            "explicit consumption cancellation waited for a pulse timeout"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn consumption_pulses_cannot_skip_time_and_release_timeout_slot_and_revision_cancel() {
    let (mut host, mut client, profile) = fixture(GameMode::Survival);
    let before = bag(&host, &profile);
    let revision = start(&mut host, &mut client);
    for _ in 0..32 {
        pulse(&mut client, Some(0), revision);
    }
    pump(
        &mut host,
        &mut client,
        Duration::from_millis(180),
        Some(revision),
    );
    assert_eq!(bag(&host, &profile), before);
    assert!(
        client
            .world()
            .resource::<ClientSession>()
            .consumption
            .is_some()
    );
    assert!(
        client
            .world()
            .resource::<ClientSession>()
            .stats
            .survival
            .hunger
            <= 20.0
    );

    // The same cancellation packet is used when releasing use or opening a menu.
    pulse(&mut client, None, revision);
    cancelled(&mut host, &mut client);
    pump(
        &mut host,
        &mut client,
        Duration::from_secs_f32(terra_core::food(FOOD).unwrap().eat_seconds + 0.1),
        None,
    );
    assert_eq!(bag(&host, &profile), before);

    start(&mut host, &mut client);
    pump(&mut host, &mut client, Duration::from_millis(500), None);
    cancelled(&mut host, &mut client);
    assert_eq!(bag(&host, &profile), before);

    start(&mut host, &mut client);
    client.world_mut().resource_mut::<SelectedInventorySlot>().0 = Some(1);
    send(&mut client, ClientControl::HeldSlot(Some(1)));
    cancelled(&mut host, &mut client);
    assert_eq!(bag(&host, &profile), before);

    start(&mut host, &mut client);
    pulse(&mut client, Some(0), revision.saturating_sub(1));
    cancelled(&mut host, &mut client);
    assert_eq!(bag(&host, &profile), before);

    start(&mut host, &mut client);
    host.world_mut()
        .resource_mut::<EditJournal>()
        .gameplay_mut()
        .inventories
        .get_mut(&profile.uuid)
        .unwrap()
        .insert(ItemKind::Stick, 1);
    cancelled(&mut host, &mut client);
    let changed = bag(&host, &profile);
    pump(
        &mut host,
        &mut client,
        Duration::from_millis(500),
        Some(revision),
    );
    assert_eq!(bag(&host, &profile), changed);
    assert_eq!(changed.count(FOOD), 3);
    assert!(
        client
            .world()
            .resource::<ClientSession>()
            .consumption
            .is_none()
    );
}

#[test]
fn consumption_completed_hold_spends_once_and_replayed_revision_cannot_feed_again() {
    let (mut host, mut client, profile) = fixture(GameMode::Survival);
    let inventory_response = client
        .world()
        .resource::<ClientSession>()
        .inventory_response;
    let revision = start(&mut host, &mut client);
    let deadline =
        Instant::now() + Duration::from_secs_f32(terra_core::food(FOOD).unwrap().eat_seconds + 2.0);
    while bag(&host, &profile).count(FOOD) == 3 {
        pump(
            &mut host,
            &mut client,
            Duration::from_millis(80),
            Some(revision),
        );
        assert!(
            Instant::now() < deadline,
            "survival consumption did not finish"
        );
    }
    wait(&mut host, &mut [&mut client], |_, clients| {
        let session = clients[0].world().resource::<ClientSession>();
        session.inventory.count(FOOD) == 2 && session.stats.survival.hunger > 37.0
    });
    let after = bag(&host, &profile);
    assert_eq!(
        client
            .world()
            .resource::<ClientSession>()
            .inventory_response,
        inventory_response
    );
    assert_eq!(
        client
            .world()
            .resource::<ClientSession>()
            .consumption_completed,
        1
    );
    assert!(after.revision > revision);
    pump(
        &mut host,
        &mut client,
        Duration::from_secs_f32(terra_core::food(FOOD).unwrap().eat_seconds + 0.1),
        Some(revision),
    );
    assert_eq!(bag(&host, &profile), after);
    assert!(
        client
            .world()
            .resource::<ClientSession>()
            .consumption
            .is_none()
    );
}

#[test]
fn consumption_zero_hunger_can_finish_while_starvation_damage_continues() {
    let (mut host, mut client, profile) = fixture_with_hunger(GameMode::Survival, 0.0);
    let revision = start(&mut host, &mut client);
    let deadline =
        Instant::now() + Duration::from_secs_f32(terra_core::food(FOOD).unwrap().eat_seconds + 2.0);
    while bag(&host, &profile).count(FOOD) == 3 {
        pump(
            &mut host,
            &mut client,
            Duration::from_millis(80),
            Some(revision),
        );
        assert!(
            Instant::now() < deadline,
            "starvation must not restart the eating timer"
        );
    }
    wait(&mut host, &mut [&mut client], |_, clients| {
        let session = clients[0].world().resource::<ClientSession>();
        session.inventory.count(FOOD) == 2 && session.stats.survival.hunger > 17.0
    });
    let vitals = client.world().resource::<ClientSession>().stats.survival;
    assert!(vitals.health > 0.0 && vitals.health < 70.0);
    assert_eq!(
        vitals.last_damage,
        Some(terra_core::DamageCause::Starvation)
    );
    assert_eq!(bag(&host, &profile).count(FOOD), 2);
}

#[test]
fn consumption_damage_cancels_before_inventory_or_nutrition_commit() {
    use terra_core::creatures::*;
    let (mut host, mut client, profile) = fixture(GameMode::Survival);
    let before = bag(&host, &profile);
    let revision = start(&mut host, &mut client);
    let position = client
        .world()
        .resource::<ClientSession>()
        .confirmed_state()
        .unwrap()
        .position;
    let home = [
        (position[0] as i64).div_euclid(32),
        (position[2] as i64).div_euclid(32),
    ];
    let id = creature_id(home, 0).unwrap();
    {
        let mut journal = host.world_mut().resource_mut::<EditJournal>();
        let population = &mut journal.gameplay_mut().creatures;
        population.explored.insert(home);
        population.animals.insert(
            id,
            Creature {
                id,
                home,
                kind: CreatureKind::Bear,
                position: [
                    f64::from(position[0]),
                    f64::from(position[1] - terra_physics::character::PLAYER_HALF_HEIGHT),
                    f64::from(position[2]) - 1.35,
                ],
                velocity: [0.0; 3],
                yaw: 0.0,
                health: CreatureKind::Bear.health(),
                behavior: CreatureBehavior::Rest,
                timer: 60.0,
                age: 0.0,
                variant: 0,
                needs: CreatureNeeds::full(CreatureKind::Bear),
            },
        );
    }
    // Keep pulses live past one full meal: repeated attacks must reset progress,
    // so a missing-pulse timeout cannot make the damage assertion pass.
    pump(
        &mut host,
        &mut client,
        Duration::from_secs_f32(terra_core::food(FOOD).unwrap().eat_seconds + 0.15),
        Some(revision),
    );
    let vitals = client.world().resource::<ClientSession>().stats.survival;
    assert!(vitals.health > 0.0 && vitals.health < 70.0);
    assert_eq!(vitals.last_damage, Some(terra_core::DamageCause::Animal));
    assert_eq!(bag(&host, &profile), before);
    pulse(&mut client, None, revision);
    cancelled(&mut host, &mut client);
    assert!(
        client
            .world()
            .resource::<ClientSession>()
            .stats
            .survival
            .hunger
            <= 20.0
    );
}

#[test]
fn consumption_reconnect_discards_partial_progress_and_requires_full_hold_again() {
    let (mut host, mut client, profile) = fixture(GameMode::Survival);
    let before = bag(&host, &profile);
    let revision = start(&mut host, &mut client);
    pump(
        &mut host,
        &mut client,
        Duration::from_millis(250),
        Some(revision),
    );
    let link = client
        .world()
        .resource::<ClientSession>()
        .connection
        .unwrap();
    client
        .world_mut()
        .trigger(lightyear::prelude::Disconnect { entity: link });
    let deadline = Instant::now() + Duration::from_secs(5);
    while !host
        .world()
        .resource::<server::ConnectedProfiles>()
        .0
        .is_empty()
    {
        host.update();
        client.update();
        std::thread::sleep(Duration::from_millis(4));
        assert!(Instant::now() < deadline);
    }
    let address = host
        .world()
        .resource::<server::ServerRuntime>()
        .address
        .unwrap();
    client = profile_client(address, profile.clone());
    client.world_mut().resource_mut::<SelectedInventorySlot>().0 = Some(0);
    wait(&mut host, &mut [&mut client], |_, clients| {
        let session = clients[0].world().resource::<ClientSession>();
        session.ready && session.inventory == before && session.confirmed_state().is_some()
    });
    assert!(
        client
            .world()
            .resource::<ClientSession>()
            .consumption
            .is_none()
    );
    pump(&mut host, &mut client, Duration::from_millis(450), None);
    assert_eq!(bag(&host, &profile), before);
    let revision = start(&mut host, &mut client);
    assert!(
        client
            .world()
            .resource::<ClientSession>()
            .consumption
            .unwrap()
            .progress
            < 0.25
    );
    pump(
        &mut host,
        &mut client,
        Duration::from_millis(250),
        Some(revision),
    );
    assert_eq!(bag(&host, &profile), before);
    pulse(&mut client, None, revision);
    cancelled(&mut host, &mut client);
}

#[test]
fn consumption_creative_completes_without_spending_and_spectator_cannot_start() {
    let (mut host, mut client, profile) = fixture(GameMode::Creative);
    let before = bag(&host, &profile);
    let revision = start(&mut host, &mut client);
    let response = client
        .world()
        .resource::<ClientSession>()
        .consumption_completed;
    let deadline =
        Instant::now() + Duration::from_secs_f32(terra_core::food(FOOD).unwrap().eat_seconds + 2.0);
    while client
        .world()
        .resource::<ClientSession>()
        .consumption_completed
        == response
    {
        pump(
            &mut host,
            &mut client,
            Duration::from_millis(80),
            Some(revision),
        );
        assert!(
            Instant::now() < deadline,
            "creative consumption did not finish"
        );
    }
    pulse(&mut client, None, revision);
    cancelled(&mut host, &mut client);
    assert_eq!(bag(&host, &profile), before);
    assert_eq!(
        client.world().resource::<ClientSession>().stats.survival,
        terra_core::SurvivalVitals::default()
    );
    let (mut host, mut client, profile) = fixture(GameMode::Spectator);
    let before = bag(&host, &profile);
    send(&mut client, ClientControl::HeldSlot(Some(0)));
    pulse(&mut client, Some(0), before.revision);
    pump(
        &mut host,
        &mut client,
        Duration::from_millis(500),
        Some(before.revision),
    );
    assert!(
        client
            .world()
            .resource::<ClientSession>()
            .consumption
            .is_none()
    );
    assert_eq!(bag(&host, &profile), before);
}
