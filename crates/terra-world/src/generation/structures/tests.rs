use super::*;
use crate::{SectionGenerator, edits::EditJournal};
use terra_core::{WorldConfig, WorldSeed};

fn terrain(seed: u64, revision: u8) -> TerraGenerator {
    TerraGenerator::new(WorldConfig {
        seed: WorldSeed(seed),
        structures_revision: revision,
        landscape_revision: 0,
        sea_level: 8.0,
        geology_revision: 0,
        ocean_revision: 0,
        base_height: 16.0,
        hill_amplitude: 0.0,
        mountain_amplitude: 0.0,
        terrace_strength: 0.0,
        caves: false,
        trees: false,
        water: false,
        rivers: false,
        biomes: false,
        ..Default::default()
    })
    .unwrap()
}

fn find(terrain: &TerraGenerator, kind: StructureKind) -> Arc<Site> {
    for z in -3..=2 {
        for x in -3..=2 {
            if let Some(site) = terrain.planned_site([x, z])
                && site.summary.kind == kind
            {
                return site;
            }
        }
    }
    panic!("bounded deterministic search did not find {kind:?}");
}

#[test]
fn new_ruins_have_broken_stone_coping_steps_and_railings_without_changing_saved_revision_two() {
    let old = find(&terrain(42042, 2), StructureKind::Ruin);
    let new = find(&terrain(42042, 3), StructureKind::Ruin);
    assert_eq!(old.summary, new.summary);
    for material in [77, 85, 93] {
        assert!(
            !old.sections
                .values()
                .flat_map(|s| &s.cells)
                .any(|&(_, m)| m == material)
        );
        assert!(
            new.sections
                .values()
                .flat_map(|s| &s.cells)
                .any(|&(_, m)| m == material)
        );
    }
    let objects: Vec<_> = new
        .sections
        .values()
        .flat_map(|s| &s.objects)
        .cloned()
        .collect();
    let chest = objects.iter().find(|o| o.material == 29).unwrap();
    assert_eq!(chest.state.unwrap().facing, 2);
    assert!(
        objects
            .iter()
            .filter(|o| matches!(o.material, 77 | 85 | 93))
            .all(|o| { o.state.unwrap().valid_for(o.position, o.material) })
    );
    let entrance = new.summary.buildings[0].entrance;
    assert!((0..3).all(|dy| new.block_at([entrance[0], entrance[1] + dy, entrance[2]]) == Some(0)));
    let mut journal = EditJournal::default();
    journal.initialize_structures(&objects).unwrap();
    assert!(journal.initialize_structures(&objects).unwrap().is_empty());
    assert!(new.sections.len() <= MAX_SITE_SECTIONS);
}

#[test]
fn regional_structures_are_seeded_bounded_and_legacy_worlds_never_retro_generate() {
    for seed in [1, 42042, 99991] {
        let generator = terrain(seed, 1);
        let repeat = terrain(seed, 1);
        for kind in [
            StructureKind::Village,
            StructureKind::Ruin,
            StructureKind::Mine,
        ] {
            let site = find(&generator, kind);
            let region = [
                site.summary.center[0].div_euclid(REGION_SIZE),
                site.summary.center[2].div_euclid(REGION_SIZE),
            ];
            let other = repeat.planned_site(region).unwrap();
            assert_eq!(site.summary, other.summary);
            assert!(site.sections.len() <= MAX_SITE_SECTIONS);
            assert!(site.sections.values().map(|p| p.cells.len()).sum::<usize>() <= MAX_SITE_CELLS);
            assert_eq!(site.sections.len(), other.sections.len());
            for (p, patch) in &site.sections {
                assert_eq!(patch.cells, other.sections[p].cells);
                assert_eq!(patch.objects, other.sections[p].objects);
                assert!(patch.cells.windows(2).all(|pair| pair[0].0 < pair[1].0));
            }
            assert!(!site.summary.containers.is_empty());
            assert!(terrain(seed, 0).planned_site(region).is_none());
        }
    }
    assert_ne!(
        find(&terrain(1, 1), StructureKind::Village).summary,
        find(&terrain(2, 1), StructureKind::Village).summary
    );
}

#[test]
fn actual_sections_and_halos_match_regional_plan_in_any_order_with_player_edits_last() {
    let terrain = terrain(42042, 1);
    let site = find(&terrain, StructureKind::Village);
    assert!(site.summary.center[0] < 0 && site.summary.center[2] < 0);
    let generator = SectionGenerator::new(terrain);
    let mut owners: Vec<_> = site.sections.keys().copied().collect();
    let indexes = [0, owners.len() / 3, owners.len() * 2 / 3, owners.len() - 1];
    owners = indexes.into_iter().map(|i| owners[i]).collect();
    let mut baseline = BTreeMap::new();
    for owner in &owners {
        let column = generator.prepare_column([owner.x, owner.z], true).unwrap();
        let section = generator
            .build_prepared(*owner, 0, &Default::default(), &column)
            .unwrap();
        for &(index, material) in &site.sections[owner].cells {
            let i = usize::from(index);
            assert_eq!(
                section.data.block([i % 16, i / 16 % 16, i / 256]).unwrap(),
                material
            );
        }
        let origin = owner.origin().unwrap();
        let mut padded = vec![u16::MAX; 18 * 18 * 18];
        site.stamp(origin.map(|v| v - 1), 18, &mut padded);
        for z in [0, 1, 16, 17] {
            for x in [0, 1, 16, 17] {
                let p = [
                    origin[0] + x as i64 - 1,
                    origin[1] + 7,
                    origin[2] + z as i64 - 1,
                ];
                if let Some(material) = site.block_at(p) {
                    assert_eq!(padded[x + 18 * (8 + 18 * z)], material);
                    assert_eq!(generator.block_at(p).unwrap(), material);
                }
            }
        }
        baseline.insert(*owner, section.data);
    }
    owners.reverse();
    for owner in owners {
        let section = generator
            .build(owner, 0, &Default::default(), true)
            .unwrap();
        assert_eq!(section.data, baseline[&owner]);
    }
    let p = site.summary.containers[0];
    let (owner, local) = global_to_section(p);
    let edits = std::collections::HashMap::from([(p, 0)]);
    assert_eq!(
        generator
            .build(owner, 1, &edits, true)
            .unwrap()
            .data
            .block(local)
            .unwrap(),
        0
    );
}

#[test]
fn villages_have_real_paired_furniture_oriented_roofs_farms_and_role_specific_loot() {
    let terrain = terrain(1, 1);
    let site = find(&terrain, StructureKind::Village);
    for role in [
        BuildingRole::Church,
        BuildingRole::Smith,
        BuildingRole::Watch,
        BuildingRole::Medic,
        BuildingRole::House,
        BuildingRole::Farm,
    ] {
        assert!(site.summary.buildings.iter().any(|b| b.role == role));
    }
    let objects: Vec<_> = site
        .sections
        .values()
        .flat_map(|p| p.objects.iter().cloned())
        .collect();
    let mut journal = EditJournal::default();
    assert!(!journal.initialize_structures(&objects).unwrap().is_empty());
    assert!(journal.initialize_structures(&objects).unwrap().is_empty());
    assert!(
        objects
            .iter()
            .any(|o| terra_core::construction::is_stair(o.material) && o.state.is_some())
    );
    for material in [102, 103, 104, 105] {
        assert!(
            site.sections
                .values()
                .any(|patch| patch.cells.iter().any(|&(_, m)| m == material)),
            "material {material} is used by actual village voxels"
        );
    }
    for object in &objects {
        assert_eq!(site.block_at(object.position), Some(object.material));
        if let Some(state) = object.state {
            assert!(state.valid_for(object.position, object.material));
            assert_eq!(journal.object_state(object.position), Some(state));
            if object.material == 59 && !state.secondary {
                assert_eq!(
                    journal.furnace_at(object.position).unwrap().outlet,
                    [
                        object.position[0],
                        object.position[1] + 2,
                        object.position[2]
                    ]
                );
            }
            if let Some(peer) = state.peer {
                let other = journal
                    .object_state(peer)
                    .expect("both halves must be generated");
                assert_eq!(other.peer, Some(object.position));
                assert_ne!(other.secondary, state.secondary);
            }
        }
        if let Some(loot) = &object.loot {
            assert!(!loot.slots.is_empty());
            assert_eq!(journal.container_at(object.position), Some(loot));
        }
        if object.crop.is_some() {
            assert!(journal.crop_at(object.position).unwrap().mature());
        }
    }
    for kind in [
        terra_core::CropKind::Wheat,
        terra_core::CropKind::Corn,
        terra_core::CropKind::Lettuce,
    ] {
        assert!(journal.crops().any(|(_, state)| state.kind == kind));
    }
    let chest = site.summary.containers[0];
    journal.set_container(chest, Default::default()).unwrap();
    assert!(journal.initialize_structures(&objects).unwrap().is_empty());
    assert_eq!(journal.container_at(chest), Some(&Default::default()));
    let other = site.summary.containers[1];
    // Match the furniture removal transaction: empty/remove private contents
    // before recording the confirmed terrain tombstone.
    journal.set_container(other, Default::default()).unwrap();
    assert_eq!(
        journal.remove_container(other).unwrap(),
        Some(Default::default())
    );
    #[cfg(feature = "bevy")]
    {
        use crate::streaming::{BlockEditRequest, ChunkStore, LoadedSection, commit_block_edit};
        let owner = global_to_section(other).0;
        let generator = SectionGenerator::new(terrain.clone());
        let generated = generator
            .build(
                owner,
                journal.revision(owner),
                &journal.snapshot(owner).unwrap(),
                false,
            )
            .unwrap();
        let mut chunks = ChunkStore::default();
        chunks.loaded.insert(
            owner,
            LoadedSection {
                data: generated.data,
                mesh: None,
                mesh_epoch: journal.mesh_epoch(owner),
                water_epoch: 0,
                water: None,
                props: generated.props,
            },
        );
        assert!(
            commit_block_edit(
                &mut chunks,
                &mut journal,
                &BlockEditRequest {
                    position: other,
                    expected_material: 29,
                    material: 0,
                },
                &terra_core::block_properties::BlockPropertiesCatalog::default()
            )
            .unwrap()
        );
        assert_eq!(chunks.block_at(other), Some(0));
    }
    #[cfg(not(feature = "bevy"))]
    journal.record(other, 0).unwrap();
    assert!(journal.initialize_structures(&objects).unwrap().is_empty());
    assert_eq!(journal.block(other), Some(0));
    assert!(journal.container_at(other).is_none());
}

#[test]
fn plaza_paths_reach_every_door_without_being_cut_off_by_the_generated_building_shell() {
    let terrain = terrain(42042, 1);
    let site = find(&terrain, StructureKind::Village);
    let y = site.summary.center[1];
    let start = [site.summary.center[0], y - 1, site.summary.center[2]];
    assert_eq!(site.block_at(start), Some(102));
    let mut seen = std::collections::HashSet::from([start]);
    let mut queue = VecDeque::from([start]);
    while let Some(p) = queue.pop_front() {
        for [dx, dz] in [[1, 0], [-1, 0], [0, 1], [0, -1]] {
            let next = [p[0] + dx, p[1], p[2] + dz];
            if site.block_at(next) == Some(102)
                && site.block_at([next[0], next[1] + 1, next[2]]).unwrap_or(0) == 0
                && site.block_at([next[0], next[1] + 2, next[2]]).unwrap_or(0) == 0
                && seen.insert(next)
            {
                queue.push_back(next);
            }
        }
    }
    for building in &site.summary.buildings {
        if !matches!(building.role, BuildingRole::Farm | BuildingRole::Bridge) {
            let [x, foot, z] = building.entrance;
            assert!(
                seen.contains(&[x, foot - 1, z - 1]),
                "{:?} door must connect to the real walkable plaza path",
                building.role
            );
        }
    }
}

#[test]
fn ordinary_columns_do_not_build_region_plans_and_regional_cache_evicts_deterministically() {
    let terrain = terrain(99991, 1);
    assert!(terrain.structures_for_column([0, 0], 1).is_empty());
    assert!(terrain.structures.0.lock().unwrap().slots.is_empty());
    let first = terrain.planned_site([-3, -3]).map(|s| s.summary.clone());
    for x in 0..CACHE_LIMIT as i64 + 4 {
        terrain.planned_site([x, -7]);
        assert!(terrain.structures.0.lock().unwrap().slots.len() <= CACHE_LIMIT);
    }
    assert!(
        !terrain
            .structures
            .0
            .lock()
            .unwrap()
            .slots
            .contains_key(&[-3, -3])
    );
    assert_eq!(
        terrain.planned_site([-3, -3]).map(|s| s.summary.clone()),
        first
    );
}

#[test]
fn cleared_tree_roots_remove_branched_crowns_outside_the_site_and_adjacent_section_halos() {
    use crate::generation::vegetation::trees_in_column;
    let mut config = terrain(42042, 0).config().clone();
    config.trees = true;
    config.tree_density = 1.0;
    let natural = TerraGenerator::new(config.clone()).unwrap();
    let candidates = trees_in_column(&natural, [176, 0, 176], 32);
    let (tree, cell) = candidates
        .iter()
        .find_map(|tree| {
            let owner = global_to_section(tree.origin).0;
            for z in tree.bounds.min[2]..tree.bounds.max[2] {
                for y in tree.bounds.min[1]..tree.bounds.max[1] {
                    for x in tree.bounds.min[0]..tree.bounds.max[0] {
                        let p = [
                            tree.origin[0] + i64::from(x),
                            tree.origin[1] + i64::from(y),
                            tree.origin[2] + i64::from(z),
                        ];
                        let other = global_to_section(p).0;
                        if [owner.x, owner.z] != [other.x, other.z] && tree.leaves_at(p).is_some() {
                            return Some((tree, p));
                        }
                    }
                }
            }
            None
        })
        .expect("actual branched tree crosses a horizontal section boundary");
    config.structures_revision = 1;
    let terrain = TerraGenerator::new(config).unwrap();
    let p = tree.origin;
    let (owner, local) = global_to_section(p);
    let summary = SiteSummary {
        id: 1,
        kind: StructureKind::Village,
        style: VillageStyle::Temperate,
        center: p,
        bounds: [p, p],
        buildings: Vec::new(),
        containers: Vec::new(),
    };
    let site = Arc::new(Site {
        summary,
        settlement: None,
        sections: BTreeMap::from([(
            owner,
            StructurePatch {
                cells: vec![((local[0] + 16 * (local[1] + 16 * local[2])) as u16, 0)],
                objects: Vec::new(),
            },
        )]),
    });
    let slot = Arc::new(OnceLock::new());
    slot.set(Some(site)).unwrap();
    terrain.structures.0.lock().unwrap().slots.insert(
        [p[0].div_euclid(REGION_SIZE), p[2].div_euclid(REGION_SIZE)],
        slot,
    );
    assert!(terrain.structure_clears_tree(p));
    let generator = SectionGenerator::new(terrain);
    let (owner, local) = global_to_section(cell);
    let prepared = generator.prepare_column([owner.x, owner.z], true).unwrap();
    assert!(prepared.trees.iter().all(|t| t.origin != p));
    let meshed = generator
        .build_prepared(owner, 0, &Default::default(), &prepared)
        .unwrap();
    let headless = generator
        .build(owner, 0, &Default::default(), false)
        .unwrap();
    assert_eq!(headless.data, meshed.data);
    assert_eq!(headless.props, meshed.props);
    assert_eq!(
        meshed.data.block(local).unwrap(),
        generator.block_at(cell).unwrap()
    );
}

#[test]
fn church_clearance_removes_nearby_overhanging_branched_crowns_but_preserves_distant_trees() {
    use crate::generation::vegetation::Tree;
    let terrain = terrain(42042, 1);
    let site = find(&terrain, StructureKind::Village);
    let church = site
        .summary
        .buildings
        .iter()
        .find(|b| b.role == BuildingRole::Church)
        .unwrap();
    let root = [
        church.bounds[0][0] - 5,
        site.summary.center[1],
        (church.bounds[0][2] + church.bounds[1][2]) / 2,
    ];
    assert!(root[0] < church.bounds[0][0] - 2);
    assert!(root[0] >= church.bounds[0][0] - BUILDING_TREE_CLEARANCE);
    assert!(
        site.block_at(root).is_none(),
        "the root itself is outside the stamped building, not already cut by its voxel plan"
    );
    let overlapping = (0..256).find_map(|seed| {
        let tree = Tree::new(root,seed,[0.0;4]);
        if tree.wood.len()<2 || tree.wood_material as u16 !=6 {return None;}
        for z in tree.bounds.min[2]..tree.bounds.max[2] {
            for y in tree.bounds.min[1]..tree.bounds.max[1] {
                for x in tree.bounds.min[0]..tree.bounds.max[0] {
                    let p = [root[0]+i64::from(x),root[1]+i64::from(y),root[2]+i64::from(z)];
                    if tree.leaves_at(p).is_some() && site.block_at(p).is_some_and(|m|m!=0) {
                        return Some((tree,p));
                    }
                }
            }
        }
        None
    }).expect("actual branched oak crown intersects the generated church roof or facade from outside the former2m clearance");
    assert!(site.clears_tree(overlapping.0.origin));
    assert!(terrain.structure_clears_tree(overlapping.0.origin));
    assert!(overlapping.0.leaves_at(overlapping.1).is_some());
    assert!(site.block_at(overlapping.1).is_some_and(|m| m != 0));
    let unrelated = [site.summary.bounds[0][0] - 32, root[1], root[2]];
    assert!(!site.clears_tree(unrelated));
    assert!(!terrain.structure_clears_tree(unrelated));
}

#[test]
fn building_tree_clearance_queries_the_full_band_across_a_section_boundary() {
    let terrain = terrain(42042, 1);
    let root = [111, 16, 176];
    let bounds = [[119, 16, 176], [123, 28, 180]];
    let site = Arc::new(Site {
        settlement: None,
        summary: SiteSummary {
            id: 1,
            kind: StructureKind::Village,
            style: VillageStyle::Temperate,
            center: bounds[0],
            bounds,
            buildings: vec![BuildingSummary {
                role: BuildingRole::Church,
                entrance: bounds[0],
                bounds,
            }],
            containers: Vec::new(),
        },
        sections: BTreeMap::new(),
    });
    let slot = Arc::new(OnceLock::new());
    slot.set(Some(site)).unwrap();
    terrain
        .structures
        .0
        .lock()
        .unwrap()
        .slots
        .insert([0, 0], slot);
    assert!(
        terrain.structure_clears_tree(root),
        "root is in the previous section and the site begins8m beyond its last cell"
    );
    assert!(!terrain.structure_clears_tree([110, 16, 176]));
}

#[test]
fn per_kind_generation_switches_preserve_other_seeded_sites_in_every_contract() {
    for revision in [1, 2, 5] {
        let original = terrain(42042, revision);
        let mut sites = vec![
            find(&original, StructureKind::Village),
            find(&original, StructureKind::Ruin),
        ];
        // Current mines need a real hillside. Flat terrain only has the
        // revision-one descending-mine contract, covered separately below.
        if revision == 1 {
            sites.push(find(&original, StructureKind::Mine));
        }
        for disabled in [
            StructureKind::Village,
            StructureKind::Ruin,
            StructureKind::Mine,
        ] {
            let mut config = original.config().clone();
            match disabled {
                StructureKind::Village | StructureKind::Kingdom | StructureKind::Outpost => {
                    config.structures.villages = false
                }
                StructureKind::Ruin => config.structures.ruins = false,
                StructureKind::Mine => config.structures.mines = false,
            }
            let filtered = TerraGenerator::new(config).unwrap();
            for site in &sites {
                let region = [site.summary.center[0], site.summary.center[2]]
                    .map(|coordinate| coordinate.div_euclid(REGION_SIZE));
                if site.summary.kind == disabled {
                    assert!(
                        filtered.planned_site(region).is_none(),
                        "disabled {disabled:?} was generated"
                    );
                    assert!(filtered.structure_site(region).is_none());
                    assert!(filtered.village_residents(region).is_empty());
                } else {
                    let unchanged = filtered.planned_site(region).unwrap();
                    assert_eq!(unchanged.summary, site.summary);
                    assert_eq!(unchanged.sections.len(), site.sections.len());
                    for (position, patch) in &site.sections {
                        assert_eq!(unchanged.sections[position].cells, patch.cells);
                        assert_eq!(unchanged.sections[position].objects, patch.objects);
                    }
                }
            }
        }
    }
}

#[test]
fn all_structures_off_has_no_plans_objects_residents_or_stamped_voxels() {
    let original = terrain(42042, 5);
    let village = find(&original, StructureKind::Village);
    let region = [village.summary.center[0], village.summary.center[2]]
        .map(|coordinate| coordinate.div_euclid(REGION_SIZE));
    let owner = global_to_section(village.summary.center).0;
    let mut config = original.config().clone();
    config.structures.set_all(false);
    let disabled = TerraGenerator::new(config.clone()).unwrap();
    assert!(disabled.structure_site(region).is_none());
    assert!(disabled.village_residents(region).is_empty());
    assert!(
        disabled
            .structures_for_column([owner.x, owner.z], 1)
            .is_empty()
    );
    assert!(disabled.structure_block(village.summary.center).is_none());
    let actual = SectionGenerator::new(disabled)
        .build(owner, 0, &Default::default(), false)
        .unwrap();
    config.structures_revision = 0;
    let expected = SectionGenerator::new(TerraGenerator::new(config).unwrap())
        .build(owner, 0, &Default::default(), false)
        .unwrap();
    assert_eq!(actual.data, expected.data);
    assert_eq!(actual.props, expected.props);
}

#[test]
fn current_mountain_mines_honor_the_individual_generation_switch() {
    for revision in [2, 5] {
        let mut config = terrain(42042, revision).config().clone();
        config.base_height = 72.0;
        config.hill_amplitude = 180.0;
        let original = TerraGenerator::new(config.clone()).unwrap();
        let (region, mine) = (-5..=5)
            .flat_map(|z| (-5..=5).map(move |x| [x, z]))
            .filter_map(|region| original.planned_site(region).map(|site| (region, site)))
            .find(|(_, site)| site.summary.kind == StructureKind::Mine)
            .unwrap_or_else(|| panic!("revision {revision} needs a real mountainous mine fixture"));
        config.structures.mines = false;
        let filtered = TerraGenerator::new(config.clone()).unwrap();
        assert!(filtered.planned_site(region).is_none());
        assert!(filtered.structure_block(mine.summary.center).is_none());
        // Disabling villages or ruins must leave this mine byte-for-byte intact.
        config.structures.mines = true;
        config.structures.villages = false;
        config.structures.ruins = false;
        let mine_only = TerraGenerator::new(config)
            .unwrap()
            .planned_site(region)
            .unwrap();
        assert_eq!(mine_only.summary, mine.summary);
        for (position, patch) in &mine.sections {
            assert_eq!(mine_only.sections[position].cells, patch.cells);
            assert_eq!(mine_only.sections[position].objects, patch.objects);
        }
    }
}
