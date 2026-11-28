//! Shared indexed sculptures. Each animated mesh includes its scales, fur markings or claws.
use super::{Kind, Part, shape::segment, voxel::Builder};
use bevy::prelude::*;

mod cats;
mod crocodile;
pub(in crate::creatures) mod snake;

type Parts = Vec<(Part, Vec3, Mesh)>;
type Tint = [f32; 3];

pub(super) fn parts(kind: Kind, variant: u8) -> Result<Parts, String> {
    match kind {
        Kind::Crocodile => crocodile::parts(),
        Kind::Snake => snake::parts(variant),
        _ => cats::parts(kind),
    }
}

fn mass(b: &mut Builder, center: [f32; 3], radii: [f32; 3], tint: Tint) -> Result<(), String> {
    b.ellipsoid(center.into(), radii.into(), tint)
}

pub(in crate::creatures) const CAT_HEAD: Vec3 = Vec3::new(0., 0.911, -0.61);

pub(in crate::creatures) fn cat_scale(kind: Kind) -> Vec3 {
    match kind {
        Kind::Jaguar => Vec3::new(0.89, 0.825, 0.79),
        Kind::Lion => Vec3::new(1.03, 1.04, 0.98),
        _ => Vec3::ONE,
    }
}

pub(in crate::creatures) const CROCODILE_NECK: Vec3 = Vec3::new(0., 0.376, -0.61);
pub(in crate::creatures) const CROCODILE_TAIL: f32 = 0.59;
pub(in crate::creatures) const CROCODILE_TAIL_STEP: f32 = 0.445;
