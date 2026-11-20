//! Revision five bounds individual wet channels to 6–22 world blocks. The
//! safety margins cover the rounded voxel floor and oblique voxel silhouettes;
//! they are not an assertion that twice RiverSample::width is the water width.
use super::RiverSample;
use crate::generation::biomes::smoothstep;

pub(super) const MIN_HALF_WIDTH: f64 = 4.8;
pub(super) const MAX_HALF_WIDTH: f64 = 10.25;

pub(super) fn clamp_half_width(width: f64) -> f64 {
    width.clamp(MIN_HALF_WIDTH, MAX_HALF_WIDTH)
}

pub(super) fn bed(river: RiverSample) -> f64 {
    let across = river.distance / river.width;
    // A genuinely wet central shelf remains at least one whole voxel below
    // every fractional water level. Rounded shoulders rise before the bank.
    // The old shallow parabola could leave only 1–3 wet blocks in a brook.
    let shore = smoothstep(0.65, 1.0, across);
    let detail = river.bed_detail * (1.0 - across.min(1.0)).powi(2);
    river.level - river.depth + (river.depth + 0.65) * shore + detail
}

#[cfg(test)]
mod tests;
