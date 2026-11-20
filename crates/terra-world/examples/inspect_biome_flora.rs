//! Find genuinely generated ecology-six flora/cacti for native camera captures.
//! No save is opened or edited. Optional argument: desert/plains/forest/cherry/spirit/highland.
use terra_core::{Config, WorldConfig};
use terra_world::TerraGenerator;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let name = std::env::args().nth(1).unwrap_or_else(|| "desert".into());
    let mut config = if std::env::var_os("TERRA_FLORA_USE_CONFIG").is_some() {
        WorldConfig::from_toml(include_str!("../../../configs/world.toml"))?
    } else {
        WorldConfig::default()
    };
    config.ecology_revision = 6;
    let terrain = TerraGenerator::new(config)?;
    let mut inspected = 0;
    for radius in 0_i64..=48 {
        for z in -radius..=radius {
            for x in -radius..=radius {
                if x.abs().max(z.abs()) != radius {
                    continue;
                }
                let point = [x * 256, z * 256];
                let weights = terrain
                    .biome_at(point[0] as f64 + 0.5, point[1] as f64 + 0.5)
                    .weights;
                let weight = match name.as_str() {
                    "desert" => weights.desert + weights.cliffted_desert,
                    "plains" => weights.plains,
                    "forest" => weights.forest,
                    "cherry" => weights.cherry_forest,
                    "spirit" => weights.spirit_blossom,
                    "highland" => weights.highland,
                    _ => return Err("choose desert/plains/forest/cherry/spirit/highland".into()),
                };
                if weight < 0.65 {
                    continue;
                }
                for dz in -1..=1 {
                    for dx in -1..=1 {
                        let tile = [point[0].div_euclid(32) + dx, point[1].div_euclid(32) + dz];
                        inspected += 1;
                        let cacti = terrain.surface_cacti(tile);
                        let flora = terrain.surface_flora(tile);
                        let target = if name == "desert" {
                            cacti.first().map(|p| p.origin)
                        } else {
                            flora.first().map(|p| p.0)
                        };
                        if let Some(p) = target {
                            let center = p.map(|v| v as f64 + 0.5);
                            println!(
                                "{}",
                                serde_json::json!({"biome":name,"seed":terrain.config().seed.0,"ecology_revision":6,
                        "tile":tile,"target":p,"flora_count":flora.len(),"flora":flora,
                        "cacti":cacti.iter().map(|c|serde_json::json!({"origin":c.origin,"height":c.height,"bounds":c.bounds,"voxels":c.voxel_count})).collect::<Vec<_>>(),
                        "camera":[center[0]+12.0,center[1]+9.0,center[2]+15.0],"look_at":center})
                            );
                            return Ok(());
                        }
                        if inspected >= 180 {
                            return Err(
                                "no suitable flora in 180 natural tiles; choose another biome/seed"
                                    .into(),
                            );
                        }
                    }
                }
            }
        }
    }
    Err("no sufficiently strong biome found in bounded search".into())
}
