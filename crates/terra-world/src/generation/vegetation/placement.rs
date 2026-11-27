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
    let grid_range = |axis: usize| {
        (start[axis] - MAX_TREE_REACH).div_euclid(SPACING)
            ..=(start[axis] + size as i64 + MAX_TREE_REACH).div_euclid(SPACING)
    };
    let plots = || grid_range(2).flat_map(|z| grid_range(0).map(move |x| (x, z)));
    let dense_forest = terrain.config().biomes && terrain.config().ecology_revision >= 15;
    // Keep original roots first for stable overlap priority. An offset second
    // lattice lets woodland crowns meet while the same trail and glade masks stay open.
    let infill = dense_forest.then(plots).into_iter().flatten();
    for ((x, z), extra) in plots().map(|p| (p, false)).chain(infill.map(|p| (p, true))) {
        let salt = if extra { 0xb33f_f012_6a64_03d9 } else { 0 };
        let seed = mix(terrain.config().seed.0
            ^ salt
            ^ (x as u64).wrapping_mul(0x9e3779b97f4a7c15)
            ^ (z as u64).wrapping_mul(0xbf58476d1ce4e5b9));
        let ticket = f64::from(random(seed, 200));
        let density = terrain.config().tree_density;
        if ticket >= density {
            continue;
        }
        let offset = |bits: u64| {
            if extra {
                (bits % 3) as i64
            } else {
                4 + (bits % 8) as i64
            }
        };
        let root_x = x * SPACING + offset(seed);
        let root_z = z * SPACING + offset(seed >> 16);
        let biome = terrain.biome_at(root_x as f64 + 0.5, root_z as f64 + 0.5);
        if extra && biome.weights.forest <= 0.55 {
            continue;
        }
        let desert = biome.aridity() > 0.5;
        let maritime = biome.weights.coastal_land_fraction() > 0.49;
        let coastal_palm = !maritime && !extra && tropical_beach(terrain, [root_x, root_z], biome);
        let probability = if maritime {
            terrain
                .biome_definitions()
                .get(terra_core::BiomeKind::MistyCoast)
                .terrain
                .tree_density
        } else if biome.weights.swamp > 0.48 {
            terrain
                .biome_definitions()
                .get(terra_core::BiomeKind::Swamp)
                .terrain
                .tree_density
                .min(1.0)
        } else if biome.weights.savanna > 0.48 {
            terrain
                .biome_definitions()
                .get(terra_core::BiomeKind::Savanna)
                .terrain
                .tree_density
                .min(1.)
        } else if coastal_palm {
            0.80
        } else if desert && terrain.desert_overhaul() {
            0.96
        } else if desert {
            0.78
        } else {
            terrain.forest_probability_with_biome(root_x as f64 + 0.5, root_z as f64 + 0.5, biome)
        };
        if ticket >= density * probability {
            continue;
        }
        let oasis = if desert && !coastal_palm {
            let Some(oasis) = terrain.oasis_at(root_x as f64 + 0.5, root_z as f64 + 0.5) else {
                continue;
            };
            Some(oasis)
        } else {
            None
        };
        let column = terrain.surface_column(root_x as f64 + 0.5, root_z as f64 + 0.5);
        let height = column.height;
        let mut species = if maritime {
            TreeSpecies::Spruce
        } else if coastal_palm {
            TreeSpecies::Palm
        } else if let Some(oasis) = oasis {
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
            TreeSpecies::in_biome(
                species_seed,
                column.biome,
                terrain.config().landscape_revision,
            )
        };
        // The saved ecology revision gates this addition. Historic trees
        // retain their exact species, cells and overlap priority.
        if terrain.config().ecology_revision >= 13
            && biome.weights.plains > 0.55
            && matches!(species, TreeSpecies::Autumn | TreeSpecies::Green)
        {
            // Solitary flowering crowns are accents among pasture oaks and white birches.
            species = match seed % 19 {
                0 | 1 => TreeSpecies::Cherry(false),
                2 => TreeSpecies::Cherry(true),
                3..=6 => TreeSpecies::Birch,
                _ => TreeSpecies::Green,
            };
        }
        if !coastal_palm
            && terrain.config().biomes
            && terrain.config().ecology_revision >= 14
            && biome.weights.forest > 0.55
        {
            // Each stand favours a species while individual candidates mix the canopy layers.
            species = match seed % 20 {
                0..=8 => TreeSpecies::Green,
                9..=13 => TreeSpecies::Birch,
                14..=16 => TreeSpecies::Spruce,
                _ => species,
            };
        }
        let alpine = terrain.highland_overhaul() && biome.weights.highland > 0.5;
        if alpine {
            species = match seed % 12 {
                0 | 1 => TreeSpecies::Birch,
                2..=5 => TreeSpecies::Fir,
                _ => TreeSpecies::Spruce,
            };
        }
        if apple_replacement(seed, species, biome, terrain.config().ecology_revision) {
            species = TreeSpecies::Apple;
        }
        let suitable_ground = if maritime {
            height > terrain.config().sea_level + 6.
                && column.slope < 0.85
                && matches!(
                    column.surface_material,
                    terra_voxel::MaterialId::Grass
                        | terra_voxel::MaterialId::MossyCoastalStone
                        | terra_voxel::MaterialId::LichenCoastalStone
                )
                && !terrain.coastal_site_footprint(root_x, root_z)
        } else if coastal_palm {
            column.slope < 1.0
                && matches!(
                    column.surface_material,
                    terra_voxel::MaterialId::Sand
                        | terra_voxel::MaterialId::WetSand
                        | terra_voxel::MaterialId::MossyStone
                        | terra_voxel::MaterialId::Grass
                )
        } else if alpine {
            column.slope <= 1.2
                && height < 178.0 + f64::from(random(seed, 708)) * 14.0
                && matches!(
                    column.surface_material,
                    terra_voxel::MaterialId::Snow
                        | terra_voxel::MaterialId::Firn
                        | terra_voxel::MaterialId::SnowCoveredSod
                )
        } else {
            match species {
                TreeSpecies::Willow => matches!(
                    column.surface_material,
                    terra_voxel::MaterialId::SwampTurf
                        | terra_voxel::MaterialId::Peat
                        | terra_voxel::MaterialId::MossyPeat
                        | terra_voxel::MaterialId::SwampMud
                        | terra_voxel::MaterialId::StonySwampMud
                        | terra_voxel::MaterialId::SwampClay
                ),
                TreeSpecies::Acacia | TreeSpecies::Baobab => matches!(
                    column.surface_material,
                    terra_voxel::MaterialId::SavannaTurf
                        | terra_voxel::MaterialId::DrySavannaTurf
                        | terra_voxel::MaterialId::DrySavannaSoil
                ),
                TreeSpecies::Bloodwood => matches!(
                    column.surface_material,
                    terra_voxel::MaterialId::BloodDirt
                        | terra_voxel::MaterialId::BlightedRootSoil
                        | terra_voxel::MaterialId::CoagulatedMud
                        | terra_voxel::MaterialId::BloodGrass
                        | terra_voxel::MaterialId::BloodMoss
                        | terra_voxel::MaterialId::CrimsonMycelium
                ),
                TreeSpecies::SpiritBlossom(_) => matches!(
                    column.surface_material,
                    terra_voxel::MaterialId::SpiritBlossomGrass | terra_voxel::MaterialId::Grass
                ),
                TreeSpecies::Mushroom(_) => {
                    matches!(
                        column.surface_material,
                        terra_voxel::MaterialId::Grass
                            | terra_voxel::MaterialId::SporeSoil
                            | terra_voxel::MaterialId::MyceliumTurf
                    ) && column.snow_cover <= 0.5
                }
                TreeSpecies::Cherry(_) => {
                    matches!(
                        column.surface_material,
                        terra_voxel::MaterialId::Grass
                            | terra_voxel::MaterialId::PetalTurf
                            | terra_voxel::MaterialId::WoodlandLitter
                    ) && column.snow_cover <= 0.5
                }
                TreeSpecies::Palm => matches!(
                    column.surface_material,
                    terra_voxel::MaterialId::Sand
                        | terra_voxel::MaterialId::DuneSand
                        | terra_voxel::MaterialId::Grass
                ),
                TreeSpecies::Spruce | TreeSpecies::Fir => matches!(
                    column.surface_material,
                    terra_voxel::MaterialId::Grass
                        | terra_voxel::MaterialId::WoodlandLitter
                        | terra_voxel::MaterialId::Snow
                ),
                _ => {
                    matches!(
                        column.surface_material,
                        terra_voxel::MaterialId::Grass | terra_voxel::MaterialId::WoodlandLitter
                    ) && column.snow_cover <= 0.5
                }
            }
        };
        let flooded = if species == TreeSpecies::Willow {
            column.water_level.is_some_and(|level| level - height > 2.2)
        } else {
            column
                .water_level
                .is_some_and(|level| height <= level + 1.0)
                || !coastal_palm && height <= terrain.config().sea_level + 2.0
        };
        if column.biome.weights.crystal_highlands > 0.38
            && (terrain.crystal_summit(root_x as f64 + 0.5, root_z as f64 + 0.5, height)
                || terrain.crystal_building_clearing(root_x, root_z))
        {
            continue;
        }
        if flooded
            || !suitable_ground
            || column.soil_depth < 1.0
            || species == TreeSpecies::Willow && terrain.marsh_clearing(root_x, root_z)
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
        let reach = if species == TreeSpecies::Palm
            || (matches!(species, TreeSpecies::Mushroom(_))
                && terrain.config().ecology_revision < 12)
        {
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
        let mut tree = if terrain.config().spirit_blossom_revision >= 2
            && terrain.config().ecology_revision >= 15
            && matches!(species, TreeSpecies::SpiritBlossom(_))
        {
            Tree::spirit_elder(origin, seed, roots, species)
        } else if species == TreeSpecies::Palm && terrain.desert_overhaul() {
            Tree::oasis_palm(origin, seed, terrain.config().ecology_revision >= 15)
        } else {
            Tree::with_species_revision(
                origin,
                seed,
                roots,
                species,
                terrain.config().ecology_revision,
            )
        };
        if alpine {
            tree.leaves_material = if species == TreeSpecies::Birch {
                terra_voxel::MaterialId::WinterBirchBranches
            } else {
                terra_voxel::MaterialId::SnowySpruceLeaves
            };
        }
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
    // Grid traversal defines overlap priority, independently of section order.
    trees
}

/// A dry planting shelf must face actual tropical sea. Grouping neighbouring
/// grid plots leaves open stretches of sand between the small beach groves.
fn tropical_beach(
    terrain: &TerraGenerator,
    [x, z]: [i64; 2],
    biome: crate::generation::biomes::BiomeSample,
) -> bool {
    if !terrain.marine_overhaul()
        || biome.weights.ocean < 0.22
        || biome.temperature < terrain.config().ocean_biome_config().warm_temperature
    {
        return false;
    }
    let district = mix(terrain.config().seed.0
        ^ (x.div_euclid(80) as u64).wrapping_mul(0x9e3779b97f4a7c15)
        ^ (z.div_euclid(80) as u64).wrapping_mul(0xbf58476d1ce4e5b9));
    if !district.is_multiple_of(3) {
        return false;
    }
    let (x, z) = (x as f64 + 0.5, z as f64 + 0.5);
    let sea = terrain.config().sea_level;
    if !(sea + 1.0..=sea + 7.0).contains(&terrain.surface_height(x, z)) {
        return false;
    }
    [[-24., 0.], [24., 0.], [0., -24.], [0., 24.]]
        .into_iter()
        .any(|[dx, dz]| {
            let shore = terrain.surface_column(x + dx, z + dz);
            shore.water_level == Some(sea)
                && shore.height < sea - 0.5
                && shore.ocean.area() == Some(crate::generation::OceanArea::CoralReef)
        })
}

fn apple_replacement(
    seed: u64,
    species: TreeSpecies,
    biome: crate::generation::biomes::BiomeSample,
    revision: u8,
) -> bool {
    if revision < 3
        || biome.weights.plains + biome.weights.forest <= 0.65
        || biome.temperature <= 0.3
        || biome.temperature >= 0.85
    {
        return false;
    }
    if revision == 3 {
        matches!(species, TreeSpecies::Green | TreeSpecies::Autumn) && random(seed, 280) < 0.14
    } else {
        random(seed, 280) < 0.05
    }
}
#[cfg(test)]
mod apple_rate_tests {
    use super::*;
    use crate::generation::biomes::{BiomeSample, BiomeWeights};
    fn habitat(forest: f64) -> BiomeSample {
        BiomeSample {
            temperature: 0.55,
            humidity: 0.7,
            weights: BiomeWeights {
                ocean: 0.,
                plains: 1. - forest,
                forest,
                desert: 0.,
                highland: 0.,
                mushroom_park: 0.,
                cliffted_desert: 0.,
                cherry_forest: 0.,
                spirit_blossom: 0.0,
                blood_forest: 0.0,
                ashlands: 0.0,
                savanna: 0.0,
                swamp: 0.0,
                petrified_forest: 0.0,
                crystal_highlands: 0.0,
                misty_coast: 0.0,
            },
        }
    }
    #[test]
    fn revision_four_apples_use_five_percent_of_all_suitable_candidates_not_only_oaks() {
        for forest in [0., 0.5, 1.] {
            let b = habitat(forest);
            let mut apples = 0;
            for n in 0..20_000 {
                let seed = super::super::mix(n);
                let selected = apple_replacement(seed, TreeSpecies::Green, b, 4);
                apples += usize::from(selected);
                for species in [
                    TreeSpecies::Autumn,
                    TreeSpecies::Birch,
                    TreeSpecies::Spruce,
                    TreeSpecies::Fir,
                    TreeSpecies::Cherry(false),
                ] {
                    assert_eq!(apple_replacement(seed, species, b, 4), selected);
                }
                assert_eq!(
                    apple_replacement(seed, TreeSpecies::Green, b, 3),
                    random(seed, 280) < 0.14
                );
                assert!(!apple_replacement(seed, TreeSpecies::Spruce, b, 3));
                for revision in 0..3 {
                    assert!(!apple_replacement(seed, TreeSpecies::Green, b, revision));
                }
            }
            assert!(
                (900..=1100).contains(&apples),
                "{forest} forest: {apples}/20000 candidates"
            );
        }
    }
    #[test]
    fn apple_tickets_do_not_introduce_trees_in_unsuitable_habitats() {
        let mut b = habitat(1.);
        for t in [0., 0.3, 0.85, 1.] {
            b.temperature = t;
            for n in 0..128 {
                assert!(!apple_replacement(n, TreeSpecies::Birch, b, 4));
            }
        }
        b = habitat(0.);
        b.weights.plains = 0.;
        b.weights.desert = 1.;
        for n in 0..128 {
            assert!(!apple_replacement(n, TreeSpecies::Green, b, 4));
        }
    }
}
