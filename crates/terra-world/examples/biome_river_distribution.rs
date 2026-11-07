//! Deterministic production-sampler coverage, not noise thresholds or bank influence.
use std::time::Instant;
use terra_core::{BiomeDefinitions, BiomeKind, WorldConfig, WorldSeed};
use terra_world::TerraGenerator;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut seeds = vec![42042, 7, 99173, 12345, 86420];
    let mut current = false;
    let mut step = 16_i64;
    let mut river_revision = None;
    for argument in std::env::args().skip(1) {
        if argument == "--current" {
            current = true;
        } else if let Some(value) = argument.strip_prefix("--seeds=") {
            seeds = value.split(',').map(str::parse).collect::<Result<_, _>>()?;
        } else if let Some(value) = argument.strip_prefix("--river-step=") {
            step = value.parse()?;
            if ![8, 16, 32].contains(&step) {
                return Err("river-step must be 8, 16 or 32 metres".into());
            }
        } else if let Some(value) = argument.strip_prefix("--river-revision=") {
            river_revision = Some(value.parse::<u8>()?);
        } else {
            return Err(format!("Unknown argument {argument}").into());
        }
    }
    if seeds.is_empty() || seeds.len() > 8 {
        return Err("Provide one to eight seeds".into());
    }
    let definitions = current
        .then(|| BiomeDefinitions::load_directory(terra_core::asset_directory().join("biome")))
        .transpose()?;
    println!(
        "mode,seed,biome_land,biome_highlands,highlands_land_percent,water_samples,land_samples,open_wet_river_samples,river_land_percent,river_area_km2,seconds"
    );
    let mut total = [0_u64; 5];
    for seed in seeds {
        let terrain = TerraGenerator::new(WorldConfig {
            seed: WorldSeed(seed),
            landscape_revision: 1,
            river_terrain_revision: river_revision.unwrap_or(if current {
                WorldConfig::default().river_terrain_revision
            } else {
                3
            }),
            biome_definitions: definitions.clone(),
            rivers: true,
            caves: false,
            trees: false,
            structures_revision: 0,
            ..Default::default()
        })?;
        let started = Instant::now();
        let mut counts = [0_u64; 5];
        // Broad regular grid, offset from chunk/noise lattice points. Ocean is
        // excluded using the same real surface identity presented by the map.
        for z in (-4096..4096).step_by(128) {
            for x in (-4096..4096).step_by(128) {
                let column = terrain.surface_column(f64::from(x) + 53.5, f64::from(z) + 79.5);
                let kind = terrain.surface_biome(&column);
                if kind != BiomeKind::Ocean {
                    counts[0] += 1;
                    counts[1] += u64::from(kind == BiomeKind::Highland);
                }
            }
        }
        // A separate denser, identical window counts real exposed channel water,
        // not regional catchments, dry banks, lakes, oceans or covered passages.
        for z in (-2048_i64..2048).step_by(step as usize) {
            for x in (-2048_i64..2048).step_by(step as usize) {
                let x = x as f64 + step as f64 * 0.5 + 0.5;
                let z = z as f64 + step as f64 * 0.5 + 0.5;
                counts[2] += 1;
                let column = terrain.surface_column(x, z);
                if terrain.surface_biome(&column) != BiomeKind::Ocean {
                    counts[3] += 1;
                    if terrain.river_at(x, z).is_some_and(|river| {
                        !river.covered
                            && river.distance < river.width
                            && column.water_level.is_some_and(|level| {
                                level > terrain.config().sea_level + 0.5
                                    && column.water_bed() < level
                                    && (level - river.level).abs() < 0.01
                            })
                    }) {
                        counts[4] += 1;
                    }
                }
            }
        }
        for (sum, count) in total.iter_mut().zip(counts) {
            *sum += count;
        }
        println!(
            "{},{seed},{},{},{:.6},{},{},{},{:.6},{:.6},{:.3}",
            if current { "current" } else { "legacy" },
            counts[0],
            counts[1],
            counts[1] as f64 / counts[0].max(1) as f64 * 100.0,
            counts[2],
            counts[3],
            counts[4],
            counts[4] as f64 / counts[3].max(1) as f64 * 100.0,
            counts[4] as f64 * (step * step) as f64 / 1_000_000.0,
            started.elapsed().as_secs_f64()
        );
    }
    println!(
        "aggregate,all,{},{},{:.6},{},{},{},{:.6},{:.6},0",
        total[0],
        total[1],
        total[1] as f64 / total[0].max(1) as f64 * 100.0,
        total[2],
        total[3],
        total[4],
        total[4] as f64 / total[3].max(1) as f64 * 100.0,
        total[4] as f64 * (step * step) as f64 / 1_000_000.0
    );
    Ok(())
}
