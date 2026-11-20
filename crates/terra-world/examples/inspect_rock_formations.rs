//! Genuine native-camera targets; no world edits or fixture blocks.
use terra_core::{Config, WorldConfig};
use terra_world::TerraGenerator;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut config = if std::env::var_os("TERRA_ROCK_USE_CONFIG").is_some() {
        WorldConfig::from_toml(include_str!("../../../configs/world.toml"))?
    } else {
        WorldConfig::default()
    };
    config.ecology_revision = 5;
    let terrain = TerraGenerator::new(config)?;
    let mut found = 0;
    for radius in 0_i64..=32 {
        for z in -radius..=radius {
            for x in -radius..=radius {
                if x.abs().max(z.abs()) != radius {
                    continue;
                }
                for rock in terrain.surface_rock_formations([x, z]) {
                    let center = rock.center.map(|v| v as f64 + 0.5);
                    let eye = [center[0] + 12.0, center[1] + 9.0, center[2] + 15.0];
                    println!(
                        "{}",
                        serde_json::json!({"seed":terrain.config().seed.0,"ecology_revision":5,"tile":[x,z],"center":rock.center,"bounds":rock.bounds,"material":rock.material,"voxels":rock.voxel_count,"camera":eye,"look_at":center})
                    );
                    found += 1;
                    if found == 2 {
                        return Ok(());
                    }
                }
            }
        }
    }
    Err("no two supported natural rock groups found within the bounded search".into())
}
