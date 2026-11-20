use super::*;
use crate::generation::structures::{GraveDungeonInfo, GraveRoomPurpose};
use terra_core::WorldConfig;

fn fixture(variant: GraveVariant) -> Site {
    let terrain = TerraGenerator::new(WorldConfig {
        structures_revision: 10,
        base_height: 60.0,
        hill_amplitude: 0.0,
        mountain_amplitude: 0.0,
        water: false,
        biomes: false,
        ..Default::default()
    })
    .unwrap();
    let seed = match variant {
        GraveVariant::Knight => 3_u32,
        GraveVariant::Priestess => 4,
        GraveVariant::Scholar => 5,
    };
    let mut b = Builder {
        terrain: &terrain,
        summary: SiteSummary {
            id: u64::from(seed) << 32,
            kind: StructureKind::GraveDungeon,
            style: VillageStyle::Highland,
            center: [-192, 60, -192],
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
    let info = adapted_layout(&b.summary, 12).unwrap();
    b.summary.grave = Some(info.clone());
    let [x, y, z] = b.summary.center;
    ground::foundation(&mut b, [x - 11, y, z - 12], [x + 11, y + 15, z + 4], 3).unwrap();
    ground::perimeter(&mut b);
    chambers::mausoleum(&mut b, variant);
    ground::path(&mut b, [x, y, z - 12], [x, y, z - 31]).unwrap();
    chambers::dungeon(&mut b, &info);
    ground::grave_groups(&mut b, true);
    b.finish().unwrap()
}

fn assert_routes(site: &Site, info: &GraveDungeonInfo) {
    for pair in info.route.windows(2) {
        assert_eq!(
            (pair[0][0] - pair[1][0]).abs() + (pair[0][2] - pair[1][2]).abs(),
            1
        );
        assert!((pair[0][1] - pair[1][1]).abs() <= 1);
    }
    for p in &info.route {
        for dy in 0..=2 {
            assert_eq!(
                site.block_at([p[0], p[1] + dy, p[2]]),
                Some(0),
                "blocked at {p:?}+{dy}"
            );
        }
        assert!(
            site.block_at([p[0], p[1] - 1, p[2]])
                .is_some_and(|id| id != 0)
        );
    }
    for floor in &info.floors {
        assert!(
            floor
                .rooms
                .iter()
                .all(|room| info.route.contains(&room.center()))
        );
        assert_eq!(site.block_at(floor.view_eye), Some(0));
    }
    for p in &info.treasures {
        assert_eq!(site.block_at(*p), Some(377));
        assert!(
            site.sections
                .values()
                .flat_map(|section| &section.objects)
                .any(|object| object.position == *p
                    && object
                        .loot
                        .as_ref()
                        .is_some_and(|inventory| inventory.occupied_slots() > 0))
        );
    }
}

#[test]
fn all_variants_have_distinct_functional_floors_real_loot_and_collision_clear_routes() {
    for variant in [
        GraveVariant::Knight,
        GraveVariant::Priestess,
        GraveVariant::Scholar,
    ] {
        let site = fixture(variant);
        let info = site.summary.grave_dungeon().unwrap();
        assert_routes(&site, &info);
        assert_eq!(info.floors.len(), usize::from(variant.levels()));
        for pair in info.floors.windows(2) {
            let signature = |floor: &crate::generation::structures::GraveFloor| {
                floor
                    .rooms
                    .iter()
                    .map(|room| {
                        (
                            room.purpose,
                            room.bounds[1][0] - room.bounds[0][0],
                            room.bounds[1][2] - room.bounds[0][2],
                        )
                    })
                    .collect::<Vec<_>>()
            };
            assert_ne!(signature(&pair[0]), signature(&pair[1]));
        }
        let final_purpose = info.floors.last().unwrap().rooms[0].purpose;
        assert_eq!(
            final_purpose,
            match variant {
                GraveVariant::Knight => GraveRoomPurpose::BurialHall,
                GraveVariant::Priestess => GraveRoomPurpose::Sanctum,
                GraveVariant::Scholar => GraveRoomPurpose::CodexVault,
            }
        );
        let [x, y, z] = site.summary.center;
        for level in 0..variant.levels() {
            let side = if level.is_multiple_of(2) { -8 } else { 8 };
            super::super::tests::assert_walkable_descent(
                &site,
                [
                    x + side,
                    if level == 0 {
                        y
                    } else {
                        y - 12 - (i64::from(level) - 1) * GRAVE_LEVEL_HEIGHT
                    },
                    z - 8,
                ],
                if level == 0 { 12 } else { GRAVE_LEVEL_HEIGHT },
                1,
            );
        }
        let repeat = fixture(variant);
        assert_eq!(site.summary, repeat.summary);
        for (owner, section) in &site.sections {
            assert_eq!(section.cells, repeat.sections[owner].cells);
            assert_eq!(section.objects, repeat.sections[owner].objects);
        }
    }
}

#[test]
fn site_preserves_outdoor_soil_and_rock_between_rooms_instead_of_a_filled_cuboid() {
    let site = fixture(GraveVariant::Scholar);
    let [x, y, z] = site.summary.center;
    let mut untouched = 0;
    for dx in -25..=25 {
        for dz in -25..=25 {
            if site.block_at([x + dx, y - 2, z + dz]).is_none() {
                untouched += 1;
            }
        }
    }
    assert!(
        untouched > 2200,
        "53x53 footing was unexpectedly filled: {untouched} soil columns remain"
    );
    for level in 1..=4 {
        assert_eq!(
            site.block_at([x + 24, y - level * GRAVE_LEVEL_HEIGHT + 2, z + 22]),
            None,
            "natural rock outside chamber shells must remain untouched"
        );
    }
    assert!(
        site.sections
            .values()
            .map(|section| section.cells.len())
            .sum::<usize>()
            < 90_000
    );
}

#[test]
fn local_foundations_reject_large_fill_or_cut_before_writing() {
    let terrain = TerraGenerator::new(WorldConfig {
        base_height: 60.0,
        hill_amplitude: 0.0,
        mountain_amplitude: 0.0,
        water: false,
        biomes: false,
        ..Default::default()
    })
    .unwrap();
    let mut b = Builder {
        terrain: &terrain,
        summary: SiteSummary {
            id: 0,
            kind: StructureKind::Cemetery,
            style: VillageStyle::Highland,
            center: [192, 60, 192],
            bounds: [[0; 3]; 2],
            buildings: Vec::new(),
            containers: Vec::new(),
            grave: None,
        },
        patches: BTreeMap::new(),
        objects: BTreeMap::new(),
        writes: 0,
        failed: false,
        seed: 0,
    };
    for y in [50, 70] {
        assert!(ground::foundation(&mut b, [190, y, 190], [194, y + 7, 194], 3).is_none());
        assert!(b.patches.is_empty());
    }
    assert_eq!(
        ground::fitted_floor(&b, [188, 188], [196, 196], 3),
        Some(60)
    );
    ground::foundation(&mut b, [190, 62, 190], [194, 69, 194], 3).unwrap();
    assert!(
        b.patches
            .values()
            .all(|cells| cells.iter().filter(|id| **id == MOSS).count() <= 75)
    );
}

#[test]
fn terrain_path_preserves_individual_treads_and_joins_natural_ground() {
    let terrain = TerraGenerator::new(WorldConfig {
        base_height: 60.0,
        hill_amplitude: 0.0,
        mountain_amplitude: 0.0,
        water: false,
        caves: false,
        biomes: false,
        ..Default::default()
    })
    .unwrap();
    let mut b = Builder {
        terrain: &terrain,
        summary: SiteSummary {
            id: 0,
            kind: StructureKind::Cemetery,
            style: VillageStyle::Highland,
            center: [192, 62, 192],
            bounds: [[0; 3]; 2],
            buildings: Vec::new(),
            containers: Vec::new(),
            grave: None,
        },
        patches: BTreeMap::new(),
        objects: BTreeMap::new(),
        writes: 0,
        failed: false,
        seed: 0,
    };
    ground::path(&mut b, [192, 62, 192], [192, 60, 184]).unwrap();
    let site = b.finish().unwrap();
    super::super::tests::assert_walkable_descent(&site, [192, 62, 192], 2, -1);
    for x in 191..=193 {
        assert_eq!(site.block_at([x, 59, 184]), Some(MOSS));
        assert_eq!(site.block_at([x, 60, 184]), Some(0));
    }
}

#[test]
fn real_revision_ten_graves_stream_every_room_stair_treasure_and_exterior_approach() {
    use terra_core::Config;
    let config = WorldConfig {
        // Match the archived revision-ten grave finder world. Missing landscape
        // metadata in that TOML meant legacy terrain, with the revision-four river.
        structures_revision: 10,
        landscape_revision: 0,
        ecology_revision: 4,
        river_terrain_revision: 4,
        spirit_blossom_revision: 2,
        blood_forest_revision: 1,
        ..WorldConfig::from_toml(include_str!("../../../../../../../../configs/world.toml"))
            .unwrap()
    };
    assert_eq!(config.structures_revision, 10);
    let terrain = TerraGenerator::new(config).unwrap();
    let generator = crate::SectionGenerator::new(terrain.clone());
    for (region, variant) in [
        ([-8, -11], GraveVariant::Knight),
        ([16, 5], GraveVariant::Priestess),
        ([16, 6], GraveVariant::Scholar),
    ] {
        let site = terrain
            .planned_site(region)
            .expect("verified revision-ten natural fixture");
        let info = site.summary.grave_dungeon().unwrap();
        assert_eq!(info.variant, variant);
        assert_routes(&site, &info);
        for p in &info.route {
            for dy in 0..=2 {
                assert_eq!(
                    generator.block_at([p[0], p[1] + dy, p[2]]).unwrap(),
                    0,
                    "streamed path obstruction at {p:?}+{dy}"
                );
            }
            assert_ne!(generator.block_at([p[0], p[1] - 1, p[2]]).unwrap(), 0);
        }
        let mut top = site.summary.center[1];
        for landing in &info.levels {
            super::super::tests::assert_walkable_descent(
                &site,
                [landing[0], top, site.summary.center[2] - 8],
                top - landing[1],
                1,
            );
            top = landing[1];
        }
        for p in info.treasures {
            assert_eq!(generator.block_at(p).unwrap(), 377);
        }
    }
    let cemetery = terrain.structure_site([-18, -15]).unwrap();
    assert_eq!(cemetery.kind, StructureKind::Cemetery);
    let grave_heights = cemetery
        .buildings
        .iter()
        .filter(|b| b.bounds[1][0] - b.bounds[0][0] == 2)
        .map(|b| b.entrance[1])
        .collect::<std::collections::BTreeSet<_>>();
    assert!(
        grave_heights.len() > 1,
        "real hillside graves must retain distinct local heights"
    );
}
