//! Bounded perception over the same edited object shapes used by movement.
use super::{Habitat, distance};

const MAX_RANGE: f64 = 96.0;
const MAX_CELLS: usize = 384;
const TARGET_PROBES: usize = 8;

pub(super) fn visible(h: &Habitat, from: [f64; 3], to: [f64; 3]) -> bool {
    if !super::navigation::valid_point(from) || !super::navigation::valid_point(to) {
        return false;
    }
    let length = distance(from, to).sqrt();
    if length > MAX_RANGE {
        return false;
    }
    let direction = std::array::from_fn::<_, 3, _>(|i| (to[i] - from[i]) / length.max(1e-12));
    let mut cell = from.map(|v| v.floor() as i64);
    let end = to.map(|v| v.floor() as i64);
    let step = direction.map(|v| {
        if v > 0.0 {
            1
        } else if v < 0.0 {
            -1
        } else {
            0
        }
    });
    let delta = direction.map(|v| {
        if v.abs() > 1e-12 {
            1.0 / v.abs()
        } else {
            f64::INFINITY
        }
    });
    let mut next = std::array::from_fn::<_, 3, _>(|i| {
        if step[i] == 0 {
            f64::INFINITY
        } else {
            ((cell[i] + i64::from(step[i] > 0)) as f64 - from[i]) / direction[i]
        }
    });
    for _ in 0..MAX_CELLS {
        let Some(material) = h.block(cell) else {
            return false;
        };
        if terra_voxel::MaterialId::from_raw(material).is_none_or(|m| m.is_solid()) {
            let mut blocked = false;
            let known = h.boxes(cell, |shape| {
                blocked |= shape.ray(from, direction, length).is_some();
            });
            if !known || blocked {
                return false;
            }
        }
        // Opened leaves can extend into neighboring air, exactly as their
        // collider's hinge halo does. Seeing through their owner cell does not
        // permit seeing through the actual rotated leaf beside it.
        for [dx, dz] in [[-1, 0], [1, 0], [0, -1], [0, 1]] {
            let neighbor = [cell[0] + dx, cell[1], cell[2] + dz];
            if h.block(neighbor).is_some_and(crate::objects::is_opening) {
                let mut blocked = false;
                h.boxes(neighbor, |shape| {
                    blocked |= shape.ray(from, direction, length).is_some()
                });
                if blocked {
                    return false;
                }
            }
        }
        if cell == end {
            return true;
        }
        // An endpoint exactly on several voxel planes belongs to `end`.
        // Do not step past a finished axis into a wall/floor merely because
        // that plane ties with the last boundary on another axis.
        let axis = (0..3)
            .filter(|&axis| cell[axis] != end[axis])
            .min_by(|&a, &b| next[a].total_cmp(&next[b]))
            .unwrap();
        cell[axis] += step[axis];
        next[axis] += delta[axis];
    }
    false
}

/// Hidden nearer actors never mask a visible enemy. Sorting and ray work have
/// fixed limits, and results keep the original authoritative player index.
pub(super) fn nearest_visible(
    h: &Habitat,
    eye: [f64; 3],
    origin: [f64; 3],
    players: &[[f64; 3]],
    radius: f64,
    accepts: impl Fn([f64; 3]) -> bool,
) -> Option<(usize, [f64; 3])> {
    nearest_visible_at(h, eye, origin, players, radius, accepts, |point| point)
}

/// Test the same anatomical endpoint used by the caller's strike, while keeping
/// player indices and body positions intact for reach and authoritative effects.
#[allow(clippy::too_many_arguments)]
pub(super) fn nearest_visible_at(
    h: &Habitat,
    eye: [f64; 3],
    origin: [f64; 3],
    players: &[[f64; 3]],
    radius: f64,
    accepts: impl Fn([f64; 3]) -> bool,
    sight_point: impl Fn([f64; 3]) -> [f64; 3],
) -> Option<(usize, [f64; 3])> {
    let mut candidates: Vec<_> = players
        .iter()
        .take(128)
        .enumerate()
        .filter(|(_, p)| {
            super::navigation::valid_point(**p)
                && distance(origin, **p) <= radius.min(MAX_RANGE).powi(2)
                && accepts(**p)
        })
        .collect();
    candidates.sort_unstable_by(|(ai, a), (bi, b)| {
        distance(origin, **a)
            .total_cmp(&distance(origin, **b))
            .then(ai.cmp(bi))
    });
    candidates
        .into_iter()
        .take(TARGET_PROBES)
        .find(|(_, point)| visible(h, eye, sight_point(**point)))
        .map(|(index, point)| (index, *point))
}

#[cfg(test)]
mod tests;
