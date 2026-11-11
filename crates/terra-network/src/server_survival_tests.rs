use super::*;
use terra_core::EffectKind;

pub(crate) fn resting_peer(health: f32) -> Peer {
    let now = Instant::now();
    Peer {
        farming: Default::default(),
        crank: Default::default(),
        chest_cursor: None,
        creature_action_at: now,
        container: None,
        held_slot: None,
        mode: Some(GameMode::Survival),
        generation: 0,
        chat: Default::default(),
        uuid: terra_core::PlayerUuid::new_v4(),
        gender: Default::default(),
        appearance: Default::default(),
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
        restored_spawn: false,
        restore_location: None,
        last_movement: Default::default(),
        vitals: SurvivalVitals {
            health,
            hunger: 50.0,
            ..Default::default()
        },
        equipment: Default::default(),
        survival: Default::default(),
        stats_sent: None,
    }
}

#[test]
fn creative_keeps_command_night_vision_but_advances_its_timer_without_survival_damage() {
    let mut peer = resting_peer(100.0);
    peer.vitals.effects.apply(EffectKind::NightVision, 0.5, 2);
    peer.vitals.effects.apply(EffectKind::Poison, 10.0, 1);
    for remaining in [0.25, 0.0] {
        reset_invulnerable_vitals(&mut peer);
        advance(
            &mut peer,
            CharacterState::default(),
            CharacterState::default(),
            false,
            false,
            GameMode::Creative,
            0.25,
        );
        assert_eq!(peer.vitals.health, 100.0);
        assert!(!peer.vitals.effects.contains(EffectKind::Poison));
        assert_eq!(
            peer.vitals
                .effects
                .get(EffectKind::NightVision)
                .map_or(0.0, |effect| effect.remaining_seconds),
            remaining
        );
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

#[test]
fn authority_armor_reduces_physical_hits_but_not_smoke_and_death_keeps_the_loadout() {
    use terra_core::{ArmorItem, ArmorMaterial, ArmorPiece, EquipmentSlot, ItemKind};
    let item = ItemKind::Armor(ArmorItem {
        material: ArmorMaterial::Iron,
        piece: ArmorPiece::Chestplate,
    });
    let mut peer = resting_peer(50.0);
    peer.equipment.set(EquipmentSlot::Chestplate, Some(item));
    peer.vitals.apply_equipment(peer.equipment);
    let armor = peer.vitals.armor_points;
    assert!(armor > 0.0);
    animal_attack(&mut peer, 10.0, GameMode::Survival);
    let expected = 50.0 - 10.0 * (1.0 - armor * 0.02);
    assert!((peer.vitals.health - expected).abs() < 0.001);
    environmental_damage(&mut peer, 2.0, GameMode::Survival, DamageCause::Smoke);
    assert!((peer.vitals.health - (expected - 2.0)).abs() < 0.001);
    let lethal = peer.vitals.max_health;
    environmental_damage(&mut peer, lethal, GameMode::Survival, DamageCause::Void);
    assert!(peer.vitals.dead());
    assert_eq!(peer.equipment.get(EquipmentSlot::Chestplate), Some(item));
    assert_eq!(peer.survival.pending_deaths, 1);
}
