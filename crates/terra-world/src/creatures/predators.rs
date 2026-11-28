use super::{
    Behavior, Creature, Habitat, Kind, biome_slug, creature_id, random, spawn_volume_loaded,
};
use terra_core::creatures::{CreatureNeeds, definitions::MobDefinition};

fn species(biome: &str, seed: u64) -> Option<(Kind, u8)> {
    let pick = (seed >> 19) % 9;
    let snakes: &[u8] = match biome {
        "savanna" if pick < 3 => return Some((Kind::Lion, 0)),
        "savanna" if pick < 5 => return Some((Kind::Tiger, 0)),
        "swamp" if pick < 3 => return Some((Kind::Crocodile, 0)),
        "swamp" if pick < 5 => return Some((Kind::Jaguar, 0)),
        "swamp" => &[0, 1, 3, 4, 5, 7],
        "savanna" => &[2, 4, 6, 7, 8],
        "forest" => &[0, 4, 6],
        "petrified_forest" => &[0, 5, 6],
        _ => return None,
    };
    Some((
        Kind::Snake,
        snakes[((seed >> 35) % snakes.len() as u64) as usize],
    ))
}

pub(super) fn natural<'a>(
    h: &Habitat,
    home: [i64; 2],
    available: usize,
    definition: impl Fn(Kind) -> &'a MobDefinition,
) -> Result<Option<Vec<Creature>>, ()> {
    let key = creature_id(home, 0).ok_or(())?;
    let seed = random(key, h.terrain.config().seed.0 ^ 0x5eed_78a9_1128);
    let center = home.map(|v| (v * 32 + 16) as f64 + 0.5);
    if !seed.is_multiple_of(8) {
        return Ok(None);
    }
    let surface = h.terrain.surface_column(center[0], center[1]);
    let biome = biome_slug(h.terrain.surface_biome(&surface));
    let Some((kind, variant)) = species(biome, seed) else {
        return Ok(None);
    };
    let settings = definition(kind);
    let count = usize::from(settings.spawn_group_size(seed >> 8, biome, 1)).min(available);
    let mut animals = Vec::with_capacity(count);
    // Bounded probes remain inside the active habitat. The mandatory water
    // and body tests run against loaded voxels, including player-made banks.
    for [dx, dz] in [
        [0, 0],
        [4, 0],
        [0, 4],
        [-4, 0],
        [0, -4],
        [8, 4],
        [-8, 4],
        [4, -8],
        [-4, -8],
    ] {
        if animals.len() == count {
            break;
        }
        let x = center[0] + f64::from(dx);
        let z = center[1] + f64::from(dz);
        let column = h.terrain.surface_column(x, z);
        if biome_slug(h.terrain.surface_biome(&column)) != biome {
            continue;
        }
        let size = settings.size;
        let yaw = ((seed >> 22) % 628) as f32 * 0.01;
        let (position, behavior) = if kind == Kind::Crocodile {
            let Some(water) = h
                .terrain
                .water_column(x, z)
                .filter(|water| water.depth >= 0.65)
            else {
                continue;
            };
            let p = [x, water.level - size[1] * 0.68, z];
            if !spawn_volume_loaded(h, p, [size[2]; 3]) {
                return Err(());
            }
            if !h.wet([x, water.level - 0.23, z])
                || !h.clear_swimmer_rotated(p, size, yaw.into())
                || ![(4., 0.), (-4., 0.), (0., 4.), (0., -4.)]
                    .into_iter()
                    .any(|(dx, dz)| {
                        h.ground_sized([x + dx, water.level, z + dz], size, yaw.into())
                            .is_some()
                    })
            {
                continue;
            }
            (p, Behavior::Swim)
        } else {
            let y = column.height.floor() + 1.0;
            let width = size[0].max(size[2]) + 1.0;
            if !spawn_volume_loaded(h, [x, y - 2.0, z], [width, size[1] + 3.0, width]) {
                return Err(());
            }
            let Some(p) = h.ground_sized([x, y, z], size, yaw.into()) else {
                continue;
            };
            (p, Behavior::Rest)
        };
        animals.push(Creature {
            id: creature_id(home, animals.len() as u8).expect("validated habitat slot"),
            kind,
            home,
            position,
            yaw,
            variant,
            behavior,
            health: settings.health,
            velocity: [0.0; 3],
            age: 0.0,
            timer: 1.6,
            needs: CreatureNeeds::full(kind),
        });
    }
    Ok((!animals.is_empty()).then_some(animals))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn all_nine_snakes_are_reachable_and_only_hot_biomes_spawn_venomous_kinds() {
        let mut variants = BTreeSet::new();
        let mut cats = BTreeSet::new();
        for biome in ["forest", "petrified_forest", "savanna", "swamp"] {
            for v in 0..1024 {
                let seed = random(v, 1128);
                let (kind, variant) = species(biome, seed).unwrap();
                assert!(!kind.venomous(variant) || matches!(biome, "savanna" | "swamp"));
                match kind {
                    Kind::Snake => {
                        variants.insert(variant);
                    }
                    Kind::Tiger | Kind::Lion => {
                        assert_eq!(biome, "savanna");
                        cats.insert(kind);
                    }
                    Kind::Crocodile | Kind::Jaguar => {
                        assert_eq!(biome, "swamp");
                        cats.insert(kind);
                    }
                    _ => unreachable!(),
                }
            }
        }
        assert_eq!(variants, (0..9).collect());
        assert_eq!(cats.len(), 4);
        assert!(species("highland", 0).is_none());
    }
    #[test]
    fn natural_crocodiles_spawn_in_generated_swamp_water_beside_walkable_banks() {
        use crate::{
            SectionGenerator, TerraGenerator,
            creatures::{spawn_cell_away_from_players, test_habitat::load_surface},
            edits::EditJournal,
        };
        use terra_core::creatures::CreaturePopulation;
        let terrain = TerraGenerator::new(Default::default()).unwrap();
        let generator = SectionGenerator::new(terrain.clone());
        let journal = EditJournal::default();
        for home in [[164, -229], [-4, -359]] {
            let chunks = load_surface(&generator, home);
            let habitat = Habitat {
                terrain: &terrain,
                chunks: &chunks,
                journal: &journal,
            };
            let mut pop = CreaturePopulation::default();
            let player = [(home[0] * 32 + 96) as f64, 12., (home[1] * 32 + 16) as f64];
            assert!(spawn_cell_away_from_players(
                &mut pop,
                &habitat,
                home,
                &[player],
                1
            ));
            assert_eq!(pop.animals.len(), 1);
            let animal = &pop.animals[&creature_id(home, 0).unwrap()];
            assert!(animal.kind == Kind::Crocodile && animal.valid());
            assert_eq!(animal.behavior, Behavior::Swim);
            let [x, _, z] = animal.position;
            let surface = terrain.surface_column(x, z);
            assert_eq!(biome_slug(terrain.surface_biome(&surface)), "swamp");
            let water = terrain.water_column(x, z).unwrap();
            assert!(water.depth >= 0.65 && habitat.wet(animal.position));
            assert!(habitat.clear_swimmer_rotated(
                animal.position,
                animal.size(),
                animal.yaw.into()
            ));
            let dry_bank = [(4., 0.), (-4., 0.), (0., 4.), (0., -4.)]
                .into_iter()
                .filter_map(|(dx, dz)| {
                    habitat.ground_rotated(
                        [x + dx, water.level, z + dz],
                        animal.kind,
                        animal.yaw.into(),
                    )
                })
                .find(|p| !habitat.wet([p[0], p[1] + 0.04, p[2]]))
                .expect("nearby dry supported bank");
            assert!((dry_bank[1] - water.level).abs() <= 1.01);
            assert!(pop.explored.contains(&home));
        }
    }
}
