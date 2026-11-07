//! Find a real populated village and dry player-height camera in the supplied world.
use std::path::PathBuf;
use terra_core::{Config, WorldConfig};
use terra_voxel::MaterialId;
use terra_world::{
    SectionGenerator, TerraGenerator,
    generation::structures::{BuildingSummary, StructureKind},
};

// Clearance survives the benchmark's eye->feet->body f32 teleport conversion.
const CAMERA_ABOVE_SUPPORT: f64 = 2.65;

fn options() -> Result<(PathBuf, i64), String> {
    let mut configs = PathBuf::from("configs");
    let mut radius = 12;
    let mut arguments = std::env::args().skip(1);
    while let Some(option) = arguments.next() {
        if matches!(option.as_str(), "--help" | "-h") {
            println!(
                "performance_scene [--configs DIR] [--max-radius 0..12]\nPrint one JSON object with a real village camera/look_at. The supplied world seed and settings are unchanged."
            );
            std::process::exit(0);
        }
        let value = arguments
            .next()
            .ok_or_else(|| format!("Missing value for {option}"))?;
        match option.as_str() {
            "--configs" => configs = value.into(),
            "--max-radius" => {
                radius = value.parse().map_err(|_| "Invalid max-radius")?;
                if !(0..=12).contains(&radius) {
                    return Err("max-radius must be 0..12".into());
                }
            }
            _ => return Err(format!("Unknown option {option}")),
        }
    }
    Ok((configs, radius))
}

fn candidate(
    generator: &SectionGenerator,
    building: &BuildingSummary,
) -> Option<([f64; 3], [f64; 3])> {
    let middle = std::array::from_fn::<_, 3, _>(|axis| {
        (building.bounds[0][axis] + building.bounds[1][axis]) as f64 * 0.5
    });
    let entrance = building.entrance;
    let difference = [
        entrance[0] as f64 - middle[0],
        entrance[2] as f64 - middle[2],
    ];
    let outward = if difference[0].abs() > difference[1].abs() {
        [difference[0].signum() as i64, 0]
    } else {
        [0, difference[1].signum() as i64]
    };
    for distance in [3, 5, 7, 9] {
        for lateral in [0, -2, 2] {
            let x = entrance[0] + outward[0] * distance - outward[1] * lateral;
            let z = entrance[2] + outward[1] * distance + outward[0] * lateral;
            for floor in (entrance[1] - 6..=entrance[1] + 3).rev() {
                let support = generator.block_at([x, floor, z]).ok()?;
                if !MaterialId::from_raw(support).is_some_and(MaterialId::is_opaque) {
                    continue;
                }
                let clear = (1..=2).all(|dy| {
                    generator
                        .block_at([x, floor + dy, z])
                        .is_ok_and(|material| {
                            material != MaterialId::FluidWater as u16
                                && MaterialId::from_raw(material).is_some_and(|id| !id.is_solid())
                        })
                });
                if clear {
                    // Keep the capsule 3 cm above the support after f32 rounding.
                    let camera = [
                        x as f64 + 0.5,
                        floor as f64 + CAMERA_ABOVE_SUPPORT,
                        z as f64 + 0.5,
                    ];
                    let target = [
                        entrance[0] as f64 + 0.5,
                        entrance[1] as f64 + 1.6,
                        entrance[2] as f64 + 0.5,
                    ];
                    return Some((camera, target));
                }
            }
        }
    }
    None
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (configs, radius) = options()?;
    let config = WorldConfig::load(configs.join("world.toml"))?;
    let seed = config.seed.0;
    let terrain = TerraGenerator::new(config)?;
    let generator = SectionGenerator::new(terrain.clone());
    for ring in 0_i64..=radius {
        for z in -ring..=ring {
            for x in -ring..=ring {
                if ring != 0 && x.abs() != ring && z.abs() != ring {
                    continue;
                }
                let Some(site) = terrain
                    .structure_site([x, z])
                    .filter(|site| site.kind == StructureKind::Village)
                else {
                    continue;
                };
                let residents = terrain.village_residents([x, z]).len();
                if residents == 0 {
                    continue;
                }
                for building in &site.buildings {
                    if let Some((camera, look_at)) = candidate(&generator, building) {
                        println!(
                            "{{\"seed\":{seed},\"region\":[{x},{z}],\"village_id\":{},\"style\":\"{:?}\",\"residents\":{residents},\"camera\":{camera:?},\"look_at\":{look_at:?}}}",
                            site.id, site.style
                        );
                        return Ok(());
                    }
                }
            }
        }
    }
    Err(format!("No populated village with a dry camera in {radius} structure regions").into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camera_keeps_capsule_off_the_floor_through_native_f32_teleport_conversion() {
        // bootstrap converts eye to feet with these player dimensions; /tp
        // adds half-height and its 2 cm clearance back to obtain the body centre.
        let eye_offset = 0.72_f32;
        let half_height = 0.9_f32;
        let teleport_clearance = 0.02_f32;
        for floor in -127..=350 {
            let eye = (f64::from(floor) + CAMERA_ABOVE_SUPPORT) as f32;
            let feet = eye - (eye_offset + half_height + teleport_clearance);
            let centre = feet + (half_height + teleport_clearance);
            let bottom = centre - half_height;
            let top = centre + half_height;
            assert!(bottom > floor as f32 + 1.0, "floor={floor} bottom={bottom}");
            assert!(top < floor as f32 + 3.0, "floor={floor} top={top}");
        }
    }
}
