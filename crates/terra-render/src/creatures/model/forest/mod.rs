//! Shared articulated meshes for crypt guardians and Blood Forest fauna.
//! Shared voxel meshes; only the Blood Slime body uses a blended jelly material.
use super::{Part, shape};
use crate::creatures::model::voxel::Builder;
use bevy::prelude::*;

mod monsters;
mod skeleton;
#[cfg(test)]
mod tests;
mod thorn;
mod wildlife;
pub(super) use monsters::{brute, stalker, wisp};
pub(super) use skeleton::parts as skeleton;
pub(super) use wildlife::{raven, slime};

const BONE: [f32; 3] = [0.80, 0.73, 0.55];
const IVORY: [f32; 3] = [0.96, 0.90, 0.73];
const DARK: [f32; 3] = [0.08, 0.025, 0.075];
const RED: [f32; 3] = [0.76, 0.045, 0.12];
const RUBY: [f32; 3] = [1.0, 0.18, 0.23];

/// Closed faceted crystal, with distinct cap vertices (no collapsed triangles).
fn crystal(
    b: &mut Builder,
    base: Vec3,
    radius: f32,
    height: f32,
    color: [f32; 3],
) -> Result<(), String> {
    let top = base + Vec3::Y * height;
    let bottom = base - Vec3::Y * radius * 0.30;
    for i in 0..6 {
        let angle = i as f32 * std::f32::consts::TAU / 6.0;
        let next = (i + 1) as f32 * std::f32::consts::TAU / 6.0;
        let a = base + Vec3::new(angle.cos() * radius, height * 0.35, angle.sin() * radius);
        let c = base + Vec3::new(next.cos() * radius, height * 0.35, next.sin() * radius);
        b.triangle([a, top, c], color)?;
        b.triangle([c, bottom, a], color.map(|v| v * 0.72))?;
    }
    Ok(())
}

fn eyes(
    b: &mut Builder,
    x: f32,
    y: f32,
    z: f32,
    width: f32,
    color: [f32; 3],
) -> Result<(), String> {
    for side in [-1.0, 1.0] {
        b.voxel([side * x, y, z], [width, width * 1.20, width * 0.35], DARK)?;
        b.voxel(
            [side * x, y, z - width * 0.19],
            [width * 0.45, width * 0.60, width * 0.12],
            color,
        )?;
    }
    Ok(())
}

/// Shared voxel occupancy; no smooth triangle rings or sphere caps are emitted.
pub(super) fn ellipsoid(
    b: &mut Builder,
    center: Vec3,
    radii: Vec3,
    color: [f32; 3],
) -> Result<(), String> {
    b.ellipsoid(center, radii, color)
}
