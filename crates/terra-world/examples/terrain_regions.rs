//! Print reproducible region metrics without Bevy, rendering or physics.
use std::{
    hint::black_box,
    time::{Duration, Instant},
};
use terra_core::{WorldConfig, WorldSeed};
use terra_world::TerraGenerator;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut config = WorldConfig::default();
    let mut scan_landforms = false;
    let mut scan_relief = false;
    let mut benchmark = false;
    for arg in std::env::args().skip(1) {
        if let Some(value) = arg.strip_prefix("--seed=") {
            config.seed = WorldSeed(value.parse()?);
        } else if let Some(value) = arg.strip_prefix("--revision=") {
            config.landscape_revision = value.parse()?;
        } else if arg == "--landforms" {
            scan_landforms = true;
        } else if arg == "--relief-scan" {
            scan_relief = true;
        } else if arg == "--benchmark" {
            benchmark = true;
        } else {
            return Err(format!("Unknown option {arg}").into());
        }
    }
    let terrain = TerraGenerator::new(config)?;
    if usize::from(scan_landforms) + usize::from(scan_relief) + usize::from(benchmark) > 1 {
        return Err("Choose --benchmark, --landforms or --relief-scan".into());
    }
    println!(
        "Seed={} generator={}",
        terrain.config().seed.0,
        terrain.config().generator_version
    );
    if benchmark {
        return benchmark_landscape(terrain.config());
    }
    if scan_relief {
        return relief_scan(terrain.config());
    }
    if scan_landforms {
        return landform_anchors(&terrain);
    }
    for [x, z] in [[0, 0], [160, -96], [512, -512], [-256, 256], [1024, 768]] {
        let mut minimum = f64::INFINITY;
        let mut maximum = f64::NEG_INFINITY;
        let mut cliffs = 0;
        let mut forest = 0.0;
        let mut samples = 0;
        for dz in (-64..=64).step_by(4) {
            for dx in (-64..=64).step_by(4) {
                let px = f64::from(x + dx) + 0.5;
                let pz = f64::from(z + dz) + 0.5;
                let column = terrain.surface_column(px, pz);
                minimum = minimum.min(column.height);
                maximum = maximum.max(column.height);
                cliffs += usize::from(column.slope > 1.05);
                forest += terrain.forest_probability(px, pz);
                samples += 1;
            }
        }
        let w = terrain.biome_at(f64::from(x), f64::from(z)).weights;
        println!(
            "X={x:5} Z={z:5} center={:.9} height={minimum:6.2}..{maximum:6.2} cliffs={cliffs:4}/{samples} forest={:.2} plains={:.2} forest_weight={:.2} highland={:.2}",
            terrain.surface_height(f64::from(x), f64::from(z)),
            forest / f64::from(samples),
            w.plains,
            w.forest,
            w.highland
        );
    }
    let mut best = [(0.0_f64, [0; 2]); 8];
    let mut nearest = [(i64::MAX, [0; 2]); 8];
    let mut coast = (i64::MAX, [0; 2]);
    for z in (-8192..=8192).step_by(128) {
        for x in (-8192..=8192).step_by(128) {
            for (i, weight) in terrain
                .biome_at(f64::from(x), f64::from(z))
                .weights
                .values()
                .into_iter()
                .enumerate()
            {
                if weight > best[i].0 {
                    best[i] = (weight, [x, z]);
                }
                let distance = i64::from(x).pow(2) + i64::from(z).pow(2);
                if weight > 0.8 && distance < nearest[i].0 {
                    nearest[i] = (distance, [x, z]);
                }
                if i == 0 && (0.3..0.7).contains(&weight) && distance < coast.0 {
                    let h = terrain.surface_height(f64::from(x), f64::from(z));
                    if (4.0..12.0).contains(&h) {
                        coast = (distance, [x, z]);
                    }
                }
            }
        }
    }
    println!("Closest strong biomes: {nearest:?}; Coast: {coast:?}");
    for (name, (weight, [x, z])) in [
        "Ocean",
        "Plains",
        "Forest",
        "Desert",
        "Highland",
        "Mushroom Park",
        "Cliffted Desert",
        "Cherry Forest",
    ]
    .into_iter()
    .zip(best)
    {
        println!(
            "{name}: X={x}, Z={z}, weight={weight:.4}, height={:.9}",
            terrain.surface_height(f64::from(x), f64::from(z))
        );
    }
    Ok(())
}

struct Measurement {
    total: Duration,
    samples: usize,
    checksum: f64,
}

fn measure(
    revision: u8,
    name: &str,
    coordinates: &[[f64; 2]],
    repeats: usize,
    mut sample: impl FnMut(f64, f64) -> f64,
) -> Measurement {
    const ROUNDS: usize = 7;
    let mut durations = Vec::with_capacity(ROUNDS);
    let mut checksum = 0.0;
    for _ in 0..ROUNDS {
        let started = Instant::now();
        let mut round_checksum = 0.0;
        for _ in 0..repeats {
            for &[x, z] in coordinates {
                round_checksum += black_box(sample(black_box(x), black_box(z)));
            }
        }
        durations.push(started.elapsed());
        checksum += black_box(round_checksum);
    }
    let total: Duration = durations.iter().sum();
    durations.sort_unstable();
    let samples = coordinates.len() * repeats * ROUNDS;
    println!(
        "benchmark revision={revision} operation={name} rounds={ROUNDS} coordinates={} repeats={repeats} samples={samples} total_ms={:.3} median_round_ms={:.3} max_round_ms={:.3} samples_per_second={:.0} checksum={checksum:.12}",
        coordinates.len(),
        total.as_secs_f64() * 1000.0,
        durations[ROUNDS / 2].as_secs_f64() * 1000.0,
        durations[ROUNDS - 1].as_secs_f64() * 1000.0,
        samples as f64 / total.as_secs_f64()
    );
    Measurement {
        total,
        samples,
        checksum,
    }
}

fn benchmark_landscape(config: &WorldConfig) -> Result<(), Box<dyn std::error::Error>> {
    let coordinates = (-32..32)
        .flat_map(|z| {
            (-32..32).map(move |x| [f64::from(x) * 117.0 + 0.5, f64::from(z) * 131.0 + 0.5])
        })
        .collect::<Vec<_>>();
    let columns = (0..64).map(|i| coordinates[i * 61]).collect::<Vec<_>>();
    println!(
        "Warm-cache CPU comparison; identical 4096 mixed-sign coordinates, 64 complete columns, seed={}. No rendering/meshing included.",
        config.seed.0
    );
    let mut results = Vec::new();
    for revision in [1, 2, 3, 4] {
        let terrain = TerraGenerator::new(WorldConfig {
            landscape_revision: revision,
            ..config.clone()
        })?;
        for &[x, z] in &coordinates {
            black_box(terrain.surface_height(x, z));
        }
        for &[x, z] in &columns {
            black_box(terrain.surface_column(x, z));
        }
        let heights = measure(revision, "surface_height", &coordinates, 16, |x, z| {
            black_box(&terrain).surface_height(x, z)
        });
        let columns = measure(revision, "surface_column", &columns, 4, |x, z| {
            let column = black_box(black_box(&terrain).surface_column(x, z));
            column.height
                + column.slope * 0.125
                + column.soil_depth * 0.0625
                + f64::from(column.surface_material as u16) * 0.0001
        });
        results.push((heights, columns));
    }
    for (index, pair) in results.windows(2).enumerate() {
        for (name, old, new) in [
            ("surface_height", &pair[0].0, &pair[1].0),
            ("surface_column", &pair[0].1, &pair[1].1),
        ] {
            assert_eq!(old.samples, new.samples);
            let ratio = new.total.as_secs_f64() / old.total.as_secs_f64();
            println!(
                "comparison operation={name} revision{}_over_revision{}={ratio:.4} time_change_percent={:.2} checksum_old={:.12} checksum_new={:.12}",
                index + 2,
                index + 1,
                (ratio - 1.0) * 100.0,
                old.checksum,
                new.checksum
            );
        }
    }
    Ok(())
}

fn relief_scan(config: &WorldConfig) -> Result<(), Box<dyn std::error::Error>> {
    let uniform = WorldConfig {
        water: false,
        rivers: false,
        biomes: false,
        ..config.clone()
    };
    let terrain = TerraGenerator::new(uniform.clone())?;
    let mut anchors: [Vec<[f64; 2]>; 5] = std::array::from_fn(|_| Vec::new());
    for z in (-6144..=6144).step_by(128) {
        for x in (-6144..=6144).step_by(128) {
            let center = [f64::from(x) + 0.5, f64::from(z) + 0.5];
            for (kind, positions) in anchors.iter_mut().enumerate() {
                if positions.len() >= 12 {
                    continue;
                }
                if [[-32.0, -32.0], [-32.0, 32.0], [32.0, -32.0], [32.0, 32.0]]
                    .into_iter()
                    .chain([[0.0, 0.0]])
                    .all(|[dx, dz]| {
                        let region = terrain.landscape_region(center[0] + dx, center[1] + dz);
                        region.weights[kind] > 0.999 && region.landmark < 0.001
                    })
                {
                    positions.push(center);
                }
            }
        }
    }
    println!(
        "Relief scan: seed={}, identical 64m patches in a uniform forest biome; water/rivers disabled to isolate relief. Flat footprint: center and its four immediate neighbours occupy the same voxel height.",
        config.seed.0
    );
    for revision in [2, 3] {
        let terrain = TerraGenerator::new(WorldConfig {
            landscape_revision: revision,
            ..uniform.clone()
        })?;
        for (name, positions) in ["plains", "hills", "plateau", "ridges", "basin"]
            .into_iter()
            .zip(&anchors)
        {
            if positions.len() < 12 {
                return Err(
                    format!("Not enough {name} relief patches: {}", positions.len()).into(),
                );
            }
            let mut ranges = 0.0;
            let mut slope_sum = 0.0;
            let mut max_slope = 0.0_f64;
            let mut flat = 0;
            let mut samples = 0;
            for &[x, z] in positions {
                let mut range = [f64::INFINITY, f64::NEG_INFINITY];
                for dz in (-32..=32).step_by(4) {
                    for dx in (-32..=32).step_by(4) {
                        let [x, z] = [x + f64::from(dx), z + f64::from(dz)];
                        let h = terrain.surface_height(x, z);
                        range[0] = range[0].min(h);
                        range[1] = range[1].max(h);
                        let neighbours = [
                            terrain.surface_height(x - 1.0, z),
                            terrain.surface_height(x + 1.0, z),
                            terrain.surface_height(x, z - 1.0),
                            terrain.surface_height(x, z + 1.0),
                        ];
                        flat += usize::from(neighbours.into_iter().all(|n| n.floor() == h.floor()));
                        let slope = ((neighbours[1] - neighbours[0]).powi(2)
                            + (neighbours[3] - neighbours[2]).powi(2))
                        .sqrt()
                            * 0.5;
                        slope_sum += slope;
                        max_slope = max_slope.max(slope);
                        samples += 1;
                    }
                }
                ranges += range[1] - range[0];
            }
            println!(
                "relief seed={} revision={revision} landform={name} patches={} mean_range_64m={:.4} flat_footprint_percent={:.2} mean_slope={:.4} max_slope={max_slope:.4} first_anchor={:?}",
                config.seed.0,
                positions.len(),
                ranges / positions.len() as f64,
                flat as f64 / samples as f64 * 100.0,
                slope_sum / samples as f64,
                positions[0]
            );
        }
    }
    Ok(())
}

fn landform_anchors(terrain: &TerraGenerator) -> Result<(), Box<dyn std::error::Error>> {
    let mut nearest = [(f64::INFINITY, [0.0; 2]); 6];
    for z in (-6144..=6144).step_by(64) {
        for x in (-6144..=6144).step_by(64) {
            let [x, z] = [f64::from(x) + 0.5, f64::from(z) + 0.5];
            let region = terrain.landscape_region(x, z);
            let distance = x * x + z * z;
            let interesting = region
                .weights
                .into_iter()
                .chain([region.landmark])
                .enumerate()
                .filter(|(i, weight)| *weight > if *i == 5 { 0.35 } else { 0.95 })
                .filter(|(i, _)| distance < nearest[*i].0)
                .map(|(i, _)| i)
                .collect::<Vec<_>>();
            if interesting.is_empty() || terrain.biome_at(x, z).weights.ocean > 0.10 {
                continue;
            }
            if terrain.surface_height(x, z) < terrain.config().sea_level + 2.0 {
                continue;
            }
            for i in interesting {
                nearest[i] = (distance, [x, z]);
            }
        }
    }
    for (name, (distance, [x, z])) in ["plains", "hills", "plateau", "ridges", "basin", "landmark"]
        .into_iter()
        .zip(nearest)
    {
        if !distance.is_finite() {
            return Err(format!(
                "No strong {name} anchor found for seed {}",
                terrain.config().seed.0
            )
            .into());
        }
        let region = terrain.landscape_region(x, z);
        let height = terrain.surface_height(x, z);
        let mut range = [height; 2];
        for dz in (-96..=96).step_by(16) {
            for dx in (-96..=96).step_by(16) {
                let h = terrain.surface_height(x + f64::from(dx), z + f64::from(dz));
                range[0] = range[0].min(h);
                range[1] = range[1].max(h);
            }
        }
        println!(
            "landscape-{name} seed={} revision={} x={x} z={z} y={height:.6} relief={:.2}..{:.2} weights={:?} landmark={:.4}",
            terrain.config().seed.0,
            terrain.config().landscape_revision,
            range[0],
            range[1],
            region.weights,
            region.landmark
        );
    }
    Ok(())
}
