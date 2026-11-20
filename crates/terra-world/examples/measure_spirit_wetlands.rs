//! Compare actual voxel water surfaces, not radius or noise thresholds.
//! cargo run -p terra-world --no-default-features --example measure_spirit_wetlands
use terra_core::{Config, WorldConfig, WorldSeed};
use terra_world::TerraGenerator;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut totals = [0_u64; 2];
    let mut cells = 0_u64;
    let mut heights = [0.0_f64; 2];
    let mut edges = 0_u64;
    let mut core_heights = [0.0_f64; 2];
    let mut core_edges = 0_u64;
    for seed in [42042, 271828, 9317] {
        let config = WorldConfig {
            seed: WorldSeed(seed),
            spirit_blossom_revision: 1,
            ..WorldConfig::from_toml(include_str!("../../../configs/world.toml"))?
        };
        let old = TerraGenerator::new(config.clone())?;
        let new = TerraGenerator::new(WorldConfig {
            spirit_blossom_revision: 2,
            ..config
        })?;
        let mut centers = Vec::new();
        'search: for z in -20..=20 {
            for x in -20..=20 {
                let p = [f64::from(x) * 1024.0, f64::from(z) * 1024.0];
                if old.biome_at(p[0], p[1]).weights.spirit_blossom > 0.85 {
                    centers.push(p);
                    if centers.len() == 6 {
                        break 'search;
                    }
                }
            }
        }
        let mut wet = [0_u64; 2];
        let mut water_view: Option<(f64, [f64; 3], f64)> = None;
        for center in &centers {
            for z in -96..=96 {
                for x in -96..=96 {
                    let px = center[0] + f64::from(x) * 4.0 + 0.5;
                    let pz = center[1] + f64::from(z) * 4.0 + 0.5;
                    let a = old.surface_column(px, pz);
                    if a.biome.weights.spirit_blossom < 0.85 {
                        continue;
                    }
                    let b = new.surface_column(px, pz);
                    if b.water_level.is_some()
                        && let Some(water) = new.water_column(px, pz)
                        && water.depth >= 1.0
                    {
                        let openness = [[-4., 0.], [4., 0.], [0., -4.], [0., 4.]]
                            .into_iter()
                            .filter(|[dx, dz]| {
                                new.water_column(px + dx, pz + dz)
                                    .is_some_and(|v| (v.level - water.level).abs() < 0.1)
                            })
                            .count();
                        let score = openness as f64 + water.depth.min(8.) * 0.1;
                        if water_view.is_none_or(|v| score > v.0) {
                            water_view = Some((score, [px, water.level, pz], water.depth));
                        }
                    }
                    cells += 1;
                    for (i, c) in [a, b].iter().enumerate() {
                        if c.water_level.is_some() {
                            wet[i] += 1;
                        }
                    }
                    if a.water_level.is_none() && b.water_level.is_none() {
                        heights[0] += (old.surface_height(px + 8.0, pz) - a.height).abs();
                        heights[1] += (new.surface_height(px + 8.0, pz) - b.height).abs();
                        edges += 1;
                        if a.height > old.config().sea_level + 12.0
                            && old.surface_height(px + 8.0, pz) > old.config().sea_level + 12.0
                            && old.biome_at(px + 8.0, pz).weights.spirit_blossom >= 0.85
                            && [px, px + 8.0].into_iter().all(|x| {
                                new.river_at(x, pz).is_none() && new.lake_at(x, pz).is_none()
                            })
                        {
                            core_heights[0] += (old.surface_height(px + 8.0, pz) - a.height).abs();
                            core_heights[1] += (new.surface_height(px + 8.0, pz) - b.height).abs();
                            core_edges += 1;
                        }
                    }
                }
            }
        }
        totals[0] += wet[0];
        totals[1] += wet[1];
        if let Some((_, target, depth)) = water_view {
            println!("WATER_VIEW seed={seed} target={target:?} depth={depth:.3}");
        }
        println!(
            "seed={seed} regions={} wet_before={} wet_after={} area_gain={:.2}%",
            centers.len(),
            wet[0],
            wet[1],
            100.0 * (wet[1] as f64 / wet[0].max(1) as f64 - 1.0)
        );
    }
    println!(
        "TOTAL cells={cells} wet_before={} wet_after={} water_area_gain={:.2}% dry_edges={edges} measured_relief_gain={:.2}%",
        totals[0],
        totals[1],
        100.0 * (totals[1] as f64 / totals[0].max(1) as f64 - 1.0),
        100.0 * (heights[1] / heights[0] - 1.0)
    );
    println!(
        "DRY_INTERIOR edges={core_edges} relief_gain={:.3}%",
        100.0 * (core_heights[1] / core_heights[0] - 1.0)
    );
    let water_gain = totals[1] as f64 / totals[0].max(1) as f64 - 1.0;
    let relief_gain = core_heights[1] / core_heights[0] - 1.0;
    if !(0.35..=0.45).contains(&water_gain) || (relief_gain - 0.20).abs() > 0.001 {
        return Err(format!("Spirit landscape calibration regressed: water={water_gain:.4}, dry relief={relief_gain:.4}").into());
    }
    Ok(())
}
