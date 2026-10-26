//! Shared, fully solid reference tools. Their working edge points along +X;
//! the haft is +Y and the broad authored profile lies in XY.
use super::geometry::{extrude, socket};
use super::palette::{LEATHER, LEATHER_LIGHT, WOOD, WOOD_DARK, WOOD_LIGHT};
use super::{Builder, Palette, ToolMaterial};
use bevy::prelude::*;

fn part(b: &mut Builder, center: [f32; 3], size: [f32; 3], color: [f32; 3]) -> Result<(), String> {
    b.bevel_box(center, size, color, Quat::IDENTITY)
}

fn beam(
    b: &mut Builder,
    start: Vec3,
    end: Vec3,
    width: f32,
    depth: f32,
    color: [f32; 3],
) -> Result<(), String> {
    let delta = end - start;
    b.bevel_box(
        ((start + end) * 0.5).to_array(),
        [width, delta.length(), depth],
        color,
        Quat::from_rotation_arc(Vec3::Y, delta.normalize()),
    )
}

fn stud(b: &mut Builder, point: [f32; 3], p: &Palette) -> Result<(), String> {
    b.voxel(point, [0.024, 0.024, 0.018], p.edge)
}

pub(super) fn bucket(b: &mut Builder, p: &Palette, filled: bool) -> Result<(), String> {
    // Twelve solid staves leave a genuine open top, with rim, bands and handle.
    b.voxel([0.5, 0.14, 0.5], [0.44, 0.075, 0.44], p.dark)?;
    for i in 0..12 {
        let angle = i as f32 * std::f32::consts::TAU / 12.;
        for (height, radius, width, thickness, color) in [
            (0.40, 0.25, 0.135, 0.51, p.body),
            (0.20, 0.255, 0.145, 0.055, p.dark),
            (0.63, 0.275, 0.151, 0.055, p.light),
        ] {
            b.bevel_box(
                [
                    0.5 + angle.sin() * radius,
                    height,
                    0.5 + angle.cos() * radius,
                ],
                [width, thickness, 0.045],
                color,
                Quat::from_rotation_y(angle),
            )?;
        }
    }
    if filled {
        let center = Vec3::new(0.5, 0.5875, 0.5);
        let rim = |angle: f32| center + Vec3::new(angle.sin(), 0.0, angle.cos()) * 0.22;
        for i in 0..12 {
            let angle = i as f32 * std::f32::consts::TAU / 12.;
            b.triangle(
                [center, rim(angle), rim(angle + std::f32::consts::TAU / 12.)],
                [0.12, 0.52, 0.68],
            )?;
        }
    }
    for side in [-1., 1.] {
        beam(
            b,
            Vec3::new(0.5 + side * 0.27, 0.58, 0.5),
            Vec3::new(0.5 + side * 0.22, 0.94, 0.5),
            0.035,
            0.035,
            p.dark,
        )?;
    }
    part(b, [0.5, 0.94, 0.5], [0.46, 0.04, 0.045], p.light)
}

pub(super) fn hoe(b: &mut Builder, p: &Palette) -> Result<(), String> {
    // A single down-curved cutting head, rather than a miniature double axe.
    let head = [
        [0.465, 0.86],
        [0.47, 0.98],
        [0.56, 1.0],
        [0.64, 0.96],
        [0.78, 0.87],
        [0.91, 0.79],
        [0.94, 0.62],
        [0.865, 0.655],
        [0.76, 0.73],
        [0.65, 0.79],
        [0.56, 0.83],
    ];
    extrude(b, &head, 0.12, p.body, p.dark, p.edge, true)?;
    socket(b, p, 0.88)?;
    for side in [-1.0, 1.0] {
        for index in 0..4_usize {
            let x = 0.65 + index as f32 * 0.065;
            let y = 0.864 - index as f32 * 0.047;
            part(
                b,
                [x, y, 0.5 + side * 0.065],
                [0.045, 0.022, 0.012],
                if index.is_multiple_of(2) {
                    p.light
                } else {
                    p.dark
                },
            )?;
        }
        for y in [0.05, 0.88] {
            part(
                b,
                [0.5, y, 0.5 + side * 0.076],
                [0.045, 0.045, 0.018],
                p.inlay,
            )?;
        }
    }
    Ok(())
}

pub(super) fn scythe(b: &mut Builder, p: &Palette) -> Result<(), String> {
    // Joined, concave crescent; the blade is thick at the spine and ends in a
    // curved working tip. Ear-clipped faces remain watertight on both sides.
    let blade = [
        [0.46, 0.865],
        [0.45, 0.985],
        [0.565, 1.0],
        [0.685, 0.975],
        [0.795, 0.915],
        [0.885, 0.83],
        [0.95, 0.725],
        [0.985, 0.61],
        [0.99, 0.51],
        [0.946, 0.60],
        [0.88, 0.688],
        [0.80, 0.76],
        [0.71, 0.816],
        [0.61, 0.85],
        [0.53, 0.86],
    ];
    extrude(b, &blade, 0.112, p.body, p.dark, p.edge, true)?;
    socket(b, p, 0.90)?;
    for side in [-1.0, 1.0] {
        for (x, y, tilt) in [
            (0.64, 0.922, -0.25),
            (0.76, 0.873, -0.54),
            (0.857, 0.794, -0.83),
        ] {
            b.bevel_box(
                [x, y, 0.5 + side * 0.064],
                [0.096, 0.023, 0.016],
                p.light,
                Quat::from_rotation_z(tilt),
            )?;
            stud(b, [x, y, 0.5 + side * 0.077], p)?;
        }
        for y in [0.04, 0.90] {
            part(
                b,
                [0.5, y, 0.5 + side * 0.084],
                [0.049, 0.052, 0.025],
                p.inlay,
            )?;
        }
    }
    // The reference's second hand-rest and tied leaf sprig are genuine solids.
    beam(
        b,
        Vec3::new(0.50, 0.48, 0.5),
        Vec3::new(0.35, 0.51, 0.5),
        0.048,
        0.060,
        WOOD,
    )?;
    for i in 0..3 {
        part(
            b,
            [0.39 + i as f32 * 0.023, 0.505, 0.5],
            [0.015, 0.068, 0.075],
            LEATHER_LIGHT,
        )?;
    }
    if p.kind == ToolMaterial::Wood {
        for (x, y, z) in [
            (0.453, 0.62, 0.555),
            (0.418, 0.66, 0.56),
            (0.46, 0.69, 0.552),
        ] {
            b.bevel_box(
                [x, y, z],
                [0.064, 0.030, 0.018],
                [0.27, 0.43, 0.105],
                Quat::from_rotation_z(0.45),
            )?;
        }
    }
    Ok(())
}

pub(super) fn watering_can(b: &mut Builder, p: &Palette) -> Result<(), String> {
    let wood = p.kind == ToolMaterial::Wood;
    let body = if wood { WOOD } else { p.body };
    let light = if wood { WOOD_LIGHT } else { p.light };
    let dark = if wood { WOOD_DARK } else { p.dark };
    // Eight stepped barrel walls, with a solid bottom and a recessed dark mouth.
    for i in 0..8_usize {
        let angle = i as f32 * std::f32::consts::TAU / 8.0;
        let (sin, cos) = angle.sin_cos();
        b.bevel_box(
            [0.35 + sin * 0.178, 0.32, 0.5 + cos * 0.178],
            [0.158, 0.51, 0.052],
            if i.is_multiple_of(3) { light } else { body },
            Quat::from_rotation_y(angle),
        )?;
        for row in 0..4_usize {
            b.voxel(
                [
                    0.35 + sin * 0.209,
                    0.13 + row as f32 * 0.115,
                    0.5 + cos * 0.209,
                ],
                [0.014, 0.063, 0.014],
                if row.is_multiple_of(2) { dark } else { light },
            )?;
        }
    }
    part(b, [0.35, 0.028, 0.5], [0.395, 0.056, 0.395], p.fitting_dark)?;
    part(b, [0.35, 0.062, 0.5], [0.435, 0.068, 0.435], p.fitting)?;
    part(b, [0.35, 0.568, 0.5], [0.435, 0.072, 0.435], p.fitting)?;
    part(b, [0.35, 0.584, 0.5], [0.33, 0.013, 0.33], dark)?;
    for y in [0.06, 0.56] {
        for x in [0.18, 0.29, 0.41, 0.52] {
            for z in [0.271, 0.729] {
                stud(b, [x, y, z], p)?;
            }
        }
        for z in [0.33, 0.46, 0.59, 0.67] {
            for x in [0.122, 0.578] {
                b.voxel([x, y, z], [0.018, 0.022, 0.022], p.edge)?;
            }
        }
    }
    // Top carrying bow: grip at (0.35, .9, .5), not at the bottom of the can.
    for x in [0.23, 0.47] {
        part(b, [x, 0.776, 0.5], [0.045, 0.36, 0.065], p.fitting_dark)?;
        part(b, [x, 0.776, 0.539], [0.021, 0.31, 0.016], p.fitting)?;
    }
    part(b, [0.35, 0.962, 0.5], [0.285, 0.076, 0.085], p.fitting)?;
    for x in [0.285, 0.33, 0.375, 0.42] {
        part(b, [x, 0.932, 0.5], [0.018, 0.027, 0.092], LEATHER)?;
    }
    // A second loop handle is open in its center, with no flat opaque plate.
    for (start, end) in [
        ([0.15, 0.50, 0.5], [0.035, 0.55, 0.5]),
        ([0.035, 0.55, 0.5], [0.035, 0.25, 0.5]),
        ([0.035, 0.25, 0.5], [0.15, 0.20, 0.5]),
    ] {
        beam(
            b,
            Vec3::from_array(start),
            Vec3::from_array(end),
            0.045,
            0.07,
            p.fitting,
        )?;
    }
    // Rising spout (+X) and an octagonal perforated rose. The holes are dark
    // recessed insets rather than transparency, retaining one shared material.
    let spout_start = Vec3::new(0.53, 0.25, 0.5);
    let rose = Vec3::new(0.81, 0.70, 0.5);
    beam(b, spout_start, rose, 0.10, 0.10, body)?;
    beam(
        b,
        spout_start + Vec3::Z * 0.055,
        rose + Vec3::Z * 0.055,
        0.024,
        0.018,
        light,
    )?;
    let outward = (rose - spout_start).normalize();
    let rotation = Quat::from_rotation_arc(Vec3::Z, outward);
    b.bevel_box(
        rose.to_array(),
        [0.285, 0.285, 0.055],
        p.fitting_dark,
        rotation,
    )?;
    b.bevel_box(
        (rose + outward * 0.035).to_array(),
        [0.248, 0.248, 0.028],
        p.light,
        rotation,
    )?;
    for row in -2_i32..=2 {
        for column in -2_i32..=2 {
            if row.abs() + column.abs() > 3 {
                continue;
            }
            let point =
                rose + rotation * Vec3::new(column as f32 * 0.047, row as f32 * 0.047, 0.052);
            b.rotated(point.to_array(), [0.021, 0.021, 0.008], dark, rotation)?;
        }
    }
    // Paired botanical badges on the barrel, so neither world-facing side is blank.
    for side in [-1.0, 1.0] {
        beam(
            b,
            Vec3::new(0.35, 0.22, 0.5 + side * 0.216),
            Vec3::new(0.35, 0.43, 0.5 + side * 0.216),
            0.019,
            0.012,
            [0.21, 0.37, 0.10],
        )?;
        for (direction, y) in [(-1.0, 0.30), (1.0, 0.35), (-1.0, 0.40)] {
            b.bevel_box(
                [0.35 + direction * 0.035, y, 0.5 + side * 0.223],
                [0.074, 0.034, 0.018],
                if direction > 0.0 {
                    [0.38, 0.56, 0.16]
                } else {
                    [0.24, 0.43, 0.12]
                },
                Quat::from_rotation_z(direction * 0.60),
            )?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod bucket_tests {
    use super::*;
    #[test]
    fn water_surface_is_inset_inside_the_staves_and_faces_upwards() {
        let palette = Palette::for_material(ToolMaterial::Iron);
        let mut empty = Builder::default();
        bucket(&mut empty, &palette, false).unwrap();
        let empty = empty.finish_batch().unwrap();
        let mut full = Builder::default();
        bucket(&mut full, &palette, true).unwrap();
        let full = full.finish_batch().unwrap();
        let mut surface = 0;
        for (point, normal) in full.positions.iter().zip(&full.normals) {
            if empty.positions.contains(point) {
                continue;
            }
            surface += 1;
            assert!((point[1] - 0.5875).abs() < 1e-5);
            assert!((point[0] - 0.5).hypot(point[2] - 0.5) <= 0.2201);
            assert!(normal[1] > 0.99);
        }
        assert_eq!(surface, 36);
    }
}
