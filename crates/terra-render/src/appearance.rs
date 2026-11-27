use bevy::prelude::*;
use terra_core::{GameConfig, WeatherKind};
use terra_world::{environment::SurfaceCondition, generation::biomes::BiomeSample};

const BIOME_COLORS: [[f32; 3]; 16] = [
    [0.56, 0.77, 0.50],
    [0.66, 0.83, 0.36],
    [0.49, 0.76, 0.33],
    [0.90, 0.78, 0.47],
    [0.58, 0.72, 0.51],
    [0.61, 0.70, 0.63],
    [0.90, 0.78, 0.47],
    [0.61, 0.70, 0.43], // Soft sage grass among the fallen cherry petals.
    [0.47, 0.57, 0.53], // Silver sage and muted moss below the Spirit canopy.
    [0.68, 0.36, 0.44], // Blood Forest: rose-crimson, readable under a closed canopy.
    [0.40, 0.34, 0.34], // Scorched valley floor.
    [0.72, 0.71, 0.34], // Sun-dried grassy savanna.
    [0.49, 0.57, 0.32], // Moss and sedges on wet peat.
    [0.63, 0.53, 0.55], // Dust and weathered stone.
    [0.66, 0.69, 0.61], // Lichen and silver grass between the alpine veins.
    [0.53, 0.65, 0.48], // Salt-tolerant grass and maritime heath.
];
pub(crate) static LINEAR_COLORS: std::sync::LazyLock<[[f32; 4]; 16]> =
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

/// Fade the live turf toward the local dune or crimson canopy before its
/// physical surface changes. The same corner samples feed grass and grass tufts.
fn turf_palette(sample: BiomeSample, sediment: f32) -> ([f32; 3], f32) {
    let fade = |value: f32, low: f32, high: f32| {
        let t = ((value - low) / (high - low)).clamp(0., 1.);
        t * t * (3. - 2. * t)
    };
    let weights = sample.weights;
    let dry = fade(sample.aridity() as f32, 0.36, 0.75);
    let red_sand = fade(sediment, 0.18, 0.46);
    let red_wood = fade(weights.blood_forest as f32, 0.11, 0.71);
    // The dune textures are painted in linear albedo, whereas authored meadow
    // vertices multiply an already-painted turf. Convert their mean albedos into
    // that palette's units so the last grass cells meet the sand luminance.
    let dune_albedo = Vec3::new(0.773, 0.501, 0.215).lerp(Vec3::new(0.471, 0.158, 0.061), red_sand);
    let forest = Vec3::from_slice(&LINEAR_COLORS[2][..3]);
    let meadow_albedo = Vec3::new(0.155556, 0.275173, 0.033548);
    let dunes = dune_albedo * forest / meadow_albedo;
    let grass = Vec3::from_array(biome_tint(sample))
        .lerp(dunes, dry)
        .lerp(Vec3::from_slice(&LINEAR_COLORS[9][..3]), red_wood);
    let palette = (weights.cherry_forest + weights.spirit_blossom + weights.blood_forest) as f32;
    (grass.to_array(), palette.max(dry).max(red_wood))
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
pub(crate) struct GrassTint(pub [[f32; 3]; 4], pub [f32; 4]);
impl GrassTint {
    pub fn new(terrain: &terra_world::TerraGenerator, origin: [i64; 3]) -> Self {
        let corners: [([f32; 3], f32); 4] = std::array::from_fn(|i| {
            let x = origin[0] as f64 + (i % 2) as f64 * 16.0;
            let z = origin[2] as f64 + (i / 2) as f64 * 16.0;
            let sample = terrain.biome_at(x, z);
            let desert = sample.aridity() as f32;
            let ochre = if desert > 0.36 {
                terrain.dune_sediment(x, z) as f32
            } else {
                0.
            };
            turf_palette(sample, ochre)
        });
        Self(
            corners.map(|(color, _)| color),
            corners.map(|(_, strength)| strength),
        )
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

        terra_core::relative_grass_tint(
            tint,
            [
                LINEAR_COLORS[2][0],
                LINEAR_COLORS[2][1],
                LINEAR_COLORS[2][2],
            ],
            bilinear(self.1, position),
        )
    }
}

fn bilinear(values: [f32; 4], point: [f32; 3]) -> f32 {
    let x = (point[0] / 16.0).clamp(0.0, 1.0);
    let z = (point[2] / 16.0).clamp(0.0, 1.0);
    let near = values[0] + (values[1] - values[0]) * x;
    let far = values[2] + (values[3] - values[2]) * x;
    near + (far - near) * z
}

pub(crate) fn tinted_ground(id: u16) -> bool {
    matches!(id, 1 | 306 | 400 | 632 | 633 | 670)
}

/// Ground materials already describe their actual surface. Only living turf
/// follows the continuous vegetation palette shared with the surrounding plants.
pub(crate) struct GroundTint {
    grass: GrassTint,
}

impl GroundTint {
    pub fn authored_grass_at(&self, position: [f32; 3]) -> [f32; 3] {
        self.grass.relative_at(position)
    }

    pub fn cave_grass_at(&self, position: [f32; 3]) -> [f32; 3] {
        self.grass.at(position)
    }

    pub fn new(terrain: &terra_world::TerraGenerator, origin: [i64; 3]) -> Self {
        Self {
            grass: GrassTint::new(terrain, origin),
        }
    }

    pub fn at(&self, position: [f32; 3], material: u16) -> [f32; 3] {
        match material {
            1 => self.grass.at(position),
            306 | 400 => self.authored_grass_at(position),
            632 | 633 | 670 => {
                let reference = if material == 670 { 12 } else { 11 };
                terra_core::relative_grass_tint(
                    self.grass.at(position),
                    LINEAR_COLORS[reference][..3].try_into().unwrap(),
                    bilinear(self.grass.1, position),
                )
            }
            _ => [1.0; 3],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cave_grass_below_sea_level_keeps_its_vegetation_color_instead_of_becoming_a_sandy_bank() {
        let tint = GroundTint {
            grass: GrassTint([[0.25, 0.62, 0.12]; 4], [0.0; 4]),
        };
        assert_eq!(tint.cave_grass_at([8.0, 4.0, 8.0]), [0.25, 0.62, 0.12]);
        assert_eq!(
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
        let a = GrassTint([[0.1; 3], [0.3; 3], [0.5; 3], [0.7; 3]], [0.0; 4]);
        let b = GrassTint([[0.3; 3], [0.9; 3], [0.7; 3], [0.2; 3]], [0.0; 4]);
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
                assert_eq!(plants.relative_at(point), ground.grass.relative_at(point));
            }
        }
        let forest = GrassTint([LINEAR_COLORS[2][..3].try_into().unwrap(); 4], [0.0; 4]);
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
    fn blossom_forests_recolor_painted_green_turf_without_changing_other_biomes() {
        let painted_turf = [0.28, 0.46, 0.08];
        for (index, blossom) in [(2, 0.0), (7, 1.0), (8, 1.0), (9, 1.0)] {
            let tint = GrassTint(
                [LINEAR_COLORS[index][..3].try_into().unwrap(); 4],
                [blossom; 4],
            )
            .relative_at([8.0, 1.0, 8.0]);
            let color = std::array::from_fn::<_, 3, _>(|axis| painted_turf[axis] * tint[axis]);
            match index {
                2 => assert_eq!(color, painted_turf),
                7 => {
                    assert!(color[1] > color[0] * 0.78, "sage orchard meadow: {color:?}");
                    assert!(color[1] > color[2] * 1.8, "orchard grass blue: {color:?}");
                }
                8 => {
                    assert!(
                        color[1] / color[0] < 1.30 && color[1] / color[2] < 1.55,
                        "muted Spirit moss: {color:?}"
                    );
                }
                _ => {
                    assert!(color[0] > color[1] * 2.5, "crimson red: {color:?}");
                    assert!(color[0] > color[2] * 3.0, "crimson blue: {color:?}");
                    let luminance = 0.2126 * color[0] + 0.7152 * color[1] + 0.0722 * color[2];
                    assert!(
                        luminance >= 0.20,
                        "Canopy turf retains visible relief: {color:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn grass_blends_toward_the_adjacent_dune_and_blood_forest_before_the_block_changes() {
        use terra_world::generation::biomes::BiomeWeights;

        let sample = |desert: f64, blood: f64| BiomeSample {
            temperature: 0.7,
            humidity: 0.35,
            weights: BiomeWeights {
                forest: 1. - desert - blood,
                desert,
                blood_forest: blood,
                ocean: 0.,
                plains: 0.,
                highland: 0.,
                mushroom_park: 0.,
                cliffted_desert: 0.,
                cherry_forest: 0.,
                spirit_blossom: 0.,
                ashlands: 0.,
                savanna: 0.,
                swamp: 0.,
                petrified_forest: 0.0,
                crystal_highlands: 0.0,
                misty_coast: 0.0,
            },
        };
        let ratio = |dry, blood, sediment| {
            let (color, _) = turf_palette(sample(dry, blood), sediment);
            color[0] / color[1]
        };
        for sediment in [0., 1.] {
            let ratios = [0.35, 0.48, 0.60, 0.72].map(|dry| ratio(dry, 0., sediment));
            assert!(ratios.windows(2).all(|pair| pair[0] < pair[1]));
        }
        assert!(ratio(0.72, 0., 1.) > ratio(0.72, 0., 0.) * 1.5);
        let crimson = [0.10, 0.32, 0.51, 0.70].map(|blood| ratio(0., blood, 0.));
        assert!(crimson.windows(2).all(|pair| pair[0] < pair[1]));
    }

    #[test]
    fn dry_biome_borders_color_turf_and_keep_the_native_albedo_of_exposed_ground() {
        let tint = GroundTint {
            grass: GrassTint([[0.36, 0.30, 0.14]; 4], [0.5; 4]),
        };
        let point = [8., 8., 8.];
        for material in [4, 5, 401, 407, 408, 492, 493, 527, 528, 596, 671, 672] {
            assert_eq!(tint.at(point, material), [1.0; 3]);
        }
        assert_eq!(tint.at(point, 1), [0.36, 0.30, 0.14]);
        assert_eq!(tint.at(point, 400), tint.grass.relative_at(point));
    }

    #[test]
    fn inland_building_blocks_keep_color_and_narrow_riverbeds_stay_sandy() {
        let tint = GroundTint {
            grass: GrassTint([[0.2, 0.6, 0.1]; 4], [0.0; 4]),
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
        grass: GrassTint([LINEAR_COLORS[2][..3].try_into().unwrap(); 4], [0.0; 4]),
    };
    assert_eq!(tint.authored_grass_at([8.0, 0.0, 8.0]), [1.0; 3]);
    for biome in LINEAR_COLORS.iter() {
        let sample = GroundTint {
            grass: GrassTint([biome[..3].try_into().unwrap(); 4], [0.0; 4]),
        };
        assert!(
            sample
                .authored_grass_at([0.0; 3])
                .into_iter()
                .all(|v| (0.65..=1.6).contains(&v))
        );
    }
}
