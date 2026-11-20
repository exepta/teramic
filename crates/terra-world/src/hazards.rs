//! Shared geometric contact for real cactus thorns. No terrain is requested here.
pub const CACTUS: u16 = 422;
pub const THORN_REACH: f64 = 0.16;
pub const CACTUS_DAMAGE: f32 = 3.0;
pub const CACTUS_INTERVAL: f64 = 0.8;

/// A vertical capsule against the thorn envelope of actual loaded cactus cells.
/// Round horizontal corners avoid damaging a body merely near a cube corner.
pub fn cactus_contact(
    center: [f64; 3],
    radius: f64,
    half_height: f64,
    mut block: impl FnMut([i64; 3]) -> Option<u16>,
) -> bool {
    if center.iter().any(|v| !v.is_finite() || v.abs() > 999_000.0)
        || !radius.is_finite()
        || !half_height.is_finite()
        || !(0.01..=4.0).contains(&radius)
        || !(radius..=8.0).contains(&half_height)
    {
        return false;
    }
    let reach = radius + THORN_REACH;
    let low = [
        center[0] - reach,
        center[1] - half_height - THORN_REACH,
        center[2] - reach,
    ];
    let high = [
        center[0] + reach,
        center[1] + half_height + THORN_REACH,
        center[2] + reach,
    ];
    let lower = low.map(|v| v.floor() as i64);
    let upper = high.map(|v| v.floor() as i64);
    let segment_low = center[1] - half_height + radius;
    let segment_high = center[1] + half_height - radius;
    for z in lower[2]..=upper[2] {
        for y in lower[1]..=upper[1] {
            for x in lower[0]..=upper[0] {
                if block([x, y, z]) != Some(CACTUS) {
                    continue;
                }
                let dx = (x as f64 - center[0])
                    .max(0.0)
                    .max(center[0] - (x + 1) as f64);
                let dz = (z as f64 - center[2])
                    .max(0.0)
                    .max(center[2] - (z + 1) as f64);
                // Bottom faces have no spines; top faces extend by the same
                // maximum as visible tips. The capsule's rounded end is real.
                let dy = (y as f64 - segment_high)
                    .max(0.0)
                    .max(segment_low - ((y + 1) as f64 + THORN_REACH));
                let horizontal = (dx.hypot(dz) - THORN_REACH).max(0.0);
                if horizontal * horizontal + dy * dy <= radius * radius {
                    return true;
                }
            }
        }
    }
    false
}

/// Oriented anatomical boxes avoid the empty corners of a rotated long mob's
/// broad-phase box. The same published thorn reach is used by the renderer.
pub fn cactus_hitbox_contact(
    body: terra_core::creatures::hitboxes::Hitbox,
    mut block: impl FnMut([i64; 3]) -> Option<u16>,
) -> bool {
    if body
        .center
        .iter()
        .any(|p| !p.is_finite() || p.abs() > 999_000.0)
        || body
            .size
            .iter()
            .any(|p| !p.is_finite() || !(0.0..=16.0).contains(p))
    {
        return false;
    }
    let corners = body.corners();
    let low = std::array::from_fn::<_, 3, _>(|i| {
        (corners.iter().map(|p| p[i]).fold(f64::INFINITY, f64::min) - THORN_REACH).floor() as i64
    });
    let high = std::array::from_fn::<_, 3, _>(|i| {
        (corners
            .iter()
            .map(|p| p[i])
            .fold(f64::NEG_INFINITY, f64::max)
            + THORN_REACH)
            .floor() as i64
    });
    for z in low[2]..=high[2] {
        for y in low[1]..=high[1] {
            for x in low[0]..=high[0] {
                if block([x, y, z]) != Some(CACTUS) {
                    continue;
                }
                let center = [
                    x as f64 + 0.5,
                    y as f64 + 0.5 + THORN_REACH * 0.5,
                    z as f64 + 0.5,
                ];
                let half = [
                    0.5 + THORN_REACH,
                    0.5 + THORN_REACH * 0.5,
                    0.5 + THORN_REACH,
                ];
                let delta = std::array::from_fn::<_, 3, _>(|i| body.center[i] - center[i]);
                let unit = [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]];
                let dot = |a: [f64; 3], b: [f64; 3]| {
                    a.into_iter().zip(b).map(|(a, b)| a * b).sum::<f64>()
                };
                let axes = unit
                    .into_iter()
                    .chain(body.axes)
                    .chain(unit.into_iter().flat_map(|a| {
                        body.axes.map(|b| {
                            [
                                a[1] * b[2] - a[2] * b[1],
                                a[2] * b[0] - a[0] * b[2],
                                a[0] * b[1] - a[1] * b[0],
                            ]
                        })
                    }));
                if axes.filter(|a| dot(*a, *a) > 1e-10).all(|axis| {
                    let a = (0..3).map(|i| half[i] * axis[i].abs()).sum::<f64>();
                    let b = (0..3)
                        .map(|i| body.size[i] * 0.5 * dot(body.axes[i], axis).abs())
                        .sum::<f64>();
                    dot(delta, axis).abs() <= a + b
                }) {
                    return true;
                }
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    fn block(p: [i64; 3]) -> Option<u16> {
        Some(if p == [-1, 0, -1] { CACTUS } else { 0 })
    }
    #[test]
    fn true_capsule_contact_hits_sides_and_top_without_a_nearby_damage_aura() {
        assert!(cactus_contact([0.47, 0.9, -0.5], 0.32, 0.9, block));
        assert!(!cactus_contact([0.50, 0.9, -0.5], 0.32, 0.9, block));
        assert!(!cactus_contact([0.40, 0.9, 0.40], 0.32, 0.9, block));
        assert!(cactus_contact([-0.5, 2.05, -0.5], 0.32, 0.9, block));
        assert!(!cactus_contact([-0.5, 2.09, -0.5], 0.32, 0.9, block));
        assert!(!cactus_contact([0.47, 0.9, -0.5], 0.32, 0.9, |_| None));
        assert!(!cactus_contact([f64::NAN, 0.0, 0.0], 0.32, 0.9, block));
    }

    #[test]
    fn rotated_mob_anatomy_rejects_empty_broad_phase_corners() {
        let c = std::f64::consts::FRAC_1_SQRT_2;
        let mut body = terra_core::creatures::hitboxes::Hitbox {
            center: [0.6, 0.5, 0.6],
            size: [0.1, 0.4, 2.0],
            axes: [[c, 0., c], [0., 1., 0.], [-c, 0., c]],
        };
        assert!(!cactus_hitbox_contact(body, block));
        body.center = [0., 0.5, 0.];
        assert!(cactus_hitbox_contact(body, block));
        assert!(!cactus_hitbox_contact(body, |_| None));
    }
}
