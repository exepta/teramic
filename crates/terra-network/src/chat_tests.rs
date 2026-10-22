use super::*;
#[test]
fn gamerules_replicate_to_two_clients_and_console_works_without_a_player_actor() {
    let (mut host, mut a, mut b) = pair();
    for (name, value, console) in [
        ("game_time_speed", 0.0, false),
        ("season_swap_interval", 1.0, true),
        ("sleep_percentage", 50.0, false),
        ("game_time_speed", 2.0, true),
    ] {
        if console {
            assert!(
                host.world_mut()
                    .resource_mut::<server::ServerConsole>()
                    .submit(format!("gamerule {name} {value}"))
            );
        } else {
            command(
                &mut a,
                ChatCommand::Send(format!("/gamerule {name} {value}")),
            );
        }
        let rule = terra_core::GameRule::lookup(name).unwrap();
        wait(&mut host, &mut [&mut a, &mut b], |host, clients| {
            rule.value(&host.world().resource::<GameConfig>().world) == value
                && clients.iter().all(|client| {
                    rule.value(&client.world().resource::<GameConfig>().world) == value
                })
        });
        assert!(
            host.world()
                .resource::<terra_world::edits::EditJournal>()
                .world_rules
                .is_some()
        );
        assert!(a.world().resource::<terra_core::WorldClock>().valid());
    }
    // A survival peer cannot elevate its rules through ordinary chat.
    host.world_mut()
        .resource_mut::<GameConfig>()
        .server
        .game_mode = terra_core::GameMode::Survival;
    command(
        &mut a,
        ChatCommand::Send("/gamerule sleep_percentage 0".into()),
    );
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients[0]
            .world()
            .resource::<ClientChat>()
            .entries
            .iter()
            .any(|e| e.text.contains("requires Creative"))
    });
    assert_eq!(
        host.world()
            .resource::<GameConfig>()
            .world
            .game_rules
            .sleep_percentage,
        50.0
    );
}
#[test]
fn season_commands_update_and_replicate_the_saved_clock_in_chat_and_console() {
    let (mut host, mut a, mut b) = pair();
    host.world_mut()
        .resource_mut::<GameConfig>()
        .world
        .time
        .paused = true;
    for (text, season, console) in [
        ("/season set winter", terra_core::Season::Winter, false),
        ("season set summer", terra_core::Season::Summer, true),
    ] {
        if console {
            assert!(
                host.world_mut()
                    .resource_mut::<crate::server::ServerConsole>()
                    .submit(text.into())
            );
        } else {
            command(&mut a, ChatCommand::Send(text.into()));
        }
        wait(&mut host, &mut [&mut a, &mut b], |host, clients| {
            host.world()
                .resource::<terra_core::WorldClock>()
                .season(host.world().resource::<GameConfig>().world.seasons)
                .season
                == season
                && clients.iter().all(|c| {
                    c.world()
                        .resource::<terra_core::WorldClock>()
                        .season(c.world().resource::<GameConfig>().world.seasons)
                        .season
                        == season
                })
        });
        assert_eq!(
            host.world()
                .resource::<terra_world::edits::EditJournal>()
                .clock,
            Some(*host.world().resource::<terra_core::WorldClock>())
        );
    }
}
use crate::client::{ChatCommand, ClientChat, ClientChatCommands, ClientSession};

fn command(app: &mut App, value: ChatCommand) {
    app.world_mut()
        .resource_mut::<ClientChatCommands>()
        .0
        .push_back(value);
}

#[test]
fn spawn_commands_replicate_and_console_authority_does_not_grant_player_permissions() {
    use crate::client::ReplicatedCreatures;
    use terra_core::creatures::spawning::is_command_id;
    use terra_world::edits::EditJournal;
    let (mut host, mut a, mut b) = pair();
    let a_id = a.world().resource::<ClientSession>().player.unwrap();
    host.world_mut()
        .resource_mut::<server::ConnectedProfiles>()
        .0
        .get_mut(&a_id)
        .unwrap()
        .name = "Alice".into();
    command(&mut a, ChatCommand::Send("/spawn teramic::cow".into()));
    wait(&mut host, &mut [&mut a, &mut b], |host, clients| {
        let population = &host.world().resource::<EditJournal>().gameplay().creatures;
        population.animals.keys().any(|&id| {
            is_command_id(id)
                && clients.iter().all(|client| {
                    client
                        .world()
                        .resource::<ReplicatedCreatures>()
                        .creatures
                        .contains_key(&id)
                })
        })
    });
    assert!(
        host.world()
            .resource::<EditJournal>()
            .gameplay()
            .creatures
            .valid()
    );
    command(
        &mut a,
        ChatCommand::Send("/spawn teramic::clown_fish".into()),
    );
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients[0]
            .world()
            .resource::<ClientChat>()
            .entries
            .iter()
            .any(|entry| entry.text.contains("No loaded water"))
    });
    let count = |host: &App| {
        host.world()
            .resource::<EditJournal>()
            .gameplay()
            .creatures
            .animals
            .keys()
            .filter(|&&id| is_command_id(id))
            .count()
    };
    assert_eq!(count(&host), 1);
    host.world_mut()
        .resource_mut::<GameConfig>()
        .server
        .game_mode = terra_core::GameMode::Survival;
    command(&mut b, ChatCommand::Send("/spawn teramic::pig".into()));
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients[1]
            .world()
            .resource::<ClientChat>()
            .entries
            .iter()
            .any(|entry| entry.text.contains("requires Creative"))
    });
    assert_eq!(count(&host), 1);
    assert!(
        host.world_mut()
            .resource_mut::<server::ServerConsole>()
            .submit("as Alice spawn teramic::pig".into())
    );
    wait(&mut host, &mut [&mut a, &mut b], |host, clients| {
        count(host) == 2
            && clients.iter().all(|client| {
                client
                    .world()
                    .resource::<ReplicatedCreatures>()
                    .creatures
                    .keys()
                    .filter(|&&id| is_command_id(id))
                    .count()
                    == 2
            })
    });
    command(&mut a, ChatCommand::Send("/spawn teramic::sheep".into()));
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients[0]
            .world()
            .resource::<ClientChat>()
            .entries
            .iter()
            .any(|entry| entry.text.contains("requires Creative"))
    });
    assert_eq!(count(&host), 2);
}

#[test]
fn snow_levels_thunder_and_ground_strikes_share_the_real_two_client_authority() {
    use server::ServerConsole;
    use terra_core::{WeatherPattern, WorldWeather};
    let (mut host, mut a, mut b) = pair();
    host.world_mut()
        .resource_mut::<GameConfig>()
        .world
        .time
        .paused = true;
    command(&mut a, ChatCommand::Send("/weather set snow 4".into()));
    wait(&mut host, &mut [&mut a, &mut b], |host, clients| {
        host.world().resource::<WorldWeather>().snow_level == 4
            && clients.iter().all(|client| {
                let state = client.world().resource::<WorldWeather>();
                state.pattern == WeatherPattern::Snow && state.snow_level == 4
            })
    });
    assert!(
        host.world_mut()
            .resource_mut::<ServerConsole>()
            .submit("weather set thunderstorm".into())
    );
    wait(&mut host, &mut [&mut a, &mut b], |host, clients| {
        host.world().resource::<WorldWeather>().pattern == WeatherPattern::Thunderstorm
            && clients.iter().all(|client| {
                client.world().resource::<WorldWeather>().pattern == WeatherPattern::Thunderstorm
            })
    });
    let center = a
        .world()
        .resource::<ClientSession>()
        .confirmed_state()
        .unwrap()
        .position;
    let seed = host.world().resource::<GameConfig>().world.seed.0;
    // Resolve a deterministic rare candidate inside the already loaded test
    // columns; no extra generation or changed production seed is needed.
    let chunks = host
        .world()
        .resource::<terra_world::streaming::ChunkStore>();
    let candidate = (1..10_000)
        .find_map(|sequence| {
            let mut state = WorldWeather::new(seed);
            state.set_pattern(WeatherPattern::Thunderstorm);
            state.sequence = sequence;
            state.advance(30.0, seed);
            let (_, ground, offset) = state.lightning_slot(seed)?;
            let x = (f64::from(center[0]) + offset[0]).floor() as i64;
            let z = (f64::from(center[2]) + offset[1]).floor() as i64;
            let top = (chunks
                .prepared_column([x.div_euclid(16), z.div_euclid(16)])?
                .surface_at(x, z)?
                .height
                - 0.5)
                .floor() as i64;
            (ground
                && chunks.block_at([x, top, z]).is_some_and(|block| block != 0)
                && [-3, 3].into_iter().all(|dx| {
                    [-3, 3].into_iter().all(|dz| {
                        chunks
                            .prepared_column([(x + dx).div_euclid(16), (z + dz).div_euclid(16)])
                            .is_some()
                    })
                }))
            .then_some(state)
        })
        .expect("loaded ground strike candidate");
    host.world_mut()
        .resource_mut::<GameConfig>()
        .world
        .time
        .paused = false;
    *host.world_mut().resource_mut::<WorldWeather>() = candidate;
    wait(&mut host, &mut [&mut a, &mut b], |host, clients| {
        let event = host.world().resource::<WorldWeather>().latest_lightning;
        event.is_some_and(|event| event.ground_strike)
            && clients
                .iter()
                .all(|client| client.world().resource::<WorldWeather>().latest_lightning == event)
    });
    let event = host
        .world()
        .resource::<WorldWeather>()
        .latest_lightning
        .unwrap();
    let block = [
        event.position[0].floor() as i64,
        (event.position[1] - 1.02).round() as i64,
        event.position[2].floor() as i64,
    ];
    assert!(
        host.world()
            .resource::<terra_world::streaming::ChunkStore>()
            .block_at(block)
            .is_some_and(|block| block != 0)
    );
}

#[test]
fn weather_commands_and_console_execute_authoritatively_without_player_permissions() {
    use server::{ConsoleOutput, ServerConsole};
    use terra_core::{WeatherPattern, WorldWeather};
    let (mut host, mut a, mut b) = pair();
    host.world_mut()
        .resource_mut::<GameConfig>()
        .world
        .time
        .paused = true;
    let a_id = a.world().resource::<ClientSession>().player.unwrap();
    host.world_mut()
        .resource_mut::<server::ConnectedProfiles>()
        .0
        .get_mut(&a_id)
        .unwrap()
        .name = "Alice".into();
    command(&mut a, ChatCommand::Send("/weather set rain".into()));
    wait(&mut host, &mut [&mut a, &mut b], |host, clients| {
        host.world().resource::<WorldWeather>().pattern == WeatherPattern::Rain
            && clients.iter().all(|client| {
                let weather = client.world().resource::<WorldWeather>();
                weather.pattern == WeatherPattern::Rain && weather.intensity() > 0.77
            })
    });
    assert_eq!(
        host.world()
            .resource::<terra_world::edits::EditJournal>()
            .weather
            .unwrap()
            .pattern,
        WeatherPattern::Rain
    );
    host.world_mut()
        .resource_mut::<GameConfig>()
        .server
        .game_mode = terra_core::GameMode::Survival;
    command(&mut b, ChatCommand::Send("/weather set snow".into()));
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients[1]
            .world()
            .resource::<ClientChat>()
            .entries
            .iter()
            .any(|entry| entry.text.contains("requires Creative"))
    });
    assert_eq!(
        host.world().resource::<WorldWeather>().pattern,
        WeatherPattern::Rain
    );
    for text in [
        "weather set storm",
        "time set 18:30",
        "list",
        "help 2",
        "gamemode survival Alice",
    ] {
        assert!(
            host.world_mut()
                .resource_mut::<ServerConsole>()
                .submit(text.into())
        );
    }
    wait(&mut host, &mut [&mut a, &mut b], |host, clients| {
        host.world().resource::<WorldWeather>().pattern == WeatherPattern::Storm
            && clients[0]
                .world()
                .resource::<ClientSession>()
                .confirmed_state()
                .is_some_and(|s| s.generation == 1)
    });
    assert!((host.world().resource::<terra_core::WorldClock>().hour() - 18.5).abs() < 0.001);
    assert_eq!(host.world().resource::<WorldWeather>().intensity(), 1.0);
    let position = a
        .world()
        .resource::<ClientSession>()
        .confirmed_state()
        .unwrap()
        .position;
    assert!(
        host.world_mut()
            .resource_mut::<ServerConsole>()
            .submit(format!(
                "tp Alice {} {} {}",
                position[0],
                position[1] + 8.0,
                position[2]
            ))
    );
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients[0]
            .world()
            .resource::<ClientSession>()
            .confirmed_state()
            .is_some_and(|s| s.generation == 2)
    });
    let console = host.world().resource::<ServerConsole>();
    assert!(console.output.iter().any(|line| matches!(line, ConsoleOutput::Line(text) if text.contains("Weather changing to storm"))));
    assert!(
        console
            .output
            .iter()
            .any(|line| matches!(line, ConsoleOutput::Line(text) if text.contains("Alice")))
    );
    assert!(
        console.output.iter().any(
            |line| matches!(line, ConsoleOutput::Line(text) if text.contains("Game mode set"))
        )
    );
    command(&mut a, ChatCommand::Send("/weather set snow".into()));
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients[0]
            .world()
            .resource::<ClientChat>()
            .entries
            .iter()
            .any(|entry| entry.text.contains("requires Creative"))
    });
    assert_eq!(
        host.world().resource::<WorldWeather>().pattern,
        WeatherPattern::Storm
    );
}

#[test]
fn time_coordinates_player_roster_and_local_clear_use_real_transport() {
    let (mut host, mut a, mut b) = pair();
    host.world_mut()
        .resource_mut::<GameConfig>()
        .world
        .time
        .paused = true;
    let b_id = b.world().resource::<ClientSession>().player.unwrap();
    host.world_mut()
        .resource_mut::<server::ConnectedProfiles>()
        .0
        .get_mut(&b_id)
        .unwrap()
        .name = "Bob".into();
    command(&mut a, ChatCommand::Send("/time set 11:30PM".into()));
    wait(&mut host, &mut [&mut a, &mut b], |host, clients| {
        (host.world().resource::<terra_core::WorldClock>().hour() - 23.5).abs() < 0.001
            && clients.iter().all(|c| {
                (c.world().resource::<terra_core::WorldClock>().hour() - 23.5).abs() < 0.001
            })
            && clients[0]
                .world()
                .resource::<ClientChat>()
                .players
                .contains(&"Bob".into())
    });
    assert_eq!(
        host.world()
            .resource::<terra_world::edits::EditJournal>()
            .clock,
        Some(*host.world().resource::<terra_core::WorldClock>())
    );
    let position = a
        .world()
        .resource::<ClientSession>()
        .confirmed_state()
        .unwrap()
        .position;
    command(
        &mut a,
        ChatCommand::Send(format!(
            "/tp {} {} {}",
            position[0] - 3.0,
            position[1] + 6.0,
            position[2]
        )),
    );
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients[0]
            .world()
            .resource::<ClientSession>()
            .confirmed_state()
            .is_some_and(|s| s.generation == 1)
    });
    command(&mut a, ChatCommand::Send("/tp Bob".into()));
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients[0]
            .world()
            .resource::<ClientSession>()
            .confirmed_state()
            .is_some_and(|s| s.generation == 2)
    });
    let target = b
        .world()
        .resource::<ClientSession>()
        .confirmed_state()
        .unwrap()
        .position;
    let actual = a
        .world()
        .resource::<ClientSession>()
        .confirmed_state()
        .unwrap()
        .position;
    assert!((Vec3::from_array(actual) - Vec3::from_array(target)).length() < 0.1);
    a.world_mut().resource_mut::<ClientChat>().notice("local");
    b.world_mut().resource_mut::<ClientChat>().notice("keep");
    command(&mut a, ChatCommand::Send("/clear".into()));
    a.update();
    assert!(a.world().resource::<ClientChat>().entries.is_empty());
    assert!(
        b.world()
            .resource::<ClientChat>()
            .entries
            .iter()
            .any(|entry| entry.text == "keep")
    );
    host.world_mut()
        .resource_mut::<GameConfig>()
        .server
        .game_mode = terra_core::GameMode::Survival;
    command(&mut b, ChatCommand::Send("/time set day".into()));
    command(&mut b, ChatCommand::Send("/tp Bob".into()));
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients[1]
            .world()
            .resource::<ClientChat>()
            .entries
            .iter()
            .filter(|entry| entry.text.contains("requires Creative"))
            .count()
            == 2
    });
    assert!((host.world().resource::<terra_core::WorldClock>().hour() - 23.5).abs() < 0.001);
}

#[test]
fn real_udp_chat_broadcast_locate_and_permission_checked_teleport() {
    let mut host = server();
    let runtime = host.world().resource::<server::ServerRuntime>();
    let mut a = walking_client(runtime.issue_token(301).unwrap());
    let mut b = client(runtime.issue_token(302).unwrap());
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients
            .iter()
            .all(|app| app.world().resource::<ClientSession>().ready)
            && clients[0]
                .world()
                .resource::<ClientSession>()
                .confirmed_state()
                .is_some()
    });
    command(&mut a, ChatCommand::Send("Hello\nneighbor".into()));
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients.iter().all(|app| {
            app.world()
                .resource::<ClientChat>()
                .entries
                .iter()
                .any(|e| e.text == "Helloneighbor" && e.author.is_some())
        })
    });
    command(&mut a, ChatCommand::Send("/locate biome forest".into()));
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients[0]
            .world()
            .resource::<ClientChat>()
            .entries
            .iter()
            .any(|e| e.locate.is_some())
    });
    let found = a
        .world()
        .resource::<ClientChat>()
        .entries
        .iter()
        .find_map(|e| e.locate.clone())
        .unwrap();
    assert_eq!(found.name, "Forest");
    let token = found.token.unwrap();
    assert!(
        !b.world()
            .resource::<ClientChat>()
            .entries
            .iter()
            .any(|e| e.locate.is_some())
    );
    // A second player cannot borrow a result token from the first player.
    command(&mut b, ChatCommand::Teleport(token));
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients[1]
            .world()
            .resource::<ClientChat>()
            .entries
            .iter()
            .any(|e| e.text.contains("expired"))
    });
    a.world_mut().resource_mut::<Desired>().0.direction = Vec3::X;
    let start = a
        .world()
        .resource::<ClientSession>()
        .confirmed_state()
        .unwrap()
        .position[0];
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients[0]
            .world()
            .resource::<ClientSession>()
            .confirmed_state()
            .is_some_and(|p| p.position[0] > start + 4.0)
    });
    a.world_mut().resource_mut::<Desired>().0.direction = Vec3::ZERO;
    command(&mut a, ChatCommand::Teleport(token));
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        let session = clients[0].world().resource::<ClientSession>();
        !session.teleporting
            && session.confirmed_state().is_some_and(|p| {
                p.generation == 1 && (p.position[0] - (found.position[0] as f32 + 0.5)).abs() < 0.1
            })
    });
    // Used capabilities are one-shot, and stale input from another reliable
    // channel cannot move the newly teleported character.
    command(&mut a, ChatCommand::Teleport(token));
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients[0]
            .world()
            .resource::<ClientChat>()
            .entries
            .iter()
            .any(|e| e.text.contains("expired"))
    });
    {
        use lightyear::prelude::MessageSender;
        use protocol::session::{InputChannel, MovementInput};
        let world = a.world_mut();
        let mut query = world.query_filtered::<&mut MessageSender<MovementInput>, With<lightyear::prelude::client::Client>>();
        query
            .single_mut(world)
            .unwrap()
            .send::<InputChannel>(MovementInput {
                generation: 0,
                sequence: 1,
                direction: [1.0, 0.0, 0.0],
                look: [0.0; 2],
                jump: false,
                sprint: true,
                auto_climb: false,
                flight_speed: None,
            });
    }
    // Permission is rechecked on use, not inferred from a client-side game mode.
    host.world_mut()
        .resource_mut::<GameConfig>()
        .server
        .game_mode = terra_core::GameMode::Survival;
    command(&mut b, ChatCommand::Teleport(token));
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients[1]
            .world()
            .resource::<ClientChat>()
            .entries
            .iter()
            .any(|e| e.text.contains("requires Creative"))
    });
    assert_eq!(
        a.world()
            .resource::<ClientSession>()
            .confirmed_state()
            .unwrap()
            .generation,
        1
    );
}

#[test]
fn coordinate_teleport_loads_air_destination_and_rejects_solid_blocks() {
    let mut host = server();
    let token = host
        .world()
        .resource::<server::ServerRuntime>()
        .issue_token(701)
        .unwrap();
    let mut a = walking_client(token);
    wait(&mut host, &mut [&mut a], |_, clients| {
        clients[0]
            .world()
            .resource::<ClientSession>()
            .confirmed_state()
            .is_some_and(|s| s.grounded)
    });
    let origin = a
        .world()
        .resource::<ClientSession>()
        .confirmed_state()
        .unwrap()
        .position;
    command(
        &mut a,
        ChatCommand::Send(format!(
            "/tp {} {} {}",
            origin[0] + 96.0,
            origin[1] + 8.0,
            origin[2]
        )),
    );
    wait(&mut host, &mut [&mut a], |_, clients| {
        let session = clients[0].world().resource::<ClientSession>();
        !session.teleporting && session.confirmed_state().is_some_and(|s| s.generation == 1)
    });
    assert!(
        (a.world()
            .resource::<ClientSession>()
            .confirmed_state()
            .unwrap()
            .position[0]
            - origin[0]
            - 96.0)
            .abs()
            < 0.1
    );
    command(
        &mut a,
        ChatCommand::Send(format!(
            "/tp {} {} {}",
            origin[0],
            origin[1] - 4.0,
            origin[2]
        )),
    );
    wait(&mut host, &mut [&mut a], |_, clients| {
        clients[0]
            .world()
            .resource::<ClientChat>()
            .entries
            .iter()
            .any(|e| e.text.contains("Teleport cancelled"))
    });
    assert_eq!(
        a.world()
            .resource::<ClientSession>()
            .confirmed_state()
            .unwrap()
            .generation,
        1
    );
}

#[test]
fn invalid_commands_and_deleted_destinations_do_not_move_players() {
    let (mut host, mut a, mut b) = pair();
    command(&mut a, ChatCommand::Send("/locate biome unknown".into()));
    command(&mut a, ChatCommand::Send("/locate biome forest".into()));
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        let chat = clients[0].world().resource::<ClientChat>();
        chat.entries
            .iter()
            .any(|e| e.text.contains("Unknown biome"))
            && chat.entries.iter().any(|e| e.locate.is_some())
    });
    let found = a
        .world()
        .resource::<ClientChat>()
        .entries
        .iter()
        .find_map(|e| e.locate.clone())
        .unwrap();
    let ground = [found.position[0], found.position[1] - 1, found.position[2]];
    host.world_mut()
        .resource_scope(|world, mut journal: Mut<terra_world::edits::EditJournal>| {
            let mut chunks = world.resource_mut::<ChunkStore>();
            let expected_material = chunks.block_at(ground).unwrap();
            terra_world::streaming::commit_block_edit(
                &mut chunks,
                &mut journal,
                &terra_world::streaming::BlockEditRequest {
                    position: ground,
                    expected_material,
                    material: 0,
                },
                &Default::default(),
            )
            .unwrap();
        });
    command(&mut a, ChatCommand::Teleport(found.token.unwrap()));
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients[0]
            .world()
            .resource::<ClientChat>()
            .entries
            .iter()
            .any(|e| e.text.contains("no longer safe"))
    });
    assert!(!a.world().resource::<ClientSession>().teleporting);
    assert_eq!(
        a.world()
            .resource::<ClientSession>()
            .confirmed_state()
            .unwrap()
            .generation,
        0
    );
}
