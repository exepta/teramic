use crate::appearance::GroundTint;
use terra_voxel::meshing::BlockFace;
use terra_world::generation::{TreeAppearance, TreeAppearanceSnapshot};

type CaveSamples = [(
    terra_world::generation::CaveStructure,
    terra_world::generation::CaveBiomeColumn,
); 256];

pub(super) fn cave_samples(
    terrain: &terra_world::TerraGenerator,
    origin: [i64; 3],
    prepared: Option<&terra_world::generation::PreparedColumn>,
) -> Box<CaveSamples> {
    // Older landscape revisions sample a different hydraulic surface for the standalone
    // cave queries. Retain their original tinting when reading an existing world.
    let config = terrain.config();
    let prepared =
        prepared.filter(|_| config.landscape_revision < 5 || config.ecology_revision >= 16);
    Box::new(std::array::from_fn(|i| {
        let x = origin[0] + (i % 16) as i64;
        let z = origin[2] + (i / 16) as i64;
        if let Some(surface) = prepared.and_then(|column| column.surface_at(x, z)) {
            return (surface.cave, surface.cave_biome);
        }
        let (x, z) = (x as f64 + 0.5, z as f64 + 0.5);
        (
            terrain.cave_structure_at(x, z),
            terrain.cave_biome_column(x, z),
        )
    }))
}

pub(super) struct SurfaceTint {
    pub caves: Option<Box<CaveSamples>>,
    pub ground: Option<GroundTint>,
    pub trees: Option<std::sync::Arc<TreeAppearanceSnapshot>>,
    pub origin: [i64; 3],
}

impl SurfaceTint {
    pub fn tree(&self, cell: [u32; 3], material: u16) -> Option<TreeAppearance> {
        self.trees.as_ref()?.at(
            std::array::from_fn(|axis| self.origin[axis] + i64::from(cell[axis])),
            material,
        )
    }

    pub fn tree_tint(&self, cell: [u32; 3], material: u16) -> [f32; 3] {
        match (material, self.tree(cell, material)) {
            (6, Some(TreeAppearance::Spruce)) => [0.54, 0.58, 0.55],
            (7, Some(TreeAppearance::Spruce)) => [0.34, 0.48, 1.18],
            (7, Some(TreeAppearance::Fir)) => [0.24, 0.34, 0.95],
            (6, Some(TreeAppearance::Palm)) => [1.16, 1.08, 0.88],
            _ => [1.0; 3],
        }
    }

    pub fn crown_atlas(
        &self,
        cell: [u32; 3],
        material: u16,
        clusters: crate::blocks::FoliageClusterMaterials,
    ) -> usize {
        if matches!(
            self.tree(cell, material),
            Some(TreeAppearance::Spruce | TreeAppearance::Fir)
        ) {
            clusters.needles.unwrap_or(clusters.leaves)
        } else {
            clusters.leaves
        }
    }

    pub fn split(&self, face: BlockFace) -> Vec<BlockFace> {
        let grid: Vec<_> = (0..face.height)
            .flat_map(|v| {
                (0..face.width).map(move |u| {
                    let mut cell = face.minimum;
                    cell[face.u_axis] += u;
                    cell[face.v_axis] += v;
                    self.tree(cell, face.material)
                })
            })
            .collect();
        super::builder::condition_faces::partition(face, &grid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn streamed_tints_match_direct_cave_samples_for_current_and_legacy_worlds() {
        for ecology_revision in [15, 22] {
            let terrain = terra_world::TerraGenerator::new(terra_core::WorldConfig {
                landscape_revision: 9,
                cave_structure_revision: 13,
                ecology_revision,
                caves: true,
                ..Default::default()
            })
            .unwrap();
            let generator = terra_world::SectionGenerator::new(terrain.clone());
            for column in [[-4, -2], [7, 3]] {
                let prepared = generator.prepare_column(column, true).unwrap();
                let origin = [column[0] * 16, -48, column[1] * 16];
                let expected = cave_samples(&terrain, origin, None);
                assert!(expected.iter().any(|(_, biome)| biome.surface != 0.0));
                assert_eq!(cave_samples(&terrain, origin, Some(&prepared)), expected);
                let next = [origin[0] + 16, origin[1], origin[2]];
                assert_eq!(
                    cave_samples(&terrain, next, Some(&prepared)),
                    cave_samples(&terrain, next, None)
                );
            }
        }
    }
}
