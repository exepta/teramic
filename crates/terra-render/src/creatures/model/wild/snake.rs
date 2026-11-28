use super::*;

pub(in crate::creatures) const COUNT: u8 = 9;
pub(in crate::creatures) const START: f32 = -0.67;
pub(in crate::creatures) const STEP: f32 = 0.172;

#[derive(Clone, Copy)]
struct Skin {
    back: Tint,
    mark: Tint,
    edge: Tint,
    belly: Tint,
}

fn rgb(hex: u32) -> Tint {
    [
        ((hex >> 16) & 255) as f32 / 255.,
        ((hex >> 8) & 255) as f32 / 255.,
        (hex & 255) as f32 / 255.,
    ]
}

fn skin(variant: u8) -> Skin {
    let [back, mark, edge, belly] = match variant {
        0 => [0x69764c, 0x343b27, 0xe8c95c, 0xbfc39a], // grass snake, paired pale neck patches
        1 => [0x967e65, 0x352e2b, 0xb9a480, 0xbfafa0], // common adder
        2 => [0xc9a668, 0x785b3b, 0xe2c88c, 0xd1ba88], // sand viper
        3 => [0xbe3628, 0x171b1b, 0xf7da83, 0xd48351], // coral snake
        4 => [0x3c9456, 0xebead1, 0x287748, 0xb6d482], // emerald tree boa
        5 => [0xb79b54, 0x39302c, 0xe3cb83, 0xd6be8c], // reticulated python
        6 => [0x242831, 0xf4e4bd, 0x9d8f7e, 0xb6a891], // banded kingsnake
        7 => [0xb98965, 0x775039, 0xd5a57a, 0xd5b899], // copperhead
        _ => [0x687773, 0x455651, 0x84928b, 0xb8bfb0], // black mamba
    }
    .map(rgb);
    Skin {
        back,
        mark,
        edge,
        belly,
    }
}

fn pigment(c: Skin, variant: u8, p: Vec3, r: f32) -> Tint {
    if p.y < -r * 0.28 {
        return c.belly;
    }
    let side = (p.x / r).abs().clamp(0., 1.);
    let length = p.z - START;
    let band = (length * 4.5).rem_euclid(1.);
    let diamond = ((length * 5.2).rem_euclid(1.) - 0.5).abs() * 2.;
    match variant {
        0 if length < 0.085 && side > 0.27 => c.edge,
        0 if band < 0.20 && side > 0.55 => c.mark,
        1 if ((p.x / r) - (diamond - 0.47) * 1.38).abs() < 0.29 => c.mark,
        2 if diamond + side * 0.86 < 0.79 => c.mark,
        2 if diamond + side * 0.86 < 1.01 => c.edge,
        3 if !(0.19..0.80).contains(&band) => c.mark,
        3 if !(0.27..0.73).contains(&band) => c.edge,
        4 if (band - side * 0.48).abs() < 0.15 && side < 0.88 => c.mark,
        5 if (diamond - side).abs() < 0.20 => c.mark,
        5 if (diamond - side).abs() < 0.34 => c.edge,
        6 if band < 0.24 => c.mark,
        7 if diamond < 0.20 + side * 0.62 => c.mark,
        8 if side > 0.54 && p.y < r * 0.14 => c.mark,
        _ => c.back,
    }
}

fn radius(t: f32, variant: u8) -> f32 {
    let bulk = match variant {
        2 | 5 | 7 => 1.13,
        0 | 3 | 8 => 0.89,
        _ => 1.,
    };
    let envelope = if t < 0.29 {
        0.58 + 0.42 * t / 0.29
    } else {
        1. - ((t - 0.29) / 0.71).powf(1.8) * 0.88
    };
    0.101 * bulk * envelope
}

pub(super) fn parts(variant: u8) -> Result<Parts, String> {
    let v = variant % 9;
    let c = skin(v);
    let viper = matches!(v, 1 | 2 | 7);
    let mut head = Builder::with_cell_size(0.0125);
    mass(
        &mut head,
        [0., 0., -0.064],
        [if viper { 0.113 } else { 0.093 }, 0.071, 0.114],
        c.back,
    )?;
    head.rounded(
        [0., -0.011, -0.132],
        [if viper { 0.124 } else { 0.144 }, 0.086, 0.124],
        0.033,
        c.back,
    )?;
    head.rounded([0., -0.039, -0.128], [0.13, 0.040, 0.124], 0.017, c.belly)?;
    for side in [-1., 1.] {
        let x = if viper { 0.094 } else { 0.079 };
        if viper {
            mass(
                &mut head,
                [side * 0.073, 0., -0.035],
                [0.050, 0.051, 0.072],
                c.back,
            )?;
        }
        head.rounded(
            [side * x, 0.012, -0.09],
            [0.029, 0.038, 0.097],
            0.009,
            c.mark,
        )?;
        head.voxel(
            [side * (x + 0.009), 0.024, -0.114],
            [0.023, 0.032, 0.038],
            [0.91, 0.70, 0.31],
        )?;
        head.voxel(
            [side * (x + 0.019), 0.025, -0.114],
            [0.01, 0.025, if viper { 0.011 } else { 0.017 }],
            [0.042, 0.035, 0.027],
        )?;
        head.voxel([side * 0.052, 0.022, -0.175], [0.019, 0.013, 0.018], c.mark)?;
    }
    let mut tongue = Builder::with_cell_size(0.006);
    let root = Vec3::new(0., -0.021, -0.162);
    let fork = root + Vec3::new(0., -0.008, -0.071);
    let pink = [0.51, 0.18, 0.25];
    segment(&mut tongue, root, fork, 0.015, 0.009, pink)?;
    for side in [-1., 1.] {
        segment(
            &mut tongue,
            fork,
            fork + Vec3::new(side * 0.026, 0.004, -0.050),
            0.009,
            0.008,
            pink,
        )?;
    }
    let pivot = Vec3::new(0., 0.138, START);
    let mut parts = vec![
        (Part::Head, pivot, head.finish()?),
        (Part::SnakeTongue, pivot, tongue.finish()?),
    ];
    for i in 0..COUNT {
        let t = (f32::from(i) + 0.5) / f32::from(COUNT);
        let r = radius(t, v);
        let offset = Vec3::new(0., r * 0.70, START + (f32::from(i) + 0.5) * STEP);
        let mut b = Builder::with_cell_size(0.018);
        for row in 0..4 {
            let u = (f32::from(i) + (row as f32 + 0.5) / 4.) / f32::from(COUNT);
            let rad = radius(u, v);
            b.rounded(
                [0., 0., ((row as f32 + 0.5) / 4. - 0.5) * STEP],
                [rad * 2., rad * 1.40, STEP * 0.37],
                rad * 0.52,
                c.back,
            )?;
        }
        let mesh =
            b.finish_painted(|p, _| pigment(c, v, Vec3::new(p.x, p.y, p.z + offset.z), r))?;
        parts.push((Part::ReptileJoint(i), offset, mesh));
    }
    Ok(parts)
}
