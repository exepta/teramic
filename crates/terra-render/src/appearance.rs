use bevy::prelude::*;
use terra_core::{GameConfig, WeatherKind};
use terra_world::{environment::SurfaceCondition, generation::biomes::BiomeSample};

const BIOME_COLORS: [[f32; 3]; 8] = [
    [0.56, 0.77, 0.50],
    [0.66, 0.83, 0.36],
    [0.49, 0.76, 0.33],
    [0.90, 0.78, 0.47],
    [0.58, 0.72, 0.51],
    [0.61, 0.70, 0.63],
    [0.90, 0.78, 0.47],
    [0.57, 0.81, 0.39],
];
pub(crate) static LINEAR_COLORS: std::sync::LazyLock<[[f32; 4]; 8]> =
    std::sync::LazyLock::new(|| {
        BIOME_COLORS.map(|c| Color::srgb(c[0], c[1], c[2]).to_linear().to_f32_array())
    });

pub(crate) fn biome_tint(sample: BiomeSample) -> [f32; 3] {
    let weights = sample.weights.values();
    std::array::from_fn(|axis| {
        weights
            .iter()
            .zip(LINEAR_COLORS.iter())
            .map(|(w, c)| *w as f32 * c[axis])
            .sum()
    })
}

pub(crate) fn condition_tint(condition: SurfaceCondition) -> [f32; 3] {
    let wet = 1.0 - condition.wetness * 0.28;
    std::array::from_fn(|axis| {
        wet * (1.0 - condition.charred) + [0.075, 0.070, 0.065][axis] * condition.charred
    })
}

pub(crate) fn weather_tint(weather: WeatherKind, intensity: f32) -> [f32; 3] {
    let target = match weather {
        WeatherKind::Clear => [1.0; 3],
        WeatherKind::Rain => [0.72, 0.78, 0.82],
        WeatherKind::Snow => [1.05, 1.05, 1.16],
    };
    target.map(|v| 1.0 + (v - 1.0) * intensity.clamp(0.0, 1.0))
}

pub(crate) fn visual_weather_tint(visual: &crate::weather::WeatherVisual) -> [f32; 3] {
    std::array::from_fn(|axis| {
        1.0 + ([0.87, 0.92, 0.95][axis] - 1.0) * visual.wetness
            + ([1.02, 1.03, 1.05][axis] - 1.0) * visual.snow
    })
}

type SurfaceMaterialStamp = ([f32; 3], f32, [f32; 3]);

pub(crate) fn update_weather_materials(
    config: Res<GameConfig>,
    visual: Option<Res<crate::weather::WeatherVisual>>,
    clock: Option<Res<terra_core::WorldClock>>,
    blocks: Res<crate::blocks::BlockAssets>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut previous: Local<Option<SurfaceMaterialStamp>>,
) {
    let tint = visual.as_ref().map_or_else(
        || weather_tint(config.client.weather, config.client.weather_intensity),
        |visual| visual_weather_tint(visual),
    );
    let wetness = visual.as_ref().map_or_else(
        || {
            if config.client.weather == WeatherKind::Rain {
                config.client.weather_intensity
            } else {
                0.0
            }
        },
        |visual| visual.wetness,
    );
    let key = (
        tint.map(|value| (value * 128.0).round() / 128.0),
        (wetness * 128.0).round() / 128.0,
        clock
            .as_deref()
            .copied()
            .unwrap_or_default()
            .season(config.world.seasons)
            .grass_tint()
            .map(|value| (value * 128.0).round() / 128.0),
    );
    if !blocks.is_changed() && previous.as_ref() == Some(&key) {
        return;
    }
    *previous = Some(key);
    for id in [1, 4, 5] {
        let Some(block) = blocks.templates.as_ref().and_then(|t| t.blocks.get(&id)) else {
            continue;
        };
        let Some(handle) = blocks.materials.get(block.face_materials[3]) else {
            continue;
        };
        let Some(definition) = blocks.cube_face_definition(id, 3) else {
            continue;
        };
        let Some(mut material) = materials.get_mut(handle) else {
            continue;
        };
        let [r, g, b, a] = definition.base_color;
        let season = if id == 1 { key.2 } else { [1.0; 3] };
        material.base_color = Color::linear_rgba(
            r * tint[0] * season[0],
            g * tint[1] * season[1],
            b * tint[2] * season[2],
            a,
        );
        // Authoritative wet films are roof-aware vertex payloads. A global
        // material must not make sheltered grass or stone glossy as well.
        material.perceptual_roughness = if visual.is_some() {
            definition.roughness
        } else {
            (definition.roughness - 0.28 * wetness).max(0.1)
        };
    }
}

/// Shared section corners make the tint continuous across negative coordinates
/// and independent of the mesh LOD. Only four climate samples per section.
pub(crate) struct GrassTint(pub [[f32; 3]; 4]);
impl GrassTint {
    pub fn new(terrain: &terra_world::TerraGenerator, origin: [i64; 3]) -> Self {
        Self(std::array::from_fn(|i| {
            biome_tint(terrain.biome_at(
                origin[0] as f64 + (i % 2) as f64 * 16.0,
                origin[2] as f64 + (i / 2) as f64 * 16.0,
            ))
        }))
    }
    pub fn at(&self, position: [f32; 3]) -> [f32; 3] {
        let x = (position[0] / 16.0).clamp(0.0, 1.0);
        let z = (position[2] / 16.0).clamp(0.0, 1.0);
        std::array::from_fn(|a| {
            let near = self.0[0][a] + (self.0[1][a] - self.0[0][a]) * x;
            let far = self.0[2][a] + (self.0[3][a] - self.0[2][a]) * x;
            near + (far - near) * z
        })
    }
    pub fn relative_at(&self, position: [f32; 3]) -> [f32; 3] {
        let tint = self.at(position);
        std::array::from_fn(|axis| (tint[axis] / LINEAR_COLORS[2][axis]).clamp(0.65, 1.6))
    }
}

/// Mean linear albedos of the shipped neutral turf, granular sand and snow.
/// Their texture relief remains intact; only the large-scale top color blends.
const TURF_ALBEDO: [f32; 3] = [0.387662, 0.383565, 0.382015];
const SNOW_ALBEDO: [f32; 3] = [0.94, 0.97, 1.0];
static SAND_ALBEDO: std::sync::LazyLock<[f32; 3]> = std::sync::LazyLock::new(|| {
    let c = Color::srgb(238.0 / 255.0, 203.0 / 255.0, 129.0 / 255.0).to_linear();
    [c.red, c.green, c.blue]
});

pub(crate) struct GroundTint {
    grass: GrassTint,
    /// Desert weight, local water level and climate-adjusted snow height.
    bands: GrassTint,
    origin_y: f32,
}

impl GroundTint {
    /// Painted PBR grass already has its own albedo, unlike the neutral turf texture.
    pub fn authored_grass_at(&self, position: [f32; 3]) -> [f32; 3] {
        self.grass.relative_at(position)
    }

    pub fn cave_grass_at(&self, position: [f32; 3]) -> [f32; 3] {
        self.grass.at(position)
    }
    pub fn new(terrain: &terra_world::TerraGenerator, origin: [i64; 3]) -> Self {
        let samples = std::array::from_fn::<_, 4, _>(|i| {
            let x = (origin[0] + (i % 2) as i64 * 16) as f64;
            let z = (origin[2] + (i / 2) as i64 * 16) as f64;
            let biome = terrain.biome_at(x, z);
            let level = terrain
                .oasis_at(x, z)
                .filter(|oasis| oasis.distance <= oasis.radius)
                .map_or_else(
                    || {
                        terrain
                            .river_at(x, z)
                            .map_or(terrain.config().sea_level, |river| {
                                river.water_level(terrain.config().sea_level)
                            })
                    },
                    |oasis| oasis.level,
                );
            (
                biome_tint(biome),
                [
                    biome.aridity() as f32,
                    level as f32,
                    (terrain.config().snow_height + (biome.temperature - 0.5) * 16.0) as f32,
                ],
            )
        });
        Self {
            grass: GrassTint(samples.map(|s| s.0)),
            bands: GrassTint(samples.map(|s| s.1)),
            origin_y: origin[1] as f32,
        }
    }

    pub fn at(&self, position: [f32; 3], material: u16) -> [f32; 3] {
        let reference = match material {
            1 => TURF_ALBEDO,
            4 => *SAND_ALBEDO,
            5 => SNOW_ALBEDO,
            _ => return [1.0; 3],
        };
        let [desert, level, snow_height] = self.bands.at(position);
        // Actual mesh elevation preserves narrow riverbeds between climate samples.
        // No extra terrain-height/river-roof searches are needed per tint sample.
        let height = self.origin_y + position[1] - 0.5;
        let coast = 1.0 - smoothstep(level, level + 3.0, height);
        let shore = 1.0 - (1.0 - coast) * (1.0 - desert);
        let snow = smoothstep(snow_height - 5.0, snow_height + 5.0, height)
            * (1.0 - smoothstep(0.35, 0.65, desert));
        let color = ground_color(self.grass.at(position), shore, snow);
        // Placed sand and full snow blocks away from their natural transition
        // bands keep their own color instead of acquiring a green upper face.
        let blend = match material {
            4 => smoothstep(0.0, 0.35, shore),
            5 => smoothstep(0.0, 0.35, snow),
            _ => 1.0,
        };
        std::array::from_fn(|axis| 1.0 + (color[axis] / reference[axis] - 1.0) * blend)
    }
}

fn smoothstep(low: f32, high: f32, value: f32) -> f32 {
    let t = ((value - low) / (high - low)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn ground_color(grass: [f32; 3], shore: f32, snow: f32) -> [f32; 3] {
    std::array::from_fn(|axis| {
        let turf = grass[axis] * TURF_ALBEDO[axis];
        let soil = turf + (SAND_ALBEDO[axis] - turf) * shore;
        soil + (SNOW_ALBEDO[axis] - soil) * snow
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cave_grass_below_sea_level_keeps_its_vegetation_color_instead_of_becoming_a_sandy_bank() {
        let tint = GroundTint {
            grass: GrassTint([[0.25, 0.62, 0.12]; 4]),
            bands: GrassTint([[0.0, 8.0, 100.0]; 4]),
            origin_y: -32.0,
        };
        assert_eq!(tint.cave_grass_at([8.0, 4.0, 8.0]), [0.25, 0.62, 0.12]);
        assert_ne!(
            tint.at([8.0, 4.0, 8.0], 1),
            tint.cave_grass_at([8.0, 4.0, 8.0])
        );
    }
    #[test]
    fn conditions_and_weather_are_neutral_when_disabled() {
        assert_eq!(condition_tint(SurfaceCondition::default()), [1.0; 3]);
        for weather in [WeatherKind::Clear, WeatherKind::Rain, WeatherKind::Snow] {
            assert_eq!(weather_tint(weather, 0.0), [1.0; 3]);
        }
        assert!(condition_tint(SurfaceCondition::new(1.0, 0.0).unwrap())[1] < 1.0);
        assert!(condition_tint(SurfaceCondition::new(0.0, 1.0).unwrap())[1] < 0.1);
    }
    #[test]
    fn tint_interpolation_preserves_shared_edges() {
        let a = GrassTint([[0.1; 3], [0.3; 3], [0.5; 3], [0.7; 3]]);
        let b = GrassTint([[0.3; 3], [0.9; 3], [0.7; 3], [0.2; 3]]);
        for z in 0..=16 {
            assert_eq!(a.at([16.0, 4.0, z as f32]), b.at([0.0, 9.0, z as f32]));
        }
    }

    #[test]
    fn plant_and_authored_grass_tints_use_the_same_forest_reference_and_section_edges() {
        let terrain = terra_world::TerraGenerator::new(terra_core::WorldConfig {
            rivers: false,
            structures_revision: 0,
            ..default()
        })
        .unwrap();
        for origin in [[-16, 48, -16], [0, 48, -16]] {
            let plants = GrassTint::new(&terrain, origin);
            let ground = GroundTint::new(&terrain, origin);
            for point in [[0.0, 1.0, 0.0], [8.0, 1.0, 8.0], [16.0, 1.0, 16.0]] {
                assert_eq!(plants.relative_at(point), ground.authored_grass_at(point));
            }
        }
        let forest = GrassTint([LINEAR_COLORS[2][..3].try_into().unwrap(); 4]);
        assert_eq!(forest.relative_at([8.0, 1.0, 8.0]), [1.0; 3]);
        let left = GrassTint::new(&terrain, [-16, 48, -16]);
        let right = GrassTint::new(&terrain, [0, 48, -16]);
        for z in [0.0, 4.0, 8.0, 16.0] {
            for (a, b) in left
                .relative_at([16.0, 1.0, z])
                .into_iter()
                .zip(right.relative_at([0.0, 1.0, z]))
            {
                assert!((a - b).abs() < 1e-6);
            }
        }
    }

    #[test]
    fn different_surface_materials_share_the_same_blended_top_albedo() {
        let terrain = terra_world::TerraGenerator::new(Default::default()).unwrap();
        let grass = biome_tint(terrain.biome_at(0.0, 0.0));
        let color = ground_color(grass, 0.5, 0.0);
        let tint = GroundTint {
            grass: GrassTint([grass; 4]),
            bands: GrassTint([[0.5, 0.0, 100.0]; 4]),
            origin_y: 0.0,
        };
        for (material, albedo) in [(1, TURF_ALBEDO), (4, *SAND_ALBEDO)] {
            let multiplier = tint.at([8.0, 20.0, 8.0], material);
            for axis in 0..3 {
                assert!((multiplier[axis] * albedo[axis] - color[axis]).abs() < 1e-6);
            }
        }
        assert_eq!(tint.at([8.0, 0.0, 8.0], 3), [1.0; 3]);
    }

    #[test]
    fn top_color_is_continuous_across_the_material_threshold() {
        let terrain = terra_world::TerraGenerator::new(Default::default()).unwrap();
        let grass = biome_tint(terrain.biome_at(0.0, 0.0));
        let before = ground_color(grass, 0.49999, 0.0);
        let after = ground_color(grass, 0.50001, 0.0);
        assert!(
            before
                .iter()
                .zip(after)
                .all(|(a, b)| (a - b).abs() < 0.0001)
        );
        let unchanged = ground_color(grass, 0.0, 0.0);
        assert!(
            (0..3).all(|axis| (unchanged[axis] / TURF_ALBEDO[axis] - grass[axis]).abs() < 1e-6)
        );
    }

    #[test]
    fn inland_building_blocks_keep_color_and_narrow_riverbeds_stay_sandy() {
        let tint = GroundTint {
            grass: GrassTint([[0.2, 0.6, 0.1]; 4]),
            bands: GrassTint([[0.0, 8.0, 112.0]; 4]),
            origin_y: 0.0,
        };
        assert_eq!(tint.at([8.0, 30.0, 8.0], 4), [1.0; 3]);
        assert_eq!(tint.at([8.0, 30.0, 8.0], 5), [1.0; 3]);
        assert_eq!(tint.at([8.0, 6.0, 8.0], 4), [1.0; 3]);
        assert_eq!(tint.at([8.0, 130.0, 8.0], 5), [1.0; 3]);
    }
}
#[test]
fn authored_grass_keeps_original_forest_albedo_and_bounds_other_biome_tints() {
    let tint = GroundTint {
        grass: GrassTint([LINEAR_COLORS[2][..3].try_into().unwrap(); 4]),
        bands: GrassTint([[0.0, 22.0, 112.0]; 4]),
        origin_y: 32.0,
    };
    assert_eq!(tint.authored_grass_at([8.0, 0.0, 8.0]), [1.0; 3]);
    for biome in LINEAR_COLORS.iter() {
        let sample = GroundTint {
            grass: GrassTint([biome[..3].try_into().unwrap(); 4]),
            bands: GrassTint([[0.0, 22.0, 112.0]; 4]),
            origin_y: 32.0,
        };
        assert!(
            sample
                .authored_grass_at([0.0; 3])
                .into_iter()
                .all(|v| (0.65..=1.6).contains(&v))
        );
    }
}
