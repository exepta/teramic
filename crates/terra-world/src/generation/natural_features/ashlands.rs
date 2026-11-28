//! Ash drifts stay open between charred snags and clusters of columnar basalt.
use super::{Cell, Habitat, TILE_SIZE, Tile, ticket};
use std::collections::BTreeMap;
use terra_voxel::MaterialId as M;

pub(super) fn build(
    habitat: &mut Habitat<'_>,
    tile: Tile,
    cells: &mut BTreeMap<Cell, u16>,
    objects: &mut Vec<crate::generation::structures::StructureObject>,
    spring: &mut Vec<(Tile, crate::water::falls::Fall)>,
) {
    let origin = tile.map(|v| v * TILE_SIZE);
    let seed = habitat.terrain.config().seed.0;
    if habitat
        .terrain
        .climate_biome_at((origin[0] + 16) as f64, (origin[1] + 16) as f64)
        .weights
        .ashlands
        < 0.28
    {
        return;
    }
    let modern = habitat.terrain.config().ecology_revision >= 20;
    if modern && super::ashlands_sanctuary::build(habitat, tile, cells, objects, spring) {
        return;
    }
    if habitat.terrain.config().ecology_revision >= 21 {
        lava_field(habitat, tile, cells);
    }
    // Distinct spacing for tiny rooted tufts and sparse metre-scale silhouettes.
    for gz in 0..5 {
        for gx in 0..5 {
            let key = ticket(seed, [tile[0] * 5 + gx, tile[1] * 5 + gz], 8319);
            let p = [
                origin[0] + gx * 6 + 1 + i64::from(key % 5),
                origin[1] + gz * 6 + 1 + i64::from((key >> 8) % 5),
            ];
            let Some((y, c)) = habitat.ground(p) else {
                continue;
            };
            if c.biome.weights.ashlands < 0.51 || c.lava_level.is_some() || habitat.reserved(p, 1) {
                continue;
            }
            let kind = match (key >> 15) % 32 {
                0..=2 if modern => M::EmberBloom,
                0..=7 => M::BurntGrass,
                10 if modern => M::FallenCharredLog,
                11 if modern => M::CinderRoot,
                8..=11 => M::AshBush,
                12..=13 => M::AshMushrooms,
                14 => M::BlackenedStump,
                15 => M::CharredSapling,
                16..=17 => M::EmberCrystals,
                18 => M::Fumarole,
                19 => M::FossilBones,
                _ => continue,
            };
            if !habitat.terrain.in_prop_patch(p, kind as u16) {
                continue;
            }
            // A vent replaces the bank's surface rock so the plume exits at grade.
            let elevation = if kind == M::Fumarole { y } else { y + 1 };
            cells.entry([p[0], elevation, p[1]]).or_insert(kind as u16);
        }
    }
    let key = ticket(seed, tile, 8320);
    let point = [
        origin[0] + 7 + i64::from(key % 18),
        origin[1] + 7 + i64::from((key >> 8) % 18),
    ];
    let Some((base, c)) = habitat.ground(point) else {
        return;
    };
    if c.biome.weights.ashlands < 0.70
        || c.lava_level.is_some()
        || habitat.reserved(point, 5)
        || (habitat.terrain.config().ecology_revision >= 21
            && cells.contains_key(&[point[0], base, point[1]]))
    {
        return;
    }
    if modern && habitat.terrain.config().trees && key % 5 <= 2 {
        super::relic_trees::TreeForm {
            root: [point[0], base, point[1]],
            height: 9. + f64::from(key % 8),
            radius: 1.5 + f64::from((key >> 8) % 7) * 0.13,
            wood: M::CharredLog,
            seed: key,
        }
        .grow(habitat, cells);
        return;
    }
    if key % 5 < 2 || modern && key % 5 == 4 {
        // Jointed columns grow in groups with a high shoulder and stepped margins.
        for dz in -3i64..=3 {
            for dx in -3i64..=3 {
                let p = [point[0] + dx, point[1] + dz];
                let variation = ticket(seed, p, 8321);
                let radius = (dx * dx + dz * dz) as f64;
                if radius > 8. + f64::from(variation % 3) {
                    continue;
                }
                let column = habitat.column(p);
                let floor = (column.height - 0.5).floor() as i64;
                if column.lava_level.is_some() || (floor - base).abs() > 3 {
                    continue;
                }
                let summit =
                    base + 2 + (3. - radius.sqrt()).round() as i64 + i64::from(variation % 4);
                for y in floor + 1..=summit {
                    cells.insert([p[0], y, p[1]], M::ColumnarBasalt as u16);
                }
            }
        }
    } else if key.is_multiple_of(3) && habitat.terrain.config().trees {
        let [x, z] = point;
        let height = 5 + i64::from(key % 5);
        let skew = if key & 8 == 0 { -1 } else { 1 };
        for i in 1..=height {
            let offset = if i >= height - 2 { skew } else { 0 };
            cells.insert([x + offset, base + i, z], M::CharredLog as u16);
            if i == height - 2 {
                cells.insert([x, base + i, z], M::CharredLog as u16);
            }
        }
        for (j, [dx, dz]) in [[1, 0], [-1, 0], [0, 1], [0, -1]].into_iter().enumerate() {
            let y = base + 2 + i64::from((key >> (j * 3)) % 3);
            for d in 1..=2 {
                cells.insert([x + dx * d, y + d - 1, z + dz * d], M::CharredLog as u16);
                cells.insert([x + dx * d, y + d, z + dz * d], M::CharredLog as u16);
            }
            cells.insert([x + dx * 2, y + 3, z + dz * 2], M::CharredSapling as u16);
        }
    }
}

/// Split basalt pressure ridges, hollow spatter chimneys and collapsed glassy
/// horns collect in separate fields. All deposits grow up from real dry ground.
fn lava_field(h: &mut Habitat<'_>, tile: Tile, cells: &mut BTreeMap<Cell, u16>) {
    let seed = h.terrain.config().seed.0;
    let key = ticket(seed, tile, 8394);
    let field = ticket(seed, tile.map(|v| v.div_euclid(3)), 8395);
    if key.is_multiple_of(4) {
        return;
    }
    let [x, z] =
        [0, 1].map(|axis| tile[axis] * TILE_SIZE + 9 + i64::from((key >> (axis * 8)) % 14));
    let Some((base, col)) = h.ground([x, z]) else {
        return;
    };
    if col.biome.weights.ashlands < 0.64 || h.reserved([x, z], 9) {
        return;
    }
    let (sin, cos) = (f64::from(field % 1024) * std::f64::consts::TAU / 1024.).sin_cos();
    let form = field % 3;
    let chimney = form == 0;
    let height = 4. + f64::from((key >> 18) % 4);
    for dz in -8_i64..=8 {
        for dx in -8_i64..=8 {
            let (u, v) = (
                dx as f64 * cos + dz as f64 * sin,
                dz as f64 * cos - dx as f64 * sin,
            );
            let side = v - 0.85 * (u * 0.46).sin();
            let salt = ticket(seed, [x + dx, z + dz], 8396);
            let grain = f64::from(salt % 13) / 16.;
            let r = (u * 0.90).hypot(v * 1.14);
            let top = match form {
                // A broken high lip surrounds a gas outlet, with apron and one breached side.
                0 => (height * (1. - ((r - 2.1) / 2.8).abs()).max(0.) + grain
                    - if u > 0. && v.abs() < 1.4 { 2.3 } else { 0. })
                .floor(),
                // A linear ridge of upturned slabs with a deep longitudinal contraction crack.
                1 => (height
                    * (1. - (u / 8.).powi(2)).max(0.)
                    * (1. - ((side.abs() - 1.6) / 3.).abs()).max(0.)
                    + grain)
                    .floor(),
                // Three uneven glassy spines grow out of the same lava dome.
                _ => (grain
                    + [(-3.1, 1.1, 0.62), (0., -0.3, 1.), (2.8, 1.4, 0.78)]
                        .into_iter()
                        .map(|(a, b, gain)| {
                            height * gain * (1. - (u - a).hypot((v - b) * 1.25) / 3.).max(0.)
                        })
                        .fold(0., f64::max))
                .floor(),
            } as i64;
            if top <= 0 && !(chimney && r < 2.6) {
                continue;
            }
            let p = [x + dx, z + dz];
            let sample = h.column(p);
            let floor = (sample.height - 0.5).floor() as i64;
            if sample.lava_level.is_some()
                || sample.water_level.is_some_and(|v| v > floor as f64 + 1.)
                || (floor - base).abs() > 4
                || sample.cave.open_to_sky()
            {
                continue;
            }
            for y in floor..=floor + top {
                let m = if chimney && dx == 0 && dz == 0 && y == floor + top {
                    M::Fumarole
                } else if chimney && r < 1.6 || form == 1 && side.abs() < 1.1 {
                    if salt.is_multiple_of(9) {
                        M::EmberCrystalOre
                    } else {
                        M::LavaCrust
                    }
                } else if form == 2 && y > floor + 1 {
                    M::VolcanicObsidian
                } else if (y - base + (u * 0.28).floor() as i64).rem_euclid(4) == 0 {
                    M::DarkVolcanicStone
                } else if top == 1 && salt.is_multiple_of(4) {
                    M::VolcanicGravel
                } else {
                    M::ColumnarBasalt
                };
                cells.insert([p[0], y, p[1]], m as u16);
            }
        }
    }
}
