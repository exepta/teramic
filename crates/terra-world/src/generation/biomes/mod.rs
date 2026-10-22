mod climate;
mod profile;
#[cfg(test)]
mod tests;

pub(super) use climate::ClimateSampler;
pub use profile::{BiomeKind, BiomeSample, BiomeWeights, TerrainProfile};

pub(super) fn smoothstep(low: f64, high: f64, value: f64) -> f64 {
    let t = ((value - low) / (high - low)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
