mod appearance;
mod aquatic;
mod forms;
pub(in crate::generation) use appearance::{PlantedAppearanceSource, TreeAppearanceCache};
pub use appearance::{TreeAppearance, TreeAppearanceSnapshot};
mod geometry;
mod ground;
pub use ground::GroundCover;
pub(crate) use ground::understory_cover;
mod placement;
#[cfg(feature = "bevy")]
mod planted;
#[cfg(feature = "bevy")]
pub use planted::planted_shape;
mod species;
#[cfg(test)]
mod tests;
mod tree;
mod trunk_fungi;
pub(crate) use trunk_fungi::attached_mushroom;

pub(crate) use placement::{trees_in_column, trees_in_region};
pub(crate) use tree::Tree;

// Includes bent trunks, the longest branch and its outermost crown cluster.
pub(crate) const MAX_TREE_REACH: i64 = 16;
pub(crate) const MAX_TREE_HEIGHT: i64 = 36;
const DIAGONAL: f32 = std::f32::consts::FRAC_1_SQRT_2;
const DIRECTIONS: [[f32; 2]; 8] = [
    [1.0, 0.0],
    [DIAGONAL, DIAGONAL],
    [0.0, 1.0],
    [-DIAGONAL, DIAGONAL],
    [-1.0, 0.0],
    [-DIAGONAL, -DIAGONAL],
    [0.0, -1.0],
    [DIAGONAL, -DIAGONAL],
];

fn mix(mut value: u64) -> u64 {
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d049bb133111eb);
    value ^ (value >> 31)
}

fn random(seed: u64, channel: u64) -> f32 {
    (mix(seed.wrapping_add(channel.wrapping_mul(0x9e3779b97f4a7c15))) >> 40) as f32 / 16_777_216.0
}
