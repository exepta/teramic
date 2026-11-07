//! Live authoritative vitals use the existing player journal and teleport lifecycle.
use super::*;
use terra_core::{DamageCause, GameMode, SurvivalVitals};

#[derive(Default)]
pub(super) struct SurvivalPeer {
    fall_peak: Option<f32>,
    pub respawn_requested: bool,
    pending_deaths: u64,
    pub(super) protection: f32,
    pub damage_epoch: u64,
}
impl SurvivalPeer {
    pub(super) fn teleported(&mut self, respawn: bool) {
        self.fall_peak = None;
        self.respawn_requested = false;
        if respawn {
            self.protection = 2.0;
        }
    }
}

pub(super) fn animal_attack(peer: &mut Peer, amount: f32, mode: GameMode) {
    environmental_damage(peer, amount, mode, DamageCause::Animal);
}

/// Resting blocks movement, not poison, breathing, effect expiry or death accounting.
pub(super) fn advance_rest(
    peer: &mut Peer,
    state: CharacterState,
    submerged: bool,
    swimming: bool,
    mode: GameMode,
    dt: f32,
) {
    advance(peer, state, state, submerged, swimming, mode, dt);
}

pub(super) fn environmental_damage(
    peer: &mut Peer,
    amount: f32,
    mode: GameMode,
    cause: DamageCause,
) {
    if mode != GameMode::Survival || peer.survival.protection > 0.0 || peer.vitals.dead() {
        return;
    }
    let health = peer.vitals.health;
    peer.vitals.hurt(amount, cause);
    if peer.vitals.health < health && cause != DamageCause::Starvation {
        peer.survival.damage_epoch = peer.survival.damage_epoch.wrapping_add(1);
    }
    if peer.vitals.dead() {
        peer.survival.pending_deaths = peer.survival.pending_deaths.saturating_add(1);
        peer.mining = None;
        peer.last_movement = CharacterInput::default();
    }
}

/// Invulnerability restores survival vitals, not an explicitly granted visual buff.
pub(super) fn reset_invulnerable_vitals(peer: &mut Peer) {
    let vision = peer.vitals.effects.get(terra_core::EffectKind::NightVision);
    peer.vitals = SurvivalVitals::default();
    peer.vitals.apply_equipment(peer.equipment);
    if let Some(vision) = vision {
        peer.vitals
            .effects
            .apply(vision.kind, vision.remaining_seconds, vision.level);
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn advance(
    peer: &mut Peer,
    before: CharacterState,
    after: CharacterState,
    submerged: bool,
    swimming: bool,
    mode: GameMode,
    dt: f32,
) {
    if mode != GameMode::Survival {
        reset_invulnerable_vitals(peer);
        peer.vitals.effects.advance(dt);
        peer.survival.fall_peak = None;
        return;
    }
    let alive = !peer.vitals.dead();
    if peer.survival.protection > 0.0 {
        peer.survival.protection = (peer.survival.protection - dt).max(0.0);
        peer.survival.fall_peak = None;
        peer.vitals.effects.advance(dt);
        return;
    }
    let life = peer.vitals.advance_life(submerged, dt);
    // Hunger damage continues while eating, but cannot permanently prevent recovery
    // from an empty hunger bar. Drowning remains an interrupt even when starving.
    if life.non_starvation_damage > 0.0 {
        peer.survival.damage_epoch = peer.survival.damage_epoch.wrapping_add(1);
    }
    let health = peer.vitals.health;
    if swimming || peer.last_movement.flight_speed.is_some() {
        peer.survival.fall_peak = None;
    } else if after.grounded {
        if let Some(peak) = peer.survival.fall_peak.take() {
            peer.vitals.land(peak - after.position.y);
        }
    } else {
        peer.survival.fall_peak = Some(
            peer.survival
                .fall_peak
                .unwrap_or(before.position.y)
                .max(before.position.y)
                .max(after.position.y),
        );
    }
    if after.position.y < -512.0 {
        peer.vitals.hurt(peer.vitals.max_health, DamageCause::Void);
    }
    if peer.vitals.health < health {
        peer.survival.damage_epoch = peer.survival.damage_epoch.wrapping_add(1);
    }
    if alive && peer.vitals.dead() {
        peer.survival.pending_deaths = peer.survival.pending_deaths.saturating_add(1);
        peer.mining = None;
        peer.last_movement = CharacterInput::default();
    }
}

pub(super) fn begin_respawns(
    mut spawn: spawn::SpawnWorld,
    config: Res<GameConfig>,
    journal: Res<EditJournal>,
    mut peers: Query<(&mut Peer, &mut MessageSender<ServerControl>), With<Connected>>,
) {
    for (mut peer, mut sender) in &mut peers {
        if !std::mem::take(&mut peer.survival.respawn_requested)
            || !peer.vitals.dead()
            || peer.chat.teleport.is_some()
        {
            continue;
        }
        // A full drop budget can defer the atomic death transfer. Keep this
        // respawn request pending until item ownership has been transferred.
        if !config.world.game_rules.keep_inventory
            && (journal
                .gameplay()
                .inventories
                .get(&peer.uuid)
                .is_some_and(|bag| bag.occupied_slots() > 0 || bag.cursor.is_some())
                || peer.equipment.iter().next().is_some())
        {
            peer.survival.respawn_requested = true;
            continue;
        }
        match spawn.position() {
            Ok(position) => {
                peer.chat.teleport = Some(chat::PendingTeleport::respawn(position));
                peer.mining = None;
                sender.send::<ControlChannel>(ServerControl::Mining(None));
                sender.send::<ControlChannel>(ServerControl::TeleportPreparing);
            }
            Err(error) => {
                tracing::warn!(%error, player=peer.id.0, "Cannot resolve a safe respawn");
                sender.send::<ControlChannel>(ServerControl::TeleportCancelled);
            }
        }
    }
}

pub(super) fn commit_and_send(
    mut journal: ResMut<EditJournal>,
    config: Res<GameConfig>,
    bodies: Query<&Transform, With<ServerCharacter>>,
    time: Res<Time<Real>>,
    mut last_sent: Local<f64>,
    mut peers: Query<(&mut Peer, &mut MessageSender<ServerControl>), With<Connected>>,
) {
    let send = time.elapsed_secs_f64() - *last_sent >= 0.1;
    if send {
        *last_sent = time.elapsed_secs_f64();
    }
    for (mut peer, mut sender) in &mut peers {
        if !peer.accepted {
            continue;
        }
        let owned_items = journal
            .gameplay()
            .inventories
            .get(&peer.uuid)
            .is_some_and(|bag| bag.occupied_slots() > 0 || bag.cursor.is_some())
            || peer.equipment.iter().next().is_some();
        if peer.vitals.dead()
            && !config.world.game_rules.keep_inventory
            && owned_items
            && let Ok(body) = bodies.get(peer.body)
        {
            let uuid = peer.uuid;
            journal
                .bypass_change_detection()
                .gameplay_mut()
                .drop_death_inventory(
                    uuid,
                    &mut peer.equipment,
                    body.translation.to_array().map(f64::from),
                );
            let equipment = peer.equipment;
            peer.vitals.apply_equipment(equipment);
        }
        let mut stats = journal
            .gameplay()
            .player_stats
            .get(&peer.uuid)
            .copied()
            .unwrap_or_default();
        stats.survival = peer.vitals;
        stats.equipment = peer.equipment;
        stats.deaths = stats
            .deaths
            .saturating_add(std::mem::take(&mut peer.survival.pending_deaths));
        // Player metadata must not force a terrain, collision or water rebuild.
        journal
            .bypass_change_detection()
            .update_player_stats(peer.uuid, stats);
        if send && peer.synced && peer.stats_sent != Some((peer.generation, stats)) {
            if stats.survival.dead() {
                sender.send::<ControlChannel>(ServerControl::Mining(None));
            }
            sender.send::<ControlChannel>(ServerControl::Stats {
                generation: peer.generation,
                stats: Box::new(stats),
            });
            peer.stats_sent = Some((peer.generation, stats));
        }
    }
}

#[cfg(test)]
#[path = "server_survival_tests.rs"]
pub(super) mod tests;
