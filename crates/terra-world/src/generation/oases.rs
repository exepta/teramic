use super::{TerraGenerator, biomes::smoothstep, seed::layer_seed};
use std::{collections::HashMap, sync::Mutex};

mod springs;
use springs::{Spring, SpringSample};

const SPACING: f64 = 256.0;
pub(super) const BANK_REACH: f64 = 18.0;

#[derive(Default)]
pub(super) struct OasisCache(Mutex<HashMap<[i64; 2], Option<Oasis>>>);

#[derive(Clone, Copy, Debug, PartialEq)]
struct Oasis {
    center: [f64; 2],
    radius: f64,
    level: f64,
    depth: f64,
    phase: f64,
    spring: Option<Spring>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SectionGenerator, TerrainGenerator};
    use terra_core::WorldConfig;
    use terra_voxel::{MaterialId, SectionPos};

    const CENTERS: [[f64; 2]; 3] = [[-2682.0, -153.0], [-2401.0, -1404.0], [-1939.0, -2166.0]];
    const REGIONAL_CENTERS: [[f64; 2]; 3] =
        [[-2159.0, -160.0], [-2406.0, 151.0], [-2682.0, -153.0]];

    #[test]
    fn isolated_lakes_have_local_levels_and_closed_supported_shores() {
        assert_closed_shores(1, CENTERS);
    }

    #[test]
    fn regional_landscape_oases_have_local_levels_and_closed_supported_shores() {
        assert_closed_shores(2, REGIONAL_CENTERS);
    }

    fn assert_closed_shores(revision: u8, centers: [[f64; 2]; 3]) {
        let terrain = TerraGenerator::new(WorldConfig {
            landscape_revision: revision,
            sea_level: 8.0,
            rivers: false,
            ..Default::default()
        })
        .unwrap();
        for [cx, cz] in centers {
            let oasis = terrain.oasis_at(cx, cz).unwrap();
            assert!(oasis.level > terrain.config().sea_level + 2.0);
            assert!(terrain.water_column(cx, cz).unwrap().depth > 2.0);
            let mut wet = 0;
            let mut shore = 0;
            for dz in -48..=48 {
                for dx in -48..=48 {
                    let x = cx + f64::from(dx) + 0.5;
                    let z = cz + f64::from(dz) + 0.5;
                    let column = terrain.surface_column(x, z);
                    let water = terrain.water_column(x, z);
                    assert_eq!(column.water_level, water.map(|w| w.level));
                    let Some(water) = water else { continue };
                    assert_eq!(water.level, oasis.level);
                    assert_eq!(column.water_bed(), water.bed);
                    assert!(terrain.sample([x, water.bed - 0.5, z]).density <= 0.0);
                    assert_eq!(column.river_cavity, None);
                    assert_eq!(column.cave_cavity, None);
                    wet += 1;
                    for [nx, nz] in [[x - 1.0, z], [x + 1.0, z], [x, z - 1.0], [x, z + 1.0]] {
                        if terrain.water_column(nx, nz).is_none() {
                            let bank = (terrain.surface_height(nx, nz) - 0.5).floor() + 1.0;
                            assert!(bank >= water.level - 0.5, "unsupported oasis edge {x},{z}");
                            shore += 1;
                        }
                    }
                }
            }
            assert!(wet > 200 && shore > 40);
        }
    }

    #[test]
    fn desert_strata_and_lake_geometry_survive_reload_and_negative_sections() {
        let config = WorldConfig {
            landscape_revision: 1,
            sea_level: 8.0,
            rivers: false,
            ..Default::default()
        };
        let terrain = TerraGenerator::new(config.clone()).unwrap();
        let reload = TerraGenerator::new(config).unwrap();
        let [x, z] = CENTERS[0];
        assert_eq!(terrain.oasis_at(x, z), reload.oasis_at(x, z));
        terrain.oases.0.lock().unwrap().clear();
        assert_eq!(terrain.oasis_at(x, z), reload.oasis_at(x, z));
        let column = terrain.surface_column(x + 42.5, z + 0.5);
        assert!(column.biome.weights.desert > 0.5);
        assert_eq!(terrain.column_material(column, 0.5, -0.5), MaterialId::Sand);
        assert_eq!(
            terrain.column_material(column, 2.0, -2.0),
            MaterialId::Sandstone
        );
        assert_eq!(
            terrain.column_material(column, 12.0, -12.0),
            MaterialId::Stone
        );
        let generator = SectionGenerator::new(terrain);
        let center = SectionPos::new((x as i64).div_euclid(16), 1, (z as i64).div_euclid(16));
        let mut meshes = 0;
        for dz in -1..=1 {
            for dx in -1..=1 {
                let position = SectionPos::new(center.x + dx, center.y, center.z + dz);
                let rendered = generator
                    .build(position, 0, &Default::default(), true)
                    .unwrap();
                let server = generator
                    .build(position, 0, &Default::default(), false)
                    .unwrap();
                assert_eq!(rendered.data, server.data);
                assert!(server.water.is_none());
                let Some(patch) = rendered.water else {
                    continue;
                };
                assert_eq!(
                    patch,
                    generator
                        .build(position, 0, &Default::default(), true)
                        .unwrap()
                        .water
                        .unwrap()
                );
                assert!(
                    patch
                        .indices
                        .iter()
                        .all(|&i| (i as usize) < patch.positions.len())
                );
                assert!(patch.positions.iter().flatten().all(|p| p.is_finite()));
                assert!(patch.surface_effects.iter().all(|effect| effect[3] == 0.0));
                meshes += 1;
            }
        }
        assert!(meshes >= 4);
    }

    #[test]
    fn desert_revision_fourteen_generates_closed_oases_in_all_three_chunk_area_bands() {
        let terrain = TerraGenerator::new(WorldConfig {
            ecology_revision: 14,
            landscape_revision: 9,
            rivers: false,
            ..Default::default()
        })
        .unwrap();
        let mut visited = [false; 3];
        // Real seed-selected basins. Sample at the exact candidate centre,
        // then rasterise their actual waterline against the 16x16 chunk area.
        'scan: for cz in -36_i64..=36 {
            for cx in -36_i64..=36 {
                let seed = layer_seed(
                    terrain.config().seed.0
                        ^ (cx as u64).wrapping_mul(0x9e3779b97f4a7c15)
                        ^ (cz as u64).wrapping_mul(0xbf58476d1ce4e5b9),
                    93,
                );
                let band = match seed % 10 {
                    0..=4 => 0,
                    5..=7 => 1,
                    _ => 2,
                };
                if visited[band] {
                    continue;
                }
                let x = (cx * 384 + 160 + i64::from((seed >> 4) % 64)) as f64;
                let z = (cz * 384 + 160 + i64::from((seed >> 12) % 64)) as f64;
                let Some(center) = terrain.oasis_at(x, z) else {
                    continue;
                };
                assert_eq!(terrain.biome_at(x, z).weights.cliffted_desert, 0.0);
                let center_water = terrain.water_column(x, z).expect("wet basin centre");
                assert!(center_water.depth > 2.0);
                let owner = SectionPos::new(
                    (x as i64).div_euclid(16),
                    (center_water.level as i64).div_euclid(16),
                    (z as i64).div_euclid(16),
                );
                let patch = SectionGenerator::new(terrain.clone())
                    .build(owner, 0, &Default::default(), true)
                    .unwrap()
                    .water
                    .expect("rendered oasis surface");
                let y = (center_water.level - (owner.y * 16) as f64) as f32;
                assert!(patch.positions.iter().any(|p| (p[1] - y).abs() < 0.001));
                let mut wet = 0;
                let extent = (center.radius * 1.25).ceil() as i32;
                for dz in -extent..=extent {
                    for dx in -extent..=extent {
                        let px = x + f64::from(dx) + 0.5;
                        let pz = z + f64::from(dz) + 0.5;
                        if let Some(s) = terrain.oasis_at(px, pz) {
                            let bed = (s.carve_desert(center.level + 4.0) - 0.5).floor() + 1.0;
                            if s.distance <= s.radius && bed < s.level {
                                wet += 1;
                            }
                        }
                    }
                }
                let area = f64::from(wet) / 256.0;
                let (lo, hi) = [(8.0, 12.0), (22.0, 40.0), (60.0, 120.0)][band];
                assert!(
                    area >= lo - 0.10 && area <= hi + 0.10,
                    "area {area}, band {lo}..{hi}"
                );
                eprintln!(
                    "Oasis {x:.0},{z:.0}: {area:.2} chunks, level {}",
                    center.level
                );
                visited[band] = true;
                if visited == [true; 3] {
                    break 'scan;
                }
            }
        }
        assert_eq!(visited, [true; 3]);
    }

    #[test]
    fn disabled_water_has_no_oasis_carving_or_sources() {
        // These three basins belong to the original isolated-oasis terrain.
        let config = WorldConfig {
            landscape_revision: 1,
            ecology_revision: 0,
            spirit_blossom_revision: 0,
            rivers: false,
            ..Default::default()
        };
        let wet = TerraGenerator::new(config.clone()).unwrap();
        for (water, biomes) in [(false, true), (true, false)] {
            let terrain = TerraGenerator::new(WorldConfig {
                water,
                biomes,
                ..config.clone()
            })
            .unwrap();
            for [x, z] in CENTERS {
                assert!(wet.oasis_at(x, z).is_some());
                assert!(wet.water_column(x, z).is_some());
                assert!(terrain.oasis_at(x, z).is_none());
                assert_eq!(
                    terrain.surface_height(x, z),
                    terrain.terrain_base_height(x, z)
                );
            }
        }
    }

    #[test]
    fn dominant_desert_keeps_sand_outside_oasis_grass_margins() {
        let terrain = TerraGenerator::new(WorldConfig {
            landscape_revision: 1,
            rivers: false,
            ..Default::default()
        })
        .unwrap();
        let mut desert = 0;
        let mut dry_grass = 0;
        for dz in (-80..=80).step_by(2) {
            for dx in (-80..=80).step_by(2) {
                let column = terrain
                    .surface_column(-2682.0 + f64::from(dx) + 0.5, -153.0 + f64::from(dz) + 0.5);
                if column.biome.aridity() <= 0.5 {
                    continue;
                }
                desert += 1;
                if column.surface_material == MaterialId::Grass {
                    assert!(column.oasis_moisture > 0.0 && column.water_level.is_none());
                    dry_grass += 1;
                } else {
                    assert_eq!(column.surface_material, MaterialId::Sand);
                }
            }
        }
        assert!(desert > 1000 && dry_grass > 30);
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OasisSample {
    pub center: [f64; 2],
    pub radius: f64,
    pub distance: f64,
    pub level: f64,
    pub depth: f64,
    pub(crate) spring: Option<SpringSample>,
}

impl OasisSample {
    // At the waterline the top solid voxel meets the integer lake level.
    // The dry rim blends both up and down into the surrounding dune field.
    pub(super) fn carve_desert(self, raw: f64) -> f64 {
        let bank = self.level - 0.5;
        let floor = if self.distance <= self.radius {
            let t = self.distance / self.radius;
            bank - self.depth * (1.0 - t * t)
        } else {
            let t = (self.distance - self.radius) / BANK_REACH;
            let lip = bank + smoothstep(0.0, 0.38, t) * 2.0;
            lip + smoothstep(0.22, 1.0, t) * (raw - lip)
        };
        self.spring
            .map_or(floor, |spring| spring.terrain(floor, raw, self.level))
    }

    pub(crate) fn surface_water_level(self) -> Option<f64> {
        self.spring
            .and_then(|s| s.water_level(self.level))
            .or_else(|| (self.distance <= self.radius).then_some(self.level))
    }

    pub(crate) fn spring_flow(self) -> Option<[f32; 2]> {
        self.spring.and_then(|spring| spring.flow())
    }

    pub fn moisture(self) -> f64 {
        1.0 - smoothstep(self.radius, self.radius + BANK_REACH, self.distance)
    }

    pub(super) fn carve(self, raw: f64) -> f64 {
        if self.distance < self.radius {
            let t = (self.distance / self.radius).clamp(0.0, 1.0);
            raw.min(self.level - self.depth + (self.depth + 0.8) * t.powi(2))
        } else {
            let blend = smoothstep(self.radius, self.radius + BANK_REACH, self.distance);
            raw.min(self.level + 0.8 + (raw - self.level - 0.8) * blend)
        }
    }
}

impl Oasis {
    fn sample(self, x: f64, z: f64) -> OasisSample {
        let dx = x - self.center[0];
        let dz = z - self.center[1];
        let (sin, cos) = self.phase.sin_cos();
        let u = (dx * cos + dz * sin) / 1.12;
        let v = (-dx * sin + dz * cos) * 1.12;
        let angle = v.atan2(u);
        let outline = 1.0
            + (angle * 3.0 + self.phase).sin() * 0.065
            + (angle * 5.0 - self.phase).cos() * 0.035;
        OasisSample {
            center: self.center,
            radius: self.radius,
            distance: u.hypot(v) / outline,
            level: self.level,
            depth: self.depth,
            spring: self.spring.and_then(|spring| spring.sample(x, z)),
        }
    }
}

impl TerraGenerator {
    /// Local lakes share the existing water columns, fluid sources and section batches.
    pub fn oasis_at(&self, x: f64, z: f64) -> Option<OasisSample> {
        if !self.config().water || !self.config().biomes || !x.is_finite() || !z.is_finite() {
            return None;
        }
        let modern = self.desert_overhaul();
        let spacing = if modern { 384.0 } else { SPACING };
        let cell = [(x / spacing).floor() as i64, (z / spacing).floor() as i64];
        let seed = layer_seed(
            self.config().seed.0
                ^ (cell[0] as u64).wrapping_mul(0x9e3779b97f4a7c15)
                ^ (cell[1] as u64).wrapping_mul(0xbf58476d1ce4e5b9),
            93,
        );
        if !modern && !seed.is_multiple_of(3) {
            return None;
        }
        let offset = if modern { 160.0 } else { 96.0 };
        let center = [
            cell[0] as f64 * spacing + offset + f64::from((seed >> 4) % 64),
            cell[1] as f64 * spacing + offset + f64::from((seed >> 12) % 64),
        ];
        let radius = desert_radius(seed);
        // The longest axis, the outline variation and the bank fit this tile.
        let reach = if modern {
            (radius + BANK_REACH) * 1.24
        } else {
            58.0
        };
        if (x - center[0]).abs().max((z - center[1]).abs()) > reach {
            return None;
        }
        let cached = self
            .oases
            .0
            .lock()
            .ok()
            .and_then(|cache| cache.get(&cell).copied());
        let oasis = cached.unwrap_or_else(|| {
            let oasis =
                if self.connected_hydrology() && self.connected_lake_conflicts(center, reach) {
                    None
                } else if modern {
                    self.build_desert_oasis(center, seed, radius)
                } else {
                    self.build_oasis(center, seed)
                };
            if let Ok(mut cache) = self.oases.0.lock() {
                if cache.len() >= 512 {
                    cache.clear();
                }
                *cache.entry(cell).or_insert(oasis)
            } else {
                oasis
            }
        })?;
        let sample = oasis.sample(x, z);
        (sample.distance < sample.radius + BANK_REACH).then_some(sample)
    }

    fn build_desert_oasis(&self, center: [f64; 2], seed: u32, radius: f64) -> Option<Oasis> {
        let biome = self.climate_biome_at(center[0], center[1]);
        if self.volcanic_watershed(biome) || biome.aridity().max(biome.weights.savanna) < 0.70 {
            return None;
        }
        let height = self.watershed_ground(center[0], center[1], biome);
        let mut oasis = Oasis {
            center,
            radius,
            level: (height - 1.5).round(),
            depth: 3.1 + radius * 0.038,
            spring: None,
            phase: f64::from(seed % 1024) * std::f64::consts::TAU / 1024.0,
        };
        if oasis.level < self.config().sea_level + 4.0 {
            return None;
        }
        // A bounded grid checks the whole basin and shore for waterways and
        // escarpments. The actual voxel floor is continuous between samples.
        let extent = ((radius + 12.0) * 1.24 / 8.0).ceil() as i32;
        for dz in -extent..=extent {
            for dx in -extent..=extent {
                let x = center[0] + f64::from(dx) * 8.0;
                let z = center[1] + f64::from(dz) * 8.0;
                if oasis.sample(x, z).distance > radius + 12.0 {
                    continue;
                }
                let climate = self.climate_biome_at(x, z);
                let ground = self.watershed_ground(x, z, climate);
                if self.volcanic_watershed(climate)
                    || climate.aridity().max(climate.weights.savanna) < 0.42
                    || ground < self.config().sea_level + 2.0
                    || (ground - height).abs() > 13.0 + radius * 0.14
                    || self.river_at(x, z).is_some()
                {
                    return None;
                }
            }
        }
        if biome.weights.savanna >= 0.70 && !seed.is_multiple_of(3) {
            oasis.spring = Spring::on_bank(self, oasis);
        }
        Some(oasis)
    }

    fn build_oasis(&self, center: [f64; 2], seed: u32) -> Option<Oasis> {
        let biome = self.climate_biome_at(center[0], center[1]);
        if biome.aridity() < 0.55 {
            return None;
        }
        let mut oasis = Oasis {
            center,
            radius: 15.0 + f64::from((seed >> 20) % 12),
            level: 0.0,
            depth: 2.7 + f64::from((seed >> 25) % 9) * 0.25,
            spring: None,
            phase: f64::from(seed % 1024) * std::f64::consts::TAU / 1024.0,
        };
        let mut low = self.base_height_with_biome(center[0], center[1], biome);
        let mut high = low;
        // Check the entire basin support, including nearby river cuts. This is
        // cached per oasis, not repeated for every voxel or surface column.
        for z in -36..=36 {
            for x in -36..=36 {
                let px = center[0] + f64::from(x);
                let pz = center[1] + f64::from(z);
                if oasis.sample(px, pz).distance > oasis.radius + 2.0 {
                    continue;
                }
                let raw = self.base_height_with_biome(px, pz, self.climate_biome_at(px, pz));
                low = low.min(raw);
                high = high.max(raw);
                if self.river_at(px, pz).is_some() {
                    return None;
                }
            }
        }
        if low < self.config().sea_level + 4.0 || high - low > 15.0 {
            return None;
        }
        oasis.level = low - 1.25;
        Some(oasis)
    }
}

// Area in horizontal 16×16 chunk equivalents, independent of aspect ratio.
// Normalise the mean squared polar outline to keep even the endpoints in range.
fn desert_radius(seed: u32) -> f64 {
    let area_hash = layer_seed(u64::from(seed), 3071);
    let (low, high) = match seed % 10 {
        0..=4 => (8, 12),
        5..=7 => (22, 40),
        _ => (60, 120),
    };
    let area = f64::from(low + area_hash % (high - low + 1));
    let mean_outline_squared = 1.0 + (0.065_f64.powi(2) + 0.035_f64.powi(2)) * 0.5;
    (area * 256.0 / (std::f64::consts::PI * mean_outline_squared)).sqrt()
}
