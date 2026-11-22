//! Reproduce the exact plans used by native generation without changing any world.
use terra_core::{Config, WorldConfig};
use terra_world::{
    TerraGenerator,
    generation::structures::{StructureKind, VillageStyle},
};

fn population(terrain: &TerraGenerator, region: [i64; 2]) -> String {
    terrain
        .village_settlement(region)
        .map_or_else(String::new, |v| {
            format!(
                " size={:?} homes={} residential_beds={} residents={}",
                v.size,
                v.homes.len(),
                v.homes.iter().map(|h| h.beds.len()).sum::<usize>(),
                v.residents.len()
            )
        })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let royal_only = std::env::var_os("TERRA_ROYAL_ONLY").is_some();
    let seeds: &[u64] = if royal_only {
        &[42042]
    } else {
        &[42042, 1337, 99173]
    };
    let base = if let Some(path) = std::env::var_os("TERRA_STRUCTURE_CONFIG") {
        let loaded = WorldConfig::load(path)?;
        println!(
            "config_same_terrain_default={}",
            loaded.same_terrain(&WorldConfig::default())
        );
        if !loaded.same_terrain(&WorldConfig::default()) {
            println!("loaded_config={loaded:?}");
            println!("default_config={:?}", WorldConfig::default());
        }
        loaded
    } else {
        WorldConfig::default()
    };
    for &seed in seeds {
        let mut config = base.clone();
        config.seed.0 = seed;
        let terrain = TerraGenerator::new(config)?;
        if let Ok(point) = std::env::var("TERRA_TERRAIN_SAMPLE") {
            let values: Vec<f64> = point.split(',').map(str::parse).collect::<Result<_, _>>()?;
            let [x, z] = values.as_slice() else {
                return Err("TERRA_TERRAIN_SAMPLE needs x,z".into());
            };
            println!(
                "seed={seed} point={x},{z} column={:?} river={:?}",
                terrain.surface_column(*x, *z),
                terrain.river_at(*x, *z)
            );
            continue;
        }
        if let Ok(region) = std::env::var("TERRA_STRUCTURE_REGION") {
            let values: Vec<i64> = region
                .split(',')
                .map(str::parse)
                .collect::<Result<_, _>>()?;
            let [x, z] = values.as_slice() else {
                return Err("TERRA_STRUCTURE_REGION needs x,z".into());
            };
            println!(
                "seed={seed} region={x},{z} site={:#?}{}",
                terrain.structure_site([*x, *z]),
                population(&terrain, [*x, *z])
            );
            continue;
        }
        let mut found = [royal_only; 9];
        if royal_only {
            found[5] = false;
            found[6] = false;
        }
        let started = std::time::Instant::now();
        'regions: for radius in 0_i64..=if royal_only { 64 } else { 12 } {
            for z in -radius..=radius {
                for x in -radius..=radius {
                    if radius > 0 && x.abs() != radius && z.abs() != radius {
                        continue;
                    }
                    if royal_only && terrain.royal_site_candidate([x, z]).is_none() {
                        continue;
                    }
                    let Some(site) = terrain.structure_site([x, z]) else {
                        continue;
                    };
                    let index = match (site.kind, site.style) {
                        (StructureKind::Village, VillageStyle::Temperate) => 0,
                        (StructureKind::Village, VillageStyle::Desert) => 1,
                        (StructureKind::Village, VillageStyle::Highland) => 2,
                        (StructureKind::Ruin, _) => 3,
                        (StructureKind::Mine, _) => 4,
                        (StructureKind::Kingdom, _) => 5,
                        (StructureKind::Outpost, _) => 6,
                        (StructureKind::Cemetery, _) => 7,
                        (StructureKind::GraveDungeon, _) => 8,
                        (
                            StructureKind::DarkDragonLair
                            | StructureKind::EarthDragonLair
                            | StructureKind::WaterDragonLair,
                            _,
                        ) => continue,
                    };
                    if !found[index] {
                        println!(
                            "seed={seed} region={x},{z} kind={:?} style={:?} center={:?} bounds={:?} buildings={} chests={}{}",
                            site.kind,
                            site.style,
                            site.center,
                            site.bounds,
                            site.buildings.len(),
                            site.containers.len(),
                            population(&terrain, [x, z])
                        );
                        found[index] = true;
                    }
                    if found.iter().all(|v| *v) {
                        break 'regions;
                    }
                }
            }
        }
        println!(
            "seed={seed} found={found:?} discovery_ms={:.2}",
            started.elapsed().as_secs_f64() * 1000.0
        );
    }
    Ok(())
}
