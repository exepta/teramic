use super::*;
use std::collections::{BTreeSet, HashSet};

fn terrain() -> TerraGenerator {
    TerraGenerator::new(terra_core::WorldConfig {
        ecology_revision: 6,
        structures_revision: 0,
        trees: false,
        caves: false,
        water: false,
        rivers: false,
        base_height: 48.0,
        hill_amplitude: 0.0,
        mountain_amplitude: 0.0,
        terrace_strength: 0.0,
        sea_level: -64.0,
        snow_height: 300.0,
        ..Default::default()
    })
    .unwrap()
}
fn habitat(terrain: &TerraGenerator, tile: Tile, biome: usize) -> Habitat<'_> {
    let mut c = terrain.surface_column(0.5, 0.5);
    c.height = 48.0;
    c.slope = 0.0;
    c.water_level = None;
    c.cave_cavity = None;
    c.river_cavity = None;
    c.oasis_moisture = 0.0;
    c.biome.temperature = 0.8;
    c.surface_material = match biome {
        3 => MaterialId::Sand,
        4 => MaterialId::Snow,
        8 => MaterialId::SpiritBlossomGrass,
        _ => MaterialId::Grass,
    };
    let weight = |kind| if biome == kind { 1.0 } else { 0.0 };
    c.biome.weights = super::super::biomes::BiomeWeights {
        ocean: 0.0,
        plains: weight(1),
        forest: weight(2),
        desert: weight(3),
        highland: weight(4),
        mushroom_park: 0.0,
        cliffted_desert: 0.0,
        cherry_forest: weight(7),
        spirit_blossom: weight(8),
        blood_forest: 0.0,
    };
    let mut columns = HashMap::new();
    for z in tile[1] * 32..tile[1] * 32 + 32 {
        for x in tile[0] * 32..tile[0] * 32 + 32 {
            columns.insert([x, z], c);
        }
    }
    Habitat {
        terrain,
        trees: Vec::new(),
        sites: Vec::new(),
        columns,
    }
}

fn connected(cells: &HashSet<Cell>, start: Cell) -> bool {
    let mut found = HashSet::from([start]);
    let mut pending = vec![start];
    while let Some(p) = pending.pop() {
        for d in [
            [1, 0, 0],
            [-1, 0, 0],
            [0, 1, 0],
            [0, -1, 0],
            [0, 0, 1],
            [0, 0, -1],
        ] {
            let next = std::array::from_fn(|i| p[i] + d[i]);
            if cells.contains(&next) && found.insert(next) {
                pending.push(next);
            }
        }
    }
    found.len() == cells.len()
}

#[test]
fn cacti_are_two_to_eight_blocks_and_arms_remain_face_connected_across_negative_sections() {
    for height in 2..=8 {
        for seed in 0..32 {
            let origin = [-1, 30, -1];
            let cells = cactus::shape(origin, height, seed);
            let set: HashSet<_> = cells.iter().copied().collect();
            assert!(connected(&set, origin));
            assert_eq!(
                set.iter().map(|p| p[1]).max().unwrap() - origin[1] + 1,
                i64::from(height)
            );
            assert!((0..i64::from(height)).all(|dy| set.contains(&[-1, 30 + dy, -1])));
            let patch = NaturalPatch {
                cells: cells.iter().map(|&p| (p, 422)).collect(),
                ..Default::default()
            };
            let owners: BTreeSet<_> = cells
                .iter()
                .map(|&p| {
                    let q = terra_voxel::global_to_section(p).0;
                    [q.x, q.y, q.z]
                })
                .collect();
            let mut reconstructed = HashSet::new();
            for owner in owners.into_iter().rev() {
                let start = owner.map(|v| v * 16);
                let mut blocks = vec![0; 4096];
                patch.stamp(start, 16, &mut blocks);
                for z in 0..16 {
                    for y in 0..16 {
                        for x in 0..16 {
                            let p = [
                                start[0] + x as i64,
                                start[1] + y as i64,
                                start[2] + z as i64,
                            ];
                            assert_eq!(
                                blocks[x + 16 * (y + 16 * z)],
                                patch.block_at(p).unwrap_or(0)
                            );
                            if blocks[x + 16 * (y + 16 * z)] == 422 {
                                reconstructed.insert(p);
                            }
                        }
                    }
                }
            }
            assert_eq!(set, reconstructed);
        }
    }
}

#[test]
fn desert_cactus_sites_are_bounded_spaced_and_reject_wrong_habitats() {
    let terrain = terrain();
    let mut plants = Vec::new();
    let mut heights = BTreeSet::new();
    for z in -4..4 {
        for x in -4..4 {
            let tile = [x, z];
            let mut context = habitat(&terrain, tile, 3);
            let mut cells = BTreeMap::new();
            let a = cactus::build(&mut context, tile, &mut cells);
            let mut duplicate = BTreeMap::new();
            assert_eq!(
                a,
                cactus::build(&mut habitat(&terrain, tile, 3), tile, &mut duplicate)
            );
            assert_eq!(cells, duplicate);
            assert!(
                cells
                    .keys()
                    .all(|p| p[0].div_euclid(32) == x && p[2].div_euclid(32) == z)
            );
            for p in &a {
                heights.insert(p.height);
                assert!(connected(
                    &cells
                        .keys()
                        .copied()
                        .filter(|cell| {
                            (0..3).all(|axis| {
                                cell[axis] >= p.bounds[0][axis] && cell[axis] <= p.bounds[1][axis]
                            })
                        })
                        .collect(),
                    p.origin
                ));
            }
            plants.extend(a);
            for mode in 0..5 {
                let mut bad = habitat(&terrain, tile, 3);
                for c in bad.columns.values_mut() {
                    match mode {
                        0 => c.surface_material = MaterialId::Grass,
                        1 => c.water_level = Some(50.0),
                        2 => c.slope = 1.4,
                        3 => c.biome.temperature = 0.1,
                        _ => c.cave_cavity = Some([45.0, 55.0]),
                    }
                }
                assert!(cactus::build(&mut bad, tile, &mut BTreeMap::new()).is_empty());
            }
        }
    }
    assert!(plants.len() > 20);
    assert_eq!(heights, (2..=8).collect());
    for (i, a) in plants.iter().enumerate() {
        for b in &plants[i + 1..] {
            assert!((a.origin[0] - b.origin[0]).pow(2) + (a.origin[2] - b.origin[2]).pow(2) >= 81);
        }
    }
}

#[test]
fn regional_flora_is_habitat_specific_grouped_and_leaves_walkable_clearings() {
    let terrain = terrain();
    for (biome, expected) in [(1, 423), (2, 425), (3, 428), (4, 427), (7, 426), (8, 316)] {
        let mut total = 0;
        let mut empty = 0;
        let mut species = HashSet::new();
        for z in -4..4 {
            for x in -4..4 {
                let tile = [x, z];
                let mut cells = BTreeMap::new();
                regional::build(&mut habitat(&terrain, tile, biome), tile, &mut cells);
                let mut other = BTreeMap::new();
                regional::build(&mut habitat(&terrain, tile, biome), tile, &mut other);
                assert_eq!(cells, other);
                empty += usize::from(cells.is_empty());
                total += cells.len();
                species.extend(cells.values().copied());
                assert!(cells.len() < 120, "dense clumps cannot cover a tile");
                assert!(
                    cells
                        .keys()
                        .all(|p| p[0].div_euclid(32) == x && p[2].div_euclid(32) == z)
                );
                let mut flooded = habitat(&terrain, tile, biome);
                for c in flooded.columns.values_mut() {
                    c.water_level = Some(51.0);
                }
                let mut rejected = BTreeMap::new();
                regional::build(&mut flooded, tile, &mut rejected);
                assert!(rejected.is_empty());
            }
        }
        assert!(
            species.contains(&expected),
            "biome {biome}, species {species:?}"
        );
        assert!(total > 40 && total < 64 * 100, "biome {biome}: {total}");
        assert!(empty > 0, "clearings in biome {biome}");
    }
}

#[test]
fn new_ecology_does_not_move_terrain_or_upgrade_saved_revision_five() {
    let current = terrain();
    let mut old = current.config().clone();
    old.ecology_revision = 5;
    let old = TerraGenerator::new(old).unwrap();
    for x in [-3201.5, -32.5, -1.5, 0.5, 16.5, 3201.5] {
        assert_eq!(current.surface_column(x, -x), old.surface_column(x, -x));
    }
    for revision in 0..=5 {
        let mut config = current.config().clone();
        config.ecology_revision = revision;
        let saved = serde_json::to_vec(&config).unwrap();
        let reload: terra_core::WorldConfig = serde_json::from_slice(&saved).unwrap();
        assert_eq!(reload.ecology_revision, revision);
        let generator = TerraGenerator::new(reload).unwrap();
        assert!(generator.surface_cacti([-1, -1]).is_empty());
        assert!(
            generator
                .surface_flora([-1, -1])
                .iter()
                .all(|(_, m)| *m < 422)
        );
    }
}

#[test]
fn cactus_voxels_share_section_direct_sample_and_authoritative_edit_paths() {
    use crate::{SectionGenerator, TerrainGenerator};
    let terrain = terrain();
    let origin = [-1, terrain.surface_height(-0.5, -0.5).ceil() as i64 + 1, -1];
    let cells = cactus::shape(origin, 8, 7);
    // Deliberately span zero and section seams. Inject only the immutable patch
    // cache; exercise the real production stamp, direct-query and edit paths.
    let mut patches: BTreeMap<Tile, BTreeMap<Cell, u16>> = BTreeMap::new();
    for &cell in &cells {
        patches
            .entry([cell[0].div_euclid(32), cell[2].div_euclid(32)])
            .or_default()
            .insert(cell, 422);
    }
    {
        let mut cache = terrain.natural_features.entries.lock().unwrap();
        for (tile, values) in patches {
            cache.0.insert(
                tile,
                Arc::new(NaturalPatch {
                    cells: values.into_iter().collect(),
                    ..Default::default()
                }),
            );
            cache.1.push_back(tile);
        }
    }
    let generator = SectionGenerator::new(terrain.clone());
    let owners: BTreeSet<_> = cells
        .iter()
        .map(|&cell| terra_voxel::global_to_section(cell).0)
        .collect();
    for owner in owners.into_iter().rev() {
        let visible = generator
            .build(owner, 0, &Default::default(), true)
            .unwrap();
        let headless = generator
            .build(owner, 0, &Default::default(), false)
            .unwrap();
        assert_eq!(visible.data, headless.data);
        for &cell in &cells {
            let (section, local) = terra_voxel::global_to_section(cell);
            if section != owner {
                continue;
            }
            assert_eq!(visible.data.block(local).unwrap(), 422);
            assert_eq!(generator.block_at(cell).unwrap(), 422);
            assert_eq!(terrain.sample(cell.map(|v| v as f64 + 0.5)).material, 422);
        }
    }
    let (section, local) = terra_voxel::global_to_section(origin);
    let edited = generator
        .build(section, 1, &HashMap::from([(origin, 0)]), false)
        .unwrap();
    assert_eq!(edited.data.block(local).unwrap(), 0);
}
