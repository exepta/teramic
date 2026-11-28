//! Command vocabulary and persistent identities, separate from natural habitat slots.
use super::{CreatureKind, CreaturePopulation, MAX_CREATURES};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpawnMob {
    pub kind: CreatureKind,
    pub variant: u8,
}

pub const MOBS: &[(&str, SpawnMob)] = &[
    (
        "teramic::swamp_witch",
        mob(CreatureKind::Villager, super::villagers::SWAMP_WITCH),
    ),
    (
        "teramic::swamp_warlock",
        mob(CreatureKind::Villager, super::villagers::SWAMP_WARLOCK),
    ),
    (
        "teramic::swamp_hermit",
        mob(CreatureKind::Villager, super::villagers::SWAMP_HERMIT),
    ),
    ("teramic::lava_worm", mob(CreatureKind::LavaWorm, 0)),
    ("teramic::sheep", mob(CreatureKind::Sheep, 0)),
    ("teramic::cow", mob(CreatureKind::Cow, 0)),
    ("teramic::pig", mob(CreatureKind::Pig, 0)),
    ("teramic::hen", mob(CreatureKind::Hen, 0)),
    ("teramic::partridge_hen", mob(CreatureKind::Hen, 1)),
    ("teramic::buff_hen", mob(CreatureKind::Hen, 2)),
    ("teramic::barred_hen", mob(CreatureKind::Hen, 3)),
    ("teramic::gold_laced_hen", mob(CreatureKind::Hen, 4)),
    ("teramic::blue_laced_hen", mob(CreatureKind::Hen, 5)),
    ("teramic::rooster", mob(CreatureKind::Rooster, 0)),
    (
        "teramic::silver_duckwing_rooster",
        mob(CreatureKind::Rooster, 1),
    ),
    ("teramic::barred_rooster", mob(CreatureKind::Rooster, 2)),
    ("teramic::blue_red_rooster", mob(CreatureKind::Rooster, 3)),
    ("teramic::tiger", mob(CreatureKind::Tiger, 0)),
    ("teramic::lion", mob(CreatureKind::Lion, 0)),
    ("teramic::crocodile", mob(CreatureKind::Crocodile, 0)),
    ("teramic::jaguar", mob(CreatureKind::Jaguar, 0)),
    ("teramic::grass_snake", mob(CreatureKind::Snake, 0)),
    ("teramic::adder", mob(CreatureKind::Snake, 1)),
    ("teramic::sand_viper", mob(CreatureKind::Snake, 2)),
    ("teramic::coral_snake", mob(CreatureKind::Snake, 3)),
    ("teramic::emerald_tree_boa", mob(CreatureKind::Snake, 4)),
    ("teramic::reticulated_python", mob(CreatureKind::Snake, 5)),
    ("teramic::kingsnake", mob(CreatureKind::Snake, 6)),
    ("teramic::copperhead", mob(CreatureKind::Snake, 7)),
    ("teramic::black_mamba", mob(CreatureKind::Snake, 8)),
    ("teramic::fish", mob(CreatureKind::Fish, 2)),
    ("teramic::clown_fish", mob(CreatureKind::Fish, 0)),
    ("teramic::bee", mob(CreatureKind::Bee, 0)),
    ("teramic::bear", mob(CreatureKind::Bear, 0)),
    ("teramic::squid", mob(CreatureKind::Squid, 0)),
    ("teramic::moon_jelly", mob(CreatureKind::MoonJelly, 0)),
    ("teramic::fire_jelly", mob(CreatureKind::FireJelly, 0)),
    ("teramic::void_jelly", mob(CreatureKind::VoidJelly, 0)),
    ("teramic::sun_jelly", mob(CreatureKind::SunJelly, 0)),
    ("teramic::crystal_jelly", mob(CreatureKind::CrystalJelly, 0)),
    ("teramic::reef_shark", mob(CreatureKind::ReefShark, 0)),
    (
        "teramic::hammerhead_shark",
        mob(CreatureKind::HammerheadShark, 0),
    ),
    ("teramic::whale_shark", mob(CreatureKind::WhaleShark, 0)),
    ("teramic::horse", mob(CreatureKind::Horse, 0)),
    ("teramic::small_spider", mob(CreatureKind::SmallSpider, 0)),
    ("teramic::medium_spider", mob(CreatureKind::MediumSpider, 0)),
    ("teramic::large_spider", mob(CreatureKind::LargeSpider, 0)),
    ("teramic::stalker", mob(CreatureKind::Stalker, 0)),
    (
        "teramic::blossom_butterfly",
        mob(CreatureKind::Butterfly, 0),
    ),
    ("teramic::petal_butterfly", mob(CreatureKind::Butterfly, 1)),
    (
        "teramic::wisteria_butterfly",
        mob(CreatureKind::Butterfly, 2),
    ),
    ("teramic::spirit_butterfly", mob(CreatureKind::Butterfly, 3)),
    (
        "teramic::goldenbloom_butterfly",
        mob(CreatureKind::Butterfly, 4),
    ),
    (
        "teramic::moonpetal_butterfly",
        mob(CreatureKind::Butterfly, 5),
    ),
    (
        "teramic::sakurawing_butterfly",
        mob(CreatureKind::Butterfly, 6),
    ),
    (
        "teramic::spirit_gloom_blossom",
        mob(CreatureKind::SpiritGloom, 0),
    ),
    (
        "teramic::spirit_gloom_moon",
        mob(CreatureKind::SpiritGloom, 1),
    ),
    ("teramic::gloom", mob(CreatureKind::SpiritGloom, 0)),
    ("teramic::standard_gloom", mob(CreatureKind::SpiritGloom, 0)),
    ("teramic::shadow_gloom", mob(CreatureKind::SpiritGloom, 1)),
    ("teramic::blood_gloom", mob(CreatureKind::SpiritGloom, 2)),
    ("teramic::light_gloom", mob(CreatureKind::SpiritGloom, 3)),
    ("teramic::forest_gloom", mob(CreatureKind::SpiritGloom, 4)),
    ("teramic::ice_gloom", mob(CreatureKind::SpiritGloom, 5)),
    ("teramic::sakura_gloom", mob(CreatureKind::SpiritGloom, 6)),
    ("teramic::spirit_fox", mob(CreatureKind::SpiritFox, 0)),
    ("teramic::skeleton", mob(CreatureKind::Skeleton, 0)),
    ("teramic::skeleton_fighter", mob(CreatureKind::Skeleton, 1)),
    ("teramic::skeleton_guardian", mob(CreatureKind::Skeleton, 2)),
    ("teramic::raven", mob(CreatureKind::Raven, 0)),
    ("teramic::blood_slime", mob(CreatureKind::BloodSlime, 0)),
    ("teramic::blood_stalker", mob(CreatureKind::BloodStalker, 0)),
    ("teramic::blood_wisp", mob(CreatureKind::BloodWisp, 0)),
    ("teramic::stone_golem", mob(CreatureKind::StoneGolem, 0)),
    (
        "teramic::weathered_stone_golem",
        mob(CreatureKind::StoneGolem, 1),
    ),
    ("teramic::thorn_brute", mob(CreatureKind::ThornBrute, 0)),
    ("teramic::dark_dragon", mob(CreatureKind::DarkDragon, 0)),
    ("teramic::fire_dragon", mob(CreatureKind::FireDragon, 0)),
    ("teramic::cinder_dragon", mob(CreatureKind::FireDragon, 1)),
    (
        "teramic::obsidian_fire_dragon",
        mob(CreatureKind::FireDragon, 2),
    ),
    ("teramic::sulfur_dragon", mob(CreatureKind::FireDragon, 3)),
    ("teramic::earth_dragon", mob(CreatureKind::EarthDragon, 0)),
    ("teramic::water_dragon", mob(CreatureKind::WaterDragon, 0)),
    ("teramic::ruin_dragon", mob(CreatureKind::DarkDragon, 3)),
    ("teramic::shadow_wyrm", mob(CreatureKind::DarkDragon, 4)),
    ("teramic::void_dragon", mob(CreatureKind::DarkDragon, 5)),
    (
        "teramic::tidecrest_dragon",
        mob(CreatureKind::WaterDragon, 3),
    ),
    (
        "teramic::abyssal_tide_dragon",
        mob(CreatureKind::WaterDragon, 4),
    ),
    (
        "teramic::icewater_dragon",
        mob(CreatureKind::WaterDragon, 5),
    ),
    (
        "teramic::mountain_earth_dragon",
        mob(CreatureKind::EarthDragon, 1),
    ),
    ("teramic::granite_dragon", mob(CreatureKind::EarthDragon, 2)),
    ("teramic::magma_dragon", mob(CreatureKind::EarthDragon, 3)),
    ("teramic::elephant", mob(CreatureKind::Elephant, 0)),
    ("teramic::zebra", mob(CreatureKind::Zebra, 0)),
    ("teramic::sand_worm", mob(CreatureKind::SandWorm, 0)),
    ("teramic::villager", mob(CreatureKind::Villager, 0)),
    ("teramic::farmer", mob(CreatureKind::Villager, 1)),
    ("teramic::bishop", mob(CreatureKind::Villager, 5)),
    ("teramic::blacksmith", mob(CreatureKind::Villager, 8)),
    ("teramic::lumberjack", mob(CreatureKind::Villager, 9)),
    ("teramic::knight", mob(CreatureKind::Villager, 10)),
    ("teramic::spearman", mob(CreatureKind::Villager, 11)),
    ("teramic::archer", mob(CreatureKind::Villager, 12)),
    (
        "teramic::train_driver",
        mob(CreatureKind::Villager, super::villagers::ENGINEER),
    ),
    (
        "teramic::miner",
        mob(CreatureKind::Villager, super::villagers::MINER),
    ),
    (
        "teramic::unemployed",
        mob(CreatureKind::Villager, super::villagers::UNEMPLOYED),
    ),
    (
        "teramic::butcher",
        mob(CreatureKind::Villager, super::villagers::BUTCHER),
    ),
    (
        "teramic::seller",
        mob(CreatureKind::Villager, super::villagers::SELLER),
    ),
    (
        "teramic::travelling_merchant",
        mob(
            CreatureKind::Villager,
            super::villagers::TRAVELLING_MERCHANT,
        ),
    ),
    (
        "teramic::nomad_a",
        mob(CreatureKind::Villager, super::villagers::NOMAD_A),
    ),
    (
        "teramic::nomad_b",
        mob(CreatureKind::Villager, super::villagers::NOMAD_B),
    ),
    (
        "teramic::nomad_c",
        mob(CreatureKind::Villager, super::villagers::NOMAD_C),
    ),
    (
        "teramic::nomad_d",
        mob(CreatureKind::Villager, super::villagers::NOMAD_D),
    ),
];

const fn mob(kind: CreatureKind, variant: u8) -> SpawnMob {
    SpawnMob { kind, variant }
}

pub fn lookup(id: &str) -> Option<SpawnMob> {
    crate::command_ids::resolve(id, MOBS.iter().copied()).ok()
}

impl SpawnMob {
    pub fn id(self) -> &'static str {
        MOBS.iter()
            .find(|(_, mob)| *mob == self)
            .map_or("unknown", |(name, _)| name)
    }
}

const COMMAND_NAMESPACE: u64 = 1 << 62;

pub fn is_command_id(id: u64) -> bool {
    (COMMAND_NAMESPACE + 1..=COMMAND_NAMESPACE + u64::from(u32::MAX)).contains(&id)
}

pub fn next_command_id(population: &CreaturePopulation) -> Option<u64> {
    if population.animals.len() >= MAX_CREATURES {
        return None;
    }
    let last = population
        .animals
        .range(COMMAND_NAMESPACE + 1..=COMMAND_NAMESPACE + u64::from(u32::MAX))
        .next_back()
        .map_or(COMMAND_NAMESPACE, |(id, _)| *id);
    let next = last.checked_add(1)?;
    is_command_id(next).then_some(next)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::creatures::{AnimalProduct, Creature, CreatureBehavior, CreatureNeeds};

    #[test]
    fn command_identities_survive_serialization_without_claiming_natural_habitats() {
        let mut population = CreaturePopulation::default();
        for (_, mob) in MOBS {
            let id = next_command_id(&population).unwrap();
            assert_ne!(Some(id), super::super::creature_id([0, 0], 0));
            assert_ne!(Some(id), super::super::bee_id([0, 0], 0));
            let animal = Creature {
                id,
                kind: mob.kind,
                home: [0, 0],
                position: [0.0, 3.0, 0.0],
                velocity: [0.0; 3],
                yaw: 0.0,
                health: mob.kind.health(),
                behavior: CreatureBehavior::Rest,
                timer: 0.0,
                age: 0.0,
                variant: mob.variant,
                needs: CreatureNeeds::full(mob.kind),
            };
            if mob.id() == "teramic::clown_fish" {
                assert_eq!(animal.product(), Some(AnimalProduct::Clownfish));
            }
            assert!(animal.valid());
            population.animals.insert(id, animal);
        }
        assert!(population.valid());
        assert!(population.explored.is_empty());
        let restored: CreaturePopulation =
            serde_json::from_str(&serde_json::to_string(&population).unwrap()).unwrap();
        assert_eq!(restored, population);
        assert_eq!(next_command_id(&restored), next_command_id(&population));
        assert!(
            !restored
                .animals
                .contains_key(&next_command_id(&restored).unwrap())
        );
        assert!(!is_command_id(COMMAND_NAMESPACE));
        assert!(!is_command_id(COMMAND_NAMESPACE + u64::from(u32::MAX) + 1));
        assert!(lookup("other::cow").is_none());
    }
}
