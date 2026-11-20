//! Real generated sections, the public authoritative spawn path and edited-voxel
//! collision; no mocked species selector or synthetic grass floor.
use super::*;
use crate::{
    SectionGenerator, TerraGenerator,
    creatures::{spawn_cell, step_budgeted},
    edits::EditJournal,
    streaming::{ChunkStore, LoadedSection},
};
use terra_core::{BiomeKind, Config, WorldConfig, creatures::CreaturePopulation};
use terra_voxel::{MaterialId, SectionPos};

fn spirit_core(terrain: &TerraGenerator) -> [i64; 2] {
    for z in (-8192..=8192).step_by(256) {
        for x in (-8192..=-256).step_by(256) {
            if [[0.0, 0.0], [-48.0, -48.0], [48.0, 48.0]]
                .into_iter()
                .any(|[dx, dz]| {
                    terrain
                        .biome_at(x as f64 + dx, z as f64 + dz)
                        .weights
                        .spirit_blossom
                        < 0.8
                })
            {
                continue;
            }
            let surface = terrain.surface_column(x as f64 + 0.5, z as f64 + 0.5);
            if surface.surface_material == MaterialId::SpiritBlossomGrass
                && surface.height > terrain.config().sea_level + 4.0
            {
                return [x / 32, z / 32];
            }
        }
    }
    panic!("seed 42042 must provide a natural Spirit Blossom interior");
}

fn loaded_habitat(generator: &SectionGenerator, home: [i64; 2]) -> ChunkStore {
    let mut chunks = ChunkStore::default();
    for dz in 0..2 {
        for dx in 0..2 {
            let column = [home[0] * 2 + dx, home[1] * 2 + dz];
            let prepared = generator.prepare_column(column, false).unwrap();
            let mut low = i64::MAX;
            let mut high = i64::MIN;
            for z in 0..16 {
                for x in 0..16 {
                    let height = prepared.local_surface(x, z).unwrap().height.floor() as i64;
                    low = low.min((height - 3).div_euclid(16));
                    high = high.max((height + 6).div_euclid(16));
                }
            }
            for y in low..=high {
                let position = SectionPos::new(column[0], y, column[1]);
                let generated = generator
                    .build_prepared(position, 0, &Default::default(), &prepared)
                    .unwrap();
                chunks.loaded.insert(
                    position,
                    LoadedSection {
                        data: generated.data,
                        mesh: None,
                        mesh_epoch: 0,
                        water_epoch: 0,
                        water: generated.water,
                        props: generated.props,
                    },
                );
            }
        }
    }
    chunks
}

#[test]
fn all_spirit_fauna_spawn_on_real_generated_blossom_habitat_and_foxes_keep_walking() {
    let terrain = TerraGenerator::new(
        WorldConfig::from_toml(include_str!("../../../../configs/world.toml")).unwrap(),
    )
    .unwrap();
    let center = spirit_core(&terrain);
    let generator = SectionGenerator::new(terrain.clone());
    let journal = EditJournal::default();
    let mut observed = std::collections::BTreeSet::new();
    let mut checked_fox = false;
    let mut offsets: Vec<_> = (-6_i64..=6)
        .flat_map(|z| (-6_i64..=6).map(move |x| [x, z]))
        .collect();
    offsets.sort_unstable_by_key(|[x, z]| x * x + z * z);
    for [dx, dz] in offsets {
        let home = [center[0] + dx, center[1] + dz];
        let seed = super::super::random(
            terra_core::creatures::creature_id(home, 0).unwrap(),
            terrain.config().seed.0,
        );
        let expected = surface_species("spirit_blossom", seed).unwrap();
        if observed.contains(&expected) || (seed >> 32).is_multiple_of(3) {
            continue;
        }
        let surface =
            terrain.surface_column((home[0] * 32 + 16) as f64, (home[1] * 32 + 16) as f64);
        if terrain.surface_biome(&surface) != BiomeKind::SpiritBlossom {
            continue;
        }
        let chunks = loaded_habitat(&generator, home);
        assert!(chunks.loaded.values().any(|s| {
            s.data
                .blocks()
                .contains(&(MaterialId::SpiritBlossomGrass as u16))
        }));
        let habitat = Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        };
        let mut population = CreaturePopulation::default();
        assert!(
            spawn_cell(&mut population, &habitat, home),
            "loaded habitat must complete: {home:?}"
        );
        for a in population.animals.values() {
            if !matches!(
                a.kind,
                Kind::Butterfly | Kind::SpiritGloom | Kind::SpiritFox
            ) {
                continue;
            }
            let column = terrain.surface_column(a.position[0], a.position[2]);
            assert_eq!(terrain.surface_biome(&column), BiomeKind::SpiritBlossom);
            assert!(a.valid());
            assert!(habitat.clear_rotated(a.position, a.size(), false, f64::from(a.yaw)));
            if a.kind == Kind::SpiritFox {
                let support = [
                    a.position[0].floor() as i64,
                    a.position[1] as i64 - 1,
                    a.position[2].floor() as i64,
                ];
                assert_eq!(
                    habitat.block(support),
                    Some(MaterialId::SpiritBlossomGrass as u16)
                );
                let mut fox = a.clone();
                let mut traveled = 0.0;
                for _ in 0..600 {
                    let old = fox.position;
                    step_budgeted(&mut fox, &habitat, &[], 0.1, true);
                    traveled += distance(old, fox.position).sqrt();
                    assert!(fox.valid());
                }
                assert_eq!(
                    fox.health, a.health,
                    "ambient spirits must not starve with no food/water requirements"
                );
                if traveled <= 1.0 {
                    continue;
                }
                checked_fox = true;
            }
            observed.insert(a.kind);
        }
        let saved = population.clone();
        assert!(!spawn_cell(&mut population, &habitat, home));
        assert_eq!(
            population, saved,
            "revisiting a habitat must not reroll its fauna"
        );
        if observed.len() == 3 && checked_fox {
            break;
        }
    }
    assert_eq!(
        observed,
        [Kind::Butterfly, Kind::SpiritGloom, Kind::SpiritFox].into(),
        "missing fauna in actual habitat {center:?}"
    );
    assert!(
        checked_fox,
        "a naturally spawned Spirit Fox must visibly walk for a minute without starving"
    );
}
