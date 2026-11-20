//! Relative generation cost and actual surface-prop coverage, no renderer or edits.
use std::time::Instant;
use terra_core::WorldConfig;
use terra_voxel::SectionPos;
use terra_world::{SectionGenerator, TerraGenerator};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let reference = TerraGenerator::new(WorldConfig::default())?;
    let anchor: [i64; 2] = (-8192..8192)
        .step_by(256)
        .flat_map(|z| (-8192..8192).step_by(256).map(move |x| [x, z]))
        .find(|p| {
            let c = reference.surface_column(p[0] as f64 + 0.5, p[1] as f64 + 0.5);
            c.biome.weights.blood_forest > 0.85 && c.water_level.is_none() && c.slope < 0.6
        })
        .ok_or("no dry Blood Forest measurement anchor")?;
    let origin = anchor.map(|v| v.div_euclid(16));
    for revision in [4, 5] {
        let terrain = TerraGenerator::new(WorldConfig {
            ecology_revision: revision,
            ..WorldConfig::default()
        })?;
        let generator = SectionGenerator::new(terrain);
        let clock = Instant::now();
        let mut prepared = Vec::new();
        for z in -6..6 {
            for x in -6..6 {
                let coordinate = [origin[0] + x, origin[1] + z];
                prepared.push((coordinate, generator.prepare_column(coordinate, false)?));
            }
        }
        let cold_ms = clock.elapsed().as_secs_f64() * 1000.0;
        let clock = Instant::now();
        for (coordinate, _) in &prepared {
            std::hint::black_box(generator.prepare_column(*coordinate, false)?);
        }
        let warm_ms = clock.elapsed().as_secs_f64() * 1000.0;
        let clock = Instant::now();
        let mut small = 0;
        let mut large = 0;
        let mut sections = 0;
        for (coordinate, column) in &prepared {
            let mut low = i64::MAX;
            let mut high = i64::MIN;
            for z in 0..16 {
                for x in 0..16 {
                    let surface = column.local_surface(x, z).ok_or("missing surface")?;
                    let ground = (surface.height - 0.5).floor() as i64;
                    low = low.min((ground + 1).div_euclid(16));
                    high = high.max((ground + 1).div_euclid(16));
                }
            }
            for y in low..=high {
                let generated = generator.build_prepared(
                    SectionPos::new(coordinate[0], y, coordinate[1]),
                    0,
                    &Default::default(),
                    column,
                )?;
                sections += 1;
                small += generated
                    .data
                    .blocks()
                    .iter()
                    .filter(|&&m| matches!(m,412..=414|416..=418))
                    .count();
                large += generated
                    .data
                    .blocks()
                    .iter()
                    .filter(|&&m| matches!(m, 419..=421))
                    .count();
            }
        }
        println!(
            "{}",
            serde_json::json!({"seed":42042,"anchor":anchor,"ecology_revision":revision,"columns":prepared.len(),"area_m2":prepared.len()*256,"cold_prepare_ms":cold_ms,"warm_prepare_ms":warm_ms,"build_ms":clock.elapsed().as_secs_f64()*1000.0,"sections":sections,"small_props":small,"large_props":large,"small_per_1024m2":small as f64/(prepared.len() as f64/4.0)})
        );
    }
    Ok(())
}
