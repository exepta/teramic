//! Revisioned rooted wind-carved arches and leaning horn formations.
//! These are mineable terrain cells, never a rectangular prefab foundation.
use super::*;
#[path = "spacing.rs"]
mod spacing;

impl TerraGenerator {
    /// Cheap candidate probe for world/debug finders. This neither samples
    /// terrain nor builds a natural patch, and does not imply a valid habitat.
    pub fn fantasy_landmark_candidate(&self, tile: [i64; 2]) -> bool {
        if tile
            .iter()
            .any(|coordinate| coordinate.unsigned_abs() > 31_250_000)
        {
            return false;
        }
        let revision = self.config().ecology_revision;
        revision >= 9 && (revision < 11 || spacing::eligible(self.config().seed.0, tile))
    }
}

pub(super) fn build(
    habitat: &mut Habitat<'_>,
    tile: Tile,
    cells: &mut BTreeMap<Cell, u16>,
) -> Option<RockFormation> {
    let world_seed = habitat.terrain.config().seed.0;
    let rare = habitat.terrain.config().ecology_revision >= 11;
    // A region owns at most one monumental landmark. Reject other tiles before
    // any habitat probing, and never look up another patch to decide spacing.
    if rare && !habitat.terrain.fantasy_landmark_candidate(tile) {
        return None;
    }
    let seed = ticket(world_seed, tile, 1901);
    let monumental = habitat.terrain.config().ecology_revision >= 10;
    // The wider silhouettes need a centered footprint. Keep the previous seed
    // placement exactly intact when loading a saved revision nine world.
    let point = if monumental {
        [
            tile[0] * 32 + 14 + i64::from(seed % 4),
            tile[1] * 32 + 14 + i64::from((seed >> 5) % 4),
        ]
    } else {
        [
            tile[0] * 32 + 12 + i64::from(seed % 8),
            tile[1] * 32 + 12 + i64::from((seed >> 5) % 8),
        ]
    };
    let (ground, column) = habitat.ground(point)?;
    let w = column.biome.weights;
    let probability = 0.18
        + w.highland * 0.30
        + w.cliffted_desert * 0.30
        + w.spirit_blossom * 0.28
        + w.blood_forest * 0.21;
    if (!rare && f64::from(seed >> 16) / 65536.0 > probability)
        || w.ocean > 0.25
        || column.oasis_moisture > 0.35
        || habitat.reserved(point, 13)
        || habitat.root_distance_squared(point) < 100
    {
        return None;
    }
    let material = if w.spirit_blossom > 0.55 {
        MaterialId::SpiritStone
    } else if w.blood_forest > 0.55 {
        MaterialId::BloodStone
    } else if w.desert + w.cliffted_desert > 0.55 {
        MaterialId::Sandstone
    } else {
        MaterialId::Stone
    };
    let turn = (seed >> 9) % 4;
    let mut planned = BTreeMap::new();
    let silhouette = if monumental {
        super::monumental::shape(seed)
    } else {
        shape(seed)
    };
    for cell in silhouette {
        let [dx, dz] = match turn {
            0 => [cell[0], cell[2]],
            1 => [-cell[2], cell[0]],
            2 => [-cell[0], -cell[2]],
            _ => [cell[2], -cell[0]],
        };
        let x = point[0] + dx;
        let z = point[1] + dz;
        if x.div_euclid(32) != tile[0] || z.div_euclid(32) != tile[1] {
            return None;
        }
        let (local, c) = habitat.ground([x, z])?;
        if (local - ground).abs() > 4 || habitat.reserved([x, z], 3) {
            return None;
        }
        let y = ground + cell[1];
        if y < local {
            continue;
        }
        let p = [x, y, z];
        if habitat.tree_occupied(p)
            || cells.contains_key(&p)
            // A root deliberately replaces one solid surface cell. The full
            // signed terrain density is <=0 there, which is valid support;
            // testing it as if it must be air rejects every rooted formation.
            // Above that support, preserve existing solid overhangs instead.
            || y > local
                && habitat
                    .terrain
                    .cave_density_with_column(p.map(|v| v as f64 + 0.5), c)
                    .0
                    <= 0.0
        {
            return None;
        }
        planned.insert(p, material as u16);
    }
    // Shape feet extend only down to their local natural support. This adapts
    // both roots independently while preserving the open space under arches.
    let roots: Vec<_> = planned
        .keys()
        .filter(|p| p[1] <= ground + 1)
        .copied()
        .collect();
    if roots.is_empty() {
        return None;
    }
    for p in roots {
        let (local, _) = habitat.ground([p[0], p[2]])?;
        for y in local..p[1] {
            planned.insert([p[0], y, p[2]], material as u16);
        }
    }
    if planned.len() < 70 {
        return None;
    }
    let bounds = planned
        .keys()
        .fold([[i64::MAX; 3], [i64::MIN; 3]], |mut b, p| {
            for a in 0..3 {
                b[0][a] = b[0][a].min(p[a]);
                b[1][a] = b[1][a].max(p[a]);
            }
            b
        });
    // Moss follows actual exposed crests, not a flat horizontal cube cap.
    if material == MaterialId::Stone && column.biome.humidity > 0.45 {
        let tops: Vec<_> = planned
            .keys()
            .filter(|p| !planned.contains_key(&[p[0], p[1] + 1, p[2]]))
            .filter(|p| ticket(u64::from(seed), [p[0], p[2]], 1902) % 5 < 3)
            .copied()
            .collect();
        for p in tops {
            planned.insert(p, MaterialId::MossyStone as u16);
        }
    }
    let formation = RockFormation {
        center: [point[0], ground + 1, point[1]],
        bounds,
        material: material as u16,
        voxel_count: planned.len(),
    };
    cells.extend(planned);
    Some(formation)
}

/// Union of a sampled curved spine and small voxel radii; sampling is fine
/// enough that every segment overlaps, so no detached stones hang in the air.
fn shape(seed: u32) -> Vec<Cell> {
    let arch = seed & 1 == 0;
    let height = 11.0 + f64::from((seed >> 3) % 5);
    let samples = if arch { 96 } else { 64 };
    let mut cells = std::collections::BTreeSet::new();
    for i in 0..=samples {
        let t = i as f64 / samples as f64;
        let spine = if arch {
            [
                -6.0 + 12.0 * t,
                (std::f64::consts::PI * t).sin() * height,
                (t * std::f64::consts::PI * 2.0).sin() * 1.8,
            ]
        } else {
            [
                (t * 2.4).sin() * 3.1 * t,
                t * height,
                (t * 3.8).sin() * 1.7 * t,
            ]
        };
        let radius = if arch {
            1.35 + (t * 7.0 + f64::from(seed % 5)).sin() * 0.25
        } else {
            2.7 * (1.0 - t).powf(0.68) + 0.45
        };
        let base = spine.map(|p| p.floor() as i64);
        for z in -3..=3 {
            for y in -3..=3 {
                for x in -3..=3 {
                    let p = [base[0] + x, base[1] + y, base[2] + z];
                    let distance = (0..3)
                        .map(|a| (p[a] as f64 - spine[a]).powi(2))
                        .sum::<f64>();
                    if p[1] >= 0 && distance <= radius * radius {
                        cells.insert(p);
                    }
                }
            }
        }
    }
    // A shorter companion horn grows from the same foot, splitting instead
    // of making an orderly pair of identical pillars.
    if !arch {
        for i in 0..=48 {
            let t = i as f64 / 48.0;
            let spine = [-t * 4.2, t * height * 0.64, (t * 2.7).sin() * 2.0];
            let r = 1.6 * (1.0 - t) + 0.55;
            let base = spine.map(|p| p.floor() as i64);
            for z in -2..=2 {
                for y in -2..=2 {
                    for x in -2..=2 {
                        let p = [base[0] + x, base[1] + y, base[2] + z];
                        if p[1] >= 0
                            && (0..3)
                                .map(|a| (p[a] as f64 - spine[a]).powi(2))
                                .sum::<f64>()
                                <= r * r
                        {
                            cells.insert(p);
                        }
                    }
                }
            }
        }
    }
    cells.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn revision_eleven_keeps_real_landmarks_rare_mineable_and_preserves_ten_saves() {
        use crate::SectionGenerator;
        let config = terra_core::WorldConfig {
            ecology_revision: 11,
            landscape_revision: 1,
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
        };
        let terrain = TerraGenerator::new(config.clone()).unwrap();
        assert!(!terrain.fantasy_landmark_candidate([i64::MIN, 0]));
        assert!(!terrain.fantasy_landmark_candidate([0, i64::MAX]));
        let (tile, formation) = (-4..=4)
            .flat_map(|z| (-4..=4).map(move |x| [x, z]))
            .map(|region| spacing::candidate(config.seed.0, region))
            .find_map(|tile| {
                terrain
                    .surface_rock_formations(tile)
                    .into_iter()
                    .find(|rock| rock.bounds[1][1] - rock.bounds[0][1] >= 17)
                    .map(|rock| (tile, rock))
            })
            .expect("rare candidates still produce actual landmarks in suitable terrain");
        assert!(spacing::eligible(config.seed.0, tile));
        let patch = terrain.natural_patch(tile);
        let high = patch
            .cells
            .iter()
            .find(|(p, _)| p[1] == formation.bounds[1][1])
            .unwrap();
        let generator = SectionGenerator::new(terrain.clone());
        let (owner, local) = terra_voxel::global_to_section(high.0);
        assert_eq!(generator.block_at(high.0).unwrap(), high.1);
        let section = generator
            .build(owner, 0, &Default::default(), false)
            .unwrap();
        assert_eq!(section.data.block(local).unwrap(), high.1);
        let mined = generator
            .build(owner, 1, &HashMap::from([(high.0, 0)]), false)
            .unwrap();
        assert_eq!(mined.data.block(local).unwrap(), 0);
        let restored = TerraGenerator::new(config.clone()).unwrap();
        assert_eq!(*patch, *restored.natural_patch(tile));

        let legacy_config = terra_core::WorldConfig {
            ecology_revision: 10,
            ..config.clone()
        };
        let legacy = TerraGenerator::new(legacy_config.clone()).unwrap();
        let dense_tile = (-12..0)
            .flat_map(|z| (-12..0).map(move |x| [x, z]))
            .filter(|&tile| !spacing::eligible(config.seed.0, tile))
            .find(|&tile| {
                legacy
                    .surface_rock_formations(tile)
                    .iter()
                    .any(|rock| rock.bounds[1][1] - rock.bounds[0][1] >= 17)
            })
            .expect("the previous dense layout contains landmarks outside rare candidate sites");
        assert!(
            terrain
                .surface_rock_formations(dense_tile)
                .iter()
                .all(|rock| rock.bounds[1][1] - rock.bounds[0][1] < 17)
        );
        let encoded = serde_json::to_vec(&legacy_config).unwrap();
        let decoded: terra_core::WorldConfig = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(decoded.ecology_revision, 10);
        let loaded = TerraGenerator::new(decoded).unwrap();
        assert_eq!(
            *legacy.natural_patch(dense_tile),
            *loaded.natural_patch(dense_tile)
        );
    }

    #[test]
    fn revision_nine_silhouettes_keep_their_saved_voxel_contract() {
        for (seed, expected) in [
            (0, 11_923_868_506_228_359_008_u64),
            (1, 3_470_651_327_995_905_520),
            (19, 11_401_277_141_996_685_901),
        ] {
            let fingerprint = shape(seed)
                .into_iter()
                .flatten()
                .flat_map(i64::to_le_bytes)
                .fold(14_695_981_039_346_656_037_u64, |hash, byte| {
                    (hash ^ u64::from(byte)).wrapping_mul(1_099_511_628_211)
                });
            assert_eq!(fingerprint, expected, "legacy seed {seed}");
        }
    }

    #[test]
    fn revision_ten_monuments_are_real_mineable_voxels_and_nine_saves_keep_their_patch() {
        use crate::SectionGenerator;
        let terrain = TerraGenerator::new(terra_core::WorldConfig {
            ecology_revision: 10,
            landscape_revision: 1,
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
        .unwrap();
        let (tile, formation) = (-12..0)
            .flat_map(|z| (-12..0).map(move |x| [x, z]))
            .find_map(|tile| {
                terrain
                    .surface_rock_formations(tile)
                    .into_iter()
                    .find(|rock| rock.bounds[1][1] - rock.bounds[0][1] >= 17)
                    .map(|rock| (tile, rock))
            })
            .expect("monumental revision ten rocks must really generate");
        let patch = terrain.natural_patch(tile);
        let restored = TerraGenerator::new(terrain.config().clone()).unwrap();
        assert_eq!(*patch, *restored.natural_patch(tile));
        assert!(formation.voxel_count < 6000);
        assert!(formation.bounds[0][0].div_euclid(32) == tile[0]);
        assert!(formation.bounds[1][2].div_euclid(32) == tile[1]);
        let high_cell = patch
            .cells
            .iter()
            .find(|(cell, _)| cell[1] == formation.bounds[1][1])
            .unwrap();
        let generator = SectionGenerator::new(terrain.clone());
        let (owner, local) = terra_voxel::global_to_section(high_cell.0);
        let section = generator
            .build(owner, 0, &Default::default(), false)
            .unwrap();
        assert_eq!(section.data.block(local).unwrap(), high_cell.1);
        assert_eq!(generator.block_at(high_cell.0).unwrap(), high_cell.1);
        let mined = generator
            .build(owner, 1, &HashMap::from([(high_cell.0, 0)]), false)
            .unwrap();
        assert_eq!(mined.data.block(local).unwrap(), 0);

        let mut saved = terrain.config().clone();
        saved.ecology_revision = 9;
        let old = TerraGenerator::new(saved.clone()).unwrap();
        let old_patch = old.natural_patch(tile);
        let encoded = serde_json::to_vec(&saved).unwrap();
        let decoded: terra_core::WorldConfig = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(decoded.ecology_revision, 9);
        let reloaded = TerraGenerator::new(decoded).unwrap();
        assert_eq!(*old_patch, *reloaded.natural_patch(tile));
        assert_ne!(*patch, *old_patch);
        assert!(
            old.surface_rock_formations(tile)
                .iter()
                .all(|rock| { rock.bounds[1][1] - rock.bounds[0][1] < 17 })
        );
    }

    #[test]
    fn fantasy_landmarks_have_connected_roots_open_arches_and_asymmetric_overhangs() {
        for seed in 0..24 {
            let cells: std::collections::BTreeSet<_> = shape(seed).into_iter().collect();
            let root = *cells.first().unwrap();
            let mut seen = std::collections::BTreeSet::from([root]);
            let mut queue = VecDeque::from([root]);
            while let Some(p) = queue.pop_front() {
                for a in 0..3 {
                    for d in [-1, 1] {
                        let mut q = p;
                        q[a] += d;
                        if cells.contains(&q) && seen.insert(q) {
                            queue.push_back(q);
                        }
                    }
                }
            }
            assert_eq!(seen.len(), cells.len(), "disconnected seed{seed}");
            assert!(cells.iter().any(|p| p[1] == 0));
            assert!(cells.iter().map(|p| p[1]).max().unwrap() >= 10);
            assert!(cells.len() < 1700);
            if seed & 1 == 0 {
                assert!(!cells.contains(&[0, 2, 0]), "the arch must remain open");
                assert!(cells.iter().any(|p| p[0].abs() < 2 && p[1] > 8));
            } else {
                assert!(cells.iter().any(|p| p[0] >= 2 && p[1] >= 7));
                assert!(cells.iter().any(|p| p[0] <= -2 && p[1] >= 5));
            }
        }
    }

    #[test]
    fn revision_nine_formations_match_real_sections_survive_edits_and_keep_old_worlds() {
        use crate::SectionGenerator;
        let terrain = TerraGenerator::new(terra_core::WorldConfig {
            ecology_revision: 9,
            landscape_revision: 1,
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
        .unwrap();
        let (tile, formation) = (-12..0)
            .flat_map(|z| (-12..0).map(move |x| [x, z]))
            .find_map(|tile| {
                terrain
                    .surface_rock_formations(tile)
                    .into_iter()
                    .find(|r| r.bounds[1][1] - r.bounds[0][1] >= 9)
                    .map(|r| (tile, r))
            })
            .expect("fantasy formations must really generate");
        let again = TerraGenerator::new(terrain.config().clone()).unwrap();
        assert_eq!(*terrain.natural_patch(tile), *again.natural_patch(tile));
        let generator = SectionGenerator::new(terrain.clone());
        let patch = terrain.natural_patch(tile);
        assert!(
            patch.cells.iter().any(|(p, _)| {
                p[1] == (terrain
                    .surface_column(p[0] as f64 + 0.5, p[2] as f64 + 0.5)
                    .height
                    - 0.5)
                    .floor() as i64
            }),
            "the real formation must embed a root in its solid natural support"
        );
        let positions: std::collections::BTreeSet<_> = patch
            .cells
            .iter()
            .map(|(p, _)| terra_voxel::global_to_section(*p).0)
            .collect();
        for owner in positions {
            let built = generator
                .build(owner, 0, &Default::default(), false)
                .unwrap();
            for &(cell, material) in &patch.cells {
                let (section, local) = terra_voxel::global_to_section(cell);
                if section == owner {
                    assert_eq!(built.data.block(local).unwrap(), material);
                }
                assert_eq!(generator.block_at(cell).unwrap(), material);
            }
        }
        let mined = patch
            .cells
            .iter()
            .find(|(p, _)| p[1] > formation.bounds[0][1] + 5)
            .unwrap()
            .0;
        let (owner, local) = terra_voxel::global_to_section(mined);
        let built = generator
            .build(owner, 1, &HashMap::from([(mined, 0)]), false)
            .unwrap();
        assert_eq!(built.data.block(local).unwrap(), 0);
        for revision in 5..=8 {
            let mut saved = terrain.config().clone();
            saved.ecology_revision = revision;
            let encoded = serde_json::to_vec(&saved).unwrap();
            let decoded: terra_core::WorldConfig = serde_json::from_slice(&encoded).unwrap();
            assert_eq!(decoded.ecology_revision, revision);
            let old = TerraGenerator::new(decoded).unwrap();
            assert!(
                old.surface_rock_formations(tile)
                    .iter()
                    .all(|r| r.bounds[1][1] - r.bounds[0][1] < 9)
            );
            assert_eq!(
                old.surface_height(-0.5, -0.5),
                terrain.surface_height(-0.5, -0.5)
            );
        }
    }
}
