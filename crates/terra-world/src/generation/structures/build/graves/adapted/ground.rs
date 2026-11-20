use super::*;

/// The median balances a small cut with a small fill. A bad site is rejected,
/// rather than concealed beneath an arbitrarily tall retaining cube.
pub(in crate::generation::structures::build) fn fitted_floor(
    b: &Builder<'_>,
    min: [i64; 2],
    max: [i64; 2],
    limit: i64,
) -> Option<i64> {
    let mut heights = Vec::new();
    for z in min[1]..=max[1] {
        for x in min[0]..=max[0] {
            heights.push(b.ground(x, z));
        }
    }
    heights.sort_unstable();
    let floor = heights[heights.len() / 2];
    (heights.iter().all(|height| (height - floor).abs() <= limit)).then_some(floor)
}

pub(super) fn foundation(
    b: &mut Builder<'_>,
    min: [i64; 3],
    max: [i64; 3],
    limit: i64,
) -> Option<()> {
    for z in min[2]..=max[2] {
        for x in min[0]..=max[0] {
            if (b.ground(x, z) - min[1]).abs() > limit || !supported(b, x, z, min[1]) {
                return None;
            }
        }
    }
    for z in min[2]..=max[2] {
        for x in min[0]..=max[0] {
            let natural = b.ground(x, z);
            b.fill([x, natural.min(min[1]) - 1, z], [x, min[1] - 1, z], MOSS);
            b.fill([x, min[1], z], [x, max[1].max(natural + 1), z], 0);
        }
    }
    b.summary.buildings.push(BuildingSummary {
        role: BuildingRole::Ruin,
        entrance: [(min[0] + max[0]) / 2, min[1], min[2]],
        bounds: [min, max],
    });
    Some(())
}

pub(super) fn perimeter(b: &mut Builder<'_>) {
    let [x, _, z] = b.summary.center;
    for d in -RADIUS..=RADIUS {
        for [dx, dz] in [[d, -RADIUS], [d, RADIUS], [-RADIUS, d], [RADIUS, d]] {
            if dz == -RADIUS && dx.abs() <= 3 {
                continue;
            }
            let p = [x + dx, b.ground(x + dx, z + dz), z + dz];
            b.set([p[0], p[1] - 1, p[2]], MOSS);
            b.set(p, if d.rem_euclid(6) == 0 { STONE } else { 391 });
            if d.rem_euclid(6) == 0 {
                b.set([p[0], p[1] + 1, p[2]], CAPITAL);
                b.set([p[0], p[1] + 2, p[2]], LANTERN);
            }
        }
    }
}

/// Three-wide local path, no more than two metres of earthwork. Heights are
/// projected from both endpoints; steep natural ridges reject the connection.
pub(in crate::generation::structures::build) fn path(
    b: &mut Builder<'_>,
    start: [i64; 3],
    end: [i64; 3],
) -> Option<Vec<[i64; 3]>> {
    let mut cells = vec![start];
    let mut cursor = start;
    for axis in [2, 0] {
        while cursor[axis] != end[axis] {
            cursor[axis] += (end[axis] - cursor[axis]).signum();
            cursor[1] = b.ground(cursor[0], cursor[2]);
            cells.push(cursor);
        }
    }
    let last = cells.len() - 1;
    cells[last][1] = end[1];
    for i in 1..cells.len() {
        cells[i][1] = cells[i][1].clamp(cells[i - 1][1] - 1, cells[i - 1][1] + 1);
    }
    if cells[last][1] != end[1] {
        return None;
    }
    for i in (0..last).rev() {
        cells[i][1] = cells[i][1].clamp(cells[i + 1][1] - 1, cells[i + 1][1] + 1);
    }
    if cells[0][1] != start[1] {
        return None;
    }
    // Resolve overlapping path shoulders before writing. A later low cell must
    // never erase the adjacent high tread or fill its headroom back in.
    let mut footprint = BTreeMap::<[i64; 2], (i64, i64)>::new();
    for p in &cells {
        for dx in -1_i64..=1 {
            for dz in -1_i64..=1 {
                let distance = dx.abs() + dz.abs();
                let entry = footprint
                    .entry([p[0] + dx, p[2] + dz])
                    .or_insert((distance, p[1]));
                if distance < entry.0 {
                    *entry = (distance, p[1]);
                }
            }
        }
    }
    if footprint
        .iter()
        .any(|(&[x, z], &(_, feet))| (b.ground(x, z) - feet).abs() > 2 || !supported(b, x, z, feet))
    {
        return None;
    }
    for (&[x, z], &(_, feet)) in &footprint {
        let ground = b.ground(x, z);
        b.fill([x, ground.min(feet) - 1, z], [x, feet - 1, z], MOSS);
        b.fill([x, feet, z], [x, feet + 3, z], 0);
    }
    for pair in cells.windows(2) {
        let (high, low) = if pair[0][1] > pair[1][1] {
            (pair[0], pair[1])
        } else {
            (pair[1], pair[0])
        };
        if high[1] == low[1] {
            continue;
        }
        let dx = high[0] - low[0];
        let dz = high[2] - low[2];
        let facing = match (dx, dz) {
            (0, 1) => 0,
            (-1, 0) => 1,
            (0, -1) => 2,
            _ => 3,
        };
        for side in -1..=1 {
            b.state(
                [
                    high[0] + if dz != 0 { side } else { 0 },
                    high[1] - 1,
                    high[2] + if dx != 0 { side } else { 0 },
                ],
                77,
                facing,
            );
        }
    }
    Some(cells)
}

pub(super) fn grave_groups(b: &mut Builder<'_>, tomb: bool) {
    let [x, _, z] = b.summary.center;
    for (row, dz) in [-17, -9, 0, 9, 18].into_iter().enumerate() {
        for (col, dx) in [-20_i64, -13, 13, 20].into_iter().enumerate() {
            if tomb && dx.abs() < 18 && dz < 6 {
                continue;
            }
            let Some(y) = fitted_floor(b, [x + dx - 1, z + dz - 1], [x + dx + 1, z + dz + 3], 1)
            else {
                continue;
            };
            if foundation(
                b,
                [x + dx - 1, y, z + dz - 1],
                [x + dx + 1, y + 3, z + dz + 3],
                1,
            )
            .is_some()
            {
                grave(
                    b,
                    x + dx,
                    y,
                    z + dz,
                    b.seed.wrapping_add((row * 4 + col) as u32),
                );
            }
        }
    }
}

pub(super) fn chapel(b: &mut Builder<'_>) -> Option<()> {
    let [x, _, z] = b.summary.center;
    let y = fitted_floor(b, [x - 5, z + 12], [x + 5, z + 22], 2)?;
    foundation(b, [x - 5, y, z + 12], [x + 5, y + 11, z + 22], 2)?;
    b.fill([x - 5, y, z + 12], [x + 5, y + 6, z + 22], STONE);
    b.fill([x - 4, y, z + 13], [x + 4, y + 5, z + 21], 0);
    b.fill([x - 1, y, z + 12], [x + 1, y + 3, z + 12], 0);
    for dx in -6_i64..=6 {
        b.fill(
            [x + dx, y + 7 + (6 - dx.abs()) / 2, z + 11],
            [x + dx, y + 7 + (6 - dx.abs()) / 2, z + 23],
            DARK,
        );
    }
    b.set([x, y + 11, z + 12], 365);
    b.set([x, y, z + 19], 368);
    let end_y = b.ground(x, z - 31);
    path(b, [x, y, z + 12], [x, end_y, z - 31])?;
    b.summary.center = [x, y, z];
    Some(())
}

fn supported(b: &Builder<'_>, x: i64, z: i64, floor: i64) -> bool {
    let column = b.terrain.surface_column(x as f64 + 0.5, z as f64 + 0.5);
    column.water_level.is_none_or(|water| water <= floor as f64)
        && crate::generation::structures::build::foundation_supported(
            b.terrain, x, z, floor, column,
        )
}
