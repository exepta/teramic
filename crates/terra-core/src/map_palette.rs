/// Renderer-derived linear albedos shared with cartography without a rendering
/// dependency in the world, storage or dedicated server crates.
#[cfg_attr(feature = "bevy", derive(bevy::prelude::Resource))]
#[derive(Clone, Debug, Default)]
pub struct MapPalette {
    /// Indexed by the existing block ID. Empty until block assets are ready.
    pub colors: Vec<[f32; 3]>,
    /// The renderer's grass tint, in `BiomeWeights::values()` order.
    pub biome_colors: [[f32; 3]; 8],
    /// The grass block uses the painted face-group model rather than neutral turf.
    pub authored_grass: bool,
    /// Authored upper-face albedo after its surface palette, before biome tint.
    /// Separate from `colors` so cave cartography keeps its existing albedos.
    pub surface_grass: Option<[f32; 3]>,
    /// The water shader's linear scattering tint, in the same biome order.
    pub water_colors: [[f32; 3]; 8],
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
        bytes(b"terra-map-albedo-2");
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
    pub fn authored_grass_tint(&self, color: [f32; 3]) -> [f32; 3] {
        std::array::from_fn(|axis| {
            (color[axis] / self.biome_colors[2][axis].max(f32::EPSILON)).clamp(0.65, 1.6)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authored_grass_preserves_forest_albedo_and_bounds_relative_biomes() {
        let palette = MapPalette {
            biome_colors: [[0.2, 0.5, 0.1]; 8],
            ..Default::default()
        };
        assert_eq!(palette.authored_grass_tint([0.2, 0.5, 0.1]), [1.0; 3]);
        assert_eq!(
            palette.authored_grass_tint([0.01, 0.5, 0.9]),
            [0.65, 1.0, 1.6]
        );
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
