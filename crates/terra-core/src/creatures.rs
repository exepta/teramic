//! Persistent authority data; no renderer or ECS required.
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
pub mod breeding;
pub mod dragon_eggs;
pub mod dragon_life;
pub mod sex;
pub use sex::CreatureSex;
pub mod definitions;
pub mod dragon_pose;
pub mod dragons;
pub mod drops;
pub mod ecology;
pub mod gloom;
pub mod golem;
pub mod hitboxes;
pub mod horses;
pub mod intentions;
pub mod lava_worm;
pub mod sandworm;
pub mod soldiers;
pub mod spawning;
pub mod spiders;
pub mod taming;
pub mod villagers;

pub const MAX_CREATURES: usize = 16_384;
pub const MAX_SPAWN_CELLS: usize = 65_536;
pub const MAX_VISIBLE_CREATURES: usize = 128;

/// Refill and jet phase shared by squid locomotion and mantle animation.
pub fn squid_jet(age: f64) -> f32 {
    let phase = (age * 0.62).fract() as f32;
    ((phase - 0.20).rem_euclid(1.) * std::f32::consts::TAU)
        .cos()
        .max(0.)
        .powi(2)
}

pub const SKELETON_RISE_SECONDS: f32 = 3.4;
pub fn skeleton_rise(remaining: f32) -> f32 {
    let p = (1.0 - remaining / SKELETON_RISE_SECONDS).clamp(0., 1.);
    p * p * (3. - 2. * p)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CreatureKind {
    Sheep,
    Cow,
    Pig,
    Fish,
    Bee,
    Bear,
    Squid,
    MoonJelly,
    FireJelly,
    VoidJelly,
    SunJelly,
    CrystalJelly,
    ReefShark,
    HammerheadShark,
    WhaleShark,
    Horse,
    SmallSpider,
    MediumSpider,
    LargeSpider,
    Stalker,
    Villager,
    // Append-only: serde/postcard discriminants are saved in worlds and sent over the wire.
    Butterfly,
    SpiritGloom,
    SpiritFox,
    Skeleton,
    Raven,
    BloodSlime,
    BloodStalker,
    BloodWisp,
    ThornBrute,
    SandWorm,
    DarkDragon,
    EarthDragon,
    WaterDragon,
    LavaWorm,
    Elephant,
    Zebra,
    FireDragon,
    StoneGolem,
}
impl CreatureKind {
    pub const ALL: [Self; 39] = [
        Self::Sheep,
        Self::Cow,
        Self::Pig,
        Self::Fish,
        Self::Bee,
        Self::Bear,
        Self::Squid,
        Self::MoonJelly,
        Self::FireJelly,
        Self::VoidJelly,
        Self::SunJelly,
        Self::CrystalJelly,
        Self::ReefShark,
        Self::HammerheadShark,
        Self::WhaleShark,
        Self::Horse,
        Self::SmallSpider,
        Self::MediumSpider,
        Self::LargeSpider,
        Self::Stalker,
        Self::Villager,
        Self::Butterfly,
        Self::SpiritGloom,
        Self::SpiritFox,
        Self::Skeleton,
        Self::Raven,
        Self::BloodSlime,
        Self::BloodStalker,
        Self::BloodWisp,
        Self::ThornBrute,
        Self::SandWorm,
        Self::DarkDragon,
        Self::EarthDragon,
        Self::WaterDragon,
        Self::LavaWorm,
        Self::Elephant,
        Self::Zebra,
        Self::FireDragon,
        Self::StoneGolem,
    ];
    pub const fn is_dragon(self) -> bool {
        matches!(
            self,
            Self::DarkDragon | Self::EarthDragon | Self::FireDragon | Self::WaterDragon
        )
    }
    pub const fn is_flying(self) -> bool {
        matches!(
            self,
            Self::Bee
                | Self::Butterfly
                | Self::SpiritGloom
                | Self::Raven
                | Self::BloodWisp
                | Self::DarkDragon
                | Self::FireDragon
                | Self::EarthDragon
                | Self::WaterDragon
        )
    }
    pub const fn variant_count(self) -> u8 {
        match self {
            Self::Villager => villagers::ROLE_COUNT,
            Self::Horse => 16,
            Self::Butterfly => 7,
            Self::SpiritGloom => gloom::COUNT,
            Self::SpiritFox
            | Self::Raven
            | Self::BloodSlime
            | Self::BloodStalker
            | Self::BloodWisp
            | Self::ThornBrute
            | Self::SandWorm
            | Self::LavaWorm
            | Self::Elephant
            | Self::Zebra => 1,
            Self::StoneGolem => 2,
            Self::Skeleton => 3,
            Self::DarkDragon | Self::WaterDragon => 6,
            _ => 4,
        }
    }
    pub fn is_spider(self) -> bool {
        matches!(
            self,
            Self::SmallSpider | Self::MediumSpider | Self::LargeSpider
        )
    }
    pub fn is_aquatic(self) -> bool {
        self == Self::Fish || self == Self::Squid || self.is_jelly() || self.is_shark()
    }
    pub fn is_jelly(self) -> bool {
        matches!(
            self,
            Self::MoonJelly
                | Self::FireJelly
                | Self::VoidJelly
                | Self::SunJelly
                | Self::CrystalJelly
        )
    }
    pub fn is_shark(self) -> bool {
        matches!(
            self,
            Self::ReefShark | Self::HammerheadShark | Self::WhaleShark
        )
    }
    pub fn hostile_to_players(self) -> bool {
        matches!(
            self,
            Self::Bear
                | Self::StoneGolem
                | Self::ReefShark
                | Self::WhaleShark
                | Self::MediumSpider
                | Self::LargeSpider
                | Self::Stalker
                | Self::Skeleton
                | Self::BloodStalker
                | Self::BloodWisp
                | Self::ThornBrute
                | Self::SandWorm
                | Self::LavaWorm
                | Self::DarkDragon
                | Self::FireDragon
                | Self::EarthDragon
                | Self::WaterDragon
        )
    }
    pub fn health(self) -> f32 {
        definitions::get(self).health
    }
    pub fn food(self) -> f32 {
        definitions::get(self).food
    }
    pub fn water(self) -> f32 {
        definitions::get(self).water
    }
    pub fn size(self) -> [f64; 3] {
        definitions::get(self).size
    }
    pub fn speed(self) -> f64 {
        definitions::get(self).speed
    }
    pub fn product(self) -> Option<AnimalProduct> {
        Some(match self {
            Self::Sheep => AnimalProduct::Mutton,
            Self::Cow => AnimalProduct::Beef,
            Self::Pig => AnimalProduct::Pork,
            Self::Fish => AnimalProduct::Fish,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnimalProduct {
    Mutton,
    Beef,
    Pork,
    Fish,
    Wool,
    CookedBeef,
    CookedPork,
    CookedMutton,
    CookedFish,
    Clownfish,
    CookedClownfish,
    Leather,
    Honey,
}
impl AnimalProduct {
    pub const ALL: [Self; 13] = [
        Self::Mutton,
        Self::Beef,
        Self::Pork,
        Self::Fish,
        Self::Wool,
        Self::CookedBeef,
        Self::CookedPork,
        Self::CookedMutton,
        Self::CookedFish,
        Self::Clownfish,
        Self::CookedClownfish,
        Self::Leather,
        Self::Honey,
    ];
    pub fn slug(self) -> &'static str {
        match self {
            Self::Mutton => "raw_mutton",
            Self::Beef => "raw_beef",
            Self::Pork => "raw_pork",
            Self::Fish => "raw_fish",
            Self::Wool => "wool",
            Self::CookedBeef => "cooked_beef",
            Self::CookedPork => "cooked_pork",
            Self::CookedMutton => "cooked_mutton",
            Self::CookedFish => "cooked_fish",
            Self::Clownfish => "raw_clownfish",
            Self::CookedClownfish => "cooked_clownfish",
            Self::Leather => "leather",
            Self::Honey => "honey",
        }
    }
    pub fn color(self) -> [f32; 3] {
        match self {
            Self::Mutton => [0.65, 0.26, 0.24],
            Self::Beef => [0.60, 0.12, 0.12],
            Self::Pork => [0.89, 0.53, 0.48],
            Self::Fish => [0.36, 0.53, 0.64],
            Self::Wool => [0.93, 0.90, 0.79],
            Self::CookedBeef | Self::CookedPork | Self::CookedMutton => [0.57, 0.28, 0.12],
            Self::CookedFish => [0.65, 0.40, 0.17],
            Self::Clownfish => [1.0, 0.38, 0.055],
            Self::CookedClownfish => [0.78, 0.36, 0.09],
            Self::Leather => [0.49, 0.25, 0.10],
            Self::Honey => [1.0, 0.65, 0.07],
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CreatureBehavior {
    #[default]
    Rest,
    Wander,
    Graze,
    Flee,
    Swim,
    Stranded,
    Dead,
    SeekFood,
    SeekWater,
    Drink,
    Shake,
    Fly,
    Land,
    Nectar,
    Crawl,
    Sleep,
    InNest,
    Leap,
    Hunt,
    Attack,
    Climb,
    Hold,
    Ink,
    Ceiling,
    Descend,
    Burrowed,
    Tremor,
    Emerge,
    Retreat,
    /// A locked, aimed sandworm projectile; the existing need goal carries its aim.
    Spit,
    DragonVolley,
    DragonWard,
    DragonSurge,
    DragonBurrow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NeedKind {
    Food,
    Water,
    Tree,
    Shelter,
    /// An explicit combat aim, never a food/water/pathfinding intention.
    AttackAim,
    /// A ranged dragon cast made aloft; persists while the body brakes or hovers.
    AirborneAttackAim,
    /// Original resting place of a territorial stone guardian.
    Territory,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct NeedGoal {
    pub kind: NeedKind,
    pub position: [i64; 3],
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CreatureNeeds {
    /// Stored with the egg registry and sent with the family snapshot. Historical
    /// region records embed CreatureNeeds and keep their original binary layout.
    #[serde(skip)]
    pub dragon: Option<dragon_life::DragonLife>,
    pub food: f32,
    pub water: f32,
    pub goal: Option<NeedGoal>,
    /// Seconds until the next bounded resource search or completed bite/sip.
    pub search_in: f32,
}
impl CreatureNeeds {
    pub fn full(kind: CreatureKind) -> Self {
        Self {
            food: kind.food(),
            water: kind.water(),
            ..Self::default()
        }
    }
    pub fn valid(&self, kind: CreatureKind) -> bool {
        self.food.is_finite()
            && (0.0..=kind.food()).contains(&self.food)
            && self.water.is_finite()
            && (0.0..=kind.water()).contains(&self.water)
            && self.search_in.is_finite()
            && (0.0..=15.0).contains(&self.search_in)
            && self.goal.is_none_or(|goal| {
                !kind.is_aquatic()
                    && (goal.kind != NeedKind::Territory || kind == CreatureKind::StoneGolem)
                    && (goal.kind != NeedKind::Tree || kind == CreatureKind::Bear)
                    && (goal.kind != NeedKind::Shelter || kind.is_spider())
                    && (goal.kind != NeedKind::AirborneAttackAim || kind.is_dragon())
                    && (goal.kind != NeedKind::AttackAim
                        || matches!(kind, CreatureKind::SandWorm | CreatureKind::LavaWorm)
                        || kind.is_spider()
                        || kind.is_dragon())
                    && goal.position.iter().all(|v| v.unsigned_abs() <= 999_000)
            })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Creature {
    pub id: u64,
    pub kind: CreatureKind,
    pub home: [i64; 2],
    /// Model base / feet; the occupied volume extends upwards.
    pub position: [f64; 3],
    pub velocity: [f32; 3],
    pub yaw: f32,
    pub health: f32,
    pub behavior: CreatureBehavior,
    pub timer: f32,
    pub age: f64,
    pub variant: u8,
    #[serde(default)]
    pub needs: CreatureNeeds,
}
impl Creature {
    pub fn title(&self) -> String {
        let role = definitions::get(self.kind).variant_title(self.variant);
        if self.kind == CreatureKind::Villager {
            format!("{} ({role})", villagers::name(self.id))
        } else {
            role.to_owned()
        }
    }
    /// Stable individual proportions; no extra save fields or runtime random IDs.
    pub fn aquatic_scale(&self) -> [f32; 3] {
        let Some(minimum) = definitions::get(self.kind).minimum_size else {
            return [1.0; 3];
        };
        let mut hash = self.id ^ 0xa43d_19e8_5cf2_0871;
        hash = (hash ^ (hash >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        hash = (hash ^ (hash >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        let t = ((hash ^ (hash >> 31)) & 0xffff) as f64 / 65535.0;
        let maximum = self.kind.size();
        std::array::from_fn(|i| ((minimum[i] + (maximum[i] - minimum[i]) * t) / maximum[i]) as f32)
    }
    pub fn size(&self) -> [f64; 3] {
        if self.kind.is_spider() {
            return spiders::occupied_size(self);
        }
        let scale = self.aquatic_scale();
        let maximum = dragons::occupied_size(self.kind, self.variant);
        std::array::from_fn(|i| maximum[i] * f64::from(scale[i]) * self.growth_scale())
    }
    pub fn dragon_stage(&self) -> dragon_life::DragonStage {
        dragon_life::DragonStage::at(
            self.needs
                .dragon
                .map_or_else(|| f64::from(self.growth()) * 12., |life| life.months),
        )
    }
    pub fn growth(&self) -> f32 {
        if let Some(life) = self.needs.dragon {
            return (life.months / 12.) as f32;
        }
        if breeding::is_birth_id(self.id) {
            (self.age / breeding::GROWTH_SECONDS).clamp(0.0, 1.0) as f32
        } else {
            1.0
        }
    }
    pub fn sex(&self) -> CreatureSex {
        match (self.kind, self.variant) {
            (CreatureKind::Villager, villagers::SWAMP_WITCH) => CreatureSex::Female,
            (CreatureKind::Villager, villagers::SWAMP_WARLOCK) => CreatureSex::Male,
            _ => CreatureSex::from_spawn_id(self.id),
        }
    }
    pub fn juvenile(&self) -> bool {
        breeding::is_birth_id(self.id)
            && self
                .needs
                .dragon
                .map_or_else(|| self.growth() < 1.0, |life| life.months < 12.)
    }
    pub fn growth_scale(&self) -> f64 {
        if self.kind.is_dragon() && breeding::is_birth_id(self.id) {
            return self
                .needs
                .dragon
                .unwrap_or_else(|| dragon_life::DragonLife::legacy(self))
                .scale();
        }
        let growth = f64::from(self.growth());
        0.45 + 0.55 * growth * growth * (3.0 - 2.0 * growth)
    }
    pub fn movement_scale(&self) -> f64 {
        self.growth_scale().sqrt()
    }
    pub fn product(&self) -> Option<AnimalProduct> {
        if self.juvenile() {
            None
        } else if self.kind == CreatureKind::Fish && self.variant < 2 {
            Some(AnimalProduct::Clownfish)
        } else {
            self.kind.product()
        }
    }
    /// Saved needs remain valid when an administrator lowers a species' limits.
    pub fn apply_definition(&mut self) {
        for (value, maximum) in [
            (&mut self.health, self.kind.health()),
            (&mut self.needs.food, self.kind.food()),
            (&mut self.needs.water, self.kind.water()),
        ] {
            if value.is_finite() && (0.0..=1000.0).contains(value) {
                *value = value.min(maximum);
            }
        }
    }
    pub fn valid(&self) -> bool {
        self.id != 0
            && self
                .position
                .iter()
                .all(|v| v.is_finite() && v.abs() <= 999_000.0)
            // Starters store their 16 m kingdom claim as home; natural resident
            // identities retain the historical 32 m habitat bounds unchanged.
            && if crate::kingdom::is_starter_id(self.id) {
                self.kind == CreatureKind::Villager && crate::kingdom::valid_claim_chunk(self.home)
            } else {
                self.home.iter().all(|v| v.unsigned_abs() <= 31_250)
            }
            && if breeding::is_birth_id(self.id) {
                breeding::species(self.kind)
            } else if spawning::is_command_id(self.id) {
                true
            } else if self.kind == CreatureKind::Bee {
                (0..4).any(|slot| bee_id(self.home, slot) == Some(self.id))
            } else if self.kind.is_spider() {
                (0..4).any(|slot| spider_id(self.home, slot) == Some(self.id))
            } else if self.kind == CreatureKind::Stalker {
                stalker_id(self.home) == Some(self.id)
            } else if self.kind == CreatureKind::Skeleton && is_skeleton_id(self.id) {
                (0..2).any(|slot| skeleton_id(self.home,slot)==Some(self.id))
            } else if self.kind == CreatureKind::SpiritGloom && is_gloom_id(self.id) {
                gloom_id(self.home) == Some(self.id)
            } else if self.kind == CreatureKind::Villager {
                villagers::slot(self.id, self.home).is_some()
                    || villagers::is_swamp_resident(self.id, self.home)
                    || crate::kingdom::is_starter_id(self.id)
            } else {
                (0..4).any(|slot| creature_id(self.home, slot) == Some(self.id))
            }
            && self
                .velocity
                .iter()
                .all(|v| v.is_finite() && v.abs() <= 32.0)
            && self.yaw.is_finite()
            && self.yaw.abs() <= std::f32::consts::TAU + 0.01
            && self.health.is_finite()
            && (0.0..=self.kind.health()).contains(&self.health)
            && self.timer.is_finite()
            && (0.0..=60.0).contains(&self.timer)
            && self.age.is_finite()
            && self.age >= 0.0
            && self.variant < self.kind.variant_count()
            && self.needs.dragon.is_none_or(|life| self.kind.is_dragon() && breeding::is_birth_id(self.id) && life.valid())
            && self.needs.valid(self.kind)
            && (self.health > 0.0 || self.behavior == CreatureBehavior::Dead)
    }
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreaturePopulation {
    /// Ownership is separate from spawn identity; owned animals never consume the wild cap.
    pub owners: BTreeMap<u64, crate::PlayerUuid>,
    pub animals: BTreeMap<u64, Creature>,
    /// Includes empty habitats and dead animals: reload never rerolls a spawn.
    pub explored: BTreeSet<[i64; 2]>,
    #[serde(default)]
    pub ecology: ecology::Ecology,
}
impl CreaturePopulation {
    pub fn natural_count(&self) -> usize {
        self.animals
            .values()
            .filter(|a| {
                a.health > 0.0
                    && a.kind != CreatureKind::Villager
                    && !self.owners.contains_key(&a.id)
            })
            .count()
    }
    pub fn valid(&self) -> bool {
        self.ecology.valid()
            && self
                .owners
                .iter()
                .all(|(id, owner)| !owner.is_nil() && self.animals.contains_key(id))
            && self.ecology.bees.keys().all(|id| {
                self.animals
                    .get(id)
                    .is_some_and(|a| a.kind == CreatureKind::Bee)
            })
            && self.animals.len() <= MAX_CREATURES
            && self.explored.len() <= MAX_SPAWN_CELLS
            && self
                .explored
                .iter()
                .all(|p| p.iter().all(|v| v.unsigned_abs() <= 31_250))
            && self.animals.iter().all(|(id, a)| {
                *id == a.id
                    && a.valid()
                    && (spawning::is_command_id(a.id)
                        || breeding::is_birth_id(a.id)
                        || a.kind.is_spider()
                        || a.kind == CreatureKind::Stalker
                        || a.kind == CreatureKind::Villager
                        || a.kind == CreatureKind::Skeleton && is_skeleton_id(a.id)
                        || a.kind == CreatureKind::SpiritGloom && is_gloom_id(a.id)
                        || self.explored.contains(&a.home))
            })
    }
}

/// Stable injective identity within a world; independent of spawn/reload order.
pub fn creature_id(cell: [i64; 2], slot: u8) -> Option<u64> {
    if slot >= 4 || cell.iter().any(|v| v.unsigned_abs() > 31_250) {
        return None;
    }
    Some((((cell[0] + 32768) as u64) << 18 | ((cell[1] + 32768) as u64) << 2 | u64::from(slot)) + 1)
}

/// Separate namespace keeps every existing animal identity unchanged.
pub fn bee_id(cell: [i64; 2], slot: u8) -> Option<u64> {
    (slot < 4)
        .then(|| creature_id(cell, slot))
        .flatten()
        .map(|id| id | (1 << 63))
}

/// Separate habitat namespace: existing herds, bees and command identities stay unchanged.
pub fn spider_id(cell: [i64; 2], slot: u8) -> Option<u64> {
    creature_id(cell, slot).map(|id| id | (1 << 61))
}

/// Appended habitat namespace; old herds, spiders, bees and births keep their IDs.
pub fn stalker_id(cell: [i64; 2]) -> Option<u64> {
    creature_id(cell, 0).map(|id| id | (1 << 59))
}

/// Crypt guardians use a separate namespace below kingdoms/residents.
pub fn skeleton_id(cell: [i64; 2], slot: u8) -> Option<u64> {
    (slot < 2)
        .then(|| creature_id(cell, slot))
        .flatten()
        .map(|id| id | (1 << 56))
}
pub fn is_skeleton_id(id: u64) -> bool {
    id >> 56 == 1
}

/// Independent cave spirits do not consume or invalidate surface habitat slots.
pub fn gloom_id(cell: [i64; 2]) -> Option<u64> {
    creature_id(cell, 0).map(|id| id | (1 << 55))
}
pub fn is_gloom_id(id: u64) -> bool {
    id >> 55 == 1
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn appended_forest_species_and_crypt_identities_preserve_all_existing_numbers() {
        assert_eq!(CreatureKind::SpiritFox as u8, 23);
        assert_eq!(CreatureKind::Skeleton as u8, 24);
        assert_eq!(CreatureKind::ThornBrute as u8, 29);
        assert_eq!(CreatureKind::Skeleton.variant_count(), 3);
        let mut ids = BTreeSet::new();
        for x in -3..=3 {
            for z in -3..=3 {
                for slot in 0..2 {
                    let home = [x, z];
                    let id = skeleton_id(home, slot).unwrap();
                    assert!(ids.insert(id));
                    assert!(is_skeleton_id(id));
                    assert_ne!(Some(id), creature_id(home, slot));
                    assert_ne!(Some(id), bee_id(home, slot));
                    assert_ne!(Some(id), spider_id(home, slot));
                    assert_ne!(Some(id), stalker_id(home));
                    assert!(!crate::kingdom::is_starter_id(id));
                    assert!(!spawning::is_command_id(id));
                }
            }
        }
    }
    #[test]
    fn seven_gloom_variants_and_cave_identity_roundtrip_without_surface_slots() {
        let home = [-1, -2];
        let id = gloom_id(home).unwrap();
        assert!(is_gloom_id(id));
        assert!(!is_skeleton_id(id) && !spawning::is_command_id(id));
        assert!(!crate::kingdom::is_starter_id(id));
        for peer in [
            creature_id(home, 0),
            bee_id(home, 0),
            spider_id(home, 0),
            stalker_id(home),
            skeleton_id(home, 0),
        ] {
            assert_ne!(Some(id), peer);
        }
        for variant in 0..gloom::COUNT {
            let mut population = CreaturePopulation::default();
            population.animals.insert(
                id,
                Creature {
                    id,
                    kind: CreatureKind::SpiritGloom,
                    home,
                    position: [-4.5, 2.0, -5.5],
                    velocity: [0.0; 3],
                    yaw: 0.0,
                    health: 0.0,
                    behavior: CreatureBehavior::Dead,
                    timer: 2.0,
                    age: 18.0,
                    variant,
                    needs: CreatureNeeds::full(CreatureKind::SpiritGloom),
                },
            );
            assert!(population.valid());
            assert!(population.explored.is_empty());
            let restored: CreaturePopulation =
                serde_json::from_str(&serde_json::to_string(&population).unwrap()).unwrap();
            assert_eq!(restored, population);
            assert!(restored.valid());
        }
    }
    #[test]
    fn saved_crypt_guard_is_valid_without_consuming_a_surface_spawn_slot() {
        let id = skeleton_id([-1, -2], 1).unwrap();
        let mut population = CreaturePopulation::default();
        population.animals.insert(
            id,
            Creature {
                id,
                kind: CreatureKind::Skeleton,
                home: [-1, -2],
                position: [-4.5, 2.0, -5.5],
                velocity: [0.0; 3],
                yaw: 0.0,
                health: 0.0,
                behavior: CreatureBehavior::Dead,
                timer: 2.0,
                age: 18.0,
                variant: 2,
                needs: CreatureNeeds::full(CreatureKind::Skeleton),
            },
        );
        assert!(population.valid());
        assert!(population.explored.is_empty());
        let restored: CreaturePopulation =
            serde_json::from_str(&serde_json::to_string(&population).unwrap()).unwrap();
        assert_eq!(restored, population);
        assert!(restored.animals.contains_key(&id));
        assert!(restored.valid());
    }
    #[test]
    fn aquatic_kind_ids_append_to_existing_species_and_individual_dimensions_are_stable() {
        assert_eq!(CreatureKind::Sheep as usize, 0);
        assert_eq!(CreatureKind::Bear as usize, 5);
        assert_eq!(CreatureKind::Squid as usize, 6);
        assert_eq!(CreatureKind::WhaleShark as usize, 14);
        for kind in CreatureKind::ALL.into_iter().filter(|k| k.is_aquatic()) {
            for n in 0..128 {
                let a = Creature {
                    id: creature_id([n, 0], 0).unwrap(),
                    kind,
                    home: [n, 0],
                    position: [0.0, 5.0, 0.0],
                    velocity: [0.0; 3],
                    yaw: 0.0,
                    health: kind.health(),
                    behavior: CreatureBehavior::Swim,
                    timer: 0.0,
                    age: 0.0,
                    variant: 0,
                    needs: CreatureNeeds::full(kind),
                };
                assert!(a.valid());
                assert_eq!(a.size(), a.clone().size());
                for (actual, maximum) in a.size().into_iter().zip(kind.size()) {
                    assert!(actual > 0.0 && actual <= maximum + 0.000001);
                }
                let size = a.size();
                if kind.is_jelly() {
                    assert!((0.15..=0.350001).contains(&size[0]));
                    assert!((0.35..=0.950001).contains(&size[1]));
                }
            }
        }
        assert!(!CreatureKind::HammerheadShark.hostile_to_players());
        assert!(CreatureKind::ReefShark.hostile_to_players());
        assert!(CreatureKind::WhaleShark.hostile_to_players());
    }
    #[test]
    fn bee_ids_do_not_overlap_old_species_and_groups_have_four_members() {
        for x in -5..=5 {
            for z in -5..=5 {
                for slot in 0..4 {
                    let id = bee_id([x, z], slot).unwrap();
                    assert_ne!(id, creature_id([x, z], slot).unwrap());
                    assert_ne!(id & (1 << 63), 0);
                }
                assert!(bee_id([x, z], 4).is_none());
            }
        }
        assert_eq!(CreatureKind::Bee.size(), [0.095, 0.095, 0.115]);
        assert_eq!(CreatureKind::Bee.product(), None);
    }
    #[test]
    fn ids_are_injective_across_negative_cells_and_slots() {
        let mut ids = BTreeSet::new();
        for x in -4..4 {
            for z in -4..4 {
                for slot in 0..4 {
                    assert!(ids.insert(creature_id([x, z], slot).unwrap()));
                }
            }
        }
        assert!(creature_id([0, 0], 4).is_none());
        assert!(creature_id([31_251, 0], 0).is_none());
    }
    #[test]
    fn requested_species_vitals_and_fish_without_food_or_water_validate() {
        for (kind, health, food, water) in [
            (CreatureKind::Cow, 10.0, 30.0, 30.0),
            (CreatureKind::Sheep, 7.0, 35.0, 25.0),
            (CreatureKind::Pig, 8.0, 20.0, 40.0),
            (CreatureKind::Fish, 3.0, 0.0, 0.0),
        ] {
            assert_eq!(
                (kind.health(), kind.food(), kind.water()),
                (health, food, water)
            );
            let mut needs = CreatureNeeds::full(kind);
            assert!(needs.valid(kind));
            needs.food = food + 1.0;
            assert!(!needs.valid(kind));
            needs.food = f32::NAN;
            assert!(!needs.valid(kind));
        }
    }
}
