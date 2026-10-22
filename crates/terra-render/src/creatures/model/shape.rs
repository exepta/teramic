use crate::players::model::Builder;
use bevy::prelude::*;

/// Closed chamfered cuboid. Thirty quads keep bevels independent of body size.
pub(super) fn rounded(
    mesh: &mut Builder,
    center: [f32; 3],
    size: [f32; 3],
    bevel: f32,
    color: [f32; 3],
) -> Result<(), String> {
    let half = Vec3::from_array(size) * 0.5;
    let bevel = bevel.min(half.min_element() * 0.75);
    let center = Vec3::from_array(center);
    let ring = |y: f32, inset: f32| {
        let x = half.x - inset;
        let z = half.z - inset;
        let cut = (bevel - inset * 0.5).min(x.min(z) * 0.8);
        [
            (-x + cut, -z),
            (x - cut, -z),
            (x, -z + cut),
            (x, z - cut),
            (x - cut, z),
            (-x + cut, z),
            (-x, z - cut),
            (-x, -z + cut),
        ]
        .map(|(x, z)| center + Vec3::new(x, y, z))
    };
    let rings = [
        ring(-half.y, bevel * 0.5),
        ring(-half.y + bevel, 0.0),
        ring(half.y - bevel, 0.0),
        ring(half.y, bevel * 0.5),
    ];
    for pair in rings.windows(2) {
        for side in 0..8 {
            let next = (side + 1) % 8;
            mesh.quad(
                [pair[0][next], pair[0][side], pair[1][side], pair[1][next]],
                color,
            )?;
        }
    }
    for (ring, top) in [(rings[0], false), (rings[3], true)] {
        for ids in [[0, 1, 2, 3], [0, 3, 4, 7], [7, 4, 5, 6]] {
            let mut points = ids.map(|i| ring[i]);
            if top {
                points.reverse();
            }
            mesh.quad(points, color)?;
        }
    }
    Ok(())
}

pub(super) fn segment(
    mesh: &mut Builder,
    a: Vec3,
    b: Vec3,
    width: f32,
    depth: f32,
    color: [f32; 3],
) -> Result<(), String> {
    let delta = b - a;
    mesh.rotated(
        ((a + b) * 0.5).to_array(),
        [width, delta.length(), depth],
        color,
        Quat::from_rotation_arc(Vec3::Y, delta.normalize_or_zero()),
    )
}
