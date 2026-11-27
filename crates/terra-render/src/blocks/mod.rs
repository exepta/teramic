#[cfg(test)]
mod color_tests;
pub(crate) mod cube;
pub mod definitions;
mod loading;
pub(crate) mod sidewalk;
#[cfg(test)]
pub(crate) use loading::test_static_scene;
pub(crate) mod mipmaps;
mod sand;
pub(crate) mod sculpt;
pub(crate) mod surface;
mod templates;
#[cfg(test)]
mod tests;
mod textures;

pub(crate) use cube::cube_faces;
#[cfg(test)]
pub(crate) use loading::authored_terrain_tests::templates as authored_terrain_templates;
pub(crate) use loading::{BlockAssets, load_blocks, poll_blocks};
pub(crate) use templates::*;
#[cfg(test)]
mod alchemy_tests;
#[cfg(test)]
mod castle_tests;
