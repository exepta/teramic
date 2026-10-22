//! Reproducible CPU generation plus a spatial debug slice. No renderer required.
use std::{io::Write, time::Instant};
use terra_core::WorldConfig;
use terra_voxel::SectionPos;
use terra_world::{SectionGenerator, TerraGenerator, TerrainGenerator};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let revision = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "3".into())
        .parse::<u8>()?;
    let generator = SectionGenerator::new(TerraGenerator::new(WorldConfig {
        cave_structure_revision: revision,
        ..Default::default()
    })?);
    let start = Instant::now();
    let mut checksum = 0_u64;
    let mut sections = 0;
    let mut prepare = 0.0;
    let mut generate = 0.0;
    for z in -8..8 {
        for x in -8..8 {
            let then = Instant::now();
            let column = generator.prepare_column([x, z], false)?;
            prepare += then.elapsed().as_secs_f64();
            let then = Instant::now();
            for y in -7..5 {
                let result = generator.build_prepared(
                    SectionPos::new(x, y, z),
                    0,
                    &Default::default(),
                    &column,
                )?;
                checksum = checksum.wrapping_add(
                    result
                        .data
                        .blocks()
                        .iter()
                        .map(|v| u64::from(*v))
                        .sum::<u64>(),
                );
                checksum = checksum.wrapping_add(result.props.len() as u64);
                sections += 1;
            }
            generate += then.elapsed().as_secs_f64();
        }
    }
    println!(
        "revision={revision} sections={sections} prepare_s={prepare:.6} generate_s={generate:.6} total_s={:.6} checksum={checksum}",
        start.elapsed().as_secs_f64()
    );
    if let Some(path) = std::env::args().nth(2) {
        let mut image = std::io::BufWriter::new(std::fs::File::create(path)?);
        writeln!(image, "P6\n512 512\n255")?;
        let colors = [
            [130.0, 122.0, 116.0],
            [222.0, 173.0, 210.0],
            [74.0, 161.0, 82.0],
            [67.0, 150.0, 221.0],
        ];
        let t = generator.terrain();
        for z in 0..512 {
            for x in 0..512 {
                let p = [
                    f64::from(x - 256) * 4.0 + 0.5,
                    -42.5,
                    f64::from(z - 256) * 4.0 + 0.5,
                ];
                let s = t.cave_biome_at(p);
                let air = t.sample(p).density > 0.0;
                let color = std::array::from_fn::<u8, 3, _>(|i| {
                    let v = (0..4)
                        .map(|b| colors[b][i] * f64::from(s.weights[b]))
                        .sum::<f64>();
                    if s.boundary_distance < 2.0 {
                        235
                    } else {
                        (v * if air { 1.0 } else { 0.22 }) as u8
                    }
                });
                image.write_all(&color)?;
            }
        }
        image.flush()?;
    }
    Ok(())
}
