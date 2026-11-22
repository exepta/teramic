use super::*;

#[test]
fn suppressed_chambers_remain_solid_and_keep_their_original_roof_and_floor_guards() {
    let old = CaveStructure {
        kind: CaveEcology::Salt,
        radius: 35.0,
        height: 20.0,
        floor: -45.5,
        ceiling: -20.5,
        radial: 0.3,
        revision: 6,
        ..Default::default()
    };
    let closed = CaveStructure {
        suppressed: true,
        revision: 7,
        ..old
    };
    assert!(old.active() && !closed.active());
    for step in 0..=124 {
        let y = old.floor - 3.0 + f64::from(step) * 0.25;
        for tunnel in [-8.0, -1.0, 0.5, 2.0] {
            let before = old.carve(y, tunnel, 20.0);
            let after = closed.carve(y, tunnel, 20.0);
            assert!(
                after <= before && after < 0.0,
                "guard at {y}: {before}->{after}"
            );
        }
    }
}
use crate::SectionGenerator;
use terra_core::{Config, WorldConfig};
use terra_voxel::global_to_section;

#[test]
fn reported_desert_gallery_has_one_connected_daylight_opening_and_roofed_side_halls() {
    let terrain = TerraGenerator::new(WorldConfig {
        seed: terra_core::WorldSeed(2286876405460410298),
        cave_structure_revision: 11,
        ..Default::default()
    })
    .unwrap();
    let generator = SectionGenerator::new(terrain.clone());
    let mut aperture = std::collections::HashSet::new();
    let mut window = None;
    for z in -9550..-9480 {
        for x in 215..355 {
            let (px, pz) = (f64::from(x) + 0.5, f64::from(z) + 0.5);
            let column = terrain.surface_column(px, pz);
            let room = column.cave;
            if !room.open_to_sky() {
                continue;
            }
            let (lip, _) = room.salt_skylight(room.floor + 1.).unwrap();
            assert_eq!(*window.get_or_insert(lip), lip);
            let top = (column.height - 0.5).floor() + 0.5;
            let surface = terrain.sample_with_column([px, top, pz], column);
            if surface.density > 0.0 {
                aperture.insert([x, z]);
            } else if column.surface_material == MaterialId::Sand && room.salt_window_wall(top) {
                assert_eq!(surface.material, MaterialId::Sand as u16);
            }
        }
    }
    assert!(
        (40..560).contains(&aperture.len()),
        "surface window area {}",
        aperture.len()
    );
    let mut connected = vec![*aperture.iter().next().unwrap()];
    while let Some(point @ [x, z]) = connected.pop() {
        if aperture.remove(&point) {
            connected.extend([[x - 1, z], [x + 1, z], [x, z - 1], [x, z + 1]]);
        }
    }
    assert!(aperture.is_empty());
    // Both ends of the same gallery used to bore straight up through the dunes.
    for [x, z] in [[240, -9520], [332, -9522]] {
        let column = terrain.surface_column(x as f64 + 0.5, z as f64 + 0.5);
        let top = (column.height - 0.5).floor() as i64;
        assert!(column.cave.active());
        for y in top - 3..=top {
            assert_ne!(generator.block_at([x, y, z]).unwrap(), 0);
        }
        assert!(
            ((column.cave.floor.ceil() as i64)..(column.cave.ceiling.floor() as i64))
                .any(|y| generator.block_at([x, y, z]).unwrap() == 0)
        );
    }
    // The unclipped chamber ceiling crosses the terrain surface here. It
    // previously painted a ring of stone and salt ore across the intact dune.
    for [x, y, z] in [[241, 62, -9514], [241, 63, -9504]] {
        assert_eq!(
            generator.block_at([x, y, z]).unwrap(),
            MaterialId::Sand as u16
        );
    }
    // The adjacent natural ravine still descends fifteen metres, with sand
    // deposited on the open banks above the buried mineral gallery.
    let bed = [295, 50, -9468];
    assert!(terrain.surface_column(295.5, -9467.5).height > 64.5);
    assert_eq!(generator.block_at(bed).unwrap(), MaterialId::Sand as u16);
    assert_eq!(generator.block_at([295, 51, -9468]).unwrap(), 0);
    let dig = [240, 60, -9520];
    let (section, local) = global_to_section(dig);
    assert_ne!(generator.block_at(dig).unwrap(), 0);
    let mut edits = crate::edits::EditJournal::default();
    edits.record(dig, MaterialId::Air as u16).unwrap();
    let rebuilt = generator
        .build(
            section,
            edits.revision(section),
            &edits.snapshot(section).unwrap(),
            false,
        )
        .unwrap();
    assert_eq!(rebuilt.data.block(local).unwrap(), MaterialId::Air as u16);
}

#[test]
fn revision_eleven_beds_and_cross_joints_replace_the_round_stone_room() {
    let previous = CaveStructure {
        kind: CaveEcology::Stone,
        center: [-192., -40., -16.],
        radius: 58.,
        height: 25.,
        revision: 10,
        ..Default::default()
    };
    let current = CaveStructure {
        revision: 11,
        ..previous
    };
    let [x, _, z] = current.center;
    assert_eq!(previous.geometry(x, z), [-61.5, -19.]);
    let [_, vault] = current.geometry(x, z);
    let [_, saddle] = current.geometry(x + 26., z);
    assert!(vault - saddle > 5.);
    let floor: Vec<_> = (10..=23)
        .map(|v| current.geometry(x, z + f64::from(v))[0])
        .collect();
    assert!(floor.windows(3).any(|w| w[0] == w[2]));
    assert!(floor.last().unwrap() - floor[0] > 1.0);
    // The side bay departs from the main axis on just one side of the strike.
    assert!(current.radial_at(x + 16., z + 36.) < 1.);
    assert!(current.radial_at(x + 16., z - 36.) > 1.);
    for (dx, dz) in [(59., 0.), (0., 59.), (-52., -31.)] {
        assert!(current.radial_at(x + dx, z + dz) > 1.);
    }
}

#[test]
fn revision_eleven_joins_a_tunnel_at_the_outer_shoulder_without_unsealing_its_water_bed() {
    let room = CaveStructure {
        kind: CaveEcology::Stone,
        revision: 11,
        radius: 50.,
        floor: -40.,
        ceiling: -26.,
        radial: 0.88,
        water: Some(WaterColumn {
            level: -36.,
            bed: -40.,
            depth: 4.,
        }),
        ..Default::default()
    };
    let previous = CaveStructure {
        revision: 10,
        ..room
    };
    assert_eq!(previous.carve(-25., 2., 20.), -1.);
    assert!(room.carve(-25., 2., 20.) >= 2.);
    assert!(room.carve(-25., -6., 20.) < 0.);
    assert!(room.carve(-41., 2., 20.) < 0.);
}

#[test]
fn modern_dripstone_rooms_emit_rooted_fluted_growth_with_air_between_opposing_tips() {
    let room = CaveStructure {
        kind: CaveEcology::Dripstone,
        floor: -48.,
        ceiling: 4.,
        radius: 60.,
        revision: 10,
        ..Default::default()
    };
    let mut opposed = 0;
    for x in 0..48 {
        for z in 0..48 {
            let material = |y: i32| {
                room.room_feature(
                    [f64::from(x) + 0.5, f64::from(y) + 0.5, f64::from(z) + 0.5],
                    4.,
                    0.,
                )
            };
            if [material(-48), material(3)] == [Some(MaterialId::DripstoneTip); 2] {
                assert_eq!(material(-22), None);
                opposed += 1;
            }
        }
    }
    assert!(opposed > 4);
}

#[test]
fn explicit_revision_three_corridors_survive_chamber_guards_without_losing_their_floor_or_roof() {
    let terrain = TerraGenerator::new(WorldConfig {
        landscape_revision: 3,
        geology_revision: 0,
        rivers: false,
        trees: false,
        biomes: false,
        ..Default::default()
    })
    .unwrap();
    let base = terrain.surface_column(0.5, 0.5);
    let room = CaveStructure {
        kind: CaveEcology::Dripstone,
        center: [0.5, -32.5, 0.5],
        radius: 48.0,
        height: 22.0,
        floor: -42.5,
        ceiling: -20.5,
        radial: 0.1,
        revision: 7,
        seed: 19,
        ..Default::default()
    };
    let [floor, ceiling] = [-46.5, -38.5];
    for suppressed in [false, true] {
        for river in [false, true] {
            let column = crate::SurfaceColumn {
                height: 20.0,
                cave: CaveStructure { suppressed, ..room },
                cave_cavity: (!river).then_some([floor, ceiling]),
                river_cavity: river.then_some([floor, ceiling]),
                ..base
            };
            for step in 1..16 {
                let y = floor + f64::from(step) * 0.5;
                let sample = terrain.sample_with_column([0.5, y, 0.5], column);
                assert!(
                    sample.density > 0.0 && sample.material == MaterialId::Air as u16,
                    "corridor resealed by chamber: suppressed={suppressed}, river={river}, y={y}, sample={sample:?}"
                );
            }
            for y in [
                floor - 0.25,
                floor - 1.0,
                floor - 1.75,
                ceiling + 0.25,
                ceiling + 1.0,
            ] {
                assert!(
                    terrain.sample_with_column([0.5, y, 0.5], column).density < 0.0,
                    "missing corridor support: suppressed={suppressed}, river={river}, y={y}"
                );
            }
            if !suppressed {
                for y in [ceiling + 4.0, ceiling + 6.0] {
                    assert!(
                        terrain.cave_density_with_column([0.5, y, 0.5], column).0 > 0.0,
                        "corridor roof protection sealed a higher chamber: river={river}, y={y}"
                    );
                }
            }
        }
    }
    let legacy = TerraGenerator::new(WorldConfig {
        landscape_revision: 2,
        ..terrain.config().clone()
    })
    .unwrap();
    let blocked = crate::SurfaceColumn {
        height: 20.0,
        cave: room,
        cave_cavity: Some([floor, ceiling]),
        river_cavity: None,
        ..base
    };
    assert!(
        legacy
            .cave_density_with_column([0.5, -44.5, 0.5], blocked)
            .0
            < 0.0
    );
}

fn revision_two() -> WorldConfig {
    WorldConfig {
        landscape_revision: 4,
        sea_level: 22.0,
        rivers: true,
        biome_definitions: Some(terra_core::BiomeDefinitions::default()),
        cave_structure_revision: 2,
        ..Default::default()
    }
}

#[test]
fn revision_five_rooms_have_irregular_seeded_outlines_and_non_radial_channels() {
    let base = CaveStructure {
        kind: CaveEcology::Salt,
        center: [-192.0, -55.0, -384.0],
        radius: 48.0,
        height: 28.0,
        seed: 0,
        revision: 5,
        ..Default::default()
    };
    let profile = |room: CaveStructure| {
        (0..24)
            .map(|i| {
                let a = f64::from(i) * std::f64::consts::TAU / 24.0;
                let x = room.center[0] + a.cos() * room.radius * 0.7;
                let z = room.center[2] + a.sin() * room.radius * 0.7;
                let value = room.radial_at(x, z);
                assert_eq!(value, room.radial_at(x, z));
                (value * 1000.0) as i64
            })
            .collect::<Vec<_>>()
    };
    let a = profile(base);
    assert!(a.iter().max().unwrap() - a.iter().min().unwrap() > 100);
    assert_ne!(a, profile(CaveStructure { seed: 219, ..base }));
    let mut wet = 0;
    let mut dry = 0;
    for u in -36..=36 {
        for v in -36..=36 {
            let x = base.center[0] + f64::from(u) + 0.5;
            let z = base.center[2] + f64::from(v) + 0.5;
            if base.radial_at(x, z) >= 0.85 {
                continue;
            }
            if base.channel_mask(f64::from(u) + 0.5, f64::from(v) + 0.5) > 0.5 {
                wet += 1;
            } else {
                dry += 1;
            }
            assert!(
                base.radial_at(x, z) + 1e-12
                    >= (x - base.center[0]).hypot(z - base.center[2]) / (base.radius * 0.985)
            );
        }
    }
    assert!(
        wet > 100 && dry > wet * 3,
        "narrow stream beds retain extensive dry shelves: {wet}/{dry}"
    );
}

#[test]
fn revision_five_small_streams_have_valid_sources_and_real_downstream_wake_cells() {
    for kind in [
        CaveEcology::Stone,
        CaveEcology::Salt,
        CaveEcology::Lush,
        CaveEcology::Waterfall,
    ] {
        let room = CaveStructure {
            kind,
            center: [-192.0, -55.0, -384.0],
            radius: 48.0,
            height: 28.0,
            seed: 0,
            revision: 5,
            dynamic_water: true,
            ..Default::default()
        };
        let mut pools = 0;
        let mut outlets = 0;
        for u in -42..42 {
            for v in -24..24 {
                let x = room.center[0] + f64::from(u) + 0.5;
                let z = room.center[2] + f64::from(v) + 0.5;
                let Some(water) = room.cascade_water(x, z) else {
                    continue;
                };
                pools += 1;
                let [floor, ceiling] = room.geometry(x, z);
                assert!(water.bed >= floor - 0.5 && water.level > water.bed);
                assert!(water.level + 2.0 < ceiling);
                let sample = CaveStructure {
                    water: Some(water),
                    radial: room.radial_at(x, z),
                    floor,
                    ceiling,
                    ..room
                };
                if let Some(p) = sample.fluid_outlet(x, z) {
                    outlets += 1;
                    assert_eq!(
                        (p[0] - x.floor() as i64).abs() + (p[2] - z.floor() as i64).abs(),
                        1
                    );
                    assert!(
                        room.cascade_water(p[0] as f64 + 0.5, p[2] as f64 + 0.5)
                            .is_none()
                    );
                    assert!(
                        room.geometry(p[0] as f64 + 0.5, p[2] as f64 + 0.5)[0] < water.level - 1.5
                    );
                    assert_eq!(p[1], water.level.ceil() as i64 - 1);
                }
            }
        }
        assert!(
            pools > 50 && outlets > 0,
            "{kind:?} must seed actual fluid updates: pools={pools}, outlets={outlets}"
        );
    }
}

#[test]
fn revision_four_salt_terraces_keep_the_saved_world_profile() {
    let room = CaveStructure {
        kind: CaveEcology::Salt,
        center: [-192.0, -55.0, -384.0],
        radius: 48.0,
        height: 28.0,
        seed: 91,
        revision: 4,
        ..Default::default()
    };
    for z in -20..20 {
        for x in -20..20 {
            let u = f64::from(x) + 0.5;
            let v = f64::from(z) + 0.5;
            let r = room.radial_at(room.center[0] + u, room.center[2] + v);
            let phase = f64::from(room.seed % 628) * 0.01;
            let rough = (u * 0.19 + v * 0.11 + phase).sin() * 0.72
                + (v * 0.27 - u * 0.13 - phase).sin() * 0.50;
            let channel =
                (u * 0.055 + f64::from(room.seed % 19)).sin() * 8.0 + (u * 0.12).sin() * 2.0;
            let width = 4.5 + (u * 0.07).sin() * 1.8;
            let basin = 1.0 - smoothstep(width, width + 3.0, (v - channel).abs());
            let wave = (u * 0.042 + phase).sin() * 3.0 + (v * 0.051 - phase).cos() * 2.0;
            let expected = room.center[1] - room.height * 0.72
                + smoothstep(0.62, 1.02, r) * room.height * 1.55
                - basin * 2.5
                + ((r * 5.0 + wave * 0.12).max(0.0)).floor() * 1.8
                + rough * 0.3;
            assert!(
                (room.geometry(room.center[0] + u, room.center[2] + v)[0] - expected.max(-118.5))
                    .abs()
                    < 1e-12
            );
        }
    }
}

#[test]
fn revision_four_ecologies_have_distinct_terraces_and_safe_basin_bounds() {
    let base = CaveStructure {
        center: [0.0, -55.0, 0.0],
        radius: 48.0,
        height: 28.0,
        seed: 91,
        revision: 4,
        ..Default::default()
    };
    let profiles: Vec<Vec<i64>> = [CaveEcology::Salt, CaveEcology::Lush, CaveEcology::Waterfall]
        .into_iter()
        .map(|kind| {
            let room = CaveStructure { kind, ..base };
            (-20..=20)
                .map(|x| (room.geometry(x as f64, 0.5)[0] * 100.0) as i64)
                .collect()
        })
        .collect();
    assert_ne!(profiles[0], profiles[1]);
    assert_ne!(profiles[1], profiles[2]);
    assert_ne!(profiles[0], profiles[2]);
    for y in [-110.0, -70.0, -20.0] {
        let room = CaveStructure {
            kind: CaveEcology::Waterfall,
            center: [0.0, y, 0.0],
            ..base
        };
        for x in -48..48 {
            for z in -48..48 {
                if let Some(water) = room.cascade_water(x as f64 + 0.5, z as f64 + 0.5) {
                    let [floor, ceiling] = room.geometry(x as f64 + 0.5, z as f64 + 0.5);
                    assert!(water.bed >= -119.0 && water.bed >= floor - 0.5);
                    assert!(water.level > water.bed && water.level + 2.0 < ceiling);
                }
            }
        }
    }
}

#[test]
fn new_cave_contract_is_explicit_validated_and_absent_in_existing_worlds() {
    assert_eq!(WorldConfig::default().cave_structure_revision, 13);
    assert_eq!(
        WorldConfig::from_toml("").unwrap().cave_structure_revision,
        0
    );
    assert_eq!(
        WorldConfig::from_toml("cave_structure_revision=7")
            .unwrap()
            .cave_structure_revision,
        7
    );
    assert!(WorldConfig::from_toml("cave_structure_revision=14").is_err());
    let old = TerraGenerator::new(WorldConfig {
        cave_structure_revision: 0,
        ..Default::default()
    })
    .unwrap();
    assert_eq!(
        old.cave_structure_at(16800.0, -17952.0),
        CaveStructure::default()
    );
    let off = TerraGenerator::new(WorldConfig {
        caves: false,
        ..Default::default()
    })
    .unwrap();
    assert_eq!(off.cave_structure_at(0.5, 0.5), CaveStructure::default());
}

fn rooms(terrain: &TerraGenerator) -> Vec<CaveStructure> {
    let mut found = Vec::new();
    for (x, z) in
        std::iter::once((87, -94)).chain((-96..96).flat_map(|z| (-96..96).map(move |x| (x, z))))
    {
        let region = terrain.cave_region([x, z]);
        let sample = terrain.cave_structure_at(region.center[0], region.center[2]);
        if sample.active() && !found.iter().any(|r: &CaveStructure| r.kind == sample.kind) {
            found.push(sample);
        }
        if [
            CaveEcology::Salt,
            CaveEcology::Dripstone,
            CaveEcology::Lush,
            CaveEcology::Waterfall,
        ]
        .into_iter()
        .all(|kind| found.iter().any(|r| r.kind == kind))
        {
            break;
        }
    }
    for kind in [
        CaveEcology::Salt,
        CaveEcology::Dripstone,
        CaveEcology::Lush,
        CaveEcology::Waterfall,
    ] {
        assert!(found.iter().any(|r| r.kind == kind), "Missing {kind:?}");
    }
    found
}

#[test]
fn revision_two_adds_larger_rooms_and_leaves_solid_bands_between_networks() {
    let current = TerraGenerator::new(revision_two()).unwrap();
    let previous = TerraGenerator::new(WorldConfig {
        landscape_revision: 4,
        sea_level: 22.0,
        rivers: true,
        biome_definitions: Some(terra_core::BiomeDefinitions::default()),
        cave_structure_revision: 1,
        ..Default::default()
    })
    .unwrap();
    let (mut old_rooms, mut new_rooms, mut closed) = (0, 0, 0);
    for z in -8..8 {
        for x in -8..8 {
            let a = previous.cave_region([x, z]);
            let b = current.cave_region([x, z]);
            old_rooms += usize::from(a.radius > 0.0);
            new_rooms += usize::from(b.radius > 0.0);
            if a.radius > 0.0 && b.radius > 0.0 && a.kind == b.kind {
                assert!(b.radius >= a.radius);
            }
            assert!(
                b.radius <= 78.0,
                "rooms must not cross the 192 m cell boundary"
            );
            let px = x as f64 * 192.0;
            let pz = z as f64 * 192.0;
            closed += usize::from(current.cave_shape(px, pz).tunnel_width <= 0.0);
            for y in [-100.0, -50.0, -10.0] {
                assert!(
                    (current.tunnel_density([px - 0.00001, y, pz])
                        - current.tunnel_density([px + 0.00001, y, pz]))
                    .abs()
                        < 0.001
                );
            }
        }
    }
    assert!(new_rooms > old_rooms);
    assert!(closed > 20 && closed < 230);
}

#[test]
fn salt_has_rock_between_deposits_and_a_curving_river_between_dry_shelves() {
    let terrain = TerraGenerator::new(revision_two()).unwrap();
    let center = [16800.0, -17952.0];
    let (mut salt, mut rock, mut wet, mut dry) = (0, 0, 0, 0);
    let mut river_z = Vec::new();
    for x in -35..=35 {
        let mut channel = Vec::new();
        for z in -35..=35 {
            let p = [
                center[0] + f64::from(x) + 0.5,
                center[1] + f64::from(z) + 0.5,
            ];
            let c = terrain.cave_structure_at(p[0], p[1]);
            assert_eq!(c.kind, CaveEcology::Salt);
            let y = (c.floor - 0.5).floor() + 0.5;
            match c.material([p[0], y, p[1]], -0.5) {
                Some(MaterialId::SaltOre) => salt += 1,
                Some(MaterialId::Stone) => rock += 1,
                _ => {}
            }
            if c.water.is_some() {
                wet += 1;
                channel.push(z);
                assert!(c.river_flow(p[0])[0] > 0.0);
            } else {
                dry += 1;
            }
        }
        if !channel.is_empty() {
            river_z.push(channel.iter().sum::<i32>() as f32 / channel.len() as f32);
        }
    }
    assert!(salt > 200 && rock > salt && wet > 100 && dry > wet);
    assert!(
        river_z.iter().copied().fold(f32::NEG_INFINITY, f32::max)
            - river_z.iter().copied().fold(f32::INFINITY, f32::min)
            > 6.0
    );
}

#[test]
fn corridor_decoration_is_sparse_rooted_and_deterministic() {
    let terrain = TerraGenerator::new(revision_two()).unwrap();
    let (mut roots, mut mushrooms, mut air) = (0, 0, 0);
    for z in (-180..180).step_by(4) {
        for x in (-180..180).step_by(4) {
            let (x, z) = (f64::from(x) + 0.5, f64::from(z) + 0.5);
            let c = terrain.surface_column(x, z);
            if c.cave.active() {
                continue;
            }
            for y in -110..-10 {
                let p = [x, f64::from(y) + 0.5, z];
                let sample = terrain.sample_with_column(p, c);
                if sample.density > 0.0 {
                    air += 1;
                }
                if matches!(sample.material, 55 | 56) {
                    assert_eq!(sample, terrain.sample_with_column(p, c));
                    let dy = if sample.material == 55 { 1.0 } else { -1.0 };
                    let support = terrain.sample_with_column([x, p[1] + dy, z], c);
                    assert!(support.density <= 0.0);
                    if sample.material == 55 {
                        roots += 1;
                    } else {
                        mushrooms += 1;
                    }
                }
            }
        }
    }
    assert!(
        roots > 0 && mushrooms > 0 && (roots + mushrooms) * 20 < air,
        "{roots} roots, {mushrooms} mushrooms, {air} air"
    );
}

#[test]
fn legacy_highland_basins_preserve_their_geometry_and_seed_real_block_overflows() {
    let terrain = TerraGenerator::new(revision_two()).unwrap();
    let room = terrain.cave_structure_at(-13344.0, -18336.0);
    assert_eq!(room.kind, CaveEcology::Waterfall);
    let [cx, _, cz] = room.center;
    let pair = (-22..=22)
        .find_map(|dz| {
            (-4..=10).find_map(|dx| {
                let x = cx + f64::from(dx) + 0.5;
                let z = cz + f64::from(dz) + 0.5;
                let high = terrain.cave_structure_at(x, z).water?;
                let low = terrain.cave_structure_at(x - 1.0, z).water?;
                (high.level - low.level >= 8.0).then_some((x, z, high, low))
            })
        })
        .expect("closed terrace needs a connected outlet, not a dry gap");
    let generator = SectionGenerator::new(terrain);
    let base = global_to_section([
        pair.0.floor() as i64,
        pair.3.level.floor() as i64,
        pair.1.floor() as i64,
    ])
    .0;
    let mut wakes = Vec::new();
    for dy in 0..=1 {
        let generated = generator
            .build(
                base.offset([0, dy, 0]).unwrap(),
                0,
                &Default::default(),
                true,
            )
            .unwrap();
        wakes.extend(generated.fluid_wake);
        if let Some(water) = generated.water {
            assert!(
                water
                    .indices
                    .iter()
                    .all(|&i| (i as usize) < water.positions.len())
            );
            assert!(
                water.surface_effects.iter().all(|effect| effect[3] == 0.0),
                "reservoirs cannot invent waterfall foam before actual gravity flow exists"
            );
        }
    }
    assert!(
        !wakes.is_empty(),
        "legacy saved basins must wake the existing real liquid scheduler without regenerating their geometry"
    );
}

#[test]
fn generated_ecologies_are_deterministic_sealed_and_restricted_to_their_climate() {
    let terrain = TerraGenerator::new(revision_two()).unwrap();
    let generator = SectionGenerator::new(terrain.clone());
    for room in rooms(&terrain) {
        println!(
            "Cave {:?}: {:?}, radius {}",
            room.kind, room.center, room.radius
        );
        let [cx, _, cz] = room.center;
        assert_eq!(room, terrain.cave_structure_at(cx, cz));
        if room.kind == CaveEcology::Salt {
            assert!(terrain.climate_biome_at(cx, cz).weights.cliffted_desert > 0.5);
        }
        if room.kind == CaveEcology::Waterfall {
            assert!(terrain.surface_height(cx, cz) >= 80.0);
        }
        for offset in [-0.4, 0.0, 0.4] {
            let x = (cx + room.radius * offset).floor() + 0.5;
            let sample = terrain.cave_structure_at(x, cz.floor() + 0.5);
            for y in [(sample.floor - 1.5).floor(), (sample.ceiling + 1.5).ceil()] {
                if sample.open_to_sky() && y > sample.floor {
                    continue;
                }
                assert!(
                    generator
                        .block_at([x.floor() as i64, y as i64, cz.floor() as i64])
                        .unwrap()
                        != 0,
                    "open roof/floor {:?} {x} {y}",
                    room.kind
                );
            }
            if let Some(water) = sample.water {
                assert!(water.bed < water.level && water.level < sample.ceiling - 2.0);
                let p = [
                    x.floor() as i64,
                    (water.bed - 1.0).floor() as i64,
                    cz.floor() as i64,
                ];
                assert!(generator.block_at(p).unwrap() != 0, "leaking pool {p:?}");
                let wet = [p[0], water.bed as i64, p[2]];
                assert!(
                    crate::water::fluid_cell_at(&terrain, &Default::default(), wet, 0).amount > 0
                );
            }
        }
        let p = room.center.map(|v| v.floor() as i64);
        let section = global_to_section(p).0;
        let a = generator
            .build(section, 0, &Default::default(), true)
            .unwrap();
        let b = generator
            .build(section, 0, &Default::default(), false)
            .unwrap();
        assert_eq!(a.data, b.data);
        assert_eq!(a.props, b.props);
        if let Some(mesh) = a.mesh {
            assert!(
                mesh.mineral_growth
                    .iter()
                    .all(|(_, _, height)| *height <= 1.25)
            );
        }
    }
}

#[test]
fn salt_lengths_are_two_to_ten_blocks_and_dripstone_contains_wide_spanning_pillars() {
    let terrain = TerraGenerator::new(revision_two()).unwrap();
    for kind in [
        CaveEcology::Salt,
        CaveEcology::Dripstone,
        CaveEcology::Lush,
        CaveEcology::Waterfall,
    ] {
        let mut room = CaveStructure {
            kind,
            center: [-80.5, -30.0, -80.5],
            radius: 45.0,
            height: 20.0,
            seed: 192837,
            revision: 2,
            salt_allowed: true,
            ..Default::default()
        };
        let mut segments = 0;
        let mut pillars = 0;
        let mut plants = 0;
        for z in -115..-45 {
            for x in -115..-45 {
                let xf = f64::from(x) + 0.5;
                let zf = f64::from(z) + 0.5;
                room.radial = room.radial_at(xf, zf);
                [room.floor, room.ceiling] = room.geometry(xf, zf);
                if !room.active() {
                    continue;
                }
                let floor = (room.floor - 0.5).floor() as i64;
                let top = (room.ceiling - 0.5).ceil() as i64;
                let materials: Vec<_> = (floor..=top)
                    .map(|y| {
                        room.material(
                            [xf, y as f64 + 0.5, zf],
                            room.carve(y as f64 + 0.5, 1.0, 90.0),
                        )
                    })
                    .collect();
                if kind == CaveEcology::Salt {
                    let count = materials
                        .iter()
                        .filter(|&&m| m == Some(MaterialId::SaltCrystal))
                        .count();
                    if count > 0 {
                        assert!((2..=10).contains(&count), "salt height {count}");
                        segments += count;
                    }
                }
                if kind == CaveEcology::Dripstone
                    && materials
                        .iter()
                        .filter(|&&m| m == Some(MaterialId::DripstoneFormation))
                        .count()
                        > 20
                {
                    pillars += 1;
                }
                plants += materials
                    .iter()
                    .filter(|&&m| {
                        matches!(
                            m,
                            Some(
                                MaterialId::CaveRoot
                                    | MaterialId::CaveVine
                                    | MaterialId::CaveMushroom
                                    | MaterialId::Wood
                            )
                        )
                    })
                    .count();
            }
        }
        if kind == CaveEcology::Salt {
            assert!(segments > 50);
        }
        if kind == CaveEcology::Dripstone {
            assert!(pillars > 10 && pillars < 400, "pillar coverage {pillars}");
        }
        if kind == CaveEcology::Lush {
            assert!(plants > 30);
        }
    }
    assert!(terrain.config().cave_structure_revision > 0);
}

#[test]
fn salt_galleries_have_supported_bridges_clear_water_and_both_crystal_growth_directions() {
    let config = WorldConfig::default();
    let terrain = TerraGenerator::new(config.clone()).unwrap();
    let mut room = None;
    'search: for z in -20..20 {
        for x in -20..20 {
            let Some(center) = terrain
                .cave_chamber_center(f64::from(x) * 192.0 + 96.0, f64::from(z) * 192.0 + 96.0)
            else {
                continue;
            };
            let sample = terrain.cave_structure_at(center[0], center[2]);
            if sample.kind == CaveEcology::Salt
                && sample.active()
                && !sample.dynamic_water
                && sample.radius > 32.0
            {
                room = Some(sample);
                break 'search;
            }
        }
    }
    let room = room.expect("natural salt gallery");
    let previous = TerraGenerator::new(WorldConfig {
        cave_structure_revision: 8,
        ..config
    })
    .unwrap();
    let mut materials = std::collections::BTreeSet::new();
    let mut waters = 0;
    let mut bridges = 0;
    let [x, _, z] = room.center.map(|v| v.floor() as i64);
    for pz in (z - 55)..=(z + 55) {
        for px in (x - 55)..=(x + 55) {
            let (sx, sz) = (px as f64 + 0.5, pz as f64 + 0.5);
            let c = terrain.surface_column(sx, sz);
            if c.cave.kind != CaveEcology::Salt || !c.cave.active() {
                continue;
            }
            let old = previous.cave_structure_at(sx, sz);
            assert_eq!(old.salt_gallery, SaltGallery::default());
            if c.cave.water.is_some() {
                waters += 1;
            }
            let mut saw_air = false;
            for iy in (c.cave.floor.ceil() as i64)..(c.cave.ceiling as i64) {
                let sy = iy as f64 + 0.5;
                let d = c.cave.carve(sy, -3.0, c.height);
                if d > 1.0 {
                    saw_air = true;
                }
                if d < 0.0 && saw_air && sy < c.cave.ceiling - 2.0 {
                    bridges += 1;
                }
                if let Some(material) = terrain.cave_ecology_material([sx, sy, sz], c, d) {
                    materials.insert(material as u16);
                }
            }
        }
    }
    assert!(
        waters > 50 && bridges > 50,
        "water {waters}, bridge {bridges}"
    );
    for prop in 459..=463 {
        assert!(materials.contains(&prop), "missing salt formation {prop}");
    }
}

#[test]
fn long_caves_cross_region_seams_with_connected_vaults_banks_and_bridges() {
    let terrain = TerraGenerator::new(WorldConfig::default()).unwrap();
    let mut lengths = std::collections::BTreeSet::new();
    let mut crossed = [false; 2];
    'scan: for x in -80..0 {
        for z in -80..0 {
            let (region, _, gallery) = terrain.gallery_region([x, z]);
            let Some(g) = gallery else {
                continue;
            };
            lengths.insert(g.half_length as i64);
            if g.half_length < 112. {
                continue;
            }
            let kind = usize::from(region.salt);
            if crossed[kind] {
                continue;
            }
            let room = terrain.cave_room_at(region.center[0], region.center[2]);
            let (sin, cos) = g.angle.sin_cos();
            // The seam falls at u=0; both region owners must supply this very same hall.
            let mut active = 0;
            let mut bridges = std::collections::BTreeSet::new();
            let mut pools = 0;
            for u in -100..=100 {
                let u = f64::from(u);
                let v = g.bend(u, room.radius);
                let px = region.center[0] + u * cos - v * sin;
                let pz = region.center[2] + u * sin + v * cos;
                let sample = terrain.cave_room_at(px, pz);
                assert_eq!(sample.center, room.center);
                if sample.active() && sample.ceiling - sample.floor > 3. {
                    active += 1;
                }
                if sample.salt_gallery.bridge.is_some() {
                    bridges.insert((u / 15.).round() as i32);
                }
                if sample.water.is_some() {
                    pools += 1;
                }
            }
            assert_eq!(active, 201, "200-block unobstructed cavern axis");
            if kind == 1 {
                assert!(
                    bridges.len() >= 2 && pools > 60,
                    "bridges {bridges:?}, wet channel {pools}"
                );
            }
            crossed[kind] = true;
            if crossed == [true, true] && lengths.len() > 4 {
                break 'scan;
            }
        }
    }
    assert_eq!(crossed, [true, true]);
    assert!(lengths.len() > 4);
}

#[test]
fn revision_eleven_karst_forms_long_wet_galleries_with_joined_pillars_and_dry_banks() {
    let terrain = TerraGenerator::new(WorldConfig {
        cave_structure_revision: 11,
        ..Default::default()
    })
    .unwrap();
    let (region, gallery) = (-32..0)
        .flat_map(|x| (-32..0).map(move |z| [x, z]))
        .find_map(|cell| {
            let (region, _, gallery) = terrain.gallery_region(cell);
            let g = gallery?;
            (region.ecology == super::super::CaveBiome::Normal
                && region.kind == CaveKind::Dripstone
                && g.half_length >= 143.)
                .then_some((region, g))
        })
        .expect("paired dripstone gallery");
    assert!(region.radius >= 45.);
    let (s, c) = gallery.angle.sin_cos();
    let mut wet = 0;
    let mut pillars = 0;
    let mut dry = 0;
    for along in (-120..=120).step_by(2) {
        let u = f64::from(along);
        for bank in (-48..=48).step_by(2) {
            let v = gallery.bend(u, region.radius) + f64::from(bank);
            let x = region.center[0] + u * c - v * s;
            let z = region.center[2] + u * s + v * c;
            let room = terrain.cave_room_at(x, z);
            if room.kind != CaveEcology::Dripstone || !room.active() {
                continue;
            }
            let mid = (room.ceiling + room.floor) * 0.5;
            if let Some(water) = room.water {
                assert!(water.depth > 0. && water.depth <= 4.);
                wet += 1;
            } else {
                dry += 1;
            }
            if room.salt_gallery.density(5., mid) < 0. && room.salt_gallery.bridge.is_none() {
                assert!(
                    room.salt_gallery.density(5., room.ceiling - 0.5) < 0.
                        && room.salt_gallery.density(5., room.floor + 0.5) < 0.
                );
                pillars += 1;
            }
        }
    }
    assert!(
        wet > 300 && dry > 300 && pillars > 0,
        "wet {wet} dry {dry} continuous columns {pillars}"
    );
}

#[test]
fn living_galleries_generate_banked_water_native_palettes_and_rooted_ornaments() {
    use std::collections::BTreeSet;
    let terrain = TerraGenerator::new(WorldConfig::default()).unwrap();
    for (kind, required) in [
        (
            CaveEcology::Lush,
            [
                MaterialId::CaveGrass,
                MaterialId::OvergrownStone,
                MaterialId::CaveWildflowers,
            ],
        ),
        (
            CaveEcology::Waterfall,
            [
                MaterialId::SnowDustStone,
                MaterialId::FrozenStone,
                MaterialId::IceCrystal,
            ],
        ),
    ] {
        let center = (-96..96)
            .flat_map(|z| (-96..96).map(move |x| [x, z]))
            .filter_map(|[x, z]| {
                terrain.cave_chamber_center(f64::from(x * 192 + 96), f64::from(z * 192 + 96))
            })
            .find(|c| {
                let room = terrain.cave_structure_at(c[0], c[2]);
                room.active() && room.kind == kind && room.radius > 25.0
            })
            .unwrap();
        let mut ids = BTreeSet::new();
        let mut mushroom_sites = BTreeSet::new();
        let mut levels = BTreeSet::new();
        let (mut bridges, mut pools, mut empty) = (0, 0, 0);
        for dz in -32..=32 {
            for dx in -32..=32 {
                let x = center[0].floor() + f64::from(dx) + 0.5;
                let z = center[2].floor() + f64::from(dz) + 0.5;
                let c = terrain.surface_column(x, z);
                if !c.cave.active() || c.cave.kind != kind {
                    continue;
                }
                if let Some(water) = c.cave.water {
                    levels.insert(water.level.floor() as i64);
                    assert_eq!(c.cave.pool_ecology_at(water.level), Some(kind));
                    assert_eq!(c.cave.pool_ecology_at(c.cave.ceiling + 1.), None);
                }
                pools += usize::from(c.cave.water.is_some());
                empty += usize::from(c.cave.water.is_none());
                if let Some([lo, hi]) = c.cave.living.bridge {
                    let y = 0.5 * f64::from(lo + hi);
                    if c.cave.floor + 2. < f64::from(lo) && c.cave.ceiling - 2. > f64::from(hi) {
                        assert!(c.cave.living.density(4.0, y) <= 0.0);
                        assert!(c.cave.living.density(4.0, f64::from(lo) - 1.) > 0.0);
                        bridges += 1;
                    }
                }
                let floor = (c.cave.floor - 0.5).floor() as i64;
                let roof = (c.cave.ceiling - 0.5).ceil() as i64;
                for iy in [floor - 2, floor, floor + 1, roof - 2, roof - 1, roof] {
                    let sample = terrain.sample_with_column([x, iy as f64 + 0.5, z], c);
                    if sample.density <= 0.0 {
                        ids.insert(sample.material);
                        if sample.material == MaterialId::CaveMushroom as u16 {
                            mushroom_sites.insert((dx, dz));
                        }
                    }
                }
            }
        }
        if kind == CaveEcology::Lush {
            assert_eq!(
                levels.len(),
                3,
                "two separate Lush falls connect three pools"
            );
            let clustered = mushroom_sites
                .iter()
                .filter(|&&(x, z)| {
                    [(x + 1, z), (x - 1, z), (x, z - 1), (x, z + 1)]
                        .iter()
                        .any(|p| mushroom_sites.contains(p))
                })
                .count();
            assert!(
                clustered >= 4,
                "warm mushroom colonies need adjacent fruiting bodies"
            );
        }
        for material in required {
            assert!(
                ids.contains(&(material as u16)),
                "{kind:?} missing {material:?}: {ids:?}"
            );
        }
        assert!(
            bridges > 0 && pools > 0 && empty > 0,
            "{kind:?} bridges={bridges} wet={pools} dry={empty}"
        );
    }
}
