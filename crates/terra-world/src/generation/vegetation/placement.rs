use super::{MAX_TREE_HEIGHT, MAX_TREE_REACH, Tree, mix, random, species::TreeSpecies};
use crate::TerraGenerator;

const SPACING: i64 = 16;

pub(crate) fn trees_in_region(terrain: &TerraGenerator, start: [i64; 3], size: usize) -> Vec<Tree> {
    candidates(terrain, start, size, true)
}
pub(crate) fn trees_in_column(terrain: &TerraGenerator, start: [i64; 3], size: usize) -> Vec<Tree> {
    candidates(terrain, start, size, false)
}
fn candidates(terrain: &TerraGenerator, start: [i64; 3], size: usize, vertical: bool) -> Vec<Tree> {
    if !terrain.config().trees || terrain.config().tree_density == 0.0 {
        return Vec::new();
    }
    let mut trees = Vec::new();
    for z in (start[2] - MAX_TREE_REACH).div_euclid(SPACING)
        ..=(start[2] + size as i64 + MAX_TREE_REACH).div_euclid(SPACING)
    {
        for x in (start[0] - MAX_TREE_REACH).div_euclid(SPACING)
            ..=(start[0] + size as i64 + MAX_TREE_REACH).div_euclid(SPACING)
        {
            let seed = mix(terrain.config().seed.0
                ^ (x as u64).wrapping_mul(0x9e3779b97f4a7c15)
                ^ (z as u64).wrapping_mul(0xbf58476d1ce4e5b9));
            let ticket = f64::from(random(seed, 200));
            let density = terrain.config().tree_density;
            if ticket >= density {
                continue;
            }
            let root_x = x * SPACING + 4 + (seed % 8) as i64;
            let root_z = z * SPACING + 4 + ((seed >> 16) % 8) as i64;
            let biome = terrain.biome_at(root_x as f64 + 0.5, root_z as f64 + 0.5);
            let desert = biome.aridity() > 0.5;
            let probability = if desert {
                0.78
            } else {
                terrain.forest_probability_with_biome(
                    root_x as f64 + 0.5,
                    root_z as f64 + 0.5,
                    biome,
                )
            };
            if ticket >= density * probability {
                continue;
            }
            let oasis = if desert {
                let Some(oasis) = terrain.oasis_at(root_x as f64 + 0.5, root_z as f64 + 0.5) else {
                    continue;
                };
                Some(oasis)
            } else {
                None
            };
            let column = terrain.surface_column(root_x as f64 + 0.5, root_z as f64 + 0.5);
            let height = column.height;
            let species = if let Some(oasis) = oasis {
                if column.oasis_moisture < 0.12
                    || height <= oasis.level + 0.6
                    || height > oasis.level + 6.0
                    || column.slope > 1.4
                {
                    continue;
                }
                TreeSpecies::Palm
            } else {
                let group = mix(terrain.config().seed.0
                    ^ (root_x.div_euclid(96) as u64).wrapping_mul(0x9e3779b97f4a7c15)
                    ^ (root_z.div_euclid(96) as u64).wrapping_mul(0xbf58476d1ce4e5b9));
                let species_seed = if terrain.config().biomes && random(seed, 225) < 0.75 {
                    group
                } else {
                    seed
                };
                TreeSpecies::in_biome(species_seed, column.biome)
            };
            let suitable_ground = match species {
                TreeSpecies::Palm => matches!(
                    column.surface_material,
                    terra_voxel::MaterialId::Sand | terra_voxel::MaterialId::Grass
                ),
                TreeSpecies::Spruce => matches!(
                    column.surface_material,
                    terra_voxel::MaterialId::Grass | terra_voxel::MaterialId::Snow
                ),
                _ => {
                    column.surface_material == terra_voxel::MaterialId::Grass
                        && column.snow_cover <= 0.5
                }
            };
            if column
                .water_level
                .is_some_and(|level| height <= level + 1.0)
                || height <= terrain.config().sea_level + 2.0
                || !suitable_ground
                || column.soil_depth < 1.0
            {
                continue;
            }
            let ground = (height - 0.5).floor() as i64;
            if terrain
                .sample_with_column(
                    [
                        root_x as f64 + 0.5,
                        ground as f64 + 0.5,
                        root_z as f64 + 0.5,
                    ],
                    column,
                )
                .density
                > 0.0
            {
                continue;
            }
            if vertical
                && (ground + MAX_TREE_HEIGHT + 1 < start[1] || ground - 5 >= start[1] + size as i64)
            {
                continue;
            }
            let origin = [root_x, ground + 1, root_z];
            let mut roots = [0.0; 4];
            let mut supported = true;
            let reach = if matches!(species, TreeSpecies::Palm | TreeSpecies::Mushroom(_)) {
                1
            } else {
                4
            };
            for (i, [dx, dz]) in [[reach, 0], [0, reach], [-reach, 0], [0, -reach]]
                .into_iter()
                .enumerate()
            {
                let root_height =
                    terrain.surface_height((root_x + dx) as f64 + 0.5, (root_z + dz) as f64 + 0.5);
                let difference = (root_height - 0.5).floor() as i64 - ground;
                roots[i] = difference as f32;
                supported &= difference.abs() <= 2;
            }
            if !supported {
                continue;
            }
            let mut tree = Tree::with_species(origin, seed, roots, species);
            if terrain.config().ecology_revision < 2 && matches!(species, TreeSpecies::Cherry(_)) {
                tree.wood_material = terra_voxel::MaterialId::Wood;
            }
            if terrain.config().geology_revision == 0 && species == TreeSpecies::Spruce {
                tree.wood_material = terra_voxel::MaterialId::Wood;
            }
            let intersects = if vertical {
                tree.bounds.intersects(origin, start, size)
            } else {
                [0, 2].into_iter().all(|i| {
                    origin[i] + i64::from(tree.bounds.max[i]) > start[i]
                        && origin[i] + i64::from(tree.bounds.min[i]) < start[i] + size as i64
                })
            };
            if intersects && !terrain.structure_clears_tree(origin) {
                trees.push(tree);
            }
        }
    }
    // Grid traversal defines overlap priority, independently of section order.
    trees
}
