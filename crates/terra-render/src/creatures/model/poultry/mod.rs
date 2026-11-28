//! Galliform bodies face -Z. All feathers on an articulated part bake to one indexed surface.
mod coat;
use super::{Kind, Part, shape::segment, voxel::Builder};
use bevy::prelude::*;
use coat::{Coat, Color};

const COMB: Color = [0.53, 0.024, 0.021];
const COMB_LIGHT: Color = [0.76, 0.049, 0.032];
const SHANK: Color = [0.64, 0.34, 0.075];
const BEAK: Color = [0.82, 0.49, 0.095];
const HORN: Color = [0.65, 0.49, 0.26];

pub(in crate::creatures) fn hip(kind: Kind) -> Vec3 {
    Vec3::new(0., if kind == Kind::Rooster { 0.275 } else { 0.235 }, 0.024)
}

pub(super) fn parts(kind: Kind, variant: u8) -> Result<Vec<(Part, Vec3, Mesh)>, String> {
    let cock = kind == Kind::Rooster;
    let c = if cock {
        &coat::ROOSTERS[..]
    } else {
        &coat::HENS[..]
    };
    let c = c[usize::from(variant) % c.len()];
    let rise = if cock { 0.405 } else { 0.35 };
    let height = if cock { 0.227 } else { 0.163 };
    let shoulder = Vec3::new(0., rise, -0.15);
    let mut body = Builder::with_resolution(21);
    let width = if cock { 0.185 } else { 0.169 };
    body.ellipsoid(
        Vec3::new(0., rise - 0.014, 0.025),
        Vec3::new(width, width * 0.91, if cock { 0.253 } else { 0.226 }),
        c.body,
    )?;
    body.ellipsoid(
        Vec3::new(0., rise + 0.002, -0.10),
        Vec3::new(width * 0.92, width * 0.92, 0.154),
        c.breast,
    )?;
    // The saddle overlaps the wing roots and covers the upper tail insertion.
    body.ellipsoid(
        Vec3::new(0., rise + 0.043, 0.148),
        Vec3::new(width * 0.84, 0.099, 0.127),
        c.body,
    )?;
    for side in [-1., 1.] {
        if cock {
            for row in 0..3 {
                let z = 0.071 + row as f32 * 0.039;
                segment(
                    &mut body,
                    Vec3::new(side * 0.074, rise + 0.126, z),
                    Vec3::new(side * (0.144 - row as f32 * 0.006), rise + 0.024, z + 0.087),
                    0.036,
                    0.031,
                    c.hackle,
                )?;
            }
        }
    }
    let mut pieces = vec![
        (Part::Body, Vec3::ZERO, body.finish()?),
        (Part::Head, shoulder, head(c, cock, height)?),
        (
            Part::Tail,
            Vec3::new(0., rise + 0.036, 0.19),
            tail(c, cock)?,
        ),
    ];
    let hip = hip(kind);
    for side in [-1., 1.] {
        pieces.push((
            Part::Wing(side),
            Vec3::new(side * (width - 0.025), rise + 0.035, -0.048),
            wing(c, side, cock)?,
        ));
        pieces.push((
            Part::Leg(side),
            Vec3::new(side * width * 0.55, hip.y, hip.z),
            leg(c, side, hip.y, cock)?,
        ));
    }
    Ok(pieces)
}

fn head(c: Coat, cock: bool, h: f32) -> Result<Mesh, String> {
    let mut b = Builder::with_cell_size(0.012);
    let nape = Vec3::new(0., h * 0.31, 0.022);
    b.ellipsoid(nape, Vec3::new(0.105, h * 0.65, 0.105), c.hackle)?;
    for side in [-1., 1.] {
        for i in 0..3 {
            let z = -0.074 + i as f32 * 0.047;
            let root = Vec3::new(side * (0.070 + i as f32 * 0.010), h * 0.78, z);
            let tip = Vec3::new(side * (0.099 + i as f32 * 0.006), 0.016, z + 0.024);
            segment(&mut b, root, tip, 0.033, 0.028, c.lacing)?;
            segment(
                &mut b,
                root + Vec3::X * side * 0.011,
                root.lerp(tip, 0.67) + Vec3::X * side * 0.013,
                0.019,
                0.025,
                c.hackle,
            )?;
        }
    }
    let skull = Vec3::new(0., h, -0.066);
    b.ellipsoid(skull, Vec3::new(0.094, 0.096, 0.098), c.hackle)?;
    for side in [-1., 1.] {
        b.ellipsoid(
            Vec3::new(side * 0.078, h - 0.004, -0.093),
            Vec3::new(0.022, 0.051, 0.058),
            COMB,
        )?;
        b.ellipsoid(
            Vec3::new(side * 0.088, h - 0.040, -0.061),
            Vec3::new(0.014, 0.026, 0.024),
            [0.77, 0.56, 0.38],
        )?;
        let eye = Vec3::new(side * 0.098, h + 0.016, -0.112);
        b.voxel(eye.to_array(), [0.017, 0.030, 0.034], [0.90, 0.48, 0.085])?;
        b.voxel(
            (eye + Vec3::new(side * 0.009, 0., -0.002)).to_array(),
            [0.010, 0.019, 0.021],
            [0.013, 0.012, 0.01],
        )?;
        let wattle = if cock { 0.056 } else { 0.032 };
        b.ellipsoid(
            Vec3::new(side * 0.026, h - 0.066, -0.115),
            Vec3::new(0.024, wattle, 0.032),
            COMB,
        )?;
    }
    for (i, (w, t, d)) in [
        (0.098, 0.055, 0.058),
        (0.066, 0.039, 0.045),
        (0.034, 0.023, 0.032),
    ]
    .into_iter()
    .enumerate()
    {
        b.rounded(
            [0., h - 0.018 - i as f32 * 0.007, -0.155 - i as f32 * 0.026],
            [w, t, d],
            0.007,
            if i == 2 { HORN } else { BEAK },
        )?;
    }
    let crown = h + 0.086;
    if c.rose_comb {
        b.rounded([0., crown, -0.066], [0.066, 0.048, 0.135], 0.014, COMB)?;
        for i in 0..3 {
            b.voxel(
                [0., crown + 0.025, -0.110 + i as f32 * 0.035],
                [0.039, 0.018, 0.022],
                COMB_LIGHT,
            )?;
        }
        segment(
            &mut b,
            Vec3::new(0., crown, -0.019),
            Vec3::new(0., crown + 0.022, 0.036),
            0.021,
            0.022,
            COMB,
        )?;
    } else {
        b.rounded(
            [0., crown - 0.005, -0.058],
            [0.044, 0.032, 0.153],
            0.009,
            COMB,
        )?;
        for (i, crest) in [0.65, 1., 0.88, 0.64].into_iter().enumerate() {
            let z = -0.115 + i as f32 * 0.041;
            let height = crest * if cock { 0.084 } else { 0.048 };
            segment(
                &mut b,
                Vec3::new(0., crown, z),
                Vec3::new(0., crown + height, z + 0.010),
                0.025,
                0.014,
                COMB_LIGHT,
            )?;
        }
    }
    b.finish()
}

fn wing(c: Coat, side: f32, cock: bool) -> Result<Mesh, String> {
    let mut b = Builder::with_cell_size(0.017);
    // Long overlapping primaries tuck under a contrasting row of coverts.
    for i in 0..5 {
        let t = i as f32 / 4.;
        let a = Vec3::new(
            side * (0.026 + t * 0.012),
            0.039 - t * 0.069,
            0.009 + t * 0.014,
        );
        let end = Vec3::new(
            side * (0.009 + t * 0.010),
            -0.025 - t * 0.092,
            0.285 - t * 0.059 + if cock { 0.036 } else { 0. },
        );
        segment(
            &mut b,
            a,
            end,
            0.041,
            0.044,
            if i % 2 == 0 { c.flights } else { c.lacing },
        )?;
        if c.barred {
            for u in [0.36, 0.60, 0.82] {
                segment(
                    &mut b,
                    a.lerp(end, u) + Vec3::X * side * 0.006,
                    a.lerp(end, u + 0.07) + Vec3::X * side * 0.006,
                    0.045,
                    0.044,
                    c.sheen,
                )?;
            }
        }
    }
    b.ellipsoid(
        Vec3::new(side * 0.024, 0.001, 0.041),
        Vec3::new(0.047, 0.066, 0.098),
        c.body,
    )?;
    for i in 0..4 {
        let t = i as f32 / 3.;
        let a = Vec3::new(side * (0.047 + t * 0.009), 0.042 - t * 0.078, 0.019);
        let tip = Vec3::new(side * 0.056, 0.006 - t * 0.092, 0.137 - t * 0.043);
        segment(&mut b, a, tip, 0.028, 0.033, c.lacing)?;
        segment(
            &mut b,
            a + Vec3::X * side * 0.009,
            a.lerp(tip, 0.7) + Vec3::X * side * 0.008,
            0.020,
            0.025,
            if c.barred { c.sheen } else { c.body },
        )?;
    }
    b.finish()
}

fn tail(c: Coat, cock: bool) -> Result<Mesh, String> {
    let mut b = Builder::with_cell_size(if cock { 0.022 } else { 0.015 });
    // The fan is short and upright on hens. On cocks the sickles arc
    // beyond it, taper and fall away from the back on both sides.
    for i in -2..=2i32 {
        let t = i as f32 * 0.5;
        segment(
            &mut b,
            Vec3::new(t * 0.012, 0., 0.),
            Vec3::new(t * 0.097, 0.147 - t.abs() * 0.044, 0.144 + t.abs() * 0.011),
            0.052,
            0.047,
            if i % 2 == 0 { c.tail } else { c.sheen },
        )?;
    }
    if cock {
        for side in [-1., 1.] {
            for plume in 0..2 {
                let t = plume as f32;
                let points = [
                    Vec3::new(side * 0.026, 0.039, 0.026),
                    Vec3::new(side * (0.047 + t * 0.027), 0.143 + t * 0.030, 0.095),
                    Vec3::new(side * (0.062 + t * 0.029), 0.222 + t * 0.047, 0.184),
                    Vec3::new(
                        side * (0.076 + t * 0.028),
                        0.236 + t * 0.040,
                        0.264 + t * 0.013,
                    ),
                    Vec3::new(
                        side * (0.087 + t * 0.028),
                        0.199 + t * 0.030,
                        0.323 + t * 0.019,
                    ),
                    Vec3::new(
                        side * (0.087 + t * 0.035),
                        0.121 + t * 0.025,
                        0.342 + t * 0.035,
                    ),
                ];
                for (i, ab) in points.windows(2).enumerate() {
                    let w = [0.046, 0.052, 0.046, 0.036, 0.023][i];
                    segment(
                        &mut b,
                        ab[0],
                        ab[1],
                        w,
                        w * 0.97,
                        if plume == 0 || i == 4 {
                            c.tail
                        } else {
                            c.sheen
                        },
                    )?;
                }
            }
        }
    }
    b.finish()
}

fn leg(c: Coat, side: f32, hip: f32, cock: bool) -> Result<Mesh, String> {
    let mut b = Builder::with_cell_size(0.013);
    let toe = Vec3::new(0., -hip + 0.016, -0.014);
    b.ellipsoid(
        Vec3::new(0., -0.014, 0.),
        Vec3::new(0.049, 0.07, 0.054),
        c.breast,
    )?;
    let hock = Vec3::new(0., -hip * 0.49, 0.028);
    segment(
        &mut b,
        Vec3::new(0., -0.025, -0.017),
        hock,
        0.040,
        0.042,
        SHANK,
    )?;
    segment(&mut b, hock, toe, 0.031, 0.033, SHANK)?;
    if cock {
        segment(
            &mut b,
            hock.lerp(toe, 0.34),
            hock + Vec3::new(side * 0.012, -0.012, 0.045),
            0.020,
            0.022,
            HORN,
        )?;
    }
    for i in [-1., 0., 1.] {
        let end = toe + Vec3::new(i * 0.050, 0., -0.097 + i.abs() * 0.023);
        segment(&mut b, toe, end, 0.021, 0.023, SHANK)?;
        segment(
            &mut b,
            end,
            end + Vec3::new(i * 0.006, -0.004, -0.018),
            0.014,
            0.014,
            HORN,
        )?;
    }
    segment(
        &mut b,
        toe,
        toe + Vec3::new(side * 0.015, 0., 0.072),
        0.021,
        0.019,
        SHANK,
    )?;
    b.voxel([0., -hip + 0.012, -0.014], [0.042, 0.024, 0.044], SHANK)?;
    b.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::mesh::VertexAttributeValues;
    use std::collections::HashSet;

    #[test]
    fn poultry_variants_have_distinct_plumage_seven_rigid_parts_and_grounded_feet() {
        for kind in [Kind::Hen, Kind::Rooster] {
            let mut coats = HashSet::new();
            for variant in 0..kind.variant_count() {
                let parts = super::super::parts_variant(kind, variant).unwrap();
                assert_eq!(parts.len(), 7);
                let mut fingerprint = 0u64;
                let mut vertices = 0;
                let mut top = f32::NEG_INFINITY;
                for (part, offset, mesh) in parts {
                    let Some(VertexAttributeValues::Float32x3(positions)) =
                        mesh.attribute(Mesh::ATTRIBUTE_POSITION)
                    else {
                        panic!("missing positions")
                    };
                    let Some(VertexAttributeValues::Float32x4(colors)) =
                        mesh.attribute(Mesh::ATTRIBUTE_COLOR)
                    else {
                        panic!("missing plumage")
                    };
                    assert!(
                        mesh.indices()
                            .unwrap()
                            .iter()
                            .all(|ix| ix < positions.len())
                    );
                    vertices += positions.len();
                    for (position, color) in positions.iter().zip(colors) {
                        top = top.max(position[1] + offset.y);
                        for v in color {
                            fingerprint =
                                fingerprint.wrapping_mul(0x100000001b3) ^ u64::from(v.to_bits());
                        }
                    }
                    if matches!(part, Part::Leg(_)) {
                        let bottom = positions
                            .iter()
                            .map(|p| p[1] + offset.y)
                            .fold(f32::INFINITY, f32::min);
                        assert!(bottom.abs() < 0.003, "{kind:?} foot y={bottom}");
                    }
                }
                assert!(
                    top > kind.size()[1] as f32 * 0.90 && top < kind.size()[1] as f32 * 1.08,
                    "{kind:?} height {top}"
                );
                assert!(
                    coats.insert(fingerprint),
                    "duplicate {kind:?} plumage {variant}"
                );
                assert!(
                    vertices < 16_000,
                    "{kind:?} variant {variant} has {vertices} vertices"
                );
            }
        }
    }
}
