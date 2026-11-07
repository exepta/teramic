//! Dedicated-console authority reuses the same peer commands and async locate work.
use super::*;
use crate::protocol::commands::{Command, help_page, help_pages, parse, split_actor};

pub(super) fn console_commands(
    peers: &mut Query<(&mut Peer, &mut MessageSender<ServerControl>), With<Connected>>,
    online: &[(String, Vec3)],
    world: &mut ChatWorld,
) {
    for _ in 0..8 {
        let Some(line) = world.console.input.pop_front() else {
            break;
        };
        let Some(text) = sanitize(&line) else {
            continue;
        };
        let text = text.strip_prefix('/').unwrap_or(&text);
        if let Some((actor, command)) = actor_command(text) {
            let actor = actor.trim_matches('"');
            let ids: Vec<_> = world
                .identities
                .0
                .iter()
                .filter(|(_, profile)| profile.name.eq_ignore_ascii_case(actor))
                .map(|(id, _)| *id)
                .collect();
            if ids.len() != 1 {
                world.console.line(if ids.is_empty() {
                    "Player is not online."
                } else {
                    "Player name is ambiguous."
                });
                continue;
            }
            let mut queued = false;
            for (mut peer, _) in peers.iter_mut() {
                if peer.id == ids[0] && peer.synced {
                    if peer.chat.requests.len() < 64 {
                        peer.chat.requests.push_back(ChatRequest::Console(format!(
                            "/{}",
                            command.trim_start_matches('/')
                        )));
                        queued = true;
                    }
                    break;
                }
            }
            if !queued {
                world
                    .console
                    .line("Player is still loading or the command queue is full.");
            }
            continue;
        }
        match text {
            "list" => {
                let names = world.console.players.join(", ");
                world.console.line(format!("{} players: {names}", online.len()));
            }
            "stop" => {
                world.console.line("Stopping server...");
                world.console.stop_requested = true;
            }
            "clear" => world.console.output.push_back(ConsoleOutput::Clear),
            _ => match parse(&format!("/{text}")) {
                Ok(Some(Command::GameRule(rule, value))) => {
                    let result = world.set_rule(rule, value);
                    world.console.line(result.unwrap_or_else(str::to_owned));
                }
                Ok(Some(Command::GameRuleList(page))) => {
                    world.console.line(format!("Gamerules - {page} / {}", crate::protocol::commands::rule_pages()));
                    for rule in crate::protocol::commands::rule_page(page) {
                        world.console.line(format!("{} = {} - {}", rule.name(), rule.value(&world.config.world), rule.description()));
                    }
                }
                Ok(Some(Command::Time(hour))) => {
                    world.clock.phase = hour / 24.0;
                    world.journal.bypass_change_detection().clock = Some(*world.clock);
                    world.console.line(format!("Time set to {:02}:{:02}.", hour.floor() as u32, ((hour.fract() * 60.0).round() as u32).min(59)));
                }
                Ok(Some(Command::Season(season))) => {
                    if world.config.world.seasons.set_season(&mut world.clock,season) {
                        world.journal.bypass_change_detection().clock=Some(*world.clock);
                        world.console.line(format!("Season set to {}.",season.name()));
                    } else {world.console.line("Seasons are disabled or the calendar cannot advance.");}
                }
                Ok(Some(Command::Weather(pattern, snow_level))) => {
                    world.set_weather(pattern, snow_level);
                    world.console.line(format!("Weather changing to {}{}.", weather_name(pattern), if matches!(pattern, terra_core::WeatherPattern::Snow | terra_core::WeatherPattern::Blizzard) { format!(" (level {snow_level})") } else { String::new() }));
                }
                Ok(Some(Command::Help(page))) => {
                    world.console.line(format!("TERAMIC Commands - {page} / {}", help_pages()));
                    for entry in help_page(page) {
                        world.console.line(format!("{} - {}", entry.usage.trim_start_matches('/'), entry.description));
                    }
                    world.console.line("Console: list | stop | as <player> <command>; tp <player> <x y z|target>; gamemode <mode> <player>");
                }
                Ok(Some(Command::EffectPlayer(name, kind, seconds, level))) => {
                    let result = effect_target(&name, &world.identities).and_then(|id| {
                        let Some((mut peer, _)) = peers.iter_mut().find(|(peer, _)| peer.id == id && peer.synced) else {
                            return Err("Player is still loading.");
                        };
                        give_effect(&mut peer, &world.config, kind, seconds, level)
                    });
                    world.console.line(result.unwrap_or_else(str::to_owned));
                }
                Ok(Some(_)) => world.console.line("Console commands need a player: as <player> <command>. Example: as Alice tp 10 80 20"),
                Err(error) => world.console.line(error),
                Ok(None) => {
                    if text.starts_with("locate") {
                        console_locate(text, world);
                    } else {
                        world.console.line("Unknown command. Use help to browse commands.");
                    }
                }
            }
        }
    }
}

fn actor_command(text: &str) -> Option<(&str, String)> {
    if let Some(rest) = text.strip_prefix("as ") {
        let (actor, command) = split_actor(rest)?;
        return (!command.is_empty()).then(|| (actor, command.to_owned()));
    }
    if let Some(rest) = text.strip_prefix("tp ") {
        let (actor, destination) = split_actor(rest)?;
        return (!destination.is_empty()).then(|| (actor, format!("tp {destination}")));
    }
    if let Some(rest) = text.strip_prefix("gamemode ") {
        let (mode, actor) = split_actor(rest)?;
        if !actor.is_empty() {
            return Some((actor, format!("gamemode {mode}")));
        }
    }
    None
}

fn console_locate(text: &str, world: &mut ChatWorld) {
    let target = match parse_command(&format!("/{text}")) {
        Ok(Some(target)) => target,
        _ => {
            world.console.line("Usage: locate biome <name> | locate struct <teramic::village|teramic::ruin|teramic::mine|teramic::kingdom|teramic::outpost>. Origin is the world spawn; use as <player> locate ... to search near a player.");
            return;
        }
    };
    if world.runtime.console_job.is_some() || world.runtime.jobs.len() >= JOB_LIMIT {
        world
            .console
            .line("A search is already running or the search queue is busy.");
        return;
    }
    let terrain = world.terrain.0.clone();
    let origin = [world.spawn.ground[0], world.spawn.ground[2]];
    world.runtime.console_job = Some(LocateJob {
        task: AsyncComputeTaskPool::get().spawn(async move { locate(&terrain, origin, target) }),
        started: Instant::now(),
        console: true,
    });
    world.console.line(format!(
        "Searching for {} near world spawn...",
        target.name()
    ));
}

pub(super) fn finish_console_search(world: &mut ChatWorld) {
    let Some(job) = world.runtime.console_job.as_mut() else {
        return;
    };
    let result = future::block_on(future::poll_once(&mut job.task));
    let expired = job.started.elapsed() > Duration::from_secs(15);
    if result.is_none() && !expired {
        return;
    }
    world.runtime.console_job = None;
    if let Some(found) = result.flatten() {
        let [x, y, z] = found.position;
        world
            .console
            .line(format!("{} found at {x}, {y}, {z}.", found.target.name()));
    } else {
        world.console.line(if expired {
            "Locate search timed out."
        } else {
            "No matching biome or structure found within 6144 blocks."
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn actors_are_explicit_and_quoted_names_are_not_shell_input() {
        assert_eq!(
            actor_command("as Alice tp 1 2 3"),
            Some(("Alice", "tp 1 2 3".into()))
        );
        assert_eq!(
            actor_command("tp \"Alice Smith\" Bob"),
            Some(("\"Alice Smith\"", "tp Bob".into()))
        );
        assert_eq!(
            actor_command("gamemode survival \"Alice Smith\""),
            Some(("\"Alice Smith\"", "gamemode survival".into()))
        );
        assert_eq!(actor_command("tp Bob"), None);
        assert_eq!(actor_command("time set night"), None);
        assert_eq!(actor_command("as \"unterminated time set day"), None);
    }
    #[test]
    fn console_queue_is_bounded_and_rejects_invalid_input() {
        let mut console = ServerConsole::default();
        assert!(!console.submit("\n".into()));
        assert!(!console.submit("x".repeat(257)));
        for _ in 0..64 {
            assert!(console.submit("list".into()));
        }
        assert!(!console.submit("stop".into()));
    }
}
