//! Native biome chooser photographs of existing terrain.
use super::*;

/// Static chooser photography: bounded deterministic selection from the real
/// generator. No terrain edits, forced flat biome or fabricated backdrop.
pub(super) fn views(terrain: &TerraGenerator, kind: terra_core::BiomeKind) -> Option<Views> {
    use terra_core::BiomeKind;
    let started = Instant::now();
    let index = BiomeKind::ALL.into_iter().position(|k| k == kind)?;
    let generator = SectionGenerator::new(terrain.clone());
    if kind == BiomeKind::Swamp && terrain.config().seed.0 == 42042 {
        return swamp_views(terrain, &generator);
    }
    if terrain.config().seed.0 == 42042 && terrain.config().ecology_revision >= 20 {
        let composition = match kind {
            BiomeKind::MistyCoast if terrain.config().ecology_revision >= 22 => Some((
                Vec3::new(259.5, 28.5, -150.5),
                Vec3::new(238.5, 37.5, -111.5),
                vec![([240, 44, -112], MaterialId::BeaconLens as u16)],
            )),
            BiomeKind::CrystalHighlands if terrain.config().ecology_revision >= 21 => Some((
                Vec3::new(-1336.5, 61.6, 551.3),
                Vec3::new(-1359.5, 58.0, 562.5),
                vec![
                    ([-1337, 61, 551], 0),
                    ([-1360, 59, 562], MaterialId::MassiveAquamarine as u16),
                ],
            )),
            BiomeKind::Ashlands => Some((
                Vec3::new(2380.5, 37., -4669.5),
                Vec3::new(2384.5, 34.7, -4652.5),
                vec![
                    ([2377, 36, -4657], MaterialId::Lava as u16),
                    ([2389, 31, -4659], 0),
                    ([2389, 30, -4659], MaterialId::GeyserVent as u16),
                ],
            )),
            BiomeKind::PetrifiedForest => Some((
                Vec3::new(-660.5, 44., 421.5),
                Vec3::new(-655.5, 40.7, 409.2),
                vec![
                    ([-660, 38, 412], MaterialId::AshenBrazier as u16),
                    ([-652, 40, 411], MaterialId::TemplePennant as u16),
                ],
            )),
            _ => None,
        };
        if let Some((eye, target, probes)) = composition
            && generator.block_at(eye.floor().as_i64vec3().to_array()).ok() == Some(0)
            && probes
                .iter()
                .all(|&(cell, material)| generator.block_at(cell).ok() == Some(material))
        {
            let overview = Transform::from_translation(eye).looking_at(target, Vec3::Y);
            return Some(Views {
                overview,
                close: overview,
                anchor: target,
                probes,
            });
        }
    }
    if kind == BiomeKind::PetrifiedForest && terrain.config().seed.0 == 42042 {
        // A real abandoned dig backed by the petrified grove on the western shelf.
        let tile = [-21, 21];
        if let Some((cell, material)) = terrain
            .surface_flora(tile)
            .into_iter()
            .find(|(_, id)| *id == MaterialId::ExcavationCrate as u16)
        {
            let target = Vec3::new(-654.5, cell[1] as f32 + 1.8, 687.8);
            let eye = Vec3::new(-675., cell[1] as f32 + 10.2, 682.5);
            if generator.block_at(eye.floor().as_i64vec3().to_array()).ok() == Some(0) {
                let overview = Transform::from_translation(eye).looking_at(target, Vec3::Y);
                return Some(Views {
                    overview,
                    close: overview,
                    anchor: target,
                    probes: vec![(cell, material)],
                });
            }
        }
    }
    // This sightline spans a crystal shelf, its lava channel and the volcanic rim.
    // Recheck the saved seed's column and fluid before using the composition.
    if kind == BiomeKind::Ashlands && terrain.config().seed.0 == 42042 {
        let eye = Vec3::new(2207.75, 28.85, -4633.37);
        let target = Vec3::new(2221.47, 29.51, -4650.57);
        let overview = Transform::from_translation(eye).looking_at(target, Vec3::Y);
        for z in -4650..=-4620 {
            for x in 2193..=2224 {
                let column = terrain.surface_column(x as f64 + 0.5, z as f64 + 0.5);
                if column.biome_kind(terrain.config().sea_level) != kind {
                    continue;
                }
                let Some(level) = column.lava_level else {
                    continue;
                };
                let bed = [x, level.floor() as i64 - 1, z];
                let air = [x, level.ceil() as i64 + 2, z];
                if generator.block_at(bed).ok() == Some(MaterialId::Lava as u16)
                    && generator.block_at(air).ok() == Some(0)
                {
                    return Some(Views {
                        overview,
                        close: overview,
                        anchor: Vec3::new(x as f32 + 0.5, level as f32, z as f32 + 0.5),
                        probes: vec![(bed, MaterialId::Lava as u16), (air, 0)],
                    });
                }
            }
        }
    }
    if kind == BiomeKind::Ocean && terrain.config().seed.0 == 42042 {
        // A real coral garden on the offshore side of this photographed coast.
        let eye = Vec3::new(-1449.42, -5.38, 1880.75);
        let target = Vec3::new(-1447.42, -7.58, 1873.75);
        if let Some(view) = reef_coast_view(terrain, &generator, eye, target) {
            return Some(view);
        }
    }
    // The slope east of the parasols sees the Fly Agaric and cream cap
    // canopies across the same clearing, with the stems visible below them.
    if kind == BiomeKind::MushroomPark && terrain.config().seed.0 == 42042 {
        let eye = Vec3::new(-892., 78., -379.);
        let target = Vec3::new(-933., 64., -349.);
        let cap = [-937, 69, -345];
        let eye_cell = eye.as_i64vec3().to_array();
        if generator.block_at(cap).ok() == Some(MaterialId::RedMushroomCap as u16)
            && generator.block_at(eye_cell).ok() == Some(0)
            && terrain
                .biome_at(target.x as f64, target.z as f64)
                .weights
                .dominant()
                == kind
        {
            let overview = Transform::from_translation(eye).looking_at(target, Vec3::Y);
            return Some(Views {
                overview,
                close: overview,
                anchor: target,
                probes: vec![(cap, MaterialId::RedMushroomCap as u16), (eye_cell, 0)],
            });
        }
    }
    let mut selected = None;
    'search: for ring in 0_i64..=64 {
        for n in -ring..=ring {
            for [gx, gz] in [[n, -ring], [n, ring], [-ring, n], [ring, n]] {
                if started.elapsed() > Duration::from_secs(40) {
                    error!(
                        ?kind,
                        "No natural biome photo with its required snow/coast habitat found within 40 seconds"
                    );
                    return None;
                }
                let [x, z] = [gx as f64 * 128.0 + 0.5, gz as f64 * 128.0 + 0.5];
                let biome = terrain.biome_at(x, z);
                // Coastlines are real biome transitions. Require a marine
                // anchor, but allow its landward neighbours to be dry land.
                let minimum_weight = if kind == BiomeKind::Ocean { 0.50 } else { 0.72 };
                if biome.weights.values()[index] < minimum_weight {
                    continue;
                }
                let column = terrain.surface_column(x, z);
                if column.biome_kind(terrain.config().sea_level) != kind {
                    continue;
                }
                if kind != BiomeKind::Ocean
                    && (column.water_level.is_some()
                        || column.height < terrain.config().sea_level + 2.0)
                {
                    continue;
                }
                // Prefer a coherent panorama, not a narrow biome transition.
                if kind != BiomeKind::Ocean
                    && [[-48.0, 0.0], [48.0, 0.0], [0.0, -48.0], [0.0, 48.0]]
                        .into_iter()
                        .any(|[dx, dz]| {
                            terrain.biome_at(x + dx, z + dz).weights.values()[index] < 0.50
                        })
                {
                    continue;
                }
                if matches!(kind, BiomeKind::Highland | BiomeKind::ClifftedDesert) {
                    let heights = [[-48.0, 0.0], [48.0, 0.0], [0.0, -48.0], [0.0, 48.0]]
                        .map(|[dx, dz]| terrain.surface_height(x + dx, z + dz));
                    let low = heights.into_iter().fold(column.height, f64::min);
                    let high = heights.into_iter().fold(column.height, f64::max);
                    if high - low < 8.0 {
                        continue;
                    }
                }
                if kind == BiomeKind::Forest {
                    let heights = [[-40.0, 0.0], [40.0, 0.0], [0.0, -40.0], [0.0, 40.0]]
                        .map(|[dx, dz]| terrain.surface_height(x + dx, z + dz));
                    let low = heights.into_iter().fold(column.height, f64::min);
                    let high = heights.into_iter().fold(column.height, f64::max);
                    if column.slope > 0.6 || high - low > 24.0 {
                        continue;
                    }
                    let roots = terrain.tree_appearance([
                        (x.floor() as i64).div_euclid(16) * 16,
                        0,
                        (z.floor() as i64).div_euclid(16) * 16,
                    ]);
                    let woodland = roots
                        .tree_roots()
                        .filter(|(root, appearance)| {
                            !matches!(appearance, TreeAppearance::Mushroom | TreeAppearance::Palm)
                                && (root[0] as f64 - x).abs() <= 24.0
                                && (root[2] as f64 - z).abs() <= 24.0
                        })
                        .take(4)
                        .count();
                    if woodland < 4 {
                        continue;
                    }
                }
                let visible = column
                    .water_level
                    .unwrap_or(column.height)
                    .max(column.height);
                // Fluids live separately from solid blocks. An ocean's surface
                // is Air in block_at; validate its real bed and open surface.
                let floor = if kind == BiomeKind::Ocean {
                    let Some(water) = terrain.water_column(x, z) else {
                        continue;
                    };
                    if water.depth < 3.0
                        || (water.level - terrain.config().sea_level).abs() > 0.01
                        || column.ocean.ice_depth > 0
                    {
                        continue;
                    }
                    water.bed as i64 - 1
                } else {
                    (visible - 0.5).floor() as i64
                };
                let probe = [x.floor() as i64, floor, z.floor() as i64];
                let air = [probe[0], visible.ceil() as i64, probe[2]];
                let Ok(material) = generator.block_at(probe) else {
                    continue;
                };
                if material == 0 || generator.block_at(air).ok() != Some(0) {
                    continue;
                }
                let mut probes = vec![(probe, material), (air, 0)];
                let coast = if kind == BiomeKind::Ocean {
                    let Some((coast, coast_probes)) = shore(terrain, &generator, [x, z]) else {
                        continue;
                    };
                    probes.extend(coast_probes);
                    Some(coast)
                } else {
                    None
                };
                if kind == BiomeKind::Highland {
                    if !snowfield_surface(material) {
                        continue;
                    }
                    // Photograph an existing snowfield, not a lone snowy cell.
                    let snow = snow_neighbours(terrain, &generator, [x, z]);
                    if snow.len() < 2 {
                        continue;
                    }
                    probes.extend(snow);
                }
                selected = Some(([x, z], visible, probes, coast));
                break 'search;
            }
        }
    }
    let Some(([x, z], visible, probes, coast)) = selected else {
        error!(
            ?kind,
            "No natural biome photo with its required snow/coast habitat found within 8192 metres"
        );
        return None;
    };
    let point = Vec3::new(x as f32, visible as f32, z as f32);
    let (radius, height) = match kind {
        BiomeKind::Highland | BiomeKind::ClifftedDesert => (80.0, 42.0),
        BiomeKind::MushroomPark => (66.0, 36.0),
        BiomeKind::Plains => (46.0, 16.0),
        BiomeKind::Savanna => (46.0, 13.0),
        BiomeKind::Swamp => (34.0, 8.0),
        BiomeKind::Forest
        | BiomeKind::CherryForest
        | BiomeKind::SpiritBlossom
        | BiomeKind::BloodForest => (62.0, 30.0),
        BiomeKind::Ocean => (62.0, 32.0),
        _ => (74.0, 28.0),
    };
    let photo_target = if let Some(coast) = coast {
        // Both the marine anchor and the verified dry coast fit the frame.
        point.lerp(coast, 0.55)
    } else if kind == BiomeKind::Forest {
        point + Vec3::Y * 8.0
    } else {
        point
    };
    let overview = if matches!(
        kind,
        BiomeKind::MushroomPark | BiomeKind::Plains | BiomeKind::Savanna
    ) {
        clear_grove_camera(&generator, photo_target, radius, height)
    } else {
        None
    }
    .unwrap_or_else(|| landscape_camera(terrain, photo_target, radius, height));
    info!(
        ?kind,
        seed = terrain.config().seed.0,
        ?probes,
        ?overview,
        "Native biome chooser photo uses actual generated terrain"
    );
    Some(Views {
        overview,
        close: landscape_camera(terrain, point - Vec3::Y * 2.5, 10.0, 9.0),
        anchor: point,
        probes,
    })
}

/// The raised caps of giant mushrooms can meet a camera in midair. Sample
/// the opening of the lens as well as the centre of its sightline.
fn clear_grove_camera(
    generator: &SectionGenerator,
    point: Vec3,
    radius: f32,
    height: f32,
) -> Option<Transform> {
    let target = point + Vec3::Y * 3.0;
    for step in 0..8 {
        let angle = (step as f32 + 3.0) * std::f32::consts::FRAC_PI_4;
        let outward = Vec3::new(angle.cos(), 0.0, angle.sin());
        let right = Vec3::new(outward.z, 0.0, -outward.x);
        let eye = point + outward * radius + Vec3::Y * height;
        let clear = [-2.0, -1.0, 0.0, 1.0, 2.0].into_iter().all(|side| {
            [0.0, 0.02, 0.10, 0.2, 0.32, 0.45, 0.62]
                .into_iter()
                .all(|depth| {
                    let p = eye.lerp(target, depth) + right * side * (1.0 + 5.0 * depth);
                    generator.block_at(p.floor().as_i64vec3().to_array()).ok() == Some(0)
                })
        });
        if clear {
            return Some(Transform::from_translation(eye).looking_at(target, Vec3::Y));
        }
    }
    None
}

fn reef_coast_view(
    terrain: &TerraGenerator,
    generator: &SectionGenerator,
    eye: Vec3,
    target: Vec3,
) -> Option<Views> {
    let reef = terrain.surface_column(eye.x as f64, eye.z as f64);
    let level = reef.water_level?;
    if reef.ocean.weights[0] < 0.5 || reef.height >= eye.y as f64 || level <= eye.y as f64 {
        return None;
    }
    // Verify air above the ocean, the flooded reef bed, and a connected
    // sandy coast within the same streamed area. All probes are natural.
    let landward = Vec3::new(-1530., level as f32, 1751.);
    for n in 1..8 {
        let point = landward.lerp(Vec3::new(eye.x, level as f32, eye.z), n as f32 * 0.10);
        let x = point.x.floor() as i64;
        let z = point.z.floor() as i64;
        let Some(water) = terrain.water_column(x as f64 + 0.5, z as f64 + 0.5) else {
            continue;
        };
        if water.depth < 3.0 {
            continue;
        }
        if let Some((_, coast)) = shore(terrain, generator, [x as f64 + 0.5, z as f64 + 0.5]) {
            let floor = [x, water.bed as i64 - 1, z];
            let surface = [x, level.ceil() as i64, z];
            let bed = [
                eye.x.floor() as i64,
                (reef.height - 0.5).floor() as i64,
                eye.z.floor() as i64,
            ];
            let a = generator.block_at(floor).ok()?;
            let b = generator.block_at(bed).ok()?;
            if a == 0 || b == 0 || generator.block_at(surface).ok() != Some(0) {
                continue;
            }
            let overview = Transform::from_translation(eye).looking_at(target, Vec3::Y);
            return Some(Views {
                overview,
                close: overview,
                anchor: Vec3::new(x as f32 + 0.5, level as f32, z as f32 + 0.5),
                probes: vec![(floor, a), (surface, 0), coast[0], coast[1]],
            });
        }
    }
    None
}

pub(super) fn snowfield_surface(material: u16) -> bool {
    matches!(
        MaterialId::from_raw(material),
        Some(
            MaterialId::Snow | MaterialId::Firn | MaterialId::IcySnow | MaterialId::SnowCoveredSod
        )
    )
}

fn snow_neighbours(
    terrain: &TerraGenerator,
    generator: &SectionGenerator,
    [x, z]: [f64; 2],
) -> Vec<([i64; 3], u16)> {
    [[-24.0, 0.0], [24.0, 0.0], [0.0, -24.0], [0.0, 24.0]]
        .into_iter()
        .filter_map(|[dx, dz]| {
            let column = terrain.surface_column(x + dx, z + dz);
            let material = column.surface_material as u16;
            if !snowfield_surface(material) || column.water_level.is_some() {
                return None;
            }
            let p = [
                (x + dx).floor() as i64,
                (column.height - 0.5).floor() as i64,
                (z + dz).floor() as i64,
            ];
            (generator.block_at(p).ok() == Some(material)).then_some((p, material))
        })
        .collect()
}

type Shore = (Vec3, [([i64; 3], u16); 2]);

fn shore(
    terrain: &TerraGenerator,
    generator: &SectionGenerator,
    [x, z]: [f64; 2],
) -> Option<Shore> {
    for distance in (16..=64).step_by(8) {
        for direction in 0..8 {
            let angle = f64::from(direction) * std::f64::consts::FRAC_PI_4;
            let [sx, sz] = [
                (x + angle.cos() * f64::from(distance)).floor() + 0.5,
                (z + angle.sin() * f64::from(distance)).floor() + 0.5,
            ];
            let column = terrain.surface_column(sx, sz);
            if column.water_level.is_some()
                || column.height < terrain.config().sea_level + 1.0
                || column.height > terrain.config().sea_level + 12.0
                || !matches!(
                    column.surface_material,
                    MaterialId::Sand | MaterialId::Grass | MaterialId::Dirt | MaterialId::Gravel
                )
            {
                continue;
            }
            let p = [
                sx.floor() as i64,
                (column.height - 0.5).floor() as i64,
                sz.floor() as i64,
            ];
            let air = [p[0], column.height.ceil() as i64, p[2]];
            let material = generator.block_at(p).ok()?;
            if MaterialId::from_raw(material).is_none_or(|m| !m.is_solid())
                || generator.block_at(air).ok() != Some(0)
            {
                continue;
            }
            return Some((
                Vec3::new(sx as f32, column.height as f32, sz as f32),
                [(p, material), (air, 0)],
            ));
        }
    }
    None
}

/// Photographs the three native landmarks from the landing side, over the water.
fn swamp_views(terrain: &TerraGenerator, generator: &SectionGenerator) -> Option<Views> {
    use terra_world::generation::natural_features::SwampLandmark::*;
    let (tile, kind) = match std::env::var("TERRA_SWAMP_LANDMARK").as_deref() {
        Ok("ruins") => ([0, -358], SunkenRuins),
        Ok("hermit") => ([6, -354], AbandonedHut),
        _ => ([164, -229], WitchesLodge),
    };
    let site = terrain.swamp_site(tile)?;
    if site.kind != kind {
        return None;
    }
    let center = Vec3::from_array(site.floor.map(|v| v as f32)) + Vec3::splat(0.5);
    let landing = Vec3::from_array(site.landing().map(|v| v as f32)) + Vec3::splat(0.5);
    let front = (landing - center).normalize();
    let right = Vec3::Y.cross(front);
    let target = center + Vec3::Y * 3.6;
    let mut landmark = site.floor;
    if kind == SunkenRuins {
        landmark[1] += 8;
    }
    let material = generator.block_at(landmark).ok()?;
    if !matches!(
        MaterialId::from_raw(material),
        Some(
            MaterialId::WillowPlanks | MaterialId::MossySwampStone | MaterialId::SwampRuinedBricks
        )
    ) {
        return None;
    }
    // A tree between the lens and the porch can conceal the house. Choose a clear sightline.
    for side in [-12., 12., -5., 5., 0.] {
        for (distance, height) in [(25., 6.), (30., 11.), (21., 4.), (34., 15.)] {
            let eye = center + front * distance + right * side + Vec3::Y * height;
            if !(0..12).all(|n| {
                generator
                    .block_at(
                        eye.lerp(target, n as f32 / 16.)
                            .floor()
                            .as_i64vec3()
                            .to_array(),
                    )
                    .ok()
                    == Some(0)
            }) {
                continue;
            }
            let camera = Transform::from_translation(eye).looking_at(target, Vec3::Y);
            return Some(Views {
                overview: camera,
                close: camera,
                anchor: center,
                probes: vec![
                    (landmark, material),
                    (eye.floor().as_i64vec3().to_array(), 0),
                ],
            });
        }
    }
    None
}
