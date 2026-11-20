use super::species::TreeSpecies;
use super::*;
use crate::{SectionGenerator, TerraGenerator, edits::EditJournal};
use std::collections::{HashSet, VecDeque};
use terra_core::{WorldConfig, WorldSeed};
use terra_voxel::{MaterialId, SectionPos, global_to_section};

fn flat(seed: u64, trees: bool) -> TerraGenerator {
    TerraGenerator::new(WorldConfig {
        seed: WorldSeed(seed),
        base_height: 12.0,
        sea_level: 8.0,
        landscape_revision: 3,
        hill_amplitude: 0.0,
        mountain_amplitude: 0.0,
        caves: false,
        trees,
        biomes: false,
        ..Default::default()
    })
    .expect("flat world")
}

fn raster(tree: &Tree) -> Vec<u16> {
    let start = [tree.origin[0] - 16, tree.origin[1] - 4, tree.origin[2] - 16];
    let mut blocks = vec![0; 40_usize.pow(3)];
    tree.stamp_wood(start, 40, &mut blocks);
    tree.stamp_leaves(start, 40, &mut blocks);
    blocks
}

#[test]
fn cherry_wood_revision_preserves_existing_tree_locations_and_old_world_materials() {
    let old = TerraGenerator::new(WorldConfig {
        seed: WorldSeed(42042),
        landscape_revision: 4,
        sea_level: 22.0,
        rivers: true,
        biome_definitions: Some(terra_core::BiomeDefinitions::default()),
        ecology_revision: 1,
        structures_revision: 4,
        ..Default::default()
    })
    .unwrap();
    let new = TerraGenerator::new(WorldConfig {
        ecology_revision: 2,
        ..old.config().clone()
    })
    .unwrap();
    let start = [-2224, -16, -3232];
    let previous = trees_in_region(&old, start, 96);
    let current = trees_in_region(&new, start, 96);
    assert_eq!(previous.len(), current.len());
    let mut cherry = 0;
    for (old, new) in previous.iter().zip(&current) {
        let mut expected = old.clone();
        if matches!(old.species, TreeSpecies::Cherry(_)) {
            assert_eq!(old.wood_material, MaterialId::Wood);
            assert_eq!(new.wood_material, MaterialId::CherryLog);
            expected.wood_material = MaterialId::CherryLog;
            cherry += 1;
        }
        assert_eq!(expected, *new);
    }
    assert!(cherry > 0, "fixture must include generated cherry trees");
}

#[cfg(feature = "bevy")]
#[test]
fn whole_tree_shape_matches_every_stamped_root_trunk_and_branch_across_species_and_seeds() {
    for species in [
        TreeSpecies::Green,
        TreeSpecies::Autumn,
        TreeSpecies::Birch,
        TreeSpecies::Spruce,
        TreeSpecies::Fir,
        TreeSpecies::Apple,
        TreeSpecies::Palm,
    ] {
        for seed in [1, 71, 42042, u64::MAX] {
            let tree =
                Tree::with_species([-17, 12, -33], mix(seed), [-2.0, 0.0, 2.0, 1.0], species);
            let shape = tree.felling_shape().expect("bounded natural tree");
            let start = std::array::from_fn(|i| tree.origin[i] + i64::from(tree.bounds.min[i]));
            let size = (0..3)
                .map(|i| (tree.bounds.max[i] - tree.bounds.min[i]) as usize)
                .max()
                .unwrap();
            let mut blocks = vec![0; size.pow(3)];
            tree.stamp_wood(start, size, &mut blocks);
            let mut stamped = HashSet::new();
            for (index, material) in blocks.into_iter().enumerate() {
                if material == tree.wood_material as u16 {
                    stamped.insert([
                        start[0] + (index % size) as i64,
                        start[1] + (index / size % size) as i64,
                        start[2] + (index / size / size) as i64,
                    ]);
                }
            }
            assert_eq!(
                shape.wood.into_iter().collect::<HashSet<_>>(),
                stamped,
                "{species:?} seed {seed}"
            );
            assert!(stamped.len() > 10);
        }
    }
}

#[test]
fn styles_are_varied_reproducible_and_inside_the_search_envelope() {
    let mut species = HashSet::new();
    let mut forms = HashSet::new();
    let mut heights = HashSet::new();
    for seed in 0..1024 {
        let seed = mix(seed);
        let tree = Tree::new([0; 3], seed, [-2.0, 0.0, 2.0, 1.0]);
        assert_eq!(tree, Tree::new([0; 3], seed, [-2.0, 0.0, 2.0, 1.0]));
        assert!(tree.wood.len() >= 20 && tree.crowns.len() >= 10);
        for axis in [0, 2] {
            assert!(i64::from(tree.bounds.min[axis]) >= -MAX_TREE_REACH);
            assert!(i64::from(tree.bounds.max[axis]) <= MAX_TREE_REACH);
        }
        assert!(tree.bounds.min[1] >= -5);
        assert!(i64::from(tree.bounds.max[1]) <= MAX_TREE_HEIGHT);
        species.insert(tree.species);
        forms.insert(tree.form as u8);
        heights.insert(tree.bounds.max[1]);
    }
    assert_eq!(species.len(), 3);
    assert_eq!(forms.len(), 3);
    assert!(heights.len() >= 8);
    assert_ne!(
        Tree::new([0; 3], mix(1), [0.0; 4]),
        Tree::new([0; 3], mix(2), [0.0; 4])
    );
}

#[test]
fn tree_density_removes_twenty_percent_without_moving_retained_trees() {
    let base = flat(42042, true);
    let full = TerraGenerator::new(WorldConfig {
        tree_density: 1.0,
        ..base.config().clone()
    })
    .unwrap();
    let mut full_count = 0;
    let mut reduced_count = 0;
    for z in (-1024..=1024).step_by(64) {
        for x in (-1024..=1024).step_by(64) {
            let start = [i64::from(x), 0, i64::from(z)];
            let original = trees_in_region(&full, start, 64);
            let reduced = trees_in_region(&base, start, 64);
            // Count owned anchors once, not crowns spilling into neighboring regions.
            let owned = |tree: &&Tree| {
                (start[0]..start[0] + 64).contains(&tree.origin[0])
                    && (start[2]..start[2] + 64).contains(&tree.origin[2])
            };
            full_count += original.iter().filter(owned).count();
            reduced_count += reduced.iter().filter(owned).count();
            assert!(reduced.iter().all(|tree| original.contains(tree)));
        }
    }
    let ratio = reduced_count as f64 / full_count as f64;
    assert!(
        (0.77..0.83).contains(&ratio),
        "full={full_count}, reduced={reduced_count}, ratio={ratio}"
    );
    let disabled = TerraGenerator::new(WorldConfig {
        tree_density: 0.0,
        ..base.config().clone()
    })
    .unwrap();
    assert!(trees_in_region(&disabled, [-32, 0, -32], 64).is_empty());
}

#[test]
fn trunks_roots_and_branches_are_wide_and_six_connected() {
    for seed in 0..24 {
        let tree = Tree::new([0; 3], mix(seed), [0.0; 4]);
        let blocks = raster(&tree);
        let mut wood = HashSet::new();
        for z in 0..40 {
            for y in 0..40 {
                for x in 0..40 {
                    if blocks[x + 40 * (y + 40 * z)] == tree.species.wood() as u16 {
                        wood.insert([x as i32 - 16, y as i32 - 4, z as i32 - 16]);
                    }
                }
            }
        }
        assert!(wood.len() > 150, "seed={seed}, wood={}", wood.len());
        assert!(
            wood.iter().filter(|p| p[1] == 1).count() >= 5,
            "thick trunk and roots"
        );
        assert!(wood.iter().any(|p| p[0].abs() >= 3 && p[1] > 4));
        assert!(wood.iter().any(|p| p[2].abs() >= 3 && p[1] > 4));
        assert!(wood.contains(&[0; 3]));
        let mut visited = HashSet::from([[0; 3]]);
        let mut queue = VecDeque::from([[0; 3]]);
        while let Some(point) = queue.pop_front() {
            for axis in 0..3 {
                for direction in [-1, 1] {
                    let mut next = point;
                    next[axis] += direction;
                    if wood.contains(&next) && visited.insert(next) {
                        queue.push_back(next);
                    }
                }
            }
        }
        assert_eq!(visited.len(), wood.len(), "seed={seed}: disconnected wood");
    }
}

#[test]
fn rasterization_matches_samples_across_negative_horizontal_and_vertical_sections() {
    let tree = Tree::new([-1, 15, -1], mix(91), [0.0; 4]);
    for x in [-1, 0] {
        for y in 0..3 {
            for z in [-1, 0] {
                let start = SectionPos::new(x, y, z).origin().expect("section origin");
                let mut blocks = vec![0; 16_usize.pow(3)];
                tree.stamp_wood(start, 16, &mut blocks);
                tree.stamp_leaves(start, 16, &mut blocks);
                for lz in 0..16 {
                    for ly in 0..16 {
                        for lx in 0..16 {
                            let global = [
                                start[0] + lx as i64,
                                start[1] + ly as i64,
                                start[2] + lz as i64,
                            ];
                            let expected = tree
                                .wood_at(global)
                                .or_else(|| tree.leaves_at(global))
                                .map_or(0, |m| m as u16);
                            assert_eq!(blocks[lx + 16 * (ly + 16 * lz)], expected, "{global:?}");
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn local_shapes_are_stable_at_large_positive_and_negative_coordinates() {
    let reference = raster(&Tree::new([0; 3], mix(42), [0.0; 4]));
    for origin in [[-1; 3], [1_i64 << 39; 3], [-(1_i64 << 39); 3]] {
        assert_eq!(reference, raster(&Tree::new(origin, mix(42), [0.0; 4])));
    }
}

#[test]
fn placements_use_full_seed_and_include_canopies_from_distant_anchors() {
    let start = [-16, 16, -16];
    let terrain = flat(42042, true);
    let trees = trees_in_region(&terrain, start, 16);
    assert!(!trees.is_empty());
    assert_eq!(trees, trees_in_region(&flat(42042, true), start, 16));
    assert_ne!(
        trees,
        trees_in_region(&flat(42042 + (1 << 32), true), start, 16)
    );
    assert!(
        (-4..=4).any(|z| (-4..=4).any(|x| {
            let start = [x * 16, 16, z * 16];
            trees_in_region(&terrain, start, 16).iter().any(|tree| {
                let distant = [0, 2]
                    .into_iter()
                    .any(|i| tree.origin[i] < start[i] - 3 || tree.origin[i] >= start[i] + 19);
                let mut blocks = vec![0; 16_usize.pow(3)];
                tree.stamp_wood(start, 16, &mut blocks);
                tree.stamp_leaves(start, 16, &mut blocks);
                distant && blocks.iter().any(|&m| m != 0)
            })
        })),
        "wide crowns must be discovered outside the former three-voxel margin"
    );
    assert!(trees_in_region(&flat(42042, false), start, 16).is_empty());
    assert!(trees_in_region(&terrain, [-16, 100, -16], 16).is_empty());
}

#[test]
fn generated_tree_sections_match_global_samples_and_any_generation_order() {
    let generator = SectionGenerator::new(flat(42042, true));
    for position in [
        SectionPos::new(-1, 1, -1),
        SectionPos::new(0, 1, -1),
        SectionPos::new(-1, 2, -1),
    ] {
        let section = generator.generate(position).expect("tree section");
        generator
            .generate(SectionPos::new(2, 1, 1))
            .expect("other section");
        assert_eq!(section, generator.generate(position).expect("repeat"));
        let origin = position.origin().expect("origin");
        for z in (0..16).step_by(4) {
            for y in (0..16).step_by(4) {
                for x in [0, 4, 8, 12, 15] {
                    let global = [
                        origin[0] + x as i64,
                        origin[1] + y as i64,
                        origin[2] + z as i64,
                    ];
                    assert_eq!(
                        section.block([x, y, z]).expect("block"),
                        generator.block_at(global).expect("global block")
                    );
                }
            }
        }
    }
}

#[test]
fn tree_boundary_edits_override_regeneration_and_neighbor_padding() {
    for ecology_revision in 0..=WorldConfig::default().ecology_revision {
        assert_tree_boundary_edits(ecology_revision);
    }
}

fn assert_tree_boundary_edits(ecology_revision: u8) {
    let mut found = None;
    for seed in 0..64 {
        let terrain = TerraGenerator::new(WorldConfig {
            ecology_revision,
            ..flat(seed, true).config().clone()
        })
        .expect("flat world with explicit ecology revision");
        for tree in trees_in_region(&terrain, [-16, 0, 0], 32) {
            if let Some(point) = negative_section_wood_crossing(&tree) {
                found = Some((terrain, point, tree.wood_material));
                break;
            }
        }
        if found.is_some() {
            break;
        }
    }
    let (terrain, point, wood) = found.unwrap_or_else(|| {
        panic!("generated wood crosses a negative section edge in ecology {ecology_revision}")
    });
    let generator = SectionGenerator::new(terrain);
    let (owner, local) = global_to_section(point);
    let adjacent = [point[0] + 1, point[1], point[2]];
    assert!(adjacent[0] < 0);
    assert_eq!(local[0], 15);
    let original = generator.generate(owner).expect("owner");
    assert_eq!(original.block(local).expect("wood"), wood as u16);
    assert_eq!(generator.block_at(adjacent).unwrap(), wood as u16);
    let mut journal = EditJournal::default();
    journal.record(point, 0).expect("mine root");
    let rebuilt = generator
        .build(
            owner,
            journal.revision(owner),
            &journal.snapshot(owner).expect("snapshot"),
            true,
        )
        .expect("rebuilt");
    assert_eq!(rebuilt.data.block(local).expect("edited block"), 0);
    assert_eq!(rebuilt.data.revision(), 1);
    let neighbor = owner.offset([1, 0, 0]).expect("neighbor");
    assert_eq!(neighbor, global_to_section(adjacent).0);
    assert_eq!(
        journal
            .snapshot(neighbor)
            .expect("neighbor padding")
            .get(&point),
        Some(&0)
    );
    assert_eq!(
        generator
            .build(
                owner,
                1,
                &journal.snapshot(owner).expect("reloaded snapshot"),
                false
            )
            .expect("reload")
            .data,
        rebuilt.data
    );
}

fn negative_section_wood_crossing(tree: &Tree) -> Option<[i64; 3]> {
    // Anchors have a four-cell inset from the placement grid. Their short roots
    // need not cross a section edge; branches can cross at another height. Only
    // inspect the two touching columns of each actual negative boundary inside
    // this tree's finite bounds, rather than assuming x=0 or root height.
    let min_x = tree.origin[0] + i64::from(tree.bounds.min[0]);
    let max_x = tree.origin[0] + i64::from(tree.bounds.max[0]);
    for boundary in (((min_x.div_euclid(16) + 1) * 16)..max_x.min(0)).step_by(16) {
        for y in tree.bounds.min[1]..tree.bounds.max[1] {
            for z in tree.bounds.min[2]..tree.bounds.max[2] {
                let p = [
                    boundary - 1,
                    tree.origin[1] + i64::from(y),
                    tree.origin[2] + i64::from(z),
                ];
                if tree.wood_at(p).is_some() && tree.wood_at([boundary, p[1], p[2]]).is_some() {
                    return Some(p);
                }
            }
        }
    }
    None
}

#[test]
fn species_material_ids_are_stable_and_distinct() {
    assert_eq!(TreeSpecies::Green.wood() as u16, 6);
    assert_eq!(TreeSpecies::Green.leaves() as u16, 7);
    assert_eq!(TreeSpecies::Autumn.leaves() as u16, 8);
    assert_eq!(TreeSpecies::Birch.leaves() as u16, 9);
    assert_eq!(TreeSpecies::Birch.wood() as u16, 10);
    assert_eq!(TreeSpecies::Fir.wood(), TreeSpecies::Spruce.wood());
    assert_eq!(TreeSpecies::Fir.leaves(), TreeSpecies::Spruce.leaves());
}

#[test]
fn firs_have_a_tapered_tiered_silhouette_distinct_from_spruces() {
    for seed in 0..64 {
        let seed = mix(seed);
        let tree = Tree::fir([0; 3], seed);
        assert_ne!(raster(&tree), raster(&Tree::spruce([0; 3], seed)));
        assert!(tree.wood.len() <= 2 && tree.crowns.len() <= 12);
        let width_at = |low: f32, high: f32| {
            let height = tree.bounds.max[1] as f32;
            let mut width = 0;
            for y in (height * low) as i64..(height * high) as i64 {
                for x in -8..=8 {
                    if tree.leaves_at([x, y, 0]).is_some() {
                        width = width.max(x.abs());
                    }
                }
            }
            width
        };
        assert!(width_at(0.15, 0.45) > width_at(0.75, 0.98));
        let top = i64::from(tree.bounds.max[1]) - 2;
        assert!(tree.leaves_at([0, top, 0]).is_some());
    }
}

#[test]
fn spruces_are_taller_single_trunk_trees_with_a_leaf_covered_tip() {
    for seed in 0..64 {
        let tree = Tree::spruce([0; 3], mix(seed));
        let original_height = 15.0 + super::random(mix(seed), 1) * 9.0;
        assert!(tree.bounds.max[1] as f32 >= original_height * 1.4);
        let last_wood = (0..tree.bounds.max[1])
            .filter(|&y| tree.wood_at([0, i64::from(y), 0]).is_some())
            .max()
            .unwrap();
        assert!(tree.leaves_at([0, i64::from(last_wood + 1), 0]).is_some());
        assert!(tree.bounds.max[1] > last_wood + 2);
        for y in 0..=last_wood {
            assert!(tree.wood_at([0, i64::from(y), 0]).is_some());
            for [x, z] in [[-1, 0], [1, 0], [0, -1], [0, 1], [1, 1]] {
                assert!(tree.wood_at([x, i64::from(y), z]).is_none());
            }
        }
    }
}

#[test]
fn mushroom_colors_have_distinct_cap_silhouettes_and_cluster_variation() {
    let mut silhouettes = HashSet::new();
    let mut cluster_counts = HashSet::new();
    for variant in 0..6 {
        let tree = Tree::mushroom([0; 3], mix(123), variant);
        let cells: Vec<_> = raster(&tree)
            .into_iter()
            .map(|m| u8::from(m != 0))
            .collect();
        assert!(
            silhouettes.insert(cells),
            "duplicate cap profile for variant {variant}"
        );
        for seed in 0..24 {
            cluster_counts.insert(Tree::mushroom([0; 3], mix(seed), variant).crowns.len());
        }
    }
    assert_eq!(cluster_counts, HashSet::from([1, 2, 3]));
}

#[test]
fn palms_spruces_and_all_six_giant_mushrooms_are_connected_and_bounded() {
    let species = [
        TreeSpecies::Palm,
        TreeSpecies::Spruce,
        TreeSpecies::Fir,
        TreeSpecies::Mushroom(0),
        TreeSpecies::Mushroom(1),
        TreeSpecies::Mushroom(2),
        TreeSpecies::Mushroom(3),
        TreeSpecies::Mushroom(4),
        TreeSpecies::Mushroom(5),
    ];
    let mut caps = HashSet::new();
    for species in species {
        for seed in 0..24 {
            let tree = Tree::with_species([-1, 15, -1], mix(seed), [0.0; 4], species);
            assert_eq!(
                tree,
                Tree::with_species([-1, 15, -1], mix(seed), [0.0; 4], species)
            );
            for axis in [0, 2] {
                assert!(i64::from(tree.bounds.min[axis]) >= -MAX_TREE_REACH);
                assert!(i64::from(tree.bounds.max[axis]) <= MAX_TREE_REACH);
            }
            assert!(i64::from(tree.bounds.max[1]) <= MAX_TREE_HEIGHT);
            let blocks = raster(&tree);
            let cells: HashSet<_> = blocks
                .iter()
                .enumerate()
                .filter_map(|(index, material)| {
                    (*material != 0).then_some([
                        (index % 40) as i32,
                        ((index / 40) % 40) as i32,
                        (index / (40 * 40)) as i32,
                    ])
                })
                .collect();
            let root = [16, 4, 16];
            assert!(cells.contains(&root));
            let mut connected = HashSet::from([root]);
            let mut queue = VecDeque::from([root]);
            while let Some(point) = queue.pop_front() {
                for axis in 0..3 {
                    for step in [-1, 1] {
                        let mut next = point;
                        next[axis] += step;
                        if cells.contains(&next) && connected.insert(next) {
                            queue.push_back(next);
                        }
                    }
                }
            }
            assert_eq!(
                connected.len(),
                cells.len(),
                "{species:?}, seed={seed}: floating voxels"
            );
            if let TreeSpecies::Mushroom(_) = species {
                assert!(blocks.iter().filter(|&&m| m == 20).count() > 25);
                assert!(
                    blocks
                        .iter()
                        .filter(|&&m| m == species.leaves() as u16)
                        .count()
                        > 50
                );
                caps.insert(species.leaves() as u16);
            }
        }
    }
    assert_eq!(caps, HashSet::from([21, 22, 23, 24, 25, 26]));
}

#[test]
fn new_structure_sampling_matches_section_stamping_at_negative_boundaries() {
    for species in [
        TreeSpecies::Palm,
        TreeSpecies::Spruce,
        TreeSpecies::Fir,
        TreeSpecies::Mushroom(4),
    ] {
        let tree = Tree::with_species([-1, 15, -1], mix(7), [0.0; 4], species);
        for section in [
            SectionPos::new(-1, 1, -1),
            SectionPos::new(0, 1, -1),
            SectionPos::new(-1, 2, 0),
            SectionPos::new(0, 0, 0),
        ] {
            let start = section.origin().unwrap();
            let mut cells = vec![0; 16_usize.pow(3)];
            tree.stamp_wood(start, 16, &mut cells);
            tree.stamp_leaves(start, 16, &mut cells);
            for (index, material) in cells.into_iter().enumerate() {
                let global = [
                    start[0] + (index % 16) as i64,
                    start[1] + ((index / 16) % 16) as i64,
                    start[2] + (index / 256) as i64,
                ];
                assert_eq!(
                    material,
                    tree.wood_at(global)
                        .or_else(|| tree.leaves_at(global))
                        .map_or(0, |m| m as u16)
                );
            }
        }
    }
}

#[test]
fn mushroom_park_places_real_structures_on_solid_ground_and_keeps_reload_stable() {
    let terrain = TerraGenerator::new(WorldConfig {
        landscape_revision: 4,
        blood_forest_revision: 0,
        sea_level: 22.0,
        rivers: true,
        biome_definitions: Some(terra_core::BiomeDefinitions::default()),
        ..Default::default()
    })
    .unwrap();
    let start = [-768, 0, 512];
    let trees = trees_in_column(&terrain, start, 256);
    let mushrooms: Vec<_> = trees
        .iter()
        .filter(|tree| matches!(tree.species, TreeSpecies::Mushroom(_)))
        .collect();
    assert!(
        mushrooms.len() >= 6,
        "exercise a genuine mushroom park: {}",
        mushrooms.len()
    );
    let generator = SectionGenerator::new(terrain);
    for tree in mushrooms.into_iter().take(8) {
        let mut support = tree.origin;
        support[1] -= 1;
        assert!(
            MaterialId::from_raw(generator.block_at(support).unwrap())
                .is_some_and(MaterialId::is_solid)
        );
        let owner = global_to_section(tree.origin).0;
        let client = generator
            .build(owner, 0, &Default::default(), true)
            .unwrap();
        let server = generator
            .build(owner, 0, &Default::default(), false)
            .unwrap();
        let repeat = generator
            .build(owner, 0, &Default::default(), true)
            .unwrap();
        assert_eq!(client.data, server.data);
        assert_eq!(client.props, server.props);
        assert_eq!(client.data, repeat.data);
        assert_eq!(client.props, repeat.props);
        assert!(
            client
                .data
                .blocks()
                .contains(&(MaterialId::MushroomStem as u16))
        );
    }
}

#[test]
fn real_desert_oases_have_supported_palms_and_persistent_dry_understory() {
    assert_oasis_palms_and_understory(1, [[-2682, -153], [-2401, -1404], [-1939, -2166]]);
}

#[test]
fn regional_landscape_oases_have_supported_palms_and_persistent_dry_understory() {
    assert_oasis_palms_and_understory(2, [[-2159, -160], [-2406, 151], [-2682, -153]]);
}

fn assert_oasis_palms_and_understory(revision: u8, centers: [[i64; 2]; 3]) {
    let terrain = TerraGenerator::new(WorldConfig {
        landscape_revision: revision,
        sea_level: 8.0,
        ..Default::default()
    })
    .unwrap();
    let mut palms = Vec::new();
    for center in centers {
        let oasis = terrain
            .oasis_at(center[0] as f64, center[1] as f64)
            .unwrap();
        assert!(oasis.distance < 1.0);
        let trees = trees_in_column(&terrain, [center[0] - 64, 0, center[1] - 64], 128);
        for tree in trees {
            let column =
                terrain.surface_column(tree.origin[0] as f64 + 0.5, tree.origin[2] as f64 + 0.5);
            if column.biome.weights.desert > 0.5 {
                assert_eq!(
                    tree.species,
                    TreeSpecies::Palm,
                    "no temperate tree in the desert"
                );
            }
            if tree.species != TreeSpecies::Palm {
                continue;
            }
            let shore = terrain
                .oasis_at(tree.origin[0] as f64 + 0.5, tree.origin[2] as f64 + 0.5)
                .unwrap();
            assert!(column.height > shore.level + 0.6);
            assert!(column.oasis_moisture > 0.12);
            assert!(shore.distance > shore.radius * 0.7);
            palms.push(tree);
        }
    }
    assert!(
        palms.len() >= 3,
        "palms on real oasis shores: {}",
        palms.len()
    );
    let generator = SectionGenerator::new(terrain);
    let mut dry_grass = 0;
    let mut owners = HashSet::new();
    for palm in palms {
        let mut support = palm.origin;
        support[1] -= 1;
        assert!(
            MaterialId::from_raw(generator.block_at(support).unwrap())
                .is_some_and(MaterialId::is_solid)
        );
        let owner = global_to_section(support).0;
        if !owners.insert(owner) {
            continue;
        }
        let client = generator
            .build(owner, 0, &Default::default(), true)
            .unwrap();
        let server = generator
            .build(owner, 0, &Default::default(), false)
            .unwrap();
        let repeat = generator
            .build(owner, 0, &Default::default(), true)
            .unwrap();
        assert_eq!(client.data, server.data);
        assert_eq!(client.props, server.props);
        assert_eq!(client.props, repeat.props);
        dry_grass += client
            .props
            .iter()
            .filter(|prop| prop.kind == crate::props::PropKind::Grass && prop.variant >= 128)
            .count();
    }
    assert!(dry_grass > 10, "oasis banks have dry grass: {dry_grass}");
}

#[test]
fn natural_apple_species_is_gated_by_saved_ecology_revision() {
    let old = TerraGenerator::new(WorldConfig {
        ecology_revision: 2,
        ..flat(42042, true).config().clone()
    })
    .unwrap();
    let fresh = TerraGenerator::new(WorldConfig {
        ecology_revision: 3,
        ..old.config().clone()
    })
    .unwrap();
    let old = trees_in_region(&old, [-128, 0, -128], 256)
        .into_iter()
        .map(|t| (t.origin, t))
        .collect::<std::collections::BTreeMap<_, _>>();
    let fresh = trees_in_region(&fresh, [-128, 0, -128], 256);
    assert!(old.values().all(|t| t.species != TreeSpecies::Apple));
    let mut apples = 0;
    for tree in fresh {
        let Some(previous) = old.get(&tree.origin) else {
            continue;
        };
        if tree.species == TreeSpecies::Apple {
            apples += 1;
            assert!(matches!(
                previous.species,
                TreeSpecies::Green | TreeSpecies::Autumn
            ));
            assert_eq!(tree.wood_material, MaterialId::Wood);
            assert_eq!(tree.species.leaves(), MaterialId::AppleLeaves);
            assert!(tree.crowns.len() >= 5);
        } else {
            assert_eq!(&tree, previous);
        }
    }
    assert!(
        apples > 0,
        "bounded natural fixture must include real apple trees"
    );
}

#[cfg(feature = "bevy")]
#[test]
fn legacy_spruce_and_cherry_wood_skins_keep_their_real_species_in_felling_and_leaf_sapling_drops() {
    use crate::{
        edits::EditJournal,
        streaming::{ChunkStore, LoadedSection},
    };
    use std::{collections::BTreeSet, sync::Arc};
    use terra_core::{ItemKind, PlayerUuid};
    use terra_voxel::{BlockSection, global_to_section};
    for (kind, expected) in [
        (TreeSpecies::Spruce, terra_core::TreeSpecies::Spruce),
        (TreeSpecies::Cherry(false), terra_core::TreeSpecies::Cherry),
        (TreeSpecies::Cherry(true), terra_core::TreeSpecies::Cherry),
    ] {
        let mut tree = Tree::with_species([0, 16, 0], 501, [0.; 4], kind);
        // Exact historic skins retained by placement.rs: geology0 conifers and
        // ecology0/1 cherry use Oak wood without becoming Oak trees.
        tree.wood_material = MaterialId::Wood;
        let shape = tree.felling_shape().unwrap();
        assert_eq!(shape.wood_material, 6);
        let terrain = TerraGenerator::new(WorldConfig {
            trees: false,
            caves: false,
            water: false,
            rivers: false,
            biomes: false,
            ..Default::default()
        })
        .unwrap();
        let generator = crate::SectionGenerator::new(terrain);
        let mut chunks = ChunkStore::default();
        let mut columns = BTreeSet::new();
        for (&cell, material) in shape
            .wood
            .iter()
            .map(|p| (p, shape.wood_material))
            .chain(shape.leaves.iter().map(|p| (p, shape.leaves_material)))
        {
            let (section, local) = global_to_section(cell);
            columns.insert([section.x, section.z]);
            chunks
                .loaded
                .entry(section)
                .or_insert_with(|| LoadedSection {
                    data: BlockSection::empty(section).unwrap(),
                    mesh: None,
                    mesh_epoch: 0,
                    water_epoch: 0,
                    water: None,
                    props: vec![],
                })
                .data
                .set_block(local, material)
                .unwrap();
        }
        for coordinate in columns {
            let mut column = generator.prepare_column(coordinate, false).unwrap();
            column.trees = vec![tree.clone()];
            chunks.set_prepared_column_for_test(coordinate, Some(Arc::new(column)));
        }
        let mut journal = EditJournal::default();
        let leaf_drop = shape.leaves.iter().find_map(|cell| {
            crate::saplings::leaf_drop(&chunks, &journal, *cell, shape.leaves_material)
        });
        assert_eq!(
            leaf_drop,
            Some(expected),
            "legacy {kind:?}: leaf attribution must use actual generator species"
        );
        let plan = crate::tree_felling::Plan::prepare(&chunks, &journal, shape.origin).unwrap();
        let expected_saplings = shape
            .leaves
            .iter()
            .filter(|cell| crate::saplings::leaf_drop_roll(**cell))
            .count() as u16;
        let mut queue = crate::tree_felling::Queue::default();
        assert!(queue.enqueue(plan, PlayerUuid::from_u128(1)));
        queue
            .finish(&mut chunks, &mut journal, &Default::default())
            .unwrap();
        assert_eq!(
            journal
                .gameplay()
                .drops
                .iter()
                .filter(|d| d.stack.item == ItemKind::Prop(expected.sapling()))
                .map(|d| d.stack.count)
                .sum::<u16>(),
            expected_saplings,
            "legacy {kind:?}: successful leaf rolls keep the original species"
        );
        assert!(
            !journal
                .gameplay()
                .drops
                .iter()
                .any(|d| d.stack.item == ItemKind::Prop(terra_core::TreeSpecies::Oak.sapling()))
        );
    }
}
