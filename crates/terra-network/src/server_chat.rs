//! Bounded command work and server-issued, permission-checked teleport targets.
use super::*;
use crate::protocol::chat::{ChatEntry, LocateResult, sanitize};
use bevy::tasks::{AsyncComputeTaskPool, Task, futures_lite::future};
use terra_world::{TerraGenerator, generation::biomes::BiomeKind, spawn::WorldSpawn};

const SEARCH_STEP: i64 = 96;
const SEARCH_RINGS: i64 = 64;
const JOB_LIMIT: usize = 4;

#[path = "server_console.rs"]
mod console;
use console::{console_commands, finish_console_search};

fn weather_name(pattern: terra_core::WeatherPattern) -> &'static str {
    match pattern {
        terra_core::WeatherPattern::Climate => "automatic climate",
        terra_core::WeatherPattern::Clear => "clear",
        terra_core::WeatherPattern::Rain => "rain",
        terra_core::WeatherPattern::Snow => "snow",
        terra_core::WeatherPattern::Storm => "storm",
        terra_core::WeatherPattern::Thunder => "thunder",
        terra_core::WeatherPattern::Thunderstorm => "thunderstorm",
        terra_core::WeatherPattern::Blizzard => "blizzard",
    }
}

/// Thread-safe terminal adapters enqueue work; only the authoritative ECS mutates gameplay.
#[derive(Resource, Default)]
pub struct ServerConsole {
    pub input: VecDeque<String>,
    pub output: VecDeque<ConsoleOutput>,
    pub players: Vec<String>,
    pub stop_requested: bool,
}

#[derive(Debug, PartialEq)]
pub enum ConsoleOutput {
    Line(String),
    Clear,
}

impl ServerConsole {
    pub fn submit(&mut self, command: String) -> bool {
        if self.input.len() >= 64 || sanitize(&command).is_none() {
            return false;
        }
        self.input.push_back(command);
        true
    }

    fn line(&mut self, line: impl Into<String>) {
        if self.output.len() >= 256 {
            self.output.pop_front();
        }
        self.output.push_back(ConsoleOutput::Line(line.into()));
    }
}

pub(super) struct PendingTeleport {
    pub position: Vec3,
    started: Instant,
    pub deltas_ready: bool,
    require_ground: bool,
    respawn: bool,
    privileged: bool,
}
impl PendingTeleport {
    pub(super) fn respawn(position: Vec3) -> Self {
        Self {
            position,
            started: Instant::now(),
            deltas_ready: false,
            require_ground: true,
            respawn: true,
            privileged: false,
        }
    }
}

pub(super) struct PeerChat {
    console_active: bool,
    roster: Vec<String>,
    requests: VecDeque<ChatRequest>,
    credits: f64,
    rate_time: Instant,
    limited: bool,
    last_limit_reply: Option<Instant>,
    pub targets: VecDeque<(u64, WorldSpawn)>,
    pub teleport: Option<PendingTeleport>,
}

impl Default for PeerChat {
    fn default() -> Self {
        Self {
            console_active: false,
            roster: Vec::new(),
            requests: VecDeque::new(),
            credits: 4.0,
            rate_time: Instant::now(),
            limited: false,
            last_limit_reply: None,
            targets: VecDeque::new(),
            teleport: None,
        }
    }
}

pub(super) enum ChatRequest {
    Text(String),
    Console(String),
    Teleport(u64),
}

pub(super) fn enqueue(peer: &mut Peer, request: ChatRequest) {
    let chat = &mut peer.chat;
    chat.credits = (chat.credits + chat.rate_time.elapsed().as_secs_f64()).min(4.0);
    chat.rate_time = Instant::now();
    if chat.credits < 1.0 || chat.requests.len() >= 4 {
        chat.limited = true;
        return;
    }
    chat.credits -= 1.0;
    chat.requests.push_back(request);
}

struct LocateJob {
    task: Task<Option<Located>>,
    started: Instant,
    console: bool,
}

struct Located {
    biome: BiomeKind,
    position: [i64; 3],
    spawn: Option<WorldSpawn>,
}

#[derive(Resource, Default)]
pub(super) struct ChatRuntime {
    jobs: HashMap<PlayerId, LocateJob>,
    next_id: u64,
    console_reply: bool,
    console_lines: VecDeque<String>,
    console_job: Option<LocateJob>,
}

impl ChatRuntime {
    pub(super) fn entry(&mut self, text: impl Into<String>) -> ChatEntry {
        self.next_id = self.next_id.saturating_add(1);
        ChatEntry {
            id: self.next_id,
            author: None,
            text: text.into(),
            locate: None,
        }
    }
}

#[derive(SystemParam)]
pub(super) struct ChatWorld<'w, 's> {
    runtime: ResMut<'w, ChatRuntime>,
    config: ResMut<'w, GameConfig>,
    clock: ResMut<'w, terra_core::WorldClock>,
    weather: ResMut<'w, terra_core::WorldWeather>,
    console: ResMut<'w, ServerConsole>,
    spawn: Res<'w, WorldSpawn>,
    terrain: ResMut<'w, terra_world::streaming::WorldTerrain>,
    journal: ResMut<'w, EditJournal>,
    chunks: Res<'w, ChunkStore>,
    identities: Res<'w, ConnectedProfiles>,
    server: Res<'w, ServerRuntime>,
    bodies: Query<
        'w,
        's,
        (
            &'static mut Transform,
            &'static mut avian3d::prelude::LinearVelocity,
            &'static mut CharacterGrounded,
        ),
        With<ServerCharacter>,
    >,
}

impl ChatWorld<'_, '_> {
    fn set_rule(&mut self, rule: terra_core::GameRule, value: f64) -> Result<String, &'static str> {
        let settings =
            terra_core::WorldRuleSettings::set(&self.config.world, &mut self.clock, rule, value)?;
        settings.apply(&mut self.config.world);
        self.terrain
            .bypass_change_detection()
            .0
            .set_world_rules(settings);
        let journal = self.journal.bypass_change_detection();
        journal.world_rules = Some(settings);
        journal.clock = Some(*self.clock);
        Ok(format!("{} = {value}", rule.name()))
    }
    fn set_weather(&mut self, pattern: terra_core::WeatherPattern, snow_level: u8) {
        self.weather.set_pattern(pattern);
        self.weather.snow_level = snow_level.clamp(1, 5);
        if self.clock.paused {
            // An explicit admin edit still takes effect when automatic world time is frozen.
            let transition = self.weather.transition_seconds;
            self.weather.advance(transition, self.config.world.seed.0);
        }
        self.journal.bypass_change_detection().weather = Some(*self.weather);
    }
}

pub(super) fn tick(
    mut peers: Query<(&mut Peer, &mut MessageSender<ServerControl>), With<Connected>>,
    mut world: ChatWorld,
) {
    world
        .runtime
        .jobs
        .retain(|id, _| peers.iter().any(|(peer, _)| peer.id == *id));
    let mut broadcast = Vec::new();
    let online: Vec<_> = peers
        .iter()
        .filter(|(peer, _)| peer.synced)
        .filter_map(|(peer, _)| {
            let name = world.identities.0.get(&peer.id)?.name.clone();
            let (body, _, _) = world.bodies.get(peer.body).ok()?;
            Some((name, body.translation))
        })
        .collect();
    let mut roster: Vec<_> = online.iter().map(|(name, _)| name.clone()).collect();
    roster.sort();
    roster.dedup();
    world.console.players.clone_from(&roster);
    console_commands(&mut peers, &online, &mut world);
    for (mut peer, mut sender) in &mut peers {
        if !peer.synced {
            continue;
        }
        if peer.chat.roster != roster {
            sender.send::<ControlChannel>(ServerControl::ChatPlayers(roster.clone()));
            peer.chat.roster.clone_from(&roster);
        }
        if peer.chat.limited
            && peer
                .chat
                .last_limit_reply
                .is_none_or(|t| t.elapsed() >= Duration::from_secs(1))
        {
            peer.chat.limited = false;
            peer.chat.last_limit_reply = Some(Instant::now());
            reply(
                &mut world.runtime,
                &mut sender,
                "Chat rate limit exceeded. Please wait.",
            );
        }
        while let Some(request) = peer.chat.requests.pop_front() {
            peer.chat.console_active = matches!(request, ChatRequest::Console(_));
            world.runtime.console_reply = peer.chat.console_active;
            match request {
                ChatRequest::Text(text) | ChatRequest::Console(text) => {
                    let Some(text) = sanitize(&text) else {
                        reply(
                            &mut world.runtime,
                            &mut sender,
                            "Chat messages must contain 1 to 256 characters.",
                        );
                        continue;
                    };
                    if !text.starts_with('/') {
                        if broadcast.len() >= 64 {
                            reply(
                                &mut world.runtime,
                                &mut sender,
                                "The chat is busy. Please try again shortly.",
                            );
                            continue;
                        }
                        let mut entry = world.runtime.entry(text);
                        entry.author = world
                            .identities
                            .0
                            .get(&peer.id)
                            .map(|profile| profile.name.clone());
                        broadcast.push(entry);
                        continue;
                    }
                    match crate::protocol::commands::parse(&text) {
                        Ok(Some(command)) => {
                            execute(command, &online, &mut peer, &mut sender, &mut world);
                            continue;
                        }
                        Err(error) => {
                            reply(&mut world.runtime, &mut sender, error);
                            continue;
                        }
                        Ok(None) => {}
                    }
                    let biome = match parse_command(&text) {
                        Ok(Some(biome)) => biome,
                        Ok(None) => {
                            reply(
                                &mut world.runtime,
                                &mut sender,
                                "Use /help to browse commands.",
                            );
                            continue;
                        }
                        Err(error) => {
                            reply(&mut world.runtime, &mut sender, error);
                            continue;
                        }
                    };
                    if world.runtime.jobs.contains_key(&peer.id) {
                        reply(
                            &mut world.runtime,
                            &mut sender,
                            "A biome search is already running.",
                        );
                        continue;
                    }
                    if world.runtime.jobs.len() >= JOB_LIMIT {
                        reply(
                            &mut world.runtime,
                            &mut sender,
                            "The search queue is busy. Please try again shortly.",
                        );
                        continue;
                    }
                    let Ok((body, _, _)) = world.bodies.get(peer.body) else {
                        continue;
                    };
                    let origin = [
                        body.translation.x.floor() as i64,
                        body.translation.z.floor() as i64,
                    ];
                    let terrain = world.terrain.0.clone();
                    world.runtime.jobs.insert(
                        peer.id,
                        LocateJob {
                            task: AsyncComputeTaskPool::get()
                                .spawn(async move { locate(&terrain, origin, biome) }),
                            started: Instant::now(),
                            console: peer.chat.console_active,
                        },
                    );
                    reply(
                        &mut world.runtime,
                        &mut sender,
                        format!("Searching for {}...", biome_name(biome)),
                    );
                }
                ChatRequest::Teleport(token) => {
                    if !can_teleport(&peer, &world.config) {
                        reply(
                            &mut world.runtime,
                            &mut sender,
                            "Teleport requires Creative mode or environment permission.",
                        );
                        continue;
                    }
                    if peer.chat.teleport.is_some() {
                        reply(
                            &mut world.runtime,
                            &mut sender,
                            "A teleport is already loading.",
                        );
                        continue;
                    }
                    let Some((_, target)) = peer
                        .chat
                        .targets
                        .iter()
                        .find(|(id, _)| *id == token)
                        .copied()
                    else {
                        reply(
                            &mut world.runtime,
                            &mut sender,
                            "This teleport target has expired. Locate the biome again.",
                        );
                        continue;
                    };
                    if !target.is_valid(&world.terrain.0, &world.journal) {
                        reply(
                            &mut world.runtime,
                            &mut sender,
                            "The destination is no longer safe. Locate the biome again.",
                        );
                        continue;
                    }
                    peer.chat.targets.retain(|(id, _)| *id != token);
                    peer.chat.teleport = Some(PendingTeleport {
                        respawn: false,
                        position: Vec3::from_array(target.feet().map(|p| p as f32))
                            + Vec3::Y * (terra_physics::character::PLAYER_HALF_HEIGHT + 0.02),
                        started: Instant::now(),
                        deltas_ready: false,
                        require_ground: true,
                        privileged: peer.chat.console_active,
                    });
                    peer.mining = None;
                    sender.send::<ControlChannel>(ServerControl::Mining(None));
                    sender.send::<ControlChannel>(ServerControl::TeleportPreparing);
                }
            }
        }
        peer.chat.console_active = false;
        world.runtime.console_reply = world
            .runtime
            .jobs
            .get(&peer.id)
            .is_some_and(|job| job.console);
        finish_search(&mut peer, &mut sender, &mut world);
        world.runtime.console_reply = peer
            .chat
            .teleport
            .as_ref()
            .is_some_and(|pending| pending.privileged);
        finish_teleport(&mut peer, &mut sender, &mut world);
    }
    world.runtime.console_reply = false;
    finish_console_search(&mut world);
    while let Some(line) = world.runtime.console_lines.pop_front() {
        world.console.line(line);
    }
    for entry in broadcast {
        for (peer, mut sender) in &mut peers {
            if peer.synced {
                sender.send::<ControlChannel>(ServerControl::Chat(entry.clone()));
            }
        }
    }
}

fn finish_search(
    peer: &mut Peer,
    sender: &mut MessageSender<ServerControl>,
    world: &mut ChatWorld,
) {
    let Some(job) = world.runtime.jobs.get_mut(&peer.id) else {
        return;
    };
    let result = future::block_on(future::poll_once(&mut job.task));
    let expired = job.started.elapsed() > Duration::from_secs(15);
    if result.is_none() && !expired {
        return;
    }
    world.runtime.jobs.remove(&peer.id);
    let Some(found) = result.flatten() else {
        reply(
            &mut world.runtime,
            sender,
            if expired {
                "Biome search timed out. Try again from another area."
            } else {
                "No matching biome was found within 6144 blocks. Try another location or world settings."
            },
        );
        return;
    };
    let mut entry = world.runtime.entry("");
    let safe = found
        .spawn
        .filter(|spawn| spawn.is_valid(&world.terrain.0, &world.journal));
    let token = if can_teleport(peer, &world.config) {
        safe.map(|spawn| {
            if peer.chat.targets.len() == 8 {
                peer.chat.targets.pop_front();
            }
            peer.chat.targets.push_back((entry.id, spawn));
            entry.id
        })
    } else {
        None
    };
    entry.locate = Some(LocateResult {
        name: biome_name(found.biome).into(),
        position: safe.map_or(found.position, |s| {
            [s.ground[0], s.ground[1] + 1, s.ground[2]]
        }),
        token,
    });
    if world.runtime.console_reply {
        let [x, y, z] = found.position;
        world.runtime.console_lines.push_back(format!(
            "Biome {} found at {x}, {y}, {z}.",
            biome_name(found.biome)
        ));
    }
    sender.send::<ControlChannel>(ServerControl::Chat(entry));
    if token.is_none() {
        reply(
            &mut world.runtime,
            sender,
            if !can_teleport(peer, &world.config) {
                "Teleport requires Creative mode or environment permission."
            } else {
                "No safe dry landing spot near this result; teleport is unavailable."
            },
        );
    }
}

fn finish_teleport(
    peer: &mut Peer,
    sender: &mut MessageSender<ServerControl>,
    world: &mut ChatWorld,
) {
    let Some(pending) = peer.chat.teleport.as_ref() else {
        return;
    };
    let position = pending.position;
    let state = if pending.require_ground {
        spawn::loaded_state(&world.chunks, &world.terrain.0, &world.journal, position)
    } else {
        teleport_space(&world.chunks, position)
    };
    let require_ground = pending.require_ground;
    let respawn = pending.respawn;
    let expired = pending.started.elapsed() > Duration::from_secs(25);
    let permitted = if respawn {
        peer.vitals.dead()
    } else {
        pending.privileged || can_teleport(peer, &world.config)
    };
    if expired || matches!(state, spawn::LoadedSpawn::Invalid) || !permitted {
        peer.chat.teleport = None;
        sender.send::<ControlChannel>(ServerControl::TeleportCancelled);
        reply(
            &mut world.runtime,
            sender,
            "Teleport cancelled: destination unavailable or no longer permitted.",
        );
        return;
    }
    if !pending.deltas_ready || !matches!(state, spawn::LoadedSpawn::Ready) {
        return;
    }
    let Some(generation) = peer.generation.checked_add(1) else {
        return;
    };
    let Ok((mut body, mut velocity, mut grounded)) = world.bodies.get_mut(peer.body) else {
        return;
    };
    body.translation = position;
    velocity.0 = Vec3::ZERO;
    grounded.0 = require_ground;
    peer.generation = generation;
    peer.inputs.clear();
    peer.received = 0;
    peer.ack = 0;
    peer.last_movement = CharacterInput::default();
    peer.survival.teleported(respawn);
    if respawn {
        peer.vitals = Default::default();
    }
    peer.chat.teleport = None;
    sender.send::<ControlChannel>(ServerControl::Teleport {
        tick: world.server.tick,
        state: PlayerState {
            gender: peer.gender,
            generation,
            player: peer.id,
            position: position.to_array(),
            grounded: require_ground,
            look: peer.look,
            survival: peer.vitals,
            ..default()
        },
    });
    if !respawn {
        reply(&mut world.runtime, sender, "Teleported.");
    }
}

fn execute(
    command: crate::protocol::commands::Command,
    online: &[(String, Vec3)],
    peer: &mut Peer,
    sender: &mut MessageSender<ServerControl>,
    world: &mut ChatWorld,
) {
    use crate::protocol::commands::Command;
    if let Command::GameRuleList(page) = command {
        reply(
            &mut world.runtime,
            sender,
            format!(
                "Gamerules - {page} / {}",
                crate::protocol::commands::rule_pages()
            ),
        );
        for rule in crate::protocol::commands::rule_page(page) {
            let line = format!(
                "{} = {} - {}",
                rule.name(),
                rule.value(&world.config.world),
                rule.description()
            );
            reply(&mut world.runtime, sender, line);
        }
        return;
    }
    if let Command::Help(page) = command {
        for entry in crate::protocol::commands::help_page(page) {
            reply(
                &mut world.runtime,
                sender,
                format!("{} - {}", entry.usage, entry.description),
            );
        }
        return;
    }
    if let Command::Mode(mode) = command {
        change_mode(mode, peer, sender, world);
        return;
    }
    if !can_teleport(peer, &world.config) {
        reply(
            &mut world.runtime,
            sender,
            "This command requires Creative mode or environment permission.",
        );
        return;
    }
    match command {
        Command::EffectGive(kind, seconds, level) => {
            if peer.game_mode(&world.config) != terra_core::GameMode::Survival || peer.vitals.dead()
            {
                reply(
                    &mut world.runtime,
                    sender,
                    "Status effects require a living Survival player.",
                );
            } else {
                peer.vitals.effects.apply(kind, seconds, level);
                reply(
                    &mut world.runtime,
                    sender,
                    format!("Applied {} {level} for {seconds}s.", kind.identifier()),
                );
            }
            return;
        }
        Command::EffectClear(kind) => {
            if let Some(kind) = kind {
                peer.vitals.effects.remove(kind);
            } else {
                peer.vitals.effects.clear();
            }
            reply(&mut world.runtime, sender, "Status effects cleared.");
            return;
        }
        _ => {}
    }
    if let Command::GameRule(rule, value) = command {
        let result = world.set_rule(rule, value);
        reply(
            &mut world.runtime,
            sender,
            result.unwrap_or_else(str::to_owned),
        );
        return;
    }
    if let Command::Spawn(mob) = command {
        if peer.chat.teleport.is_some() {
            reply(
                &mut world.runtime,
                sender,
                "Finish teleporting before spawning a mob.",
            );
            return;
        }
        let Ok((body, _, _)) = world.bodies.get(peer.body) else {
            return;
        };
        let origin = (body.translation - Vec3::Y * terra_physics::character::PLAYER_HALF_HEIGHT)
            .to_array()
            .map(f64::from);
        let result = terra_world::creatures::spawning::near(
            &world.journal.gameplay().creatures,
            &terra_world::creatures::Habitat {
                terrain: &world.terrain.0,
                chunks: &world.chunks,
                journal: &world.journal,
            },
            mob,
            origin,
            peer.look[0],
        );
        match result {
            Ok(animal) => {
                let [x, y, z] = animal.position;
                world
                    .journal
                    .gameplay_mut()
                    .creatures
                    .animals
                    .insert(animal.id, animal);
                reply(
                    &mut world.runtime,
                    sender,
                    format!("Spawned {} at {x:.1}, {y:.1}, {z:.1}.", mob.id()),
                );
            }
            Err(error) => reply(&mut world.runtime, sender, error),
        }
        return;
    }
    if let Command::Season(season) = command {
        if world
            .config
            .world
            .seasons
            .set_season(&mut world.clock, season)
        {
            world.journal.bypass_change_detection().clock = Some(*world.clock);
            reply(
                &mut world.runtime,
                sender,
                format!("Season set to {}.", season.name()),
            );
        } else {
            reply(
                &mut world.runtime,
                sender,
                "Seasons are disabled or the calendar cannot advance.",
            );
        }
        return;
    }
    if let Command::Time(hour) = command {
        world.clock.phase = hour / 24.0;
        world.journal.bypass_change_detection().clock = Some(*world.clock);
        reply(
            &mut world.runtime,
            sender,
            format!(
                "Time set to {:02}:{:02}.",
                hour.floor() as u32,
                ((hour.fract() * 60.0).round() as u32).min(59)
            ),
        );
        return;
    }
    if let Command::Weather(pattern, snow_level) = command {
        world.set_weather(pattern, snow_level);
        reply(
            &mut world.runtime,
            sender,
            format!(
                "Weather changing to {}{}.",
                weather_name(pattern),
                if matches!(
                    pattern,
                    terra_core::WeatherPattern::Snow | terra_core::WeatherPattern::Blizzard
                ) {
                    format!(" (level {snow_level})")
                } else {
                    String::new()
                }
            ),
        );
        return;
    }
    if peer.chat.teleport.is_some() {
        reply(&mut world.runtime, sender, "A teleport is already loading.");
        return;
    }
    let position = match command {
        Command::Coordinates(feet) => {
            Vec3::from_array(feet) + Vec3::Y * (terra_physics::character::PLAYER_HALF_HEIGHT + 0.02)
        }
        Command::Player(name) => {
            let matches: Vec<_> = online
                .iter()
                .filter(|(n, _)| n.eq_ignore_ascii_case(&name))
                .collect();
            if matches.len() != 1 {
                reply(
                    &mut world.runtime,
                    sender,
                    if matches.is_empty() {
                        "Player is not online."
                    } else {
                        "Player name is ambiguous."
                    },
                );
                return;
            }
            matches[0].1
        }
        Command::Time(_)
        | Command::Season(_)
        | Command::GameRule(_, _)
        | Command::GameRuleList(_)
        | Command::Mode(_)
        | Command::Weather(_, _)
        | Command::Help(_)
        | Command::Spawn(_)
        | Command::EffectGive(..)
        | Command::EffectClear(_) => return,
    };
    peer.chat.teleport = Some(PendingTeleport {
        respawn: false,
        position,
        started: Instant::now(),
        deltas_ready: false,
        require_ground: false,
        privileged: peer.chat.console_active,
    });
    peer.mining = None;
    sender.send::<ControlChannel>(ServerControl::Mining(None));
    sender.send::<ControlChannel>(ServerControl::TeleportPreparing);
}

fn teleport_space(chunks: &ChunkStore, position: Vec3) -> spawn::LoadedSpawn {
    use terra_physics::character::{PLAYER_HALF_HEIGHT, PLAYER_RADIUS};
    let half = Vec3::new(PLAYER_RADIUS, PLAYER_HALF_HEIGHT, PLAYER_RADIUS);
    let low = (position - half).floor().as_i64vec3();
    let high = (position + half - Vec3::splat(0.001)).floor().as_i64vec3();
    for z in low.z..=high.z {
        for y in low.y..=high.y {
            for x in low.x..=high.x {
                let Some(material) = chunks.block_at([x, y, z]) else {
                    return spawn::LoadedSpawn::Waiting;
                };
                if terra_voxel::MaterialId::from_raw(material)
                    .is_none_or(terra_voxel::MaterialId::is_solid)
                {
                    return spawn::LoadedSpawn::Invalid;
                }
            }
        }
    }
    spawn::LoadedSpawn::Ready
}

fn reply(
    runtime: &mut ChatRuntime,
    sender: &mut MessageSender<ServerControl>,
    text: impl Into<String>,
) {
    let text = text.into();
    if runtime.console_reply && runtime.console_lines.len() < 256 {
        runtime.console_lines.push_back(text.clone());
    }
    sender.send::<ControlChannel>(ServerControl::Chat(runtime.entry(text)));
}

fn can_teleport(peer: &Peer, config: &GameConfig) -> bool {
    !peer.vitals.dead()
        && (peer.chat.console_active
            || peer.game_mode(config) != terra_core::GameMode::Survival
            || peer.permissions.environment)
}

fn change_mode(
    mode: terra_core::GameMode,
    peer: &mut Peer,
    sender: &mut MessageSender<ServerControl>,
    world: &mut ChatWorld,
) {
    if !peer.chat.console_active && !peer.permissions.game_mode {
        reply(
            &mut world.runtime,
            sender,
            "Changing game mode requires game_mode permission.",
        );
        return;
    }
    if peer.vitals.dead() || peer.chat.teleport.is_some() {
        reply(
            &mut world.runtime,
            sender,
            "Respawn or finish teleporting before changing game mode.",
        );
        return;
    }
    if peer.game_mode(&world.config) == terra_core::GameMode::Spectator
        && mode != terra_core::GameMode::Spectator
        && world.bodies.get(peer.body).ok().is_none_or(|(body, _, _)| {
            !terra_physics::character::space_clear(&world.chunks, body.translation)
        })
    {
        reply(
            &mut world.runtime,
            sender,
            "Move into loaded open space before leaving Spectator.",
        );
        return;
    }
    if peer.mode != Some(mode) {
        let Some(generation) = peer.generation.checked_add(1) else {
            return;
        };
        let Ok((body, mut velocity, grounded)) = world.bodies.get_mut(peer.body) else {
            return;
        };
        peer.mode = Some(mode);
        peer.generation = generation;
        peer.inputs.clear();
        peer.received = 0;
        peer.ack = 0;
        peer.last_movement = CharacterInput::default();
        peer.survival.teleported(false);
        peer.mining = None;
        velocity.0 = Vec3::ZERO;
        if mode != terra_core::GameMode::Survival {
            peer.vitals = Default::default();
        }
        let mut stats = world
            .journal
            .gameplay()
            .player_stats
            .get(&peer.uuid)
            .copied()
            .unwrap_or_default();
        stats.game_mode = Some(mode);
        stats.survival = peer.vitals;
        world
            .journal
            .bypass_change_detection()
            .update_player_stats(peer.uuid, stats);
        peer.stats_sent = None;
        sender.send::<ControlChannel>(ServerControl::Mining(None));
        sender.send::<ControlChannel>(ServerControl::GameMode {
            mode,
            allow_flight: peer.can_fly(&world.config),
            tick: world.server.tick,
            state: PlayerState {
                gender: peer.gender,
                generation,
                player: peer.id,
                position: body.translation.to_array(),
                grounded: grounded.0,
                look: peer.look,
                survival: peer.vitals,
                ..default()
            },
        });
    }
    reply(
        &mut world.runtime,
        sender,
        format!("Game mode set to {}.", mode.label()),
    );
}

fn biome_name(kind: BiomeKind) -> &'static str {
    match kind {
        BiomeKind::Ocean => "Ocean",
        BiomeKind::Plains => "Plains",
        BiomeKind::Forest => "Forest",
        BiomeKind::Desert => "Desert",
        BiomeKind::Highland => "Highland",
        BiomeKind::MushroomPark => "Mushroom Park",
        BiomeKind::ClifftedDesert => "Cliffted Desert",
    }
}

fn parse_command(text: &str) -> Result<Option<BiomeKind>, &'static str> {
    let parts: Vec<_> = text.split_whitespace().collect();
    if parts == ["/help"] {
        return Ok(None);
    }
    if parts.len() < 3 || parts[0] != "/locate" || parts[1] != "biome" {
        return Err("Usage: /locate biome <name>. Use /help for biome names.");
    }
    match parts[2..].join("_").to_ascii_lowercase().as_str() {
        "ocean" => Ok(Some(BiomeKind::Ocean)),
        "plains" => Ok(Some(BiomeKind::Plains)),
        "forest" => Ok(Some(BiomeKind::Forest)),
        "desert" => Ok(Some(BiomeKind::Desert)),
        "highland" => Ok(Some(BiomeKind::Highland)),
        "mushroom_park" | "mushroompark" => Ok(Some(BiomeKind::MushroomPark)),
        "cliffted_desert" | "cliffteddesert" => Ok(Some(BiomeKind::ClifftedDesert)),
        _ => Err("Unknown biome. Use /help for biome names."),
    }
}

fn locate(terrain: &TerraGenerator, origin: [i64; 2], biome: BiomeKind) -> Option<Located> {
    if !terrain.config().biomes && biome != BiomeKind::Forest {
        return None;
    }
    let sample = |x: i64, z: i64| {
        if x.abs() > 990_000
            || z.abs() > 990_000
            || terrain
                .biome_at(x as f64 + 0.5, z as f64 + 0.5)
                .weights
                .dominant()
                != biome
        {
            return None;
        }
        let height = terrain.surface_height(x as f64 + 0.5, z as f64 + 0.5);
        let mut found = Located {
            biome,
            position: [x, height.ceil() as i64, z],
            spawn: None,
        };
        // Only the winning neighborhood resolves expensive trees/clearance.
        let journal = EditJournal::default();
        'landing: for ring in 0_i64..=6 {
            for dz in -ring..=ring {
                for dx in -ring..=ring {
                    if dx.abs().max(dz.abs()) != ring {
                        continue;
                    }
                    let sx = x + dx * 4;
                    let sz = z + dz * 4;
                    if terrain
                        .biome_at(sx as f64 + 0.5, sz as f64 + 0.5)
                        .weights
                        .dominant()
                        != biome
                    {
                        continue;
                    }
                    let height = terrain.surface_height(sx as f64 + 0.5, sz as f64 + 0.5);
                    let spawn = WorldSpawn {
                        ground: [sx, (height - 0.5).floor() as i64, sz],
                    };
                    if spawn.is_valid(terrain, &journal) {
                        found.spawn = Some(spawn);
                        break 'landing;
                    }
                }
            }
        }
        Some(found)
    };
    if let Some(found) = sample(origin[0], origin[1]) {
        return Some(found);
    }
    for ring in 1..=SEARCH_RINGS {
        for offset in -ring..=ring {
            for [dx, dz] in [[offset, -ring], [offset, ring]] {
                if let Some(found) =
                    sample(origin[0] + dx * SEARCH_STEP, origin[1] + dz * SEARCH_STEP)
                {
                    return Some(found);
                }
            }
        }
        for offset in (-ring + 1)..ring {
            for [dx, dz] in [[-ring, offset], [ring, offset]] {
                if let Some(found) =
                    sample(origin[0] + dx * SEARCH_STEP, origin[1] + dz * SEARCH_STEP)
                {
                    return Some(found);
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_are_explicit_and_support_multiword_biomes() {
        assert_eq!(
            parse_command("/locate biome desert"),
            Ok(Some(BiomeKind::Desert))
        );
        assert_eq!(
            parse_command("/locate biome Mushroom Park"),
            Ok(Some(BiomeKind::MushroomPark))
        );
        assert!(parse_command("/locate desert").is_err());
        assert!(parse_command("/teleport 1 2 3").is_err());
        assert!(parse_command("/locate biome desert extra").is_err());
    }

    #[test]
    fn locating_is_deterministic_and_does_not_fabricate_disabled_biomes() {
        let terrain = TerraGenerator::new(terra_core::WorldConfig {
            biomes: false,
            caves: false,
            trees: false,
            water: false,
            hill_amplitude: 0.0,
            mountain_amplitude: 0.0,
            ..default()
        })
        .unwrap();
        let a = locate(&terrain, [-32, -48], BiomeKind::Forest).unwrap();
        let b = locate(&terrain, [-32, -48], BiomeKind::Forest).unwrap();
        assert_eq!(a.position, b.position);
        assert_eq!(a.spawn, b.spawn);
        assert!(a.spawn.is_some());
        assert!(locate(&terrain, [-32, -48], BiomeKind::Desert).is_none());
    }
}
