/// Renderer-derived linear albedos shared with cartography without a rendering
/// dependency in the world, storage or dedicated server crates.
#[cfg_attr(feature = "bevy", derive(bevy::prelude::Resource))]
#[derive(Clone, Debug, Default)]
pub struct MapPalette {
    /// Indexed by the existing block ID. Empty until block assets are ready.
    pub colors: Vec<[f32; 3]>,
    /// The renderer's grass tint, in `BiomeWeights::values()` order.
    pub biome_colors: [[f32; 3]; 7],
    /// The water shader's linear scattering tint, in the same biome order.
    pub water_colors: [[f32; 3]; 7],
    pub revision: u64,
}
