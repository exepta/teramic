//! Revision-five surface fields. The existing hydrology and cave CSG run afterwards.
use super::{
    TerraGenerator, TerrainGenerator,
    biomes::{BiomeSample, BiomeWeights, smoothstep},
    seed::layer_seed,
};
use fastnoise_lite::{CellularReturnType, FastNoiseLite, FractalType, NoiseType};
use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex, OnceLock},
};
use terra_core::{BiomeKind, LandscapeDefinitions, TerrainKind, WorldConfig};

const SPACING: f64 = 16.0;
const CELLS: i64 = 8;
const SIDE: usize = 11;
const CACHE_LIMIT: usize = 128;
// Seamlessly fold noise coordinates before FNL's i32 lattice conversion. This
// keeps the existing +/-2^40 world precision budget valid, including high octaves.
const NOISE_PERIOD: f64 = 65_536.0;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct TerrainFields {
    pub continentalness: f64,
    pub erosion: f64,
    pub temperature: f64,
    pub humidity: f64,
    pub weirdness: f64,
    pub peaks_valleys: f64,
    pub ridge_strength: f64,
}

#[derive(Debug, Clone, Copy)]
pub struct TerrainDebugSample {
    pub fields: TerrainFields,
    pub profile: TerrainKind,
    pub height: f64,
    pub density: f64,
    pub biome: BiomeKind,
    pub river_strength: f64,
    pub water_level: Option<f64>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct SurfaceShape {
    pub strength: f64,
}

#[derive(Clone, Copy, Default)]
pub(super) struct TerrainNode {
    pub fields: TerrainFields,
    pub weights: [f64; 10],
    pub height: f64,
    pub overhangs: f64,
}

impl TerrainNode {
    pub fn profile(self) -> TerrainKind {
        let mut index = 0;
        for i in 1..self.weights.len() {
            if self.weights[i] > self.weights[index] {
                index = i;
            }
        }
        TerrainKind::ALL[index]
    }

    fn values(self) -> [f64; 19] {
        let f = self.fields;
        let mut values = [0.0; 19];
        values[..7].copy_from_slice(&[
            f.continentalness,
            f.erosion,
            f.temperature,
            f.humidity,
            f.weirdness,
            f.peaks_valleys,
            f.ridge_strength,
        ]);
        values[7] = self.height;
        values[8] = self.overhangs;
        values[9..].copy_from_slice(&self.weights);
        values
    }

    fn from_values(v: [f64; 19]) -> Self {
        let mut weights = [0.0; 10];
        let total: f64 = v[9..].iter().map(|v| v.max(0.0)).sum();
        for (weight, value) in weights.iter_mut().zip(&v[9..]) {
            *weight = value.max(0.0) / total.max(f64::EPSILON);
        }
        Self {
            fields: TerrainFields {
                continentalness: v[0].clamp(-1.0, 1.0),
                erosion: v[1].clamp(0.0, 1.0),
                temperature: v[2].clamp(0.0, 1.0),
                humidity: v[3].clamp(0.0, 1.0),
                weirdness: v[4].clamp(-1.0, 1.0),
                peaks_valleys: v[5].clamp(-1.0, 1.0),
                ridge_strength: v[6].clamp(0.0, 1.0),
            },
            weights,
            height: v[7].clamp(-119.0, 350.0),
            overhangs: v[8].clamp(0.0, 1.0),
        }
    }
}

struct TerrainTile {
    key: [i64; 2],
    nodes: [OnceLock<TerrainNode>; SIDE * SIDE],
}

impl TerrainTile {
    fn new(key: [i64; 2]) -> Self {
        Self {
            key,
            nodes: std::array::from_fn(|_| OnceLock::new()),
        }
    }
}

type TileCell = Arc<TerrainTile>;
#[derive(Default)]
struct FieldCache {
    tiles: HashMap<[i64; 2], TileCell>,
    order: VecDeque<[i64; 2]>,
}

pub(super) struct AdvancedSampler {
    broad: FastNoiseLite,
    smooth: FastNoiseLite,
    ridge: FastNoiseLite,
    cells: FastNoiseLite,
    warp: FastNoiseLite,
    density: FastNoiseLite,
    continents: FastNoiseLite,
    climate: FastNoiseLite,
    definitions: LandscapeDefinitions,
    biome_scale: f64,
    ocean_scale: f64,
    sea_level: f64,
    amplitude: f64,
    fantasy_relief: bool,
    regional_mountains: bool,
    regional_variety: bool,
    cache: Mutex<FieldCache>,
}

fn noise(
    seed: u64,
    layer: u64,
    kind: NoiseType,
    fractal: FractalType,
    octaves: i32,
) -> FastNoiseLite {
    let mut noise = FastNoiseLite::with_seed(layer_seed(seed, layer) as i32);
    noise.set_frequency(Some(1.0));
    noise.set_noise_type(Some(kind));
    noise.set_fractal_type(Some(fractal));
    noise.set_fractal_octaves(Some(octaves));
    noise
}

impl AdvancedSampler {
    pub fn new(config: &WorldConfig) -> Self {
        let seed = config.seed.0;
        let mut cells = noise(seed, 410, NoiseType::Cellular, FractalType::None, 1);
        cells.set_cellular_return_type(Some(CellularReturnType::Distance2Sub));
        Self {
            broad: noise(seed, 401, NoiseType::OpenSimplex2, FractalType::FBm, 3),
            smooth: noise(seed, 402, NoiseType::OpenSimplex2S, FractalType::FBm, 3),
            ridge: noise(seed, 403, NoiseType::OpenSimplex2, FractalType::Ridged, 4),
            warp: noise(seed, 404, NoiseType::OpenSimplex2S, FractalType::FBm, 2),
            density: noise(seed, 405, NoiseType::OpenSimplex2, FractalType::None, 1),
            continents: noise(seed, 406, NoiseType::OpenSimplex2S, FractalType::FBm, 3),
            climate: noise(seed, 407, NoiseType::OpenSimplex2S, FractalType::FBm, 2),
            cells,
            definitions: config
                .biome_definitions
                .as_ref()
                .map_or_else(LandscapeDefinitions::legacy_v5, |definitions| {
                    definitions.landscapes.clone()
                }),
            biome_scale: config.biome_scale
                * if config.landscape_revision >= 8 {
                    2.0
                } else {
                    1.0
                },
            ocean_scale: config.ocean_scale
                * if config.landscape_revision >= 8 {
                    2.0
                } else {
                    1.0
                },
            sea_level: config.sea_level,
            amplitude: ((config.hill_amplitude + config.mountain_amplitude) / 60.0).clamp(0.0, 2.0),
            fantasy_relief: config.landscape_revision >= 6,
            regional_mountains: config.landscape_revision >= 7,
            regional_variety: config.landscape_revision >= 8,
            cache: Default::default(),
        }
    }

    fn tile(&self, key: [i64; 2]) -> TileCell {
        let mut cache = self.cache.lock().unwrap_or_else(|error| error.into_inner());
        if let Some(tile) = cache.tiles.get(&key) {
            return tile.clone();
        }
        if cache.tiles.len() == CACHE_LIMIT {
            let unused = cache.order.iter().position(|key| {
                cache
                    .tiles
                    .get(key)
                    .is_some_and(|tile| Arc::strong_count(tile) == 1)
            });
            if let Some(index) = unused {
                let oldest = cache.order.remove(index).unwrap();
                cache.tiles.remove(&oldest);
            }
        }
        let tile = Arc::new(TerrainTile::new(key));
        // An active sampler owns an Arc while its stencil is initialized. Never
        // evict that tile; if all are busy, this query uses a private bounded tile.
        if cache.tiles.len() < CACHE_LIMIT {
            cache.tiles.insert(key, tile.clone());
            cache.order.push_back(key);
        }
        tile
    }

    fn node_at<'a>(&self, tile: &'a TerrainTile, index: usize) -> &'a TerrainNode {
        tile.nodes[index].get_or_init(|| {
            self.node(
                (tile.key[0] * CELLS + (index % SIDE) as i64 - 1) as f64 * SPACING,
                (tile.key[1] * CELLS + (index / SIDE) as i64 - 1) as f64 * SPACING,
            )
        })
    }

    pub fn height(&self, x: f64, z: f64) -> f64 {
        let gx = (x / SPACING).floor() as i64;
        let gz = (z / SPACING).floor() as i64;
        let tile = self.tile([gx.div_euclid(CELLS), gz.div_euclid(CELLS)]);
        let ix = gx.rem_euclid(CELLS) as usize;
        let iz = gz.rem_euclid(CELLS) as usize;
        let tx = x / SPACING - gx as f64;
        let tz = z / SPACING - gz as f64;
        let rows = std::array::from_fn(|row| {
            cubic(
                std::array::from_fn(|column| {
                    self.node_at(&tile, ix + column + (iz + row) * SIDE).height
                }),
                tx,
            )
        });
        cubic(rows, tz).clamp(-119.0, 350.0)
    }

    pub fn sample(&self, x: f64, z: f64) -> TerrainNode {
        let gx = (x / SPACING).floor() as i64;
        let gz = (z / SPACING).floor() as i64;
        let tile = self.tile([gx.div_euclid(CELLS), gz.div_euclid(CELLS)]);
        let ix = gx.rem_euclid(CELLS) as usize;
        let iz = gz.rem_euclid(CELLS) as usize;
        let tx = x / SPACING - gx as f64;
        let tz = z / SPACING - gz as f64;
        let mut rows = [[0.0; 19]; 4];
        for (row, values) in rows.iter_mut().enumerate() {
            let samples = std::array::from_fn::<_, 4, _>(|column| {
                self.node_at(&tile, ix + column + (iz + row) * SIDE)
                    .values()
            });
            for channel in 0..19 {
                values[channel] = cubic(samples.map(|s| s[channel]), tx);
            }
        }
        TerrainNode::from_values(std::array::from_fn(|channel| {
            cubic(rows.map(|s| s[channel]), tz)
        }))
    }

    fn node(&self, x: f64, z: f64) -> TerrainNode {
        let d = &self.definitions;
        let wx = x + field(&self.warp, x, z, d.warp_scale) * d.warp_strength;
        let wz = z + field(&self.warp, x + 317.0, z - 149.0, d.warp_scale) * d.warp_strength;
        let continentalness =
            (field(&self.continents, wx, wz, self.ocean_scale) * 1.65).clamp(-1.0, 1.0);
        let erosion = unit(field(&self.smooth, wx - 701.0, wz + 311.0, d.region_scale * 0.6) * 1.5);
        let temperature = unit(field(&self.climate, x + 1133.0, z - 791.0, self.biome_scale) * 1.6);
        let humidity = unit(
            field(
                &self.climate,
                x - 4291.0,
                z + 8191.0,
                self.biome_scale * 0.83,
            ) * 1.6,
        );
        let weirdness =
            (field(&self.broad, wx + 271.0, wz, d.region_scale * 0.7) * 1.7).clamp(-1.0, 1.0);
        let peaks_valleys = (1.0 - (3.0 * weirdness.abs() - 2.0).abs()).clamp(-1.0, 1.0);
        let ridge_strength = unit(field(&self.ridge, wx, wz, d.region_scale));
        let district =
            ((1.0 + field(&self.cells, wx, wz, d.region_scale * 0.65)) * 2.0).clamp(0.0, 1.0);
        let fields = TerrainFields {
            continentalness,
            erosion,
            temperature,
            humidity,
            weirdness,
            peaks_valleys,
            ridge_strength,
        };
        let mut weights = if self.regional_mountains {
            // A separate broad field confines mountain families to occasional
            // ranges. Ridged noise alone is high across most of the continent.
            let region =
                unit(field(&self.broad, wx - 5317.0, wz + 4123.0, d.region_scale * 2.0) * 1.75);
            profile_weights_scaled(fields, district, d, smoothstep(0.60, 0.79, region))
        } else {
            profile_weights(fields, district, d)
        };
        if self.regional_variety {
            let cold = 1.0 - smoothstep(0.28, 0.40, temperature);
            let hot = smoothstep(0.60, 0.72, temperature);
            let lost = (weights[4] + weights[5]) * (1.0 - cold) + weights[8] * (1.0 - hot);
            weights[4] *= cold;
            weights[5] *= cold;
            weights[8] *= hot;
            weights[2] += lost * humidity;
            weights[0] += lost * (1.0 - humidity);
        }
        // Shared local districts vary the relief within a biome without changing
        // its label. Two samples per cached terrain node, never per voxel.
        let district = if self.regional_variety {
            field(&self.smooth, wx + 1881.0, wz - 377.0, 1250.0)
        } else {
            0.0
        };
        let wrinkles = if self.regional_variety {
            field(&self.broad, wx - 533.0, wz + 977.0, 145.0)
        } else {
            0.0
        };
        let fantasy = self.fantasy_relief.then(|| {
            super::fantasy_relief::Fields::new(
                field(&self.smooth, wx + 1371.0, wz - 943.0, d.region_scale * 0.44),
                field(&self.ridge, wx - 719.0, wz + 359.0, d.region_scale * 0.16),
                field(&self.broad, wx + 3071.0, wz + 1213.0, d.region_scale * 0.52),
                field(
                    &self.smooth,
                    wx - 2147.0,
                    wz - 1811.0,
                    d.region_scale * 0.31,
                ),
            )
        });
        let mut height = 0.0;
        let mut overhangs = 0.0;
        for (profile, weight) in d.profiles.iter().zip(weights) {
            if weight == 0.0 {
                continue;
            }
            let scale = profile.horizontal_scale;
            let broad = field(&self.broad, wx, wz, scale);
            let soft = field(&self.smooth, wx, wz, scale * 0.65);
            let billow = 2.0 * soft.abs() - 1.0;
            let ridged = unit(field(&self.ridge, wx, wz, scale));
            let mut shape = match profile.kind {
                TerrainKind::Plains => 0.52 + billow * 0.24 + soft * 0.18,
                TerrainKind::RollingPlains => 0.55 + broad * 0.32 + billow * 0.22,
                TerrainKind::Forest => 0.55 + soft * 0.42 + broad * 0.18,
                TerrainKind::MountainForest => 0.16 + ridged.powf(1.45) * 0.82,
                TerrainKind::FantasyMountains => {
                    0.10 + (ridged * 0.60 + unit(peaks_valleys) * 0.40).powf(1.3) * 0.90
                }
                TerrainKind::SnowyHighlands => {
                    0.20 + self.hybrid(wx / scale, wz / scale) * 0.35 + ridged * 0.43
                }
                TerrainKind::CherryGrove => 0.53 + soft * 0.32 + billow * 0.16,
                TerrainKind::SpiritForest => {
                    0.24 + self.hybrid(wx / scale, wz / scale) * 0.44 + ridged * 0.28
                }
                TerrainKind::Desert => {
                    let angle = d.dune_wind_angle.to_radians();
                    let wind = x * angle.cos() + z * angle.sin();
                    let dune = ((wind / 43.0 + soft * 1.2).sin() * 0.5 + 0.5).powi(3);
                    0.42 + broad * 0.25 + dune * 0.19 + smoothstep(0.65, 0.9, ridged) * 0.16
                }
                TerrainKind::Ocean => {
                    let depth = spline(
                        continentalness,
                        &[[-1.0, 0.02], [-0.5, 0.15], [-0.18, 0.52], [0.18, 1.0]],
                    );
                    let bottom = profile.min_height
                        + (profile.max_height.min(self.sea_level) - profile.min_height) * depth;
                    let valley = 1.0 - smoothstep(-0.95, -0.25, peaks_valleys);
                    let relief = broad * 5.0 * profile.roughness
                        + (ridged - 0.5) * profile.ridge_strength * 6.0
                        - valley * profile.erosion * 3.0;
                    height += (bottom + relief) * weight;
                    continue;
                }
            };
            // Analytic erosion lowers coherent valleys and suppresses fine relief;
            // actual routed river/basin erosion remains in the existing hydrology.
            let valley = 1.0 - smoothstep(-0.95, -0.25, peaks_valleys);
            shape -= valley * profile.erosion * 0.13;
            shape += (ridged - 0.5) * profile.ridge_strength * 0.14;
            shape += broad * profile.roughness * (1.0 - erosion * profile.erosion) * 0.12;
            if let Some(fantasy) = fantasy {
                use super::fantasy_relief::Style;
                let style = match profile.kind {
                    TerrainKind::Plains => Style::Plains,
                    TerrainKind::RollingPlains => Style::Hills,
                    TerrainKind::Forest => Style::Forest,
                    TerrainKind::MountainForest | TerrainKind::FantasyMountains => Style::Mountain,
                    TerrainKind::SnowyHighlands => Style::Alpine,
                    TerrainKind::CherryGrove => Style::Grove,
                    TerrainKind::SpiritForest => Style::Spirit,
                    TerrainKind::Desert => Style::Desert,
                    TerrainKind::Ocean => unreachable!("ocean has its unchanged seabed path"),
                };
                shape = if self.regional_mountains
                    && !matches!(
                        profile.kind,
                        TerrainKind::MountainForest
                            | TerrainKind::FantasyMountains
                            | TerrainKind::SnowyHighlands
                    ) {
                    // Lowland biomes keep soft relief rather than inheriting
                    // the new cliffs and rock shelves from every fantasy style.
                    let softness = if matches!(
                        profile.kind,
                        TerrainKind::Plains | TerrainKind::RollingPlains
                    ) {
                        0.65
                    } else {
                        0.80
                    };
                    0.5 + (shape - 0.5) * softness
                } else {
                    fantasy.shape(style, shape, broad, erosion * profile.erosion)
                };
            }
            if self.regional_variety {
                let mountain = matches!(
                    profile.kind,
                    TerrainKind::MountainForest
                        | TerrainKind::FantasyMountains
                        | TerrainKind::SnowyHighlands
                );
                if mountain {
                    shape += wrinkles * 0.045 + district * 0.055;
                } else {
                    let undulation = 0.25 + smoothstep(-0.55, 0.55, district) * 1.05;
                    let basin = (1.0 - smoothstep(-0.65, -0.15, district)) * 0.10;
                    shape = 0.5 + (shape - 0.5) * undulation + wrinkles * 0.11 - basin;
                }
            }
            shape = 0.5 + (shape - 0.5) * self.amplitude;
            shape = shape.clamp(0.0, 1.0);
            let target = profile.min_height + (profile.max_height - profile.min_height) * shape;
            // Continents provide the common shoreline anchor, independent of biome IDs.
            let coast = smoothstep(-0.14, 0.24, continentalness);
            height += (self.sea_level + (target - self.sea_level) * coast) * weight;
            overhangs += profile.overhangs * weight * (1.0 - erosion * 0.5);
        }
        TerrainNode {
            fields,
            weights,
            height,
            overhangs,
        }
    }

    fn hybrid(&self, x: f64, z: f64) -> f64 {
        let mut result = 0.0;
        let mut feedback = 1.0;
        let mut amplitude = 0.57;
        for octave in 0..3 {
            let frequency = 2.0_f64.powi(octave);
            let signal = unit(field(&self.density, x * frequency, z * frequency, 1.0));
            result += signal * amplitude * feedback;
            feedback = (signal * 1.8).clamp(0.0, 1.0);
            amplitude *= 0.5;
        }
        result.clamp(0.0, 1.0)
    }

    pub fn density(&self, p: [f64; 3], column: super::SurfaceColumn, original: f64) -> f64 {
        let strength = column.surface_shape.strength;
        let depth = column.height - p[1];
        if strength <= 0.005 || !(3.0..28.0).contains(&depth) {
            return original;
        }
        let x = (p[0] / 38.0).rem_euclid(NOISE_PERIOD);
        let z = (p[2] / 38.0).rem_euclid(NOISE_PERIOD);
        let tx = smoothstep(0.0, NOISE_PERIOD, x);
        let tz = smoothstep(0.0, NOISE_PERIOD, z);
        let noise = |x, z| f64::from(self.density.get_noise_3d(x, p[1] / 23.0, z));
        let a = noise(x, z) * (1.0 - tx) + noise(x - NOISE_PERIOD, z) * tx;
        let b = noise(x, z - NOISE_PERIOD) * (1.0 - tx)
            + noise(x - NOISE_PERIOD, z - NOISE_PERIOD) * tx;
        let carve = ((a + (b - a) * tz) - (0.58 - strength * 0.28)) * 12.0;
        original.max(carve.min(depth - 4.0).min(27.0 - depth))
    }
}

fn profile_weights(f: TerrainFields, district: f64, d: &LandscapeDefinitions) -> [f64; 10] {
    profile_weights_scaled(f, district, d, 1.0)
}

fn profile_weights_scaled(
    f: TerrainFields,
    district: f64,
    d: &LandscapeDefinitions,
    mountain_permission: f64,
) -> [f64; 10] {
    let marine = 1.0 - smoothstep(-0.16, 0.20, f.continentalness);
    let ocean_weight = d.get(TerrainKind::Ocean).weight;
    let ocean = marine * ocean_weight / (marine * ocean_weight + (1.0 - marine));
    let land = 1.0 - ocean;
    let forest = smoothstep(0.30, 0.72, f.humidity);
    let dry = smoothstep(0.56, 0.78, f.temperature) * (1.0 - smoothstep(0.25, 0.49, f.humidity));
    let mountain = smoothstep(
        0.38,
        0.74,
        f.ridge_strength * 0.55 + unit(f.peaks_valleys) * 0.45,
    ) * (1.0 - smoothstep(0.55, 0.90, f.erosion) * 0.70);
    let mountain = if mountain_permission < 1.0 {
        mountain * mountain_permission.max(0.0)
    } else {
        mountain
    };
    let cold = 1.0 - smoothstep(0.24, 0.45, f.temperature);
    let fantasy = mountain * smoothstep(-0.15, 0.70, f.peaks_valleys) * (1.0 - cold * 0.6);
    let snowy = mountain * cold;
    let mountain_forest = mountain * (1.0 - fantasy) * (1.0 - cold) * (0.25 + forest * 0.75);
    let cherry = smoothstep(0.32, 0.48, f.temperature)
        * (1.0 - smoothstep(0.65, 0.8, f.temperature))
        * forest
        * smoothstep(0.48, 0.78, district)
        * (1.0 - mountain)
        * 0.8;
    let spirit = smoothstep(0.64, 0.88, district)
        * smoothstep(0.5, 0.76, f.humidity)
        * (1.0 - cold)
        * (0.3 + mountain * 0.7);
    let rolling = (1.0 - mountain) * (1.0 - f.erosion * 0.65) * (1.0 - forest * 0.7);
    let mut weights = [
        (1.0 - mountain) * (1.0 - forest) * (0.25 + f.erosion),
        rolling,
        (1.0 - mountain) * forest,
        mountain_forest * 1.5,
        fantasy * 1.8,
        snowy * 1.8,
        cherry * 1.6,
        spirit,
        dry * 3.0,
        0.0,
    ];
    // A rare humid cellular district must form an actual region, not merely a
    // permanently subordinate modifier mixed into every neighbouring mountain.
    for (index, weight) in weights.iter_mut().enumerate().take(9) {
        if index == TerrainKind::SpiritForest as usize {
            *weight *= 2.4;
        } else {
            *weight *= 1.0 - spirit * 0.85;
        }
    }
    let mut total = 0.0;
    for (w, profile) in weights.iter_mut().zip(d.profiles) {
        *w *= profile.weight;
        total += *w;
    }
    if total <= f64::EPSILON {
        if let Some(index) = d.profiles[..9].iter().position(|p| p.weight > 0.0) {
            weights[index] = land;
        } else {
            weights[9] = 1.0;
            return weights;
        }
    } else {
        for w in &mut weights[..9] {
            *w *= land / total;
        }
    }
    weights[9] = ocean;
    weights
}

fn unit(value: f64) -> f64 {
    (0.5 + value * 0.5).clamp(0.0, 1.0)
}

fn field(noise: &FastNoiseLite, x: f64, z: f64, scale: f64) -> f64 {
    let x = (x / scale).rem_euclid(NOISE_PERIOD);
    let z = (z / scale).rem_euclid(NOISE_PERIOD);
    let tx = smoothstep(0.0, NOISE_PERIOD, x);
    let tz = smoothstep(0.0, NOISE_PERIOD, z);
    let n = |x, z| f64::from(noise.get_noise_2d(x, z));
    let a = n(x, z) * (1.0 - tx) + n(x - NOISE_PERIOD, z) * tx;
    let b = n(x, z - NOISE_PERIOD) * (1.0 - tx) + n(x - NOISE_PERIOD, z - NOISE_PERIOD) * tx;
    a + (b - a) * tz
}

fn cubic(v: [f64; 4], t: f64) -> f64 {
    0.5 * ((2.0 * v[1])
        + (-v[0] + v[2]) * t
        + (2.0 * v[0] - 5.0 * v[1] + 4.0 * v[2] - v[3]) * t * t
        + (-v[0] + 3.0 * v[1] - 3.0 * v[2] + v[3]) * t * t * t)
}

/// Shape-preserving Hermite spline: each interval has a continuous flat tangent.
fn spline(value: f64, points: &[[f64; 2]]) -> f64 {
    if value <= points[0][0] {
        return points[0][1];
    }
    for pair in points.windows(2) {
        if value <= pair[1][0] {
            let blend = smoothstep(pair[0][0], pair[1][0], value);
            return pair[0][1] + (pair[1][1] - pair[0][1]) * blend;
        }
    }
    points[points.len() - 1][1]
}

impl TerraGenerator {
    pub fn terrain_fields(&self, x: f64, z: f64) -> TerrainFields {
        if self.config().landscape_revision < 5 {
            let n = self.landscape_noise(x, z);
            let biome = self.climate_biome_at(x, z);
            return TerrainFields {
                continentalness: n.continentalness,
                erosion: n.erosion,
                temperature: biome.temperature,
                humidity: biome.humidity,
                weirdness: n.peaks_valleys,
                peaks_valleys: n.peaks_valleys,
                ridge_strength: unit(n.peaks_valleys),
            };
        }
        self.advanced.sample(x, z).fields
    }

    pub fn terrain_profile(&self, x: f64, z: f64) -> TerrainKind {
        if self.config().landscape_revision < 5 {
            return match self.climate_biome_at(x, z).weights.dominant() {
                BiomeKind::Ocean => TerrainKind::Ocean,
                BiomeKind::Plains => TerrainKind::Plains,
                BiomeKind::Forest | BiomeKind::BloodForest => TerrainKind::Forest,
                BiomeKind::Desert | BiomeKind::ClifftedDesert => TerrainKind::Desert,
                BiomeKind::Highland => TerrainKind::SnowyHighlands,
                BiomeKind::MushroomPark => TerrainKind::SpiritForest,
                BiomeKind::CherryForest => TerrainKind::CherryGrove,
                BiomeKind::SpiritBlossom => TerrainKind::SpiritForest,
            };
        }
        self.advanced.sample(x, z).profile()
    }

    pub fn terrain_profile_weights(&self, x: f64, z: f64) -> [f64; 10] {
        self.advanced.sample(x, z).weights
    }

    pub fn terrain_base_height(&self, x: f64, z: f64) -> f64 {
        if self.config().landscape_revision >= 5 {
            return if self.config().hill_amplitude == 0.0 && self.config().mountain_amplitude == 0.0
            {
                self.config().base_height
            } else {
                self.advanced_height(x, z)
            };
        }
        self.base_height_with_biome(x, z, self.climate_biome_at(x, z))
    }

    pub(super) fn advanced_height(&self, x: f64, z: f64) -> f64 {
        self.spirit_terraced_height(x, z, self.advanced.height(x, z))
    }

    pub(super) fn advanced_biome(&self, x: f64, z: f64) -> BiomeSample {
        self.biome_from_node(self.advanced.sample(x, z))
    }

    /// Region centres need only one noise evaluation, independent of the
    /// sixteen-node interpolation stencil used for local terrain.
    pub(super) fn biome_at_region_center(&self, x: f64, z: f64) -> BiomeSample {
        self.biome_from_node(self.advanced.node(x, z))
    }

    fn biome_from_node(&self, sample: TerrainNode) -> BiomeSample {
        let f = sample.fields;
        let w = sample.weights;
        let ocean = w[9];
        let land = 1.0 - ocean;
        if self.config().landscape_revision >= 7 {
            let mushrooms =
                w[2] * smoothstep(0.50, 0.82, f.humidity) * (1.0 - unit(f.weirdness)) * 0.75;
            let cliffs = if self.config().geology_revision > 0 {
                // Ridge interiors must be able to become a visible Cliffted
                // Desert, including the selected-biome world creation path.
                w[8] * smoothstep(0.40, 0.75, f.ridge_strength)
            } else {
                0.0
            };
            // The biome's label and vegetation must describe the actual relief:
            // a warm mountain range is not mislabeled as a flat Plains biome.
            return BiomeSample {
                temperature: f.temperature,
                humidity: f.humidity,
                weights: BiomeWeights {
                    ocean,
                    plains: w[0] + w[1],
                    forest: w[2] + w[3] - mushrooms,
                    highland: w[4] + w[5],
                    desert: w[8] - cliffs,
                    cliffted_desert: cliffs,
                    cherry_forest: w[6],
                    spirit_blossom: w[7],
                    mushroom_park: mushrooms,
                    blood_forest: 0.0,
                },
            };
        }
        let desert = land
            * smoothstep(0.56, 0.78, f.temperature)
            * (1.0 - smoothstep(0.25, 0.49, f.humidity));
        let cliffted_desert = if self.config().geology_revision > 0 {
            desert * f.ridge_strength * (1.0 - f.erosion * 0.7)
        } else {
            0.0
        };
        let highland =
            (land - desert) * (1.0 - smoothstep(0.26, 0.50, f.temperature)) * f.ridge_strength;
        let rest = (land - desert - highland).max(0.0);
        let forest = rest * smoothstep(0.30, 0.72, f.humidity);
        let cherry_forest = forest
            * smoothstep(0.32, 0.48, f.temperature)
            * (1.0 - smoothstep(0.65, 0.80, f.temperature))
            * unit(f.weirdness);
        let mushroom_park = (forest - cherry_forest)
            * smoothstep(0.50, 0.82, f.humidity)
            * (1.0 - unit(f.weirdness))
            * 0.75;
        BiomeSample {
            temperature: f.temperature,
            humidity: f.humidity,
            weights: BiomeWeights {
                ocean,
                desert: desert - cliffted_desert,
                highland,
                forest: (forest - cherry_forest - mushroom_park).max(0.0),
                plains: rest - forest,
                cherry_forest,
                spirit_blossom: 0.0,
                blood_forest: 0.0,
                mushroom_park,
                cliffted_desert,
            },
        }
    }

    pub fn terrain_debug_sample(&self, position: [f64; 3]) -> TerrainDebugSample {
        let [x, _, z] = position;
        let column = self.surface_column(x, z);
        let fields = self.terrain_fields(x, z);
        TerrainDebugSample {
            fields,
            profile: self.terrain_profile(x, z),
            height: column.height,
            density: f64::from(self.sample(position).density),
            biome: self.surface_biome(&column),
            river_strength: self.river_at(x, z).map_or(0.0, |r| {
                1.0 - smoothstep(r.width, r.width + 20.0, r.distance)
            }),
            water_level: column.water_level,
        }
    }
}

#[cfg(test)]
#[path = "advanced/region_tests.rs"]
mod region_tests;
#[cfg(test)]
mod tests;
