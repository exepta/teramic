//! Locate actual desert lakes for reproducible generation and rendering checks.
use terra_core::{BiomeDefinitions, WorldConfig};
use terra_world::TerraGenerator;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let terrain = TerraGenerator::new(WorldConfig {
        biome_definitions: Some(BiomeDefinitions::load_directory(
            terra_core::asset_directory().join("biome"),
        )?),
        ..WorldConfig::default()
    })?;
    let mut regions = Vec::new();
    for z in -24..=24 {
        for x in -24..=24 {
            let point = [f64::from(x * 384 + 192), f64::from(z * 384 + 192)];
            let Some(oasis) = terrain.oasis_at(point[0], point[1]) else {
                continue;
            };
            regions.push(oasis);
        }
    }
    regions.sort_by(|a, b| {
        let distance = |p: [f64; 2]| p[0].mul_add(p[0], p[1] * p[1]);
        distance(a.center).total_cmp(&distance(b.center))
    });
    let mut shore_ruins = Vec::new();
    for z in -30..=30 {
        for x in -30..=30 {
            let center = [x, z].map(|v| (v * 384 + 192) as f64);
            if terrain.biome_at(center[0], center[1]).aridity() < 0.55 {
                continue;
            }
            if let Some(site) = terrain.structure_site([x, z])
                && site.kind == terra_world::generation::structures::StructureKind::Ruin
                && let Some(oasis) = regions.iter().min_by_key(|o| {
                    (site.center[0] as f64 - o.center[0]).hypot(site.center[2] as f64 - o.center[1])
                        as i64
                })
            {
                let [x, y, z] = site.center;
                let distance =
                    (x as f64 - oasis.center[0]).hypot(z as f64 - oasis.center[1]) - oasis.radius;
                shore_ruins.push((distance, [x, y, z], *oasis));
            }
        }
    }
    shore_ruins.sort_by(|a, b| a.0.total_cmp(&b.0));
    for (gap, ruin, oasis) in shore_ruins.iter().take(4) {
        println!(
            "ruin={ruin:?} oasis={:?} level={} shore_distance={gap:.0}",
            oasis.center, oasis.level
        );
    }
    if let Some(first) = regions.first() {
        let [x, z] = first.center.map(|v| (v as i64).div_euclid(16) * 16);
        let mut palms = std::collections::BTreeSet::new();
        for dz in -3..=3 {
            for dx in -3..=3 {
                let trees = terrain.tree_appearance([
                    x + dx * 16,
                    (first.level as i64 / 16) * 16,
                    z + dz * 16,
                ]);
                palms.extend(
                    trees
                        .tree_roots()
                        .filter(|(_, t)| *t == terra_world::generation::TreeAppearance::Palm)
                        .map(|(p, _)| p),
                );
            }
        }
        println!("palms around first shore={palms:?}");
    }
    for oasis in regions.iter().take(12) {
        let [cx, cz] = oasis.center;
        let camera = [
            cx + oasis.radius * 0.88 + 7.0,
            cz + oasis.radius * 0.62 + 5.0,
        ];
        let y = terrain
            .surface_height(camera[0], camera[1])
            .max(oasis.level)
            + 11.5;
        println!(
            "center={cx:.0},{:.0},{cz:.0} radius={:.1} camera={:.1},{y:.1},{:.1}",
            oasis.level, oasis.radius, camera[0], camera[1]
        );
    }
    println!("Total oases: {}", regions.len());
    Ok(())
}
