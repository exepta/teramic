//! Camera views use generated blood water and real leaf undersides, never edits.
use super::*;
mod camera;
mod observation;

pub(super) fn views(world: &mut World, terrain: &TerraGenerator) -> Option<Views> {
    let started = Instant::now();
    let generator = SectionGenerator::new(terrain.clone());
    let explicit = std::env::var("TERRA_BLOOD_VIEW")
        .ok()
        .and_then(|value| serde_json::from_str::<[i64; 2]>(&value).ok());
    let mut candidates = Vec::new();
    if let Some(point) = explicit {
        candidates.push(point);
    }
    for ring in 0_i64..=96 {
        if explicit.is_none() {
            for n in -ring..=ring {
                for [x, z] in [[n, -ring], [n, ring], [-ring, n], [ring, n]] {
                    candidates.push([x * 96, z * 96]);
                }
            }
        }
        for [x, z] in candidates.drain(..) {
            if started.elapsed() > Duration::from_secs(30) {
                return None;
            }
            if terrain.biome_at(x as f64, z as f64).weights.blood_forest < 0.60 {
                continue;
            }
            for dz in (-80..=80).step_by(8) {
                for dx in (-80..=80).step_by(8) {
                    if started.elapsed() > Duration::from_secs(30) {
                        return None;
                    }
                    let p = [x + dx, z + dz];
                    let column = terrain.surface_column(p[0] as f64 + 0.5, p[1] as f64 + 0.5);
                    let Some(water) = column.water_level else {
                        continue;
                    };
                    if column.biome.weights.blood_forest < 0.60 || water - column.height < 0.5 {
                        continue;
                    }
                    let bed = [p[0], column.height.floor() as i64, p[1]];
                    let material = generator.block_at(bed).ok()?;
                    if material == 0 {
                        continue;
                    }
                    let Some((overview, water_points)) = camera::water_view(terrain, p, started)
                    else {
                        continue;
                    };
                    let Some(first) = find_material(
                        terrain,
                        [p[0] as f64, p[1] as f64],
                        MaterialId::BloodLeaves,
                        3,
                    ) else {
                        continue;
                    };
                    let mut leaf = None;
                    'leaf: for y in first[1] - 12..=first[1] + 4 {
                        for dz in -3..=3 {
                            for dx in -3..=3 {
                                let point = [first[0] + dx, y, first[2] + dz];
                                if generator.block_at(point).ok() == Some(406)
                                    && generator.block_at([point[0], point[1] - 1, point[2]]).ok()
                                        == Some(0)
                                {
                                    leaf = Some(point);
                                    break 'leaf;
                                }
                            }
                        }
                    }
                    let Some(leaf) = leaf else {
                        continue;
                    };
                    let target = Vec3::new(p[0] as f32 + 0.5, water as f32, p[1] as f32 + 0.5);
                    let canopy =
                        Vec3::from_array(leaf.map(|v| v as f32)) + Vec3::new(0.5, -0.8, 0.5);
                    let close = surface_eye(terrain, canopy + Vec3::new(4., -3., 6.));
                    if generator
                        .block_at(close.floor().as_i64vec3().to_array())
                        .ok()
                        != Some(0)
                    {
                        continue;
                    }
                    info!(
                        ?p,
                        water,
                        ?leaf,
                        weight = column.biome.weights.blood_forest,
                        "Native Blood Forest uses natural blood water and leaf undersides"
                    );
                    world.insert_resource(camera::Evidence::water(water_points));
                    return Some(Views {
                        overview,
                        close: Transform::from_translation(close).looking_at(canopy, Vec3::Y),
                        anchor: target.lerp(canopy, 0.5),
                        probes: vec![
                            (bed, material),
                            (leaf, 406),
                            ([leaf[0], leaf[1] - 1, leaf[2]], 0),
                        ],
                    });
                }
            }
        }
        if explicit.is_some() {
            break;
        }
    }
    None
}

pub(super) fn install(app: &mut App) {
    observation::install(app);
}

pub(super) fn inspect(world: &mut World, stage: usize) -> Result<bool, String> {
    observation::inspect(world, stage)
}

pub(super) fn picker_views(world: &mut World, terrain: &TerraGenerator) -> Option<Views> {
    camera::picker_views(world, terrain)
}

pub(super) fn inspect_picker(world: &mut World) -> Result<(), String> {
    camera::inspect(world, 0).map(|_| ())
}
