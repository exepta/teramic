//! Farm animals share the eight-part gait and blink rig. Each species has its
//! own shoulder, haunches and skull; all surface detail is baked into these meshes.
use super::{Kind, Part, eyes, shape, voxel::Builder};
use bevy::prelude::*;

mod cow;
mod pig;
mod sheep;

type Tint = [f32; 3];
type Parts = Vec<(Part, Vec3, Mesh)>;

pub(super) fn parts(kind: Kind) -> Result<Parts, String> {
    match kind {
        Kind::Cow => cow::parts(),
        Kind::Sheep => sheep::parts(),
        Kind::Pig => pig::parts(),
        _ => Err("Expected a farm animal".into()),
    }
}

fn mass(b: &mut Builder, center: [f32; 3], radius: [f32; 3], color: Tint) -> Result<(), String> {
    b.ellipsoid(center.into(), radius.into(), color)
}

/// Keep a flat bony orbit under each eye as the face slopes into the muzzle.
fn eye_sockets(head: &mut Builder, eye: [f32; 2], skin: Tint) -> Result<(), String> {
    for side in [-1., 1.] {
        head.rounded(
            [side * (eye[0] - 0.045), 0.105, eye[1]],
            [0.108, 0.138, 0.151],
            0.019,
            skin,
        )?;
    }
    Ok(())
}

struct Rig {
    head: Vec3,
    eye: [f32; 2], // lateral offset, depth; all three use the same eyelid height
    tail: Vec3,
    hip_height: f32,
    leg_x: f32,
    front_z: f32,
    rear_z: f32,
    thigh_width: f32,
    hoof_width: f32,
    leg_coat: Tint,
    pastern: Tint,
    hoof: Tint,
}

impl Rig {
    fn assemble(self, body: Mesh, head: Mesh, skin: Tint, tail: Mesh) -> Result<Parts, String> {
        let eyes = [-1., 1.].map(|side| eyes::Eye {
            center: Vec3::new(side * self.eye[0], 0.105, self.eye[1]),
            width: 0.10,
            side,
            skin,
        });
        let (head, lids) = eyes::attach(head, &eyes)?;
        let mut parts = vec![
            (Part::Body, Vec3::ZERO, body),
            (Part::Head, self.head, head),
            (Part::Eyelids, self.head, lids),
        ];
        for (hind, z) in [(false, self.front_z), (true, self.rear_z)] {
            let leg = self.leg(hind)?;
            for (side, mesh) in [(-1., leg.clone()), (1., leg)] {
                parts.push((
                    Part::Leg(if hind { side } else { -side }),
                    Vec3::new(side * self.leg_x, self.hip_height, z),
                    mesh,
                ));
            }
        }
        parts.push((Part::Tail, self.tail, tail));
        Ok(parts)
    }

    fn leg(&self, hind: bool) -> Result<Mesh, String> {
        let mut limb = Builder::with_resolution(16);
        let h = self.hip_height;
        let w = self.thigh_width * if hind { 1.12 } else { 0.91 };
        let hoof = self.hoof_width;
        // Hind legs descend forward to the stifle, then back to the hock.
        let knee_z = if hind { -w * 0.27 } else { -w * 0.08 };
        let hock_z = if hind { w * 0.20 } else { 0.0 };
        mass(
            &mut limb,
            [0., 0.035, 0.],
            [w * 0.55, h * 0.40, w * 0.65],
            self.leg_coat,
        )?;
        shape::segment(
            &mut limb,
            Vec3::new(0., 0.01, 0.),
            Vec3::new(0., -h * 0.34, knee_z),
            w * 0.78,
            w * 0.88,
            self.leg_coat,
        )?;
        shape::segment(
            &mut limb,
            Vec3::new(0., -h * 0.32, knee_z),
            Vec3::new(0., -h * 0.60, hock_z),
            hoof * 0.78,
            hoof * 0.81,
            self.pastern,
        )?;
        shape::segment(
            &mut limb,
            Vec3::new(0., -h * 0.55, hock_z),
            Vec3::new(0., -h * 0.89, -0.012),
            hoof * 0.61,
            hoof * 0.71,
            self.pastern,
        )?;
        // The hoof rests on the animation ground plane at full extension.
        for side in [-1., 1.] {
            limb.rounded(
                [side * hoof * 0.27, -h + hoof * 0.265, -0.028],
                [hoof * 0.45, hoof * 0.53, hoof * 1.15],
                hoof * 0.07,
                self.hoof,
            )?;
        }
        limb.finish()
    }
}
