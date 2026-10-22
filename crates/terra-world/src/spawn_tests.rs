use crate::edits::EditJournal;
use crate::spawn::{WorldSpawn, find_land_spawn};
use crate::water::{FluidCell, fluid_cell_at};
use crate::{SectionGenerator, TerraGenerator};
use terra_core::{WorldConfig, WorldSeed};
use terra_voxel::global_to_section;

fn flat_config() -> WorldConfig {
    WorldConfig {
        base_height: 16.0,
        hill_amplitude: 0.0,
        mountain_amplitude: 0.0,
        caves: false,
        trees: false,
        rivers: false,
        biomes: false,
        ..Default::default()
    }
}

fn assert_dry_support(terrain: &TerraGenerator, journal: &EditJournal, spawn: WorldSpawn) {
    assert!(spawn.is_valid(terrain, journal));
    let generator = SectionGenerator::new(terrain.clone());
    let block = |p| {
        journal
            .block(p)
            .unwrap_or_else(|| generator.block_at(p).unwrap())
    };
    assert!(
        terra_voxel::MaterialId::from_raw(block(spawn.ground))
            .is_some_and(terra_voxel::MaterialId::is_natural_ground)
    );
    for height in 1..=3 {
        let p = [spawn.ground[0], spawn.ground[1] + height, spawn.ground[2]];
        assert_eq!(block(p), 0, "blocked spawn clearance at {p:?}");
        assert_eq!(fluid_cell_at(terrain, journal, p, 0).amount, 0);
    }
    let feet = spawn.feet();
    assert_eq!(feet[0], spawn.ground[0] as f64 + 0.5);
    assert_eq!(feet[2], spawn.ground[2] as f64 + 0.5);
    assert!((spawn.ground[1] as f64 + 1.0..=spawn.ground[1] as f64 + 1.25).contains(&feet[1]));
}

#[test]
fn ocean_origin_relocates_to_dry_land_deterministically_without_world_edits() {
    for seed in [1, 42] {
        let config = WorldConfig {
            seed: WorldSeed(seed),
            ..Default::default()
        };
        let terrain = TerraGenerator::new(config.clone()).unwrap();
        let journal = EditJournal::default();
        assert!(terrain.water_column(8.5, 24.5).is_some());
        let first = find_land_spawn(&terrain, &journal, [8, 24]).unwrap();
        assert_ne!([first.ground[0], first.ground[2]], [8, 24]);
        assert_dry_support(&terrain, &journal, first);
        let replay = TerraGenerator::new(config).unwrap();
        let second = find_land_spawn(&replay, &journal, [8, 24]).unwrap();
        assert_eq!(first.ground, second.ground);
        assert_eq!(journal.edited_blocks(), 0);
        assert_eq!(journal.fluid_cells().count(), 0);
        assert_eq!(journal.changed_sections().count(), 0);
    }
}

#[test]
fn negative_ocean_origin_finds_land_without_coordinate_rounding_errors() {
    let terrain = TerraGenerator::new(WorldConfig::default()).unwrap();
    let journal = EditJournal::default();
    let origin = [-3328, -8192];
    assert!(terrain.water_column(-3327.5, -8191.5).is_some());
    let spawn = find_land_spawn(&terrain, &journal, origin).unwrap();
    assert_dry_support(&terrain, &journal, spawn);
    assert_ne!([spawn.ground[0], spawn.ground[2]], origin);
}

#[test]
fn dry_negative_section_boundary_keeps_the_origin() {
    let terrain = TerraGenerator::new(flat_config()).unwrap();
    let journal = EditJournal::default();
    let spawn = find_land_spawn(&terrain, &journal, [-17, -33]).unwrap();
    assert_eq!(spawn.ground, [-17, 15, -33]);
    assert_dry_support(&terrain, &journal, spawn);
}

#[test]
fn new_ground_materials_are_valid_but_glass_and_fixed_water_are_not_land() {
    let terrain = TerraGenerator::new(flat_config()).unwrap();
    let original = find_land_spawn(&terrain, &EditJournal::default(), [-17, -33]).unwrap();
    for material in [11, 13, 19, 14, 15] {
        let mut journal = EditJournal::default();
        journal.record(original.ground, material).unwrap();
        assert_eq!(
            original.is_valid(&terrain, &journal),
            [11, 13, 19].contains(&material)
        );
    }
}

#[test]
fn river_above_sea_level_is_not_dry_land() {
    let terrain = TerraGenerator::new(WorldConfig {
        rivers: true,
        ..Default::default()
    })
    .unwrap();
    let journal = EditJournal::default();
    let (origin, river) = (-192..=192)
        .step_by(4)
        .find_map(|z| {
            (16..=144).step_by(4).find_map(|x| {
                let water = terrain.water_column(f64::from(x) + 0.5, f64::from(z) + 0.5)?;
                (water.level > terrain.config().sea_level + 1.0)
                    .then_some(([i64::from(x), i64::from(z)], water))
            })
        })
        .expect("fixture must include a real elevated river");
    assert!(river.level > terrain.config().sea_level + 1.0);
    let spawn = find_land_spawn(&terrain, &journal, origin).unwrap();
    assert_ne!([spawn.ground[0], spawn.ground[2]], origin);
    assert_dry_support(&terrain, &journal, spawn);
}

#[test]
fn disabled_water_allows_dry_ground_below_sea_level() {
    let terrain = TerraGenerator::new(WorldConfig {
        base_height: -16.0,
        water: false,
        ..flat_config()
    })
    .unwrap();
    let journal = EditJournal::default();
    let spawn = find_land_spawn(&terrain, &journal, [8, 24]).unwrap();
    assert_eq!(spawn.ground, [8, -17, 24]);
    assert_dry_support(&terrain, &journal, spawn);
}

#[test]
fn removed_support_in_a_loaded_save_invalidates_the_previous_spawn() {
    let terrain = TerraGenerator::new(flat_config()).unwrap();
    let mut journal = EditJournal::default();
    let original = find_land_spawn(&terrain, &journal, [-17, -33]).unwrap();
    journal.record(original.ground, 0).unwrap();
    let section = global_to_section(original.ground).0;
    let mut loaded = EditJournal::default();
    loaded.import_delta(journal.section_delta(section)).unwrap();
    assert!(!original.is_valid(&terrain, &loaded));
    let replacement = find_land_spawn(&terrain, &loaded, [-17, -33]).unwrap();
    assert_dry_support(&terrain, &loaded, replacement);
    assert_eq!(loaded.block(original.ground), Some(0));
    assert_eq!(loaded.edited_blocks(), 1);
}

#[test]
fn isolated_surface_block_is_not_an_acceptable_landing() {
    let terrain = TerraGenerator::new(flat_config()).unwrap();
    let mut journal = EditJournal::default();
    let original = find_land_spawn(&terrain, &journal, [8, 24]).unwrap();
    let beneath = [
        original.ground[0],
        original.ground[1] - 1,
        original.ground[2],
    ];
    journal.record(beneath, 0).unwrap();
    assert!(!original.is_valid(&terrain, &journal));
    let replacement = find_land_spawn(&terrain, &journal, [8, 24]).unwrap();
    assert_dry_support(&terrain, &journal, replacement);
    assert_eq!(journal.block(beneath), Some(0));
}

#[test]
fn all_three_clearance_cells_honor_saved_blocks_and_leaves() {
    let terrain = TerraGenerator::new(flat_config()).unwrap();
    let empty = EditJournal::default();
    let original = find_land_spawn(&terrain, &empty, [8, 24]).unwrap();
    for (height, material) in [(1, 3), (2, 6), (3, 7)] {
        let mut journal = EditJournal::default();
        let p = [
            original.ground[0],
            original.ground[1] + height,
            original.ground[2],
        ];
        journal.record(p, material).unwrap();
        assert!(!original.is_valid(&terrain, &journal));
        let replacement = find_land_spawn(&terrain, &journal, [8, 24]).unwrap();
        assert_dry_support(&terrain, &journal, replacement);
        assert_eq!(journal.block(p), Some(material));
    }
}

#[test]
fn flowing_water_in_saved_landing_cells_invalidates_dry_spawn() {
    let terrain = TerraGenerator::new(flat_config()).unwrap();
    let original = find_land_spawn(&terrain, &EditJournal::default(), [8, 24]).unwrap();
    for offset in [[0, 0], [-1, -1], [1, 1]] {
        let p = [
            original.ground[0] + offset[0],
            original.ground[1] + 1,
            original.ground[2] + offset[1],
        ];
        let mut journal = EditJournal::default();
        journal
            .set_fluid(
                p,
                FluidCell {
                    amount: 1,
                    flow: [1, 0],
                    source: false,
                },
            )
            .unwrap();
        let mut loaded = EditJournal::default();
        loaded
            .import_delta(journal.section_delta(global_to_section(p).0))
            .unwrap();
        assert!(
            !original.is_valid(&terrain, &loaded),
            "wet landing at {p:?}"
        );
        let replacement = find_land_spawn(&terrain, &loaded, [8, 24]).unwrap();
        assert_dry_support(&terrain, &loaded, replacement);
        assert_eq!(loaded.fluid(p).unwrap().amount, 1);
    }
}

#[test]
fn entirely_flooded_world_returns_an_error_without_creating_an_island() {
    let terrain = TerraGenerator::new(WorldConfig {
        base_height: -16.0,
        ..flat_config()
    })
    .unwrap();
    let journal = EditJournal::default();
    assert!(find_land_spawn(&terrain, &journal, [8, 24]).is_err());
    assert_eq!(journal.edited_blocks(), 0);
    assert_eq!(journal.changed_sections().count(), 0);
}

#[test]
fn out_of_precision_range_origins_fail_without_wrapping() {
    let terrain = TerraGenerator::new(flat_config()).unwrap();
    let journal = EditJournal::default();
    for origin in [[i64::MIN, 0], [0, i64::MAX]] {
        assert!(find_land_spawn(&terrain, &journal, origin).is_err());
    }
}
