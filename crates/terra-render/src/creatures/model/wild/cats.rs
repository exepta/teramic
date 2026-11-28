use super::*;

const INK: Tint = [0.095, 0.074, 0.065];
const CREAM: Tint = [0.91, 0.83, 0.66];
const PINK: Tint = [0.53, 0.28, 0.24];

struct Coat {
    kind: Kind,
    gold: Tint,
    chest: Tint,
    mane: Tint,
}

impl Coat {
    // The vertical rows stagger and warp along the flank. Jaguar rosettes have a
    // warm open center and missing arcs; tiger bars split and taper to the belly.
    fn pattern(&self, p: Vec3, tint: Tint) -> Tint {
        if tint != self.gold {
            return tint;
        }
        match self.kind {
            Kind::Tiger => {
                let row = p.z * 5.8 + (p.y * 7.3 + p.z * 3.1).sin() * 0.23 + p.x * 0.49;
                let line = (row - row.round()).abs();
                let fork = ((row + 0.27) - (row + 0.27).round()).abs();
                let width = (0.15 + (p.y - 0.63) * 0.21).clamp(0.055, 0.22);
                if line < width || (p.y * 9. + p.z * 3.4).sin() > 0.42 && fork < width * 0.47 {
                    INK
                } else {
                    tint
                }
            }
            Kind::Jaguar => {
                let z = p.z * 4.1 + (p.x * 8. + p.y * 4.).sin() * 0.13;
                let row = z.floor();
                let flank = (p.y - 0.69).atan2(p.x.abs().max(0.05));
                let y = flank * 1.58 + row * 0.49 + (p.z * 15.).sin() * 0.10;
                let dx = z.rem_euclid(1.) - 0.5;
                let dy = y.rem_euclid(1.) - 0.5;
                let oval = dx * dx * 1.07 + dy * dy * 0.85;
                if p.y < 0.49 {
                    return if (p.y * 9. + p.z * 4.).sin() > 0.48
                        && (p.z * 29. + p.x * 5.).cos() > 0.26
                    {
                        INK
                    } else {
                        tint
                    };
                }
                if (0.043..0.152).contains(&oval) && !(dx < -0.11 && dy > 0.20)
                    || oval < 0.011 && row.rem_euclid(3.) < 1.
                {
                    INK
                } else if oval <= 0.043 {
                    [0.71, 0.38, 0.12]
                } else {
                    tint
                }
            }
            _ => tint,
        }
    }
}

// Express the pattern in metre-space across the whole animal.
fn skin(b: Builder, offset: Vec3, c: &Coat) -> Result<Mesh, String> {
    b.finish_painted(|p, tint| c.pattern(p + offset, tint))
}

pub(super) fn parts(kind: Kind) -> Result<Parts, String> {
    let c = Coat {
        kind,
        gold: match kind {
            Kind::Tiger => [0.86, 0.40, 0.105],
            Kind::Lion => [0.73, 0.53, 0.29],
            _ => [0.82, 0.57, 0.25],
        },
        chest: if kind == Kind::Lion {
            [0.81, 0.67, 0.46]
        } else {
            CREAM
        },
        mane: [0.43, 0.255, 0.13],
    };
    let mut body = Builder::with_cell_size(if kind == Kind::Jaguar { 0.042 } else { 0.054 });
    mass(&mut body, [0., 0.69, 0.025], [0.305, 0.266, 0.632], c.gold)?;
    mass(&mut body, [0., 0.73, -0.34], [0.352, 0.313, 0.355], c.gold)?;
    mass(&mut body, [0., 0.72, 0.436], [0.305, 0.292, 0.354], c.gold)?;
    mass(&mut body, [0., 0.91, -0.54], [0.276, 0.235, 0.253], c.gold)?;
    mass(&mut body, [0., 0.62, -0.46], [0.27, 0.22, 0.22], c.chest)?;
    for side in [-1., 1.] {
        mass(
            &mut body,
            [side * 0.241, 0.77, -0.29],
            [0.119, 0.26, 0.235],
            c.gold,
        )?;
        mass(
            &mut body,
            [side * 0.222, 0.75, 0.43],
            [0.113, 0.24, 0.242],
            c.gold,
        )?;
    }
    let head = super::CAT_HEAD;
    let mut out = vec![
        (Part::Body, Vec3::ZERO, skin(body, Vec3::ZERO, &c)?),
        (Part::Head, head, skull(&c)?),
    ];
    let mut jaw = Builder::with_resolution(11);
    jaw.rounded([0., -0.028, -0.099], [0.245, 0.082, 0.210], 0.032, c.chest)?;
    jaw.rounded(
        [0., 0.005, -0.093],
        [0.202, 0.028, 0.167],
        0.012,
        [0.51, 0.25, 0.25],
    )?;
    out.push((
        Part::Jaw,
        head + Vec3::new(0., -0.040, -0.223),
        jaw.finish()?,
    ));
    for hind in [false, true] {
        for side in [-1., 1.] {
            let pivot = Vec3::new(
                side * if hind { 0.212 } else { 0.230 },
                0.634,
                if hind { 0.43 } else { -0.38 },
            );
            out.push((
                Part::Leg(if hind { side } else { -side }),
                pivot,
                leg(&c, hind, pivot)?,
            ));
        }
    }
    let pivot = Vec3::new(0., 0.765, 0.66);
    out.push((Part::Tail, pivot, tail(&c)?));
    let scale = super::cat_scale(kind);
    Ok(out
        .into_iter()
        .map(|(part, pivot, mesh)| (part, pivot * scale, mesh.scaled_by(scale)))
        .collect())
}

fn skull(c: &Coat) -> Result<Mesh, String> {
    let mut b = Builder::with_resolution(24);
    if c.kind == Kind::Lion {
        mass(&mut b, [0., -0.027, 0.037], [0.420, 0.390, 0.268], c.mane)?;
        for i in -2i32..=2 {
            let x = i as f32 * 0.105;
            let y = 0.285 - (i.abs() as f32) * 0.028;
            mass(
                &mut b,
                [x, y, -0.076],
                [0.090, 0.063, 0.117],
                if i % 2 == 0 {
                    [0.51, 0.30, 0.14]
                } else {
                    [0.60, 0.37, 0.18]
                },
            )?;
        }
        for side in [-1., 1.] {
            for (x, y, z) in [
                (0.27, 0.13, 0.035),
                (0.29, 0.006, -0.019),
                (0.238, -0.13, -0.031),
                (0.14, -0.21, -0.041),
            ] {
                segment(
                    &mut b,
                    Vec3::new(side * x, y + 0.075, z),
                    Vec3::new(side * (x + 0.096), y - 0.121, z - 0.007),
                    0.113,
                    0.106,
                    if y > 0.0 {
                        [0.57, 0.35, 0.17]
                    } else {
                        [0.37, 0.22, 0.105]
                    },
                )?;
            }
        }
    }
    let wide = if c.kind == Kind::Jaguar { 0.276 } else { 0.247 };
    mass(&mut b, [0., 0.046, -0.091], [wide, 0.198, 0.240], c.gold)?;
    mass(&mut b, [0., 0.097, -0.199], [0.190, 0.145, 0.158], c.gold)?;
    for side in [-1., 1.] {
        if c.kind != Kind::Lion {
            // Cheek bars sweep back from the pale orbital patch; the crown
            // and snout retain distinct markings at typical viewing distance.
            for (y, z) in [(0.087, -0.210), (0.042, -0.174), (-0.017, -0.136)] {
                segment(
                    &mut b,
                    Vec3::new(side * 0.171, y + 0.017, z - 0.018),
                    Vec3::new(side * (wide - 0.006), y + 0.027, z + 0.065),
                    0.033,
                    0.042,
                    INK,
                )?;
            }
            b.rotated(
                [side * 0.119, 0.161, -0.262],
                [0.118, 0.037, 0.063],
                CREAM,
                Quat::from_rotation_z(side * 0.16),
            )?;
            for row in 0..2 {
                let z = -0.203 + row as f32 * 0.084;
                segment(
                    &mut b,
                    Vec3::new(side * 0.033, 0.228, z),
                    Vec3::new(side * 0.119, 0.217, z + 0.038),
                    0.034,
                    0.034,
                    INK,
                )?;
            }
        }
        let ear = Vec3::new(side * 0.184, 0.236, -0.007);
        mass(&mut b, ear.to_array(), [0.082, 0.092, 0.067], INK)?;
        mass(
            &mut b,
            (ear + Vec3::new(0., 0.004, -0.040)).to_array(),
            [0.062, 0.060, 0.039],
            c.gold,
        )?;
        mass(
            &mut b,
            (ear + Vec3::new(0., 0.012, 0.053)).to_array(),
            [0.046, 0.037, 0.021],
            if c.kind == Kind::Lion { c.mane } else { CREAM },
        )?;
        mass(
            &mut b,
            [side * 0.209, -0.006, -0.128],
            [0.064, 0.116, 0.122],
            if c.kind == Kind::Tiger { CREAM } else { c.gold },
        )?;
        b.rotated(
            [side * 0.149, 0.081, -0.292],
            [0.112, 0.062, 0.063],
            INK,
            Quat::from_rotation_z(side * 0.10),
        )?;
        b.voxel(
            [side * 0.149, 0.085, -0.326],
            [0.069, 0.042, 0.022],
            [0.80, 0.65, 0.20],
        )?;
        b.voxel([side * 0.146, 0.086, -0.339], [0.027, 0.037, 0.014], INK)?;
        mass(
            &mut b,
            [side * 0.076, -0.008, -0.329],
            [0.096, 0.077, 0.096],
            c.chest,
        )?;
        for (y, z) in [(-0.002, -0.374), (-0.025, -0.347)] {
            b.voxel([side * 0.148, y, z], [0.017, 0.017, 0.022], INK)?;
        }
    }
    b.rounded(
        [0., 0.044, -0.382],
        [0.128, 0.065, 0.070],
        0.021,
        if c.kind == Kind::Jaguar { INK } else { PINK },
    )?;
    b.finish()
}

fn leg(c: &Coat, hind: bool, pivot: Vec3) -> Result<Mesh, String> {
    let mut b = Builder::with_resolution(20);
    let knee = Vec3::new(0., -0.207, if hind { -0.118 } else { 0.045 });
    let hock = Vec3::new(0., -0.422, if hind { 0.091 } else { 0.024 });
    let ankle = Vec3::new(0., -0.56, -0.014);
    mass(
        &mut b,
        [0., -0.032, 0.],
        [0.126, 0.209, if hind { 0.170 } else { 0.126 }],
        c.gold,
    )?;
    segment(&mut b, Vec3::ZERO, knee, 0.202, 0.219, c.gold)?;
    segment(
        &mut b,
        knee,
        hock,
        0.150,
        if hind { 0.157 } else { 0.140 },
        c.gold,
    )?;
    segment(&mut b, hock, ankle, 0.112, 0.122, c.gold)?;
    b.rounded([0., -0.581, -0.048], [0.186, 0.099, 0.177], 0.040, c.gold)?;
    for toe in -1..=1 {
        b.rounded(
            [
                toe as f32 * 0.065,
                -0.601,
                -0.131 - f32::from(toe == 0) * 0.022,
            ],
            [0.066, 0.066, 0.090],
            0.021,
            if c.kind == Kind::Tiger { CREAM } else { c.gold },
        )?;
    }
    skin(b, pivot, c)
}

fn tail(c: &Coat) -> Result<Mesh, String> {
    let mut b = Builder::with_cell_size(0.042);
    let mut p = Vec3::ZERO;
    for i in 0..8 {
        let t = (i + 1) as f32 / 8.;
        let q = Vec3::new(0.17 * (t * 2.7).sin(), -0.39 * (t * 2.90).sin(), t * 1.18);
        let w = 0.12 - 0.055 * t;
        segment(
            &mut b,
            p,
            q,
            w,
            w,
            if c.kind == Kind::Lion || i % 2 == 0 && i < 7 {
                c.gold
            } else {
                INK
            },
        )?;
        p = q;
    }
    if c.kind == Kind::Lion {
        mass(&mut b, p.to_array(), [0.081, 0.084, 0.119], c.mane)?;
    }
    b.finish()
}
