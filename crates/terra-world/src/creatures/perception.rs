//! Bounded perception over the same edited object shapes used by movement.
use super::{Habitat, distance};

const MAX_RANGE: f64 = 96.0;
const MAX_CELLS: usize = 384;
const TARGET_PROBES: usize = 8;

/// Once an attacker commits its body to a swing, stepping around its shoulder
/// evades that swing. Vertical neighbours can still be in reach.
pub(super) fn in_melee_arc(yaw: f32, from: [f64; 3], target: [f64; 3]) -> bool {
    let dx = target[0] - from[0];
    let dz = target[2] - from[2];
    let (s, c) = f64::from(yaw).sin_cos();
    -s * dx - c * dz >= 0.36 * dx.hypot(dz) - 0.04
}

pub(super) fn visible(h: &Habitat, from: [f64; 3], to: [f64; 3]) -> bool {
    obstruction(h, from, to).is_none()
}

pub(super) fn obstruction(h: &Habitat, from: [f64; 3], to: [f64; 3]) -> Option<[i64; 3]> {
    if !super::navigation::valid_point(from) || !super::navigation::valid_point(to) {
        return Some(from.map(|v| v.floor() as i64));
    }
    let length = distance(from, to).sqrt();
    if length > MAX_RANGE {
        return Some(from.map(|v| v.floor() as i64));
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
            return Some(cell);
        };
        if terra_voxel::MaterialId::from_raw(material).is_none_or(|m| m.is_solid()) {
            let mut blocked = false;
            let known = h.boxes(cell, |shape| {
                blocked |= shape.ray(from, direction, length).is_some();
            });
            if !known || blocked {
                return Some(cell);
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
                    return Some(neighbor);
                }
            }
        }
        if cell == end {
            return None;
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
    Some(cell)
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
    nearest_visible_target(
        h,
        eye,
        origin,
        players.iter().copied().take(128),
        radius,
        accepts,
        |p| p,
        sight_point,
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn nearest_visible_target<T: Copy>(
    h: &Habitat,
    eye: [f64; 3],
    origin: [f64; 3],
    targets: impl Iterator<Item = T>,
    radius: f64,
    accepts: impl Fn(T) -> bool,
    range_point: impl Fn(T) -> [f64; 3],
    sight_point: impl Fn(T) -> [f64; 3],
) -> Option<(usize, T)> {
    let mut candidates: Vec<_> = targets
        .enumerate()
        .filter_map(|(i, target)| {
            let point = range_point(target);
            let distance = distance(origin, point);
            (super::navigation::valid_point(point)
                && distance <= radius.min(MAX_RANGE).powi(2)
                && accepts(target))
            .then_some((i, target, distance))
        })
        .collect();
    candidates.sort_unstable_by(|a, b| a.2.total_cmp(&b.2).then(a.0.cmp(&b.0)));
    candidates
        .into_iter()
        .take(TARGET_PROBES)
        .find(|(_, target, _)| visible(h, eye, sight_point(*target)))
        .map(|(index, target, _)| (index, target))
}

#[cfg(test)]
pub(super) mod tests;
