use crate::creatures::model::voxel::Builder;
use bevy::prelude::*;

/// A painted voxel volume with stepped corners and exact authored outer bounds.
pub(super) fn rounded(
    mesh: &mut Builder,
    center: [f32; 3],
    size: [f32; 3],
    bevel: f32,
    color: [f32; 3],
) -> Result<(), String> {
    mesh.rounded(center, size, bevel, color)
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
