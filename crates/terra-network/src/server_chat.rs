//! Bounded command work and server-issued, permission-checked teleport targets.
use super::*;
use crate::protocol::chat::{ChatEntry, LocateResult, sanitize};
use bevy::tasks::{AsyncComputeTaskPool, Task, futures_lite::future};
use terra_world::{
    TerraGenerator,
    generation::{CaveEcology, biomes::BiomeKind, structures::StructureKind},
    spawn::WorldSpawn,
};

const SEARCH_STEP: i64 = 96;
const SEARCH_RINGS: i64 = 64;
const JOB_LIMIT: usize = 4;

#[path = "server_cave_locate.rs"]
mod cave_locate;
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
    ground: GroundCheck,
    creative_only: bool,
    respawn: bool,
    privileged: bool,
}
impl PendingTeleport {
    fn allows_mode(&self, mode: terra_core::GameMode) -> bool {
        !self.creative_only || mode == terra_core::GameMode::Creative
    }

    pub(super) fn respawn(position: Vec3) -> Self {
        Self {
            position,
            started: Instant::now(),
            deltas_ready: false,
            ground: GroundCheck::Surface,
            creative_only: false,
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
    targets: VecDeque<(u64, LocateLanding)>,
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
    target: LocateTarget,
    label: Option<String>,
    position: [i64; 3],
    spawn: Option<WorldSpawn>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LocateTarget {
    Biome(BiomeKind),
    Cave(CaveEcology),
    Structure(StructureKind),
}

impl LocateTarget {
    fn name(self) -> &'static str {
        match self {
            Self::Biome(kind) => biome_name(kind),
            Self::Cave(CaveEcology::Salt) => "Salt Cave",
            Self::Cave(CaveEcology::Lush) => "Lush Cave",
            Self::Cave(CaveEcology::Dripstone) => "Dripstone Cave",
            Self::Cave(CaveEcology::Waterfall) => "Ice Cave",
            Self::Cave(_) => "Cave",
            Self::Structure(StructureKind::Village) => "Village",
            Self::Structure(StructureKind::Ruin) => "Ruin",
            Self::Structure(StructureKind::Mine) => "Mine",
            Self::Structure(StructureKind::Kingdom) => "Kingdom",
            Self::Structure(StructureKind::Outpost) => "Outpost",
            Self::Structure(StructureKind::Cemetery) => "Cemetery",
            Self::Structure(StructureKind::GraveDungeon) => "Grave Dungeon",
            Self::Structure(StructureKind::DarkDragonLair) => "Dark Dragon Lair",
            Self::Structure(StructureKind::EarthDragonLair) => "Earth Dragon Lair",
            Self::Structure(StructureKind::WaterDragonLair) => "Water Dragon Lair",
        }
    }
}

#[derive(Clone, Copy)]
enum GroundCheck {
    Surface,
    Cave(WorldSpawn),
    Air,
}

#[derive(Clone, Copy)]
enum LocateLanding {
    Dry(WorldSpawn),
    Cave(WorldSpawn),
    Creative([i64; 3]),
}

impl LocateLanding {
    fn permits(self, mode: terra_core::GameMode) -> bool {
        !matches!(self, Self::Creative(_)) || mode == terra_core::GameMode::Creative
    }

    fn feet(self) -> [i64; 3] {
        match self {
            Self::Dry(spawn) | Self::Cave(spawn) => {
                [spawn.ground[0], spawn.ground[1] + 1, spawn.ground[2]]
            }
            Self::Creative(feet) => feet,
        }
    }

    fn is_valid(self, terrain: &TerraGenerator, journal: &EditJournal) -> bool {
        match self {
            Self::Dry(spawn) => spawn.is_valid(terrain, journal),
            Self::Cave(spawn) => cave_locate::clear(terrain, journal, spawn),
            Self::Creative(feet) => creative_clearance(terrain, journal, feet),
        }
    }
}

struct PendingEffect {
    actor: PlayerId,
    target: PlayerId,
    kind: terra_core::EffectKind,
    seconds: f32,
    level: u8,
    console: bool,
}

#[derive(Resource, Default)]
pub(super) struct ChatRuntime {
    jobs: HashMap<PlayerId, LocateJob>,
    next_id: u64,
    console_reply: bool,
    console_lines: VecDeque<String>,
    console_job: Option<LocateJob>,
    effects: Vec<PendingEffect>,
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
        if rule == terra_core::GameRule::Pvp && self.config.server.pvp.is_some() {
            return Err(
                "PvP is controlled by pvp in server.toml; omit it to use the world gamerule",
            );
        }
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
        if self.clock.paused || !self.config.world.game_rules.weather_cycle {
            // An explicit admin edit still takes effect when automatic time or weather is frozen.
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
                    let target = match parse_command(&text) {
                        Ok(Some(target)) => target,
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
                            "A locate search is already running.",
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
                                .spawn(async move { locate(&terrain, origin, target) }),
                            started: Instant::now(),
                            console: peer.chat.console_active,
                        },
                    );
                    reply(
                        &mut world.runtime,
                        &mut sender,
                        format!("Searching for {}...", target.name()),
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
                            "This teleport target has expired. Run locate again.",
                        );
                        continue;
                    };
                    if !target.permits(peer.game_mode(&world.config))
                        || !target.is_valid(&world.terrain.0, &world.journal)
                    {
                        reply(
                            &mut world.runtime,
                            &mut sender,
                            "The destination is no longer safe. Run locate again.",
                        );
                        continue;
                    }
                    peer.chat.targets.retain(|(id, _)| *id != token);
                    peer.chat.teleport = Some(PendingTeleport {
                        respawn: false,
                        position: Vec3::from_array(target.feet().map(|p| p as f32))
                            + Vec3::new(0.5, 0.0, 0.5)
                            + Vec3::Y * (terra_physics::character::PLAYER_HALF_HEIGHT + 0.02),
                        started: Instant::now(),
                        deltas_ready: false,
                        ground: match target {
                            LocateLanding::Cave(spawn) => GroundCheck::Cave(spawn),
                            LocateLanding::Dry(_) => GroundCheck::Surface,
                            LocateLanding::Creative(_) => GroundCheck::Air,
                        },
                        creative_only: matches!(target, LocateLanding::Creative(_)),
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
    apply_pending_effects(&mut peers, &mut world);
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
                "Locate search timed out. Try again from another area."
            } else {
                "No matching biome or structure was found within 6144 blocks. Try another location or world settings."
            },
        );
        return;
    };
    let mut entry = world.runtime.entry("");
    let cave = matches!(found.target, LocateTarget::Cave(_));
    let safe = found
        .spawn
        .map(|spawn| {
            if cave {
                LocateLanding::Cave(spawn)
            } else {
                LocateLanding::Dry(spawn)
            }
        })
        .filter(|target| target.is_valid(&world.terrain.0, &world.journal));
    let landing = safe.or_else(|| {
        if cave {
            return (found.spawn.is_none()
                && peer.game_mode(&world.config) == terra_core::GameMode::Creative
                && creative_clearance(&world.terrain.0, &world.journal, found.position))
            .then_some(LocateLanding::Creative(found.position));
        }
        (peer.game_mode(&world.config) == terra_core::GameMode::Creative).then(|| {
            LocateLanding::Creative(creative_landing(
                &world.terrain.0,
                &world.journal,
                found.position,
            ))
        })
    });
    let token = if can_teleport(peer, &world.config) {
        landing.map(|landing| {
            if peer.chat.targets.len() == 8 {
                peer.chat.targets.pop_front();
            }
            peer.chat.targets.push_back((entry.id, landing));
            entry.id
        })
    } else {
        None
    };
    entry.locate = Some(LocateResult {
        name: found
            .label
            .clone()
            .unwrap_or_else(|| found.target.name().into()),
        position: landing.map_or(found.position, LocateLanding::feet),
        token,
    });
    if world.runtime.console_reply {
        let [x, y, z] = found.position;
        world.runtime.console_lines.push_back(format!(
            "{} found at {x}, {y}, {z}.",
            found.label.as_deref().unwrap_or(found.target.name())
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
    let state = match pending.ground {
        GroundCheck::Cave(spawn) => {
            cave_locate::loaded(&world.chunks, &world.terrain.0, &world.journal, spawn)
        }
        GroundCheck::Surface => {
            spawn::loaded_state(&world.chunks, &world.terrain.0, &world.journal, position)
        }
        GroundCheck::Air => teleport_space(&world.chunks, position),
    };
    let require_ground = !matches!(pending.ground, GroundCheck::Air);
    let respawn = pending.respawn;
    let expired = pending.started.elapsed() > Duration::from_secs(25);
    let permitted = (if respawn {
        peer.vitals.dead()
    } else {
        pending.privileged || can_teleport(peer, &world.config)
    }) && pending.allows_mode(peer.game_mode(&world.config));
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
        peer.vitals.apply_equipment(peer.equipment);
        peer.vitals.health = peer.vitals.max_health;
        peer.vitals.stamina = peer.vitals.max_stamina;
    }
    peer.chat.teleport = None;
    sender.send::<ControlChannel>(ServerControl::Teleport {
        tick: world.server.tick,
        state: Box::new(PlayerState {
            kingdom_heraldry: world
                .journal
                .gameplay()
                .kingdoms
                .heraldry_for_member(peer.uuid),
            gender: peer.gender,
            appearance: peer.appearance,
            generation,
            player: peer.id,
            position: position.to_array(),
            grounded: require_ground,
            look: peer.look,
            survival: peer.vitals,
            equipment: peer.equipment,
            ..default()
        }),
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
            let result = give_effect(peer, &world.config, kind, seconds, level);
            reply(
                &mut world.runtime,
                sender,
                result.unwrap_or_else(str::to_owned),
            );
            return;
        }
        Command::EffectPlayer(ref name, kind, seconds, level) => {
            let target = effect_target(name, &world.identities);
            match target {
                Ok(target)
                    if target != peer.id
                        && !peer.chat.console_active
                        && !peer.permissions.environment =>
                {
                    reply(
                        &mut world.runtime,
                        sender,
                        "Affecting another player requires environment permission.",
                    );
                }
                Ok(target) if world.runtime.effects.len() < 64 => {
                    world.runtime.effects.push(PendingEffect {
                        actor: peer.id,
                        target,
                        kind,
                        seconds,
                        level,
                        console: peer.chat.console_active,
                    })
                }
                Ok(_) => reply(
                    &mut world.runtime,
                    sender,
                    "The effect queue is busy. Try again shortly.",
                ),
                Err(error) => reply(&mut world.runtime, sender, error),
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
        | Command::EffectPlayer(..)
        | Command::EffectClear(_) => return,
    };
    peer.chat.teleport = Some(PendingTeleport {
        respawn: false,
        position,
        started: Instant::now(),
        deltas_ready: false,
        ground: GroundCheck::Air,
        creative_only: false,
        privileged: peer.chat.console_active,
    });
    peer.mining = None;
    sender.send::<ControlChannel>(ServerControl::Mining(None));
    sender.send::<ControlChannel>(ServerControl::TeleportPreparing);
}

fn effect_target(name: &str, identities: &ConnectedProfiles) -> Result<PlayerId, &'static str> {
    let mut players = identities
        .0
        .iter()
        .filter(|(_, profile)| profile.name.eq_ignore_ascii_case(name));
    let id = players
        .next()
        .map(|(id, _)| *id)
        .ok_or("Player is not online.")?;
    if players.next().is_some() {
        return Err("Player name is ambiguous.");
    }
    Ok(id)
}

fn give_effect(
    peer: &mut Peer,
    config: &GameConfig,
    kind: terra_core::EffectKind,
    seconds: f32,
    level: u8,
) -> Result<String, &'static str> {
    if peer.vitals.dead()
        || (peer.game_mode(config) != terra_core::GameMode::Survival
            && kind != terra_core::EffectKind::NightVision)
    {
        return Err(
            "Status effects require a living Survival player; Night Vision also works in Creative and Spectator.",
        );
    }
    peer.vitals.effects.apply(kind, seconds, level);
    Ok(format!(
        "Applied {} {level} for {seconds}s.",
        kind.identifier()
    ))
}

fn apply_pending_effects(
    peers: &mut Query<(&mut Peer, &mut MessageSender<ServerControl>), With<Connected>>,
    world: &mut ChatWorld,
) {
    for effect in std::mem::take(&mut world.runtime.effects) {
        let mut result = Err("Player is not online or is still loading.");
        for (mut peer, _) in peers.iter_mut() {
            if peer.id == effect.target && peer.synced {
                result = give_effect(
                    &mut peer,
                    &world.config,
                    effect.kind,
                    effect.seconds,
                    effect.level,
                );
                break;
            }
        }
        world.runtime.console_reply = effect.console;
        for (peer, mut sender) in peers.iter_mut() {
            if peer.id == effect.actor {
                reply(
                    &mut world.runtime,
                    &mut sender,
                    result.unwrap_or_else(str::to_owned),
                );
                break;
            }
        }
    }
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
            peer.vitals.apply_equipment(peer.equipment);
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
        stats.equipment = peer.equipment;
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
            state: Box::new(PlayerState {
                kingdom_heraldry: world
                    .journal
                    .gameplay()
                    .kingdoms
                    .heraldry_for_member(peer.uuid),
                gender: peer.gender,
                appearance: peer.appearance,
                generation,
                player: peer.id,
                position: body.translation.to_array(),
                grounded: grounded.0,
                look: peer.look,
                survival: peer.vitals,
                equipment: peer.equipment,
                ..default()
            }),
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
        BiomeKind::CherryForest => "Cherry Forest",
        BiomeKind::SpiritBlossom => "Spirit Blossom",
        BiomeKind::BloodForest => "Blood Forest",
    }
}

fn parse_command(text: &str) -> Result<Option<LocateTarget>, &'static str> {
    let parts: Vec<_> = text.split_whitespace().collect();
    if parts == ["/help"] {
        return Ok(None);
    }
    if parts.len() < 3 || parts[0] != "/locate" {
        return Err("Usage: /locate biome <name> | /locate struct teramic::<structure>.");
    }
    if parts[1] == "struct" {
        if parts.len() != 3 {
            return Err(
                "Usage: /locate struct <teramic::village|teramic::ruin|teramic::mine|teramic::kingdom|teramic::outpost|teramic::cemetery|teramic::grave_dungeon|teramic::dark_dragon_lair|teramic::earth_dragon_lair|teramic::water_dragon_lair>.",
            );
        }
        return match parts[2].to_ascii_lowercase().as_str() {
            "teramic::village" => Ok(Some(LocateTarget::Structure(StructureKind::Village))),
            "teramic::ruin" => Ok(Some(LocateTarget::Structure(StructureKind::Ruin))),
            "teramic::mine" => Ok(Some(LocateTarget::Structure(StructureKind::Mine))),
            "teramic::kingdom" => Ok(Some(LocateTarget::Structure(StructureKind::Kingdom))),
            "teramic::outpost" => Ok(Some(LocateTarget::Structure(StructureKind::Outpost))),
            "teramic::cemetery" => Ok(Some(LocateTarget::Structure(StructureKind::Cemetery))),
            "teramic::dark_dragon_lair" => {
                Ok(Some(LocateTarget::Structure(StructureKind::DarkDragonLair)))
            }
            "teramic::earth_dragon_lair" => Ok(Some(LocateTarget::Structure(
                StructureKind::EarthDragonLair,
            ))),
            "teramic::water_dragon_lair" => Ok(Some(LocateTarget::Structure(
                StructureKind::WaterDragonLair,
            ))),
            "teramic::grave_dungeon" => {
                Ok(Some(LocateTarget::Structure(StructureKind::GraveDungeon)))
            }
            _ => Err(
                "Unknown structure. Use teramic::village, teramic::ruin, teramic::mine, teramic::kingdom, teramic::outpost, teramic::cemetery, teramic::grave_dungeon, teramic::dark_dragon_lair, teramic::earth_dragon_lair or teramic::water_dragon_lair.",
            ),
        };
    }
    if parts[1] != "biome" {
        return Err("Usage: /locate biome <name> | /locate struct teramic::<structure>.");
    }
    let name = parts[2..].join("_").to_ascii_lowercase();
    if let Some((_, ecology)) = crate::protocol::commands::CAVE_BIOMES
        .iter()
        .find(|(slug, _)| *slug == name)
    {
        return Ok(Some(LocateTarget::Cave(*ecology)));
    }
    let biome = match name.as_str() {
        "ocean" => BiomeKind::Ocean,
        "plains" => BiomeKind::Plains,
        "forest" => BiomeKind::Forest,
        "desert" => BiomeKind::Desert,
        "highland" => BiomeKind::Highland,
        "mushroom_park" | "mushroompark" => BiomeKind::MushroomPark,
        "cliffted_desert" | "cliffteddesert" => BiomeKind::ClifftedDesert,
        "cherry_forest" | "cherryforest" => BiomeKind::CherryForest,
        "spirit_blossom" | "spiritblossom" | "spirit-blossom" => BiomeKind::SpiritBlossom,
        "blood_forest" | "bloodforest" | "blood-forest" | "blutwald" => BiomeKind::BloodForest,
        _ => return Err("Unknown biome. Use /help for biome names."),
    };
    Ok(Some(LocateTarget::Biome(biome)))
}

fn locate(terrain: &TerraGenerator, origin: [i64; 2], target: LocateTarget) -> Option<Located> {
    match target {
        LocateTarget::Cave(kind) => cave_locate::find(terrain, origin, kind),
        LocateTarget::Biome(biome) => locate_biome(terrain, origin, biome),
        LocateTarget::Structure(kind) => locate_structure(terrain, origin, kind),
    }
}

fn locate_biome(terrain: &TerraGenerator, origin: [i64; 2], biome: BiomeKind) -> Option<Located> {
    if !terrain.config().biomes && biome != BiomeKind::Forest {
        return None;
    }
    let sample = |x: i64, z: i64| {
        if x.abs() > 990_000
            || z.abs() > 990_000
            || terrain.surface_biome(&terrain.surface_column(x as f64 + 0.5, z as f64 + 0.5))
                != biome
        {
            return None;
        }
        let height = terrain.surface_height(x as f64 + 0.5, z as f64 + 0.5);
        let mut found = Located {
            target: LocateTarget::Biome(biome),
            label: None,
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
                        .surface_biome(&terrain.surface_column(sx as f64 + 0.5, sz as f64 + 0.5))
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

fn locate_structure(
    terrain: &TerraGenerator,
    origin: [i64; 2],
    kind: StructureKind,
) -> Option<Located> {
    use terra_world::generation::structures::REGION_SIZE;
    if terrain.config().structures_revision == 0
        || origin.iter().any(|v| v.unsigned_abs() > 990_000)
    {
        return None;
    }
    let region = origin.map(|v| v.div_euclid(REGION_SIZE));
    let radius = SEARCH_STEP * SEARCH_RINGS;
    let deadline = Instant::now() + Duration::from_secs(12);
    let mut best: Option<(i64, [i64; 3], Option<String>)> = None;
    'search: for ring in 0_i64..=radius / REGION_SIZE + 1 {
        for dz in -ring..=ring {
            for dx in -ring..=ring {
                if dx.abs().max(dz.abs()) != ring {
                    continue;
                }
                // Planning is immutable, cached and off the main thread. Even a
                // sparse/disabled kind must not leave a cancelled worker scanning.
                if Instant::now() >= deadline {
                    break 'search;
                }
                let candidate = [region[0] + dx, region[1] + dz];
                if matches!(kind, StructureKind::Kingdom | StructureKind::Outpost)
                    && terrain.royal_site_candidate(candidate) != Some(kind)
                {
                    continue;
                }
                if kind.dragon().is_some() && !terrain.dragon_lair_candidate(candidate) {
                    continue;
                }
                if matches!(kind, StructureKind::Cemetery | StructureKind::GraveDungeon)
                    && terrain.grave_site_candidate(candidate) != Some(kind)
                {
                    continue;
                }
                let Some(site) = terrain.structure_site(candidate) else {
                    continue;
                };
                if site.kind != kind
                    || site.center[0].unsigned_abs() > 990_000
                    || site.center[2].unsigned_abs() > 990_000
                {
                    continue;
                }
                let distance =
                    (site.center[0] - origin[0]).pow(2) + (site.center[2] - origin[1]).pow(2);
                if distance <= radius.pow(2)
                    && best
                        .as_ref()
                        .is_none_or(|(previous, _, _)| distance < *previous)
                {
                    let position = site
                        .buildings
                        .first()
                        .map_or(site.center, |building| building.entrance);
                    let label = site
                        .grave_dungeon()
                        .map(|info| format!("Tomb of {}", info.name));
                    best = Some((distance, position, label));
                }
            }
        }
        // Any unvisited region is at least this far from the origin. This
        // conservatively checks neighboring regions before returning a winner.
        if best.as_ref().is_some_and(|(distance, _, _)| {
            ring > 1 && *distance < ((ring - 1) * REGION_SIZE).pow(2)
        }) {
            break;
        }
    }
    let (_, position, label) = best?;
    let spawn = nearby_dry_landing(terrain, position);
    Some(Located {
        target: LocateTarget::Structure(kind),
        label,
        position,
        spawn,
    })
}

fn nearby_dry_landing(terrain: &TerraGenerator, position: [i64; 3]) -> Option<WorldSpawn> {
    let journal = EditJournal::default();
    for ring in 0_i64..=6 {
        for dz in -ring..=ring {
            for dx in -ring..=ring {
                if dx.abs().max(dz.abs()) != ring {
                    continue;
                }
                let [x, z] = [position[0] + dx * 4, position[2] + dz * 4];
                let height = terrain.surface_height(x as f64 + 0.5, z as f64 + 0.5);
                let spawn = WorldSpawn {
                    ground: [x, (height - 0.5).floor() as i64, z],
                };
                if spawn.is_valid(terrain, &journal) {
                    return Some(spawn);
                }
            }
        }
    }
    None
}

fn creative_clearance(terrain: &TerraGenerator, journal: &EditJournal, feet: [i64; 3]) -> bool {
    let generator = terra_world::SectionGenerator::new(terrain.clone());
    (0..3).all(|height| {
        let p = [feet[0], feet[1] + height, feet[2]];
        journal
            .block(p)
            .or_else(|| generator.block_at(p).ok())
            .and_then(terra_voxel::MaterialId::from_raw)
            .is_some_and(|material| !material.is_solid())
    })
}

fn creative_landing(
    terrain: &TerraGenerator,
    journal: &EditJournal,
    position: [i64; 3],
) -> [i64; 3] {
    for rise in 0..=128 {
        let feet = [position[0], position[1] + rise, position[2]];
        if creative_clearance(terrain, journal, feet) {
            return feet;
        }
    }
    // A creative result always retains its capability. This high-air fallback
    // still goes through loaded clearance validation before moving the player.
    [
        position[0],
        if terrain.config().bounded_world {
            358
        } else {
            (terrain
                .surface_height(position[0] as f64 + 0.5, position[2] as f64 + 0.5)
                .ceil() as i64)
                .max(position[1])
                + 128
        },
        position[2],
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_are_explicit_and_support_multiword_biomes() {
        assert_eq!(
            parse_command("/locate biome desert"),
            Ok(Some(LocateTarget::Biome(BiomeKind::Desert)))
        );
        assert_eq!(
            parse_command("/locate biome Mushroom Park"),
            Ok(Some(LocateTarget::Biome(BiomeKind::MushroomPark)))
        );
        for name in [
            "Spirit Blossom",
            "spirit_blossom",
            "spirit-blossom",
            "spiritblossom",
        ] {
            assert_eq!(
                parse_command(&format!("/locate biome {name}")),
                Ok(Some(LocateTarget::Biome(BiomeKind::SpiritBlossom)))
            );
        }
        for name in [
            "Blood Forest",
            "blood_forest",
            "blood-forest",
            "bloodforest",
            "Blutwald",
        ] {
            assert_eq!(
                parse_command(&format!("/locate biome {name}")),
                Ok(Some(LocateTarget::Biome(BiomeKind::BloodForest)))
            );
        }
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
        let a = locate(&terrain, [-32, -48], LocateTarget::Biome(BiomeKind::Forest)).unwrap();
        let b = locate(&terrain, [-32, -48], LocateTarget::Biome(BiomeKind::Forest)).unwrap();
        assert_eq!(a.position, b.position);
        assert_eq!(a.spawn, b.spawn);
        assert!(a.spawn.is_some());
        assert!(locate(&terrain, [-32, -48], LocateTarget::Biome(BiomeKind::Desert)).is_none());
    }

    #[test]
    fn structure_commands_share_explicit_namespaced_ids_and_bounded_generation() {
        for (id, kind) in [
            ("teramic::village", StructureKind::Village),
            ("teramic::ruin", StructureKind::Ruin),
            ("teramic::mine", StructureKind::Mine),
            ("teramic::kingdom", StructureKind::Kingdom),
            ("teramic::outpost", StructureKind::Outpost),
            ("teramic::cemetery", StructureKind::Cemetery),
            ("teramic::grave_dungeon", StructureKind::GraveDungeon),
            ("teramic::dark_dragon_lair", StructureKind::DarkDragonLair),
            ("teramic::earth_dragon_lair", StructureKind::EarthDragonLair),
            ("teramic::water_dragon_lair", StructureKind::WaterDragonLair),
        ] {
            assert_eq!(
                parse_command(&format!("/locate struct {id}")),
                Ok(Some(LocateTarget::Structure(kind)))
            );
        }
        for command in [
            "/locate struct village",
            "/locate struct teramic::missing",
            "/locate struct teramic::village extra",
            "/locate struct",
        ] {
            assert!(parse_command(command).is_err(), "{command}");
        }
        let terrain = TerraGenerator::new(terra_core::WorldConfig {
            structures_revision: 0,
            ..default()
        })
        .unwrap();
        assert!(locate_structure(&terrain, [-385, -1], StructureKind::Village).is_none());
        assert!(locate_structure(&terrain, [i64::MIN, 0], StructureKind::Mine).is_none());
    }

    #[test]
    fn structure_results_use_real_plans_and_are_stable_at_negative_coordinates() {
        let terrain = TerraGenerator::new(terra_core::WorldConfig {
            seed: terra_core::WorldSeed(42042),
            landscape_revision: 1,
            caves: false,
            trees: false,
            structures_revision: 2,
            ..default()
        })
        .unwrap();
        let site = terrain
            .structure_site([2, -2])
            .expect("seeded village plan");
        assert_eq!(site.kind, StructureKind::Village);
        let origin = [site.center[0], site.center[2]];
        let a = locate_structure(&terrain, origin, StructureKind::Village).unwrap();
        let b = locate_structure(&terrain, origin, StructureKind::Village).unwrap();
        assert_eq!(a.position, site.buildings[0].entrance);
        assert_eq!(a.position, b.position);
        assert_eq!(a.spawn, b.spawn);
    }

    #[test]
    fn creative_locate_landings_accept_water_but_revalidate_edited_clearance() {
        let terrain = TerraGenerator::new(terra_core::WorldConfig {
            base_height: 0.0,
            hill_amplitude: 0.0,
            mountain_amplitude: 0.0,
            biomes: false,
            caves: false,
            trees: false,
            structures_revision: 0,
            water: true,
            sea_level: 12.0,
            ..default()
        })
        .unwrap();
        let journal = EditJournal::default();
        let found = locate_biome(&terrain, [-32, -48], BiomeKind::Forest).unwrap();
        assert!(found.spawn.is_none(), "no dry land in this flooded world");
        let landing = LocateLanding::Creative(creative_landing(&terrain, &journal, found.position));
        assert!(landing.is_valid(&terrain, &journal));
        assert_eq!([landing.feet()[0], landing.feet()[2]], [-32, -48]);
        assert!(landing.feet()[1] < 12, "Creative may land in water");
        let mut journal = journal;
        journal.record(landing.feet(), 3).unwrap();
        assert!(!landing.is_valid(&terrain, &journal));
    }

    #[test]
    fn creative_air_capabilities_are_not_reusable_after_a_mode_change_or_during_loading() {
        use terra_core::GameMode;
        let air = LocateLanding::Creative([-17, 12, -1]);
        assert!(air.permits(GameMode::Creative));
        assert!(!air.permits(GameMode::Survival));
        assert!(!air.permits(GameMode::Spectator));
        let dry = LocateLanding::Dry(WorldSpawn {
            ground: [-17, 11, -1],
        });
        assert!(
            dry.permits(GameMode::Survival),
            "ordinary permission checks still apply separately"
        );
        let mut pending = PendingTeleport::respawn(Vec3::ZERO);
        pending.respawn = false;
        pending.creative_only = true;
        assert!(pending.allows_mode(GameMode::Creative));
        assert!(!pending.allows_mode(GameMode::Survival));
        assert!(!pending.allows_mode(GameMode::Spectator));
        pending.creative_only = false;
        assert!(pending.allows_mode(GameMode::Survival));
    }
}
