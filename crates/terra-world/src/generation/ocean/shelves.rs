//! Stepped marine shelves and widely separated, bedrock-supported sea stacks.
use super::{BiomeSample, NoiseFn, TerraGenerator, smoothstep};
use crate::generation::seed::layer_seed;

#[derive(Clone, Copy)]
struct SeaStack {
    /// Elliptical distance from the summit, in units of the apron radius.
    radius: f64,
    side: f64,
    along: f64,
    crest: f64,
    arch: bool,
}

impl TerraGenerator {
    pub fn marine_overhaul(&self) -> bool {
        self.config().ecology_revision >= 15
            && self.config().landscape_revision >= 9
            && self.config().ocean_revision > 0
            && self.config().biomes
            && self.config().water
    }

    pub(in crate::generation) fn marine_height(
        &self,
        x: f64,
        z: f64,
        biome: BiomeSample,
        height: f64,
    ) -> f64 {
        let weight = biome.weights.ocean;
        if !self.marine_overhaul() || weight <= 0.40 {
            return height;
        }
        let sea = self.config().sea_level;
        let depth = sea - height;
        if depth <= 0.0 {
            return height;
        }
        let blend = smoothstep(0.40, 0.78, weight) * smoothstep(0., 8., depth);
        let field = self.valleys.get([x / 75.0 + 14.8, z / 62.0 - 41.1]);
        let shelves = smoothstep(-0.13, 0.01, field) * 3.2
            + smoothstep(0.13, 0.24, field) * 4.4
            + smoothstep(0.34, 0.43, field) * 3.8;
        let warmth = smoothstep(0.46, 0.68, biome.temperature);
        let shelf = height
            + blend
                * ((shelves * (0.64 + warmth * 0.46) - 2.4).min((depth - 1.6).max(0.0))
                    + warmth * smoothstep(18.0, 62.0, depth) * (depth * 0.42));
        let Some(stack) = sea_stack(self.config().seed.0, x, z) else {
            return shelf;
        };
        // Windward ribs break up the elliptical apron and the cliff line.
        // Taper their displacement to zero at its edge to join the open bed.
        let outcrop = self.hills.get([x / 9.0 + 58.3, z / 12.0 - 76.2]);
        let outline = stack.radius + 0.25 * outcrop * (1.0 - stack.radius);
        let rim = 1.0 - smoothstep(0.26, 0.98, outline);
        let cliff = smoothstep(0.44, 0.84, rim);
        let summit = sea + stack.crest + (1.0 - stack.radius) * 3.0 + outcrop * 4.0;
        shelf
            + (summit - shelf).max(0.0)
                * (0.27 * rim + 0.73 * cliff)
                * smoothstep(0.58, 0.84, weight)
    }

    pub(in crate::generation) fn stack_opening(
        &self,
        x: f64,
        z: f64,
        biome: BiomeSample,
        height: f64,
    ) -> Option<[f64; 2]> {
        let sea = self.config().sea_level;
        if !self.marine_overhaul() || biome.weights.ocean < 0.7 || height < sea - 1.0 {
            return None;
        }
        let stack = sea_stack(self.config().seed.0, x, z)?;
        if !stack.arch || stack.along.abs() > 24.0 {
            return None;
        }
        let half_span = 6.4;
        let curve = 1.0 - (stack.side / half_span).powi(2);
        if curve <= 0.0 {
            return None;
        }
        Some([sea - 4.0, sea + curve.sqrt() * 5.4])
    }
}

/// Each 512m district rolls once. Even a selected district contains just one
/// isolated footprint. Centres have room for the full apron on every side.
fn sea_stack(seed: u64, x: f64, z: f64) -> Option<SeaStack> {
    const PERIOD: f64 = 512.0;
    let ix = (x / PERIOD).floor() as i64;
    let iz = (z / PERIOD).floor() as i64;
    let hash = layer_seed(
        seed ^ (ix as u64).wrapping_mul(0x9e3779b97f4a7c15),
        iz as u64,
    );
    if !hash.is_multiple_of(7) {
        return None;
    }
    let cx = (ix as f64) * PERIOD + 78.0 + f64::from((hash >> 5) & 255) * 1.38;
    let cz = (iz as f64) * PERIOD + 78.0 + f64::from((hash >> 14) & 255) * 1.38;
    let (sx, sz) = (x - cx, z - cz);
    let (side, along) = if hash & 2 == 0 { (sx, sz) } else { (sz, sx) };
    let radius = (side / (31.0 + f64::from((hash >> 22) & 7)))
        .hypot(along / (21.0 + f64::from((hash >> 25) & 7)));
    (radius < 1.0).then_some(SeaStack {
        radius,
        side,
        along,
        crest: 8.0 + f64::from((hash >> 6) & 7),
        arch: hash & 1 == 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sea_stack_has_a_navigable_tidal_arch_and_preserves_the_legacy_seabed() {
        use crate::generation::{SectionGenerator, TerrainGenerator};
        use terra_core::{BiomeDefinitions, WorldConfig, WorldSeed};
        use terra_voxel::MaterialId;
        let config = WorldConfig {
            seed: WorldSeed(42042),
            ecology_revision: 15,
            biome_definitions: Some(
                BiomeDefinitions::load_directory(terra_core::asset_directory().join("biome"))
                    .unwrap(),
            ),
            ..Default::default()
        };
        let old = TerraGenerator::new(WorldConfig {
            ecology_revision: 14,
            ..config.clone()
        })
        .unwrap();
        let new = TerraGenerator::new(config).unwrap();
        let [x, z] = [2219, 130];
        let center = new.surface_column(x as f64 + 0.5, z as f64 + 0.5);
        let sea = new.config().sea_level;
        assert!(old.surface_height(x as f64 + 0.5, z as f64 + 0.5) < sea - 50.);
        assert!(
            center.height > sea + 10.
                && center.water_bed() < sea
                && center.water_level == Some(sea)
        );
        let section = SectionGenerator::new(new.clone());
        for dx in -25..=25 {
            for y in [sea as i64 - 1, sea as i64 + 1] {
                let cell = [x + dx, y, z];
                let m = section.block_at(cell).unwrap();
                assert!(
                    !MaterialId::from_raw(m).unwrap().is_solid(),
                    "blocked tidal passage at {cell:?}"
                );
            }
        }
        let roof = [x, sea as i64 + 8, z];
        assert!(
            MaterialId::from_raw(section.block_at(roof).unwrap())
                .unwrap()
                .is_solid()
        );
        assert_eq!(
            section.block_at(roof).unwrap(),
            new.sample(roof.map(|v| v as f64 + 0.5)).material
        );
    }

    #[test]
    fn sea_stack_centres_are_seeded_sparse_and_openings_fit_below_the_crest() {
        let mut footprint = 0;
        let mut arches = 0;
        for iz in -8..8 {
            for ix in -8..8 {
                for dz in (0..512).step_by(12) {
                    for dx in (0..512).step_by(12) {
                        if let Some(stack) =
                            sea_stack(157, (ix * 512 + dx) as f64, (iz * 512 + dz) as f64)
                        {
                            footprint += 1;
                            if stack.arch && stack.radius < 0.3 {
                                arches += 1;
                                assert!(stack.crest > 5.4);
                            }
                        }
                    }
                }
            }
        }
        let total = 16 * 16 * 43 * 43;
        assert!(footprint > 100 && footprint < total / 100);
        assert!(arches > 0);
    }
}
