/// Renderer-derived linear albedos shared with cartography without a rendering
/// dependency in the world, storage or dedicated server crates.
#[cfg_attr(feature = "bevy", derive(bevy::prelude::Resource))]
#[derive(Clone, Debug, Default)]
pub struct MapPalette {
    /// Indexed by the existing block ID. Empty until block assets are ready.
    pub colors: Vec<[f32; 3]>,
    /// The renderer's grass tint, in `BiomeWeights::values()` order.
    pub biome_colors: [[f32; 3]; 16],
    /// The grass block uses the painted face-group model rather than neutral turf.
    pub authored_grass: bool,
    /// Authored upper-face albedo after its surface palette, before biome tint.
    /// Separate from `colors` so cave cartography keeps its existing albedos.
    pub surface_grass: Option<[f32; 3]>,
    /// The water shader's linear scattering tint, in the same biome order.
    pub water_colors: [[f32; 3]; 16],
    pub revision: u64,
}

impl MapPalette {
    /// Persistent albedo identity, independent of the process-local revision.
    /// Bump the raster tag when the cartographic albedo algorithm changes.
    pub fn fingerprint(&self) -> u64 {
        let mut hash = 0xcbf29ce484222325_u64;
        let mut bytes = |bytes: &[u8]| {
            for byte in bytes {
                hash = (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3);
            }
        };
        bytes(b"terra-map-albedo-6");
        bytes(&(self.colors.len() as u64).to_le_bytes());
        for color in self
            .colors
            .iter()
            .chain(self.biome_colors.iter())
            .chain(self.water_colors.iter())
        {
            for value in color {
                bytes(&value.to_bits().to_le_bytes());
            }
        }
        bytes(&[
            u8::from(self.authored_grass),
            u8::from(self.surface_grass.is_some()),
        ]);
        if let Some(color) = self.surface_grass {
            for value in color {
                bytes(&value.to_bits().to_le_bytes());
            }
        }
        hash
    }

    /// Painted grass already supplies the forest albedo; other biomes are relative.
    pub fn authored_grass_tint(&self, color: [f32; 3], blossom_weight: f32) -> [f32; 3] {
        relative_grass_tint(color, self.biome_colors[2], blossom_weight)
    }
}

pub const DUNE_ALBEDO: [f32; 3] = [0.7729, 0.5007, 0.2145];
pub const RED_DUNE_ALBEDO: [f32; 3] = [0.4714, 0.1579, 0.0606];

/// Linear mean top albedos of regionally painted soil. These are measured
/// from the shipped tiles; relief and individual grains remain in the texture.
pub fn regional_soil_albedo(material: u16) -> Option<[f32; 3]> {
    Some(match material {
        401 => [0.2057, 0.0872, 0.0963],
        407 => [0.2447, 0.0538, 0.0680],
        408 => [0.2445, 0.0474, 0.0714],
        492 => [0.1899, 0.0644, 0.0590],
        493 => [0.1331, 0.0155, 0.0250],
        505 => [0.1531, 0.2040, 0.0543],
        519 => [0.1138, 0.1009, 0.0276],
        520 => [0.2098, 0.1422, 0.0651],
        527 => DUNE_ALBEDO,
        528 => RED_DUNE_ALBEDO,
        596 => [0.2674, 0.3084, 0.1171],
        _ => return None,
    })
}

/// Mixed biomes share their local palette. Single-biome interiors retain the
/// authored variation of their separate surface materials.
pub fn biome_edge_strength(dominant: f32) -> f32 {
    let t = ((0.985 - dominant) / 0.21).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Preserve the established green-biome palette, while allowing the blossom
/// forests to shift painted green turf into sage, silver moss and crimson. The weight is
/// interpolated with the climate field, so neither section nor biome boundaries
/// introduce a hue step. Multipliers stay bounded for saturated colors.
pub fn relative_grass_tint(color: [f32; 3], forest: [f32; 3], blossom_weight: f32) -> [f32; 3] {
    let blend = blossom_weight.clamp(0.0, 1.0);
    std::array::from_fn(|axis| {
        let relative = color[axis] / forest[axis].max(f32::EPSILON);
        let ordinary = relative.clamp(0.65, 1.6);
        ordinary + (relative.clamp(0.25, 8.0) - ordinary) * blend
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authored_grass_preserves_forest_albedo_and_bounds_relative_biomes() {
        let palette = MapPalette {
            biome_colors: [[0.2, 0.5, 0.1]; 16],
            ..Default::default()
        };
        assert_eq!(palette.authored_grass_tint([0.2, 0.5, 0.1], 0.0), [1.0; 3]);
        assert_eq!(
            palette.authored_grass_tint([0.01, 0.5, 0.9], 0.0),
            [0.65, 1.0, 1.6]
        );
    }

    #[test]
    fn blossom_tints_extend_hue_range_only_inside_their_smooth_weight() {
        let forest = [0.20, 0.54, 0.09];
        let lilac = [0.38, 0.41, 0.60];
        let ordinary = relative_grass_tint(lilac, forest, 0.0);
        let blossom = relative_grass_tint(lilac, forest, 1.0);
        assert_eq!(ordinary[2], 1.6);
        assert!(blossom[2] > 6.0);
        for step in 0..=10 {
            let weight = step as f32 / 10.0;
            let tint = relative_grass_tint(lilac, forest, weight);
            for axis in 0..3 {
                let expected = ordinary[axis] + (blossom[axis] - ordinary[axis]) * weight;
                assert!((tint[axis] - expected).abs() < 1e-6);
                assert!((0.25..=8.0).contains(&tint[axis]));
            }
        }
    }

    #[test]
    fn persistent_palette_identity_tracks_albedos_not_runtime_revisions() {
        let palette = MapPalette {
            colors: vec![[0.1, 0.2, 0.3]],
            surface_grass: Some([0.3, 0.5, 0.1]),
            ..Default::default()
        };
        let fingerprint = palette.fingerprint();
        let mut changed = palette.clone();
        changed.revision = 917;
        assert_eq!(changed.fingerprint(), fingerprint);
        for field in 0..5 {
            let mut changed = palette.clone();
            match field {
                0 => changed.colors[0][0] += 0.01,
                1 => changed.biome_colors[2][1] += 0.01,
                2 => changed.water_colors[0][2] += 0.01,
                3 => changed.authored_grass = true,
                _ => changed.surface_grass = None,
            }
            assert_ne!(changed.fingerprint(), fingerprint);
        }
    }
}
