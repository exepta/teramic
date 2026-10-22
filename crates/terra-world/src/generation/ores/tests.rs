use super::*;
use crate::{SectionGenerator, TerrainGenerator};
use terra_core::{OreGeneration, WorldConfig};
use terra_voxel::{MaterialId, global_to_section};

#[test]
fn bounded_deposits_obey_absolute_height_caps_without_changing_legacy_worlds() {
    let native = TerraGenerator::new(WorldConfig::default()).unwrap();
    let legacy = TerraGenerator::new(WorldConfig {
        bounded_world: false,
        ..Default::default()
    })
    .unwrap();
    for (ore, cap) in [
        (OreKind::Coal, 200.0),
        (OreKind::Iron, 170.0),
        (OreKind::Gold, 56.0),
        (OreKind::Diamond, -5.0),
        (OreKind::Ruby, -32.0),
    ] {
        let mut checked = false;
        native.deposits([-128, -128, -128], 64, |d| {
            if d.ore == ore {
                assert_eq!(d.ceiling, cap);
                checked = true;
            }
        });
        assert!(checked);
    }
    legacy.deposits([-64, 0, -64], 64, |d| assert!(d.ceiling.is_infinite()));
}

fn config() -> WorldConfig {
    let mut world = WorldConfig {
        base_height: 48.0,
        hill_amplitude: 0.0,
        mountain_amplitude: 0.0,
        biomes: false,
        caves: false,
        trees: false,
        water: false,
        ..Default::default()
    };
    for ore in OreKind::ALL {
        let disabled = OreVeinConfig {
            frequency: 0.0,
            ..world.ores.vein(ore)
        };
        match ore {
            OreKind::Salt => world.ores.salt = disabled,
            OreKind::Iron => world.ores.iron = disabled,
            OreKind::Coal => world.ores.coal = disabled,
            OreKind::Gold => world.ores.gold = disabled,
            OreKind::Diamond => world.ores.diamond = disabled,
            OreKind::Ruby => world.ores.ruby = disabled,
        }
    }
    world.ores.iron = OreVeinConfig {
        min_depth: 5.0,
        max_depth: 512.0,
        frequency: 1.0,
        radius: 8.0,
        cave_affinity: 0.0,
    };
    world
}
fn ore_position(terrain: &TerraGenerator) -> [i64; 3] {
    let mut position = None;
    terrain.deposits([-64, -64, -64], 64, |deposit| {
        let global = deposit.center.map(|value| value.floor() as i64);
        if position.is_none()
            && global.iter().all(|value| (-64..=0).contains(value))
            && terrain.sample(global.map(|v| v as f64 + 0.5)).material == OreKind::Iron.material()
        {
            position = Some(global);
        }
    });
    position.expect("rich deterministic deposit fixture")
}

#[test]
fn section_veins_match_direct_samples_at_negative_coordinates_and_neighbor_boundaries() {
    let terrain = TerraGenerator::new(config()).unwrap();
    let global = ore_position(&terrain);
    let center = global_to_section(global).0;
    let generator = SectionGenerator::new(terrain.clone());
    let mut count = 0;
    for dx in 0..=1 {
        let position = center.offset([dx, 0, 0]).unwrap();
        let section = generator.generate(position).unwrap();
        for z in 0..16 {
            for y in 0..16 {
                for x in 0..16 {
                    let global = position.global_block([x, y, z]).unwrap();
                    let direct = terrain.sample(global.map(|v| v as f64 + 0.5)).material;
                    assert_eq!(section.block([x, y, z]).unwrap(), direct);
                    if direct == 31 {
                        count += 1;
                    }
                }
            }
        }
        assert_eq!(section, generator.generate(position).unwrap());
    }
    assert!(count > 10);
    assert_eq!(terrain.version(), 5);
}

#[test]
fn legacy_worlds_keep_original_stone_and_native_ore_edits_survive_reload() {
    let world = config();
    let terrain = TerraGenerator::new(world.clone()).unwrap();
    let ore = ore_position(&terrain);
    let (position, local) = global_to_section(ore);
    let generator = SectionGenerator::new(terrain);
    let unedited = generator.generate(position).unwrap();
    let edits = crate::generation::sections::BlockOverrides::from([(ore, 0)]);
    let first = generator.build(position, 1, &edits, false).unwrap();
    let reload = generator.build(position, 1, &edits, false).unwrap();
    assert_eq!(first.data, reload.data);
    assert_eq!(first.data.block(local), Ok(0));
    assert!(
        first
            .data
            .blocks()
            .iter()
            .filter(|&&material| material == 31)
            .count()
            > 1
    );
    let mut legacy = world;
    legacy.ores = OreGeneration::legacy();
    let generator = SectionGenerator::new(TerraGenerator::new(legacy).unwrap());
    let old = generator.build(position, 1, &edits, false).unwrap();
    assert!(
        !old.data
            .blocks()
            .iter()
            .any(|&material| (30..=35).contains(&material))
    );
    let placed = crate::generation::sections::BlockOverrides::from([(ore, 34)]);
    let old = generator.build(position, 2, &placed, false).unwrap();
    assert_eq!(old.data.block(local), Ok(34));
    assert_eq!(
        old.data
            .blocks()
            .iter()
            .filter(|&&material| material == 34)
            .count(),
        1
    );
    assert!(unedited.blocks().contains(&31));
}

#[test]
fn frequency_zero_and_depth_controls_only_change_ore_material_not_the_landscape() {
    let mut world = config();
    let enabled = TerraGenerator::new(world.clone()).unwrap();
    let global = ore_position(&enabled);
    let ore = enabled.sample(global.map(|v| v as f64 + 0.5));
    assert_eq!(ore.material, 31);
    world.ores.iron.frequency = 0.0;
    let disabled = TerraGenerator::new(world.clone())
        .unwrap()
        .sample(global.map(|v| v as f64 + 0.5));
    assert_eq!(disabled.material, MaterialId::Stone as u16);
    assert_eq!(ore.density, disabled.density);
    world.ores.iron.frequency = 1.0;
    world.ores.iron.min_depth = 900.0;
    world.ores.iron.max_depth = 1000.0;
    let too_deep = TerraGenerator::new(world)
        .unwrap()
        .sample(global.map(|v| v as f64 + 0.5));
    assert_eq!(too_deep, disabled);
}

#[test]
fn salt_deposits_are_concentrated_in_salt_cave_regions_with_rare_ordinary_cave_occurrences() {
    let mut world = config();
    world.cave_structure_revision = 3;
    world.geology_revision = 0;
    world.ores = OreGeneration::default();
    let terrain = TerraGenerator::new(world).unwrap();
    let mut salty = 0;
    let mut ordinary = 0;
    let mut wall_biased = 0;
    terrain.deposits([-512, -256, -512], 512, |deposit| {
        if deposit.ore != OreKind::Salt {
            return;
        }
        if terrain.salt_cave_region(deposit.center.map(|value| value.floor() as i64)) {
            salty += 1;
        } else {
            ordinary += 1;
        }
        if deposit.cave {
            wall_biased += 1;
        }
    });
    assert!(
        ordinary > 0 && salty > ordinary * 3,
        "salt={salty}, ordinary={ordinary}"
    );
    assert!(wall_biased * 10 > (salty + ordinary) * 8);
}

#[test]
fn actual_salt_cave_wall_coverage_is_recognizable_and_ordinary_caves_remain_sparse() {
    let world = WorldConfig {
        cave_structure_revision: 3,
        trees: false,
        geology_revision: 0,
        ..Default::default()
    };
    let terrain = TerraGenerator::new(world).unwrap();
    let generator = SectionGenerator::new(terrain.clone());
    let mut inside = [0_u64; 2];
    let mut outside = [0_u64; 2];
    let mut example = None;
    let normals = [
        [-1, 0, 0],
        [1, 0, 0],
        [0, -1, 0],
        [0, 1, 0],
        [0, 0, -1],
        [0, 0, 1],
    ];
    for z in (-8..=8).step_by(2) {
        for x in (-8..=8).step_by(2) {
            for y in [-5, -3, -1] {
                let position = terra_voxel::SectionPos::new(x, y, z);
                let data = generator.generate(position).unwrap();
                for z in 1..15 {
                    for y in 1..15 {
                        for x in 1..15 {
                            let local = [x, y, z];
                            let material = data.block(local).unwrap();
                            if material != 3 && !(30..=35).contains(&material) {
                                continue;
                            }
                            let global = position.global_block(local).unwrap();
                            if global
                                .iter()
                                .any(|value| !(12..=84).contains(&value.rem_euclid(96)))
                            {
                                continue;
                            }
                            let normal = normals.into_iter().find(|normal| {
                                let neighbor = std::array::from_fn(|axis| {
                                    (local[axis] as i32 + normal[axis]) as usize
                                });
                                data.block(neighbor).unwrap() == 0
                            });
                            let Some(normal) = normal else {
                                continue;
                            };
                            let salty = terrain.salt_cave_region(global);
                            let totals = if salty { &mut inside } else { &mut outside };
                            totals[0] += 1;
                            totals[1] += u64::from(material == 30);
                            if salty && material == 30 && example.is_none() {
                                example = Some((global, normal));
                            }
                        }
                    }
                }
            }
        }
    }
    let inside_ratio = inside[1] as f64 / inside[0].max(1) as f64;
    let outside_ratio = outside[1] as f64 / outside[0].max(1) as f64;
    println!(
        "seed=42042 actual salt cave wall: {example:?}, inside={inside:?} ({inside_ratio:.3}), ordinary={outside:?} ({outside_ratio:.3})"
    );
    assert!(inside[0] > 100 && outside[0] > 100);
    assert!(example.is_some() && inside_ratio > 0.08);
    assert!(outside_ratio < 0.025 && inside_ratio > outside_ratio * 4.0);
}

#[test]
fn revision_four_salt_veins_are_sparse_separate_deposits_and_can_be_buried() {
    let mut world = WorldConfig::default();
    world.cave_biomes.distribution = terra_core::config::CaveDistribution {
        normal_weight: 0.0,
        salt_weight: 1.0,
        lush_weight: 0.0,
        cascade_weight: 0.0,
    };
    world.ores.salt.frequency = 1.0;
    let terrain = TerraGenerator::new(world).unwrap();
    let mut salt = 0;
    let mut buried = 0;
    terrain.deposits([-256, -96, -256], 512, |d| {
        if d.ore == OreKind::Salt {
            salt += 1;
            buried += usize::from(!d.cave);
            assert!(d.layered);
            assert!(terrain.salt_cave_region(d.center.map(|v| v.floor() as i64)));
        }
    });
    assert!(
        (10..1500).contains(&salt),
        "bounded rare prospects, even with frequency=1: {salt}"
    );
    assert!(buried > 0, "not every vein is glued onto an exposed wall");
}
