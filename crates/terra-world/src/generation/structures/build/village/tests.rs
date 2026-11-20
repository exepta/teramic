use super::*;
use std::collections::{BTreeSet, HashSet, VecDeque};
use terra_core::{WorldConfig, WorldSeed};

fn terrain(revision: u8) -> TerraGenerator {
    TerraGenerator::new(WorldConfig {
        seed: WorldSeed(42042),
        structures_revision: revision,
        landscape_revision: 0,
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

fn builder(terrain: &TerraGenerator, seed: u32) -> Builder<'_> {
    Builder {
        terrain,
        summary: SiteSummary {
            id: u64::from(seed),
            kind: StructureKind::Village,
            style: VillageStyle::Temperate,
            center: [0, 16, 0],
            bounds: [[0; 3]; 2],
            buildings: Vec::new(),
            containers: Vec::new(),
            grave: None,
        },
        patches: BTreeMap::new(),
        objects: BTreeMap::new(),
        writes: 0,
        failed: false,
        seed,
    }
}

fn objects(site: &Site) -> impl Iterator<Item = &StructureObject> {
    site.sections.values().flat_map(|p| &p.objects)
}

#[test]
fn furnished_houses_keep_authored_machine_fronts_and_peer_axes_in_all_rotations_and_styles() {
    use terra_core::{ItemKind, ResourcePropKind, beds::DyeColor, flower_pot, textiles};
    for style in [
        VillageStyle::Temperate,
        VillageStyle::Desert,
        VillageStyle::Highland,
    ] {
        for facing in 0..4 {
            for role in [
                BuildingRole::House,
                BuildingRole::Medic,
                BuildingRole::Smith,
                BuildingRole::Church,
                BuildingRole::Watch,
            ] {
                let terrain = terrain(3);
                let plot = Plot {
                    center: [-32, -48],
                    width: 10,
                    depth: 10,
                    height: 5,
                    facing,
                    floor: 16,
                    role,
                    variant: 17,
                };
                let mut builder = builder(&terrain, 1);
                builder.summary.style = style;
                buildings::build(&mut builder, &plot);
                let site = builder.finish().unwrap();
                let carpets: Vec<_> = site
                    .sections
                    .values()
                    .flat_map(|p| &p.cells)
                    .filter(|(_, m)| textiles::is_carpet(*m))
                    .collect();
                assert!(!carpets.is_empty());
                assert!(carpets.len() <= 16, "furniture stays bounded");
                let pots: Vec<_> = objects(&site)
                    .filter(|o| o.material == flower_pot::MATERIAL)
                    .collect();
                assert_eq!(pots.len(), 2);
                for pot in pots {
                    let expected = if pot.position == plot.point(plot.width - 2, 1, 2) {
                        ItemKind::Flower(match style {
                            VillageStyle::Temperate => DyeColor::Red,
                            VillageStyle::Desert => DyeColor::Yellow,
                            VillageStyle::Highland => DyeColor::White,
                        })
                    } else if role == BuildingRole::Medic {
                        ItemKind::Prop(ResourcePropKind::Mushroom)
                    } else {
                        ItemKind::Flower(DyeColor::Purple)
                    };
                    assert_eq!(
                        flower_pot::contents(pot.state.unwrap().facing),
                        Some(expected),
                        "rotation preserves the plant identity: {role:?} {pot:?}"
                    );
                    let support = [pot.position[0], pot.position[1] - 1, pot.position[2]];
                    assert_eq!(site.block_at(support), Some(43));
                    assert_eq!(pot.position[1], plot.floor + 1);
                }
                for object in objects(&site) {
                    if let Some(state) = object.state {
                        assert!(
                            state.valid_for(object.position, object.material),
                            "{object:?}"
                        );
                        if matches!(object.material, 28 | 29 | 59) {
                            let local_facing = if object.material == 29
                                && matches!(role, BuildingRole::Church | BuildingRole::Watch)
                            {
                                0
                            } else {
                                2
                            };
                            assert_eq!(
                                state.facing,
                                (facing + local_facing) % 4,
                                "front faces the room: {role:?} {object:?}"
                            );
                        }
                        if object.material == 28 {
                            let peer = state.peer.unwrap();
                            assert_eq!(
                                peer,
                                std::array::from_fn(|axis| {
                                    object.position[axis]
                                        + state.right()[axis] * if state.secondary { -1 } else { 1 }
                                }),
                                "rendered two-cell bench and saved footprint must agree"
                            );
                        }
                    }
                }
                let batch: Vec<_> = objects(&site).cloned().collect();
                let mut journal = crate::edits::EditJournal::default();
                journal.initialize_structures(&batch).unwrap();
                assert!(journal.initialize_structures(&batch).unwrap().is_empty());
                let deltas: Vec<_> = journal
                    .changed_sections()
                    .map(|p| journal.section_delta(p))
                    .collect();
                let mut restored = crate::edits::EditJournal::default();
                for delta in deltas {
                    restored.import_delta(delta).unwrap();
                }
                assert!(restored.initialize_structures(&batch).unwrap().is_empty());
                for object in batch.iter().filter(|o| o.material == flower_pot::MATERIAL) {
                    assert_eq!(restored.object_state(object.position), object.state);
                }
            }
        }
    }
}

#[test]
fn revision_two_keeps_its_existing_furniture_and_new_decor_does_not_overwrite_tombstones() {
    let old = terrain(2);
    let new = terrain(3);
    let legacy = plan(&old, [-1, -2], 1).unwrap();
    let furnished = plan(&new, [-1, -2], 1).unwrap();
    assert_eq!(
        legacy.summary, furnished.summary,
        "site IDs, roads, bounds and entrances are stable"
    );
    assert!(
        !legacy
            .sections
            .values()
            .flat_map(|p| &p.cells)
            .any(|(_, m)| (106..=131).contains(m))
    );
    assert!(objects(&legacy).filter(|o| o.material == 59).all(|o| {
        let state = o.state.unwrap();
        legacy
            .summary
            .buildings
            .iter()
            .find(|b| b.role == BuildingRole::Smith)
            .is_some_and(|b| {
                let door = objects(&legacy).find(|o| o.position == b.entrance).unwrap();
                door.state.unwrap().facing == state.facing
            })
    }));
    let batch: Vec<_> = objects(&furnished).cloned().collect();
    let pot = batch
        .iter()
        .find(|o| o.material == terra_core::flower_pot::MATERIAL)
        .unwrap();
    let mut journal = crate::edits::EditJournal::default();
    journal.record(pot.position, 0).unwrap();
    journal.initialize_structures(&batch).unwrap();
    assert_eq!(journal.block(pot.position), Some(0));
    assert_eq!(journal.object_state(pot.position), None);
}

#[test]
fn seeded_street_layouts_have_reachable_oriented_roles_and_preserve_revision_one() {
    let generator = terrain(2);
    let mut layouts = BTreeSet::new();
    for seed in [1, 65538, 131076] {
        let site = plan(&generator, [-1, -2], seed).expect("flat village must fit");
        let repeat = plan(&generator, [-1, -2], seed).unwrap();
        assert_eq!(site.summary, repeat.summary);
        for (owner, patch) in &site.sections {
            assert_eq!(patch.cells, repeat.sections[owner].cells);
            assert_eq!(patch.objects, repeat.sections[owner].objects);
        }
        layouts.insert(
            site.summary
                .buildings
                .iter()
                .map(|b| (b.role as u8, b.entrance))
                .collect::<Vec<_>>(),
        );
        for role in [
            BuildingRole::House,
            BuildingRole::Church,
            BuildingRole::Smith,
            BuildingRole::Medic,
            BuildingRole::Watch,
            BuildingRole::Farm,
        ] {
            assert!(site.summary.buildings.iter().any(|b| b.role == role));
        }
        let facings: BTreeSet<_> = objects(&site)
            .filter(|o| (65..=67).contains(&o.material))
            .filter_map(|o| o.state.map(|s| s.facing))
            .collect();
        assert!(facings.len() >= 3, "facades face their real street");
        assert!(site.sections.len() <= MAX_SITE_SECTIONS);
        assert!(site.sections.values().map(|p| p.cells.len()).sum::<usize>() <= MAX_SITE_CELLS);
        for axis in [0, 2] {
            let region = if axis == 0 { -1 } else { -2 };
            assert!(site.summary.bounds[0][axis] >= region * REGION_SIZE + 96);
            assert!(site.summary.bounds[1][axis] <= region * REGION_SIZE + 288);
        }
        for object in objects(&site) {
            if let Some(state) = object.state {
                assert!(
                    state.valid_for(object.position, object.material),
                    "{object:?}"
                );
                if let Some(peer) = state.peer {
                    let other = objects(&site).find(|o| o.position == peer).unwrap();
                    assert_eq!(other.state.unwrap().peer, Some(object.position));
                }
            }
        }
        assert_connected_streets(&site);
    }
    assert_eq!(layouts.len(), 3);
    let old = terrain(1);
    for seed in [1, 65538, 131076] {
        let delegated = super::super::plan(&old, [-1, -2], seed).unwrap();
        let legacy = super::super::plan_legacy(&old, [-1, -2], seed).unwrap();
        assert_eq!(delegated.summary, legacy.summary);
        for (p, patch) in &legacy.sections {
            assert_eq!(delegated.sections[p].cells, patch.cells);
            assert_eq!(delegated.sections[p].objects, patch.objects);
        }
    }
}

fn assert_connected_streets(site: &Site) {
    let start = [site.summary.center[0], site.summary.center[2]];
    let walkable = |p: [i64; 2]| {
        matches!(site.block_at([p[0], 15, p[1]]), Some(78 | 86 | 102))
            && (16..=17).all(|y| site.block_at([p[0], y, p[1]]).is_none_or(|m| m == 0))
    };
    assert!(walkable(start));
    let mut seen = HashSet::from([start]);
    let mut pending = VecDeque::from([start]);
    while let Some(p) = pending.pop_front() {
        for [x, z] in [[1, 0], [-1, 0], [0, 1], [0, -1]] {
            let next = [p[0] + x, p[1] + z];
            if walkable(next) && seen.insert(next) {
                pending.push_back(next);
            }
        }
    }
    for door in objects(site).filter(|o| (65..=67).contains(&o.material)) {
        let state = door.state.unwrap();
        if state.secondary {
            continue;
        }
        let [dx, dz] = rotate([0, -4], state.facing);
        let portal = [door.position[0] + dx, door.position[2] + dz];
        assert!(
            seen.contains(&portal),
            "street must reach {door:?}, portal {portal:?}"
        );
    }
}

#[test]
fn rotated_modern_roofs_and_tower_treads_face_the_actual_rise() {
    let terrain = terrain(0);
    for role in [BuildingRole::Church, BuildingRole::Watch] {
        for facing in 0..4 {
            let plot = Plot {
                center: [0, 0],
                width: 9,
                depth: 9,
                height: 5,
                facing,
                floor: 16,
                role,
                variant: 1,
            };
            let mut builder = builder(&terrain, 1);
            buildings::build(&mut builder, &plot);
            let site = builder.finish().unwrap();
            for (x, local_facing) in [(-1, 1), (10, 3)] {
                let p = plot.point(x, plot.height, -1);
                let roof = objects(&site).find(|o| o.position == p).unwrap();
                assert_eq!(
                    roof.material,
                    if role == BuildingRole::Watch { 77 } else { 74 }
                );
                assert_eq!(roof.state.unwrap().facing, (local_facing + facing) % 4);
                let eave = plot.point(x, plot.height - 1, -1);
                assert_eq!(
                    objects(&site)
                        .find(|o| o.position == eave)
                        .unwrap()
                        .state
                        .unwrap()
                        .facing,
                    4
                );
            }
            let tower = [plot.width - 3, plot.depth - 3];
            let treads = plot.height + 6 + if role == BuildingRole::Watch { 2 } else { 0 };
            for level in 0..treads {
                let (dx, dz, next_x, next_z) = match level % 4 {
                    0 => (0, 0, 1, 0),
                    1 => (1, 0, 1, 1),
                    2 => (1, 1, 0, 1),
                    _ => (0, 1, 0, 0),
                };
                let p = plot.point(tower[0] + dx, level, tower[1] + dz);
                let tread = objects(&site).find(|o| o.position == p).unwrap();
                let expected = rotate([next_x - dx, next_z - dz], facing);
                let high = terra_core::construction::stair_basis(tread.state.unwrap().facing)[2];
                assert_eq!([i64::from(high[0]), i64::from(high[2])], expected);
            }
        }
    }
}

#[test]
fn graded_street_crossings_cannot_overwrite_an_existing_tread_or_side_lane() {
    let terrain = terrain(0);
    let mut builder = builder(&terrain, 1);
    let mut roads = paths::Routes::new([0, 0], &[], [[1000, 1000], [1001, 1001]]);
    roads.connect(&mut builder, [9, 18, 0]).unwrap();
    let before = builder.patches.clone();
    let shapes = builder.objects.clone();
    roads.connect(&mut builder, [8, 16, 5]).unwrap();
    for (owner, patch) in before {
        for (i, &material) in patch.iter().enumerate().filter(|(_, m)| **m != u16::MAX) {
            assert_eq!(
                builder.patches[&owner][i], material,
                "branch cannot erase prior stairs or clearance"
            );
        }
    }
    for (p, shape) in shapes {
        assert_eq!(builder.objects.get(&p), Some(&shape));
    }

    let mut builder = self::builder(&terrain, 1);
    let mut roads = paths::Routes::new([0, 0], &[], [[1000, 1000], [1001, 1001]]);
    roads.connect(&mut builder, [9, 16, 0]).unwrap();
    roads.connect(&mut builder, [5, 16, 8]).unwrap();
    assert_eq!(builder.finish().unwrap().block_at([5, 16, 0]), Some(0));
}

#[test]
fn clamped_bridge_decks_reject_submerged_origins_before_emitting_a_segment() {
    let terrain = TerraGenerator::new(WorldConfig {
        base_height: 0.0,
        sea_level: 8.0,
        water: true,
        ..terrain(0).config().clone()
    })
    .unwrap();
    let mut builder = builder(&terrain, 1);
    assert_eq!(builder.ground(0, 0), 0);
    assert_eq!(terrain.water_column(0.5, 0.5).unwrap().level, 8.0);
    let mut roads = paths::Routes::new([0, 0], &[], [[1000, 1000], [1001, 1001]]);
    assert_eq!(roads.connect(&mut builder, [20, 9, 0]), None);
    assert!(builder.patches.is_empty());
    assert!(builder.objects.is_empty());
}

#[test]
fn bridge_clearance_uses_full_deck_top_and_each_lanes_own_water_level() {
    assert!(paths::bridge_clearance(8, Some(8.0), [Some(8.0); 3]));
    assert!(paths::bridge_clearance(8, None, [None, Some(8.9), None]));
    assert!(!paths::bridge_clearance(8, Some(9.0), [None; 3]));
    assert!(!paths::bridge_clearance(-1, Some(8.0), [Some(8.0); 3]));
    for lane in 0..3 {
        for water in [9.0, 9.25] {
            let mut levels = [Some(8.0); 3];
            levels[lane] = Some(water);
            assert!(
                !paths::bridge_clearance(8, Some(8.0), levels),
                "lane {lane} cannot be flooded at water level {water}"
            );
        }
    }
}

#[test]
fn revision_four_terrain_preserves_all_village_styles_for_three_real_seeds() {
    for seed in [42042, 1337, 99173] {
        assert_village_styles(
            WorldConfig {
                seed: WorldSeed(seed),
                landscape_revision: 4,
                sea_level: 22.0,
                rivers: true,
                biome_definitions: Some(terra_core::BiomeDefinitions::default()),
                structures_revision: 2,
                ..Default::default()
            },
            12,
        );
    }
}

#[test]
fn default_terrain_discovers_all_village_styles_for_three_real_seeds() {
    for seed in [42042, 1337, 99173] {
        assert_village_styles(
            WorldConfig {
                seed: WorldSeed(seed),
                ..Default::default()
            },
            32,
        );
    }
}

fn assert_village_styles(config: WorldConfig, radius_limit: i64) {
    let seed = config.seed.0;
    let revision = config.landscape_revision;
    let terrain = TerraGenerator::new(config).unwrap();
    let mut found = BTreeSet::new();
    'regions: for radius in 0..=radius_limit {
        for z in -radius..=radius {
            for x in -radius..=radius {
                if radius > 0 && x.abs() != radius && z.abs() != radius {
                    continue;
                }
                if let Some(site) = terrain.structure_site([x, z])
                    && site.kind == StructureKind::Village
                {
                    found.insert(site.style as u8);
                }
                if found.len() == 3 {
                    break 'regions;
                }
            }
        }
    }
    assert_eq!(
        found.len(),
        3,
        "actual terrain revision={revision} seed={seed}: found={found:?}"
    );
}

#[test]
fn dirt_path_steps_face_uphill_for_all_axes_and_both_height_directions() {
    let terrain = terrain(0);
    for [dx, dz] in [[1, 0], [-1, 0], [0, 1], [0, -1]] {
        for height in [14, 18] {
            let mut builder = builder(&terrain, 1);
            let mut roads = paths::Routes::new([0, 0], &[], [[1000, 1000], [1001, 1001]]);
            roads
                .connect(&mut builder, [dx * 9, height, dz * 9])
                .unwrap();
            let site = builder.finish().unwrap();
            let stairs: Vec<_> = objects(&site).filter(|o| o.material == 78).collect();
            assert!(!stairs.is_empty());
            assert!(objects(&site).any(|o| o.material == 86));
            for stair in stairs {
                let basis = terra_core::construction::stair_basis(stair.state.unwrap().facing)[2];
                let sign = if height < 16 { -1 } else { 1 };
                assert_eq!(
                    [i64::from(basis[0]), i64::from(basis[2])],
                    [dx * sign, dz * sign]
                );
                assert_eq!(
                    site.block_at([stair.position[0], stair.position[1] + 1, stair.position[2]]),
                    Some(0)
                );
            }
        }
    }
}

#[test]
fn modern_gables_have_in_plane_framed_attic_windows_without_replacing_the_roof() {
    let terrain = terrain(0);
    for style in [VillageStyle::Temperate, VillageStyle::Highland] {
        for facing in 0..4 {
            let plot = Plot {
                center: [0, 0],
                width: 9,
                depth: 9,
                height: 5,
                facing,
                floor: 16,
                role: BuildingRole::House,
                variant: 1,
            };
            let mut builder = builder(&terrain, 1);
            builder.summary.style = style;
            buildings::build(&mut builder, &plot);
            let site = builder.finish().unwrap();
            let wood = if style == VillageStyle::Highland {
                18
            } else {
                16
            };
            let window = if style == VillageStyle::Highland {
                70
            } else {
                68
            };
            let stairs = if style == VillageStyle::Highland {
                76
            } else {
                74
            };
            for z in [0, plot.depth] {
                for y in [plot.height + 1, plot.height + 2] {
                    let p = plot.point(4, y, z);
                    let attic = objects(&site).find(|o| o.position == p).unwrap();
                    assert_eq!(attic.material, window);
                    assert_eq!(attic.state.unwrap().facing, facing);
                    for x in [3, 5] {
                        assert_eq!(site.block_at(plot.point(x, y, z)), Some(wood));
                    }
                }
                assert_eq!(site.block_at(plot.point(4, plot.height + 4, z)), Some(wood));
                let roof = objects(&site)
                    .find(|o| o.position == plot.point(4, plot.height + 5, z))
                    .unwrap();
                assert_eq!(roof.material, stairs);
                assert_eq!(roof.state.unwrap().facing, (1 + facing) % 4);
            }
            let entrance = plot.point(4, 0, 0);
            assert_eq!(
                site.block_at(entrance),
                Some(if style == VillageStyle::Highland {
                    67
                } else {
                    65
                })
            );
        }
    }
}
