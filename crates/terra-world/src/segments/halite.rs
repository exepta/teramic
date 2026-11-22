//! A common cleavage angle and staggered prism terminations give each salt
//! crystal the same habit as the small, authored halite outcrops.
use super::Profile;

pub(super) fn visit(
    profile: Profile,
    seed: u64,
    coarse: bool,
    emit: &mut impl FnMut([[f32; 3]; 3], [f32; 3]),
) {
    let angle = (seed & 255) as f32 * std::f32::consts::TAU / 256.;
    let (s, c) = angle.sin_cos();
    let full = f32::from(profile.below) + 1. + f32::from(profile.above);
    let start = f32::from(if profile.hanging {
        profile.above
    } else {
        profile.below
    });
    // Each shoot grows from its own footing and ends with a short bevel.
    let shoots = [
        (-0.04, -0.035, 0.204, 1.00),
        (0.205, 0.072, 0.117, 0.73),
        (-0.163, 0.167, 0.101, 0.56),
        (0.062, -0.208, 0.103, 0.40),
    ];
    let outline = [
        (-1., -0.71),
        (-0.71, -1.),
        (0.71, -1.),
        (1., -0.71),
        (1., 0.71),
        (0.71, 1.),
        (-0.71, 1.),
        (-1., 0.71),
    ];
    for (shoot, (cx, cz, width, fraction)) in shoots.into_iter().enumerate() {
        let tip = full * fraction;
        if start >= tip {
            continue;
        }
        let end = (start + 1.).min(tip);
        let bevel = tip - (tip * 0.22).min(0.49);
        let point = |height: f32, i: usize| {
            let t = height / tip;
            let spread = 1. + t * 0.22;
            let shoulder = 1. - 0.14 * t - 0.47 * ((height - bevel) / (tip - bevel)).clamp(0., 1.);
            let (a, b) = outline[i];
            let x = cx * spread + width * a * shoulder;
            let z = cz * spread + width * 0.82 * b * shoulder;
            let y = height - start;
            [
                0.5 + c * x - s * z,
                if profile.hanging { 1. - y } else { y },
                0.5 + s * x + c * z,
            ]
        };
        let mut levels = [start; 4];
        let mut nlevels = 1;
        if !coarse && full > 1. && shoot == 0 && start + 0.4 < end.min(bevel) {
            levels[nlevels] = start + 0.4;
            nlevels += 1;
        }
        if bevel > start && bevel < end {
            levels[nlevels] = bevel;
            nlevels += 1;
        }
        levels[nlevels] = end;
        let mut polygon = |mut triangle: [[f32; 3]; 3], color| {
            if profile.hanging {
                triangle.swap(1, 2);
            }
            emit(triangle, color);
        };
        for i in 0..8 {
            let j = (i + 1) % 8;
            let tint = 0.70 + ((i + (seed >> 10) as usize + shoot) % 5) as f32 * 0.067;
            let color = if shoot == 1 || i == 4 {
                [tint, tint * 0.84, tint * 0.72]
            } else {
                [tint, tint * 0.93, tint]
            };
            for layers in levels[..=nlevels].windows(2) {
                let [a, b, c, d] = [
                    point(layers[0], i),
                    point(layers[0], j),
                    point(layers[1], j),
                    point(layers[1], i),
                ];
                polygon([a, d, b], color);
                polygon([d, c, b], color);
            }
            // Triangulate the planar bevel face as a fan about a boundary
            // vertex to keep its silhouette identical at every viewing distance.
            if i > 0 && i < 7 {
                if start == 0. {
                    polygon([point(0., 0), point(0., i), point(0., j)], color);
                }
                if end == tip {
                    polygon([point(tip, 0), point(tip, j), point(tip, i)], color);
                }
            }
        }
    }
}
