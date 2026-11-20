use super::*;
use noise::NoiseFn;

pub(super) fn build(
    habitat: &mut Habitat<'_>,
    tile: Tile,
    cells: &mut BTreeMap<Cell, u16>,
) -> Option<RockFormation> {
    let seed = ticket(habitat.terrain.config().seed.0, tile, 1501);
    let point = [
        tile[0] * TILE_SIZE + 13 + i64::from(seed % 7),
        tile[1] * TILE_SIZE + 13 + i64::from((seed >> 5) % 7),
    ];
    if habitat.reserved(point, 10) || habitat.root_distance_squared(point) < 100 {
        return None;
    }
    let (ground, column) = habitat.ground(point)?;
    let biome = column.biome.weights;
    if biome.ocean > 0.2 || column.oasis_moisture > 0.15 {
        return None;
    }
    // Most tiles remain empty. Ridges and dry cliffs favor exposed rock;
    // woodland/plains retain occasional outcrops in existing clearings.
    let probability = 0.08
        + biome.highland * 0.52
        + biome.cliffted_desert * 0.42
        + biome.forest * 0.14
        + biome.blood_forest * 0.20
        + biome.spirit_blossom * 0.18;
    if f64::from(seed >> 16) / 65536.0 >= probability {
        return None;
    }
    let material = if biome.blood_forest > 0.6 {
        MaterialId::BloodStone
    } else if biome.spirit_blossom > 0.6 {
        MaterialId::SpiritStone
    } else if biome.desert + biome.cliffted_desert > 0.55 {
        MaterialId::Sandstone
    } else {
        MaterialId::Stone
    };
    let yaw = f64::from((seed >> 12) % 360).to_radians();
    let (sin, cos) = yaw.sin_cos();
    let height = 2.6 + f64::from((seed >> 8) % 11) * 0.16;
    let lobes = [
        ([-1.1, 0.8, 0.0], [3.5, height, 3.0]),
        ([2.1, 0.1, 1.0], [2.6, height * 0.68, 2.5]),
        ([0.0, -0.3, -2.5], [2.4, 1.8, 2.3]),
    ];
    let mut planned = BTreeMap::new();
    for dz in -6..=6 {
        for dx in -6..=6 {
            let x = point[0] + dx;
            let z = point[1] + dz;
            let u = dx as f64 * cos + dz as f64 * sin;
            let v = -dx as f64 * sin + dz as f64 * cos;
            let rough = habitat
                .terrain
                .hills
                .get([x as f64 * 0.31 + 31.7, z as f64 * 0.31 - 72.4])
                * 0.14;
            let top = (-2..=7).rev().find(|dy| {
                lobes.iter().any(|(center, radius)| {
                    let normalized = [
                        (u - center[0]) / radius[0],
                        (*dy as f64 - center[1]) / radius[1],
                        (v - center[2]) / radius[2],
                    ];
                    normalized.into_iter().map(|v| v * v).sum::<f64>() <= 1.0 + rough
                })
            });
            let Some(top) = top else { continue };
            let (local, local_column) = habitat.ground([x, z])?;
            if (local - ground).abs() > 3 || habitat.reserved([x, z], 3) {
                return None;
            }
            let top = ground + top;
            if top < local {
                continue;
            }
            // Embed only the first rock cell into supported terrain; never
            // manufacture a deep rectangular foundation below a formation.
            for y in local..=top {
                let position = [x, y, z];
                if habitat.tree_occupied(position) {
                    return None;
                }
                let moss = material == MaterialId::Stone
                    && local_column.biome.humidity > 0.55
                    && y == top
                    && habitat
                        .terrain
                        .hills
                        .get([x as f64 * 0.09, z as f64 * 0.09])
                        > 0.18;
                planned.insert(
                    position,
                    if moss {
                        MaterialId::MossyStone
                    } else {
                        material
                    } as u16,
                );
            }
        }
    }
    let exposed = planned.keys().filter(|p| p[1] > ground).count();
    if exposed < 24 {
        return None;
    }
    let bounds = planned
        .keys()
        .fold([[i64::MAX; 3], [i64::MIN; 3]], |mut b, p| {
            for axis in 0..3 {
                b[0][axis] = b[0][axis].min(p[axis]);
                b[1][axis] = b[1][axis].max(p[axis]);
            }
            b
        });
    let formation = RockFormation {
        center: [point[0], ground + 1, point[1]],
        bounds,
        material: material as u16,
        voxel_count: planned.len(),
    };
    cells.extend(planned);
    Some(formation)
}
