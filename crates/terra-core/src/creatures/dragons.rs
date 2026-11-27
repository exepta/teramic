//! Shared timing of dragon poses, attack impacts and their audiovisual cues.
use super::{Creature, CreatureBehavior as B, CreatureKind as K};

/// The appended forms keep the saved IDs of the original species and coats.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lineage {
    Shadow,
    Tidal,
    Stone,
}

pub const fn lineage(kind: K, variant: u8) -> Option<Lineage> {
    match (kind, variant) {
        (K::DarkDragon, 3..=5) => Some(Lineage::Shadow),
        (K::WaterDragon, 3..=5) => Some(Lineage::Tidal),
        (K::EarthDragon, 1..=3) => Some(Lineage::Stone),
        _ => None,
    }
}

pub const fn impact_phase(action: B) -> f32 {
    match action {
        B::DragonVolley => 0.72,
        B::DragonSurge => 0.61,
        _ => 0.55,
    }
}

/// All volley geometry, including the spread at the target, is shared by the
/// server's sweep and the client meshes. Positions are in world metres.
pub fn missile_path(
    form: Lineage,
    index: u8,
    t: f32,
    start: bevy_math::Vec3,
    aim: bevy_math::Vec3,
) -> bevy_math::Vec3 {
    use bevy_math::Vec3;
    let dir = aim - start;
    let side = dir.cross(Vec3::Y).try_normalize().unwrap_or(Vec3::X);
    let arch = 4. * t * (1. - t);
    let spread = f32::from(index) - 1.;
    let (apex, fan) = match form {
        Lineage::Shadow => (1.1, 3.6),
        Lineage::Tidal => (2.8, 2.1),
        Lineage::Stone => (dir.length().min(36.) * 0.22, 1.6),
    };
    start.lerp(aim, t)
        + Vec3::Y * (arch * (apex + spread.abs() * 0.9))
        + side * spread * (fan * arch + t * 2.5)
}

pub fn enraged(a: &Creature) -> bool {
    lineage(a.kind, a.variant) == Some(Lineage::Shadow)
        && a.health > 0.
        && a.health < a.kind.health() * 0.30
}

pub fn cast_damage(a: &Creature) -> f32 {
    let base = super::definitions::get(a.kind)
        .combat
        .expect("dragon combat definition")
        .damage;
    if enraged(a) { base * 1.25 } else { base }
}

/// Absorption begins after the ward has visibly risen and ends as it fades.
pub fn incoming_damage(a: &Creature, damage: f32) -> f32 {
    if a.behavior == B::DragonWard
        && lineage(a.kind, a.variant).is_some()
        && (0.22..0.82).contains(&phase(a.behavior, a.timer))
    {
        damage * 0.30
    } else {
        damage
    }
}

pub const fn duration(behavior: B) -> f32 {
    match behavior {
        B::Emerge => 1.6,
        B::Leap => 1.4,
        B::Land => 1.35,
        B::Spit => 2.4,
        B::Attack => 1.5,
        B::Tremor => 2.0,
        B::Ink => 2.6,
        B::DragonVolley => 2.65,
        B::DragonWard => 3.1,
        B::DragonSurge => 2.3,
        B::DragonBurrow => 4.4,
        B::Dead => 1.8,
        _ => 1.0,
    }
}

pub fn phase(behavior: B, remaining: f32) -> f32 {
    (1.0 - remaining / duration(behavior)).clamp(0.0, 1.0)
}

pub const fn model_scale(kind: K, variant: u8) -> f32 {
    match lineage(kind, variant) {
        Some(Lineage::Stone) => return 1.38,
        Some(Lineage::Shadow) => return 1.36,
        Some(Lineage::Tidal) => return 1.20,
        None => {}
    }
    match kind {
        super::CreatureKind::EarthDragon => 0.88,
        super::CreatureKind::DarkDragon => 1.10,
        _ => 1.0,
    }
}

pub const fn flies(kind: K) -> bool {
    matches!(kind, K::DarkDragon | K::FireDragon)
}

pub const fn wing_scale(kind: K, variant: u8) -> f32 {
    if matches!(lineage(kind, variant), Some(Lineage::Stone)) {
        0.55
    } else {
        1.0
    }
}

pub fn occupied_size(kind: K, variant: u8) -> [f64; 3] {
    let ratio = f64::from(model_scale(kind, variant) / model_scale(kind, 0));
    kind.size().map(|s| s * ratio)
}

pub fn muzzle(animal: &super::Creature) -> [f64; 3] {
    let scale = f64::from(model_scale(animal.kind, animal.variant)) * animal.growth_scale();
    let (sin, cos) = f64::from(animal.yaw).sin_cos();
    let local = super::dragon_pose::Pose::of(animal, 0.0)
        .bone(super::dragon_pose::Joint::Breath)
        .translation;
    let [x, y, z] = local.to_array().map(|v| f64::from(v) * scale);
    [
        animal.position[0] + cos * x + sin * z,
        animal.position[1] + y,
        animal.position[2] - sin * x + cos * z,
    ]
}

pub const fn casting(behavior: B) -> bool {
    matches!(
        behavior,
        B::Attack | B::Spit | B::Tremor | B::Ink | B::DragonVolley | B::DragonSurge | B::DragonWard
    )
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum DragonLoot {
    DarkScale,
    EarthScale,
    WaterScale,
    Fang,
    Hide,
    ShadowCore,
    Blood,
    ShadowCrystal,
    ShadowHeart,
    Membrane,
    EarthCrystal,
    RawMeat,
    CookedMeat,
    Dust,
    Claw,
    Horn,
    WaterHeart,
    IceEssence,
    WaterEssence,
}

impl DragonLoot {
    pub const fn slug(self) -> &'static str {
        match self {
            Self::DarkScale => "dragon_dark_scale",
            Self::EarthScale => "dragon_earth_scale",
            Self::WaterScale => "dragon_water_scale",
            Self::Fang => "dragon_fang",
            Self::Hide => "dragon_hide",
            Self::ShadowCore => "dragon_shadow_core",
            Self::Blood => "dragon_blood",
            Self::ShadowCrystal => "dragon_shadow_crystal",
            Self::ShadowHeart => "dragon_shadow_heart",
            Self::Membrane => "dragon_membrane",
            Self::EarthCrystal => "dragon_earth_crystal",
            Self::RawMeat => "dragon_raw_meat",
            Self::CookedMeat => "dragon_cooked_meat",
            Self::Dust => "dragon_dust",
            Self::Claw => "dragon_claw",
            Self::Horn => "dragon_horn",
            Self::WaterHeart => "dragon_water_heart",
            Self::IceEssence => "dragon_ice_essence",
            Self::WaterEssence => "dragon_water_essence",
        }
    }
}
