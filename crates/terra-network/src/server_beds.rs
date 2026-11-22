//! Transient sleep votes and atomic flower-dye use on persistent paired beds.
use super::*;
use terra_core::{
    ItemKind, WorldClock,
    beds::{bed_color, dyed_facing, is_bed},
};
use terra_world::{edits::BlockObjectState, streaming::commit_object_state};

struct SleepVote {
    anchor: [i64; 3],
    material: u16,
    facing: u8,
    expires: Instant,
    entered: Instant,
}
#[derive(Resource, Default)]
pub(super) struct SleepRuntime {
    votes: HashMap<PlayerId, SleepVote>,
    notices: Vec<(PlayerId, String)>,
    published: Vec<SleepingPlayer>,
    sent_to: std::collections::HashSet<PlayerId>,
}
impl SleepRuntime {
    pub(super) fn blocks_movement(
        &mut self,
        player: PlayerId,
        input: CharacterInput,
        jump: bool,
    ) -> bool {
        if !self.is_sleeping(player) {
            return false;
        }
        if jump || input.direction.length_squared() > 0.001 {
            self.cancel(player);
            false
        } else {
            true
        }
    }
    #[cfg(test)]
    fn quorum_ready(&self, players: impl Iterator<Item = PlayerId>, now: Instant) -> bool {
        self.quorum_percentage(players, now, 100.0)
    }
    fn quorum_percentage(
        &self,
        players: impl Iterator<Item = PlayerId>,
        now: Instant,
        percentage: f64,
    ) -> bool {
        let mut count = 0_usize;
        let mut resting = 0_usize;
        for player in players {
            if self.votes.get(&player).is_some_and(|vote| {
                now.saturating_duration_since(vote.entered) >= Duration::from_secs(2)
            }) {
                resting += 1;
            }
            count += 1;
        }
        let required = (count as f64 * percentage / 100.0).ceil() as usize;
        count > 0 && resting >= required.max(1)
    }
    pub(super) fn is_sleeping(&self, player: PlayerId) -> bool {
        self.votes.contains_key(&player)
    }
    pub(super) fn occupies_bed(&self, anchor: [i64; 3]) -> bool {
        self.votes.values().any(|vote| vote.anchor == anchor)
    }
    fn restore_player(
        &self,
        player: PlayerId,
        mode: terra_core::GameMode,
        vitals: &mut terra_core::SurvivalVitals,
    ) {
        if self.is_sleeping(player) && mode == terra_core::GameMode::Survival && !vitals.dead() {
            vitals
                .effects
                .apply(terra_core::EffectKind::Regeneration, 5.0, 1);
        }
    }
    pub(super) fn cancel(&mut self, player: PlayerId) {
        if self.votes.remove(&player).is_some() {
            self.notice(player, "You left the bed.");
        }
    }
    fn notice(&mut self, player: PlayerId, message: impl Into<String>) {
        if self.notices.len() < 128 {
            self.notices.push((player, message.into()));
        }
    }
}
fn sleep_hour(clock: WorldClock, config: &GameConfig) -> f64 {
    if config.world.seasons.enabled {
        clock.season(config.world.seasons).season.sleep_hour()
    } else {
        19.0
    }
}
fn available(clock: WorldClock, config: &GameConfig) -> bool {
    clock.hour() >= sleep_hour(clock, config) || clock.hour() < 8.0
}
fn next_morning(clock: WorldClock) -> Option<WorldClock> {
    Some(WorldClock {
        day: if clock.hour() < 8.0 {
            clock.day
        } else {
            clock.day.checked_add(1)?
        },
        phase: 8.0 / 24.0,
        ..clock
    })
}
fn dye_inventory(
    inventory: &terra_core::Inventory,
    source: InventorySource,
    color: terra_core::beds::DyeColor,
    mode: terra_core::GameMode,
) -> Option<terra_core::Inventory> {
    let item = ItemKind::Dye(color);
    if inventory.revision != source.revision
        || inventory
            .slot(usize::from(source.slot))
            .is_none_or(|stack| stack.item != item)
    {
        return None;
    }
    let mut next = inventory.clone();
    if mode == terra_core::GameMode::Survival && !next.take_from(usize::from(source.slot), item) {
        return None;
    }
    Some(next)
}
pub(super) fn interact(
    position: [i64; 3],
    source: Option<InventorySource>,
    peer: &Peer,
    world: &mut ControlWorld,
) -> Result<bool, String> {
    let material = world
        .chunks
        .block_at(position)
        .filter(|m| is_bed(*m))
        .ok_or("Missing bed")?;
    let state = world
        .journal
        .object_state(position)
        .ok_or("Missing bed state")?;
    let partner = state.peer.ok_or("Missing bed partner")?;
    let other = world
        .journal
        .object_state(partner)
        .ok_or("Missing bed partner state")?;
    if world.chunks.block_at(partner) != Some(material)
        || other.peer != Some(position)
        || other.secondary == state.secondary
        || other.facing != state.facing
    {
        return Err("Invalid bed partner".into());
    }
    let held = if let Some(source) = source {
        let bag = world
            .journal
            .gameplay()
            .inventories
            .get(&peer.uuid)
            .ok_or("Missing inventory")?;
        if bag.revision != source.revision {
            return Ok(false);
        }
        bag.slot(usize::from(source.slot)).map(|s| s.item)
    } else {
        None
    };
    if let Some(ItemKind::Dye(color)) = held {
        let current = world
            .journal
            .tint_at(position)
            .map(terra_core::beds::DyeColor::Rgb)
            .unwrap_or_else(|| bed_color(material, state.facing));
        if current == color {
            return Ok(false);
        }
        for cell in [position, partner] {
            let section = terra_voxel::global_to_section(cell).0;
            if world.journal.revision(section) > u64::MAX - 16
                || world.journal.appearance_epoch(section) > u64::MAX - 16
                || world.journal.fluid_epoch(section) > u64::MAX - 16
            {
                return Err("Bed state revision exhausted".into());
            }
            for x in -1..=1 {
                for y in -1..=1 {
                    for z in -1..=1 {
                        let neighbor = section.offset([x, y, z]).map_err(|e| e.to_string())?;
                        if world.journal.mesh_epoch(neighbor) > u64::MAX - 32
                            || world.journal.water_mesh_epoch(neighbor) > u64::MAX - 32
                        {
                            return Err("Bed mesh revision exhausted".into());
                        }
                    }
                }
            }
        }
        let inventory = world
            .journal
            .gameplay()
            .inventories
            .get(&peer.uuid)
            .ok_or("Missing inventory")?;
        if inventory.revision == u64::MAX || world.journal.gameplay().revision == u64::MAX {
            return Err("Inventory revision exhausted".into());
        }
        let Some(inventory) = dye_inventory(
            inventory,
            source.ok_or("Missing dye source")?,
            color,
            peer.game_mode(&world.config),
        ) else {
            return Ok(false);
        };
        for (cell, old) in [(position, state), (partner, other)] {
            commit_object_state(
                &mut world.chunks,
                &mut world.journal,
                cell,
                Some(BlockObjectState {
                    facing: dyed_facing(old.facing, color),
                    ..old
                }),
            )?;
            world.journal.set_tint(
                cell,
                match color {
                    terra_core::beds::DyeColor::Rgb(rgb) => Some(rgb),
                    _ => None,
                },
            )?;
        }
        if peer.game_mode(&world.config) == terra_core::GameMode::Survival {
            let game = world.journal.gameplay_mut();
            game.inventories.insert(peer.uuid, inventory);
            game.revision += 1;
        }
        return Ok(true);
    }
    let anchor = if state.secondary { partner } else { position };
    let Some(sleep) = world.sleep.as_mut() else {
        return Err("Sleep unavailable".into());
    };
    if !available(*world.clock, &world.config) {
        sleep.notice(
            peer.id,
            format!(
                "Sleep is available from {:02}:00 until 08:00.",
                sleep_hour(*world.clock, &world.config) as u8
            ),
        );
        return Ok(true);
    }
    if sleep.votes.remove(&peer.id).is_some() {
        sleep.notice(peer.id, "You left the bed.");
        return Ok(true);
    }
    if sleep.votes.values().any(|vote| vote.anchor == anchor) {
        sleep.notice(peer.id, "This bed is occupied.");
        return Ok(true);
    }
    sleep.votes.insert(
        peer.id,
        SleepVote {
            anchor,
            material,
            facing: state.facing,
            expires: Instant::now() + Duration::from_secs(120),
            entered: Instant::now(),
        },
    );
    sleep.notice(peer.id, "Resting. Waiting for the other players to sleep.");
    Ok(true)
}

#[derive(SystemParam)]
pub(super) struct SleepWorld<'w, 's> {
    clock: ResMut<'w, WorldClock>,
    weather: ResMut<'w, terra_core::WorldWeather>,
    played: ResMut<'w, crate::world_time::PlayedDays>,
    config: Res<'w, GameConfig>,
    journal: ResMut<'w, EditJournal>,
    chunks: Res<'w, ChunkStore>,
    sleep: ResMut<'w, SleepRuntime>,
    chat: ResMut<'w, chat::ChatRuntime>,
    bodies: Query<'w, 's, &'static Transform, With<ServerCharacter>>,
}
pub(super) fn tick(
    mut peers: Query<(&mut Peer, &mut MessageSender<ServerControl>), With<Connected>>,
    mut world: SleepWorld,
) {
    if world.sleep.votes.is_empty()
        && world.sleep.notices.is_empty()
        && world.sleep.published.is_empty()
    {
        return;
    }
    let eligible: Vec<_> = peers
        .iter()
        .filter_map(|(peer, _)| {
            (peer.synced
                && !peer.vitals.dead()
                && peer.chat.teleport.is_none()
                && peer.game_mode(&world.config) != terra_core::GameMode::Spectator)
                .then_some((peer.id, peer.body))
        })
        .collect();
    let now = Instant::now();
    let invalid: Vec<_> = world
        .sleep
        .votes
        .iter()
        .filter_map(|(id, vote)| {
            let valid = available(*world.clock, &world.config)
                && vote.expires > now
                && world.chunks.block_at(vote.anchor) == Some(vote.material)
                && world.journal.object_state(vote.anchor).is_some_and(|s| {
                    s.facing == vote.facing
                        && !s.secondary
                        && s.peer.is_some_and(|p| {
                            world.chunks.block_at(p) == Some(vote.material)
                                && world.journal.object_state(p).is_some_and(|o| {
                                    o.secondary
                                        && o.peer == Some(vote.anchor)
                                        && o.facing == s.facing
                                })
                        })
                })
                && eligible
                    .iter()
                    .find(|(player, _)| player == id)
                    .and_then(|(_, body)| world.bodies.get(*body).ok())
                    .is_some_and(|body| {
                        let center =
                            Vec3::from_array(vote.anchor.map(|v| v as f32)) + Vec3::splat(0.5);
                        body.translation.distance_squared(center) <= 6.0 * 6.0
                    });
            (!valid).then_some(*id)
        })
        .collect();
    for id in invalid {
        world.sleep.votes.remove(&id);
        world.sleep.notice(id, "You left the bed.");
    }
    if world.sleep.quorum_percentage(
        eligible.iter().map(|(id, _)| *id),
        now,
        world.config.world.game_rules.sleep_percentage,
    ) {
        if let Some(next) = next_morning(*world.clock) {
            let skipped = (next.day - world.clock.day) as f64 + next.phase - world.clock.phase;
            world.played.sleep(skipped);
            *world.clock = next;
            let seed = world.config.world.seed.0;
            let seasons = world.config.world.seasons;
            world
                .weather
                .advance_seasonal(skipped * 1440.0, seed, next, seasons);
            world.journal.bypass_change_detection().clock = Some(next);
            world.journal.bypass_change_detection().weather = Some(*world.weather);
            for (mut peer, _) in &mut peers {
                let mode = peer.game_mode(&world.config);
                world.sleep.restore_player(peer.id, mode, &mut peer.vitals);
            }
            for (id, _) in &eligible {
                world.sleep.notice(*id, "You wake at 08:00.");
            }
        }
        world.sleep.votes.clear();
    }
    let mut snapshot: Vec<_> = world
        .sleep
        .votes
        .iter()
        .map(|(player, vote)| SleepingPlayer {
            player: *player,
            anchor: vote.anchor,
            facing: vote.facing % 4,
        })
        .collect();
    snapshot.sort_unstable_by_key(|s| s.player.0);
    let changed = snapshot != world.sleep.published;
    if changed {
        world.sleep.published = snapshot.clone();
    }
    let connected: std::collections::HashSet<_> = peers
        .iter()
        .filter_map(|(peer, _)| peer.synced.then_some(peer.id))
        .collect();
    world.sleep.sent_to.retain(|id| connected.contains(id));
    for (peer, mut sender) in &mut peers {
        if peer.synced && (changed || !world.sleep.sent_to.contains(&peer.id)) {
            sender.send::<ControlChannel>(ServerControl::SleepingPlayers(snapshot.clone()));
            world.sleep.sent_to.insert(peer.id);
        }
    }
    let notices = std::mem::take(&mut world.sleep.notices);
    for (id, message) in notices {
        if let Some((_, mut sender)) = peers.iter_mut().find(|(peer, _)| peer.id == id) {
            sender.send::<ControlChannel>(ServerControl::Chat(world.chat.entry(message)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dye_consumes_only_the_selected_stack_and_creative_preserves_every_stack() {
        use terra_core::{GameMode, Inventory, ItemStack, beds::DyeColor};
        let item = ItemKind::Dye(DyeColor::Purple);
        let bag = Inventory {
            slots: vec![
                ItemStack { item, count: 8 },
                ItemStack::EMPTY,
                ItemStack { item, count: 2 },
            ],
            revision: 7,
            ..Default::default()
        };
        let source = InventorySource {
            slot: 2,
            revision: 7,
        };
        let next = dye_inventory(&bag, source, DyeColor::Purple, GameMode::Survival).unwrap();
        assert_eq!(next.slot(0).unwrap().count, 8);
        assert_eq!(next.slot(2).unwrap().count, 1);
        assert_eq!(next.revision, 8);
        assert_eq!(
            dye_inventory(&bag, source, DyeColor::Purple, GameMode::Creative).unwrap(),
            bag
        );
        assert!(
            dye_inventory(
                &bag,
                InventorySource { slot: 1, ..source },
                DyeColor::Purple,
                GameMode::Survival
            )
            .is_none()
        );
        assert!(
            dye_inventory(
                &bag,
                InventorySource {
                    revision: 6,
                    ..source
                },
                DyeColor::Purple,
                GameMode::Survival
            )
            .is_none()
        );
        assert!(dye_inventory(&bag, source, DyeColor::Blue, GameMode::Survival).is_none());
    }
    fn vote(now: Instant, anchor: [i64; 3]) -> SleepVote {
        SleepVote {
            anchor,
            material: 71,
            facing: 0,
            entered: now,
            expires: now + Duration::from_secs(120),
        }
    }
    #[test]
    fn successful_rest_restores_only_living_survival_voters_for_five_seconds() {
        use terra_core::{EffectKind, GameMode, SurvivalVitals};
        let now = Instant::now();
        let mut sleep = SleepRuntime::default();
        sleep.votes.insert(PlayerId(1), vote(now, [0, 1, 0]));
        for (player, mode, health, expected) in [
            (1, GameMode::Survival, 40.0, true),
            (2, GameMode::Survival, 40.0, false),
            (1, GameMode::Creative, 40.0, false),
            (1, GameMode::Spectator, 40.0, false),
            (1, GameMode::Survival, 0.0, false),
        ] {
            let mut vitals = SurvivalVitals {
                health,
                ..Default::default()
            };
            sleep.restore_player(PlayerId(player), mode, &mut vitals);
            assert_eq!(vitals.effects.contains(EffectKind::Regeneration), expected);
            if expected {
                assert_eq!(
                    vitals
                        .effects
                        .get(EffectKind::Regeneration)
                        .unwrap()
                        .remaining_seconds,
                    5.0
                );
            }
        }
        sleep.cancel(PlayerId(1));
        let mut vitals = SurvivalVitals::default();
        sleep.restore_player(PlayerId(1), GameMode::Survival, &mut vitals);
        assert!(!vitals.effects.contains(EffectKind::Regeneration));
    }
    #[test]
    fn multiplayer_waits_for_every_eligible_player_and_two_seconds_in_bed() {
        let now = Instant::now();
        let mut sleep = SleepRuntime::default();
        sleep.votes.insert(PlayerId(1), vote(now, [0, 1, 0]));
        assert!(!sleep.quorum_ready([PlayerId(1)].into_iter(), now + Duration::from_millis(1999)));
        assert!(sleep.quorum_ready([PlayerId(1)].into_iter(), now + Duration::from_secs(2)));
        assert!(!sleep.quorum_ready(
            [PlayerId(1), PlayerId(2)].into_iter(),
            now + Duration::from_secs(3)
        ));
        sleep
            .votes
            .insert(PlayerId(2), vote(now + Duration::from_secs(1), [2, 1, 0]));
        assert!(!sleep.quorum_ready(
            [PlayerId(1), PlayerId(2)].into_iter(),
            now + Duration::from_millis(2999)
        ));
        assert!(sleep.quorum_ready(
            [PlayerId(1), PlayerId(2)].into_iter(),
            now + Duration::from_secs(3)
        ));
        assert!(!sleep.quorum_ready([].into_iter(), now + Duration::from_secs(3)));
    }
    #[test]
    fn configurable_sleep_votes_round_up_and_zero_still_needs_one_resting_player() {
        let now = Instant::now();
        let mut sleep = SleepRuntime::default();
        sleep.votes.insert(PlayerId(1), vote(now, [0, 1, 0]));
        let players = || [PlayerId(1), PlayerId(2), PlayerId(3)].into_iter();
        assert!(!sleep.quorum_percentage(players(), now, 0.0));
        assert!(sleep.quorum_percentage(players(), now + Duration::from_secs(2), 0.0));
        assert!(sleep.quorum_percentage(players(), now + Duration::from_secs(2), 30.0));
        assert!(!sleep.quorum_percentage(players(), now + Duration::from_secs(2), 50.0));
        sleep.votes.insert(PlayerId(2), vote(now, [2, 1, 0]));
        assert!(sleep.quorum_percentage(players(), now + Duration::from_secs(2), 50.0));
    }
    #[test]
    fn authority_freezes_resting_players_and_movement_or_jump_leaves_the_bed() {
        let now = Instant::now();
        let mut sleep = SleepRuntime::default();
        sleep.votes.insert(PlayerId(1), vote(now, [0, 1, 0]));
        assert!(sleep.blocks_movement(PlayerId(1), CharacterInput::default(), false));
        assert!(sleep.is_sleeping(PlayerId(1)));
        assert!(!sleep.blocks_movement(
            PlayerId(1),
            CharacterInput {
                direction: Vec3::X,
                ..Default::default()
            },
            false
        ));
        assert!(!sleep.is_sleeping(PlayerId(1)));
        sleep.votes.insert(PlayerId(1), vote(now, [0, 1, 0]));
        assert!(!sleep.blocks_movement(PlayerId(1), CharacterInput::default(), true));
        assert!(!sleep.is_sleeping(PlayerId(1)));
    }
    #[test]
    fn sleeping_targets_the_next_morning_without_skipping_an_extra_day_after_midnight() {
        for (hour, day) in [(19.0, 8), (23.0, 8), (2.0, 7)] {
            let clock = WorldClock {
                day: 7,
                phase: hour / 24.0,
                ..Default::default()
            };
            let next = next_morning(clock).unwrap();
            assert_eq!(next.day, day);
            assert_eq!(next.hour(), 8.0);
            let mut played = crate::world_time::PlayedDays::default();
            played.sleep((next.day - clock.day) as f64 + next.phase - clock.phase);
            assert!(played.slept);
            assert!((played.take() - (8.0 - hour).rem_euclid(24.) / 24.).abs() < 1e-12);
            assert!(!played.slept);
            assert_eq!(played.take(), 0.);
        }
        assert!(
            next_morning(WorldClock {
                day: u64::MAX,
                phase: 0.9,
                ..Default::default()
            })
            .is_none()
        );
    }
    #[test]
    fn each_season_uses_its_own_evening_threshold() {
        let config = GameConfig::default();
        for (day, hour) in [(0, 19.0), (7, 20.0), (14, 19.0), (21, 18.0)] {
            let clock = WorldClock {
                day,
                phase: hour / 24.0,
                ..Default::default()
            };
            assert!(available(clock, &config));
            assert!(!available(
                WorldClock {
                    phase: (hour - 0.1) / 24.0,
                    ..clock
                },
                &config
            ));
        }
    }

    #[test]
    fn disabling_seasons_keeps_a_fixed_evening_threshold() {
        let mut config = GameConfig::default();
        config.world.seasons.enabled = false;
        for day in [0, 7, 14, 21, 28, 365] {
            let clock = WorldClock {
                day,
                phase: 19.0 / 24.0,
                ..Default::default()
            };
            assert_eq!(sleep_hour(clock, &config), 19.0);
            assert!(available(clock, &config));
            assert!(!available(
                WorldClock {
                    phase: 18.9 / 24.0,
                    ..clock
                },
                &config
            ));
            assert!(available(
                WorldClock {
                    phase: 3.0 / 24.0,
                    ..clock
                },
                &config
            ));
        }
    }
}
