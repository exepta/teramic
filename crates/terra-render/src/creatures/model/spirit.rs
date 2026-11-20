//! Authored shared meshes for the Spirit Blossom fauna. Rounded voxel forms,
//! petal wings, curled tails and separate luminous details stay inside our style.
use super::{Part, shape};
use crate::creatures::model::voxel::Builder;
use bevy::prelude::*;

const CREAM: [f32; 3] = [0.97, 0.96, 1.0];
const INK: [f32; 3] = [0.22, 0.13, 0.36];
const CYAN: [f32; 3] = [0.37, 0.83, 0.98];
const PINK: [f32; 3] = [1.0, 0.40, 0.72];

fn flower(mesh: &mut Builder, center: Vec3, radius: f32, tint: [f32; 3]) -> Result<(), String> {
    for n in 0..5 {
        let angle = n as f32 * std::f32::consts::TAU / 5.0;
        shape::rounded(
            mesh,
            (center + Vec3::new(angle.sin(), angle.cos(), 0.0) * radius * 0.55).to_array(),
            [radius * 0.8, radius * 0.8, radius * 0.30],
            radius * 0.18,
            tint,
        )?;
    }
    shape::rounded(
        mesh,
        (center - Vec3::Z * radius * 0.18).to_array(),
        [radius * 0.40; 3],
        radius * 0.09,
        [1.0, 0.86, 0.35],
    )
}

mod butterfly;
mod gloom;
pub(super) use butterfly::butterfly;
pub(super) use gloom::{flame_spirit, gloom};

pub(super) fn fox() -> Result<Vec<(Part, Vec3, Mesh)>, String> {
    let mut body = Builder::default();
    shape::rounded(&mut body, [0.0, 0.42, 0.07], [0.43, 0.47, 0.79], 0.08, CYAN)?;
    shape::rounded(
        &mut body,
        [0.0, 0.51, -0.19],
        [0.46, 0.52, 0.36],
        0.09,
        CREAM,
    )?;
    for side in [-1.0, 1.0] {
        shape::rounded(
            &mut body,
            [side * 0.21, 0.59, -0.22],
            [0.20, 0.21, 0.28],
            0.04,
            CREAM,
        )?;
    }
    let mut head = Builder::default();
    shape::rounded(&mut head, [0.0, 0.0, 0.0], [0.48, 0.35, 0.39], 0.07, CREAM)?;
    shape::rounded(
        &mut head,
        [0.0, -0.08, -0.24],
        [0.25, 0.16, 0.26],
        0.04,
        CREAM,
    )?;
    shape::rounded(
        &mut head,
        [0.0, -0.055, -0.37],
        [0.09, 0.07, 0.05],
        0.012,
        INK,
    )?;
    for side in [-1.0, 1.0] {
        for n in 0..4 {
            let t = n as f32 / 3.0;
            shape::rounded(
                &mut head,
                [side * (0.16 + t * 0.025), 0.18 + t * 0.065, 0.01],
                [0.19 * (1.0 - t * 0.75), 0.085, 0.13 * (1.0 - t * 0.6)],
                0.015,
                CYAN,
            )?;
            head.voxel(
                [side * (0.16 + t * 0.025), 0.18 + t * 0.065, -0.06],
                [0.10 * (1.0 - t * 0.8), 0.065, 0.012],
                [0.73, 0.56, 0.94],
            )?;
        }
        shape::rounded(
            &mut head,
            [side * 0.17, 0.015, -0.19],
            [0.10, 0.055, 0.04],
            0.01,
            INK,
        )?;
        head.voxel([side * 0.15, 0.028, -0.213], [0.028, 0.024, 0.008], CYAN)?;
    }
    flower(&mut head, Vec3::new(-0.24, 0.19, -0.08), 0.12, PINK)?;
    let mut parts = vec![
        (Part::Body, Vec3::ZERO, body.finish()?),
        (Part::Head, Vec3::new(0.0, 0.77, -0.34), head.finish()?),
    ];
    for side in [-1.0, 1.0] {
        for front in [-1.0, 1.0] {
            let mut leg = Builder::default();
            shape::rounded(&mut leg, [0.0, -0.12, 0.0], [0.13, 0.29, 0.15], 0.03, CYAN)?;
            shape::rounded(
                &mut leg,
                [0.0, -0.25, -0.035],
                [0.16, 0.09, 0.22],
                0.02,
                CREAM,
            )?;
            parts.push((
                Part::Leg(side * front),
                Vec3::new(side * 0.16, 0.30, front * 0.26),
                leg.finish()?,
            ));
        }
    }
    let mut tails = Builder::default();
    for fan in [-1.0, 0.0, 1.0] {
        let mut previous = Vec3::ZERO;
        for n in 0..18 {
            let t = n as f32 / 17.0;
            let a = t * std::f32::consts::TAU * 0.85;
            let radius = 0.36 * (1.0 - t * 0.66);
            let next = Vec3::new(
                fan * 0.28 * t,
                0.25 + a.sin() * radius + fan.abs() * 0.09,
                0.37 - a.cos() * radius,
            );
            let width = 0.19 * (1.0 - t * 0.86);
            shape::segment(
                &mut tails,
                previous,
                next,
                width,
                width,
                if n > 11 { CREAM } else { CYAN },
            )?;
            previous = next;
        }
    }
    parts.push((Part::Tail, Vec3::new(0.0, 0.47, 0.39), tails.finish()?));
    Ok(parts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::mesh::VertexAttributeValues;
    #[test]
    fn spirit_meshes_are_finite_distinct_and_have_animated_appendages() {
        let mut signatures = std::collections::BTreeSet::new();
        for variant in 0..7 {
            let parts = butterfly(variant).unwrap();
            assert_eq!(
                parts
                    .iter()
                    .filter(|(part, _, _)| matches!(part, Part::Wing(_)))
                    .count(),
                2
            );
            let mut hash = 0_u64;
            for (_, _, mesh) in &parts {
                let Some(VertexAttributeValues::Float32x4(colors)) =
                    mesh.attribute(Mesh::ATTRIBUTE_COLOR)
                else {
                    panic!("vertex colors missing")
                };
                for value in colors.iter().flatten() {
                    hash = hash
                        .wrapping_mul(31)
                        .wrapping_add(u64::from(value.to_bits()));
                }
            }
            assert!(signatures.insert(hash));
        }
        for parts in [
            gloom(0).unwrap(),
            gloom(1).unwrap(),
            fox().unwrap(),
            butterfly(6).unwrap(),
        ] {
            let count: usize = parts.iter().map(|(_, _, mesh)| mesh.count_vertices()).sum();
            assert!(count > 100 && count < 30_000, "shared model budget {count}");
            for (_, offset, mesh) in parts {
                assert!(offset.is_finite());
                let Some(VertexAttributeValues::Float32x3(vertices)) =
                    mesh.attribute(Mesh::ATTRIBUTE_POSITION)
                else {
                    panic!("positions missing")
                };
                assert!(
                    vertices
                        .iter()
                        .flatten()
                        .all(|v| v.is_finite() && v.abs() < 2.0)
                );
                assert!(mesh.indices().is_some());
            }
        }
    }
}
