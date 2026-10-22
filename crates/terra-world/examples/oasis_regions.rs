//! Locate actual desert lakes for reproducible generation and rendering checks.
use terra_core::WorldConfig;
use terra_world::TerraGenerator;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let terrain = TerraGenerator::new(WorldConfig::default())?;
    let mut regions = Vec::new();
    for z in (-4096..=4096).step_by(256) {
        for x in (-4096..=4096).step_by(256) {
            if let Some(oasis) = terrain.oasis_at(f64::from(x + 128), f64::from(z + 128)) {
                regions.push(oasis);
            }
        }
    }
    regions.sort_by(|a, b| {
        let distance = |p: [f64; 2]| p[0].mul_add(p[0], p[1] * p[1]);
        distance(a.center).total_cmp(&distance(b.center))
    });
    for oasis in regions.iter().take(12) {
        println!("{oasis:?}");
    }
    println!("Total oases: {}", regions.len());
    Ok(())
}
