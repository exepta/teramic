use super::*;
use crate::{DamageCause, ItemKind, SurvivalVitals, creatures::AnimalProduct};

#[test]
fn all_thirteen_effect_definitions_have_valid_stable_unique_identities() {
    for (index, kind) in EffectKind::ALL.into_iter().enumerate() {
        let definition = effect_definition(kind);
        assert!(definition.valid());
        assert_eq!(definition.kind, kind);
        assert_eq!(kind.index(), index);
        assert_eq!(kind.identifier(), definition.id);
        assert!(
            EffectKind::ALL[..index]
                .iter()
                .all(|prior| prior.identifier() != kind.identifier())
        );
        let mut state = StatusEffects::default();
        assert!(state.apply(kind, 10.0, 1));
        assert!(state.valid());
        assert_eq!(state.iter().count(), 1);
    }
    assert!(std::mem::size_of::<SurvivalVitals>() <= 256);
}

#[test]
fn invalid_definitions_and_unbounded_effect_inputs_are_rejected() {
    let mut definition = effect_definition(EffectKind::Poison).clone();
    for invalid in [f32::NAN, f32::INFINITY, 0.0, -1.0, MAX_EFFECT_SECONDS + 1.0] {
        definition.max_duration_seconds = invalid;
        assert!(!definition.valid());
    }
    let mut state = StatusEffects::default();
    for seconds in [f32::NAN, f32::INFINITY, -1.0, 0.0] {
        assert!(!state.apply(EffectKind::Poison, seconds, 1));
    }
    assert!(!state.apply(EffectKind::Poison, 10.0, 0));
    assert!(!state.apply(EffectKind::Poison, 10.0, u8::MAX));
    assert_eq!(state, StatusEffects::default());
    assert!(state.apply(EffectKind::Speed, f32::MAX, 1));
    assert_eq!(
        state.get(EffectKind::Speed).unwrap().remaining_seconds,
        MAX_EFFECT_SECONDS
    );
    let before = state;
    state.advance(f32::NAN);
    state.advance(-1.0);
    assert_eq!(before, state);
}

#[test]
fn refreshes_use_one_fixed_slot_and_do_not_accumulate_levels_or_duration() {
    let mut state = StatusEffects::default();
    for _ in 0..1_000 {
        state.apply(EffectKind::Speed, 10.0, 1);
    }
    assert_eq!(state.iter().count(), 1);
    assert_eq!(
        state.get(EffectKind::Speed).unwrap().remaining_seconds,
        10.0
    );
    assert_eq!(state.get(EffectKind::Speed).unwrap().level, 1);
    state.apply(EffectKind::Speed, 5.0, 2);
    assert_eq!(state.get(EffectKind::Speed).unwrap().level, 2);
    assert_eq!(
        state.get(EffectKind::Speed).unwrap().remaining_seconds,
        10.0
    );
    assert!((state.movement_multiplier(false) - 1.69).abs() < 0.001);
    assert!(state.remove(EffectKind::Speed));
    assert_eq!(state.movement_multiplier(false), 1.0);
}

#[test]
fn poison_exposure_does_not_stack_with_smoke_damage_or_erase_jellyfish_poison() {
    let mut vitals = SurvivalVitals {
        hunger: 50.0,
        ..Default::default()
    };
    vitals.effects.set_smoke_exposure(true);
    assert!(vitals.effects.contains(EffectKind::Poison));
    assert_eq!(
        vitals
            .effects
            .get(EffectKind::Poison)
            .unwrap()
            .remaining_seconds,
        0.0
    );
    vitals.advance_life(false, 0.25);
    assert_eq!(vitals.health, 100.0);
    vitals.effects.apply(EffectKind::Poison, 10.0, 1);
    vitals.advance_life(false, 0.25);
    assert!((vitals.health - 99.625).abs() < 0.001);
    vitals.effects.set_smoke_exposure(false);
    assert_eq!(
        vitals
            .effects
            .get(EffectKind::Poison)
            .unwrap()
            .remaining_seconds,
        9.75
    );
    assert!(!vitals.effects.smoke_exposed());
    for _ in 0..39 {
        vitals.advance_life(false, 0.25);
    }
    assert!(!vitals.effects.contains(EffectKind::Poison));
    assert!((vitals.health - 85.0).abs() < 0.001);
}

#[test]
fn effect_damage_expires_at_fractional_duration_and_does_not_revive_the_dead() {
    let mut vitals = SurvivalVitals {
        hunger: 50.0,
        ..Default::default()
    };
    vitals.effects.apply(EffectKind::Bleeding, 0.1, 1);
    vitals.advance_life(false, 0.25);
    assert!((vitals.health - 99.8).abs() < 0.001);
    assert!(!vitals.effects.contains(EffectKind::Bleeding));
    vitals.health = 0.1;
    vitals.effects.apply(EffectKind::Poison, 10.0, 1);
    vitals.effects.apply(EffectKind::Regeneration, 10.0, 1);
    vitals.advance_life(false, 0.25);
    assert!(vitals.dead());
    assert_eq!(vitals.last_damage, Some(DamageCause::Poison));
}

#[test]
fn resistances_counter_only_their_matching_damage_and_freezing_slowdown() {
    let mut vitals = SurvivalVitals {
        hunger: 50.0,
        ..Default::default()
    };
    vitals.effects.apply(EffectKind::Freeze, 10.0, 1);
    vitals.effects.apply(EffectKind::OnFire, 10.0, 1);
    assert!(vitals.effects.movement_multiplier(false) < 1.0);
    vitals.effects.apply(EffectKind::FreezeResistance, 10.0, 1);
    vitals.effects.apply(EffectKind::FireResistance, 10.0, 1);
    assert_eq!(vitals.effects.movement_multiplier(false), 1.0);
    vitals.advance_life(false, 0.25);
    assert_eq!(vitals.health, 100.0);
    vitals.effects.apply(EffectKind::Bleeding, 10.0, 1);
    vitals.advance_life(false, 0.25);
    assert_eq!(vitals.health, 99.5);
    assert!(vitals.valid());
}

#[test]
fn water_affinity_improves_swimming_and_conserves_oxygen() {
    let mut vitals = SurvivalVitals::default();
    vitals.effects.apply(EffectKind::WaterAffinity, 10.0, 1);
    assert_eq!(vitals.effects.movement_multiplier(false), 1.0);
    assert!(vitals.effects.movement_multiplier(true) > 1.0);
    for _ in 0..4 {
        vitals.advance_life(true, 0.25);
    }
    assert!((vitals.oxygen - 97.5).abs() < 0.001);
    vitals.effects.apply(EffectKind::Strength, 1.0, 1);
    assert_eq!(vitals.effects.attack_multiplier(), 1.5);
}

#[test]
fn shield_is_a_consumable_pool_and_expires_without_becoming_extra_health() {
    let mut vitals = SurvivalVitals {
        hunger: 50.0,
        ..Default::default()
    };
    vitals.effects.apply(EffectKind::ShieldPower, 1.0, 1);
    assert_eq!(vitals.effects.shield_capacity(), 20.0);
    vitals.hurt(12.0, DamageCause::Animal);
    assert_eq!(vitals.health, 100.0);
    assert_eq!(vitals.effects.shield_points(), 8.0);
    vitals.hurt(10.0, DamageCause::Animal);
    assert_eq!(vitals.health, 98.0);
    assert_eq!(vitals.effects.shield_points(), 0.0);
    vitals.effects.apply(EffectKind::ShieldPower, 1.0, 1);
    vitals.hurt(1.0, DamageCause::Poison);
    assert_eq!(vitals.health, 97.0);
    assert_eq!(vitals.effects.shield_points(), 20.0);
    for _ in 0..4 {
        vitals.advance_life(false, 0.25);
    }
    assert_eq!(vitals.effects.shield_points(), 0.0);
    assert!(!vitals.effects.contains(EffectKind::ShieldPower));
    assert!(vitals.valid());
}

#[test]
fn sleep_and_food_regeneration_share_one_card_and_never_double_heal() {
    let mut vitals = SurvivalVitals {
        health: 40.0,
        hunger: 10.0,
        ..Default::default()
    };
    assert!(vitals.eat(ItemKind::Animal(AnimalProduct::CookedBeef)));
    assert_eq!(vitals.active_effects().count(), 1);
    vitals.effects.apply(EffectKind::Regeneration, 5.0, 1);
    assert_eq!(vitals.active_effects().count(), 1);
    for _ in 0..20 {
        vitals.advance_life(false, 0.25);
    }
    assert!((vitals.health - 50.0).abs() < 0.001);
    assert!(!vitals.effects.contains(EffectKind::Regeneration));
    assert_eq!(vitals.active_effects().count(), 1);
    for _ in 0..12 {
        vitals.advance_life(false, 0.25);
    }
    assert!((vitals.health - 51.5).abs() < 0.001);
    assert_eq!(vitals.active_effects().count(), 0);
}

#[test]
fn json_state_roundtrip_and_validation_preserve_effects_and_remaining_shield() {
    let mut vitals = SurvivalVitals::default();
    for kind in EffectKind::ALL {
        vitals.effects.apply(kind, 10.0, 1);
    }
    vitals.effects.absorb_damage(7.0);
    let json = serde_json::to_string(&vitals).unwrap();
    let reloaded: SurvivalVitals = serde_json::from_str(&json).unwrap();
    assert_eq!(reloaded, vitals);
    assert_eq!(reloaded.effects.shield_points(), 13.0);
    assert!(reloaded.valid());
    let mut invalid = reloaded;
    invalid.effects.shield = f32::NAN;
    assert!(!invalid.valid());
    invalid = reloaded;
    invalid.effects.slots[0].as_mut().unwrap().remaining_seconds = f32::INFINITY;
    assert!(!invalid.valid());
}

#[test]
fn empty_and_populated_effects_remain_compatible_with_legacy_text_saves() {
    for mut vitals in [
        SurvivalVitals::default(),
        SurvivalVitals {
            health: 41.0,
            ..Default::default()
        },
    ] {
        let text = toml::to_string(&vitals).unwrap();
        let loaded: SurvivalVitals = toml::from_str(&text).unwrap();
        assert_eq!(loaded, vitals);
        vitals.effects.apply(EffectKind::Poison, 7.25, 1);
        vitals.effects.apply(EffectKind::ShieldPower, 4.25, 1);
        vitals.effects.absorb_damage(8.0);
        let text = toml::to_string(&vitals).unwrap();
        let loaded: SurvivalVitals = toml::from_str(&text).unwrap();
        assert_eq!(loaded, vitals);
    }
}

#[test]
fn serialized_effect_lists_cannot_contain_duplicates_or_exceed_the_bounded_set() {
    let duplicate = r#"{"slots":[{"kind":"poison","remaining_seconds":10.0,"level":1},{"kind":"poison","remaining_seconds":11.0,"level":1}]}"#;
    assert!(serde_json::from_str::<StatusEffects>(duplicate).is_err());
    let invalid = r#"{"slots":[{"kind":"poison","remaining_seconds":-2.0,"level":1}]}"#;
    assert!(serde_json::from_str::<StatusEffects>(invalid).is_err());
    let invalid = r#"{"slots":[{"kind":"poison","remaining_seconds":10.0,"level":0}]}"#;
    assert!(serde_json::from_str::<StatusEffects>(invalid).is_err());
}

#[test]
fn actual_poison_damage_is_reported_even_when_regeneration_increases_health() {
    let mut vitals = SurvivalVitals {
        health: 50.0,
        hunger: 50.0,
        ..Default::default()
    };
    vitals.effects.apply(EffectKind::Poison, 10.0, 1);
    vitals.effects.apply(EffectKind::Regeneration, 5.0, 1);
    let outcome = vitals.advance_life(false, 0.25);
    assert!((outcome.non_starvation_damage - 0.375).abs() < 0.0001);
    assert!(vitals.health > 50.0);
    assert_eq!(vitals.last_damage, Some(DamageCause::Poison));
    assert_eq!(vitals.recovery_delay, 4.0);
}

#[test]
fn starving_water_affinity_users_are_not_reported_drowning_before_air_runs_out() {
    let mut vitals = SurvivalVitals {
        health: 50.0,
        hunger: 0.0,
        oxygen: 0.06,
        ..Default::default()
    };
    vitals.effects.apply(EffectKind::WaterAffinity, 10.0, 1);
    let outcome = vitals.advance_life(true, 1.0 / 60.0);
    assert_eq!(outcome.non_starvation_damage, 0.0);
    assert!(vitals.oxygen > 0.0);
    assert_eq!(vitals.last_damage, Some(DamageCause::Starvation));
    let outcome = vitals.advance_life(true, 1.0 / 60.0);
    assert!(outcome.non_starvation_damage > 0.0);
    assert_eq!(vitals.oxygen, 0.0);
}

#[test]
fn immune_and_absorbed_damage_is_not_reported_as_health_damage() {
    let mut vitals = SurvivalVitals {
        health: 50.0,
        hunger: 50.0,
        ..Default::default()
    };
    vitals.effects.apply(EffectKind::OnFire, 10.0, 1);
    vitals.effects.apply(EffectKind::FireResistance, 10.0, 1);
    assert_eq!(vitals.advance_life(false, 0.25).non_starvation_damage, 0.0);
    vitals.effects.remove(EffectKind::FireResistance);
    vitals.effects.apply(EffectKind::ShieldPower, 10.0, 1);
    assert_eq!(vitals.advance_life(false, 0.25).non_starvation_damage, 0.0);
    assert_eq!(vitals.health, 50.0);
    assert!(vitals.effects.shield_points() < vitals.effects.shield_capacity());
}

#[test]
fn shield_validation_rejects_charge_above_its_effect_capacity() {
    let mut effects = StatusEffects::default();
    effects.apply(EffectKind::ShieldPower, 10.0, 1);
    assert!(effects.valid());
    effects.shield = effects.shield_capacity() + 1.0;
    assert!(!effects.valid());
    effects.shield = 100.0;
    assert!(!effects.valid());
}

#[test]
fn catalog_fingerprint_is_stable_canonical_and_changes_with_authority_rules() {
    let definitions = EffectKind::ALL.map(|kind| effect_definition(kind).clone());
    let hash = fingerprint_definitions(definitions.iter());
    assert_eq!(hash, effect_catalog_fingerprint());
    let mut reordered = definitions.clone();
    let mut json = serde_json::to_value(&definitions[0]).unwrap();
    let map = json.as_object_mut().unwrap();
    let kind = map.remove("kind").unwrap();
    map.insert("kind".into(), kind);
    reordered[0] = serde_json::from_value(json).unwrap();
    assert_eq!(fingerprint_definitions(reordered.iter()), hash);
    reordered[0].modifiers.damage_per_second += 0.5;
    assert_ne!(fingerprint_definitions(reordered.iter()), hash);
    assert_eq!(effect_catalog_fingerprint(), effect_catalog_fingerprint());
}

#[test]
fn loading_normalization_only_reduces_effects_and_never_grants_extra_shield() {
    let mut effects = StatusEffects::default();
    effects.apply(EffectKind::ShieldPower, 10.0, 1);
    effects.shield = 100.0;
    assert!(!effects.valid());
    effects.normalize_to_definitions().unwrap();
    assert_eq!(effects.shield_points(), effects.shield_capacity());
    assert_eq!(effects.shield_points(), 20.0);
    effects.slots[EffectKind::ShieldPower.index()]
        .as_mut()
        .unwrap()
        .level = 10;
    effects.shield = 100.0;
    effects.normalize_to_definitions().unwrap();
    assert_eq!(
        effects.get(EffectKind::ShieldPower).unwrap().level,
        effect_definition(EffectKind::ShieldPower).max_level
    );
    assert_eq!(effects.shield_points(), effects.shield_capacity());
    assert!(effects.valid());
    effects.shield = f32::NAN;
    assert!(effects.normalize_to_definitions().is_err());
}
