//! Three cached loadouts share the same skeleton and original joint anchors.
use super::*;
mod limbs;
mod skull;
mod torso;

const STEEL: [f32; 3] = [0.30, 0.33, 0.39];
const EDGE: [f32; 3] = [0.57, 0.61, 0.67];
pub(super) const HEAD: Vec3 = Vec3::new(0.0, 1.64, 0.0);

pub(in crate::creatures::model) fn parts(variant: u8) -> Result<Vec<(Part, Vec3, Mesh)>, String> {
    let (head, pupils) = skull::meshes(variant == 2)?;
    let mut out = vec![
        (Part::Body, Vec3::ZERO, torso::mesh(variant)?),
        (Part::Head, HEAD, head),
        (Part::SkeletonEyes, HEAD, pupils),
    ];
    out.extend(limbs::parts(variant)?);
    Ok(out)
}
