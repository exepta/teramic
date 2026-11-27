//! Fractured stone plates with shelf fungi rooted in the joints, and recessed spiral runes.
use super::{Part, shape, voxel::Builder};
use bevy::prelude::*;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::creatures) enum Joint {
    Chest,
    Face,
    Shoulder(i8),
    Forearm(i8),
    Leg(i8),
    Foot(i8),
    Seal,
    Shock,
    Chips,
}
impl Joint {
    pub(in crate::creatures) fn pivot(self) -> Vec3 {
        match self {
            Self::Chest => Vec3::new(0., 1.94, 0.04),
            Self::Face => Vec3::new(0., 3.28, -0.36),
            Self::Shoulder(s) => Vec3::new(f32::from(s) * 1.03, 3.08, -0.01),
            Self::Forearm(s) => Vec3::new(f32::from(s) * 1.38, 2.08, -0.05),
            Self::Leg(s) => Vec3::new(f32::from(s) * 0.49, 1.74, 0.08),
            Self::Foot(s) => Vec3::new(f32::from(s) * 0.57, 0.61, 0.03),
            Self::Seal | Self::Shock | Self::Chips => Vec3::new(0., 0.04, 0.),
        }
    }
}
const SHALE: [f32; 3] = [0.41, 0.38, 0.397];
const CREVICE: [f32; 3] = [0.23, 0.213, 0.236];
const RUNE: [f32; 3] = [0.85, 0.33, 1.];
fn tint(c: [f32; 3], k: f32) -> [f32; 3] {
    c.map(|v| (v * k).min(1.))
}
fn stone(b: &mut Builder, p: [f32; 3], size: [f32; 3], tone: f32) -> Result<(), String> {
    let bevel = size[0].min(size[1]).min(size[2]) * 0.36;
    shape::rounded(b, p, size, bevel, tint([0.68, 0.615, 0.635], tone))
}
fn rock_surface(p: Vec3, color: [f32; 3], old: bool) -> [f32; 3] {
    // Fungi and moss keep their authored pigment. The stone has cross-bedded
    // mineral layers, offset joints and ochre lichen on exposed knuckles.
    if color[2] < color[1] || color[0] - color[1] > 0.15 {
        return color;
    }
    let slope = p.y + 0.23 * p.x - 0.12 * p.z + 0.044 * (p.x * 7. + p.z * 4.).sin();
    let beds = slope * 3.7;
    let joint = (p.x * 1.94 + p.z * 1.43 + 0.42 * beds.floor() + 0.20 * slope).rem_euclid(1.);
    let seam = beds.rem_euclid(1.);
    let grain = (p.x * 10. + p.z * 4.4).sin() * (p.y * 9. - p.x * 3.8).cos();
    if old && grain > 0.78 && seam > 0.32 {
        return [0.71, 0.63, 0.27];
    }
    let value = if joint < 0.052 && seam < 0.76 || seam < 0.09 && grain > -0.28 {
        0.56
    } else if (0.09..0.21).contains(&seam) && grain > 0.02 {
        1.18
    } else if grain < -0.42 {
        0.88
    } else {
        1.
    };
    tint(color, value)
}
fn fungus(b: &mut Builder, p: Vec3, r: f32, old: bool) -> Result<(), String> {
    let cap = if old {
        [0.81, 0.58, 0.19]
    } else {
        [0.89, 0.16, 0.32]
    };
    b.ellipsoid(
        p + Vec3::Y * r * 0.19,
        Vec3::new(r * 0.21, r * 0.53, r * 0.21),
        [0.78, 0.6, 0.51],
    )?;
    b.ellipsoid(
        p + Vec3::Y * r * 0.57,
        Vec3::new(r, r * 0.18, r * 0.8),
        tint(cap, 1.23),
    )?;
    b.ellipsoid(
        p + Vec3::Y * r * 0.69,
        Vec3::new(r * 0.92, r * 0.35, r * 0.75),
        cap,
    )?;
    for (x, z) in [(-0.32, -0.36), (0.15, -0.13), (0.40, 0.22)] {
        b.ellipsoid(
            p + Vec3::new(x, 0.95, z) * r,
            Vec3::new(0.16, 0.075, 0.12) * r,
            if old {
                [0.92, 0.76, 0.38]
            } else {
                [1., 0.49, 0.56]
            },
        )?;
    }
    Ok(())
}
fn moss(b: &mut Builder, x: f32, y: f32, z: f32, old: bool) -> Result<(), String> {
    let lichen = if old {
        [0.59, 0.55, 0.17]
    } else {
        [0.42, 0.42, 0.18]
    };
    for i in 0..if old { 7 } else { 3 } {
        let f = i as f32;
        let len = 0.12 + (f * 1.87).sin().abs() * if old { 0.5 } else { 0.18 };
        shape::rounded(
            b,
            [x + f * 0.055, y - len * 0.5, z + 0.045 * (f * 1.82).cos()],
            [0.095, len, 0.095],
            0.025,
            tint(lichen, 0.78 + 0.25 * (f + 1.).sin().abs()),
        )?;
    }
    Ok(())
}
fn spiral(b: &mut Builder, center: Vec3, k: f32) -> Result<(), String> {
    let points = [
        [-0.48, 0.08],
        [-0.48, 0.46],
        [0.44, 0.46],
        [0.44, -0.43],
        [-0.30, -0.43],
        [-0.30, 0.22],
        [0.19, 0.22],
        [0.19, -0.18],
        [-0.075, -0.18],
        [-0.075, 0.045],
    ];
    for pair in points.windows(2) {
        let point = |u: [f32; 2]| center + Vec3::new(u[0] * k, u[1] * k, 0.);
        shape::segment(b, point(pair[0]), point(pair[1]), 0.073 * k, 0.062, RUNE)?;
    }
    Ok(())
}
pub(super) fn parts(variant: u8) -> Result<Vec<(Part, Vec3, Mesh)>, String> {
    let old = variant == 1;
    let mut result = Vec::new();
    let mut add = |j: Joint, lit: bool, b: Builder| -> Result<(), String> {
        let surface = if lit {
            b.finish()?
        } else {
            b.finish_painted(|p, color| rock_surface(j.pivot() + p, color, old))?
        };
        result.push((Part::Golem(j, lit), j.pivot(), surface));
        Ok(())
    };
    let mesh = || Builder::with_cell_size(0.045);
    let mut chest = mesh();
    shape::rounded(
        &mut chest,
        [0., 0.45, 0.03],
        [1.64, 1.52, 1.15],
        0.22,
        CREVICE,
    )?;
    for row in 0..3 {
        for col in 0..3 {
            let r = row as f32;
            let c = col as f32 - 1.;
            let x = c * (0.41 + r * 0.083);
            let y = -0.03 + r * 0.46 + (c * 2. + r).sin() * 0.055;
            stone(
                &mut chest,
                [x, y, -0.38 - r * 0.055],
                [0.40 + r * 0.13, 0.46, 0.42],
                0.80 + 0.18 * (c * 3. + r * 1.8).cos(),
            )?;
            stone(
                &mut chest,
                [x, y + 0.07, 0.43],
                [0.45 + r * 0.13, 0.48, 0.43],
                0.78 + 0.11 * (c + r).sin(),
            )?;
        }
    }
    for s in [-1., 1.] {
        stone(
            &mut chest,
            [s * 0.46, -0.02, 0.01],
            [0.68, 0.50, 1.01],
            0.78,
        )?;
        moss(&mut chest, s * 0.69, 1.15, -0.68, old)?;
        moss(&mut chest, s * 0.54, 1.07, 0.64, old)?;
        for n in 0..3 {
            let u = n as f32;
            fungus(
                &mut chest,
                Vec3::new(
                    s * (0.31 + 0.24 * u),
                    1.19 + 0.07 * u,
                    0.12 + 0.18 * (u * 2.8).sin(),
                ),
                0.21 + 0.055 * u,
                old,
            )?;
        }
    }
    add(Joint::Chest, false, chest)?;
    let mut face = mesh();
    shape::rounded(
        &mut face,
        [0., 0.08, -0.09],
        [0.96, 0.90, 0.91],
        0.15,
        CREVICE,
    )?;
    for s in [-1., 1.] {
        stone(&mut face, [s * 0.28, 0.34, -0.35], [0.54, 0.28, 0.66], 1.1)?;
        stone(&mut face, [s * 0.44, 0.025, -0.24], [0.23, 0.44, 0.58], 0.9)?;
        stone(
            &mut face,
            [s * 0.255, -0.16, -0.42],
            [0.24, 0.37, 0.49],
            1.13,
        )?;
        moss(&mut face, s * 0.44, 0.29, -0.24, old)?;
    }
    stone(&mut face, [0., 0.04, -0.54], [0.24, 0.42, 0.35], 1.16)?;
    stone(&mut face, [0., -0.26, -0.24], [0.66, 0.23, 0.51], 0.82)?;
    fungus(&mut face, Vec3::new(-0.28, 0.44, 0.04), 0.19, old)?;
    add(Joint::Face, false, face)?;
    let mut eyes = mesh();
    for s in [-1., 1.] {
        eyes.voxel([s * 0.255, 0.13, -0.57], [0.18, 0.09, 0.045], RUNE)?;
    }
    add(Joint::Face, true, eyes)?;
    for side in [-1i8, 1] {
        let s = f32::from(side);
        let mut arm = mesh();
        stone(&mut arm, [s * 0.04, 0.01, 0.], [0.94, 0.86, 0.98], 1.04)?;
        stone(&mut arm, [s * 0.12, 0.0, -0.44], [0.80, 0.69, 0.22], 0.78)?;
        shape::segment(
            &mut arm,
            Vec3::new(s * 0.07, -0.20, 0.),
            Vec3::new(s * 0.34, -0.93, -0.04),
            0.49,
            0.57,
            SHALE,
        )?;
        for k in 0..3 {
            stone(
                &mut arm,
                [s * (0.16 + 0.07 * k as f32), -0.38 - 0.21 * k as f32, -0.18],
                [0.50, 0.27, 0.52],
                0.89 + 0.052 * k as f32,
            )?;
        }
        moss(&mut arm, s * 0.12, 0.38, -0.40, old)?;
        fungus(&mut arm, Vec3::new(s * 0.17, 0.37, 0.), 0.32, old)?;
        add(Joint::Shoulder(side), false, arm)?;
        let mut shoulder_rune = Builder::with_cell_size(0.024);
        spiral(&mut shoulder_rune, Vec3::new(s * 0.12, 0.02, -0.566), 0.58)?;
        add(Joint::Shoulder(side), true, shoulder_rune)?;
        let mut hand = mesh();
        stone(&mut hand, [0., -0.34, -0.04], [0.71, 0.87, 0.78], 0.87)?;
        stone(
            &mut hand,
            [s * 0.06, -0.50, -0.38],
            [0.65, 0.55, 0.24],
            0.99,
        )?;
        stone(
            &mut hand,
            [s * 0.03, -0.93, -0.04],
            [0.75, 0.47, 0.80],
            0.86,
        )?;
        for i in 0..3 {
            let x = (i as f32 - 1.) * 0.23;
            stone(
                &mut hand,
                [x, -1.23, -0.26],
                [0.21, 0.43, 0.34],
                1.03 - i as f32 * 0.048,
            )?;
            stone(&mut hand, [x, -1.42, -0.32], [0.19, 0.15, 0.25], 0.91)?;
        }
        stone(
            &mut hand,
            [-s * 0.39, -1.03, -0.28],
            [0.24, 0.41, 0.39],
            0.97,
        )?;
        for i in 0..3 {
            fungus(
                &mut hand,
                Vec3::new(s * (0.25 + 0.035 * i as f32), 0.04 - 0.22 * i as f32, -0.04),
                0.24 - i as f32 * 0.035,
                old,
            )?;
        }
        moss(&mut hand, -0.21, -0.34, -0.48, old)?;
        add(Joint::Forearm(side), false, hand)?;
        let mut fore_rune = Builder::with_cell_size(0.021);
        spiral(&mut fore_rune, Vec3::new(s * 0.03, -0.48, -0.511), 0.47)?;
        add(Joint::Forearm(side), true, fore_rune)?;
        let mut leg = mesh();
        stone(&mut leg, [0., -0.36, 0.03], [0.74, 0.80, 0.84], 0.79)?;
        stone(&mut leg, [s * 0.065, -0.72, -0.2], [0.63, 0.49, 0.70], 1.07)?;
        stone(&mut leg, [s * 0.085, -0.99, 0.12], [0.54, 0.45, 0.63], 0.80)?;
        add(Joint::Leg(side), false, leg)?;
        let mut foot = mesh();
        stone(&mut foot, [0., -0.28, -0.09], [0.76, 0.55, 0.83], 0.95)?;
        for i in 0..3 {
            stone(
                &mut foot,
                [(i as f32 - 1.) * 0.24, -0.455, -0.46],
                [0.22, 0.27, 0.52],
                1.06 - i as f32 * 0.06,
            )?;
        }
        add(Joint::Foot(side), false, foot)?;
    }
    // An interrupted band of squared glyphs. Small diamonds and a pale tip
    // every quarter turn make its rotation readable from either side.
    let mut seal = Builder::with_cell_size(0.032);
    for i in 0..12 {
        let a = std::f32::consts::TAU * i as f32 / 12.;
        let radial = Vec3::new(a.cos(), 0., a.sin());
        let tangent = Vec3::new(-a.sin(), 0., a.cos());
        let base = radial + Vec3::Y * 0.06;
        let line = |b: &mut Builder, from, to| shape::segment(b, from, to, 0.035, 0.042, RUNE);
        line(&mut seal, base - tangent * 0.195, base + tangent * 0.195)?;
        line(
            &mut seal,
            base - tangent * 0.16,
            base - tangent * 0.16 - radial * 0.074,
        )?;
        let mark = base + Vec3::Y * 0.14;
        seal.rotated(
            mark.to_array(),
            [0.088, 0.096, 0.066],
            [1., 0.73, 0.96],
            Quat::from_rotation_y(-a) * Quat::from_rotation_z(0.78),
        )?;
        if i % 3 == 0 {
            line(&mut seal, base + tangent * 0.15, mark + tangent * 0.15)?;
        }
    }
    add(Joint::Seal, true, seal)?;
    let mut wave = Builder::with_cell_size(0.05);
    let mut chips = mesh();
    for i in 0..14 {
        let a = i as f32 * std::f32::consts::TAU / 14.;
        let dir = Vec3::new(a.cos(), 0., a.sin());
        let base = dir * (0.89 + 0.12 * (i as f32 * 2.9).sin());
        shape::segment(
            &mut wave,
            base,
            base * 1.14 + Vec3::Y * (0.18 + 0.15 * (i % 3) as f32),
            0.09,
            0.075,
            RUNE,
        )?;
        stone(
            &mut chips,
            (dir * 1.12 + Vec3::Y * (0.1 + 0.1 * (i % 4) as f32)).to_array(),
            [0.17, 0.15, 0.12],
            1.15,
        )?;
    }
    add(Joint::Shock, true, wave)?;
    add(Joint::Chips, false, chips)?;
    Ok(result)
}
