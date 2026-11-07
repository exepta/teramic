//! Reproducible production hydrology anchors and bounded warm/cold probe costs.
use std::time::Instant;
use terra_core::{WorldConfig, WorldSeed};
use terra_world::TerraGenerator;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut seeds = vec![42042, 7, 99173];
    let mut radius = 2_i64;
    for arg in std::env::args().skip(1) {
        if let Some(seed) = arg.strip_prefix("--seed=") {
            seeds = vec![seed.parse()?];
        } else if let Some(value) = arg.strip_prefix("--radius=") {
            radius = value.parse()?;
            if !(0..=8).contains(&radius) {
                return Err("radius must be between 0 and 8".into());
            }
        } else {
            return Err(format!("Unknown option {arg}").into());
        }
    }
    for seed in seeds {
        let terrain = TerraGenerator::new(WorldConfig {
            seed: WorldSeed(seed),
            landscape_revision: 3,
            rivers: true,
            caves: false,
            trees: false,
            structures_revision: 0,
            ..Default::default()
        })?;
        let start = Instant::now();
        let mut courses = 0;
        let mut lakes = 0;
        let mut coasts = 0;
        let mut lengths = [f64::INFINITY, 0.0_f64];
        let mut radii = [f64::INFINITY, 0.0_f64];
        let mut queries = Vec::new();
        for z in -radius..=radius {
            for x in -radius..=radius {
                let Some(region) = terrain
                    .hydrology_region_at(x as f64 * 2048.0 + 1024.0, z as f64 * 2048.0 + 1024.0)
                else {
                    continue;
                };
                courses += 1;
                coasts += usize::from(region.reaches_sea);
                lengths[0] = lengths[0].min(region.course_length);
                lengths[1] = lengths[1].max(region.course_length);
                if let Some(lake) = region.lake {
                    lakes += 1;
                    radii[0] = radii[0].min(lake.radius);
                    radii[1] = radii[1].max(lake.radius);
                    let [cx, cz] = lake.center;
                    let water = terrain.water_column(cx, cz);
                    println!(
                        "LAKE seed={seed} region={:?} anchor=[{cx:.3},{cz:.3}] radius={:.1} level={:.3} depth={:.3} inlet_outlet={} course_m={:.1}",
                        region.region,
                        lake.radius,
                        lake.level,
                        water.map_or(0.0, |w| w.depth),
                        region.reaches_sea,
                        region.course_length,
                    );
                }
                if let Some(point) = region.course[4..45].iter().find(|p| {
                    terrain
                        .river_at(p[0], p[2])
                        .is_some_and(|r| !r.covered && r.level > terrain.config().sea_level + 0.5)
                        && terrain
                            .water_column(p[0], p[2])
                            .is_some_and(|w| w.depth >= 1.0)
                }) {
                    println!(
                        "RIVER seed={seed} region={:?} anchor=[{:.3},{:.3}] source={:?} mouth={:?} sea={} length_m={:.1}",
                        region.region,
                        point[0],
                        point[2],
                        region.source,
                        region.mouth,
                        region.reaches_sea,
                        region.course_length,
                    );
                }
                queries.extend(region.course.into_iter().map(|p| [p[0], p[2]]));
            }
        }
        let cold_ms = start.elapsed().as_secs_f64() * 1000.0;
        let warm = Instant::now();
        for _ in 0..10 {
            for &[x, z] in &queries {
                std::hint::black_box((terrain.river_at(x, z), terrain.lake_at(x, z)));
            }
        }
        println!(
            "SUMMARY seed={seed} regions={} courses={courses} lakes={lakes} coast_outlets={coasts} length_m={:.1}..{:.1} lake_radius_m={:.1}..{:.1} cold_ms={cold_ms:.3} warm_queries={} warm_ms={:.3}",
            (radius * 2 + 1).pow(2),
            lengths[0],
            lengths[1],
            radii[0],
            radii[1],
            queries.len() * 10,
            warm.elapsed().as_secs_f64() * 1000.0,
        );
    }
    Ok(())
}
