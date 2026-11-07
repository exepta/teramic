//! Bounded revision-five profile diagnostics; the default scan does not build rivers.
//! Optional world snapshots retain the exact scan settings, including disabled caves and trees.
use std::{
    fmt::Write as _,
    fs,
    hint::black_box,
    io::Write as _,
    path::PathBuf,
    time::{Duration, Instant},
};
use terra_core::{TerrainKind, WorldConfig, WorldSeed};
use terra_world::{TerraGenerator, generation::TerrainFields};

const MAX_SAMPLES: usize = 1024;
const MAP_STEP: f64 = 16.0;

struct Options {
    seeds: Vec<u64>,
    radius: f64,
    step: f64,
    output: Option<PathBuf>,
    maps: bool,
    benchmark: bool,
    world_snapshots: bool,
    map_side: usize,
    center: [f64; 2],
}

fn options() -> Result<Options, String> {
    let mut options = Options {
        seeds: Vec::new(),
        radius: 8192.0,
        step: 512.0,
        output: None,
        maps: false,
        benchmark: false,
        world_snapshots: false,
        map_side: 32,
        center: [0.0; 2],
    };
    let mut args = std::env::args().skip(1);
    while let Some(argument) = args.next() {
        if argument == "--help" || argument == "-h" {
            println!(
                "terrain_profiles [--seed 42042,7,99173] [--radius 8192] [--step 512] [--output DIR] [--maps] [--map-center X,Z] [--map-side 32|64] [--world-snapshots] [--benchmark]\nDefault: at most 32x32 base-height samples, no hydrology. Radius and step are metres.\n--maps additionally samples a local 32x32 grid at 16m spacing using actual water/river diagnostics; 64x64 is explicitly opt-in.\n--world-snapshots writes the exact scanned WorldConfig, including all profiles and disabled caves/trees, to seed-SEED-world.toml; requires --output DIR and cannot be combined with --benchmark.\n--benchmark compares revisions4/5 CPU-only: 4096 warmup queries, 7x16x4096 warm queries, then 128 cold tile queries on a fresh generator; no FPS measurement."
            );
            std::process::exit(0);
        }
        if argument == "--maps" {
            options.maps = true;
            continue;
        }
        if argument == "--benchmark" {
            options.benchmark = true;
            continue;
        }
        if argument == "--world-snapshots" {
            options.world_snapshots = true;
            continue;
        }
        let (key, value) = if let Some((key, value)) = argument.split_once('=') {
            (key.to_owned(), value.to_owned())
        } else {
            let value = args
                .next()
                .ok_or_else(|| format!("Missing value for {argument}"))?;
            (argument, value)
        };
        match key.as_str() {
            "--seed" => {
                for seed in value.split(',') {
                    options
                        .seeds
                        .push(seed.parse().map_err(|_| format!("Invalid seed {seed}"))?);
                }
            }
            "--radius" => options.radius = value.parse().map_err(|_| "Invalid radius")?,
            "--step" => options.step = value.parse().map_err(|_| "Invalid step")?,
            "--output" => options.output = Some(PathBuf::from(value)),
            "--map-side" => options.map_side = value.parse().map_err(|_| "Invalid map side")?,
            "--map-center" => {
                let (x, z) = value.split_once(',').ok_or("Map center must be X,Z")?;
                options.center = [
                    x.parse().map_err(|_| "Invalid map X")?,
                    z.parse().map_err(|_| "Invalid map Z")?,
                ];
            }
            _ => return Err(format!("Unknown option {key}")),
        }
    }
    if !options.radius.is_finite()
        || options.radius <= 0.0
        || options.radius > 100_000.0
        || !options.step.is_finite()
        || options.step <= 0.0
        || options
            .center
            .iter()
            .any(|v| !v.is_finite() || v.abs() > 100_000.0)
    {
        return Err("Radius/step must be finite and positive; radius and map coordinates are limited to 100000m".into());
    }
    if ![32, 64].contains(&options.map_side) {
        return Err("--map-side must be 32 or 64".into());
    }
    let side = (options.radius * 2.0 / options.step).ceil().max(1.0);
    if side > (MAX_SAMPLES as f64).sqrt() {
        return Err(format!(
            "Scan exceeds {MAX_SAMPLES} samples; increase --step or decrease --radius"
        ));
    }
    if options.seeds.is_empty() {
        options.seeds.push(WorldConfig::default().seed.0);
    }
    if options.seeds.len() > 16 {
        return Err("At most 16 seeds per invocation".into());
    }
    if options.world_snapshots && options.benchmark {
        return Err("Choose either --world-snapshots or --benchmark".into());
    }
    if options.world_snapshots && options.output.is_none() {
        return Err("--world-snapshots requires --output DIR".into());
    }
    if options.maps && options.output.is_none() {
        return Err("--maps requires --output DIR".into());
    }
    if options.maps && options.benchmark {
        return Err("Choose either --maps or --benchmark".into());
    }
    Ok(options)
}

#[derive(Clone, Copy)]
struct Anchor {
    position: [f64; 2],
    height: f64,
    weight: f64,
    dominant: TerrainKind,
    fields: TerrainFields,
}

fn scan(terrain: &TerraGenerator, options: &Options) -> Result<String, std::fmt::Error> {
    let side = (options.radius * 2.0 / options.step).ceil().max(1.0) as usize;
    let spacing = options.radius * 2.0 / side as f64;
    let mut counts = [0usize; 10];
    let mut nearest: [Option<Anchor>; 10] = [None; 10];
    let mut best: [Option<Anchor>; 10] = [None; 10];
    let mut range = [f64::INFINITY, f64::NEG_INFINITY];
    for z in 0..side {
        for x in 0..side {
            let x = -options.radius + (x as f64 + 0.5) * spacing + 0.5;
            let z = -options.radius + (z as f64 + 0.5) * spacing + 0.5;
            let height = terrain.terrain_base_height(x, z);
            let fields = terrain.terrain_fields(x, z);
            let dominant = terrain.terrain_profile(x, z);
            let weights = terrain.terrain_profile_weights(x, z);
            assert!(
                height.is_finite() && weights.iter().all(|v| v.is_finite()),
                "nonfinite sample at {x},{z}"
            );
            range[0] = range[0].min(height);
            range[1] = range[1].max(height);
            counts[dominant as usize] += 1;
            for (index, weight) in weights.into_iter().enumerate() {
                let anchor = Anchor {
                    position: [x, z],
                    height,
                    weight,
                    dominant,
                    fields,
                };
                if best[index].is_none_or(|old| weight > old.weight) {
                    best[index] = Some(anchor);
                }
                if dominant as usize == index
                    && nearest[index]
                        .is_none_or(|old| x.hypot(z) < old.position[0].hypot(old.position[1]))
                {
                    nearest[index] = Some(anchor);
                }
            }
        }
    }
    let mut report = format!(
        "seed={} landscape_revision={} scan_side={side} samples={} radius_m={} spacing_m={spacing:.3}\nbase_height_range_m={:.6}..{:.6}\nBASE PROFILE SCAN: no rivers, lakes, cave density or final surface-height queries.\nAnchor heights below are base-envelope heights, before hydrology carving.\n",
        terrain.config().seed.0,
        terrain.config().landscape_revision,
        side * side,
        options.radius,
        range[0],
        range[1]
    );
    for kind in TerrainKind::ALL {
        let index = kind as usize;
        writeln!(
            report,
            "profile={kind:?} label=\"{}\" dominant_samples={} fraction={:.6}",
            kind.label(),
            counts[index],
            counts[index] as f64 / (side * side) as f64
        )?;
        for (label, anchor) in [
            ("nearest_dominant", nearest[index]),
            ("strongest_weight", best[index]),
        ] {
            if let Some(anchor) = anchor.filter(|anchor| anchor.weight > 0.0) {
                let f = anchor.fields;
                writeln!(
                    report,
                    "  {label}: x={:.6} z={:.6} base_height={:.9} weight={:.9} actual_dominant={:?} continentalness={:.6} erosion={:.6} temperature={:.6} humidity={:.6} weirdness={:.6} peaks_valleys={:.6} ridge_strength={:.6}",
                    anchor.position[0],
                    anchor.position[1],
                    anchor.height,
                    anchor.weight,
                    anchor.dominant,
                    f.continentalness,
                    f.erosion,
                    f.temperature,
                    f.humidity,
                    f.weirdness,
                    f.peaks_valleys,
                    f.ridge_strength
                )?;
            } else {
                writeln!(
                    report,
                    "  {label}: not sampled (not proof of global absence)"
                )?;
            }
        }
    }
    Ok(report)
}

fn byte(value: f64) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}
fn gray(value: f64) -> [u8; 3] {
    [byte(value); 3]
}

fn maps(terrain: &TerraGenerator, options: &Options) -> Result<(), Box<dyn std::error::Error>> {
    println!(
        "map_contract: height=final surface; density sampled at base_height-0.5m, negative=solid/dark positive=air/light; river white=strong; water black=none blue=present; biome palette uses stable IDs0..7"
    );
    const BIOME_COLORS: [[u8; 3]; 8] = [
        [45, 109, 184],
        [116, 176, 76],
        [36, 110, 62],
        [223, 180, 99],
        [225, 238, 246],
        [155, 122, 182],
        [187, 113, 75],
        [232, 143, 176],
    ];
    const NAMES: [&str; 10] = [
        "height",
        "continentalness",
        "erosion",
        "temperature",
        "humidity",
        "peaks_valleys",
        "final_density",
        "biome_id",
        "river_network",
        "water_level",
    ];
    let side = options.map_side;
    let mut layers: [Vec<u8>; 10] = std::array::from_fn(|_| Vec::with_capacity(side * side * 3));
    for z in 0..side {
        for x in 0..side {
            let x = options.center[0] + (x as f64 + 0.5 - side as f64 * 0.5) * MAP_STEP + 0.5;
            let z = options.center[1] + (z as f64 + 0.5 - side as f64 * 0.5) * MAP_STEP + 0.5;
            let y = terrain.terrain_base_height(x, z) - 0.5;
            let sample = terrain.terrain_debug_sample([x, y, z]);
            let f = sample.fields;
            let height = if sample.height < terrain.config().sea_level {
                [
                    20,
                    byte(0.15 + (sample.height + 120.0) / 350.0),
                    byte(0.40 + (sample.height + 120.0) / 350.0),
                ]
            } else {
                gray((sample.height + 120.0) / 470.0)
            };
            let pixels = [
                height,
                gray(f.continentalness * 0.5 + 0.5),
                gray(f.erosion),
                [
                    byte(f.temperature),
                    byte(1.0 - (f.temperature - 0.5).abs() * 2.0),
                    byte(1.0 - f.temperature),
                ],
                gray(f.humidity),
                gray(f.peaks_valleys * 0.5 + 0.5),
                gray(0.5 + sample.density / 6.0),
                BIOME_COLORS[sample.biome as usize],
                gray(sample.river_strength),
                sample
                    .water_level
                    .map_or([0; 3], |level| [0, byte((level + 120.0) / 470.0), 255]),
            ];
            for (layer, pixel) in layers.iter_mut().zip(pixels) {
                layer.extend_from_slice(&pixel);
            }
        }
    }
    let directory = options.output.as_ref().ok_or("Missing output directory")?;
    for (name, bytes) in NAMES.into_iter().zip(layers) {
        let path = directory.join(format!("seed-{}-{name}.ppm", terrain.config().seed.0));
        let mut file = fs::File::create(&path)?;
        write!(file, "P6\n{side} {side}\n255\n")?;
        file.write_all(&bytes)?;
        println!(
            "map={} center_x={} center_z={} step_m={MAP_STEP}",
            path.display(),
            options.center[0],
            options.center[1]
        );
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let options = options()?;
    if let Some(output) = &options.output {
        fs::create_dir_all(output)?;
    }
    for &seed in &options.seeds {
        if options.benchmark {
            let report = benchmark(seed)?;
            print!("{report}");
            if let Some(output) = &options.output {
                fs::write(output.join(format!("seed-{seed}-benchmark.txt")), report)?;
            }
            continue;
        }
        let terrain = TerraGenerator::new(WorldConfig {
            seed: WorldSeed(seed),
            landscape_revision: 5,
            caves: false,
            trees: false,
            water: true,
            rivers: true,
            structures_revision: 0,
            ..Default::default()
        })?;
        let report = scan(&terrain, &options)?;
        print!("{report}");
        if let Some(output) = &options.output {
            fs::write(output.join(format!("seed-{seed}-profiles.txt")), report)?;
            if options.world_snapshots {
                let path = output.join(format!("seed-{seed}-world.toml"));
                terra_core::save_toml(terrain.config(), &path)?;
                println!("world_snapshot={}", path.display());
            }
        }
        if options.maps {
            maps(&terrain, &options)?;
        }
    }
    Ok(())
}

struct Timing {
    total: Duration,
    median_round: Duration,
    maximum_query: Duration,
    checksum: f64,
    queries: usize,
}

fn timed_queries(
    terrain: &TerraGenerator,
    points: &[[f64; 2]],
    rounds: usize,
    repeats: usize,
) -> Result<Timing, Box<dyn std::error::Error>> {
    let mut elapsed = Vec::with_capacity(rounds);
    let mut maximum_query = Duration::ZERO;
    let mut checksum = 0.0;
    for _ in 0..rounds {
        let round = Instant::now();
        for _ in 0..repeats {
            for &[x, z] in points {
                let query = Instant::now();
                let height = black_box(terrain.surface_height(black_box(x), black_box(z)));
                maximum_query = maximum_query.max(query.elapsed());
                if !height.is_finite() {
                    return Err(format!(
                        "Nonfinite benchmark height at seed{} revision{} {x},{z}",
                        terrain.config().seed.0,
                        terrain.config().landscape_revision
                    )
                    .into());
                }
                checksum += height;
            }
        }
        elapsed.push(round.elapsed());
    }
    if !checksum.is_finite() {
        return Err("Nonfinite benchmark checksum".into());
    }
    let total = elapsed.iter().sum();
    elapsed.sort_unstable();
    Ok(Timing {
        total,
        median_round: elapsed[rounds / 2],
        maximum_query,
        checksum,
        queries: points.len() * rounds * repeats,
    })
}

fn benchmark(seed: u64) -> Result<String, Box<dyn std::error::Error>> {
    const LOCAL_SIDE: usize = 64;
    const ROUNDS: usize = 7;
    const REPEATS: usize = 16;
    let local: Vec<_> = (0..LOCAL_SIDE)
        .flat_map(|z| (0..LOCAL_SIDE).map(move |x| [x as f64 * 2.0 + 0.5, z as f64 * 2.0 + 0.5]))
        .collect();
    let cold: Vec<_> = (0..128)
        .map(|i| [f64::from(i) * 128.0 + 0.5, 0.5])
        .collect();
    let mut report = format!(
        "CPU SURFACE SAMPLING BENCHMARK seed={seed} arch={} os={} debug_assertions={}\nmode=headless,no_renderer,no_gpu,no_fps; water=false,rivers=false,caves=false,trees=false,structures_revision=0\noperation=surface_height; all revisions use identical coordinates and config except landscape_revision\nwarmup_queries=4096 (excluded from timing); warm_local=64x64@2m in one128m tile extent; rounds=7 repeats=16 queries_per_revision=458752\ncold=128 distinct128m-spaced positions, fresh generator, one pass only; no repeated global cache thrashing\nTimes are wall clock including loop/timer overhead and possible OS scheduling, not pure CPU cycles or GPU timings.\n",
        std::env::consts::ARCH,
        std::env::consts::OS,
        cfg!(debug_assertions)
    );
    writeln!(
        report,
        "Heights differ by design between revisions4/5; finite checksums are integrity markers, not equality targets."
    )?;
    for revision in [4, 5] {
        let config = WorldConfig {
            seed: WorldSeed(seed),
            landscape_revision: revision,
            water: false,
            rivers: false,
            caves: false,
            trees: false,
            structures_revision: 0,
            ..Default::default()
        };
        let terrain = TerraGenerator::new(config.clone())?;
        for &[x, z] in &local {
            black_box(terrain.surface_height(black_box(x), black_box(z)));
        }
        let warm = timed_queries(&terrain, &local, ROUNDS, REPEATS)?;
        // Cold and warm measurements never share a generator or its 128-tile cache.
        let cold_terrain = TerraGenerator::new(config)?;
        let cold = timed_queries(&cold_terrain, &cold, 1, 1)?;
        for (operation, measurement) in [("warm_local", warm), ("cold_tiles", cold)] {
            writeln!(
                report,
                "revision={revision} operation={operation} queries={} total_ms={:.6} median_round_ms={:.6} max_query_wall_us={:.6} checksum={:.12}",
                measurement.queries,
                measurement.total.as_secs_f64() * 1000.0,
                measurement.median_round.as_secs_f64() * 1000.0,
                measurement.maximum_query.as_secs_f64() * 1_000_000.0,
                measurement.checksum
            )?;
        }
    }
    Ok(report)
}
