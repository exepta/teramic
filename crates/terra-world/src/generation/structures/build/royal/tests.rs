use super::*;
use terra_core::{WorldConfig, WorldSeed};

fn terrain(winter: bool, revision: u8) -> TerraGenerator {
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
        snow_height: if winter { 8.0 } else { 112.0 },
        sea_level: 0.0,
        ..Default::default()
    })
    .unwrap()
}
fn objects(site: &Site) -> impl Iterator<Item = &StructureObject> {
    site.sections.values().flat_map(|s| &s.objects)
}

#[test]
fn revision_seven_stronghold_toggles_are_independent_and_do_not_reroll_disabled_sites() {
    for (kingdoms, outposts) in [(false, false), (true, false), (false, true), (true, true)] {
        let mut config = terrain(false, 7).config().clone();
        config.structures.villages = false;
        config.structures.kingdoms = kingdoms;
        config.structures.outposts = outposts;
        let generator = TerraGenerator::new(config).unwrap();
        for (seed, enabled, kind) in [
            (522, kingdoms, StructureKind::Kingdom),
            (261, outposts, StructureKind::Outpost),
        ] {
            let site = super::super::plan(&generator, [0, 0], seed);
            assert_eq!(site.is_some(), enabled);
            if let Some(site) = site {
                assert_eq!(site.summary.kind, kind);
            }
        }
    }
    let mut legacy = terrain(false, 6).config().clone();
    legacy.structures.kingdoms = false;
    legacy.structures.outposts = false;
    let legacy = TerraGenerator::new(legacy).unwrap();
    // The revision-six generator still reads its original villages flag.
    assert!(plan(&legacy, [0, 0], 522).is_some());
    assert!(plan(&legacy, [0, 0], 261).is_some());
}

#[test]
fn revision_seven_doors_fill_real_jambs_with_closed_leaves_across_both_wall_axes() {
    for (seed, winter) in [(522, false), (522, true), (261, false)] {
        let site = plan(&terrain(winter, 7), [0, 0], seed).unwrap();
        for door in objects(&site)
            .filter(|o| o.material == 65 && o.state.is_some_and(|state| !state.secondary))
        {
            let state = door.state.unwrap();
            assert!(state.facing < 4, "A generated door starts closed");
            let [x, y, z] = door.position;
            let tangent = if state.facing.is_multiple_of(2) {
                [1, 0, 0]
            } else {
                [0, 0, 1]
            };
            // The asset is authored across X and thin along Z. Its rotated
            // physical leaf must span the tangent of the actual wall opening.
            let leaf = terra_core::block_shapes::builtin_boxes(65)
                .iter()
                .find(|shape| shape.hinged)
                .unwrap()
                .rotated_y(state.facing);
            let across = if tangent[0] == 1 { 0 } else { 2 };
            let normal = if across == 0 { 2 } else { 0 };
            assert!(leaf.maximum[across] - leaf.minimum[across] > 0.7);
            assert!(leaf.maximum[normal] - leaf.minimum[normal] < 0.04);
            for sign in [-1, 1] {
                let jamb = [x + tangent[0] * sign, y, z + tangent[2] * sign];
                assert!(
                    site.block_at(jamb).is_some_and(|m| !matches!(m, 0 | 27)),
                    "Door {door:?} floats beside an unfilled opening at {jamb:?}"
                );
            }
            assert!(site.block_at([x, y + 2, z]).is_some_and(|m| m != 0));
            assert_eq!(site.block_at([x, y + 1, z]), Some(65));
            let upper = objects(&site)
                .find(|o| o.position == [x, y + 1, z])
                .unwrap();
            assert_eq!(upper.state.unwrap().peer, Some(door.position));
        }
        assert!(site.sections.len() <= MAX_SITE_SECTIONS);
        assert!(site.sections.values().map(|s| s.cells.len()).sum::<usize>() <= MAX_SITE_CELLS);
    }
}

#[test]
fn revision_seven_hillside_fields_keep_natural_heights_and_perimeters_get_grass_slopes() {
    let generator = TerraGenerator::new(WorldConfig {
        seed: WorldSeed(42042),
        structures_revision: 7,
        // Fixed hillside sites exercise the original landscape-one contract.
        landscape_revision: 1,
        ecology_revision: 4,
        river_terrain_revision: 4,
        spirit_blossom_revision: 0,
        blood_forest_revision: 0,
        ..Default::default()
    })
    .unwrap();
    let kingdom = generator.planned_site([-1, -1]).unwrap();
    let origin = [-192, -192];
    let field = kingdom
        .summary
        .buildings
        .iter()
        .find(|b| b.role == BuildingRole::Farm && b.bounds[1][2] > origin[1] + 85)
        .unwrap();
    let mut elevations = std::collections::BTreeSet::new();
    for crop in objects(&kingdom).filter(|o| {
        o.crop.is_some()
            && [0, 2].into_iter().all(|axis| {
                (field.bounds[0][axis]..=field.bounds[1][axis]).contains(&o.position[axis])
            })
    }) {
        let [x, y, z] = crop.position;
        let natural =
            (generator.surface_height(x as f64 + 0.5, z as f64 + 0.5) - 0.5).floor() as i64;
        assert_eq!(y, natural, "A field must not excavate its hillside");
        assert_eq!(kingdom.block_at([x, y + 1, z]), Some(0));
        elevations.insert(y);
    }
    assert!(
        elevations.len() > 1,
        "Exercise a real sloping planted field"
    );
    let outpost = generator.planned_site([1, 4]).unwrap();
    let [cx, cz] = [576, 1728];
    let base =
        (generator.surface_height(cx as f64 + 0.5, cz as f64 + 0.5) - 0.5).floor() as i64 + 1;
    let mut previous = base;
    let mut changed = false;
    // West of the palisade, clear of the gate road and occupied buildings.
    for dx in 31..=55 {
        let x = cx - dx;
        let natural =
            (generator.surface_height(x as f64 + 0.5, cz as f64 + 0.5) - 0.5).floor() as i64 + 1;
        let y = (base - 30..=base + 30)
            .rev()
            .find(|y| outpost.block_at([x, *y - 1, cz]) == Some(1))
            .unwrap_or(natural);
        assert!(
            y.abs_diff(previous) <= 1,
            "No vertical platform wall at x={x}"
        );
        if y != natural {
            assert_eq!(outpost.block_at([x, y - 1, cz]), Some(1));
            assert_eq!(outpost.block_at([x, y, cz]), Some(0));
        }
        changed |= y != base;
        previous = y;
    }
    assert!(
        changed,
        "Exercise a sloped transition into the actual hillside"
    );
    for site in [&kingdom, &outpost] {
        for axis in [0, 2] {
            let local_origin = if site.summary.kind == StructureKind::Outpost {
                if axis == 0 { 576 } else { 1728 }
            } else {
                -192
            };
            assert!(site.summary.bounds[0][axis] >= local_origin - 96);
            assert!(site.summary.bounds[1][axis] <= local_origin + 96);
        }
        assert!(site.sections.len() <= MAX_SITE_SECTIONS);
        assert!(site.sections.values().map(|s| s.cells.len()).sum::<usize>() <= MAX_SITE_CELLS);
    }
}

#[test]
fn authored_strongholds_have_exact_seeded_initial_populations_real_beds_and_capture_markers() {
    for (seed, winter, layout) in [
        (522, false, SettlementLayout::KingdomTemperate),
        (522, true, SettlementLayout::KingdomWinter),
        (261, false, SettlementLayout::Outpost),
        (4897, false, SettlementLayout::KingdomTemperate),
        (4897, true, SettlementLayout::KingdomWinter),
        (3456, false, SettlementLayout::Outpost),
    ] {
        assert!(selected(seed));
        let terrain = terrain(winter, 6);
        let site = plan(&terrain, [-1, -2], seed)
            .expect("Authored dry flat stronghold must fit its budget");
        assert_eq!(site.summary.layout(), Some(layout));
        assert!(site.summary.domain().unwrap().valid());
        assert!(site.summary.bot_kingdom().is_some());
        assert_eq!(site.block_at(site.summary.center), Some(castle::BERGFRIED));
        let settlement = site.settlement.as_ref().unwrap();
        assert_eq!(settlement.layout, layout);
        assert_eq!(settlement.domain(), site.summary.domain().unwrap());
        let civilians = settlement
            .residents
            .iter()
            .filter(|r| terra_core::creatures::villagers::is_civilian(r.role))
            .count();
        let soldiers = settlement
            .residents
            .iter()
            .filter(|r| {
                terra_core::creatures::soldiers::SoldierLoadout::from_role(r.role).is_some()
            })
            .count();
        if layout == SettlementLayout::Outpost {
            assert_eq!(civilians, 0);
            assert_eq!(soldiers, 4 + ((seed >> 7) % 7) as usize);
        } else {
            assert_eq!(civilians, 50 + (seed % 31) as usize);
            assert_eq!(soldiers, 26 + ((seed >> 7) % 39) as usize);
            assert_eq!(settlement.homes.len(), 20);
        }
        let military: std::collections::BTreeSet<_> = settlement
            .residents
            .iter()
            .filter(|r| {
                terra_core::creatures::soldiers::SoldierLoadout::from_role(r.role).is_some()
            })
            .map(|r| r.role)
            .collect();
        assert_eq!(military, std::collections::BTreeSet::from([10, 11, 12]));
        let beds: Vec<_> = objects(&site)
            .filter(|o| (71..=73).contains(&o.material) && o.state.is_some_and(|s| !s.secondary))
            .collect();
        assert_eq!(beds.len(), civilians + soldiers);
        assert_eq!(settlement.residents.len(), beds.len());
        for resident in &settlement.residents {
            assert_eq!(resident.layout, layout);
            assert_eq!(resident.bot_kingdom(), settlement.bot_kingdom());
            let bed = objects(&site).find(|o| o.position == resident.bed).unwrap();
            let head = bed.state.unwrap().peer.unwrap();
            assert!(objects(&site).any(|o| {
                o.position == head
                    && o.state
                        .is_some_and(|s| s.secondary && s.peer == Some(resident.bed))
            }));
            assert_eq!(
                site.block_at([resident.bed[0], resident.bed[1] - 1, resident.bed[2]]),
                Some(16)
            );
            assert_eq!(site.block_at(resident.door), Some(65));
            let landing = [resident.door[0], resident.door[1], resident.door[2] + 1];
            assert_eq!(
                site.block_at(landing),
                Some(0),
                "Every resident has an unobstructed outdoor landing"
            );
            assert_eq!(
                site.block_at([landing[0], landing[1] - 1, landing[2]]),
                Some(43)
            );
        }
        assert!(site.sections.len() <= MAX_SITE_SECTIONS);
        assert!(site.sections.values().map(|s| s.cells.len()).sum::<usize>() <= MAX_SITE_CELLS);
        assert!(
            objects(&site).all(|o| o.state.is_none_or(|s| s.valid_for(o.position, o.material)))
        );
        let mut journal = crate::edits::EditJournal::default();
        journal
            .initialize_structures(&objects(&site).cloned().collect::<Vec<_>>())
            .unwrap();
        assert!(
            journal
                .initialize_structures(&objects(&site).cloned().collect::<Vec<_>>())
                .unwrap()
                .is_empty()
        );
    }
}

#[test]
fn winter_terraces_temperate_ring_and_palisade_outpost_are_distinct_real_layouts() {
    let temperate = plan(&terrain(false, 6), [0, 0], 522).unwrap();
    let winter = plan(&terrain(true, 6), [0, 0], 522).unwrap();
    let outpost = plan(&terrain(false, 6), [0, 0], 261).unwrap();
    let bounds = |s: &Site| {
        [
            s.summary.bounds[1][0] - s.summary.bounds[0][0],
            s.summary.bounds[1][1] - s.summary.bounds[0][1],
            s.summary.bounds[1][2] - s.summary.bounds[0][2],
        ]
    };
    assert!(bounds(&temperate)[0] > 140 && bounds(&temperate)[2] > 140);
    assert!(bounds(&winter)[1] > bounds(&temperate)[1]);
    // The fort is compact; its graded 32m approach also belongs to its bounds.
    assert!(bounds(&outpost)[0] < 90 && bounds(&outpost)[2] < 100);
    for site in [&temperate, &winter] {
        for role in [
            BuildingRole::Keep,
            BuildingRole::Gate,
            BuildingRole::Church,
            BuildingRole::Barracks,
            BuildingRole::Market,
            BuildingRole::Farm,
        ] {
            assert!(site.summary.buildings.iter().any(|b| b.role == role));
        }
        assert!(
            site.summary
                .buildings
                .iter()
                .filter(|b| b.role == BuildingRole::Watch)
                .count()
                >= 11
        );
    }
    assert!(
        outpost
            .summary
            .buildings
            .iter()
            .any(|b| b.role == BuildingRole::Stable)
    );
    assert!(
        !outpost
            .summary
            .buildings
            .iter()
            .any(|b| b.role == BuildingRole::Church)
    );
    let red = terra_core::textiles::dyed(103, DyeColor::Red).unwrap();
    let blue = terra_core::textiles::dyed(103, DyeColor::Blue).unwrap();
    assert!(
        winter
            .sections
            .values()
            .flat_map(|s| &s.cells)
            .filter(|(_, m)| *m == red)
            .count()
            > 1000
    );
    assert!(
        temperate
            .sections
            .values()
            .flat_map(|s| &s.cells)
            .filter(|(_, m)| *m == blue)
            .count()
            > 1000
    );
    let [cx, base, cz] = [192, 16, 192];
    // The south gate is continuous walking space, not a painted gate over solid walls.
    for z in 66..=86 {
        for x in -2..=2 {
            for y in 0..=1 {
                assert_eq!(temperate.block_at([cx + x, base + y, cz + z]), Some(0));
            }
        }
    }
}

#[test]
fn saved_revision_five_still_uses_its_original_village_generator_and_old_limits() {
    let old = terrain(false, 5);
    let fresh = terrain(false, 6);
    let old_site = super::super::plan(&old, [0, 0], 522).unwrap();
    assert_eq!(old_site.summary.kind, StructureKind::Village);
    assert_eq!(
        old_site.settlement.as_ref().unwrap().layout,
        SettlementLayout::Village
    );
    assert!(
        old_site.sections.len() <= 256
            && old_site
                .sections
                .values()
                .map(|s| s.cells.len())
                .sum::<usize>()
                <= 300_000
    );
    assert_eq!(
        super::super::plan(&fresh, [0, 0], 522)
            .unwrap()
            .summary
            .kind,
        StructureKind::Kingdom
    );
    let again = super::super::plan(&old, [0, 0], 522).unwrap();
    assert_eq!(old_site.summary, again.summary);
    assert_eq!(
        old_site.sections.keys().collect::<Vec<_>>(),
        again.sections.keys().collect::<Vec<_>>()
    );
    for (key, patch) in &old_site.sections {
        assert_eq!(patch.cells, again.sections[key].cells);
        assert_eq!(patch.objects, again.sections[key].objects);
    }
}

#[test]
fn keep_stairwells_reach_all_four_stories_and_winter_ward_gate_meets_its_ramp() {
    use std::collections::{BTreeSet, VecDeque};
    for (winter, revision) in [(false, 6), (true, 6), (false, 7), (true, 7)] {
        let site = plan(&terrain(winter, revision), [0, 0], 522).unwrap();
        let keep = site
            .summary
            .buildings
            .iter()
            .find(|b| b.role == BuildingRole::Keep)
            .unwrap();
        let entrance = keep.entrance;
        let clear = |p: [i64; 3]| matches!(site.block_at(p), Some(0 | 65));
        let stand = |p: [i64; 3]| {
            clear(p)
                && clear([p[0], p[1] + 1, p[2]])
                && site
                    .block_at([p[0], p[1] - 1, p[2]])
                    .is_some_and(|m| !matches!(m, 0 | 27 | 65))
        };
        assert!(stand(entrance));
        let mut reached = BTreeSet::from([entrance]);
        let mut queue = VecDeque::from([entrance]);
        while let Some(p) = queue.pop_front() {
            for [dx, dz] in [[1, 0], [-1, 0], [0, 1], [0, -1]] {
                for dy in [-1, 0, 1] {
                    let next = [p[0] + dx, p[1] + dy, p[2] + dz];
                    if next[1] < entrance[1]
                        || next[1] > entrance[1] + 22
                        || [0, 2].into_iter().any(|axis| {
                            next[axis] < keep.bounds[0][axis] || next[axis] > keep.bounds[1][axis]
                        })
                    {
                        continue;
                    }
                    if stand(next) && reached.insert(next) {
                        queue.push_back(next);
                    }
                }
            }
        }
        for level in [0, 6, 12, 18] {
            assert!(
                reached.contains(&[entrance[0], entrance[1] + level, entrance[2] - 11]),
                "Storey {level} is sealed off (winter={winter})"
            );
        }
        let base = entrance[1] - if winter { 14 } else { 5 };
        let floor = |z: i64| {
            if winter {
                (-z).min(14)
            } else {
                (-z - 4).max(0) / 2
            }
        };
        for z in -15..=-5 {
            let rise = floor(z);
            let point = [195, base + rise, 192 + z];
            assert!(
                clear(point) && clear([point[0], point[1] + 1, point[2]]),
                "The gate approach must have real head clearance"
            );
            assert!(matches!(
                site.block_at([point[0], point[1] - 1, point[2]]),
                Some(43 | 77)
            ));
        }
        assert_eq!(
            base + floor(-15),
            entrance[1],
            "The ward gate and outside ramp must end at the same height"
        );
    }
}

#[test]
fn real_hillside_gate_roads_join_natural_ground_without_grass_overhangs_or_terminal_walls() {
    use std::collections::{BTreeSet, VecDeque};
    let terrain = TerraGenerator::new(WorldConfig {
        seed: WorldSeed(42042),
        structures_revision: 6,
        // Preserve the recorded cave/gate coordinates when fresh-world defaults evolve.
        landscape_revision: 1,
        ecology_revision: 4,
        river_terrain_revision: 4,
        spirit_blossom_revision: 0,
        blood_forest_revision: 0,
        ..Default::default()
    })
    .unwrap();
    let mut graded = false;
    for (region, kind) in [
        ([1, 4], StructureKind::Outpost),
        ([-1, -1], StructureKind::Kingdom),
        ([0, -1], StructureKind::Kingdom),
    ] {
        let site = terrain
            .planned_site(region)
            .expect("Known real terrain stronghold must retain its walkable approach");
        assert_eq!(site.summary.kind, kind);
        let gate = site
            .summary
            .buildings
            .iter()
            .find(|b| b.role == BuildingRole::Gate)
            .unwrap();
        let start = gate.entrance;
        let length = if kind == StructureKind::Outpost {
            32
        } else {
            22
        };
        let ground = |x: i64, z: i64| {
            (terrain.surface_height(x as f64 + 0.5, z as f64 + 0.5) - 0.5).floor() as i64 + 1
        };
        let block = |p: [i64; 3]| {
            site.block_at(p).unwrap_or_else(|| {
                let column = terrain.surface_column(p[0] as f64 + 0.5, p[2] as f64 + 0.5);
                let sample = terrain.sample_with_column(p.map(|v| v as f64 + 0.5), column);
                if sample.density <= 0.0 {
                    sample.material
                } else {
                    0
                }
            })
        };
        let stand = |p: [i64; 3]| {
            let support = block([p[0], p[1] - 1, p[2]]);
            block(p) == 0
                && block([p[0], p[1] + 1, p[2]]) == 0
                && (matches!(support, 43 | 77) || crate::props::placement::ground_support(support))
        };
        // Independently inspect the finished voxel world, including open caves,
        // instead of trusting the nominal heightfield used by the old road.
        let physical_ground = |x, z| {
            let nominal = ground(x, z);
            (nominal - 32..=nominal + 2)
                .rev()
                .map(|y| [x, y, z])
                .find(|p| stand(*p))
                .expect("The outside approach must have actual supported walking space")[1]
        };
        assert!(stand(start));
        let endpoint_natural = physical_ground(start[0], start[2] + length);
        let endpoint = [endpoint_natural, endpoint_natural + 1]
            .into_iter()
            .map(|y| [start[0], y, start[2] + length])
            .find(|p| stand(*p) && matches!(site.block_at([p[0], p[1] - 1, p[2]]), Some(43 | 77)))
            .expect("The final stair must join the natural surface at half-step height");
        assert!(stand(endpoint));
        let outside = [
            start[0],
            physical_ground(start[0], start[2] + length + 1),
            start[2] + length + 1,
        ];
        assert!(
            stand(outside),
            "region={region:?} gate={start:?} endpoint={endpoint:?} outside={outside:?} natural_column={:?} cells={:?}",
            terrain.surface_column(outside[0] as f64 + 0.5, outside[2] as f64 + 0.5),
            (outside[1] - 2..=outside[1] + 2)
                .map(|y| {
                    let p = [outside[0], y, outside[2]];
                    let column = terrain.surface_column(p[0] as f64 + 0.5, p[2] as f64 + 0.5);
                    let sample = terrain.sample_with_column(p.map(|v| v as f64 + 0.5), column);
                    (
                        y,
                        block(p),
                        sample.density,
                        sample.material,
                        site.block_at(p),
                    )
                })
                .collect::<Vec<_>>()
        );
        assert!(endpoint[1].abs_diff(outside[1]) <= 1);
        if region == [1, 4] {
            assert!(
                outside[1] + 4 < ground(outside[0], outside[2]),
                "This regression must still include the actual open cave below the nominal surface"
            );
        }
        let mut reached = BTreeSet::from([start]);
        let mut queue = VecDeque::from([start]);
        while let Some(p) = queue.pop_front() {
            for [dx, dz] in [[1, 0], [-1, 0], [0, 1], [0, -1]] {
                for dy in [-1, 0, 1] {
                    let next = [p[0] + dx, p[1] + dy, p[2] + dz];
                    if next[0].abs_diff(start[0]) > 3
                        || next[2] < start[2]
                        || next[2] > outside[2]
                        || next[1].abs_diff(start[1]) > 40
                    {
                        continue;
                    }
                    if stand(next) && reached.insert(next) {
                        queue.push_back(next);
                    }
                }
            }
        }
        assert!(
            reached.contains(&outside),
            "The gate must connect through actual two-cell headroom all the way onto natural terrain"
        );
        for dz in 5..=length {
            let z = start[2] + dz;
            let p = (start[1] - 32..=start[1] + 32)
                .map(|y| [start[0], y, z])
                .find(|p| {
                    stand(*p) && matches!(site.block_at([p[0], p[1] - 1, p[2]]), Some(43 | 77))
                })
                .unwrap();
            graded |= p[1] != start[1];
            let top = ground(start[0], z) + 7;
            for y in p[1]..=top.max(p[1] + 3) {
                assert_eq!(
                    block([p[0], y, p[2]]),
                    0,
                    "A grass ceiling must not remain over the outside path"
                );
            }
        }
    }
    assert!(
        graded,
        "The regression must cover genuinely graded production terrain"
    );
}
