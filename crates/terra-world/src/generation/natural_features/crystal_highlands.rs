//! Alpine mineral colonies. Each quartz needle grows out of a common host
//! rock; lower sheltered aprons support mats of saxifrage and hardy grasses.
use super::{BTreeMap, Cell, Habitat, TILE_SIZE, Tile, ticket};
use noise::NoiseFn;
use terra_voxel::MaterialId as M;

pub(super) fn build(
    h: &mut Habitat<'_>,
    tile: Tile,
    cells: &mut BTreeMap<Cell, u16>,
    objects: &mut Vec<crate::generation::structures::StructureObject>,
) {
    let lo = tile.map(|v| v * TILE_SIZE);
    let seed = h.terrain.config().seed.0;
    let anchor = ticket(seed, tile, 0xc455_7a11);
    let center = [
        lo[0] + 12 + i64::from(anchor % 8),
        lo[1] + 12 + i64::from((anchor >> 8) % 8),
    ];
    if h.terrain
        .climate_biome_at(center[0] as f64 + 0.5, center[1] as f64 + 0.5)
        .weights
        .crystal_highlands
        < 0.32
    {
        return;
    }
    let col = h.column(center);
    if col.biome.weights.crystal_highlands < 0.32 || h.reserved(center, 6) {
        return;
    }
    let geo = h
        .terrain
        .hills
        .get([center[0] as f64 / 64. + 81.2, center[1] as f64 / 61. - 22.6]);
    let ground = (col.height - 0.5).floor() as i64;
    if col.biome.weights.crystal_highlands > 0.51
        && col.water_level.is_none_or(|w| w <= col.height)
        && !col.cave.open_to_sky()
        && col.slope < 2.4
    {
        if geo > -0.26 && !anchor.is_multiple_of(6) {
            let major = geo > 0.04 && !(anchor >> 11).is_multiple_of(4);
            let base = if major {
                8.0 + f64::from((anchor >> 14) % 6)
            } else {
                3.4 + f64::from((anchor >> 14) % 4)
            };
            colony(h, cells, objects, center, ground, base, anchor);
        } else if geo < -0.24 && (anchor >> 12).is_multiple_of(7) {
            arch(h, cells, center, anchor);
        }
    }
    if h.terrain.config().trees
        && geo < 0.20
        && anchor.is_multiple_of(3)
        && super::crystal_landmarks::candidate(h.terrain, tile).is_none()
    {
        krummholz(h, lo, anchor, cells);
    }
    for patch in 0..7 {
        let p = ticket(seed, tile, 0x5afe_7400 + patch);
        let ax = lo[0] + 4 + i64::from(p % 24);
        let az = lo[1] + 4 + i64::from((p >> 7) % 24);
        let mineral = geo > 0.18 && patch == 0;
        // Mineral pockets are individual exposures on a ledge. The denser
        // number of samples is for spreading moss and grass over a patch.
        let samples = if mineral { 2 + p % 3 } else { 11 + p % 11 };
        for k in 0..samples {
            let n = ticket(seed, [ax + i64::from(k), az], 0x1a9c);
            let x = ax + i64::from(n % 9) - 4;
            let z = az + i64::from((n >> 6) % 9) - 4;
            let Some((y, column)) = h.ground([x, z]) else {
                continue;
            };
            if column.biome.weights.crystal_highlands < 0.45
                || h.reserved([x, z], 0)
                || cells.contains_key(&[x, y + 1, z])
            {
                continue;
            }
            let id = if h
                .terrain
                .crystal_summit(x as f64 + 0.5, z as f64 + 0.5, column.height)
            {
                if !mineral && !n.is_multiple_of(4) {
                    continue;
                }
                if mineral {
                    M::MixedCrystalVein
                } else {
                    M::HighlandScree
                }
            } else if mineral {
                match (n >> 16) % 9 {
                    0 => M::AuriciteDeposit,
                    1 | 2 => M::MixedCrystalVein,
                    3 => M::CrystalCrown,
                    _ => M::CrystalShardCluster,
                }
            } else if column.surface_material == M::Grass {
                match (k + p) % 7 {
                    0..=2 => M::AlpineCrystalBloom,
                    3 => M::FrostedMoss,
                    _ => M::HighlandTussock,
                }
            } else if column.slope > 1.19 {
                if n.is_multiple_of(7) {
                    M::GlacierBoulder
                } else {
                    M::HighlandScree
                }
            } else if matches!(column.surface_material, M::Snow | M::SnowCoveredSod)
                || (p >> 16).is_multiple_of(5)
            {
                M::FrostedMoss
            } else {
                match (p + k) % 7 {
                    0 | 1 => M::AlpineCrystalBloom,
                    2 => M::HighlandScree,
                    _ => M::HighlandTussock,
                }
            };
            cells.insert([x, y + 1, z], id as u16);
        }
    }
}

/// Sparse wind-pruned larches and dwarf flowering trees occupy sheltered
/// shoulders. The talus and exposed summit crests stay clear of tall growth.
fn krummholz(h: &mut Habitat<'_>, lo: Tile, key: u32, cells: &mut BTreeMap<Cell, u16>) {
    let phase = key as f64 * (std::f64::consts::TAU / u32::MAX as f64);
    let (ex, ez) = (
        (phase.cos() * 1.41).round() as i64,
        (phase.sin() * 1.41).round() as i64,
    );
    let pink = key & 8 != 0;
    for k in 0..3 {
        let n = key.rotate_left(k * 7);
        let x = lo[0] + 5 + i64::from(n % 22);
        let z = lo[1] + 5 + i64::from((n >> 9) % 22);
        let Some((floor, c)) = h.ground([x, z]) else {
            continue;
        };
        if !matches!(
            c.surface_material,
            M::Grass | M::CrystalGravel | M::SnowCoveredSod
        ) || c.slope > 1.25
            || h.terrain
                .crystal_summit(x as f64 + 0.5, z as f64 + 0.5, c.height + 7.)
            || h.reserved([x, z], 3)
            || cells.contains_key(&[x, floor + 1, z])
        {
            continue;
        }
        let height = if pink {
            3 + i64::from(n % 2)
        } else {
            4 + i64::from(n % 3)
        };
        for y in 1..=height {
            cells.insert([x, floor + y, z], M::SpruceLog as u16);
        }
        if pink {
            for (dx, dz, rise) in [(ex, ez, 0), (-ez, ex, 1), (ez, -ex, 1), (-ex, -ez, 0)] {
                cells.insert([x + dx, floor + height - 1, z + dz], M::SpruceLog as u16);
                let center = [x + dx * 2, floor + height + rise, z + dz * 2];
                for i in -2i64..=2 {
                    for j in -2i64..=2 {
                        if i * i + j * j > 5 {
                            continue;
                        }
                        for dy in 0..=(i * i + j * j < 3) as i64 {
                            let mat = if j == 1 && dy == 1 {
                                M::PinkBlossomLeaves
                            } else {
                                M::LavenderBlossomLeaves
                            };
                            cells
                                .entry([center[0] + i, center[1] + dy, center[2] + j])
                                .or_insert(mat as u16);
                        }
                    }
                }
            }
        } else {
            for dy in 2..=height + 2 {
                let r: i64 = if dy >= height + 2 {
                    0
                } else if dy % 2 == 0 && dy < height {
                    2
                } else {
                    1
                };
                for i in -r..=r {
                    for j in -r..=r {
                        if i * i + j * j > r * r + 1 || i == 0 && j == 0 && dy <= height {
                            continue;
                        }
                        let mat = if c.height > 90. && (dy >= height || (i + j).rem_euclid(3) == 1)
                        {
                            M::SnowySpruceLeaves
                        } else {
                            M::Leaves
                        };
                        cells
                            .entry([x + i, floor + dy, z + j])
                            .or_insert(mat as u16);
                    }
                }
            }
        }
    }
}

fn colony(
    h: &mut Habitat<'_>,
    cells: &mut BTreeMap<Cell, u16>,
    objects: &mut Vec<crate::generation::structures::StructureObject>,
    center: Tile,
    ground: i64,
    base: f64,
    seed: u32,
) {
    let cyan = !(seed >> 8).is_multiple_of(4);
    let radius = if base > 7. { 5 } else { 3 };
    for dx in -radius..=radius {
        for dz in -radius..=radius {
            let r = ((dx * dx + dz * dz) as f64).sqrt() / radius as f64;
            if r > 1.13 {
                continue;
            }
            let x = center[0] + dx;
            let z = center[1] + dz;
            let col = h.column([x, z]);
            let floor = (col.height - 0.5).floor() as i64;
            let top = ground + ((1. - r).max(0.) * radius as f64 * 0.7).round() as i64;
            if floor + 5 < top || floor > top + 2 {
                continue;
            }
            for y in floor..=top {
                cells.insert(
                    [x, y, z],
                    if r < 0.8 {
                        M::CrystalOre
                    } else {
                        M::HighlandCliffStone
                    } as u16,
                );
            }
        }
    }
    // Broken, stepped ore benches beneath the crystals expose seams on
    // vertical faces. Each column grows from its own measured ground level.
    if base > 7. {
        let side = if seed & 32 == 0 { -1 } else { 1 };
        for i in -3_i64..=3 {
            for j in 0..3_i64 {
                let x = center[0] + side * (4 + j);
                let z = center[1] + i;
                let c = h.column([x, z]);
                let floor = (c.height - 0.5).floor() as i64;
                let stride = (j + i.abs() / 2).min(3);
                let ledge = ground + 3 - stride;
                if !(floor..=floor + 4).contains(&ledge) {
                    continue;
                }
                for y in floor..=ledge {
                    let mineral = if y == ledge && j > 0 && i.rem_euclid(3) == 0 {
                        if c.height > 96. || c.surface_material == M::Snow {
                            M::Snow
                        } else {
                            M::CrystalGravel
                        }
                    } else if (y + i / 2 - j).rem_euclid(3) == 0 {
                        M::CrystalOre
                    } else {
                        M::HighlandCliffStone
                    };
                    cells.insert([x, y, z], mineral as u16);
                }
            }
        }
    }
    let phi = (seed % 360) as f64 * std::f64::consts::PI / 180.;
    // Axes remain upright like natural quartz; each oblique termination shares
    // six uninterrupted faces across all editable slices of the mother crystal.
    let mut spike = |x: i64, z: i64, height: i64, massive: bool, blue: bool| {
        let col = h.column([x, z]);
        let y = (col.height - 0.5).floor() as i64;
        let id = match (massive, blue) {
            (true, false) => M::MassiveAmethyst,
            (true, true) => M::MassiveAquamarine,
            (false, false) => M::AmethystLance,
            (false, true) => M::AquamarineLance,
        };
        let width = terra_core::quartz_prisms::diameter(id as u16);
        for iz in 0..width {
            for ix in 0..width {
                let facing = terra_core::quartz_prisms::code(id as u16, ix, iz, 0);
                if terra_core::quartz_prisms::slice(id as u16, facing).is_none() {
                    continue;
                }
                let xx = x + i64::from(ix) - i64::from(width / 2);
                let zz = z + i64::from(iz) - i64::from(width / 2);
                let floor = (h.column([xx, zz]).height - 0.5).floor() as i64;
                for yy in floor + 1..y {
                    cells
                        .entry([xx, yy, zz])
                        .or_insert(M::HighlandCliffStone as u16);
                }
            }
        }
        quartz(cells, objects, [x, y, z], height, id);
    };
    spike(center[0], center[1], base.round() as i64, base >= 8., cyan);
    let count = if base > 7. { 4 } else { 3 };
    for i in 0..count {
        let a = phi + std::f64::consts::TAU * i as f64 / count as f64;
        let distance = if base > 7. {
            4.2 + i as f64 * 0.28
        } else {
            3.5
        };
        let size = (base * (0.37 + 0.085 * f64::from(seed.rotate_left(i * 5) % 5))).round() as i64;
        spike(
            center[0] + (a.cos() * distance).round() as i64,
            center[1] + (a.sin() * distance).round() as i64,
            size.max(4),
            false,
            if i == 2 { !cyan } else { cyan },
        );
    }
    // Small chips follow one broken seam in the host rock. Leave the spaces
    // between the large points open so the whole outcrop reads as one landmark.
    let seam = phi + 0.4;
    let exposures = if base > 7. { 5 } else { 3 };
    for i in 0..exposures {
        let offset = 2. * f64::from(i) - f64::from(exposures - 1);
        let x = center[0] + (seam.cos() * offset - seam.sin() * radius as f64).round() as i64;
        let z = center[1] + (seam.sin() * offset + seam.cos() * radius as f64).round() as i64;
        let Some((floor, _)) = h.ground([x, z]) else {
            continue;
        };
        let surface = (floor..=floor + 4)
            .rev()
            .find(|&y| cells.contains_key(&[x, y, z]))
            .unwrap_or(floor);
        if cells
            .get(&[x, surface, z])
            .is_some_and(|id| M::from_raw(*id).is_none_or(|host| !host.is_opaque()))
        {
            continue;
        }
        let vein = match i {
            0 => M::MixedCrystalVein,
            1 => M::HighlandCrystalShards,
            2 => M::CrystalShardCluster,
            _ => M::HighlandScree,
        };
        cells.entry([x, surface + 1, z]).or_insert(vein as u16);
    }
}
fn quartz(
    cells: &mut BTreeMap<Cell, u16>,
    objects: &mut Vec<crate::generation::structures::StructureObject>,
    base: Cell,
    height: i64,
    material: M,
) {
    use terra_core::quartz_prisms::{code, diameter, head, slice};
    let id = material as u16;
    let d = diameter(id);
    let point = i64::from(head(id));
    let shoulder = height.max(point + 1) - point;
    for dy in 0..shoulder + point {
        let layer = (dy - shoulder + 1).max(0) as u8;
        for z in 0..d {
            for x in 0..d {
                let facing = code(id, x, z, layer);
                if slice(id, facing).is_none() {
                    continue;
                }
                let p = [
                    base[0] + i64::from(x) - i64::from(d / 2),
                    base[1] + dy,
                    base[2] + i64::from(z) - i64::from(d / 2),
                ];
                if cells
                    .get(&p)
                    .is_some_and(|old| terra_core::quartz_prisms::is_quartz(*old))
                {
                    continue;
                }
                cells.insert(p, id);
                objects.push(crate::generation::structures::StructureObject {
                    position: p,
                    material: id,
                    state: Some(crate::edits::BlockObjectState {
                        facing,
                        ..Default::default()
                    }),
                    loot: None,
                    crop: None,
                });
            }
        }
    }
}

/// Natural flysch bridge with a walkable lintel and two grounded abutments.
fn arch(h: &mut Habitat<'_>, cells: &mut BTreeMap<Cell, u16>, p: Tile, seed: u32) {
    let along_x = seed & 1 == 0;
    let loc = |u: i64, v: i64| {
        if along_x {
            [p[0] + u, p[1] + v]
        } else {
            [p[0] + v, p[1] + u]
        }
    };
    let span = 8 + i64::from((seed >> 7) % 4);
    let end = [loc(-span, 0), loc(span, 0)].map(|xz| (h.column(xz).height - 0.5).floor() as i64);
    if (end[0] - end[1]).abs() > 7 {
        return;
    }
    let spring = end[0].max(end[1]) + 3;
    for u in -span..=span {
        let t = u as f64 / span as f64;
        let soffit = spring + ((1. - t * t).sqrt() * 6.5).round() as i64;
        for v in -1..=1 {
            let [x, z] = loc(u, v);
            let floor = (h.column([x, z]).height - 0.5).floor() as i64;
            let bottom = if u.abs() >= span - 1 { floor } else { soffit };
            for y in bottom..=soffit + 2 {
                cells.insert(
                    [x, y, z],
                    if y == soffit + 2 && (u + v) % 4 == 0 {
                        M::CrystalGravel
                    } else {
                        M::HighlandCliffStone
                    } as u16,
                );
            }
        }
    }
}

/// A hanging spring feeds a narrow notch in a steep downhill ledge. Only the
/// two source cells persist; gravity and the fluid solver carry water over the lip.
pub(super) fn glacial_spring(
    h: &mut Habitat<'_>,
    tile: Tile,
    cells: &mut BTreeMap<Cell, u16>,
    objects: &mut Vec<crate::generation::structures::StructureObject>,
    streams: &mut Vec<(Cell, crate::water::falls::Fall)>,
) {
    use crate::generation::structures::StructureObject;
    let cfg = h.terrain.config();
    if !cfg.water || !ticket(cfg.seed.0, tile, 0x1ce0_4402).is_multiple_of(9) {
        return;
    }
    let center = tile.map(|n| n * TILE_SIZE + 16);
    if h.terrain
        .climate_biome_at(center[0] as f64, center[1] as f64)
        .weights
        .crystal_highlands
        < 0.72
        || h.reserved(center, 8)
    {
        return;
    }
    let mut slope = (0., [0, 0]);
    for [dx, dz] in [[1, 0], [-1, 0], [0, 1], [0, -1]] {
        let high = h.column([center[0] - dx * 6, center[1] - dz * 6]);
        let low = h.column([center[0] + dx * 6, center[1] + dz * 6]);
        let drop = high.height - low.height;
        if drop > slope.0 && high.water_level.is_none() && !high.cave.open_to_sky() {
            slope = (drop, [dx, dz]);
        }
    }
    if !(6.0..=24.).contains(&slope.0) {
        return;
    }
    let [dx, dz] = slope.1;
    let position = |u: i64, v: i64| [center[0] + dx * u - dz * v, center[1] + dz * u + dx * v];
    if cells.iter().any(|(p, m)| {
        let along = (p[0] - center[0]) * dx + (p[2] - center[1]) * dz;
        let across = (p[2] - center[1]) * dx - (p[0] - center[0]) * dz;
        (-6..=5).contains(&along) && (-1..=2).contains(&across) && !matches!(*m, 750..=758)
    }) {
        return;
    }
    let mut bed = i64::MAX;
    let mut source_y = 0;
    let mut route = 0_u64;
    for u in -6..=5 {
        let [x, z] = position(u, 0);
        let natural = (h.column([x, z]).height - 0.5).floor() as i64;
        let prior = bed;
        bed = bed.min(natural - 1);
        if u == -6 {
            source_y = bed + 1;
        } else {
            route |= ((source_y - bed - 1).clamp(0, 31) as u64) << (5 * (u + 5));
        }
        for v in -1..=2 {
            let [x, z] = position(u, v);
            let original = (h.column([x, z]).height - 0.5).floor() as i64;
            if v == 0 || v == 1 {
                let head = if u == -6 { bed + 1 } else { prior + 1 };
                let mut span = crate::water::falls::Fall::new(
                    bed + 1,
                    head,
                    source_y,
                    [(-dx * (u + 6)) as i8, (-dz * (u + 6)) as i8],
                );
                span.route = route;
                span.authored = true;
                streams.push(([x, bed + 1, z], span));
                for y in bed + 1..=original + 1 {
                    cells.insert([x, y, z], M::Air as u16);
                }
                for y in original.min(bed)..=bed {
                    cells.insert([x, y, z], M::HighlandCliffStone as u16);
                }
                if u == -6 {
                    let p = [x, bed + 1, z];
                    cells.insert(p, M::FluidWater as u16);
                    objects.push(StructureObject {
                        position: p,
                        material: M::FluidWater as u16,
                        state: None,
                        loot: None,
                        crop: None,
                    });
                }
            } else if original < bed + 1 {
                for y in original..=bed + 1 {
                    cells.insert([x, y, z], M::HighlandCliffStone as u16);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SectionGenerator, TerraGenerator};
    use std::collections::BTreeSet;
    use terra_core::WorldConfig;

    #[test]
    fn mother_crystal_has_a_compact_stepped_tip_and_connected_shaft() {
        let mut cells = BTreeMap::new();
        let mut objects = Vec::new();
        quartz(&mut cells, &mut objects, [0, 0, 0], 16, M::MassiveAmethyst);
        assert_eq!(objects.len(), cells.len());
        assert!(
            objects
                .iter()
                .all(|o| o.state.unwrap().valid_for(o.position, o.material))
        );
        let crown: Vec<_> = objects.iter().filter(|o| o.position[1] == 15).collect();
        let mut cap_min = bevy_math::Vec3::splat(f32::INFINITY);
        let mut cap_max = bevy_math::Vec3::splat(f32::NEG_INFINITY);
        for o in crown {
            let shape =
                terra_core::quartz_prisms::slice(o.material, o.state.unwrap().facing).unwrap();
            for face in &shape.faces {
                assert_eq!(face.normal.abs().element_sum(), 1.);
                for &p in &face.vertices {
                    assert_eq!(p * 4., (p * 4.).round());
                    if p.y == 1. {
                        let world = p + bevy_math::Vec3::from_array(o.position.map(|n| n as f32));
                        cap_min = cap_min.min(world);
                        cap_max = cap_max.max(world);
                    }
                }
            }
        }
        let cap = cap_max - cap_min;
        assert!(
            cap.x > 0. && cap.x <= 0.75 && cap.z > 0. && cap.z <= 0.75,
            "{cap:?}"
        );
        let mut frontier = vec![[0, 0, 0]];
        while let Some(p) = frontier.pop() {
            if cells.remove(&p).is_none() {
                continue;
            }
            for axis in 0..3 {
                for dy in [-1, 1] {
                    let mut next = p;
                    next[axis] += dy;
                    frontier.push(next);
                }
            }
        }
        assert!(cells.is_empty());
    }

    #[test]
    fn exposed_highland_summits_only_carry_rock_snow_and_minerals() {
        let terrain = TerraGenerator::new(WorldConfig::default()).unwrap();
        let mut bare = 0;
        for z in (0..640).step_by(13) {
            for x in (-1536..-1120).step_by(13) {
                let c = terrain.surface_column(x as f64 + 0.5, z as f64 + 0.5);
                if c.biome.weights.crystal_highlands > 0.65
                    && terrain.crystal_summit(x as f64 + 0.5, z as f64 + 0.5, c.height)
                    && c.water_level.is_none()
                {
                    assert!(matches!(
                        c.surface_material,
                        M::HighlandCliffStone | M::Snow
                    ));
                    bare += 1;
                }
            }
        }
        assert!(bare > 100);
    }

    #[test]
    fn quartz_colonies_grow_on_real_cold_uplands_and_leave_revision_twenty_intact() {
        let terrain = TerraGenerator::new(WorldConfig::default()).unwrap();
        let earlier = TerraGenerator::new(WorldConfig {
            ecology_revision: 20,
            ..Default::default()
        })
        .unwrap();
        let generator = SectionGenerator::new(terrain.clone());
        let mut forms = BTreeSet::new();
        let mut tip = None;
        for tz in 8..=12 {
            for tx in -44..=-40 {
                let tile = [tx, tz];
                let [x, z] = tile.map(|v| v * TILE_SIZE + 16);
                assert_eq!(
                    earlier
                        .biome_at(x as f64, z as f64)
                        .weights
                        .crystal_highlands,
                    0.
                );
                let patch = terrain.natural_patch(tile);
                for &(p, mat) in &patch.cells {
                    if (739..=758).contains(&mat) || terra_core::quartz_prisms::is_quartz(mat) {
                        forms.insert(mat);
                    }
                    if terra_core::quartz_prisms::is_quartz(mat) {
                        let ground = terrain.surface_height(p[0] as f64 + 0.5, p[2] as f64 + 0.5);
                        if p[1] as f64 > ground + 6. {
                            tip = Some((p, mat));
                        }
                    }
                }
            }
        }
        let (p, mat) =
            tip.expect("an exposed quartz needle at least six metres above its host rock");
        assert_eq!(generator.block_at(p).unwrap(), mat);
        for m in [
            M::MassiveAmethyst,
            M::AquamarineLance,
            M::CrystalShardCluster,
            M::AlpineCrystalBloom,
            M::HighlandTussock,
            M::FrostedMoss,
        ] {
            assert!(
                forms.contains(&(m as u16)),
                "missing {m:?} in the quartz province"
            );
        }
    }
}
