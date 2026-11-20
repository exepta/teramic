use super::super::forest::ellipsoid;
use super::{CREAM, INK, Part, shape};
use crate::creatures::model::voxel::Builder;
use bevy::prelude::*;

/// A small flower spirit: layered porcelain petals surround a calm dark face.
/// Every appendage is cached once; animation only moves seven rigid parts.
pub(crate) fn gloom(variant: u8) -> Result<Vec<(Part, Vec3, Mesh)>, String> {
    let palette = terra_core::creatures::gloom::palette(variant);
    let (shell, edge, glow) = (palette.shell, palette.petal, palette.glow);
    let mut body = Builder::with_resolution(26);
    ellipsoid(
        &mut body,
        Vec3::new(0.0, 0.45, 0.035),
        Vec3::new(0.255, 0.255, 0.23),
        shell,
    )?;
    // The forward rim is open: the face has a broad uninterrupted silhouette,
    // without white sclera, a mouth or large staring pupils.
    for n in 0..12 {
        let a = n as f32 * std::f32::consts::TAU / 12.0;
        ellipsoid(
            &mut body,
            Vec3::new(a.sin() * 0.235, 0.45 + a.cos() * 0.228, -0.145),
            Vec3::new(0.071, 0.068, 0.077),
            if n % 3 == 0 { edge } else { shell },
        )?;
    }
    // Back plates and branching crystal antennae give the spirit a readable
    // silhouette from the sides and rear as well as its friendly front.
    for side in [-1.0, 1.0] {
        for n in 0..3 {
            let y = 0.32 + n as f32 * 0.115;
            ellipsoid(
                &mut body,
                Vec3::new(side * 0.10, y, 0.221),
                Vec3::new(0.09, 0.083, 0.038),
                edge,
            )?;
        }
        let base = Vec3::new(side * 0.115, 0.66, 0.015);
        let fork = Vec3::new(side * 0.165, 0.76, 0.015);
        shape::segment(&mut body, base, fork, 0.026, 0.025, shell)?;
        for (tip, width) in [
            (Vec3::new(side * 0.18, 0.845, 0.018), 0.047),
            (Vec3::new(side * 0.235, 0.80, 0.018), 0.034),
        ] {
            shape::segment(&mut body, fork, tip, width, width * 0.55, glow)?;
            ellipsoid(&mut body, tip, Vec3::splat(width * 0.33), CREAM)?;
        }
    }
    let mut face = Builder::with_resolution(22);
    ellipsoid(
        &mut face,
        Vec3::new(0.0, 0.45, -0.185),
        Vec3::new(0.215, 0.205, 0.082),
        [0.035, 0.065, 0.13],
    )?;
    let mut tail = Builder::with_resolution(28);
    let mut previous = Vec3::ZERO;
    // A descending stem opens into a tightening curl below the lowest petal.
    // The former shallow spiral was almost entirely occluded by the shell.
    for n in 1..=26 {
        let (next, width) = if n <= 10 {
            let t = n as f32 / 10.0;
            let smooth = t * t * (3.0 - 2.0 * t);
            (
                Vec3::new(0.055 * smooth, -0.245 * t, -0.012 * t),
                0.095 * (1.0 - t * 0.64),
            )
        } else {
            let t = (n - 10) as f32 / 16.0;
            let angle = -std::f32::consts::FRAC_PI_2 + t * std::f32::consts::TAU * 1.10;
            let radius = 0.055 * (1.0 - t * 0.80);
            (
                Vec3::new(
                    0.055 + angle.cos() * radius,
                    -0.190 + angle.sin() * radius,
                    -0.012 - t * 0.006,
                ),
                0.034 * (1.0 - t * 0.78),
            )
        };
        shape::segment(&mut tail, previous, next, width, width * 0.68, glow)?;
        previous = next;
    }
    let mut out = vec![
        (Part::Body, Vec3::ZERO, body.finish()?),
        (Part::Head, Vec3::ZERO, face.finish()?),
        (
            Part::SpiritGlow,
            Vec3::new(0.0, 0.218, -0.225),
            tail.finish()?,
        ),
    ];
    for side in [-1.0, 1.0] {
        let mut wings = Builder::with_resolution(22);
        for (rise, spread) in [(0.18, 0.27), (-0.12, 0.23)] {
            let base = Vec3::ZERO;
            let tip = Vec3::new(side * spread, rise, 0.025);
            let upper = Vec3::new(side * spread * 0.36, rise + 0.052, 0.0);
            let lower = Vec3::new(side * spread * 0.73, rise - 0.073, 0.028);
            wings.quad([base, upper, tip, lower], edge)?;
            shape::segment(&mut wings, base, tip, 0.012, 0.015, glow)?;
            for n in 1..=3 {
                let t = n as f32 / 4.0;
                let point = base.lerp(tip, t);
                for endpoint in [upper.lerp(tip, t * 0.75), lower.lerp(tip, t * 0.75)] {
                    shape::segment(&mut wings, point, endpoint, 0.007, 0.011, shell)?;
                }
            }
        }
        out.push((
            Part::SpiritArm(side),
            Vec3::new(side * 0.205, 0.465, 0.045),
            wings.finish()?,
        ));
        // Orbit slots carry the tiny independent oval eyes. Their animation is
        // locked to the face, with a gentle blink rather than rotating satellites.
        let mut eye = Builder::with_resolution(12);
        ellipsoid(
            &mut eye,
            Vec3::ZERO,
            Vec3::new(0.025, 0.047, 0.012),
            glow.map(|v| v * 0.72 + 0.28),
        )?;
        out.push((
            Part::SpiritOrbit(side),
            Vec3::new(side * 0.083, 0.46, -0.272),
            eye.finish()?,
        ));
    }
    Ok(out)
}

pub(crate) fn flame_spirit(
    body_color: [f32; 3],
    accent: [f32; 3],
    hostile: bool,
) -> Result<Vec<(Part, Vec3, Mesh)>, String> {
    let mut body = Builder::default();
    ellipsoid(
        &mut body,
        Vec3::new(0.0, 0.46, 0.0),
        Vec3::new(0.29, 0.28, 0.25),
        body_color,
    )?;
    ellipsoid(
        &mut body,
        Vec3::new(0.0, 0.25, 0.02),
        Vec3::new(0.21, 0.16, 0.18),
        accent,
    )?;
    let mut head = Builder::default();
    for side in [-1.0, 1.0] {
        shape::rounded(
            &mut head,
            [side * 0.12, 0.48, -0.228],
            [0.09, 0.14, 0.035],
            0.018,
            INK,
        )?;
        head.voxel([side * 0.12, 0.465, -0.250], [0.044, 0.070, 0.013], accent)?;
        head.voxel([side * 0.10, 0.519, -0.252], [0.02, 0.023, 0.014], CREAM)?;
        if hostile {
            head.rotated(
                [side * 0.12, 0.565, -0.23],
                [0.13, 0.028, 0.025],
                INK,
                Quat::from_rotation_z(side * 0.3),
            )?;
        }
    }
    head.voxel([0.0, 0.37, -0.243], [0.045, 0.022, 0.016], INK)?;
    let mut tail = Builder::default();
    for (base, direction, radius) in [
        (Vec3::new(0.0, 0.70, 0.0), 1.0, 0.11),
        (Vec3::new(0.0, 0.16, 0.0), -1.0, 0.09),
    ] {
        let mut previous = base;
        for n in 1..=9 {
            let t = n as f32 / 9.0;
            let angle = t * 5.3;
            let next = base
                + Vec3::new(
                    angle.sin() * radius * t,
                    direction * t * 0.22,
                    angle.cos() * 0.035 * t,
                );
            shape::segment(
                &mut tail,
                previous,
                next,
                0.074 * (1.0 - t * 0.86),
                0.065 * (1.0 - t * 0.80),
                accent,
            )?;
            previous = next;
        }
    }
    let mut out = vec![
        (Part::Body, Vec3::ZERO, body.finish()?),
        (Part::Head, Vec3::ZERO, head.finish()?),
        (Part::SpiritGlow, Vec3::ZERO, tail.finish()?),
    ];
    for side in [-1.0, 1.0] {
        let mut arm = Builder::default();
        let mut p = Vec3::ZERO;
        for n in 1..=5 {
            let t = n as f32 / 5.0;
            let next = Vec3::new(side * t * 0.20, t * t * 0.12, -t * 0.03);
            shape::segment(
                &mut arm,
                p,
                next,
                0.07 * (1.0 - t * 0.65),
                0.08 * (1.0 - t * 0.65),
                body_color,
            )?;
            p = next;
        }
        out.push((
            Part::SpiritArm(side),
            Vec3::new(side * 0.23, 0.36, 0.0),
            arm.finish()?,
        ));
        let mut orbit = Builder::default();
        ellipsoid(&mut orbit, Vec3::ZERO, Vec3::splat(0.045), accent)?;
        shape::segment(
            &mut orbit,
            Vec3::Y * 0.035,
            Vec3::new(0.028, 0.095, 0.0),
            0.027,
            0.026,
            body_color,
        )?;
        out.push((
            Part::SpiritOrbit(side),
            Vec3::new(side * 0.39, 0.58, 0.08),
            orbit.finish()?,
        ));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::mesh::VertexAttributeValues;

    #[test]
    fn gloom_keeps_two_small_oval_eyes_and_two_veined_wings_in_a_bounded_rig() {
        for variant in 0..terra_core::creatures::gloom::COUNT {
            let parts = gloom(variant).unwrap();
            assert_eq!(parts.len(), 7);
            let count: usize = parts.iter().map(|(_, _, mesh)| mesh.count_vertices()).sum();
            assert!(count < 16_000, "gloom rig: {count} vertices");
            let eyes: Vec<_> = parts
                .iter()
                .filter(|(part, _, _)| matches!(part, Part::SpiritOrbit(_)))
                .collect();
            assert_eq!(eyes.len(), 2);
            let (_, tail_root, tail) = parts
                .iter()
                .find(|(part, _, _)| matches!(part, Part::SpiritGlow))
                .unwrap();
            let Some(VertexAttributeValues::Float32x3(tail_points)) =
                tail.attribute(Mesh::ATTRIBUTE_POSITION)
            else {
                panic!("tail positions")
            };
            let below = tail_points
                .iter()
                .filter(|p| p[1] + tail_root.y < 0.12)
                .count();
            assert!(
                below > tail_points.len() / 3,
                "curled trail must clear the lower shell"
            );
            let low = tail_points
                .iter()
                .map(|p| p[1] + tail_root.y)
                .fold(f32::INFINITY, f32::min);
            assert!(
                (-0.06..0.01).contains(&low),
                "compact floating trail: {low}"
            );
            assert!(
                tail_root.z < -0.22,
                "trail must sit in front of the lower petal"
            );
            for (_, pivot, mesh) in eyes {
                assert!(pivot.z < -0.26);
                let Some(VertexAttributeValues::Float32x3(vertices)) =
                    mesh.attribute(Mesh::ATTRIBUTE_POSITION)
                else {
                    panic!("positions")
                };
                let low = vertices
                    .iter()
                    .map(|p| Vec3::from_array(*p))
                    .fold(Vec3::splat(f32::INFINITY), Vec3::min);
                let high = vertices
                    .iter()
                    .map(|p| Vec3::from_array(*p))
                    .fold(Vec3::splat(f32::NEG_INFINITY), Vec3::max);
                let size = high - low;
                assert!(size.x < 0.07 && size.y < 0.11);
                assert!(size.y > size.x * 1.5, "a soft oval, not a glaring disc");
            }
            assert_eq!(
                parts
                    .iter()
                    .filter(|(part, _, _)| matches!(part, Part::SpiritArm(_)))
                    .count(),
                2
            );
        }
    }
}
