//! Shared bounded survival rules. Only the authority commits damage and food consumption.
use crate::{ActiveEffect, EffectKind, ItemKind, StatusEffects, effect_definition};
use serde::{Deserialize, Serialize};

pub const MAX_VITAL: f32 = 100.0;
pub const JUMP_STAMINA: f32 = 10.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DamageCause {
    Fall,
    Drowning,
    Starvation,
    Void,
    Animal,
    Lightning,
    FallingMineral,
    Smoke,
    Poison,
    Freeze,
    Bleeding,
    Rotten,
    Fire,
}
impl DamageCause {
    pub fn message(self) -> &'static str {
        match self {
            Self::Fall => "You fell from too high.",
            Self::Drowning => "You ran out of breath.",
            Self::Starvation => "You ran out of food.",
            Self::Void => "You fell beyond the world.",
            Self::Animal => "You were attacked by an animal.",
            Self::Lightning => "You were struck by lightning.",
            Self::FallingMineral => "You were struck by falling stone.",
            Self::Smoke => "You suffocated in smoke.",
            Self::Poison => "You succumbed to poison.",
            Self::Freeze => "You froze to death.",
            Self::Bleeding => "You bled out.",
            Self::Rotten => "You were overcome by sickness.",
            Self::Fire => "You burned to death.",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SurvivalVitals {
    pub health: f32,
    pub stamina: f32,
    pub hunger: f32,
    pub oxygen: f32,
    pub exhausted: bool,
    pub stamina_delay: f32,
    pub recovery_delay: f32,
    pub last_damage: Option<DamageCause>,
    pub saturation: f32,
    pub regeneration_seconds: f32,
    pub regeneration_rate: f32,
    pub effects: StatusEffects,
    pub max_health: f32,
    pub max_stamina: f32,
    pub armor_points: f32,
}
impl Default for SurvivalVitals {
    fn default() -> Self {
        Self {
            health: MAX_VITAL,
            stamina: MAX_VITAL,
            hunger: MAX_VITAL,
            oxygen: MAX_VITAL,
            exhausted: false,
            stamina_delay: 0.0,
            recovery_delay: 0.0,
            last_damage: None,
            saturation: 0.0,
            regeneration_seconds: 0.0,
            regeneration_rate: 0.0,
            effects: StatusEffects::default(),
            max_health: MAX_VITAL,
            max_stamina: MAX_VITAL,
            armor_points: 0.0,
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct SurvivalEffort {
    pub moving: bool,
    pub sprinting: bool,
    pub swimming: bool,
    pub submerged: bool,
    pub jumped: bool,
}

/// Damage committed during one life tick, before any compensating regeneration.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct SurvivalLifeOutcome {
    pub non_starvation_damage: f32,
}

impl SurvivalVitals {
    pub fn valid(self) -> bool {
        [self.hunger, self.oxygen, self.saturation]
            .into_iter()
            .all(|value| value.is_finite() && (0.0..=MAX_VITAL).contains(&value))
            && [self.max_health, self.max_stamina]
                .into_iter()
                .all(|value| {
                    value.is_finite()
                        && (MAX_VITAL..=MAX_VITAL + crate::equipment::MAX_EQUIPMENT_BONUS)
                            .contains(&value)
                })
            && self.health.is_finite()
            && (0.0..=self.max_health).contains(&self.health)
            && self.stamina.is_finite()
            && (0.0..=self.max_stamina).contains(&self.stamina)
            && self.armor_points.is_finite()
            && (0.0..=crate::MAX_ARMOR_POINTS).contains(&self.armor_points)
            && [self.stamina_delay, self.recovery_delay]
                .into_iter()
                .all(|value| value.is_finite() && (0.0..=10.0).contains(&value))
            && self.regeneration_seconds.is_finite()
            && (0.0..=60.0).contains(&self.regeneration_seconds)
            && self.regeneration_rate.is_finite()
            && (0.0..=5.0).contains(&self.regeneration_rate)
            && ((self.regeneration_seconds == 0.0) == (self.regeneration_rate == 0.0))
            && self.effects.valid()
    }
    pub fn dead(self) -> bool {
        self.health <= 0.0
    }
    /// Always recompute from the server-validated loadout; equipping never restores spent vitals.
    pub fn apply_equipment(&mut self, equipment: crate::EquipmentSlots) {
        let bonuses = if equipment.valid() {
            equipment.bonuses()
        } else {
            Default::default()
        };
        self.max_health = MAX_VITAL + bonuses.health_bonus;
        self.max_stamina = MAX_VITAL + bonuses.stamina_bonus;
        self.armor_points = bonuses.armor_points;
        self.health = self.health.min(self.max_health);
        self.stamina = self.stamina.min(self.max_stamina);
    }
    pub fn can_sprint(self) -> bool {
        !self.dead() && !self.exhausted && self.stamina > 0.0
    }
    pub fn can_jump(self) -> bool {
        !self.dead() && !self.exhausted && self.stamina >= JUMP_STAMINA
    }
    /// Food retains its data-defined fractional healing rate, but shares one regeneration card.
    pub fn active_effects(self) -> impl Iterator<Item = ActiveEffect> {
        EffectKind::ALL.into_iter().filter_map(move |kind| {
            let timed = self.effects.get(kind);
            if kind == EffectKind::Regeneration && self.regeneration_seconds > 0.0 {
                Some(ActiveEffect {
                    kind,
                    remaining_seconds: self
                        .regeneration_seconds
                        .max(timed.map_or(0.0, |e| e.remaining_seconds)),
                    level: timed.map_or(1, |effect| effect.level),
                })
            } else {
                timed
            }
        })
    }
    /// Used by authority and prediction with the same successful physics tick.
    pub fn advance_effort(&mut self, effort: SurvivalEffort, dt: f32) {
        if self.dead() || !dt.is_finite() || dt <= 0.0 {
            return;
        }
        let dt = dt.min(0.25);
        let strenuous = (effort.moving && (effort.sprinting || effort.swimming)) || effort.jumped;
        let drain = if effort.moving && effort.sprinting {
            12.0
        } else {
            0.0
        } + if effort.moving && effort.swimming {
            4.0
        } else {
            0.0
        };
        if effort.submerged {
            self.stamina = self.max_stamina;
            self.stamina_delay = 0.0;
            self.exhausted = false;
        } else if strenuous {
            self.stamina =
                (self.stamina - drain * dt - if effort.jumped { JUMP_STAMINA } else { 0.0 })
                    .max(0.0);
            self.stamina_delay = 1.0;
        } else {
            let recovery = (dt - self.stamina_delay).max(0.0);
            self.stamina_delay = (self.stamina_delay - dt).max(0.0);
            self.stamina = (self.stamina + recovery * 18.0).min(self.max_stamina);
        }
        if self.stamina <= 0.01 {
            self.exhausted = true;
        } else if self.stamina >= 25.0 {
            self.exhausted = false;
        }
        let metabolism = 0.025
            + if effort.moving { 0.025 } else { 0.0 }
            + if effort.moving && effort.sprinting {
                0.15
            } else {
                0.0
            }
            + if effort.moving && effort.swimming {
                0.07
            } else {
                0.0
            };
        self.spend_food(metabolism * dt + if effort.jumped { 0.2 } else { 0.0 });
    }
    pub fn advance_life(&mut self, submerged: bool, dt: f32) -> SurvivalLifeOutcome {
        let mut outcome = SurvivalLifeOutcome::default();
        if self.dead() || !dt.is_finite() || dt <= 0.0 {
            return outcome;
        }
        let dt = dt.min(0.25);
        self.recovery_delay = (self.recovery_delay - dt).max(0.0);
        if submerged {
            let drain = 5.0 * self.effects.oxygen_multiplier();
            let unprotected = if drain > 0.0 {
                (dt - self.oxygen / drain).max(0.0)
            } else {
                0.0
            };
            self.oxygen = (self.oxygen - dt * drain).max(0.0);
            let health = self.health;
            self.hurt(unprotected * 8.0, DamageCause::Drowning);
            outcome.non_starvation_damage += health - self.health;
        } else {
            self.oxygen = (self.oxygen + dt * 30.0).min(MAX_VITAL);
        }
        if self.hunger <= 0.0 {
            self.hurt(dt, DamageCause::Starvation);
        } else if self.health < self.max_health && self.hunger >= 75.0 && self.recovery_delay <= 0.0
        {
            self.health = (self.health + dt * 1.25).min(self.max_health);
            self.spend_food(dt * 0.16);
        }
        let mut food_recovery = 0.0;
        if self.regeneration_seconds > 0.0 {
            let active = dt.min(self.regeneration_seconds);
            self.regeneration_seconds = (self.regeneration_seconds - dt).max(0.0);
            if !self.dead() && self.hunger > 0.0 && self.recovery_delay <= 0.0 {
                food_recovery = active * self.regeneration_rate;
            }
            if self.regeneration_seconds <= 0.0 {
                self.regeneration_rate = 0.0;
            }
        }
        let mut status_recovery: f32 = 0.0;
        // Timed poison is independent of smoke: furnace damage is committed by its own authority.
        for effect in self.effects.timed_effects() {
            let active = dt.min(effect.remaining_seconds);
            let modifiers = effect_definition(effect.kind).modifiers;
            let scale = active * f32::from(effect.level);
            let cause = match effect.kind {
                EffectKind::Poison => DamageCause::Poison,
                EffectKind::Freeze => DamageCause::Freeze,
                EffectKind::Bleeding => DamageCause::Bleeding,
                EffectKind::Rotten => DamageCause::Rotten,
                EffectKind::OnFire => DamageCause::Fire,
                _ => DamageCause::Animal,
            };
            let health = self.health;
            self.hurt(modifiers.damage_per_second * scale, cause);
            outcome.non_starvation_damage += health - self.health;
            self.spend_food(modifiers.hunger_per_second * scale);
            status_recovery = status_recovery.max(modifiers.health_per_second * scale);
        }
        if !self.dead() {
            self.health = (self.health + food_recovery.max(status_recovery)).min(self.max_health);
        }
        self.effects.advance(dt);
        outcome.non_starvation_damage = outcome.non_starvation_damage.clamp(0.0, self.max_health);
        outcome
    }
    fn spend_food(&mut self, amount: f32) {
        let reserve = amount.min(self.saturation);
        self.saturation = (self.saturation - reserve).max(0.0);
        self.hunger = (self.hunger - (amount - reserve)).max(0.0);
    }
    pub fn hurt(&mut self, amount: f32, cause: DamageCause) {
        if self.dead() || !amount.is_finite() || amount <= 0.0 {
            return;
        }
        let amount = match cause {
            DamageCause::Fire => amount * self.effects.fire_damage_multiplier(),
            DamageCause::Freeze => amount * self.effects.freeze_damage_multiplier(),
            _ => amount,
        };
        let amount = if matches!(
            cause,
            DamageCause::Animal | DamageCause::Lightning | DamageCause::FallingMineral
        ) {
            amount * (1.0 - (self.armor_points * 0.02).clamp(0.0, 0.8))
        } else {
            amount
        };
        let amount = if matches!(
            cause,
            DamageCause::Animal
                | DamageCause::Fall
                | DamageCause::Lightning
                | DamageCause::FallingMineral
                | DamageCause::Fire
                | DamageCause::Freeze
        ) {
            self.effects.absorb_damage(amount)
        } else {
            amount
        };
        if amount <= 0.0 {
            return;
        }
        self.health = (self.health - amount).max(0.0);
        self.last_damage = Some(cause);
        self.recovery_delay = 4.0;
    }
    pub fn land(&mut self, distance: f32) {
        self.hurt((distance - 3.0).max(0.0) * 8.0, DamageCause::Fall);
    }
    pub fn eat(&mut self, item: ItemKind) -> bool {
        let Some(food) = crate::food(item) else {
            return false;
        };
        if self.dead() || self.hunger >= MAX_VITAL {
            return false;
        }
        self.hunger = (self.hunger + food.nutrition).min(MAX_VITAL);
        self.saturation = (self.saturation + food.saturation).min(MAX_VITAL);
        if let Some(regeneration) = food.regeneration {
            // A new meal refreshes a bounded effect; eating never stacks its rate or duration.
            self.regeneration_seconds = regeneration.duration_seconds;
            self.regeneration_rate = regeneration.health_per_second;
        }
        true
    }
    /// Stable artwork stages: full, small bite, half, mostly eaten, bare bone.
    pub fn hunger_stage(self) -> usize {
        if self.hunger >= 90.0 {
            0
        } else if self.hunger >= 75.0 {
            1
        } else if self.hunger >= 50.0 {
            2
        } else if self.hunger > 10.0 {
            3
        } else {
            4
        }
    }
}

pub fn nutrition(item: ItemKind) -> Option<f32> {
    crate::food(item).map(|food| food.nutrition)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ResourcePropKind;
    #[test]
    fn diving_uses_air_not_stamina_but_surface_swimming_still_costs_energy() {
        let mut v = SurvivalVitals {
            stamina: 0.0,
            exhausted: true,
            ..Default::default()
        };
        let mut effort = SurvivalEffort {
            moving: true,
            sprinting: true,
            swimming: true,
            submerged: true,
            ..Default::default()
        };
        for _ in 0..60 * 10 {
            v.advance_effort(effort, 1.0 / 60.0);
            v.advance_life(true, 1.0 / 60.0);
            assert_eq!(v.stamina, MAX_VITAL);
            assert!(v.can_sprint());
        }
        assert!((v.oxygen - 50.0).abs() < 0.01);
        effort.submerged = false;
        for _ in 0..60 * 2 {
            v.advance_effort(effort, 1.0 / 60.0);
            v.advance_life(false, 1.0 / 60.0);
        }
        assert_eq!(v.oxygen, MAX_VITAL);
        assert!((v.stamina - 68.0).abs() < 0.01);
        assert!(v.valid());
    }
    fn ticks(v: &mut SurvivalVitals, effort: SurvivalEffort, underwater: bool, count: usize) {
        for _ in 0..count {
            v.advance_effort(effort, 1.0 / 60.0);
            v.advance_life(underwater, 1.0 / 60.0);
        }
    }
    #[test]
    fn sprint_exhausts_then_recovers_with_hysteresis_and_jump_has_a_cost() {
        let mut v = SurvivalVitals::default();
        ticks(
            &mut v,
            SurvivalEffort {
                moving: true,
                sprinting: true,
                ..Default::default()
            },
            false,
            510,
        );
        assert!(v.exhausted && !v.can_sprint() && !v.can_jump());
        ticks(&mut v, SurvivalEffort::default(), false, 90);
        assert!(v.exhausted && v.stamina < 25.0);
        ticks(&mut v, SurvivalEffort::default(), false, 60);
        assert!(v.can_sprint() && v.can_jump());
        let before = v.stamina;
        v.advance_effort(
            SurvivalEffort {
                jumped: true,
                ..Default::default()
            },
            1.0 / 60.0,
        );
        assert!((before - v.stamina - JUMP_STAMINA).abs() < 0.001);
        assert!(v.valid());
    }
    #[test]
    fn breathing_drowning_food_starvation_regeneration_and_death_are_bounded() {
        let mut v = SurvivalVitals::default();
        ticks(&mut v, SurvivalEffort::default(), true, 1200);
        assert!(v.health > 99.9 && v.oxygen < 0.001);
        ticks(&mut v, SurvivalEffort::default(), true, 120);
        assert!((v.health - 84.0).abs() < 0.02);
        ticks(&mut v, SurvivalEffort::default(), false, 360);
        assert_eq!(v.oxygen, 100.0);
        assert!(v.health > 84.0 && v.hunger < 100.0);
        v.hunger = 0.0;
        assert!(v.eat(ItemKind::Prop(ResourcePropKind::Mushroom)));
        assert_eq!(v.hunger, 18.0);
        assert!(!v.eat(ItemKind::Block(3)));
        v.hunger = 0.0;
        ticks(&mut v, SurvivalEffort::default(), false, 6000);
        assert!(v.dead() && v.last_damage == Some(DamageCause::Starvation));
        assert!(!v.eat(ItemKind::Prop(ResourcePropKind::Mushroom)));
        assert!(v.valid());
    }
    #[test]
    fn saturation_is_spent_before_hunger_and_full_or_dead_players_cannot_eat() {
        let item = ItemKind::Prop(ResourcePropKind::Mushroom);
        let mut v = SurvivalVitals {
            hunger: 40.0,
            saturation: 0.1,
            ..Default::default()
        };
        ticks(&mut v, SurvivalEffort::default(), false, 360);
        assert_eq!(v.saturation, 0.0);
        assert!((v.hunger - 39.95).abs() < 0.002);
        v.hunger = 99.0;
        v.saturation = 99.0;
        assert!(v.eat(item));
        assert_eq!(v.hunger, MAX_VITAL);
        assert_eq!(v.saturation, MAX_VITAL);
        let before = v;
        assert!(!v.eat(item));
        assert_eq!(before, v);
        v.health = 0.0;
        v.hunger = 1.0;
        assert!(!v.eat(item));
        assert!(v.valid());
    }
    #[test]
    fn meal_regeneration_expires_without_stacking_and_damage_suppresses_recovery() {
        let cooked = ItemKind::Animal(crate::creatures::AnimalProduct::CookedBeef);
        let mut v = SurvivalVitals {
            health: 40.0,
            hunger: 10.0,
            ..Default::default()
        };
        assert!(v.eat(cooked));
        assert_eq!(v.regeneration_seconds, 8.0);
        assert_eq!(v.regeneration_rate, 0.5);
        ticks(&mut v, SurvivalEffort::default(), false, 500);
        assert!((v.health - 44.0).abs() < 0.003);
        assert_eq!(v.regeneration_seconds, 0.0);
        assert_eq!(v.regeneration_rate, 0.0);
        v.hunger = 10.0;
        assert!(v.eat(cooked));
        v.hunger = 10.0;
        assert!(v.eat(cooked));
        assert_eq!(v.regeneration_seconds, 8.0);
        assert_eq!(v.regeneration_rate, 0.5);
        v.hurt(10.0, DamageCause::Animal);
        let injured_health = v.health;
        ticks(&mut v, SurvivalEffort::default(), false, 180);
        assert_eq!(v.health, injured_health);
        ticks(&mut v, SurvivalEffort::default(), false, 320);
        assert!((v.health - injured_health - 2.0).abs() < 0.02);
        assert_eq!(v.regeneration_seconds, 0.0);
        assert!(v.valid());
    }
    #[test]
    fn legacy_text_vitals_default_food_reserves_and_reject_unbounded_effects() {
        let mut v: SurvivalVitals = toml::from_str(
            "health = 52.0\nstamina = 73.0\nhunger = 46.0\noxygen = 91.0\nexhausted = false",
        )
        .unwrap();
        assert_eq!(v.saturation, 0.0);
        assert_eq!(v.regeneration_seconds, 0.0);
        assert_eq!(v.regeneration_rate, 0.0);
        assert!(v.valid());
        v.saturation = f32::NAN;
        assert!(!v.valid());
        v.saturation = MAX_VITAL;
        v.regeneration_rate = 1.0;
        assert!(!v.valid());
        v.regeneration_seconds = 61.0;
        assert!(!v.valid());
        v.regeneration_seconds = 60.0;
        assert!(v.valid());
        v.regeneration_rate = f32::INFINITY;
        assert!(!v.valid());
    }
    #[test]
    fn falls_invalid_time_and_hunger_artwork_are_stable() {
        let mut v = SurvivalVitals::default();
        v.land(3.0);
        assert_eq!(v.health, 100.0);
        v.land(8.0);
        assert_eq!(v.health, 60.0);
        let before = v;
        v.advance_effort(SurvivalEffort::default(), f32::NAN);
        v.advance_life(true, -1.0);
        assert_eq!(v, before);
        for (hunger, stage) in [
            (100.0, 0),
            (90.0, 0),
            (89.99, 1),
            (75.0, 1),
            (74.99, 2),
            (50.0, 2),
            (49.99, 3),
            (30.0, 3),
            (10.01, 3),
            (10.0, 4),
            (0.0, 4),
        ] {
            v.hunger = hunger;
            assert_eq!(v.hunger_stage(), stage);
        }
        v.health = f32::NAN;
        assert!(!v.valid());
    }
}
