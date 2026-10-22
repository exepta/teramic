//! Print reproducible region metrics without Bevy, rendering or physics.
use terra_core::WorldConfig;
use terra_world::TerraGenerator;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let terrain = TerraGenerator::new(WorldConfig::default())?;
    println!(
        "Seed={} generator={}",
        terrain.config().seed.0,
        terrain.config().generator_version
    );
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
    let mut best = [(0.0_f64, [0; 2]); 6];
    let mut nearest = [(i64::MAX, [0; 2]); 6];
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
