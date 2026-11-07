use super::*;
use terra_core::{WorldConfig, WorldSeed};

#[derive(Debug, Default)]
struct Density {
    rivers: usize,
    tributaries: usize,
    brooks: usize,
    river_metres: f64,
    brook_metres: f64,
}

fn terrain(seed: u64, revision: u8) -> TerraGenerator {
    TerraGenerator::new(WorldConfig {
        seed: WorldSeed(seed),
        river_terrain_revision: revision,
        landscape_revision: 1,
        rivers: true,
        caves: false,
        trees: false,
        structures_revision: 0,
        ..Default::default()
    })
    .unwrap()
}

fn visible_length(terrain: &TerraGenerator, course: &[[f64; 3]; POINTS]) -> f64 {
    course
        .windows(2)
        .filter_map(|pair| {
            let [x, z] = [
                (pair[0][0] + pair[1][0]) * 0.5,
                (pair[0][2] + pair[1][2]) * 0.5,
            ];
            if !(-2048.0..2048.0).contains(&x) || !(-2048.0..2048.0).contains(&z) {
                return None;
            }
            let river = terrain.river_at(x, z)?;
            let water = terrain.water_column(x, z)?;
            (!river.covered
                && river.distance < river.width
                && water.bed < water.level
                && water.level > terrain.config().sea_level + 0.5)
                .then_some((pair[1][0] - pair[0][0]).hypot(pair[1][2] - pair[0][2]))
        })
        .sum()
}

fn density(terrain: &TerraGenerator) -> Density {
    let mut density = Density::default();
    for z in -1..=0 {
        for x in -1..=0 {
            let Some(region) = terrain.hydrology_region_at(
                f64::from(x) * 2048.0 + 1024.0,
                f64::from(z) * 2048.0 + 1024.0,
            ) else {
                continue;
            };
            density.rivers += 1;
            density.tributaries += region.tributaries.len();
            density.river_metres += visible_length(terrain, &region.course);
            for branch in &region.tributaries {
                density.river_metres += visible_length(terrain, &branch.course);
            }
            for branch in &region.distributaries {
                density.river_metres += visible_length(terrain, &branch.course);
            }
        }
    }
    // Include boundary courses, but count only their actual open wet length in
    // the same 4096 x 4096 m window, not bank-influence area or ocean water.
    for z in -4..=4 {
        for x in -4..=4 {
            let cell = [x, z];
            if !terrain.drainage_enabled(cell, true) {
                continue;
            }
            let path = terrain.build_drainage(cell, true);
            if terrain.brook_conflicts_with_region(&path) {
                continue;
            }
            let length = visible_length(terrain, &path.points);
            if length > 0.0 {
                density.brooks += 1;
                density.brook_metres += length;
            }
        }
    }
    density
}

#[test]
fn revision_three_increases_actual_open_watercourse_density_on_the_main_terrain() {
    let mut old_length = 0.0;
    let mut new_length = 0.0;
    let mut old_brooks = 0;
    let mut new_brooks = 0;
    let mut new_tributaries = 0;
    for seed in [42042, 7, 99173] {
        let before = density(&terrain(seed, 2));
        let after = density(&terrain(seed, 3));
        println!(
            "hydrology_density seed={seed} area_km2=16.777216 before={before:?} after={after:?}"
        );
        assert_eq!(before.tributaries, 0);
        assert_eq!(
            before.rivers, after.rivers,
            "existing catchments remain bounded"
        );
        old_length += before.river_metres + before.brook_metres;
        new_length += after.river_metres + after.brook_metres;
        old_brooks += before.brooks;
        new_brooks += after.brooks;
        new_tributaries += after.tributaries;
    }
    assert!(
        new_tributaries > 0,
        "normal terrain must receive real tributaries"
    );
    assert!(
        new_brooks > old_brooks,
        "brook habitats must visibly expand"
    );
    assert!(
        new_length > old_length * 1.15,
        "open wet channel length must increase meaningfully: {old_length} -> {new_length}"
    );
}

#[test]
fn revision_three_keeps_old_course_selection_and_river_toggle_contracts() {
    for seed in [42042, 7, 99173] {
        let old = terrain(seed, 2);
        let new = terrain(seed, 3);
        for z in -4..=4 {
            for x in -4..=4 {
                let cell = [x, z];
                if old.drainage_enabled(cell, true) {
                    assert!(new.drainage_enabled(cell, true));
                    assert_eq!(
                        old.build_drainage(cell, true).points,
                        new.build_drainage(cell, true).points
                    );
                }
            }
        }
        let mut disabled = new.config().clone();
        disabled.rivers = false;
        let old_disabled = TerraGenerator::new(WorldConfig {
            river_terrain_revision: 2,
            ..disabled.clone()
        })
        .unwrap();
        let new_disabled = TerraGenerator::new(disabled).unwrap();
        for [x, z] in [[-320.5, -175.5], [-85.5, 16.5], [1302.5, -1024.5]] {
            assert_eq!(old_disabled.river_at(x, z), new_disabled.river_at(x, z));
        }
    }
}

#[test]
fn revision_four_brooks_fit_their_cells_and_preserve_regional_rivers_and_disabled_worlds() {
    for seed in [42042, 7, 99173, 12345, 86420] {
        let old = terrain(seed, 3);
        let current = terrain(seed, 4);
        let [spacing, _, _] = current.drainage_layout(true);
        assert_eq!(spacing, 416.0);
        assert_eq!(old.drainage_layout(true), [SPACING, START, STEP]);
        for z in -4..=4 {
            for x in -4..=4 {
                let cell = [x, z];
                let path = current.build_drainage(cell, true);
                let maximum_radius = path.width * 1.38 + 1.2 + 12.0;
                assert_eq!(path.extensions, [0.0, 0.0]);
                for point in path.points {
                    assert!((point[0] - x as f64 * spacing).abs() + maximum_radius < spacing * 0.5);
                    assert!((point[2] - z as f64 * spacing).abs() + maximum_radius < spacing * 0.5);
                }
                for boundary in [-spacing * 0.5, spacing * 0.5] {
                    assert!(
                        current
                            .drainage_at(
                                x as f64 * spacing - 80.0,
                                z as f64 * spacing + boundary,
                                true
                            )
                            .is_none()
                    );
                }
            }
        }
        for z in -1..=0 {
            for x in -1..=0 {
                let [x, z] = [
                    f64::from(x) * 2048.0 + 1024.0,
                    f64::from(z) * 2048.0 + 1024.0,
                ];
                if let Some(region) = current.hydrology_region_at(x, z) {
                    let original = old.hydrology_region_at(x, z).unwrap();
                    assert_eq!(region.course, original.course);
                    assert_eq!(region.tributaries.len(), original.tributaries.len());
                    assert!(region.tributaries.len() <= 3);
                    for tributary in &region.tributaries {
                        assert!(
                            tributary
                                .course
                                .windows(2)
                                .all(|pair| pair[1][1] <= pair[0][1])
                        );
                        let mouth = current
                            .river_at(tributary.mouth[0], tributary.mouth[2])
                            .unwrap();
                        assert!((mouth.level - tributary.mouth[1]).abs() < 1e-6);
                    }
                }
            }
        }
        let old = TerraGenerator::new(WorldConfig {
            rivers: false,
            ..old.config().clone()
        })
        .unwrap();
        let current = TerraGenerator::new(WorldConfig {
            rivers: false,
            ..current.config().clone()
        })
        .unwrap();
        for [x, z] in [
            [-2048.5, -1024.5],
            [-16.001, -16.001],
            [-16.0, -16.0],
            [-15.999, -15.999],
            [1302.5, -1024.5],
        ] {
            assert_eq!(old.river_at(x, z), current.river_at(x, z));
            assert_eq!(old.surface_column(x, z), current.surface_column(x, z));
        }
    }
}

#[test]
fn revision_four_increases_real_open_river_coverage_across_multiple_seeds() {
    fn coverage(terrain: &TerraGenerator) -> [u64; 2] {
        let mut counts = [0, 0];
        for z in (-2048..2048).step_by(16) {
            for x in (-2048..2048).step_by(16) {
                let [x, z] = [f64::from(x) + 8.5, f64::from(z) + 8.5];
                let column = terrain.surface_column(x, z);
                if terrain.surface_biome(&column) == terra_core::BiomeKind::Ocean {
                    continue;
                }
                counts[0] += 1;
                if terrain.river_at(x, z).is_some_and(|river| {
                    !river.covered
                        && river.distance < river.width
                        && column.water_level.is_some_and(|level| {
                            level > terrain.config().sea_level + 0.5
                                && column.water_bed() < level
                                && (level - river.level).abs() < 0.01
                        })
                }) {
                    counts[1] += 1;
                }
            }
        }
        counts
    }
    let mut before = [0, 0];
    let mut after = [0, 0];
    for seed in [42042, 7, 99173, 12345, 86420] {
        let config = WorldConfig {
            biome_definitions: Some(terra_core::BiomeDefinitions::default()),
            ..terrain(seed, 3).config().clone()
        };
        let old = TerraGenerator::new(config.clone()).unwrap();
        let current = TerraGenerator::new(WorldConfig {
            river_terrain_revision: 4,
            ..config
        })
        .unwrap();
        let old_coverage = coverage(&old);
        let new_coverage = coverage(&current);
        println!("river_coverage seed={seed} old={old_coverage:?} current={new_coverage:?}");
        assert!(new_coverage[1] > old_coverage[1], "seed {seed}");
        for index in 0..2 {
            before[index] += old_coverage[index];
            after[index] += new_coverage[index];
        }
    }
    // Count only real open channel water and normalize by real land, not bank
    // influence, potential catchments, or ocean/lake water. No timing assertion.
    assert!(after[1] * before[0] * 100 > before[1] * after[0] * 110);
}
