//! Locate reproducible wooded shore views for native water/reflection inspection.
use terra_core::WorldConfig;
use terra_voxel::{MaterialId, SectionPos};
use terra_world::{SectionGenerator, TerraGenerator};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let terrain = TerraGenerator::new(WorldConfig::default())?;
    let level = terrain.config().sea_level;
    let mut candidates = Vec::new();
    for z in (-4096..=4096).step_by(32) {
        for x in (-4096..=4096).step_by(32) {
            let land = [f64::from(x), f64::from(z)];
            let height = terrain.surface_height(land[0], land[1]);
            if !(level + 2.0..level + 10.0).contains(&height)
                || terrain.forest_probability(land[0], land[1]) < 0.24
            {
                continue;
            }
            for [dx, dz] in [
                [32.0, 0.0],
                [-32.0, 0.0],
                [0.0, 32.0],
                [0.0, -32.0],
                [24.0, 24.0],
                [-24.0, 24.0],
                [24.0, -24.0],
                [-24.0, -24.0],
            ] {
                let water = [land[0] + dx, land[1] + dz];
                let bed = terrain.surface_height(water[0], water[1]);
                if (level - 5.0..level - 0.6).contains(&bed) {
                    let distance = i64::from(x).pow(2) + i64::from(z).pow(2);
                    candidates.push((distance, land, water, height, bed));
                    break;
                }
            }
        }
    }
    candidates.sort_unstable_by_key(|candidate| candidate.0);
    let generator = SectionGenerator::new(terrain);
    let mut found = 0;
    for (_, land, water, height, bed) in candidates.into_iter().take(100) {
        let x = (land[0] as i64).div_euclid(16);
        let z = (land[1] as i64).div_euclid(16);
        let mut wood = 0;
        for dz in -1..=1 {
            for dx in -1..=1 {
                for y in 0..=2 {
                    let section = generator.generate(SectionPos::new(x + dx, y, z + dz))?;
                    wood += section
                        .blocks()
                        .iter()
                        .filter(|&&material| {
                            material == MaterialId::Wood as u16
                                || material == MaterialId::WeatheredWood as u16
                        })
                        .count();
                }
            }
        }
        if wood < 12 {
            continue;
        }
        println!(
            "land={land:?} ground={height:.3} water={water:?} bed={bed:.3} wood_blocks={wood} eye=[{:.1}, {:.1}, {:.1}] target=[{:.1}, {:.1}, {:.1}]",
            water[0],
            level + 5.0,
            water[1],
            land[0],
            level + 3.0,
            land[1]
        );
        found += 1;
        if found == 8 {
            break;
        }
    }
    Ok(())
}
