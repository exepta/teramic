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

fn revision_two() -> WorldConfig {
    WorldConfig {
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
    assert_eq!(WorldConfig::default().cave_structure_revision, 8);
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
    assert!(WorldConfig::from_toml("cave_structure_revision=9").is_err());
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
