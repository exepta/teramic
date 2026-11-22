//! A metre-scale articulated skeleton. All species share bone attachment points;
//! their armour, horns, membrane colours, crystal fins and tail tips are sculpted separately.
mod anatomy;
mod effects;
mod forms;
mod geometry;
mod spells;
use terra_core::creatures::dragons::{Lineage, lineage};
mod wings;
pub(in crate::creatures) use terra_core::creatures::dragon_pose::Joint;
use terra_core::creatures::dragon_pose::{TAIL_BONES, tail_node, wrist};
pub(in crate::creatures) use terra_core::creatures::dragons::model_scale as scale;

use super::{Kind, Part};
use crate::players::model::Builder;
use bevy::prelude::*;
use geometry::*;

#[derive(Clone, Copy)]
struct Coat {
    kind: Kind,
    variant: u8,
    shadow: Rgb,
    scale: Rgb,
    ridge: Rgb,
    belly: Rgb,
    horn: Rgb,
    wing: Rgb,
    light: Rgb,
    growth: Rgb,
}

impl Coat {
    fn new(kind: Kind, variant: u8) -> Self {
        let (shadow, scale, ridge, belly, horn, wing, light, growth) = match (kind, variant) {
            // Obsidian, granite and tidal coats use the shape vocabulary of their bloodline.
            (Kind::EarthDragon, 2) => (
                [0.18, 0.20, 0.24],
                [0.39, 0.42, 0.46],
                [0.62, 0.64, 0.65],
                [0.73, 0.71, 0.63],
                [0.84, 0.81, 0.73],
                [0.48, 0.42, 0.36],
                [0.94, 0.68, 0.30],
                [0.39, 0.46, 0.32],
            ),
            (Kind::EarthDragon, 3) => (
                [0.12, 0.08, 0.10],
                [0.27, 0.21, 0.24],
                [0.43, 0.29, 0.26],
                [0.55, 0.37, 0.28],
                [0.97, 0.46, 0.18],
                [0.68, 0.20, 0.09],
                [1.0, 0.38, 0.035],
                [0.96, 0.34, 0.06],
            ),
            (Kind::DarkDragon, 3..=5) => {
                let void = variant == 5;
                let slate = if variant == 4 {
                    [0.21, 0.20, 0.27]
                } else {
                    [0.18, 0.12, 0.25]
                };
                (
                    [0.072, 0.045, 0.10],
                    slate,
                    [0.33, 0.24, 0.40],
                    [0.50, 0.41, 0.55],
                    [0.66, 0.54, 0.71],
                    if void {
                        [0.33, 0.08, 0.64]
                    } else {
                        [0.31, 0.13, 0.45]
                    },
                    [0.89, 0.22, 1.0],
                    if void {
                        [0.81, 0.18, 1.]
                    } else {
                        [0.63, 0.28, 0.84]
                    },
                )
            }
            (Kind::WaterDragon, 3) => (
                [0.055, 0.18, 0.34],
                [0.09, 0.40, 0.64],
                [0.26, 0.72, 0.83],
                [0.89, 0.92, 0.80],
                [0.92, 0.95, 0.89],
                [0.18, 0.74, 0.85],
                [0.26, 1., 0.96],
                [0.48, 0.96, 0.98],
            ),
            (Kind::WaterDragon, 4) => (
                [0.09, 0.06, 0.25],
                [0.19, 0.19, 0.47],
                [0.45, 0.32, 0.70],
                [0.72, 0.73, 0.88],
                [0.81, 0.76, 0.94],
                [0.44, 0.30, 0.76],
                [0.75, 0.45, 1.],
                [0.56, 0.65, 0.99],
            ),
            (Kind::WaterDragon, 5) => (
                [0.28, 0.49, 0.63],
                [0.52, 0.76, 0.88],
                [0.81, 0.94, 0.98],
                [0.96, 0.99, 1.],
                [0.93, 1., 1.],
                [0.53, 0.84, 0.96],
                [0.55, 0.98, 1.],
                [0.72, 0.96, 1.],
            ),
            (Kind::EarthDragon, _) => (
                [0.29, 0.17, 0.12],
                [0.61, 0.37, 0.22],
                [0.78, 0.54, 0.32],
                [0.93, 0.77, 0.52],
                [0.97, 0.83, 0.61],
                [0.85, 0.58, 0.28],
                [1.0, 0.71, 0.12],
                [0.48, 0.54, 0.14],
            ),
            (Kind::WaterDragon, 1) => (
                [0.27, 0.48, 0.65],
                [0.69, 0.85, 0.94],
                [0.91, 0.97, 1.0],
                [0.88, 0.98, 1.0],
                [0.99, 0.99, 1.0],
                [0.48, 0.80, 0.98],
                [0.27, 1.0, 1.0],
                [0.56, 0.96, 1.0],
            ),
            (Kind::WaterDragon, 2) => (
                [0.035, 0.10, 0.22],
                [0.075, 0.25, 0.46],
                [0.13, 0.49, 0.66],
                [0.47, 0.80, 0.84],
                [0.43, 0.90, 0.93],
                [0.09, 0.46, 0.64],
                [0.04, 1.0, 0.89],
                [0.12, 0.83, 0.84],
            ),
            (Kind::WaterDragon, _) => (
                [0.10, 0.21, 0.42],
                [0.18, 0.42, 0.74],
                [0.33, 0.64, 0.91],
                [0.95, 0.98, 1.00],
                [0.96, 0.99, 1.0],
                [0.26, 0.66, 0.93],
                [0.21, 0.94, 1.0],
                [0.39, 0.89, 1.0],
            ),
            (Kind::DarkDragon, 2) => (
                [0.10, 0.06, 0.14],
                [0.19, 0.11, 0.26],
                [0.42, 0.27, 0.50],
                [0.71, 0.58, 0.75],
                [0.84, 0.67, 0.89],
                [0.52, 0.10, 0.68],
                [0.95, 0.18, 1.0],
                [0.88, 0.21, 0.99],
            ),
            (Kind::DarkDragon, 1) => (
                [0.15, 0.11, 0.19],
                [0.31, 0.24, 0.35],
                [0.46, 0.40, 0.50],
                [0.78, 0.71, 0.78],
                [0.94, 0.82, 0.78],
                [0.40, 0.20, 0.51],
                [0.78, 0.30, 1.0],
                [0.72, 0.45, 0.83],
            ),
            _ => (
                [0.13, 0.095, 0.17],
                [0.25, 0.18, 0.32],
                [0.40, 0.31, 0.45],
                [0.82, 0.72, 0.84],
                [0.89, 0.77, 0.83],
                [0.48, 0.22, 0.64],
                [0.86, 0.24, 1.0],
                [0.75, 0.34, 0.91],
            ),
        };
        Self {
            kind,
            variant,
            shadow,
            scale,
            ridge,
            belly,
            horn,
            wing,
            light,
            growth,
        }
    }
}

pub(super) fn parts(kind: Kind, variant: u8) -> Result<Vec<(Part, Vec3, Mesh)>, String> {
    let coat = Coat::new(kind, variant);
    let form = lineage(kind, variant);
    let mut joints = vec![
        Joint::Trunk,
        Joint::Neck,
        Joint::Skull,
        Joint::Eyes,
        Joint::Jaw,
    ];
    joints.extend((0..TAIL_BONES).map(Joint::Tail));
    for side in [-1, 1] {
        joints.extend([Joint::WingArm(side), Joint::Sail(side)]);
        for hind in [false, true] {
            joints.extend([
                Joint::Thigh { side, hind },
                Joint::Foot { side, hind },
                Joint::Paw { side, hind },
            ]);
        }
    }
    joints.extend([Joint::Breath, Joint::Impact, Joint::Rune]);
    if form.is_some() {
        joints.extend((0..3).map(Joint::Missile));
        joints.extend([Joint::Ward, Joint::Surge]);
    }
    let mut parts = joints
        .into_iter()
        .map(|joint| {
            let mut mesh = Builder::default();
            match joint {
                Joint::Trunk => anatomy::trunk(&mut mesh, coat)?,
                Joint::Neck => anatomy::neck(&mut mesh, coat)?,
                Joint::Skull | Joint::Eyes | Joint::Jaw => anatomy::head(&mut mesh, coat, joint)?,
                Joint::Tail(i) => anatomy::tail(&mut mesh, coat, i)?,
                Joint::Thigh { side, hind }
                | Joint::Foot { side, hind }
                | Joint::Paw { side, hind } => {
                    anatomy::leg(&mut mesh, coat, joint, f32::from(side), hind)?
                }
                Joint::WingArm(s) | Joint::Sail(s) => {
                    wings::wing(&mut mesh, coat, joint, f32::from(s))?
                }
                Joint::Breath
                | Joint::Impact
                | Joint::Rune
                | Joint::Missile(_)
                | Joint::Ward
                | Joint::Surge => effects::build(&mut mesh, coat, joint)?,
            }
            if let Some(form) = form {
                forms::sculpt(&mut mesh, coat, form, joint)?;
            }
            let offset = joint.pivot() * scale(kind, variant);
            // Attack effects are authored in their attachment frame.
            let shift = if joint.is_effect() {
                Vec3::ZERO
            } else {
                -joint.pivot()
            };
            let wing_factor = if matches!(joint, Joint::WingArm(_) | Joint::Sail(_)) {
                terra_core::creatures::dragons::wing_scale(kind, variant)
            } else {
                1.
            };
            let mesh = mesh
                .finish()?
                .translated_by(shift)
                .scaled_by(Vec3::splat(scale(kind, variant) * wing_factor));
            Ok((Part::Dragon(joint), offset, mesh))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let mut stream = Builder::default();
    effects::breath(&mut stream, coat, true)?;
    parts.push((
        Part::DragonLight(Joint::Breath),
        Vec3::ZERO,
        stream
            .finish()?
            .scaled_by(Vec3::splat(scale(kind, variant))),
    ));
    if kind == Kind::DarkDragon {
        for joint in [Joint::Neck, Joint::Sail(-1), Joint::Sail(1)] {
            let mut mesh = Builder::default();
            match joint {
                Joint::Neck => anatomy::heart(&mut mesh, coat)?,
                Joint::Sail(side) => wings::lit_rims(&mut mesh, coat, f32::from(side))?,
                _ => unreachable!(),
            }
            let mesh = mesh
                .finish()?
                .translated_by(-joint.pivot())
                .scaled_by(Vec3::splat(scale(kind, variant)));
            parts.push((
                Part::DragonLight(joint),
                joint.pivot() * scale(kind, variant),
                mesh,
            ));
        }
    }
    if let Some(form) = form {
        for joint in [
            Joint::Trunk,
            Joint::Skull,
            Joint::Tail(3),
            Joint::Tail(4),
            Joint::Tail(5),
        ] {
            let mut mesh = Builder::default();
            forms::marks(&mut mesh, coat, form, joint)?;
            let mesh = mesh
                .finish()?
                .translated_by(-joint.pivot())
                .scaled_by(Vec3::splat(scale(kind, variant)));
            parts.push((
                Part::DragonLight(joint),
                joint.pivot() * scale(kind, variant),
                mesh,
            ));
        }
    }
    if matches!(form, Some(Lineage::Stone | Lineage::Shadow)) {
        let mut lit = Builder::default();
        spells::fissure(&mut lit, coat, true)?;
        parts.push((
            Part::DragonLight(Joint::Rune),
            Vec3::ZERO,
            lit.finish()?.scaled_by(Vec3::splat(scale(kind, variant))),
        ));
    }
    Ok(parts)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn greater_dragon_shapes_and_spell_parts_are_finite_for_every_coat() {
        for kind in [Kind::DarkDragon, Kind::EarthDragon, Kind::WaterDragon] {
            for variant in 0..kind.variant_count() {
                let Some(_) = lineage(kind, variant) else {
                    continue;
                };
                let parts = parts(kind, variant).unwrap();
                assert_eq!(
                    parts
                        .iter()
                        .filter(|(part, _, _)| matches!(part, Part::Dragon(Joint::Missile(_))))
                        .count(),
                    3
                );
                for (_, _, mesh) in parts {
                    let positions = mesh
                        .attribute(Mesh::ATTRIBUTE_POSITION)
                        .unwrap()
                        .as_float3()
                        .unwrap();
                    assert!(
                        !positions.is_empty() && positions.iter().flatten().all(|n| n.is_finite()),
                        "{kind:?} {variant}"
                    );
                }
            }
        }
    }
}
