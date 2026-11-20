//! Native biome chooser photographs of existing terrain.
use super::*;

/// Static chooser photography: bounded deterministic selection from the real
/// generator. No terrain edits, forced flat biome or fabricated backdrop.
pub(super) fn views(terrain: &TerraGenerator, kind: terra_core::BiomeKind) -> Option<Views> {
    use terra_core::BiomeKind;
    let started = Instant::now();
    let index = BiomeKind::ALL.into_iter().position(|k| k == kind)?;
    let generator = SectionGenerator::new(terrain.clone());
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
                    if material != MaterialId::Snow as u16 {
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
        BiomeKind::Forest
        | BiomeKind::CherryForest
        | BiomeKind::MushroomPark
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
    let overview = landscape_camera(terrain, photo_target, radius, height);
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

fn snow_neighbours(
    terrain: &TerraGenerator,
    generator: &SectionGenerator,
    [x, z]: [f64; 2],
) -> Vec<([i64; 3], u16)> {
    [[-24.0, 0.0], [24.0, 0.0], [0.0, -24.0], [0.0, 24.0]]
        .into_iter()
        .filter_map(|[dx, dz]| {
            let column = terrain.surface_column(x + dx, z + dz);
            if column.surface_material != MaterialId::Snow || column.water_level.is_some() {
                return None;
            }
            let p = [
                (x + dx).floor() as i64,
                (column.height - 0.5).floor() as i64,
                (z + dz).floor() as i64,
            ];
            (generator.block_at(p).ok() == Some(MaterialId::Snow as u16))
                .then_some((p, MaterialId::Snow as u16))
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
