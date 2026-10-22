use bevy::{mesh::MeshVertexAttribute, render::render_resource::VertexFormat};
use terra_world::{TerraGenerator, generation::biomes::BiomeSample};

/// Linear scattering colors. Ocean, plains and forest retain the original tint.
pub(crate) const DEFAULT_TINT: [f32; 3] = [0.025, 0.20, 0.16];
pub(crate) const COLORS: [[f32; 3]; 6] = [
    DEFAULT_TINT,
    DEFAULT_TINT,
    DEFAULT_TINT,
    [0.05, 0.28, 0.42],
    [0.015, 0.065, 0.16],
    [0.27, 0.15, 0.38],
];

/// COLOR and both UV sets already carry depth, current and waterfall metadata.
pub(crate) const ATTRIBUTE_BIOME_TINT: MeshVertexAttribute =
    MeshVertexAttribute::new("Water_BiomeTint", 1_883_214_071, VertexFormat::Float32x3);

pub(super) fn biome_tint(sample: BiomeSample) -> [f32; 3] {
    let mut weights = sample.weights.values();
    weights[3] += weights[6];
    std::array::from_fn(|channel| {
        weights
            .into_iter()
            .zip(COLORS)
            .map(|(weight, color)| weight as f32 * color[channel])
            .sum()
    })
}

/// Shared world-space corners keep adjacent sections, including negative ones,
/// continuous while requiring only four biome queries per uploaded water batch.
pub(crate) struct WaterTint([[f32; 3]; 4], [f32; 4]);

impl WaterTint {
    pub(crate) fn new(terrain: &TerraGenerator, origin: [i64; 3]) -> Self {
        let samples = std::array::from_fn::<_, 4, _>(|corner| {
            terrain.biome_at(
                (origin[0] + (corner % 2) as i64 * 16) as f64,
                (origin[2] + (corner / 2) as i64 * 16) as f64,
            )
        });
        Self(
            samples.map(biome_tint),
            samples.map(|sample| sample.weights.ocean as f32),
        )
    }

    pub(crate) fn ocean_at(&self, point: [f32; 3]) -> f32 {
        let x = (point[0] / 16.0).clamp(0.0, 1.0);
        let z = (point[2] / 16.0).clamp(0.0, 1.0);
        let near = self.1[0] * (1.0 - x) + self.1[1] * x;
        let far = self.1[2] * (1.0 - x) + self.1[3] * x;
        near * (1.0 - z) + far * z
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
        assert!(COLORS[5][0] > COLORS[5][1] && COLORS[5][2] > COLORS[5][1]);
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
                    assert_eq!(a.at([16.0, 4.0, p]), b.at([0.0, 9.0, p]));
                    assert_eq!(a.at([p, 4.0, 16.0]), c.at([p, 9.0, 0.0]));
                    assert_eq!(a.at([p, 0.0, p]), below.at([p, 16.0, p]));
                }
            }
        }
    }

    #[test]
    fn interpolation_is_smooth_and_bounded() {
        let tint = WaterTint([COLORS[1], COLORS[3], COLORS[4], COLORS[5]], [0.0; 4]);
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
