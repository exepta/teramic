use super::*;

pub(super) fn meshes(variant: u8) -> Result<(Mesh, Mesh), String> {
    let mut head = Builder::default();
    let skin = coat(variant, [0.0, 1.60, -0.72]);
    // The cheek and long, downward-sloping bridge connect to a broad muzzle.
    head.ellipsoid(
        Vec3::new(0.0, 0.055, -0.045),
        Vec3::new(0.153, 0.19, 0.175),
        skin,
    )?;
    shape::segment(
        &mut head,
        Vec3::new(0.0, 0.08, -0.09),
        Vec3::new(0.0, -0.13, -0.36),
        0.245,
        0.24,
        skin,
    )?;
    let muzzle = if variant == ZEBRA_COAT {
        [0.14, 0.135, 0.13]
    } else if matches!(variant, 0..=4 | 7..=14) {
        [0.83, 0.68, 0.55]
    } else {
        coat(variant, [0.0, 1.4, -1.03])
    };
    shape::rounded(
        &mut head,
        [0.0, -0.14, -0.402],
        [0.27, 0.185, 0.18],
        0.038,
        muzzle,
    )?;
    head.voxel(
        [0.0, -0.22, -0.478],
        [0.19, 0.012, 0.024],
        muzzle.map(|v| v * 0.55),
    )?;
    for side in [-1.0, 1.0] {
        // Tapered ears remain inside the original two-metre standing profile.
        for (y, width, depth) in [
            (0.24, 0.092, 0.11),
            (0.31, 0.075, 0.085),
            (0.3625, 0.045, 0.055),
        ] {
            head.voxel([side * 0.105, y, 0.005], [width, 0.075, depth], skin)?;
        }
        head.voxel(
            [side * 0.105, 0.29, -0.052],
            [0.036, 0.12, 0.012],
            [0.39, 0.25, 0.22],
        )?;
        // Raised socket sits behind the independently rastered 12-pixel eye.
        shape::rounded(
            &mut head,
            [side * 0.151, 0.095, -0.10],
            [0.055, 0.13, 0.135],
            0.014,
            skin.map(|v| v * 0.72),
        )?;
        head.voxel(
            [side * 0.105, -0.122, -0.494],
            [0.045, 0.038, 0.012],
            [0.13, 0.095, 0.075],
        )?;
    }
    if matches!(variant, 0 | 1 | 9..=12) {
        for (y, z, length) in [
            (0.09, -0.226, 0.075),
            (0.025, -0.278, 0.075),
            (-0.045, -0.337, 0.075),
        ] {
            head.voxel(
                [0.0, y, z],
                [if variant == 9 { 0.075 } else { 0.045 }, length, 0.022],
                [0.94, 0.92, 0.85],
            )?;
        }
    }
    // Thick layered forelock rather than a perfectly straight central strip.
    for i in 0..3 {
        head.voxel(
            [
                (i as f32 - 1.0) * 0.065,
                0.235 - (i % 2) as f32 * 0.035,
                -0.10,
            ],
            [0.085, 0.115, 0.14],
            mane(variant),
        )?;
    }
    let specs = [-1.0, 1.0].map(|side| eyes::Eye {
        center: Vec3::new(side * 0.185, 0.095, -0.10),
        width: 0.105,
        side,
        skin,
    });
    let mesh = if variant == ZEBRA_COAT {
        head.finish_painted(|p, tint| {
            if tint == skin {
                coat(variant, (p + HEAD).to_array())
            } else {
                tint
            }
        })?
    } else {
        head.finish()?
    };
    eyes::attach(mesh, &specs)
}
