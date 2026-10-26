//! Fixed-size, authority-owned status effects shared by prediction, storage and the HUD.
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

pub const EFFECT_COUNT: usize = 14;
pub const MAX_EFFECT_SECONDS: f32 = 3_600.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum EffectKind {
    Poison,
    Freeze,
    Bleeding,
    Rotten,
    Slow,
    Speed,
    Strength,
    WaterAffinity,
    OnFire,
    FireResistance,
    FreezeResistance,
    Regeneration,
    ShieldPower,
    NightVision,
}

impl EffectKind {
    pub const ALL: [Self; EFFECT_COUNT] = [
        Self::Poison,
        Self::Freeze,
        Self::Bleeding,
        Self::Rotten,
        Self::Slow,
        Self::Speed,
        Self::Strength,
        Self::WaterAffinity,
        Self::OnFire,
        Self::FireResistance,
        Self::FreezeResistance,
        Self::Regeneration,
        Self::ShieldPower,
        Self::NightVision,
    ];

    pub const fn index(self) -> usize {
        self as usize
    }

    pub fn identifier(self) -> &'static str {
        effect_definition(self).id.as_str()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectStacking {
    RefreshStrongest,
    Extend,
    Replace,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EffectDefinition {
    pub kind: EffectKind,
    pub id: String,
    pub name: String,
    pub description: String,
    pub beneficial: bool,
    pub max_duration_seconds: f32,
    pub max_level: u8,
    pub stacking: EffectStacking,
    #[serde(default)]
    pub modifiers: EffectModifiers,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct EffectModifiers {
    pub damage_per_second: f32,
    pub health_per_second: f32,
    pub hunger_per_second: f32,
    pub movement_multiplier: f32,
    pub swimming_multiplier: f32,
    pub attack_multiplier: f32,
    pub fire_damage_multiplier: f32,
    pub freeze_damage_multiplier: f32,
    pub oxygen_multiplier: f32,
    pub shield_points: f32,
}

impl Default for EffectModifiers {
    fn default() -> Self {
        Self {
            damage_per_second: 0.0,
            health_per_second: 0.0,
            hunger_per_second: 0.0,
            movement_multiplier: 1.0,
            swimming_multiplier: 1.0,
            attack_multiplier: 1.0,
            fire_damage_multiplier: 1.0,
            freeze_damage_multiplier: 1.0,
            oxygen_multiplier: 1.0,
            shield_points: 0.0,
        }
    }
}

impl EffectDefinition {
    pub fn valid(&self) -> bool {
        self.id.starts_with("teramic::")
            && !self.name.trim().is_empty()
            && self.name.len() <= 96
            && self.description.len() <= 512
            && self.max_duration_seconds.is_finite()
            && (0.1..=MAX_EFFECT_SECONDS).contains(&self.max_duration_seconds)
            && (1..=10).contains(&self.max_level)
            && [
                self.modifiers.damage_per_second,
                self.modifiers.health_per_second,
                self.modifiers.hunger_per_second,
            ]
            .into_iter()
            .all(|value| value.is_finite() && (0.0..=10.0).contains(&value))
            && [
                self.modifiers.movement_multiplier,
                self.modifiers.swimming_multiplier,
                self.modifiers.attack_multiplier,
            ]
            .into_iter()
            .all(|value| value.is_finite() && (0.1..=4.0).contains(&value))
            && [
                self.modifiers.fire_damage_multiplier,
                self.modifiers.freeze_damage_multiplier,
                self.modifiers.oxygen_multiplier,
            ]
            .into_iter()
            .all(|value| value.is_finite() && (0.0..=1.0).contains(&value))
            && self.modifiers.shield_points.is_finite()
            && (0.0..=100.0).contains(&self.modifiers.shield_points)
    }
}

const SOURCES: [(&str, &str); EFFECT_COUNT] = [
    (
        "poison",
        include_str!("../../../assets/terra/effects/poison.json"),
    ),
    (
        "freeze",
        include_str!("../../../assets/terra/effects/freeze.json"),
    ),
    (
        "bleeding",
        include_str!("../../../assets/terra/effects/bleeding.json"),
    ),
    (
        "rotten",
        include_str!("../../../assets/terra/effects/rotten.json"),
    ),
    (
        "slow",
        include_str!("../../../assets/terra/effects/slow.json"),
    ),
    (
        "speed",
        include_str!("../../../assets/terra/effects/speed.json"),
    ),
    (
        "strength",
        include_str!("../../../assets/terra/effects/strength.json"),
    ),
    (
        "water_affinity",
        include_str!("../../../assets/terra/effects/water_affinity.json"),
    ),
    (
        "on_fire",
        include_str!("../../../assets/terra/effects/on_fire.json"),
    ),
    (
        "fire_resistance",
        include_str!("../../../assets/terra/effects/fire_resistance.json"),
    ),
    (
        "freeze_resistance",
        include_str!("../../../assets/terra/effects/freeze_resistance.json"),
    ),
    (
        "regeneration",
        include_str!("../../../assets/terra/effects/regeneration.json"),
    ),
    (
        "shield_power",
        include_str!("../../../assets/terra/effects/shield_power.json"),
    ),
    (
        "nightvision",
        include_str!("../../../assets/terra/effects/nightvision.json"),
    ),
];

fn parse_definition(
    source: &str,
    expected: EffectKind,
    id: &str,
) -> Result<EffectDefinition, String> {
    let definition: EffectDefinition = serde_json::from_str(source).map_err(|e| e.to_string())?;
    if definition.kind != expected || definition.id != id || !definition.valid() {
        return Err("Invalid effect identity, duration, level or modifiers".into());
    }
    Ok(definition)
}

pub fn effect_definition(kind: EffectKind) -> &'static EffectDefinition {
    static DEFINITIONS: OnceLock<[EffectDefinition; EFFECT_COUNT]> = OnceLock::new();
    &DEFINITIONS.get_or_init(|| {
        std::array::from_fn(|index| {
            let (name, source) = SOURCES[index];
            let id = format!("teramic::{name}");
            let fallback = parse_definition(source, EffectKind::ALL[index], &id)
                .expect("embedded effect definitions must be valid");
            let path = crate::asset_directory()
                .join("terra/effects")
                .join(format!("{name}.json"));
            match std::fs::read_to_string(&path) {
                Ok(source) => match parse_definition(&source, EffectKind::ALL[index], &id) {
                    Ok(definition) => definition,
                    Err(error) => {
                        eprintln!(
                            "Invalid effect definition {}: {error}; using builtin values",
                            path.display()
                        );
                        fallback
                    }
                },
                Err(_) => fallback,
            }
        })
    })[kind.index()]
}

fn fingerprint_definitions<'a>(definitions: impl IntoIterator<Item = &'a EffectDefinition>) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    let mut feed = |byte: u8| {
        hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
    };
    for byte in b"teramic-status-effects-v1\0" {
        feed(*byte);
    }
    for definition in definitions {
        // Struct serialization fixes field order, independent of source JSON formatting/key order.
        for byte in serde_json::to_vec(definition).expect("validated effect definitions serialize")
        {
            feed(byte);
        }
        feed(0);
    }
    hash
}

/// Content-contract checksum, not authentication: reject divergent client prediction rules.
pub fn effect_catalog_fingerprint() -> u64 {
    static FINGERPRINT: OnceLock<u64> = OnceLock::new();
    *FINGERPRINT.get_or_init(|| fingerprint_definitions(EffectKind::ALL.map(effect_definition)))
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ActiveEffect {
    pub kind: EffectKind,
    pub remaining_seconds: f32,
    pub level: u8,
}

/// At most one timed instance per kind. Smoke exposure is separate from jellyfish poison.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct StatusEffects {
    #[serde(with = "effect_slots")]
    slots: [Option<ActiveEffect>; EFFECT_COUNT],
    smoke_exposure: bool,
    shield: f32,
}

mod effect_slots {
    use super::{ActiveEffect, EFFECT_COUNT, MAX_EFFECT_SECONDS};
    use serde::{
        Deserializer, Serializer,
        de::{self, SeqAccess, Visitor},
        ser::SerializeSeq,
    };

    pub fn serialize<S: Serializer>(
        slots: &[Option<ActiveEffect>; EFFECT_COUNT],
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        let mut sequence = serializer.serialize_seq(Some(slots.iter().flatten().count()))?;
        for effect in slots.iter().flatten() {
            sequence.serialize_element(effect)?;
        }
        sequence.end()
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<[Option<ActiveEffect>; EFFECT_COUNT], D::Error> {
        struct EffectSlotsVisitor;
        impl<'de> Visitor<'de> for EffectSlotsVisitor {
            type Value = [Option<ActiveEffect>; EFFECT_COUNT];
            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(
                    formatter,
                    "at most {EFFECT_COUNT} unique, finite status effects"
                )
            }
            fn visit_seq<A: SeqAccess<'de>>(
                self,
                mut sequence: A,
            ) -> Result<Self::Value, A::Error> {
                if sequence
                    .size_hint()
                    .is_some_and(|count| count > EFFECT_COUNT)
                {
                    return Err(de::Error::custom("too many status effects"));
                }
                let mut slots = [None; EFFECT_COUNT];
                while let Some(effect) = sequence.next_element::<ActiveEffect>()? {
                    let index = effect.kind.index();
                    if slots[index].is_some()
                        || !effect.remaining_seconds.is_finite()
                        || !(0.0..=MAX_EFFECT_SECONDS).contains(&effect.remaining_seconds)
                        || effect.remaining_seconds == 0.0
                        || !(1..=10).contains(&effect.level)
                    {
                        return Err(de::Error::custom("duplicate or invalid status effect"));
                    }
                    slots[index] = Some(effect);
                }
                Ok(slots)
            }
        }
        deserializer.deserialize_seq(EffectSlotsVisitor)
    }
}

impl Default for StatusEffects {
    fn default() -> Self {
        Self {
            slots: [None; EFFECT_COUNT],
            smoke_exposure: false,
            shield: 0.0,
        }
    }
}

impl StatusEffects {
    pub fn valid(self) -> bool {
        self.bounded()
            && self.shield <= self.shield_capacity()
            && self.slots.into_iter().flatten().all(|effect| {
                let definition = effect_definition(effect.kind);
                effect.level <= definition.max_level
                    && effect.remaining_seconds <= definition.max_duration_seconds
            })
    }

    fn bounded(self) -> bool {
        self.shield.is_finite()
            && (0.0..=100.0).contains(&self.shield)
            && (self.slots[EffectKind::ShieldPower.index()].is_some() || self.shield == 0.0)
            && self.slots.into_iter().enumerate().all(|(index, slot)| {
                slot.is_none_or(|effect| {
                    effect.kind.index() == index
                        && effect.remaining_seconds.is_finite()
                        && (0.0..=MAX_EFFECT_SECONDS).contains(&effect.remaining_seconds)
                        && effect.remaining_seconds > 0.0
                        && (1..=10).contains(&effect.level)
                })
            })
    }

    /// Loading after intentional JSON edits can only reduce stored duration, level and charge.
    pub fn normalize_to_definitions(&mut self) -> Result<(), &'static str> {
        if !self.bounded() {
            return Err("Invalid stored status effect bounds");
        }
        for effect in self.slots.iter_mut().flatten() {
            let definition = effect_definition(effect.kind);
            effect.level = effect.level.min(definition.max_level);
            effect.remaining_seconds = effect
                .remaining_seconds
                .min(definition.max_duration_seconds);
        }
        self.shield = self.shield.min(self.shield_capacity());
        Ok(())
    }

    pub fn apply(&mut self, kind: EffectKind, seconds: f32, level: u8) -> bool {
        let definition = effect_definition(kind);
        if !seconds.is_finite() || seconds <= 0.0 || level == 0 || level > definition.max_level {
            return false;
        }
        let duration = seconds.min(definition.max_duration_seconds);
        let slot = &mut self.slots[kind.index()];
        let new = if let Some(current) = *slot {
            match definition.stacking {
                EffectStacking::RefreshStrongest => ActiveEffect {
                    kind,
                    remaining_seconds: current.remaining_seconds.max(duration),
                    level: current.level.max(level),
                },
                EffectStacking::Extend => ActiveEffect {
                    kind,
                    remaining_seconds: (current.remaining_seconds + duration)
                        .min(definition.max_duration_seconds),
                    level: current.level.max(level),
                },
                EffectStacking::Replace => ActiveEffect {
                    kind,
                    remaining_seconds: duration,
                    level,
                },
            }
        } else {
            ActiveEffect {
                kind,
                remaining_seconds: duration,
                level,
            }
        };
        let changed = *slot != Some(new);
        *slot = Some(new);
        let previous_shield = self.shield;
        if kind == EffectKind::ShieldPower {
            self.shield = self
                .shield
                .max((definition.modifiers.shield_points * f32::from(new.level)).min(100.0));
        }
        changed || self.shield != previous_shield
    }

    pub fn remove(&mut self, kind: EffectKind) -> bool {
        if kind == EffectKind::ShieldPower {
            self.shield = 0.0;
        }
        self.slots[kind.index()].take().is_some()
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn set_smoke_exposure(&mut self, exposed: bool) {
        self.smoke_exposure = exposed;
    }

    pub fn smoke_exposed(self) -> bool {
        self.smoke_exposure
    }

    pub fn get(self, kind: EffectKind) -> Option<ActiveEffect> {
        self.slots[kind.index()].or_else(|| {
            (kind == EffectKind::Poison && self.smoke_exposure).then_some(ActiveEffect {
                kind,
                remaining_seconds: 0.0,
                level: 1,
            })
        })
    }

    pub fn timed(self, kind: EffectKind) -> Option<ActiveEffect> {
        self.slots[kind.index()]
    }

    pub fn contains(self, kind: EffectKind) -> bool {
        self.get(kind).is_some()
    }

    pub fn iter(self) -> impl Iterator<Item = ActiveEffect> {
        EffectKind::ALL
            .into_iter()
            .filter_map(move |kind| self.get(kind))
    }

    pub(crate) fn timed_effects(self) -> impl Iterator<Item = ActiveEffect> {
        self.slots.into_iter().flatten()
    }

    /// Prediction can advance timers without committing authority-only damage.
    pub fn advance(&mut self, dt: f32) {
        if !dt.is_finite() || dt <= 0.0 {
            return;
        }
        let dt = dt.min(0.25);
        for slot in &mut self.slots {
            if let Some(effect) = slot {
                effect.remaining_seconds = (effect.remaining_seconds - dt).max(0.0);
                if effect.remaining_seconds <= 0.0 {
                    *slot = None;
                }
            }
        }
        if self.slots[EffectKind::ShieldPower.index()].is_none() {
            self.shield = 0.0;
        }
    }

    fn factor(self, select: impl Fn(EffectModifiers) -> f32) -> f32 {
        self.iter().fold(1.0, |factor, effect| {
            factor * select(effect_definition(effect.kind).modifiers).powi(i32::from(effect.level))
        })
    }

    pub fn movement_multiplier(self, swimming: bool) -> f32 {
        let movement = self
            .iter()
            .filter(|effect| {
                effect.kind != EffectKind::Freeze || !self.contains(EffectKind::FreezeResistance)
            })
            .fold(1.0, |factor, effect| {
                factor
                    * effect_definition(effect.kind)
                        .modifiers
                        .movement_multiplier
                        .powi(i32::from(effect.level))
            });
        (movement
            * if swimming {
                self.factor(|m| m.swimming_multiplier)
            } else {
                1.0
            })
        .clamp(0.1, 4.0)
    }

    pub fn attack_multiplier(self) -> f32 {
        self.factor(|m| m.attack_multiplier).clamp(0.1, 4.0)
    }

    pub fn oxygen_multiplier(self) -> f32 {
        self.factor(|m| m.oxygen_multiplier).clamp(0.0, 1.0)
    }

    pub fn fire_damage_multiplier(self) -> f32 {
        self.factor(|m| m.fire_damage_multiplier).clamp(0.0, 1.0)
    }

    pub fn freeze_damage_multiplier(self) -> f32 {
        self.factor(|m| m.freeze_damage_multiplier).clamp(0.0, 1.0)
    }

    pub fn shield_points(self) -> f32 {
        self.shield
    }

    pub fn shield_capacity(self) -> f32 {
        self.get(EffectKind::ShieldPower).map_or(0.0, |effect| {
            (effect_definition(effect.kind).modifiers.shield_points * f32::from(effect.level))
                .min(100.0)
        })
    }

    pub fn absorb_damage(&mut self, amount: f32) -> f32 {
        if !amount.is_finite() || amount <= 0.0 {
            return 0.0;
        }
        let absorbed = amount.min(self.shield);
        self.shield -= absorbed;
        amount - absorbed
    }
}

#[cfg(test)]
mod tests;
