use super::*;
use terra_core::EffectKind;

fn resting_peer(health: f32) -> Peer {
    let now = Instant::now();
    Peer {
        chest_cursor: None,
        creature_action_at: now,
        container: None,
        held_slot: None,
        mode: Some(GameMode::Survival),
        generation: 0,
        chat: Default::default(),
        uuid: terra_core::PlayerUuid::new_v4(),
        gender: Default::default(),
        mining: None,
        consumption: None,
        gameplay_sent: None,
        permissions: terra_core::PlayerPermissions {
            game_mode: false,
            build: true,
            flight: false,
            environment: false,
        },
        id: PlayerId(1),
        transport_id: 1,
        body: Entity::PLACEHOLDER,
        accepted: true,
        synced: true,
        view: 1,
        known: Default::default(),
        inputs: Default::default(),
        received: 0,
        ack: 0,
        look: [0.0; 2],
        last_input: now,
        joined: now,
        last_request: 0,
        edits_left: 20.0,
        rate_time: now,
        spawned: true,
        last_movement: Default::default(),
        vitals: SurvivalVitals {
            health,
            hunger: 50.0,
            ..Default::default()
        },
        survival: Default::default(),
        stats_sent: None,
    }
}

#[test]
fn awaiting_sleep_quorum_does_not_pause_poison_expiry_or_commit_death_twice() {
    let state = CharacterState {
        position: Vec3::new(8.0, 2.0, 8.0),
        grounded: true,
        ..Default::default()
    };
    for health in [50.0, 0.1] {
        let mut peer = resting_peer(health);
        peer.vitals.effects.apply(EffectKind::Poison, 0.5, 1);
        for _ in 0..60 {
            advance_rest(
                &mut peer,
                state,
                false,
                false,
                GameMode::Survival,
                1.0 / 60.0,
            );
        }
        if health > 1.0 {
            assert!((peer.vitals.health - 49.25).abs() < 0.001);
            assert!(!peer.vitals.effects.contains(EffectKind::Poison));
            assert_eq!(peer.survival.pending_deaths, 0);
        } else {
            assert!(peer.vitals.dead());
            assert_eq!(peer.survival.pending_deaths, 1);
        }
        assert_eq!(peer.vitals.stamina, terra_core::MAX_VITAL);
        assert_eq!(peer.vitals.hunger, 50.0);
        assert!(peer.survival.damage_epoch > 0);
        assert_eq!(peer.vitals.last_damage, Some(DamageCause::Poison));
    }
}

#[test]
fn resting_still_uses_actual_underwater_oxygen_rules_and_does_not_hurt_creative() {
    let state = CharacterState {
        position: Vec3::new(8.0, 2.0, 8.0),
        grounded: true,
        ..Default::default()
    };
    let mut peer = resting_peer(50.0);
    peer.vitals.oxygen = 0.0;
    advance_rest(&mut peer, state, true, true, GameMode::Survival, 0.25);
    assert_eq!(peer.vitals.health, 48.0);
    assert_eq!(peer.vitals.last_damage, Some(DamageCause::Drowning));
    peer.vitals.effects.apply(EffectKind::Poison, 10.0, 1);
    advance_rest(&mut peer, state, true, true, GameMode::Creative, 0.25);
    assert_eq!(peer.vitals, SurvivalVitals::default());
}
