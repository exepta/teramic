use super::*;
use terra_core::{BiomeDefinitions, WorldConfig};

fn terrain() -> TerraGenerator {
    TerraGenerator::new(WorldConfig {
        landscape_revision: 8,
        river_terrain_revision: 7,
        rivers: true,
        water: true,
        trees: false,
        caves: false,
        cave_structure_revision: 0,
        structures_revision: 0,
        biome_definitions: Some(BiomeDefinitions::default()),
        ..Default::default()
    })
    .unwrap()
}

fn course_length(path: &RiverPath) -> f64 {
    path.points
        .windows(2)
        .map(|p| (p[1][0] - p[0][0]).hypot(p[1][2] - p[0][2]))
        .sum()
}

#[test]
fn default_terrain_has_a_continuous_wet_course_beyond_one_thousand_chunks_with_long_branches() {
    let terrain = terrain();
    let mut found = None;
    let mut forked = None;
    let mut longest = 0.0_f64;
    let mut long_without_tributary = 0;
    'regions: for z in -4..=4 {
        for x in -4..=4 {
            let center = [(x as f64 + 0.5) * BASIN, (z as f64 + 0.5) * BASIN];
            let Some(path) = terrain.macro_path_at(center[0], center[1]) else {
                continue;
            };
            let length = course_length(&path.main);
            longest = longest.max(length);
            if length > 16_000.0 && path.tributaries.is_empty() {
                long_without_tributary += 1;
            }
            if !path.distributaries.is_empty() {
                forked = Some(path.clone());
            }
            if found.is_none()
                && length > 16_000.0
                && path
                    .tributaries
                    .iter()
                    .any(|p| course_length(&p.path) > 2500.0)
            {
                found = Some(path);
            }
            if found.is_some() && forked.is_some() {
                break 'regions;
            }
        }
    }
    let network = found.unwrap_or_else(|| panic!(
        "default generation must contain real long land rivers, not only oversized catchment extents: longest={longest}, long_without_tributary={long_without_tributary}"
    ));
    let main = &network.main;
    eprintln!(
        "long hydrology source={:?} mouth={:?} length={} tributaries={} forks={}",
        main.frame.world(main.points[0][0], main.points[0][2]),
        main.frame
            .world(main.points[POINTS - 1][0], main.points[POINTS - 1][2]),
        course_length(main),
        network.tributaries.len(),
        network.distributaries.len()
    );
    let mut samples = 0;
    for pair in main.points.windows(2) {
        let distance = (pair[1][0] - pair[0][0]).hypot(pair[1][2] - pair[0][2]);
        let count = (distance / 16.0).ceil() as usize;
        for i in 0..count {
            let t = i as f64 / count as f64;
            let p = main.frame.world(
                pair[0][0] + (pair[1][0] - pair[0][0]) * t,
                pair[0][2] + (pair[1][2] - pair[0][2]) * t,
            );
            let sample = terrain
                .sample_network(&network, p[0], p[1])
                .expect("no gap in macro channel");
            assert!(sample.distance < sample.width);
            assert!(sample.level > sample.bed_height() + 0.5);
            if samples % 8 == 0 {
                let actual = terrain.surface_column(p[0], p[1]);
                assert!(
                    actual
                        .water_level
                        .is_some_and(|level| level > actual.water_bed()),
                    "dry rendered course at {p:?}: {actual:?}"
                );
            }
            samples += 1;
        }
    }
    assert!(samples > 1000);
    for branch in &network.tributaries {
        assert!(course_length(&branch.path) > 2500.0);
        let last = branch.path.points[POINTS - 1];
        let join = branch.path.frame.world(last[0], last[2]);
        let parent = terrain.sample_network(&network, join[0], join[1]).unwrap();
        assert!((last[1] - parent.level).abs() < 0.01);
        assert!(
            branch
                .path
                .points
                .windows(2)
                .all(|p| p[1][1] <= p[0][1] + 1e-8)
        );
    }
    let forked = forked.expect("long-distance outgoing forks must exist on default land terrain");
    let main = &forked.main;
    for branch in &forked.distributaries {
        assert!(course_length(&branch.path) > 2500.0);
        let split = main.points[branch.split_index];
        let start = branch.path.points[0];
        assert_eq!(
            main.frame.world(split[0], split[2]),
            branch.path.frame.world(start[0], start[2])
        );
        assert_eq!(split[1], start[1]);
        assert!(
            branch
                .path
                .points
                .windows(2)
                .all(|p| p[1][1] <= p[0][1] + 1e-8)
        );
        assert!(branch.path.lake.is_some());
    }
}

#[test]
fn small_river_target_lengths_vary_without_affecting_saved_revision_six() {
    let terrain = terrain();
    let mut lengths = Vec::new();
    for z in -3..=3 {
        for x in -3..=3 {
            if let Some(path) = terrain.build_catchment(
                [x, z],
                [x as f64 * 2048.0, z as f64 * 2048.0],
                2048.0,
                false,
            ) {
                lengths.push(course_length(&path.main));
            }
        }
    }
    assert!(lengths.len() >= 3);
    let low = lengths.iter().copied().fold(f64::INFINITY, f64::min);
    let high = lengths.iter().copied().fold(0.0_f64, f64::max);
    assert!(
        low < 650.0 && high > 1000.0,
        "short/long local streams missing: {lengths:?}"
    );
}
