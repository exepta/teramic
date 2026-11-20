use super::*;
use terra_core::WorldConfig;

fn fixture(variant: GraveVariant) -> Site {
    let terrain = TerraGenerator::new(WorldConfig {
        base_height: 60.0,
        hill_amplitude: 0.0,
        mountain_amplitude: 0.0,
        structures_revision: 9,
        water: false,
        biomes: false,
        ..Default::default()
    })
    .unwrap();
    let seed = match variant {
        GraveVariant::Knight => 3,
        GraveVariant::Priestess => 4,
        GraveVariant::Scholar => 5,
    };
    let mut b = Builder {
        terrain: &terrain,
        summary: SiteSummary {
            id: u64::from(seed) << 32,
            kind: StructureKind::GraveDungeon,
            style: VillageStyle::Highland,
            center: [-192, 64, -192],
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
    };
    courtyard(&mut b);
    dungeon(&mut b);
    b.finish().unwrap()
}

#[test]
fn all_three_tombs_have_connected_clear_supported_routes_and_real_treasure() {
    for variant in [
        GraveVariant::Knight,
        GraveVariant::Priestess,
        GraveVariant::Scholar,
    ] {
        let site = fixture(variant);
        let info = site.summary.grave_dungeon().unwrap();
        assert_eq!(info.variant, variant);
        assert_eq!(info.levels.len(), usize::from(variant.levels()));
        for pair in info.route.windows(2) {
            let a = pair[0];
            let b = pair[1];
            assert!(
                (a[0] - b[0]).abs() + (a[2] - b[2]).abs() <= 1,
                "disconnected route {a:?} -> {b:?}"
            );
            assert!((a[1] - b[1]).abs() <= 1, "unclimbable stair {a:?} -> {b:?}");
        }
        for p in &info.route {
            for dy in 0..=2 {
                assert_eq!(
                    site.block_at([p[0], p[1] + dy, p[2]]),
                    Some(0),
                    "blocked route {variant:?} at {p:?}+{dy}"
                );
            }
            assert!(
                site.block_at([p[0], p[1] - 1, p[2]])
                    .is_some_and(|material| material != 0),
                "unsupported route {variant:?} at {p:?}"
            );
        }
        for p in &info.treasures {
            assert_eq!(site.block_at(*p), Some(377));
            assert!(
                site.sections
                    .values()
                    .flat_map(|p| &p.objects)
                    .any(|o| o.position == *p && o.loot.is_some())
            );
        }
        assert!(site.settlement.is_none());
        assert!(site.sections.len() <= 256);
        assert!(site.sections.values().map(|p| p.cells.len()).sum::<usize>() < 300_000);
        let repeat = fixture(variant);
        assert_eq!(site.summary, repeat.summary);
        assert_eq!(site.sections.len(), repeat.sections.len());
        for (position, patch) in &site.sections {
            let other = &repeat.sections[position];
            assert_eq!(patch.cells, other.cells);
            assert_eq!(patch.objects, other.objects);
        }
    }
}

#[test]
fn old_revisions_and_non_spirit_regions_cannot_spawn_graves() {
    for revision in [0, 1, 2] {
        let terrain = TerraGenerator::new(WorldConfig {
            spirit_blossom_revision: revision,
            biomes: false,
            ..Default::default()
        })
        .unwrap();
        for region in [[0, 0], [-1, -1], [31, -42]] {
            assert!(plan(&terrain, region, 177).is_none());
        }
    }
}

#[test]
fn natural_spirit_sites_reach_all_levels_through_the_public_voxel_generator() {
    use crate::SectionGenerator;
    use terra_core::Config;
    let terrain = TerraGenerator::new(WorldConfig {
        structures_revision: 9,
        // These fixed sites were recorded from the original Spirit TOML world:
        // an absent landscape revision selected legacy terrain, not Rust defaults.
        landscape_revision: 0,
        ecology_revision: 4,
        river_terrain_revision: 4,
        spirit_blossom_revision: 2,
        blood_forest_revision: 0,
        ..WorldConfig::from_toml(include_str!("../../../../../../../configs/world.toml")).unwrap()
    })
    .unwrap();
    for (region, variant) in [
        ([-12, -4], GraveVariant::Scholar),
        ([14, 6], GraveVariant::Knight),
        ([8, 15], GraveVariant::Priestess),
    ] {
        let site = terrain.structure_site(region).unwrap();
        assert_natural_approach(&terrain, &terrain.planned_site(region).unwrap());
        let info = site.grave_dungeon().unwrap();
        assert_eq!(info.variant, variant);
        let generator = SectionGenerator::new(terrain.clone());
        for p in std::iter::once(info.entrance).chain(info.levels.iter().copied()) {
            for dy in 0..=2 {
                assert_eq!(
                    generator.block_at([p[0], p[1] + dy, p[2]]).unwrap(),
                    0,
                    "streamed obstruction at {p:?}"
                );
            }
            assert_ne!(generator.block_at([p[0], p[1] - 1, p[2]]).unwrap(), 0);
        }
        for p in info.treasures {
            assert_eq!(generator.block_at(p).unwrap(), 377);
        }
        assert!(site.containers.len() >= usize::from(variant.levels()));
    }
    let cemetery = terrain.structure_site([-8, -10]).unwrap();
    assert_natural_approach(&terrain, &terrain.planned_site([-8, -10]).unwrap());
    assert_eq!(cemetery.kind, StructureKind::Cemetery);
    assert!(cemetery.grave_dungeon().is_none());
    let old = TerraGenerator::new(WorldConfig {
        spirit_blossom_revision: 1,
        ..terrain.config().clone()
    })
    .unwrap();
    assert_eq!(old.grave_site_candidate([-8, -10]), None);
    let disabled = TerraGenerator::new(WorldConfig {
        structures: terra_core::StructureSettings {
            ruins: false,
            ..Default::default()
        },
        ..terrain.config().clone()
    })
    .unwrap();
    assert_eq!(disabled.grave_site_candidate([-8, -10]), None);
}

/// Read the same oriented collision boxes used by player physics, including
/// sub-voxel treads. A non-air voxel alone cannot prove a stair is traversable.
fn collision_floor(site: &Site, x: f64, z: f64, nominal_feet: i64) -> f64 {
    let cell_x = x.floor() as i64;
    let cell_z = z.floor() as i64;
    let mut height = f64::NEG_INFINITY;
    for y in nominal_feet - 2..nominal_feet {
        let cell = [cell_x, y, cell_z];
        let Some(material) = site.block_at(cell).filter(|id| *id != 0) else {
            continue;
        };
        if terra_core::construction::is_shape(material) {
            let owner = terra_voxel::global_to_section(cell).0;
            let facing = site.sections[&owner]
                .objects
                .iter()
                .find(|o| o.position == cell)
                .and_then(|o| o.state)
                .map_or(0, |state| state.facing);
            for bounds in terra_core::construction::placed_boxes(material, facing, [None; 6]) {
                let local = [x - cell_x as f64, z - cell_z as f64];
                if [0, 2]
                    .into_iter()
                    .enumerate()
                    .all(|(i, a)| local[i] >= bounds.minimum[a] && local[i] < bounds.maximum[a])
                {
                    height = height.max(y as f64 + bounds.maximum[1]);
                }
            }
        } else {
            height = height.max((y + 1) as f64);
        }
    }
    assert!(
        height.is_finite(),
        "no support at ({x}, {z}), feet={nominal_feet}"
    );
    height
}

pub(super) fn assert_walkable_descent(site: &Site, start: [i64; 3], depth: i64, direction: i64) {
    let mut previous = None;
    let mut first = None;
    let mut last = 0.0;
    // Twelfths sample every third-height riser plus both floor connections.
    for tick in -6..=depth * 12 + 6 {
        let distance = tick as f64 / 12.0 + 0.00001;
        let z = if direction > 0 {
            start[2] as f64 + distance
        } else {
            (start[2] + 1) as f64 - distance
        };
        let feet = start[1] - (distance.floor() as i64).clamp(0, depth);
        let height = collision_floor(site, start[0] as f64 + 0.5, z, feet);
        if let Some(previous) = previous {
            let drop: f64 = previous - height;
            assert!(
                (-0.000001..=0.334).contains(&drop),
                "unwalkable stair at {start:?}, distance={distance}, height={height}, preceding={previous}"
            );
        }
        first.get_or_insert(height);
        previous = Some(height);
        last = height;
    }
    assert!((first.unwrap() - last - depth as f64).abs() < 0.000001);
}

#[test]
fn actual_stair_collision_has_third_block_steps_and_flush_landings_in_both_directions() {
    for variant in [
        GraveVariant::Knight,
        GraveVariant::Priestess,
        GraveVariant::Scholar,
    ] {
        let site = fixture(variant);
        let [x, y, z] = site.summary.center;
        for level in 0..variant.levels() {
            let side = if level % 2 == 0 { -10 } else { 10 };
            assert_walkable_descent(
                &site,
                [x + side, y - i64::from(level) * GRAVE_LEVEL_HEIGHT, z - 10],
                GRAVE_LEVEL_HEIGHT,
                1,
            );
        }
        // The fixture platform is four metres above its flat natural terrain.
        assert_walkable_descent(&site, [x, y, z - RADIUS], 4, -1);
    }
}

#[test]
fn rising_approach_terrain_replaces_last_treads_with_a_flush_landing() {
    let terrain = TerraGenerator::new(WorldConfig {
        base_height: 60.0,
        hill_amplitude: 0.0,
        mountain_amplitude: 0.0,
        biomes: false,
        structures_revision: 9,
        water: false,
        ..Default::default()
    })
    .unwrap();
    let mut builder = Builder {
        terrain: &terrain,
        summary: SiteSummary {
            id: 3 << 32,
            kind: StructureKind::GraveDungeon,
            style: VillageStyle::Highland,
            center: [-192, 64, -192],
            bounds: [[0; 3]; 2],
            buildings: Vec::new(),
            containers: Vec::new(),
            grave: None,
        },
        patches: BTreeMap::new(),
        objects: BTreeMap::new(),
        writes: 0,
        failed: false,
        seed: 3,
    };
    courtyard(&mut builder);
    let [x, y, z] = builder.summary.center;
    // At the second cell the terrain rises to 63: the proposed foot level is
    // 62. The preceding 63m tread must become a flat landing, too.
    level_approach_landing(&mut builder, 2, 63);
    let site = builder.finish().unwrap();
    let mut previous = 64.0;
    for tick in -6..=30 {
        let distance = tick as f64 / 12.0 + 0.00001;
        let point_z = (z - RADIUS + 1) as f64 - distance;
        let feet = if distance < 1.0 { y } else { 63 };
        let height = collision_floor(&site, x as f64 + 0.5, point_z, feet);
        assert!(
            (-0.000001..=0.334).contains(&(previous - height)),
            "rising approach lip at {distance}: {previous} -> {height}"
        );
        previous = height;
    }
    assert!((previous - 63.0).abs() < 1e-9);
}

fn assert_natural_approach(terrain: &TerraGenerator, site: &Site) {
    let [x, y, z] = site.summary.center;
    let (last_step, ground) = (0..=16)
        .find_map(|step| {
            let ground = (terrain.surface_height(x as f64 + 0.5, (z - RADIUS - step) as f64 + 0.5)
                - 0.5)
                .floor() as i64
                + 1;
            (ground >= y - step).then_some((step, ground))
        })
        .expect("accepted site must connect to natural ground within its approach budget");
    let depth = y - ground;
    assert!((0..=16).contains(&depth));
    assert_walkable_descent(site, [x, y, z - RADIUS], depth, -1);
    for step in depth..=last_step {
        let floor = collision_floor(
            site,
            x as f64 + 0.5,
            (z - RADIUS - step) as f64 + 0.5,
            ground,
        );
        assert!(
            (floor - ground as f64).abs() < 1e-9,
            "uneven terrain connection {step}"
        );
    }
}
