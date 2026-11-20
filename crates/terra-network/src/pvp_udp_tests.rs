use super::*;
use crate::protocol::session::ClientControl;
use terra_core::{
    Config, DamageCause, GameMode, Inventory, ItemKind, ItemStack, PlayerLocation, PlayerProfile,
    PlayerStats, ServerConfig, SurvivalVitals,
};
use terra_world::edits::EditJournal;

#[test]
fn real_udp_pvp_server_policy_overrides_both_welcomes_and_controls_authoritative_player_damage() {
    for enabled in [false, true] {
        let mut host = server();
        {
            // Parse the same top-level setting an administrator writes in server.toml.
            let policy = ServerConfig::from_toml(&format!("pvp = {enabled}")).unwrap();
            let mut config = host.world_mut().resource_mut::<GameConfig>();
            config.server.pvp = policy.pvp;
            config.server.game_mode = GameMode::Survival;
            config.server.natural_mob_limit = Some(0);
            config.world.game_rules.pvp = !enabled;
        }
        let profiles = [PlayerProfile::default(), PlayerProfile::default()];
        let locations = [
            PlayerLocation {
                position: [8.5, 4.92, 10.5],
                look: [0.0; 2],
                flying: false,
            },
            PlayerLocation {
                position: [8.5, 4.92, 7.5],
                look: [0.0; 2],
                flying: false,
            },
        ];
        let marker = ItemKind::Block(16);
        {
            let mut journal = host.world_mut().resource_mut::<EditJournal>();
            // A real dry floor and open loaded room avoid fall, water, mob and spawn
            // recovery damage while preserving ordinary authoritative player physics.
            for x in 5..=11 {
                for z in 4..=13 {
                    for y in 3..=8 {
                        journal
                            .record([x, y, z], if y == 3 { 3 } else { 0 })
                            .unwrap();
                    }
                }
            }
            for (profile, location) in profiles.iter().zip(locations) {
                journal
                    .update_player_location(profile.uuid, location)
                    .unwrap();
                journal.update_player_stats(
                    profile.uuid,
                    PlayerStats {
                        survival: SurvivalVitals {
                            health: 70.0,
                            hunger: 20.0,
                            ..Default::default()
                        },
                        game_mode: Some(GameMode::Survival),
                        ..Default::default()
                    },
                );
            }
            let mut bag = Inventory::default();
            bag.creative_set(
                1,
                ItemStack {
                    item: marker,
                    count: 1,
                },
            );
            journal
                .gameplay_mut()
                .inventories
                .insert(profiles[0].uuid, bag);
        }
        let address = host
            .world()
            .resource::<server::ServerRuntime>()
            .address
            .unwrap();
        let mut a = profile_client(address, profiles[0].clone());
        let mut b = profile_client(address, profiles[1].clone());
        wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
            clients.iter().all(|client| {
                let session = client.world().resource::<client::ClientSession>();
                session.ready
                    && session
                        .confirmed_state()
                        .is_some_and(|state| state.grounded)
            })
        });
        for replica in [&a, &b] {
            assert_eq!(
                replica
                    .world()
                    .resource::<GameConfig>()
                    .world
                    .game_rules
                    .pvp,
                enabled
            );
            assert_eq!(
                replica
                    .world()
                    .resource::<client::ClientSession>()
                    .game_mode,
                GameMode::Survival
            );
        }
        // Exercise continuing effective GameRules publication as well as Welcome.
        for policy in [!enabled, enabled] {
            host.world_mut().resource_mut::<GameConfig>().server.pvp = Some(policy);
            wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
                clients.iter().all(|client| {
                    client.world().resource::<GameConfig>().world.game_rules.pvp == policy
                })
            });
        }
        let attacker = a
            .world()
            .resource::<client::ClientSession>()
            .player
            .unwrap();
        let victim = b
            .world()
            .resource::<client::ClientSession>()
            .player
            .unwrap();
        // Creature/PvP cooldowns use wall-clock time. Keep pumping both actual
        // connections until initial cooldown and any spawn protection have elapsed.
        let ready_at = Instant::now() + Duration::from_millis(500);
        wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
            Instant::now() >= ready_at
                && clients.iter().all(|client| {
                    let session = client.world().resource::<client::ClientSession>();
                    session
                        .confirmed_state()
                        .is_some_and(|state| state.grounded && state.survival.health == 70.0)
                })
        });
        let shooter = a
            .world()
            .resource::<client::ClientSession>()
            .confirmed_state()
            .unwrap()
            .position;
        let target = b
            .world()
            .resource::<client::ClientSession>()
            .confirmed_state()
            .unwrap()
            .position;
        assert!((shooter[0] - target[0]).abs() < 0.1);
        assert!((shooter[1] - target[1]).abs() < 0.1);
        assert!((shooter[2] - target[2] - 3.0).abs() < 0.1);
        assert!(
            a.world()
                .resource::<client::ReplicatedPlayers>()
                .players
                .contains_key(&victim)
        );
        // Send even when disabled: forged client UI state cannot bypass policy.
        send(
            &mut a,
            ClientControl::AttackPlayer {
                player: victim,
                look: [0.0; 2],
            },
        );
        // The reliable ordered held-item marker proves the preceding AttackPlayer
        // packet reached authority even when damage is deliberately rejected.
        a.world_mut()
            .resource_mut::<client::SelectedInventorySlot>()
            .0 = Some(1);
        send(&mut a, ClientControl::HeldSlot(Some(1)));
        let expected = if enabled { 69.0 } else { 70.0 };
        wait(&mut host, &mut [&mut a, &mut b], |host, clients| {
            let victim_client = clients[1].world().resource::<client::ClientSession>();
            let attacker_view = clients[1].world().resource::<client::ReplicatedPlayers>();
            let victim_view = clients[0].world().resource::<client::ReplicatedPlayers>();
            attacker_view
                .players
                .get(&attacker)
                .is_some_and(|track| track.current.held_item == Some(marker))
                && victim_view
                    .players
                    .get(&victim)
                    .is_some_and(|track| track.current.survival.health == expected)
                && victim_client
                    .confirmed_state()
                    .is_some_and(|state| state.survival.health == expected)
                && victim_client.stats.survival.health == expected
                && host
                    .world()
                    .resource::<EditJournal>()
                    .gameplay()
                    .player_stats[&profiles[1].uuid]
                    .survival
                    .health
                    == expected
        });
        let vitals = b.world().resource::<client::ClientSession>().stats.survival;
        assert_eq!(vitals.last_damage, enabled.then_some(DamageCause::Player));
        assert_eq!(
            host.world().resource::<GameConfig>().world.game_rules.pvp,
            !enabled,
            "live server policy does not rewrite the saved rule"
        );
    }
}
