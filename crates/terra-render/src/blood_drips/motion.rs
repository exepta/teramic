use super::*;

pub(super) fn hash(cell: [i64; 3]) -> u64 {
    let mut state = 0x846c_a39d_17be_0231_u64;
    for value in cell {
        state ^= value as u64;
        state = state.wrapping_mul(0xbf58_476d_1ce4_e5b9);
        state ^= state >> 27;
    }
    state
}

pub(super) fn pose(source: Source, time: f32) -> Option<(Transform, bool)> {
    let variation = (source.seed % 65536) as f32 / 65535.0;
    let cycle = 5.0 + variation * 4.0;
    let phase = (time + variation * cycle).rem_euclid(cycle);
    let growth = 0.55;
    let height = (source.origin.y - 0.085 - source.impact - 0.05).max(0.01);
    let fall = (height * 2.0 / 12.0).sqrt();
    let splash = 0.42;
    if phase >= growth + fall + splash {
        return None;
    }
    let mut point = source.origin;
    let scale;
    let landed = phase >= growth + fall;
    if phase < growth {
        let q = phase / growth;
        let grow = q * q * (3.0 - 2.0 * q);
        scale = Vec3::new(0.048, 0.085, 0.048) * grow.max(0.001);
        point.y -= scale.y;
    } else if !landed {
        let t = phase - growth;
        let q = (t / fall).clamp(0.0, 1.0);
        point.y -= 0.085 + 6.0 * t * t;
        // No side displacement at release/impact: both joins stay continuous.
        point.x += (q * std::f32::consts::PI).sin() * (variation - 0.5) * 0.036;
        scale = Vec3::new(
            0.048 - 0.003 * q,
            0.085 - 0.035 * q + 0.06 * (q * std::f32::consts::PI).sin(),
            0.048 - 0.003 * q,
        );
    } else {
        let q = ((phase - growth - fall) / splash).clamp(0.0, 1.0);
        let shrink = 1.0 - q * q;
        let radius = (0.045 + 0.18 * q) * shrink;
        scale = Vec3::new(radius, 0.05 * (1.0 - q).max(0.001), radius);
        point.y = source.impact + scale.y;
    }
    Some((Transform::from_translation(point).with_scale(scale), landed))
}
