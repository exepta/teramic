use super::*;
use terra_core::{WorldConfig, WorldSeed};

#[test]
fn brooks_use_existing_drains_and_flow_downhill_across_negative_section_edges() {
    let mut found = 0;
    let mut stepped = 0;
    let mut biomes = [false; 3];
    for seed in [42042, 7, 99173] {
        let config = WorldConfig {
            seed: WorldSeed(seed),
            rivers: false,
            landscape_revision: 2,
            ..Default::default()
        };
        let a = TerraGenerator::new(config.clone()).unwrap();
        let b = TerraGenerator::new(config.clone()).unwrap();
        let old = TerraGenerator::new(WorldConfig {
            ecology_revision: 0,
            ..config
        })
        .unwrap();
        for cell in (-5..=5).flat_map(|z| (-5..=5).map(move |x| [x, z])) {
            if !a.drainage_enabled(cell, true) {
                continue;
            }
            let path = a.build_drainage(cell, true);
            let mut previous = f64::INFINITY;
            for pair in path.points.windows(2) {
                stepped += usize::from((0.25..=4.0).contains(&(pair[0][1] - pair[1][1])));
            }
            for point in &path.points {
                let sample = a.river_at(point[0], point[2]).unwrap();
                assert_eq!(Some(sample), b.river_at(point[0], point[2]));
                assert!(old.river_at(point[0], point[2]).is_none());
                assert!(sample.level <= previous + 1e-9);
                assert!(sample.width < 3.5 && sample.depth <= 1.31);
                assert!((sample.flow[0].hypot(sample.flow[1]) - 1.0).abs() < 1e-5);
                previous = sample.level;
                let w = a.climate_biome_at(point[0], point[2]).weights;
                biomes[0] |= w.cherry_forest > 0.5;
                biomes[1] |= w.forest > 0.5;
                biomes[2] |= w.highland > 0.5;
            }
            found += 1;
            if found < 4 {
                eprintln!(
                    "brook seed={seed} at={:?} width={}",
                    path.points[24], path.width
                );
            }
        }
        assert!(a.rivers.0.lock().unwrap().len() <= 256);
    }
    assert!(
        found > 10 && stepped > 10,
        "paths={found}, small steps={stepped}"
    );
    assert!(
        biomes.into_iter().all(|b| b),
        "brooks must reach cherry, forest and highland: {biomes:?}"
    );
}

#[test]
fn revision_three_preserves_whole_cherry_forest_and_highland_brooks_without_hydro_conflicts() {
    let mut habitats = [false; 3];
    let mut accepted = 0;
    let mut rejected = 0;
    let mut negative = false;
    for seed in [42042, 7, 99173] {
        let terrain = TerraGenerator::new(WorldConfig {
            seed: WorldSeed(seed),
            landscape_revision: 3,
            river_terrain_revision: 1,
            rivers: true,
            caves: false,
            trees: false,
            structures_revision: 0,
            ..Default::default()
        })
        .unwrap();
        for cell in (-5..=5).flat_map(|z| (-5..=5).map(move |x| [x, z])) {
            if !terrain.drainage_enabled(cell, true) {
                continue;
            }
            let path = terrain.build_drainage(cell, true);
            let conflict = terrain.brook_conflicts_with_region(&path);
            if conflict {
                rejected += 1;
                for point in path.points.iter().step_by(6) {
                    assert!(terrain.drainage_at(point[0], point[2], true).is_none());
                }
                continue;
            }
            accepted += 1;
            negative |= cell[0] < 0 || cell[1] < 0;
            let mut previous = f64::INFINITY;
            for point in path.points {
                let sample = terrain.river_at(point[0], point[2]).unwrap();
                assert!(sample.level <= previous + 1e-9);
                previous = sample.level;
                assert!(sample.width < 3.5 && sample.depth <= 1.31);
                assert!(terrain.connected_river_at(point[0], point[2]).is_none());
                assert!(terrain.lake_at(point[0], point[2]).is_none());
                assert!(terrain.water_column(point[0], point[2]).is_some());
                let w = terrain.climate_biome_at(point[0], point[2]).weights;
                habitats[0] |= w.cherry_forest > 0.5;
                habitats[1] |= w.forest > 0.5;
                habitats[2] |= w.highland > 0.5;
            }
        }
        assert!(terrain.rivers.0.lock().unwrap().len() <= 256);
        assert!(terrain.rivers.1.lock().unwrap().len() <= 256);
    }
    assert!(
        accepted > 10 && rejected > 0 && negative,
        "{accepted}/{rejected}/{negative}"
    );
    assert!(
        habitats.into_iter().all(|present| present),
        "missing brook habitats {habitats:?}"
    );
}
