//! Fixed-input worker throughput, without ECS pacing, renderer, GPU or session cache.
use std::{hint::black_box, thread, time::Instant};
use terra_core::WorldConfig;
use terra_voxel::{SectionPos, global_to_section};
use terra_world::{SectionGenerator, TerraGenerator, generation::sections::BlockOverrides};

const WORKERS: usize = 4;
const ITERATIONS: usize = 5;
const RADIUS: i64 = 12;
const VERTICAL_RADIUS: i64 = 3;

#[derive(Default)]
struct Timings {
    prepare_worker_seconds: f64,
    build_worker_seconds: f64,
    sections: usize,
    props: usize,
    quads: usize,
    checksum: u64,
}

fn run(
    generator: &SectionGenerator,
    center: SectionPos,
    with_mesh: bool,
) -> Result<Timings, String> {
    let columns: Vec<_> = (-RADIUS..=RADIUS)
        .flat_map(|z| (-RADIUS..=RADIUS).map(move |x| [center.x + x, center.z + z]))
        .collect();
    thread::scope(|scope| {
        let jobs: Vec<_> = (0..WORKERS)
            .map(|worker| {
                let columns = &columns;
                scope.spawn(move || {
                    let mut result = Timings::default();
                    let edits = BlockOverrides::new();
                    for &column in columns.iter().skip(worker).step_by(WORKERS) {
                        let start = Instant::now();
                        let prepared = generator
                            .prepare_column(column, with_mesh)
                            .map_err(|e| e.to_string())?;
                        result.prepare_worker_seconds += start.elapsed().as_secs_f64();
                        for y in center.y - VERTICAL_RADIUS..=center.y + VERTICAL_RADIUS {
                            let start = Instant::now();
                            let section = generator
                                .build_prepared(
                                    SectionPos::new(column[0], y, column[1]),
                                    0,
                                    &edits,
                                    &prepared,
                                )
                                .map_err(|e| e.to_string())?;
                            result.build_worker_seconds += start.elapsed().as_secs_f64();
                            result.sections += 1;
                            result.props += section.props.len();
                            result.quads +=
                                section.mesh.as_ref().map_or(0, |mesh| mesh.faces.len());
                            result.checksum = result.checksum.wrapping_add(
                                section.data.blocks().iter().map(|&b| u64::from(b)).sum(),
                            );
                            black_box(section);
                        }
                    }
                    Ok::<_, String>(result)
                })
            })
            .collect();
        let mut totals = Timings::default();
        for job in jobs {
            let value = job.join().map_err(|_| "benchmark worker panicked")??;
            totals.prepare_worker_seconds += value.prepare_worker_seconds;
            totals.build_worker_seconds += value.build_worker_seconds;
            totals.sections += value.sections;
            totals.props += value.props;
            totals.quads += value.quads;
            totals.checksum = totals.checksum.wrapping_add(value.checksum);
        }
        Ok(totals)
    })
}

fn main() -> Result<(), String> {
    let mut config = WorldConfig::default();
    if let Ok(value) = std::env::var("TERRA_BENCH_LANDSCAPE") {
        config.landscape_revision = value.parse::<u8>().map_err(|error| error.to_string())?;
    }
    if let Ok(value) = std::env::var("TERRA_BENCH_GEOLOGY") {
        config.geology_revision = value.parse::<u8>().map_err(|error| error.to_string())?;
    }
    if let Ok(value) = std::env::var("TERRA_BENCH_ORES") {
        config.ores.enabled = match value.as_str() {
            "on" => true,
            "off" => false,
            _ => return Err("TERRA_BENCH_ORES must be on or off".into()),
        };
    }
    let terrain = TerraGenerator::new(config).map_err(|e| e.to_string())?;
    let height = terrain
        .surface_height(8.0, 24.0)
        .max(terrain.config().sea_level)
        + 2.0;
    let (mut center, _) = global_to_section([8, height.floor() as i64, 24]);
    if let Ok(value) = std::env::var("TERRA_BENCH_CENTER") {
        let axes = value
            .split(',')
            .map(|axis| {
                axis.trim()
                    .parse::<i64>()
                    .map_err(|error| error.to_string())
            })
            .collect::<Result<Vec<_>, _>>()?;
        let axes: [i64; 3] = axes
            .try_into()
            .map_err(|_| "TERRA_BENCH_CENTER needs three section coordinates")?;
        center = SectionPos::new(axes[0], axes[1], axes[2]);
    }
    println!(
        "ore_generation={} feature_revision={} geology_revision={} landscape_revision={}",
        terrain.config().ores.enabled,
        terrain.config().ores.revision,
        terrain.config().geology_revision,
        terrain.config().landscape_revision
    );
    println!(
        "worker_benchmark seed={} version={} center={center:?} radius={RADIUS}/{VERTICAL_RADIUS} workers={WORKERS} warmup=1 iterations={ITERATIONS}",
        terrain.config().seed.0,
        terrain.config().generator_version
    );
    println!(
        "render_preparation includes padded voxels, greedy meshing, props and water; its difference from headless_generation is NOT isolated meshing time; worker seconds are accumulated elapsed spans, wall seconds measure throughput"
    );
    let generator = SectionGenerator::new(terrain);
    for with_mesh in [false, true] {
        let mode = if with_mesh {
            "render_preparation"
        } else {
            "headless_generation"
        };
        black_box(run(&generator, center, with_mesh)?);
        let mut walls = Vec::with_capacity(ITERATIONS);
        for iteration in 1..=ITERATIONS {
            let start = Instant::now();
            let result = run(&generator, center, with_mesh)?;
            let wall = start.elapsed().as_secs_f64();
            walls.push(wall);
            println!(
                "mode={mode} iteration={iteration} wall_seconds={wall:.6} prepare_worker_seconds={:.6} build_worker_seconds={:.6} sections={} props={} quads={} block_checksum={}",
                result.prepare_worker_seconds,
                result.build_worker_seconds,
                result.sections,
                result.props,
                result.quads,
                result.checksum
            );
        }
        walls.sort_unstable_by(f64::total_cmp);
        println!(
            "mode={mode} mean_wall_seconds={:.6} median_wall_seconds={:.6} min_wall_seconds={:.6} max_wall_seconds={:.6}",
            walls.iter().sum::<f64>() / ITERATIONS as f64,
            walls[ITERATIONS / 2],
            walls[0],
            walls[ITERATIONS - 1]
        );
    }
    Ok(())
}
