use bevy::{mesh::MeshVertexAttribute, render::render_resource::VertexFormat};
use terra_world::{TerraGenerator, generation::biomes::BiomeSample};

/// Linear scattering colors. Ocean, plains and forest retain the original tint.
pub(crate) const DEFAULT_TINT: [f32; 3] = [0.025, 0.20, 0.16];
/// Clear brine receives its light from the chamber rather than the surface forest.
pub(crate) const SALT_POOL_TINT: [f32; 3] = [0.009, 0.092, 0.135];
pub(crate) fn pool_tint(cave: terra_world::generation::CaveStructure, y: f64) -> Option<[f32; 3]> {
    use terra_world::generation::CaveEcology;
    match cave.pool_ecology_at(y)? {
        CaveEcology::Salt => Some(SALT_POOL_TINT),
        CaveEcology::Lush => Some([0.012, 0.24, 0.205]),
        CaveEcology::Waterfall => Some([0.022, 0.17, 0.33]),
        _ => None,
    }
}

pub(crate) const COAST_TINT: [f32; 3] = [0.048, 0.20, 0.21];
pub(crate) const GLACIAL_TINT: [f32; 3] = [0.030, 0.235, 0.295];
pub(crate) const PETRIFIED_TINT: [f32; 3] = [0.12, 0.18, 0.215];
pub(crate) const SWAMP_TINT: [f32; 3] = [0.15, 0.117, 0.045];
pub(crate) const SAVANNA_TINT: [f32; 3] = [0.026, 0.32, 0.29];
pub(crate) const THERMAL_TINT: [f32; 3] = [0.036, 0.50, 0.47];
pub(crate) const ASHLANDS_TINT: [f32; 3] = [0.075, 0.11, 0.12];
pub(crate) const CHERRY_TINT: [f32; 3] = [0.035, 0.34, 0.36];
pub(crate) const SPIRIT_TINT: [f32; 3] = [0.14, 0.62, 0.71];
pub(crate) const BLOOD_TINT: [f32; 3] = [0.51, 0.041, 0.09];
pub(crate) const COLORS: [[f32; 3]; 6] = [
    DEFAULT_TINT,
    DEFAULT_TINT,
    DEFAULT_TINT,
    [0.05, 0.28, 0.42],
    [0.015, 0.065, 0.16],
    [0.045, 0.34, 0.32],
];

/// RGB scattering plus explicit Spirit glow weight. COLOR and both UV sets
/// already carry depth, current and waterfall metadata.
pub(crate) const ATTRIBUTE_BIOME_TINT: MeshVertexAttribute =
    MeshVertexAttribute::new("Water_BiomeTint", 1_883_214_071, VertexFormat::Float32x4);
pub(crate) const ATTRIBUTE_MEDIUM_WEIGHTS: MeshVertexAttribute = MeshVertexAttribute::new(
    "Water_MediumWeights",
    1_883_214_073,
    VertexFormat::Float32x2,
);

pub(super) fn biome_tint(sample: BiomeSample) -> [f32; 3] {
    let mut weights = sample.weights.values();
    weights[3] += weights[6];
    std::array::from_fn(|channel| {
        let original: f32 = weights
            .into_iter()
            .zip(COLORS)
            .map(|(weight, color)| weight as f32 * color[channel])
            .sum();
        original
            + weights[7] as f32 * CHERRY_TINT[channel]
            + weights[8] as f32 * SPIRIT_TINT[channel]
            + weights[9] as f32 * BLOOD_TINT[channel]
            + weights[10] as f32 * ASHLANDS_TINT[channel]
            + weights[11] as f32 * SAVANNA_TINT[channel]
            + weights[12] as f32 * SWAMP_TINT[channel]
            + weights[13] as f32 * PETRIFIED_TINT[channel]
            + weights[14] as f32 * GLACIAL_TINT[channel]
            + weights[15] as f32 * COAST_TINT[channel]
    })
}

/// The same optical palette is used above and below the waterline. The
/// continuous temperature ramp avoids stripes where marine habitats meet.
pub(super) fn habitat_tint(sample: BiomeSample, marine: bool) -> [f32; 3] {
    let base = biome_tint(sample);
    if !marine || sample.weights.ocean <= 0.0 {
        return base;
    }
    let smooth = |low: f64, high: f64| {
        let t = ((sample.temperature - low) / (high - low)).clamp(0., 1.) as f32;
        t * t * (3. - 2. * t)
    };
    let temperate = smooth(0.28, 0.43);
    let tropical = smooth(0.50, 0.68);
    let cold = [0.011, 0.109, 0.275];
    let normal = [0.016, 0.225, 0.335];
    let warm = [0.027, 0.46, 0.42];
    std::array::from_fn(|i| {
        let marine =
            (cold[i] + (normal[i] - cold[i]) * temperate) * (1. - tropical) + warm[i] * tropical;
        base[i] + (marine - DEFAULT_TINT[i]) * sample.weights.ocean as f32
    })
}

/// Shared world-space corners keep adjacent sections, including negative ones,
/// continuous while requiring only four biome queries per uploaded water batch.
pub(crate) struct WaterTint([[f32; 3]; 4], [f32; 4], [f32; 4], [[f32; 2]; 4]);

impl WaterTint {
    pub(crate) fn new(terrain: &TerraGenerator, origin: [i64; 3]) -> Self {
        let samples = std::array::from_fn::<_, 4, _>(|corner| {
            terrain.biome_at(
                (origin[0] + (corner % 2) as i64 * 16) as f64,
                (origin[2] + (corner / 2) as i64 * 16) as f64,
            )
        });
        Self(
            samples.map(|sample| habitat_tint(sample, terrain.marine_overhaul())),
            samples.map(|sample| sample.weights.ocean as f32),
            samples.map(|sample| sample.weights.spirit_blossom as f32),
            samples.map(|sample| {
                [
                    sample.weights.blood_forest as f32,
                    sample.weights.swamp as f32,
                ]
            }),
        )
    }

    pub(crate) fn optics(
        &self,
        origin: [i64; 3],
        point: [f32; 3],
        column: Option<&terra_world::PreparedColumn>,
    ) -> ([f32; 4], [f32; 2]) {
        let x = origin[0] + point[0].clamp(0., 15.) as i64;
        let z = origin[2] + point[2].clamp(0., 15.) as i64;
        let y = origin[1] as f64 + f64::from(point[1]);
        let world = [
            origin[0] as f64 + f64::from(point[0]),
            y,
            origin[2] as f64 + f64::from(point[2]),
        ];
        if column.is_some_and(|c| c.geothermal_water(world)) {
            let [r, g, b] = THERMAL_TINT;
            return ([r, g, b, 0.], [0.; 2]);
        }
        let surface = column.and_then(|c| c.surface_at(x, z));
        if let Some([r, g, b]) = surface.and_then(|c| pool_tint(c.cave, y)) {
            // Negative spirit weight selects the chamber's water-light response.
            ([r, g, b, -1.0], [0.; 2])
        } else {
            let mut weights = self.medium_at(point);
            if let Some(c) = surface.filter(|c| c.water_level.is_some_and(|h| (h - y).abs() < 1.)) {
                weights[1] = weights[1].max(c.ocean.swamp);
            }
            (self.payload(point), weights)
        }
    }

    pub(crate) fn ocean_at(&self, point: [f32; 3]) -> f32 {
        let x = (point[0] / 16.0).clamp(0.0, 1.0);
        let z = (point[2] / 16.0).clamp(0.0, 1.0);
        let near = self.1[0] * (1.0 - x) + self.1[1] * x;
        let far = self.1[2] * (1.0 - x) + self.1[3] * x;
        near * (1.0 - z) + far * z
    }

    /// Interpolate blood viscosity and suspended swamp sediment independently.
    pub(crate) fn medium_at(&self, point: [f32; 3]) -> [f32; 2] {
        let x = (point[0] / 16.).clamp(0., 1.);
        let z = (point[2] / 16.).clamp(0., 1.);
        std::array::from_fn(|i| {
            (self.3[0][i] * (1. - x) + self.3[1][i] * x) * (1. - z)
                + (self.3[2][i] * (1. - x) + self.3[3][i] * x) * z
        })
    }

    /// Explicit Spirit weight avoids guessing emission from a mixed RGB hue.
    pub(crate) fn payload(&self, point: [f32; 3]) -> [f32; 4] {
        let color = self.at(point);
        let x = (point[0] / 16.).clamp(0., 1.);
        let z = (point[2] / 16.).clamp(0., 1.);
        let weight = (self.2[0] * (1. - x) + self.2[1] * x) * (1. - z)
            + (self.2[2] * (1. - x) + self.2[3] * x) * z;
        [color[0], color[1], color[2], weight]
    }
    pub(crate) fn at(&self, point: [f32; 3]) -> [f32; 3] {
        let x = (point[0] / 16.0).clamp(0.0, 1.0);
        let z = (point[2] / 16.0).clamp(0.0, 1.0);
        std::array::from_fn(|channel| {
            let front = self.0[0][channel] * (1.0 - x) + self.0[1][channel] * x;
            let back = self.0[2][channel] * (1.0 - x) + self.0[3][channel] * x;
            front * (1.0 - z) + back * z
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_world::generation::biomes::BiomeWeights;

    fn sample(weights: [f64; 6]) -> BiomeSample {
        BiomeSample {
            temperature: 0.5,
            humidity: 0.5,
            weights: BiomeWeights {
                ocean: weights[0],
                plains: weights[1],
                forest: weights[2],
                desert: weights[3],
                highland: weights[4],
                mushroom_park: weights[5],
                cliffted_desert: 0.0,
                cherry_forest: 0.0,
                spirit_blossom: 0.0,
                blood_forest: 0.0,
                ashlands: 0.0,
                savanna: 0.0,
                swamp: 0.0,
                petrified_forest: 0.0,
                crystal_highlands: 0.0,
                misty_coast: 0.0,
            },
        }
    }

    #[test]
    fn biome_palette_preserves_lowlands_and_blends_special_biomes() {
        for (index, expected) in COLORS.into_iter().enumerate() {
            let mut weights = [0.0; 6];
            weights[index] = 1.0;
            assert_eq!(biome_tint(sample(weights)), expected);
        }
        let mixed = biome_tint(sample([0.0, 0.25, 0.0, 0.25, 0.0, 0.5]));
        for (channel, value) in mixed.into_iter().enumerate() {
            assert!(
                (value
                    - (DEFAULT_TINT[channel] * 0.25
                        + COLORS[3][channel] * 0.25
                        + COLORS[5][channel] * 0.5))
                    .abs()
                    < 1e-6
            );
        }
        assert!(COLORS[3][2] > DEFAULT_TINT[2]);
        assert!(COLORS[4][0] + COLORS[4][1] + COLORS[4][2] < DEFAULT_TINT.into_iter().sum::<f32>());
        assert!(COLORS[5][1] > COLORS[5][0] && COLORS[5][2] > COLORS[5][0]);
    }

    #[test]
    fn mineral_scattering_is_confined_to_the_generated_thermal_basin() {
        let terrain = TerraGenerator::new(Default::default()).unwrap();
        let generator = terra_world::SectionGenerator::new(terrain.clone());
        let bed = [2386, 29, -4657];
        let (section, local) = terra_voxel::global_to_section(bed);
        let column = generator
            .prepare_column([section.x, section.z], false)
            .unwrap();
        let origin = section.origin().unwrap();
        let near = local.map(|n| n as f32 + 0.5);
        let palette = WaterTint::new(&terrain, origin);
        let (tint, weights) = palette.optics(origin, near, Some(&column));
        assert_eq!(
            tint,
            [THERMAL_TINT[0], THERMAL_TINT[1], THERMAL_TINT[2], 0.]
        );
        assert_eq!(weights, [0.; 2]);
        assert!(!column.geothermal_water([2386.5, 14., -4656.5]));
        assert!(!column.geothermal_water([2365.5, 29., -4656.5]));
    }

    #[test]
    fn spirit_water_blends_continuously_with_existing_biomes() {
        let mut spirit = sample([0.0; 6]);
        spirit.weights.spirit_blossom = 1.0;
        assert_eq!(biome_tint(spirit), SPIRIT_TINT);
        let mut previous = DEFAULT_TINT;
        for step in 0..=100 {
            let weight = f64::from(step) / 100.0;
            let mut mixed = sample([0.0, 0.0, 1.0 - weight, 0.0, 0.0, 0.0]);
            mixed.weights.spirit_blossom = weight;
            let color = biome_tint(mixed);
            for channel in 0..3 {
                let expected = DEFAULT_TINT[channel] * (1.0 - weight as f32)
                    + SPIRIT_TINT[channel] * weight as f32;
                assert!((color[channel] - expected).abs() < 1e-6);
                assert!((color[channel] - previous[channel]).abs() < 0.007);
            }
            previous = color;
        }
    }

    #[test]
    fn blood_color_and_viscosity_blend_independently_from_spirit_glow() {
        let mut red = sample([0.; 6]);
        red.weights.blood_forest = 1.0;
        let color = biome_tint(red);
        assert_eq!(color, BLOOD_TINT);
        // Readable carmine scattering under canopy fill, without Spirit emission.
        let luminance = color[0] * 0.2126 + color[1] * 0.7152 + color[2] * 0.0722;
        assert!((0.14..0.20).contains(&luminance));
        assert!(color[0] > color[2] && color[2] > color[1]);
        let tint = WaterTint(
            [BLOOD_TINT; 4],
            [0.; 4],
            [0.25; 4],
            [[0., 1.], [1., 0.], [0., 1.], [1., 0.]],
        );
        for x in 0..=16 {
            let point = [x as f32, 0., 7.];
            assert_eq!(tint.payload(point)[3], 0.25);
            assert_eq!(tint.medium_at(point)[1], 1. - x as f32 / 16.);
            assert!((tint.medium_at(point)[0] - x as f32 / 16.).abs() < 1e-6);
        }
        for weight in [0.0, 0.25, 0.5, 0.75, 1.0] {
            let mut mixed = sample([0., 0., 1. - weight, 0., 0., 0.]);
            mixed.weights.blood_forest = weight;
            let color = biome_tint(mixed);
            for i in 0..3 {
                assert!(
                    (color[i]
                        - (DEFAULT_TINT[i] * (1. - weight as f32) + BLOOD_TINT[i] * weight as f32))
                        .abs()
                        < 1e-6
                );
            }
        }
    }

    #[test]
    fn water_tint_matches_across_sections_and_vertical_falls() {
        let terrain = TerraGenerator::new(Default::default()).unwrap();
        for x in [-512, -32, -16, 0, 16, 496] {
            for z in [-64, -16, 0, 32] {
                let a = WaterTint::new(&terrain, [x, 0, z]);
                let b = WaterTint::new(&terrain, [x + 16, 0, z]);
                let c = WaterTint::new(&terrain, [x, 0, z + 16]);
                let below = WaterTint::new(&terrain, [x, -32, z]);
                for step in 0..=32 {
                    let p = step as f32 * 0.5;
                    assert_eq!(a.payload([16.0, 4.0, p]), b.payload([0.0, 9.0, p]));
                    assert_eq!(a.payload([p, 4.0, 16.0]), c.payload([p, 9.0, 0.0]));
                    assert_eq!(a.payload([p, 0.0, p]), below.payload([p, 16.0, p]));
                    assert_eq!(a.medium_at([16., 4., p]), b.medium_at([0., 9., p]));
                    assert_eq!(a.medium_at([p, 4., 16.]), c.medium_at([p, 9., 0.]));
                }
            }
        }
    }

    #[test]
    fn spirit_glow_weight_is_explicit_smooth_and_zero_for_other_biomes() {
        let tinted = WaterTint([DEFAULT_TINT; 4], [0.; 4], [0., 1., 0., 1.], [[0.; 2]; 4]);
        for x in 0..=16 {
            let value = tinted.payload([x as f32, 0., 7.]);
            assert!((value[3] - x as f32 / 16.).abs() < 1e-6);
        }
        let ordinary = WaterTint([CHERRY_TINT; 4], [0.; 4], [0.; 4], [[0.; 2]; 4]);
        assert_eq!(
            ordinary.payload([8., 0., 8.]),
            [CHERRY_TINT[0], CHERRY_TINT[1], CHERRY_TINT[2], 0.]
        );
        const {
            assert!(CHERRY_TINT[2] >= CHERRY_TINT[1] && CHERRY_TINT[1] > CHERRY_TINT[0]);
            assert!(SPIRIT_TINT[1] > SPIRIT_TINT[0] && SPIRIT_TINT[2] > SPIRIT_TINT[0]);
        }
    }

    #[test]
    fn cold_open_seas_and_tropical_lagoons_have_distinct_scattering() {
        let mut ocean = sample([1., 0., 0., 0., 0., 0.]);
        let colors = [0.12, 0.47, 0.81].map(|temp| {
            ocean.temperature = temp;
            habitat_tint(ocean, true)
        });
        assert!(colors[0][1] < colors[1][1] && colors[1][1] < colors[2][1]);
        assert!(colors[0][2] > colors[0][1] * 2.0 && colors[2][1] > colors[2][2]);
        assert_eq!(habitat_tint(ocean, false), DEFAULT_TINT);
    }
    #[test]
    fn interpolation_is_smooth_and_bounded() {
        let tint = WaterTint(
            [COLORS[1], COLORS[3], COLORS[4], COLORS[5]],
            [0.0; 4],
            [0.0; 4],
            [[0.0; 2]; 4],
        );
        let mut previous = tint.at([0.0, 1.0, 8.0]);
        for step in 1..=160 {
            let color = tint.at([step as f32 * 0.1, 1.0, 8.0]);
            for (channel, value) in color.into_iter().enumerate() {
                assert!((value - previous[channel]).abs() < 0.005);
                assert!((0.0..=1.0).contains(&value));
            }
            previous = color;
        }
    }
}
