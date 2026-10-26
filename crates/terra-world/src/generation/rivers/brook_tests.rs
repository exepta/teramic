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
