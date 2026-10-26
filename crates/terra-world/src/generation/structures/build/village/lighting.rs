//! Supported street lights alongside the existing routes, never in their lanes.
use super::*;

pub(super) fn build(builder: &mut Builder<'_>, roads: &paths::Routes, occupied: &[[[i64; 2]; 2]]) {
    let mut candidates = std::collections::BTreeSet::new();
    for p in &roads.walk {
        for [dx, dz] in [[-1, 0], [1, 0], [0, -1], [0, 1]] {
            let q = [p[0] + dx, p[1] + dz];
            if roads.walk.contains(&q)
                || occupied
                    .iter()
                    .any(|b| (0..2).all(|i| (b[0][i] - 1..=b[1][i] + 1).contains(&q[i])))
            {
                continue;
            }
            candidates.insert(q);
        }
    }
    let center = builder.summary.center;
    let mut candidates: Vec<_> = candidates.into_iter().collect();
    candidates
        .sort_unstable_by_key(|p| ((p[0] - center[0]).pow(2) + (p[1] - center[2]).pow(2), *p));
    let mut placed = Vec::<[i64; 2]>::with_capacity(20);
    let fence = builder.palette().6;
    for p in candidates {
        if placed
            .iter()
            .any(|q| (p[0] - q[0]).pow(2) + (p[1] - q[1]).pow(2) < 18 * 18)
        {
            continue;
        }
        let column = builder
            .terrain
            .surface_column(p[0] as f64 + 0.5, p[1] as f64 + 0.5);
        let base = (column.height - 0.5).floor() as i64 + 1;
        if column.water_level.is_some_and(|level| level > base as f64)
            || !foundation_supported(builder.terrain, p[0], p[1], base, column)
            || !(base..=base + 3).all(|y| {
                let (owner, [x, y, z]) = global_to_section([p[0], y, p[1]]);
                builder
                    .patches
                    .get(&owner)
                    .is_none_or(|patch| matches!(patch[x + y * 16 + z * 256], 0 | u16::MAX))
            })
        {
            continue;
        }
        builder.fill([p[0], base, p[1]], [p[0], base + 2, p[1]], fence);
        builder.set([p[0], base + 3, p[1]], 40);
        placed.push(p);
        if placed.len() == 20 {
            break;
        }
    }
}
