//! CPU-only canonical plant geometry shared by section rendering and picking.
mod aquatic;
mod botanical_detail;
mod botany;
mod crops;
mod fern;
mod flowers;
mod fruit;
mod geometry;
mod leaf;
mod mushrooms;
mod saplings;
mod scarecrow;
pub use fruit::apple;
mod shapes;
mod sizing;
mod trunk_fungi;
mod webs;
pub use shapes::template;
pub(crate) use sizing::bounds as template_bounds;
#[cfg(test)]
mod tests;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PropDetail {
    Near,
    Middle,
    Far,
}

#[derive(Default, Clone, Debug, PartialEq)]
pub struct PropMesh {
    pub material: usize,
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub colors: Vec<[f32; 4]>,
    pub indices: Vec<u32>,
}

#[derive(Default, Clone)]
pub struct PropPalette {
    /// Neutral shared material for authored crop vertex colors.
    pub crop: usize,
    pub foliage: [usize; 3],
    pub stem: usize,
    pub dry_grass: [usize; 3],
    pub twig: usize,
    pub flowers: [usize; 8],
    pub mushroom: usize,
    pub stone: usize,
}
