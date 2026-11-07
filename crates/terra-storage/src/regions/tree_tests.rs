use super::*;
use terra_core::{FruitPhase, FruitState, TreeRecord, TreeSpecies, TreeStage};
fn temp() -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!(
        "teramic-trees-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&p).unwrap();
    p
}
#[test]
fn planted_species_progress_exact_cells_and_picked_fruit_survive_atomic_save_and_reload() {
    let root = temp();
    let world = WorldId(200);
    let mut j = EditJournal::default();
    for (n, species) in TreeSpecies::ALL.into_iter().enumerate() {
        let id = n as u64 + 1;
        let p = [n as i64 * 17 - 40, 16, -17];
        let mature = species == TreeSpecies::Apple;
        if mature {
            for (offset, m) in [
                ([0, 0, 0], species.wood()),
                ([0, 1, 0], species.wood()),
                ([1, 2, 0], species.leaves()),
            ] {
                j.record(std::array::from_fn(|a| p[a] + i64::from(offset[a])), m)
                    .unwrap();
            }
        }
        assert!(
            j.gameplay_mut().trees.insert(
                TreeRecord {
                    id,
                    root: p,
                    species,
                    seed: id,
                    planted_by: Some(PlayerUuid::from_u128(1)),
                    stage: if mature {
                        TreeStage::Mature
                    } else {
                        TreeStage::Sapling
                    },
                    growth_seconds: 231.5,
                    bounds: None,
                    wood: if mature {
                        vec![[0, 0, 0], [0, 1, 0]]
                    } else {
                        vec![]
                    },
                    leaves: if mature { vec![[1, 2, 0]] } else { vec![] },
                    fruit: if mature {
                        Some(FruitState {
                            phase: FruitPhase::Ripe,
                            remaining: 5,
                            picked: 0b00100101,
                            cycle_day: 10,
                            last_harvest_day: Some(12),
                            revision: 9,
                        })
                    } else {
                        None
                    },
                    revision: 3
                }
                .with_bounds()
            )
        );
    }
    let apple = j.gameplay().trees.get(5).unwrap();
    assert_eq!(apple.stage, TreeStage::Mature);
    assert_eq!(apple.fruit.unwrap().picked, 0b00100101);
    assert_eq!(apple.fruit.unwrap().remaining, 5);
    // Natural fruit records share the same versioned metadata without becoming planted trees.
    assert!(j.gameplay_mut().trees.insert(TreeRecord {
        id: 99,
        root: [99, 14, 99],
        species: TreeSpecies::Apple,
        seed: 199,
        planted_by: None,
        stage: TreeStage::Mature,
        growth_seconds: 0.,
        bounds: None,
        wood: vec![],
        leaves: vec![],
        fruit: Some(FruitState::new(14)),
        revision: 0
    }));
    save_edits(&root, world, &j).unwrap();
    assert_eq!(
        read(&root.join(INDEX)).unwrap().get(12..16),
        Some(&b"ID12"[..])
    );
    let mut loaded = load_edits(&root, world).unwrap();
    assert_eq!(loaded.gameplay().trees, j.gameplay().trees);
    let p = loaded.gameplay().trees.get(5).unwrap().root;
    loaded.record(p, 0).unwrap();
    loaded.record(p, 6).unwrap();
    assert!(!loaded.gameplay().trees.get(5).unwrap().owns(p, 6));
    let after = loaded.gameplay().trees.clone();
    save_edits(&root, world, &loaded).unwrap();
    assert_eq!(load_edits(&root, world).unwrap().gameplay().trees, after);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn old_index_without_tree_extension_loads_empty_without_changing_old_item_tags() {
    let root = temp();
    let world = WorldId(201);
    let j = EditJournal::default();
    save_edits(&root, world, &j).unwrap();
    assert_eq!(
        read(&root.join(INDEX)).unwrap().get(12..16),
        Some(&b"IDX6"[..])
    );
    assert_eq!(
        load_edits(&root, world).unwrap().gameplay().trees,
        terra_core::TreeRegistry::default()
    );
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn invalid_fruit_masks_and_overlapping_tree_cells_are_rejected_before_save() {
    let root = temp();
    let mut j = EditJournal::default();
    let mut tree = TreeRecord {
        id: 1,
        root: [0, 16, 0],
        species: TreeSpecies::Apple,
        seed: 1,
        planted_by: None,
        stage: TreeStage::Mature,
        growth_seconds: 0.,
        bounds: None,
        wood: vec![],
        leaves: vec![],
        fruit: Some(FruitState {
            phase: FruitPhase::Ripe,
            remaining: 8,
            picked: 1,
            cycle_day: 0,
            last_harvest_day: None,
            revision: 0,
        }),
        revision: 0,
    };
    assert!(!tree.valid());
    assert!(!j.gameplay_mut().trees.insert(tree.clone()));
    tree.fruit = None;
    tree.wood = vec![[0, 0, 0]];
    tree.leaves = vec![[0, 0, 0]];
    tree.refresh_bounds();
    assert!(!tree.valid());
    j.gameplay_mut().trees.trees.insert(1, tree);
    assert!(save_edits(&root, WorldId(202), &j).is_err());
    fs::remove_dir_all(root).unwrap();
}

fn attributed_apple(id: u64, stage: TreeStage) -> TreeRecord {
    let mut wood = (0..=6).map(|y| [0, y, 0]).collect::<Vec<[i8; 3]>>();
    for distance in 1..=3 {
        wood.extend([
            [distance, 4, 0],
            [-distance, 4, 0],
            [0, 4, distance],
            [0, 4, -distance],
        ]);
    }
    let mut leaves = Vec::new();
    for (y, radius) in [(5, 3), (6, 4), (7, 5), (8, 4), (9, 2)] {
        for x in -radius..=radius {
            for z in -radius..=radius {
                let cell = [x, y, z];
                if x * x + z * z <= radius * radius && !wood.contains(&cell) {
                    leaves.push(cell);
                }
            }
        }
    }
    TreeRecord {
        id,
        root: [-1, 15, 15],
        species: TreeSpecies::Apple,
        seed: 42042,
        planted_by: Some(PlayerUuid::from_u128(201)),
        stage,
        growth_seconds: 1400.,
        wood,
        leaves,
        bounds: None,
        fruit: Some(FruitState {
            phase: FruitPhase::Ripe,
            remaining: 5,
            picked: 0b00100101,
            cycle_day: 10,
            last_harvest_day: Some(12),
            revision: 9,
        }),
        revision: 77,
    }
    .with_bounds()
}
fn install_attributed_tree(journal: &mut EditJournal, tree: TreeRecord) {
    // Growth commits cells before attribution. Ordinary edits after attribution
    // intentionally relinquish ownership, so fixture ordering must match it.
    for (&offset, material) in tree
        .wood
        .iter()
        .map(|cell| (cell, tree.species.wood()))
        .chain(tree.leaves.iter().map(|cell| (cell, tree.species.leaves())))
    {
        journal.record(tree.global(offset), material).unwrap();
    }
    assert!(journal.gameplay_mut().trees.insert(tree));
}

#[test]
fn generated_tree_exact_cells_partial_felling_and_picked_fruit_survive_id11_reload() {
    let directory = temp();
    let world = WorldId(203);
    let mut journal = EditJournal::default();
    let tree = attributed_apple(201, TreeStage::Generated);
    assert!(tree.valid());
    assert!(tree.wood.len() + tree.leaves.len() > 200);
    install_attributed_tree(&mut journal, tree.clone());
    save_edits(&directory, world, &journal).unwrap();
    assert_eq!(
        read(&directory.join(INDEX)).unwrap().get(12..16),
        Some(&b"ID12"[..])
    );
    let mut loaded = load_edits(&directory, world).unwrap();
    assert_eq!(loaded.gameplay().trees.get(tree.id), Some(&tree));
    assert_eq!(
        loaded.gameplay().trees.get(tree.id).unwrap().stage,
        TreeStage::Generated
    );
    assert!(
        loaded
            .gameplay()
            .trees
            .get(tree.id)
            .unwrap()
            .stage
            .is_mature()
    );
    for (&offset, material) in tree
        .wood
        .iter()
        .map(|p| (p, tree.species.wood()))
        .chain(tree.leaves.iter().map(|p| (p, tree.species.leaves())))
    {
        let cell = tree.global(offset);
        assert_eq!(loaded.block(cell), Some(material));
        assert_eq!(
            loaded
                .gameplay()
                .trees
                .owner(cell, material)
                .map(|tree| tree.id),
            Some(tree.id)
        );
    }
    let removed_branch = tree.global([3, 4, 0]);
    let removed_leaf = tree.global([5, 7, 0]);
    loaded.record(removed_branch, 0).unwrap();
    loaded.record(removed_branch, tree.species.wood()).unwrap();
    loaded.record(removed_leaf, 0).unwrap();
    let partial = loaded.gameplay().trees.get(tree.id).unwrap().clone();
    assert_eq!(partial.stage, TreeStage::Generated);
    assert_eq!(partial.fruit, tree.fruit);
    assert!(!partial.wood.contains(&[3, 4, 0]));
    assert!(!partial.leaves.contains(&[5, 7, 0]));
    assert_eq!(tree.bounds.unwrap()[1][0], 5);
    assert_eq!(partial.bounds.unwrap()[1][0], 4);
    assert!(
        loaded
            .gameplay()
            .trees
            .owner(removed_branch, tree.species.wood())
            .is_none()
    );
    assert!(partial.valid());
    save_edits(&directory, world, &loaded).unwrap();
    let mut reloaded = load_edits(&directory, world).unwrap();
    assert_eq!(reloaded.gameplay().trees.get(tree.id), Some(&partial));
    assert_eq!(reloaded.block(removed_branch), Some(tree.species.wood()));
    assert_eq!(reloaded.block(removed_leaf), Some(0));
    assert!(
        reloaded
            .gameplay()
            .trees
            .owner(removed_branch, tree.species.wood())
            .is_none()
    );
    assert_eq!(
        reloaded
            .gameplay()
            .trees
            .owner(tree.root, tree.species.wood())
            .map(|tree| tree.id),
        Some(tree.id)
    );
    reloaded.record(tree.root, 0).unwrap();
    reloaded.record(tree.root, tree.species.wood()).unwrap();
    let felled = reloaded.gameplay().trees.get(tree.id).unwrap().clone();
    assert_eq!(felled.stage, TreeStage::GeneratedFelled);
    assert!(felled.stage.is_generated());
    assert!(felled.fruit.is_none());
    assert!(!felled.wood.is_empty());
    save_edits(&directory, world, &reloaded).unwrap();
    assert_eq!(
        load_edits(&directory, world)
            .unwrap()
            .gameplay()
            .trees
            .get(tree.id),
        Some(&felled)
    );
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn frozen_id11_mature_tag_keeps_legacy_stage_attribution_and_fruit_without_promotion() {
    // Postcard encodes the former three-variant enum as its u32 discriminant.
    // This frozen DTO explicitly writes tag 1 and never knows Generated/tag 3.
    #[derive(Serialize)]
    struct LegacyTreeRecord<'a> {
        id: u64,
        root: [i64; 3],
        species: TreeSpecies,
        seed: u64,
        planted_by: Option<PlayerUuid>,
        stage: u32,
        growth_seconds: f32,
        bounds: Option<[[i8; 3]; 2]>,
        wood: &'a [[i8; 3]],
        leaves: &'a [[i8; 3]],
        fruit: Option<FruitState>,
        revision: u64,
    }
    #[derive(Serialize)]
    struct LegacyRegistry<'a> {
        revision: u64,
        trees: BTreeMap<u64, LegacyTreeRecord<'a>>,
    }
    #[derive(Serialize)]
    struct LegacyIndex<'a> {
        prior: KingdomIndex10,
        trees: LegacyRegistry<'a>,
    }
    let directory = temp();
    let world = WorldId(204);
    let mut journal = EditJournal::default();
    let tree = attributed_apple(202, TreeStage::Mature);
    install_attributed_tree(&mut journal, tree.clone());
    save_edits(&directory, world, &journal).unwrap();
    let current = read(&directory.join(INDEX)).unwrap();
    let index: TreeIndex = decode(b"ID12", &current).unwrap();
    let prior: KingdomIndex10 = index.prior.into();
    let expected = encode(
        b"ID11",
        &TreeIndex11 {
            prior,
            trees: index.trees,
        },
    )
    .unwrap();
    let frozen_index: TreeIndex11 = decode(b"ID11", &expected).unwrap();
    let legacy = LegacyIndex {
        prior: frozen_index.prior,
        trees: LegacyRegistry {
            revision: journal.gameplay().trees.revision,
            trees: BTreeMap::from([(
                tree.id,
                LegacyTreeRecord {
                    id: tree.id,
                    root: tree.root,
                    species: tree.species,
                    seed: tree.seed,
                    planted_by: tree.planted_by,
                    stage: 1,
                    growth_seconds: tree.growth_seconds,
                    bounds: tree.bounds,
                    wood: &tree.wood,
                    leaves: &tree.leaves,
                    fruit: tree.fruit,
                    revision: tree.revision,
                },
            )]),
        },
    };
    let frozen = encode(b"ID11", &legacy).unwrap();
    assert_eq!(
        frozen, expected,
        "appending Generated must preserve the complete legacy ID11 encoding"
    );
    fs::write(directory.join(INDEX), frozen).unwrap();
    let loaded = load_edits(&directory, world).unwrap();
    let restored = loaded.gameplay().trees.get(tree.id).unwrap();
    assert_eq!(restored, &tree);
    assert_eq!(restored.stage, TreeStage::Mature);
    assert_ne!(restored.stage, TreeStage::Generated);
    assert!(restored.stage.is_mature());
    save_edits(&directory, world, &loaded).unwrap();
    assert_eq!(
        load_edits(&directory, world)
            .unwrap()
            .gameplay()
            .trees
            .get(tree.id),
        Some(&tree)
    );
    fs::remove_dir_all(directory).unwrap();
}
