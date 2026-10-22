use super::*;
use protocol::{
    inventory::{InventoryAction, InventoryCommand, InventoryOutcome},
    session::*,
};
use terra_core::{
    GameMode, ItemKind, PlayerProfile, PlayerStats, ResourcePropKind, SurvivalVitals,
};
use terra_world::edits::EditJournal;

fn survival(stats: PlayerStats) -> (App, App, PlayerProfile) {
    let mut host = server();
    host.world_mut()
        .resource_mut::<GameConfig>()
        .server
        .game_mode = GameMode::Survival;
    let profile = PlayerProfile::default();
    {
        let mut journal = host.world_mut().resource_mut::<EditJournal>();
        journal.update_player_stats(profile.uuid, stats);
        journal
            .gameplay_mut()
            .inventories
            .entry(profile.uuid)
            .or_default()
            .insert(ItemKind::Prop(ResourcePropKind::Mushroom), 3);
    }
    let address = host
        .world()
        .resource::<server::ServerRuntime>()
        .address
        .unwrap();
    let mut a = profile_client(address, profile.clone());
    wait(&mut host, &mut [&mut a], |_, clients| {
        let session = clients[0].world().resource::<client::ClientSession>();
        session.ready
            && session.stats.blocks_broken == stats.blocks_broken
            && session
                .inventory
                .count(ItemKind::Prop(ResourcePropKind::Mushroom))
                == 3
            && session
                .confirmed_state()
                .is_some_and(|s| s.grounded || stats.survival.dead())
    });
    (host, a, profile)
}
fn frame(host: &mut App, client: &mut App) {
    host.update();
    client.update();
    std::thread::sleep(Duration::from_millis(1));
}

#[test]
fn jellyfish_contact_gives_ten_second_poison_that_expires_after_the_contact_ends() {
    use terra_core::{EffectKind, creatures::*};
    let (mut host, mut client, profile) = survival(PlayerStats {
        survival: SurvivalVitals {
            health: 80.0,
            hunger: 60.0,
            ..Default::default()
        },
        ..Default::default()
    });
    let position = client
        .world()
        .resource::<client::ClientSession>()
        .confirmed_state()
        .unwrap()
        .position
        .map(f64::from);
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
                kind: CreatureKind::MoonJelly,
                home,
                position: [position[0], position[1] - 0.3, position[2]],
                velocity: [0.0; 3],
                yaw: 0.0,
                health: CreatureKind::MoonJelly.health(),
                behavior: CreatureBehavior::Rest,
                timer: 60.0,
                age: 0.0,
                variant: 0,
                needs: CreatureNeeds::full(CreatureKind::MoonJelly),
            },
        );
    }
    wait(&mut host, &mut [&mut client], |host, clients| {
        host.world()
            .resource::<EditJournal>()
            .gameplay()
            .player_stats[&profile.uuid]
            .survival
            .effects
            .contains(EffectKind::Poison)
            && clients[0]
                .world()
                .resource::<client::ClientSession>()
                .stats
                .survival
                .effects
                .timed(EffectKind::Poison)
                .is_some_and(|effect| {
                    effect.remaining_seconds > 9.0 && effect.remaining_seconds <= 10.0
                })
    });
    host.world_mut()
        .resource_mut::<EditJournal>()
        .gameplay_mut()
        .creatures
        .animals
        .remove(&id);
    wait(&mut host, &mut [&mut client], |_, clients| {
        !clients[0]
            .world()
            .resource::<client::ClientSession>()
            .stats
            .survival
            .effects
            .contains(EffectKind::Poison)
    });
    let stats = client.world().resource::<client::ClientSession>().stats;
    assert!(stats.survival.health < 80.0 && stats.survival.health > 0.0);
    assert_eq!(
        stats.survival.last_damage,
        Some(terra_core::DamageCause::Poison)
    );
    assert!(stats.survival.valid());
}

#[test]
fn bear_strikes_replicate_authoritative_death_once_and_do_not_hurt_creative() {
    use terra_core::creatures::*;
    for creative in [false, true] {
        let (mut host, mut a, profile) = survival(PlayerStats {
            survival: SurvivalVitals {
                health: 10.0,
                hunger: 60.0,
                ..default()
            },
            ..default()
        });
        if creative {
            host.world_mut()
                .resource_mut::<GameConfig>()
                .server
                .game_mode = GameMode::Creative;
        }
        let p = a
            .world()
            .resource::<client::ClientSession>()
            .confirmed_state()
            .unwrap()
            .position;
        let home = [(p[0] as i64).div_euclid(32), (p[2] as i64).div_euclid(32)];
        let id = creature_id(home, 0).unwrap();
        {
            let mut journal = host.world_mut().resource_mut::<EditJournal>();
            let population = &mut journal.gameplay_mut().creatures;
            population.explored.insert(home);
            population.animals.insert(
                id,
                Creature {
                    id,
                    kind: CreatureKind::Bear,
                    home,
                    position: [
                        f64::from(p[0]),
                        f64::from(p[1] - terra_physics::character::PLAYER_HALF_HEIGHT),
                        f64::from(p[2]) - 1.35,
                    ],
                    velocity: [0.0; 3],
                    yaw: 0.0,
                    health: 24.0,
                    behavior: CreatureBehavior::Rest,
                    timer: 60.0,
                    age: 0.0,
                    variant: 0,
                    needs: CreatureNeeds::full(CreatureKind::Bear),
                },
            );
        }
        wait(&mut host, &mut [&mut a], |_, clients| {
            clients[0]
                .world()
                .resource::<client::ReplicatedCreatures>()
                .creatures
                .get(&id)
                .is_some_and(|c| c.current.kind == CreatureKind::Bear)
        });
        if creative {
            for _ in 0..120 {
                frame(&mut host, &mut a);
            }
            assert_eq!(
                host.world()
                    .resource::<EditJournal>()
                    .gameplay()
                    .player_stats[&profile.uuid]
                    .survival
                    .health,
                100.0
            );
        } else {
            wait(&mut host, &mut [&mut a], |_, clients| {
                clients[0]
                    .world()
                    .resource::<client::ClientSession>()
                    .dead()
            });
            for _ in 0..30 {
                frame(&mut host, &mut a);
            }
            let stats = host
                .world()
                .resource::<EditJournal>()
                .gameplay()
                .player_stats[&profile.uuid];
            assert_eq!(stats.deaths, 1);
            assert_eq!(
                stats.survival.last_damage,
                Some(terra_core::DamageCause::Animal)
            );
        }
    }
}

#[test]
fn game_mode_is_per_player_permission_checked_and_survives_reconnect() {
    use client::{ChatCommand, ClientChat, ClientChatCommands};
    let mut host = server();
    let owner = PlayerProfile::default();
    let visitor = PlayerProfile::default();
    {
        let mut config = host.world_mut().resource_mut::<GameConfig>();
        config.server.game_mode = GameMode::Survival;
        let mut permissions = config.server.permissions_for(owner.uuid);
        permissions.game_mode = true;
        config
            .server
            .player_permissions
            .insert(owner.uuid, permissions);
    }
    let address = host
        .world()
        .resource::<server::ServerRuntime>()
        .address
        .unwrap();
    let mut a = profile_client(address, owner.clone());
    let mut b = profile_client(address, visitor);
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients
            .iter()
            .all(|c| c.world().resource::<client::ClientSession>().ready)
    });
    let inventory = a
        .world()
        .resource::<client::ClientSession>()
        .inventory
        .clone();
    b.world_mut()
        .resource_mut::<ClientChatCommands>()
        .0
        .push_back(ChatCommand::Send("/gamemode 1".into()));
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients[1]
            .world()
            .resource::<ClientChat>()
            .entries
            .iter()
            .any(|e| e.text.contains("game_mode permission"))
    });
    a.world_mut()
        .resource_mut::<ClientChatCommands>()
        .0
        .push_back(ChatCommand::Send("/gamemode creative".into()));
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients[0]
            .world()
            .resource::<client::ClientSession>()
            .game_mode
            == GameMode::Creative
    });
    assert!(a.world().resource::<client::ClientSession>().allow_flight);
    assert_eq!(
        a.world().resource::<client::ClientSession>().inventory,
        inventory
    );
    assert_eq!(
        b.world().resource::<client::ClientSession>().game_mode,
        GameMode::Survival
    );
    a.world_mut()
        .resource_mut::<ClientChatCommands>()
        .0
        .push_back(ChatCommand::Send("/gamemode 2".into()));
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients[0]
            .world()
            .resource::<client::ClientSession>()
            .game_mode
            == GameMode::Spectator
            && clients[1]
                .world()
                .resource::<client::ReplicatedPlayers>()
                .players
                .is_empty()
    });
    assert!(a.world().resource::<client::ClientSession>().allow_flight);
    assert_eq!(
        a.world().resource::<client::ClientSession>().inventory,
        inventory
    );
    assert_eq!(
        host.world()
            .resource::<EditJournal>()
            .gameplay()
            .player_stats[&owner.uuid]
            .game_mode,
        Some(GameMode::Spectator)
    );
    a.world_mut()
        .resource_mut::<ClientChatCommands>()
        .0
        .push_back(ChatCommand::Send("/gamemode 0".into()));
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients[0]
            .world()
            .resource::<client::ClientSession>()
            .game_mode
            == GameMode::Survival
    });
    assert!(!a.world().resource::<client::ClientSession>().allow_flight);
    assert_eq!(
        host.world()
            .resource::<EditJournal>()
            .gameplay()
            .player_stats[&owner.uuid]
            .game_mode,
        Some(GameMode::Survival)
    );
    // Old flight inputs cannot revoke the connection after the generation changes.
    {
        use lightyear::prelude::{MessageSender, client::Client};
        let world = a.world_mut();
        world
            .query_filtered::<&mut MessageSender<MovementInput>, With<Client>>()
            .single_mut(world)
            .unwrap()
            .send::<InputChannel>(MovementInput {
                generation: 1,
                sequence: 100,
                direction: [0.0, 1.0, 0.0],
                look: [0.0; 2],
                jump: false,
                sprint: false,
                auto_climb: false,
                flight_speed: Some(10.0),
            });
    }
    for _ in 0..30 {
        host.update();
        a.update();
        b.update();
    }
    assert!(
        a.world()
            .resource::<client::ClientSession>()
            .error
            .is_none()
    );
    a.world_mut()
        .resource_mut::<ClientChatCommands>()
        .0
        .push_back(ChatCommand::Send("/gamemode 1".into()));
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients[0]
            .world()
            .resource::<client::ClientSession>()
            .game_mode
            == GameMode::Creative
    });
    let link = a
        .world()
        .resource::<client::ClientSession>()
        .connection
        .unwrap();
    a.world_mut()
        .trigger(lightyear::prelude::Disconnect { entity: link });
    let deadline = Instant::now() + Duration::from_secs(5);
    while host.world().resource::<server::ConnectedProfiles>().0.len() != 1 {
        host.update();
        a.update();
        b.update();
        assert!(Instant::now() < deadline, "Disconnect did not finish");
        std::thread::sleep(Duration::from_millis(1));
    }
    let mut rejoined = profile_client(address, owner.clone());
    wait(&mut host, &mut [&mut rejoined, &mut b], |_, clients| {
        clients[0].world().resource::<client::ClientSession>().ready
    });
    assert_eq!(
        rejoined
            .world()
            .resource::<client::ClientSession>()
            .game_mode,
        GameMode::Creative
    );
    assert_eq!(
        rejoined
            .world()
            .resource::<client::ClientSession>()
            .inventory,
        inventory
    );
}
fn input(client: &mut App, sequence: u64, jump: bool, sprint: bool) {
    use lightyear::prelude::MessageSender;
    let world = client.world_mut();
    world.query_filtered::<&mut MessageSender<MovementInput>, With<lightyear::prelude::client::Client>>()
        .single_mut(world).unwrap().send::<InputChannel>(MovementInput {
            generation: 0, sequence, direction: [1.0, 0.0, 0.0], look: [0.0; 2], jump, sprint,
            auto_climb: false, flight_speed: None,
        });
}

#[test]
fn legacy_instant_eat_packets_cannot_bypass_held_consumption() {
    let stats = PlayerStats {
        blocks_broken: 71,
        survival: SurvivalVitals {
            health: 78.0,
            hunger: 40.0,
            ..default()
        },
        ..default()
    };
    let (mut host, mut a, profile) = survival(stats);
    let revision = a
        .world()
        .resource::<client::ClientSession>()
        .inventory
        .revision;
    let command = InventoryCommand {
        revision,
        action: InventoryAction::Eat { slot: 0 },
    };
    a.world_mut()
        .write_message(client::InventoryRequest(command));
    wait(&mut host, &mut [&mut a], |_, clients| {
        let s = clients[0].world().resource::<client::ClientSession>();
        s.last_inventory == Some(InventoryOutcome::Invalid)
    });
    let before = host
        .world()
        .resource::<EditJournal>()
        .gameplay()
        .inventories[&profile.uuid]
        .clone();
    assert_eq!(before.slot(0).unwrap().count, 3);
    assert!(
        a.world()
            .resource::<client::ClientSession>()
            .stats
            .survival
            .hunger
            <= 40.0
    );
    a.world_mut()
        .resource_mut::<client::ClientSession>()
        .last_inventory = None;
    a.world_mut()
        .write_message(client::InventoryRequest(command));
    wait(&mut host, &mut [&mut a], |_, clients| {
        clients[0]
            .world()
            .resource::<client::ClientSession>()
            .last_inventory
            == Some(InventoryOutcome::Invalid)
    });
    assert_eq!(
        host.world()
            .resource::<EditJournal>()
            .gameplay()
            .inventories[&profile.uuid],
        before
    );
    assert_eq!(
        a.world()
            .resource::<client::ClientSession>()
            .stats
            .blocks_broken,
        71
    );
}

#[test]
fn two_survival_profiles_keep_their_own_vitals_inventory_and_counters() {
    let (mut host, mut a, first) = survival(PlayerStats {
        blocks_broken: 67,
        survival: SurvivalVitals {
            health: 51.0,
            hunger: 40.0,
            ..default()
        },
        ..default()
    });
    let second = PlayerProfile::default();
    {
        let mut journal = host.world_mut().resource_mut::<EditJournal>();
        journal.update_player_stats(
            second.uuid,
            PlayerStats {
                blocks_broken: 91,
                survival: SurvivalVitals {
                    health: 34.0,
                    hunger: 73.0,
                    ..default()
                },
                ..default()
            },
        );
        journal
            .gameplay_mut()
            .inventories
            .entry(second.uuid)
            .or_default()
            .insert(ItemKind::Prop(ResourcePropKind::Mushroom), 9);
    }
    let address = host
        .world()
        .resource::<server::ServerRuntime>()
        .address
        .unwrap();
    let mut b = profile_client(address, second.clone());
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        let s = clients[1].world().resource::<client::ClientSession>();
        s.ready
            && s.stats.blocks_broken == 91
            && s.inventory
                .count(ItemKind::Prop(ResourcePropKind::Mushroom))
                == 9
    });
    a.world_mut()
        .resource_mut::<client::SelectedInventorySlot>()
        .0 = Some(0);
    let started = Instant::now();
    let mut pulse = Instant::now();
    while a
        .world()
        .resource::<client::ClientSession>()
        .inventory
        .count(ItemKind::Prop(ResourcePropKind::Mushroom))
        == 3
    {
        assert!(started.elapsed() < Duration::from_secs(5));
        if Instant::now() >= pulse {
            a.world_mut()
                .write_message(client::ConsumptionRequest(Some(0)));
            pulse = Instant::now() + Duration::from_millis(80);
        }
        host.update();
        a.update();
        b.update();
        std::thread::sleep(Duration::from_millis(2));
    }
    a.world_mut()
        .write_message(client::ConsumptionRequest(None));
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        let s = clients[0].world().resource::<client::ClientSession>();
        s.stats.survival.hunger > 57.0
            && s.inventory
                .count(ItemKind::Prop(ResourcePropKind::Mushroom))
                == 2
    });
    let s = b.world().resource::<client::ClientSession>();
    assert_eq!(s.uuid, Some(second.uuid));
    assert_eq!(s.stats.survival.health, 34.0);
    assert!(s.stats.survival.hunger > 72.0 && s.stats.survival.hunger <= 73.0);
    assert_eq!(s.stats.blocks_broken, 91);
    assert_eq!(
        s.inventory
            .count(ItemKind::Prop(ResourcePropKind::Mushroom)),
        9
    );
    let journal = host.world().resource::<EditJournal>();
    assert_eq!(
        journal.gameplay().player_stats[&first.uuid].blocks_broken,
        67
    );
    assert_eq!(
        journal.gameplay().player_stats[&second.uuid].blocks_broken,
        91
    );
}

#[test]
fn exhausted_movement_packets_cannot_sprint_or_jump_past_server_rules() {
    let stats = PlayerStats {
        blocks_broken: 19,
        survival: SurvivalVitals {
            stamina: 0.0,
            exhausted: true,
            stamina_delay: 10.0,
            hunger: 70.0,
            ..default()
        },
        ..default()
    };
    let (mut host, mut a, _) = survival(stats);
    for sequence in 1..=30 {
        input(&mut a, sequence, true, true);
        frame(&mut host, &mut a);
    }
    wait(&mut host, &mut [&mut a], |_, clients| {
        clients[0]
            .world()
            .resource::<client::ClientSession>()
            .confirmed_state()
            .is_some_and(|s| s.ack == 30)
    });
    let confirmed = a
        .world()
        .resource::<client::ClientSession>()
        .confirmed_state()
        .unwrap();
    assert!(confirmed.velocity[0] <= terra_physics::character::WALK_SPEED + 0.01);
    assert!(confirmed.grounded && confirmed.survival.exhausted);
}

#[test]
fn dead_reconnect_and_respawn_keep_inventory_counters_and_uuid_but_reset_prediction_generation() {
    let stats = PlayerStats {
        blocks_broken: 83,
        deaths: 4,
        survival: SurvivalVitals {
            health: 0.0,
            last_damage: Some(terra_core::DamageCause::Fall),
            ..default()
        },
        ..default()
    };
    let (mut host, mut a, profile) = survival(stats);
    wait(&mut host, &mut [&mut a], |_, clients| {
        let session = clients[0].world().resource::<client::ClientSession>();
        session.dead() && session.confirmed_state().is_some()
    });
    assert_eq!(
        a.world().resource::<client::ClientSession>().stats.deaths,
        4
    );
    let link = a
        .world()
        .resource::<client::ClientSession>()
        .connection
        .unwrap();
    a.world_mut()
        .trigger(lightyear::prelude::Disconnect { entity: link });
    let deadline = Instant::now() + Duration::from_secs(5);
    while !host
        .world()
        .resource::<server::ConnectedProfiles>()
        .0
        .is_empty()
    {
        frame(&mut host, &mut a);
        assert!(Instant::now() < deadline);
    }
    let address = host
        .world()
        .resource::<server::ServerRuntime>()
        .address
        .unwrap();
    a = profile_client(address, profile.clone());
    wait(&mut host, &mut [&mut a], |_, clients| {
        let session = clients[0].world().resource::<client::ClientSession>();
        // Welcome, vitals and the private gameplay inventory are separate
        // reliable messages; do not start respawning before all are restored.
        session.ready
            && session.dead()
            && session.confirmed_state().is_some()
            && session.stats.blocks_broken == 83
            && session.stats.deaths == 4
            && session.inventory.slot(0).is_some_and(|stack| {
                stack.item == ItemKind::Prop(ResourcePropKind::Mushroom) && stack.count == 3
            })
    });
    let position = a
        .world()
        .resource::<client::ClientSession>()
        .confirmed_state()
        .unwrap()
        .position;
    a.world_mut()
        .spawn(terra_physics::character::body_bundle(Vec3::from_array(
            position,
        )));
    a.world_mut().write_message(client::RespawnRequest);
    wait(&mut host, &mut [&mut a], |_, clients| {
        let s = clients[0].world().resource::<client::ClientSession>();
        !s.dead()
            && !s.teleporting
            && s.confirmed_state()
                .is_some_and(|p| p.generation == 1 && p.grounded)
    });
    let s = a.world().resource::<client::ClientSession>();
    assert_eq!(s.stats.deaths, 4);
    assert_eq!(s.stats.blocks_broken, 83);
    assert_eq!(s.inventory.slot(0).unwrap().count, 3);
    assert_eq!(s.uuid, Some(profile.uuid));
    send(&mut a, ClientControl::Respawn { generation: 0 });
    for _ in 0..20 {
        frame(&mut host, &mut a);
    }
    assert_eq!(
        a.world()
            .resource::<client::ClientSession>()
            .confirmed_state()
            .unwrap()
            .generation,
        1
    );
    let journal = host.world().resource::<EditJournal>();
    assert_eq!(journal.gameplay().player_stats[&profile.uuid].deaths, 4);
}

#[test]
fn authoritative_fall_death_is_counted_once_and_creative_ignores_it() {
    for creative in [false, true] {
        let (mut host, mut a, profile) = survival(PlayerStats {
            blocks_broken: 31,
            ..default()
        });
        if creative {
            host.world_mut()
                .resource_mut::<GameConfig>()
                .server
                .game_mode = GameMode::Creative;
        }
        let entity = host
            .world_mut()
            .query_filtered::<Entity, With<terra_physics::character::CharacterBody>>()
            .single(host.world())
            .unwrap();
        host.world_mut()
            .get_mut::<Transform>(entity)
            .unwrap()
            .translation
            .y += 25.0;
        host.world_mut()
            .get_mut::<terra_physics::character::CharacterGrounded>(entity)
            .unwrap()
            .0 = false;
        if creative {
            for _ in 0..240 {
                frame(&mut host, &mut a);
            }
            assert_eq!(
                a.world()
                    .resource::<client::ClientSession>()
                    .stats
                    .survival
                    .health,
                100.0
            );
            assert_eq!(
                host.world()
                    .resource::<EditJournal>()
                    .gameplay()
                    .player_stats[&profile.uuid]
                    .deaths,
                0
            );
        } else {
            wait(&mut host, &mut [&mut a], |_, clients| {
                clients[0]
                    .world()
                    .resource::<client::ClientSession>()
                    .dead()
            });
            for _ in 0..30 {
                frame(&mut host, &mut a);
            }
            let stats = host
                .world()
                .resource::<EditJournal>()
                .gameplay()
                .player_stats[&profile.uuid];
            assert_eq!(stats.deaths, 1);
            assert_eq!(
                stats.survival.last_damage,
                Some(terra_core::DamageCause::Fall)
            );
        }
    }
}

#[test]
fn diving_uses_live_water_at_the_body_and_causes_authoritative_drowning() {
    use lightyear::prelude::MessageSender;
    let initial = PlayerStats {
        blocks_broken: 27,
        survival: SurvivalVitals {
            oxygen: 1.0,
            hunger: 60.0,
            ..default()
        },
        ..default()
    };
    let (mut host, mut a, _) = survival(initial);
    let position = a
        .world()
        .resource::<client::ClientSession>()
        .confirmed_state()
        .unwrap()
        .position;
    // Change the controlled fixture's world contract before inserting a small full water column.
    host.world_mut().resource_mut::<GameConfig>().world.water = true;
    let config = host.world().resource::<GameConfig>().world.clone();
    host.world_mut()
        .insert_resource(terra_world::streaming::WorldTerrain(
            terra_world::TerraGenerator::new(config).unwrap(),
        ));
    let x = position[0].floor() as i64;
    let z = position[2].floor() as i64;
    let bottom = (position[1] - terra_physics::character::PLAYER_HALF_HEIGHT).floor() as i64;
    for dx in -1..=1 {
        for dz in -1..=1 {
            for y in bottom..=bottom + 4 {
                host.world_mut()
                    .resource_mut::<EditJournal>()
                    .set_fluid(
                        [x + dx, y, z + dz],
                        terra_world::water::FluidCell {
                            amount: 8,
                            source: true,
                            flow: [0; 2],
                        },
                    )
                    .unwrap();
            }
        }
    }
    let deadline = Instant::now() + Duration::from_secs(15);
    for sequence in 1..=3000 {
        let world = a.world_mut();
        world.query_filtered::<&mut MessageSender<MovementInput>, With<lightyear::prelude::client::Client>>()
            .single_mut(world).unwrap().send::<InputChannel>(MovementInput {
                generation: 0, sequence, direction: [0.0, -1.0, 0.0], look: [0.0; 2],
                jump: false, sprint: false, auto_climb: false, flight_speed: None,
            });
        frame(&mut host, &mut a);
        let v = a.world().resource::<client::ClientSession>().stats.survival;
        if v.oxygen < 0.1 && v.health < 99.0 {
            break;
        }
        assert!(
            Instant::now() < deadline && sequence < 3000,
            "dive failed: {v:?}"
        );
    }
    assert_eq!(
        a.world()
            .resource::<client::ClientSession>()
            .stats
            .survival
            .last_damage,
        Some(terra_core::DamageCause::Drowning)
    );
}
