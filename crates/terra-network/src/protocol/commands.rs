//! Shared command grammar and completion vocabulary; execution stays on the server.
#[derive(Debug, PartialEq)]
pub enum Command {
    Help(usize),
    Weather(terra_core::WeatherPattern, u8),
    Mode(terra_core::GameMode),
    Time(f64),
    Season(terra_core::Season),
    GameRule(terra_core::GameRule, f64),
    GameRuleList(usize),
    Coordinates([f32; 3]),
    Player(String),
    Spawn(terra_core::creatures::spawning::SpawnMob),
    EffectGive(terra_core::EffectKind, f32, u8),
    EffectPlayer(String, terra_core::EffectKind, f32, u8),
    EffectClear(Option<terra_core::EffectKind>),
}

pub const HELP_PAGE_SIZE: usize = 5;
pub const RULE_PAGE_SIZE: usize = 8;
pub fn rule_pages() -> usize {
    terra_core::GameRule::ALL.len().div_ceil(RULE_PAGE_SIZE)
}
pub fn rule_page(page: usize) -> &'static [terra_core::GameRule] {
    let start = page.saturating_sub(1).min(rule_pages() - 1) * RULE_PAGE_SIZE;
    &terra_core::GameRule::ALL[start..(start + RULE_PAGE_SIZE).min(terra_core::GameRule::ALL.len())]
}

pub struct CommandDescription {
    pub usage: &'static str,
    pub description: &'static str,
}

pub const CATALOGUE: &[CommandDescription] = &[
    CommandDescription {
        usage: "/effect [@player] <effect> <level> <seconds> | clear [effect]",
        description: "Apply or clear a survival status effect; requires environment permission.",
    },
    CommandDescription {
        usage: "/gamerule <rule> <value> | list [page]",
        description: "Configure sleep votes, season duration and world time speed.",
    },
    CommandDescription {
        usage: "/help [page]",
        description: "Browse commands and their descriptions.",
    },
    CommandDescription {
        usage: "/locate biome <name>",
        description: "Find a biome and a safe teleport destination.",
    },
    CommandDescription {
        usage: "/locate struct <teramic::village|teramic::ruin|teramic::mine|teramic::kingdom|teramic::outpost>",
        description: "Find a generated structure; Creative also supports air and water landings.",
    },
    CommandDescription {
        usage: "/time set <time>",
        description: "Set day, night, morning, afternoon or HH:MM.",
    },
    CommandDescription {
        usage: "/weather set <weather> [snow level]",
        description: "Clear, rain, snow 1-5, storm, thunder or thunderstorm.",
    },
    CommandDescription {
        usage: "/tp <x y z|player>",
        description: "Teleport to coordinates or an online player.",
    },
    CommandDescription {
        usage: "/gamemode <survival|creative|spectator>",
        description: "Change game mode; aliases 0, 1 and 2 also work.",
    },
    CommandDescription {
        usage: "/spawn <mob>",
        description: "Spawn a mob nearby, for example teramic::clown_fish.",
    },
    CommandDescription {
        usage: "/clear",
        description: "Clear your local chat history.",
    },
    CommandDescription {
        usage: "/season set <spring|summer|autumn|winter>",
        description: "Advance the world calendar to a season; keeps the time of day.",
    },
];

pub fn help_pages() -> usize {
    CATALOGUE.len().div_ceil(HELP_PAGE_SIZE)
}

#[cfg(test)]
mod locate_tests {
    use super::*;

    #[test]
    fn structure_completion_is_shared_by_chat_console_and_explicit_player_context() {
        assert_eq!(
            suggestions("/locate struct teramic::v", &[]),
            ["/locate struct teramic::village"]
        );
        assert_eq!(
            suggestions("/locate struct teramic::k", &[]),
            ["/locate struct teramic::kingdom"]
        );
        assert_eq!(
            console_suggestions("locate struct teramic::o", &[]),
            ["locate struct teramic::outpost"]
        );
        assert_eq!(
            console_suggestions("locate struct teramic::m", &[]),
            ["locate struct teramic::mine"]
        );
        assert_eq!(
            console_suggestions("as Alice locate struct teramic::r", &[]),
            ["as Alice locate struct teramic::ruin"]
        );
        assert_eq!(
            suggestions("/locate ", &[]),
            ["/locate biome ", "/locate struct "]
        );
        assert!(
            CATALOGUE
                .iter()
                .any(|entry| entry.usage.starts_with("/locate struct "))
        );
        assert!((1..=help_pages()).all(|page| help_page(page).len() <= HELP_PAGE_SIZE));
    }
}

#[cfg(test)]
mod rule_tests {
    use super::*;
    #[test]
    fn rules_share_validated_grammar_and_completion_between_chat_and_console() {
        for (name, value) in [
            ("sleep_percentage", "0"),
            ("sleep_percentage", "100"),
            ("season_swap_interval", "1"),
            ("game_time_speed", "0.5"),
            ("game_time_speed", "0"),
        ] {
            assert!(matches!(
                parse(&format!("/gamerule {name} {value}")),
                Ok(Some(Command::GameRule(_, _)))
            ));
        }
        for command in [
            "/gamerule sleep_percentage -1",
            "/gamerule sleep_percentage 101",
            "/gamerule season_swap_interval 0",
            "/gamerule season_swap_interval 1.5",
            "/gamerule game_time_speed NaN",
            "/gamerule game_time_speed inf",
            "/gamerule nope 1",
            "/gamerule list 0",
            "/gamerule list 3",
        ] {
            assert!(parse(command).is_err(), "{command}");
        }
        assert_eq!(parse("/gamerule list"), Ok(Some(Command::GameRuleList(1))));
        assert_eq!(rule_page(1).len(), 8);
        assert_eq!(rule_page(2), &[terra_core::GameRule::Pvp]);
        assert!(rule_page(1).len() <= 8);
        assert_eq!(
            console_suggestions("gamerule game_time_speed 0.", &[]),
            ["gamerule game_time_speed 0.5"]
        );
    }
}

#[cfg(test)]
mod effect_tests {
    use super::*;

    #[test]
    fn direct_effect_commands_support_self_named_players_levels_and_duration_order() {
        use terra_core::EffectKind::NightVision;
        assert_eq!(
            parse("/effect teramic::nightvision 2 100"),
            Ok(Some(Command::EffectGive(NightVision, 100.0, 2)))
        );
        assert_eq!(
            parse("/effect @Alice teramic::nightvision 2 200"),
            Ok(Some(Command::EffectPlayer(
                "Alice".into(),
                NightVision,
                200.0,
                2
            )))
        );
        assert_eq!(
            parse("/effect @\"Alice Smith\" nightvision 1 10"),
            Ok(Some(Command::EffectPlayer(
                "Alice Smith".into(),
                NightVision,
                10.0,
                1
            )))
        );
        for invalid in [
            "/effect @ nightvision 1 10",
            "/effect nightvision 0 10",
            "/effect nightvision 4 10",
            "/effect nightvision 1 NaN",
            "/effect nightvision 1 inf",
            "/effect nightvision 1 0",
            "/effect nightvision 1 3601",
            "/effect @Alice nightvision 2",
            "/effect @\"Alice nightvision 1 10",
        ] {
            assert!(parse(invalid).is_err(), "{invalid}");
        }
        let players = ["Alice".into(), "Alice Smith".into()];
        assert!(suggestions("/effect @Al", &players).contains(&"/effect @Alice ".into()));
        assert!(suggestions("/effect @Al", &players).contains(&"/effect @\"Alice Smith\" ".into()));
        assert_eq!(
            suggestions("/effect @\"Alice S", &players),
            ["/effect @\"Alice Smith\" "]
        );
        assert_eq!(
            suggestions("/effect @Alice teramic::nightv", &players),
            ["/effect @Alice teramic::nightvision"]
        );
        assert_eq!(
            suggestions("/effect @aLiCe teramic::nightv", &players),
            ["/effect @Alice teramic::nightvision"]
        );
        assert_eq!(
            console_suggestions("effect @\"alice smith\" teramic::nightv", &players),
            ["effect @\"Alice Smith\" teramic::nightvision"]
        );
        assert!(
            suggestions("/effect nightvision ", &players).contains(&"/effect nightvision 2".into())
        );
        assert!(
            suggestions("/effect nightvision 2 ", &players)
                .contains(&"/effect nightvision 2 200".into())
        );
        assert_eq!(
            console_suggestions("effect @Alice teramic::nightv", &players),
            ["effect @Alice teramic::nightvision"]
        );
    }

    #[test]
    fn effects_use_registered_ids_bounded_values_and_shared_console_completion() {
        for kind in terra_core::EffectKind::ALL {
            let short = kind.identifier().strip_prefix("teramic::").unwrap();
            assert_eq!(
                parse(&format!("/effect give {short} 10 1")),
                Ok(Some(Command::EffectGive(kind, 10.0, 1)))
            );
            assert_eq!(
                parse(&format!("/effect give {}", kind.identifier())),
                Ok(Some(Command::EffectGive(kind, 30.0, 1)))
            );
            assert_eq!(
                parse(&format!("/effect clear {short}")),
                Ok(Some(Command::EffectClear(Some(kind))))
            );
        }
        assert_eq!(parse("/effect clear"), Ok(Some(Command::EffectClear(None))));
        for bad in [
            "/effect give missing",
            "/effect give poison NaN",
            "/effect give poison inf",
            "/effect give poison 0",
            "/effect give poison -10",
            "/effect give poison 3601",
            "/effect give poison 10 0",
            "/effect give poison 10 11",
            "/effect give poison 10 1 extra",
            "/effect clear missing",
        ] {
            assert!(parse(bad).is_err(), "{bad}");
        }
        assert_eq!(
            suggestions("/effect give poi", &[]),
            ["/effect give poison"]
        );
        assert_eq!(
            console_suggestions("as Alice effect give teramic::poi", &[]),
            ["as Alice effect give teramic::poison"]
        );
        assert!(
            suggestions("/effect give poison ", &[]).contains(&"/effect give poison 10".into())
        );
        assert!(
            suggestions("/effect give poison 10 ", &[])
                .contains(&"/effect give poison 10 1".into())
        );
        assert_eq!(help_pages(), 3);
        assert!(parse("/help 3").is_ok());
    }
}

/// Pages are one-based in both chat and the dedicated console.
pub fn help_page(page: usize) -> &'static [CommandDescription] {
    let start = page.saturating_sub(1).min(help_pages() - 1) * HELP_PAGE_SIZE;
    &CATALOGUE[start..(start + HELP_PAGE_SIZE).min(CATALOGUE.len())]
}

pub fn parse(text: &str) -> Result<Option<Command>, &'static str> {
    if let Some(rest) = text.strip_prefix("/effect @\"") {
        let (name, values) = rest.split_once('"').ok_or("Unclosed player name.")?;
        let values: Vec<_> = values.split_whitespace().collect();
        if name.trim().is_empty() || values.len() != 3 {
            return Err("Usage: /effect @\"player name\" <effect> <level> <seconds>");
        }
        let (kind, seconds, level) = parse_effect_values(values[0], values[1], values[2])?;
        return Ok(Some(Command::EffectPlayer(
            name.into(),
            kind,
            seconds,
            level,
        )));
    }
    if let Some(name) = text
        .strip_prefix("/tp ")
        .map(str::trim)
        .and_then(|name| name.strip_prefix('"'))
        .and_then(|name| name.strip_suffix('"'))
    {
        return if name.is_empty() {
            Err("Player name must not be empty.")
        } else {
            Ok(Some(Command::Player(name.into())))
        };
    }
    let parts: Vec<_> = text.split_whitespace().collect();
    match parts.first().copied() {
        Some("/effect") => {
            let usage =
                "Usage: /effect [@player] <effect> <level> <seconds> or /effect clear [effect]";
            match parts.as_slice() {
                [_, "clear"] => Ok(Some(Command::EffectClear(None))),
                [_, "clear", name] => effect_kind(name)
                    .map(|kind| Some(Command::EffectClear(Some(kind))))
                    .ok_or("Unknown effect. Tab lists available effects."),
                [_, "give", name, rest @ ..] if rest.len() <= 2 => {
                    let kind =
                        effect_kind(name).ok_or("Unknown effect. Tab lists available effects.")?;
                    let seconds = rest.first().map_or(Ok(30.0), |seconds| {
                        seconds.parse::<f32>().map_err(|_| usage)
                    })?;
                    let level = rest
                        .get(1)
                        .map_or(Ok(1), |level| level.parse::<u8>().map_err(|_| usage))?;
                    let definition = terra_core::effects::effect_definition(kind);
                    if !seconds.is_finite()
                        || !(0.1..=definition.max_duration_seconds).contains(&seconds)
                        || !(1..=definition.max_level).contains(&level)
                    {
                        return Err(usage);
                    }
                    Ok(Some(Command::EffectGive(kind, seconds, level)))
                }
                [_, name, level, seconds] => {
                    let (kind, seconds, level) = parse_effect_values(name, level, seconds)?;
                    Ok(Some(Command::EffectGive(kind, seconds, level)))
                }
                [_, player, name, level, seconds] if player.starts_with('@') => {
                    let player = player.trim_start_matches('@');
                    if player.is_empty() {
                        return Err("Player name must not be empty.");
                    }
                    let (kind, seconds, level) = parse_effect_values(name, level, seconds)?;
                    Ok(Some(Command::EffectPlayer(
                        player.into(),
                        kind,
                        seconds,
                        level,
                    )))
                }
                _ => Err(usage),
            }
        }
        Some("/gamerule") => match parts.as_slice() {
            [_, "list"] => Ok(Some(Command::GameRuleList(1))),
            [_, "list", page] => page
                .parse::<usize>()
                .ok()
                .filter(|p| (1..=rule_pages()).contains(p))
                .map(|p| Some(Command::GameRuleList(p)))
                .ok_or("Invalid gamerule page."),
            [_, name, value] => {
                let rule = terra_core::GameRule::lookup(name)
                    .ok_or("Unknown gamerule. Use /gamerule list.")?;
                let value = value
                    .parse::<f64>()
                    .ok()
                    .filter(|v| rule.valid(*v))
                    .ok_or("Invalid gamerule value. Use /gamerule list for limits.")?;
                Ok(Some(Command::GameRule(rule, value)))
            }
            _ => Err("Usage: /gamerule <rule> <value> or /gamerule list [page]"),
        },
        Some("/help") => match parts.as_slice() {
            [_] => Ok(Some(Command::Help(1))),
            [_, page] => page
                .parse::<usize>()
                .ok()
                .filter(|page| (1..=help_pages()).contains(page))
                .map(|page| Some(Command::Help(page)))
                .ok_or("Invalid help page. Use /help and Tab to browse pages."),
            _ => Err("Usage: /help [page]"),
        },
        Some("/weather") => {
            use terra_core::WeatherPattern as P;
            let usage =
                "Usage: /weather set <clear|rain|snow [1-5]|storm|thunder|thunderstorm|blizzard>";
            let (pattern, level) = match parts.as_slice() {
                [_, "set", "clear"] => (P::Clear, 1),
                [_, "set", "rain"] => (P::Rain, 1),
                [_, "set", "snow"] => (P::Snow, 1),
                [_, "set", "storm"] => (P::Storm, 1),
                [_, "set", "thunder"] => (P::Thunder, 1),
                [_, "set", "thunderstorm"] => (P::Thunderstorm, 1),
                [_, "set", "blizzard"] => (P::Blizzard, 5),
                [_, "set", "snow", level] => {
                    let level = level
                        .parse::<u8>()
                        .ok()
                        .filter(|n| (1..=5).contains(n))
                        .ok_or(usage)?;
                    (if level == 5 { P::Blizzard } else { P::Snow }, level)
                }
                _ => return Err(usage),
            };
            Ok(Some(Command::Weather(pattern, level)))
        }
        Some("/gamemode") => match parts.as_slice() {
            [_, "survival" | "0"] => Ok(Some(Command::Mode(terra_core::GameMode::Survival))),
            [_, "creative" | "1"] => Ok(Some(Command::Mode(terra_core::GameMode::Creative))),
            [_, "spectator" | "2"] => Ok(Some(Command::Mode(terra_core::GameMode::Spectator))),
            _ => Err("Usage: /gamemode <survival|0|creative|1|spectator|2>"),
        },
        Some("/spawn") => match parts.as_slice() {
            [_, id] => terra_core::creatures::spawning::lookup(id)
                .map(|mob| Some(Command::Spawn(mob)))
                .ok_or("Unknown mob. Use /spawn teramic::<mob>; Tab lists available mobs."),
            _ => Err("Usage: /spawn <mob>, for example /spawn teramic::clown_fish"),
        },
        Some("/time") => match parts.as_slice() {
            [_, "set", time] => hour(time).map(|h| Some(Command::Time(h))),
            _ => Err("Usage: /time set <day|night|morning|afternoon|HH:MM|HH:MMAM/PM>"),
        },
        Some("/season") => match parts.as_slice() {
            [_, "set", name] => match *name {
                "spring" => Ok(Some(Command::Season(terra_core::Season::Spring))),
                "summer" => Ok(Some(Command::Season(terra_core::Season::Summer))),
                "autumn" => Ok(Some(Command::Season(terra_core::Season::Autumn))),
                "winter" => Ok(Some(Command::Season(terra_core::Season::Winter))),
                _ => Err("Usage: /season set <spring|summer|autumn|winter>"),
            },
            _ => Err("Usage: /season set <spring|summer|autumn|winter>"),
        },
        Some("/tp") => match parts.as_slice() {
            [_, x, y, z] => {
                let mut position = [0.0_f32; 3];
                for (out, input) in position.iter_mut().zip([x, y, z]) {
                    *out = input
                        .parse()
                        .map_err(|_| "Coordinates must be finite numbers.")?;
                    if !out.is_finite() || out.abs() > 990_000.0 {
                        return Err("Coordinates must be within -990000 and 990000.");
                    }
                }
                Ok(Some(Command::Coordinates(position)))
            }
            [_, name] => Ok(Some(Command::Player((*name).into()))),
            _ => Err("Usage: /tp <x> <y> <z> or /tp <player>"),
        },
        _ => Ok(None),
    }
}

fn parse_effect_values(
    name: &str,
    level: &str,
    seconds: &str,
) -> Result<(terra_core::EffectKind, f32, u8), &'static str> {
    let kind = effect_kind(name).ok_or("Unknown effect. Tab lists available effects.")?;
    let definition = terra_core::effects::effect_definition(kind);
    let level = level
        .parse::<u8>()
        .ok()
        .filter(|level| (1..=definition.max_level).contains(level))
        .ok_or("Invalid effect level. Tab lists supported levels.")?;
    let seconds = seconds
        .parse::<f32>()
        .ok()
        .filter(|seconds| {
            seconds.is_finite() && (0.1..=definition.max_duration_seconds).contains(seconds)
        })
        .ok_or("Invalid effect duration. Use a finite value from 0.1 to 3600 seconds.")?;
    Ok((kind, seconds, level))
}

fn effect_kind(name: &str) -> Option<terra_core::EffectKind> {
    terra_core::EffectKind::ALL.into_iter().find(|kind| {
        kind.identifier() == name || kind.identifier().strip_prefix("teramic::") == Some(name)
    })
}

fn hour(value: &str) -> Result<f64, &'static str> {
    let value = value.to_ascii_lowercase();
    match value.as_str() {
        "day" => return Ok(12.0),
        "night" => return Ok(0.0),
        "morning" => return Ok(6.0),
        "afternoon" => return Ok(15.0),
        _ => {}
    }
    let error = "Invalid time. Use 00:00 to 23:59 or 1:00AM to 12:59PM.";
    let (number, meridian) = if let Some(number) = value.strip_suffix("am") {
        (number, Some(false))
    } else if let Some(number) = value.strip_suffix("pm") {
        (number, Some(true))
    } else {
        (value.as_str(), None)
    };
    let (hours, minutes) = number.split_once(':').ok_or(error)?;
    if hours.is_empty()
        || hours.len() > 2
        || minutes.len() != 2
        || !hours
            .bytes()
            .chain(minutes.bytes())
            .all(|c| c.is_ascii_digit())
    {
        return Err(error);
    }
    let mut hours: u32 = hours.parse().map_err(|_| error)?;
    let minutes: u32 = minutes.parse().map_err(|_| error)?;
    if minutes > 59 || meridian.map_or(hours > 23, |_| !(1..=12).contains(&hours)) {
        return Err(error);
    }
    if let Some(pm) = meridian {
        hours = hours % 12 + if pm { 12 } else { 0 };
    }
    Ok(f64::from(hours) + f64::from(minutes) / 60.0)
}

pub fn suggestions(prefix: &str, players: &[String]) -> Vec<String> {
    if !prefix.starts_with('/') {
        if let Some((before, name)) = prefix.rsplit_once('@')
            && before.chars().last().is_none_or(|c| c.is_whitespace())
        {
            return players
                .iter()
                .filter(|p| p.to_lowercase().starts_with(&name.to_lowercase()))
                .take(64)
                .map(|p| format!("{before}@{p}"))
                .collect();
        }
        return Vec::new();
    }
    let prefix = prefix.to_ascii_lowercase();
    if !prefix.starts_with('/') {
        return Vec::new();
    }
    let choices: Vec<String> = if prefix.starts_with("/effect give ") {
        effect_suggestions(&prefix, "/effect give ", false)
    } else if prefix.starts_with("/effect clear ") {
        effect_suggestions(&prefix, "/effect clear ", true)
    } else if prefix.starts_with("/effect ") {
        direct_effect_suggestions(&prefix, players)
    } else if prefix.starts_with("/gamerule list ") {
        (1..=rule_pages())
            .map(|p| format!("/gamerule list {p}"))
            .collect()
    } else if prefix.starts_with("/gamerule ") {
        let parts: Vec<_> = prefix.split_whitespace().collect();
        if let Some(rule) = parts
            .get(1)
            .and_then(|name| terra_core::GameRule::lookup(name))
            && (parts.len() >= 3 || prefix.ends_with(' '))
        {
            let values: &[&str] = match rule {
                terra_core::GameRule::SleepPercentage => &["0", "25", "50", "100"],
                terra_core::GameRule::SeasonSwapInterval => &["1", "7", "14", "28"],
                terra_core::GameRule::GameTimeSpeed => &["0", "0.5", "1", "2"],
                terra_core::GameRule::KeepInventory
                | terra_core::GameRule::DaylightCycle
                | terra_core::GameRule::WeatherCycle
                | terra_core::GameRule::MobGriefing
                | terra_core::GameRule::FireSpread
                | terra_core::GameRule::Pvp => &["0", "1"],
            };
            values
                .iter()
                .map(|v| format!("/gamerule {} {v}", rule.name()))
                .collect()
        } else {
            std::iter::once("/gamerule list".to_owned())
                .chain(
                    terra_core::GameRule::ALL
                        .iter()
                        .map(|r| format!("/gamerule {} ", r.name())),
                )
                .collect()
        }
    } else if prefix.starts_with("/spawn ") {
        terra_core::creatures::spawning::MOBS
            .iter()
            .map(|(id, _)| format!("/spawn {id}"))
            .collect()
    } else if prefix.starts_with("/gamemode ") {
        ["survival", "creative", "spectator", "0", "1", "2"]
            .iter()
            .map(|mode| format!("/gamemode {mode}"))
            .collect()
    } else if prefix.starts_with("/weather set snow ") {
        (1..=5)
            .map(|level| format!("/weather set snow {level}"))
            .collect()
    } else if prefix.starts_with("/weather set ") {
        [
            "clear",
            "rain",
            "snow",
            "storm",
            "thunder",
            "thunderstorm",
            "blizzard",
        ]
        .iter()
        .map(|v| format!("/weather set {v}"))
        .collect()
    } else if prefix.starts_with("/weather ") {
        vec!["/weather set ".into()]
    } else if prefix.starts_with("/help ") {
        (1..=help_pages())
            .map(|page| format!("/help {page}"))
            .collect()
    } else if prefix.starts_with("/tp ") {
        players
            .iter()
            .map(|name| {
                if name.contains(char::is_whitespace) {
                    format!("/tp \"{name}\"")
                } else {
                    format!("/tp {name}")
                }
            })
            .collect()
    } else if prefix.starts_with("/time set ") {
        [
            "day",
            "night",
            "morning",
            "afternoon",
            "12:00",
            "12:00AM",
            "12:00PM",
        ]
        .iter()
        .map(|v| format!("/time set {v}"))
        .collect()
    } else if prefix.starts_with("/season set ") {
        ["spring", "summer", "autumn", "winter"]
            .iter()
            .map(|v| format!("/season set {v}"))
            .collect()
    } else if prefix.starts_with("/season ") {
        vec!["/season set ".into()]
    } else if prefix.starts_with("/time ") {
        vec!["/time set ".into()]
    } else if prefix.starts_with("/locate biome ") {
        [
            "desert",
            "forest",
            "plains",
            "highland",
            "ocean",
            "mushroom_park",
            "cliffted_desert",
            "cherry_forest",
        ]
        .iter()
        .map(|v| format!("/locate biome {v}"))
        .collect()
    } else if prefix.starts_with("/locate struct ") {
        [
            "teramic::village",
            "teramic::ruin",
            "teramic::mine",
            "teramic::kingdom",
            "teramic::outpost",
        ]
        .iter()
        .map(|v| format!("/locate struct {v}"))
        .collect()
    } else if prefix.starts_with("/locate ") {
        vec!["/locate biome ".into(), "/locate struct ".into()]
    } else {
        [
            "/help",
            "/locate biome ",
            "/locate struct ",
            "/time set ",
            "/tp ",
            "/clear",
            "/gamemode ",
            "/weather set ",
            "/spawn ",
            "/season set ",
            "/gamerule ",
            "/effect ",
        ]
        .iter()
        .map(|v| (*v).into())
        .collect()
    };
    choices
        .into_iter()
        .filter(|v| {
            v.to_ascii_lowercase().starts_with(&prefix) || effect_player_completion(v, &prefix)
        })
        .take(64)
        .collect()
}

fn effect_player_completion(candidate: &str, prefix: &str) -> bool {
    let Some(typed) = prefix.strip_prefix("/effect @") else {
        return false;
    };
    let incomplete = if let Some(quoted) = typed.strip_prefix('"') {
        !quoted.contains('"')
    } else {
        !typed.contains(char::is_whitespace)
    };
    if !incomplete {
        return false;
    }
    candidate
        .strip_prefix("/effect @")
        .and_then(split_actor)
        .is_some_and(|(actor, _)| {
            actor
                .trim_matches('"')
                .to_ascii_lowercase()
                .starts_with(typed.trim_start_matches('"'))
        })
}

fn direct_effect_suggestions(prefix: &str, players: &[String]) -> Vec<String> {
    let rest = prefix.strip_prefix("/effect ").unwrap_or_default();
    let (command, rest) = if rest.starts_with('@') {
        let Some((player, tail)) = split_actor(rest.trim_start_matches('@')) else {
            return players
                .iter()
                .map(|player| format!("/effect @{} ", quoted(player)))
                .collect();
        };
        let player = players
            .iter()
            .find(|name| name.eq_ignore_ascii_case(player.trim_matches('"')))
            .map_or_else(|| player.to_owned(), |name| quoted(name));
        (format!("/effect @{player} "), tail)
    } else {
        ("/effect ".to_owned(), rest)
    };
    let parts: Vec<_> = rest.split_whitespace().collect();
    if let Some(kind) = parts.first().and_then(|name| effect_kind(name))
        && (parts.len() > 1 || rest.ends_with(' '))
    {
        let name = parts[0];
        if parts.len() > 2 || parts.len() == 2 && rest.ends_with(' ') {
            return ["5", "10", "30", "60", "100", "200", "300"]
                .map(|seconds| format!("{command}{name} {} {seconds}", parts[1]))
                .into();
        }
        return (1..=terra_core::effects::effect_definition(kind).max_level)
            .map(|level| format!("{command}{name} {level}"))
            .collect();
    }
    let qualified = rest.starts_with("teramic:");
    let mut result: Vec<_> = terra_core::EffectKind::ALL
        .into_iter()
        .map(|kind| {
            let name = if qualified {
                kind.identifier()
            } else {
                kind.identifier().trim_start_matches("teramic::")
            };
            format!("{command}{name}")
        })
        .collect();
    if command == "/effect " {
        result.extend(["/effect give ".into(), "/effect clear".into()]);
        result.extend(
            players
                .iter()
                .map(|player| format!("/effect @{} ", quoted(player))),
        );
    }
    result
}

fn effect_suggestions(prefix: &str, command: &str, clear: bool) -> Vec<String> {
    let rest = prefix.strip_prefix(command).unwrap_or_default();
    let parts: Vec<_> = rest.split_whitespace().collect();
    if !clear
        && let Some(kind) = parts.first().and_then(|name| effect_kind(name))
        && (parts.len() > 1 || rest.ends_with(' '))
    {
        let name = parts[0];
        if parts.len() > 2 || parts.len() == 2 && rest.ends_with(' ') {
            let seconds = parts[1];
            return (1..=terra_core::effects::effect_definition(kind).max_level)
                .map(|level| format!("{command}{name} {seconds} {level}"))
                .collect();
        }
        return ["5", "10", "30", "60", "300"]
            .map(|seconds| format!("{command}{name} {seconds}"))
            .into();
    }
    let qualified = rest.starts_with("teramic:");
    terra_core::EffectKind::ALL
        .into_iter()
        .map(|kind| {
            let name = if qualified {
                kind.identifier()
            } else {
                kind.identifier()
                    .strip_prefix("teramic::")
                    .unwrap_or(kind.identifier())
            };
            format!("{command}{name}")
        })
        .collect()
}

/// Console completion uses the same vocabulary, with no slash and explicit actors.
pub fn console_suggestions(prefix: &str, players: &[String]) -> Vec<String> {
    let prefix = prefix
        .trim_start()
        .strip_prefix('/')
        .unwrap_or(prefix.trim_start());
    if let Some(rest) = prefix.strip_prefix("as ") {
        if let Some((actor, command)) = split_actor(rest) {
            return suggestions(&format!("/{command}"), players)
                .into_iter()
                .map(|s| format!("as {actor} {}", s.trim_start_matches('/')))
                .collect();
        }
        return players
            .iter()
            .filter(|name| name.to_lowercase().starts_with(&rest.to_lowercase()))
            .map(|name| format!("as {} ", quoted(name)))
            .take(64)
            .collect();
    }
    let mut choices: Vec<_> = suggestions(&format!("/{prefix}"), players)
        .into_iter()
        .map(|s| s.trim_start_matches('/').to_owned())
        .collect();
    choices.extend(
        ["as ", "list", "stop"]
            .into_iter()
            .filter(|s| s.starts_with(prefix))
            .map(str::to_owned),
    );
    choices.truncate(64);
    choices
}

pub fn quoted(name: &str) -> String {
    if name.contains(char::is_whitespace) {
        format!("\"{name}\"")
    } else {
        name.to_owned()
    }
}

/// Separates one optionally quoted actor from a command; never evaluates shell input.
pub fn split_actor(text: &str) -> Option<(&str, &str)> {
    let text = text.trim_start();
    let end = if let Some(quoted) = text.strip_prefix('"') {
        quoted.find('"')? + 2
    } else {
        text.find(char::is_whitespace)?
    };
    let rest = text[end..].trim_start();
    Some((&text[..end], rest))
}

/// Plain text runs only; user input is never interpreted as markup.
pub fn mention_spans(text: &str, players: &[String]) -> Vec<(String, bool)> {
    let mut runs = Vec::new();
    let mut start = 0;
    let mut cursor = 0;
    while cursor < text.len() {
        let tail = &text[cursor..];
        if tail.starts_with('@')
            && text[..cursor]
                .chars()
                .last()
                .is_none_or(|c| c.is_whitespace() || matches!(c, '(' | '['))
        {
            let found = players
                .iter()
                .filter(|p| !p.is_empty())
                .filter(|p| {
                    tail.get(1..p.len() + 1)
                        .is_some_and(|v| v.eq_ignore_ascii_case(p))
                        && tail
                            .get(p.len() + 1..)
                            .and_then(|s| s.chars().next())
                            .is_none_or(|c| !c.is_alphanumeric() && !matches!(c, '_' | '-'))
                })
                .max_by_key(|p| p.len());
            if let Some(name) = found {
                if start < cursor {
                    runs.push((text[start..cursor].into(), false));
                }
                let end = cursor + 1 + name.len();
                runs.push((text[cursor..end].into(), true));
                cursor = end;
                start = end;
                continue;
            }
        }
        cursor += tail.chars().next().map_or(1, char::len_utf8);
    }
    if start < text.len() {
        runs.push((text[start..].into(), false));
    }
    runs
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn season_commands_complete_and_validate_for_chat_and_console() {
        for (name, season) in [
            ("spring", terra_core::Season::Spring),
            ("summer", terra_core::Season::Summer),
            ("autumn", terra_core::Season::Autumn),
            ("winter", terra_core::Season::Winter),
        ] {
            assert_eq!(
                parse(&format!("/season set {name}")),
                Ok(Some(Command::Season(season)))
            );
        }
        assert_eq!(
            console_suggestions("season set w", &[]),
            ["season set winter"]
        );
        assert_eq!(suggestions("/sea", &[]), ["/season set "]);
        for bad in [
            "/season winter",
            "/season set",
            "/season set unknown",
            "/season set winter extra",
        ] {
            assert!(parse(bad).is_err());
        }
    }
    #[test]
    fn weather_help_and_console_completions_share_validated_vocabulary() {
        for (name, pattern) in [
            ("clear", terra_core::WeatherPattern::Clear),
            ("rain", terra_core::WeatherPattern::Rain),
            ("snow", terra_core::WeatherPattern::Snow),
            ("storm", terra_core::WeatherPattern::Storm),
            ("thunder", terra_core::WeatherPattern::Thunder),
            ("thunderstorm", terra_core::WeatherPattern::Thunderstorm),
        ] {
            assert_eq!(
                parse(&format!("/weather set {name}")),
                Ok(Some(Command::Weather(pattern, 1)))
            );
        }
        for bad in [
            "/weather rain",
            "/weather set snow 0",
            "/weather set snow 6",
            "/weather set rain extra",
            "/help 0",
            "/help 999",
            "/help -1",
        ] {
            assert!(parse(bad).is_err());
        }
        assert_eq!(parse("/help"), Ok(Some(Command::Help(1))));
        assert_eq!(
            parse("/weather set snow 4"),
            Ok(Some(Command::Weather(terra_core::WeatherPattern::Snow, 4)))
        );
        assert_eq!(
            parse("/weather set snow 5"),
            Ok(Some(Command::Weather(
                terra_core::WeatherPattern::Blizzard,
                5
            )))
        );
        assert_eq!(
            parse("/weather set blizzard"),
            Ok(Some(Command::Weather(
                terra_core::WeatherPattern::Blizzard,
                5
            )))
        );
        assert_eq!(
            console_suggestions("weather set snow ", &[]),
            (1..=5)
                .map(|level| format!("weather set snow {level}"))
                .collect::<Vec<_>>()
        );
        assert_eq!(parse("/help 2"), Ok(Some(Command::Help(2))));
        assert_eq!(help_page(1).len(), 5);
        assert_eq!(help_page(2).len(), 5);
        assert_eq!(
            console_suggestions("weather set s", &[]),
            ["weather set snow", "weather set storm"]
        );
        assert_eq!(
            console_suggestions("as Al", &["Alice Smith".into()]),
            ["as \"Alice Smith\" "]
        );
        assert_eq!(
            console_suggestions("as \"Alice Smith\" weather set r", &[]),
            ["as \"Alice Smith\" weather set rain"]
        );
        assert!(console_suggestions("as Alice ", &[]).contains(&"as Alice weather set ".into()));
        assert!(
            console_suggestions("", &[])
                .iter()
                .all(|v| !v.starts_with('/'))
        );
    }
    #[test]
    fn spawn_commands_and_console_completion_use_registered_mob_variants() {
        let clownfish = terra_core::creatures::spawning::lookup("teramic::clown_fish").unwrap();
        assert_eq!(
            parse("/spawn teramic::clown_fish"),
            Ok(Some(Command::Spawn(clownfish)))
        );
        assert_eq!(
            suggestions("/spawn teramic::cl", &[]),
            ["/spawn teramic::clown_fish"]
        );
        assert_eq!(
            console_suggestions("as Alice spawn teramic::cl", &[]),
            ["as Alice spawn teramic::clown_fish"]
        );
        assert_eq!(suggestions("/sp", &[]), ["/spawn "]);
        assert!(
            help_page(2)
                .iter()
                .any(|entry| entry.usage == "/spawn <mob>")
        );
        for invalid in [
            "/spawn",
            "/spawn teramic::unknown",
            "/spawn wrong::cow",
            "/spawn teramic::cow 2",
        ] {
            assert!(parse(invalid).is_err());
        }
        for (id, _) in terra_core::creatures::spawning::MOBS {
            assert!(matches!(
                parse(&format!("/spawn {id}")),
                Ok(Some(Command::Spawn(_)))
            ));
        }
    }
    #[test]
    fn mentions_complete_real_players_and_preserve_plain_text_boundaries() {
        let names = vec!["Alice".into(), "Al".into(), "Bob".into()];
        assert_eq!(suggestions("hello @ali", &names), vec!["hello @Alice"]);
        assert_eq!(
            mention_spans("Hi @Alice, and @Bob!", &names),
            vec![
                ("Hi ".into(), false),
                ("@Alice".into(), true),
                (", and ".into(), false),
                ("@Bob".into(), true),
                ("!".into(), false)
            ]
        );
        assert!(
            mention_spans("mail@Alice @AliceExtra <b>", &names)
                .iter()
                .all(|(_, mention)| !mention)
        );
    }
    #[test]
    fn game_modes_accept_aliases_and_complete_without_extra_arguments() {
        use terra_core::GameMode::*;
        for (name, mode) in [
            ("survival", Survival),
            ("0", Survival),
            ("creative", Creative),
            ("1", Creative),
            ("spectator", Spectator),
            ("2", Spectator),
        ] {
            assert_eq!(
                parse(&format!("/gamemode {name}")),
                Ok(Some(Command::Mode(mode)))
            );
        }
        for invalid in ["/gamemode", "/gamemode 3", "/gamemode creative someone"] {
            assert!(parse(invalid).is_err());
        }
        assert_eq!(suggestions("/game", &[]), vec!["/gamemode ", "/gamerule "]);
        assert_eq!(suggestions("/gamemode c", &[]), vec!["/gamemode creative"]);
    }
    #[test]
    fn time_formats_and_boundaries() {
        for (text, expected) in [
            ("day", 12.0),
            ("morning", 6.0),
            ("afternoon", 15.0),
            ("night", 0.0),
            ("12:00AM", 0.0),
            ("12:00PM", 12.0),
            ("11:30pm", 23.5),
            ("23:30", 23.5),
        ] {
            assert_eq!(hour(text), Ok(expected));
        }
        for bad in [
            "24:00", "00:00AM", "13:00PM", "12:60", "NaN", "-1:00", "1:2", "12:00:00",
        ] {
            assert!(hour(bad).is_err());
        }
    }
    #[test]
    fn coordinates_are_bounded_and_names_are_preserved() {
        assert_eq!(
            parse("/tp -1.5 80 2"),
            Ok(Some(Command::Coordinates([-1.5, 80.0, 2.0])))
        );
        assert_eq!(
            parse("/tp Alice"),
            Ok(Some(Command::Player("Alice".into())))
        );
        assert_eq!(
            parse("/tp \"Alice Smith\""),
            Ok(Some(Command::Player("Alice Smith".into())))
        );
        for bad in [
            "/tp NaN 0 0",
            "/tp inf 0 0",
            "/tp 1000000 0 0",
            "/tp 1 2",
            "/time set day extra",
        ] {
            assert!(parse(bad).is_err());
        }
        assert_eq!(
            suggestions("/tp a", &["Alice".into(), "Bob".into()]),
            ["/tp Alice"]
        );
        assert!(suggestions("hello", &[]).is_empty());
    }
}
