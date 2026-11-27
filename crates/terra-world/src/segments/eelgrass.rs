//! Square-edged blades on a sixteenth-block grid. Steps are offset from metre
//! boundaries so neighbouring sections share exactly the same cross-section.
use super::Profile;

type Rect = [i32; 4]; // x0, z0, x1, z1 in sixteenths of a block

fn shelf(
    rect: Rect,
    adjacent: Option<Rect>,
    y: f32,
    top: bool,
    color: [f32; 3],
    emit: &mut impl FnMut([[f32; 3]; 4], [f32; 3]),
) {
    let [x, z, r, b] = rect;
    let mut cap = |[x, z, r, b]: Rect| {
        if r <= x || b <= z {
            return;
        }
        let mut quad =
            [[x, z], [x, b], [r, b], [r, z]].map(|[x, z]| [x as f32 / 16., y, z as f32 / 16.]);
        if !top {
            quad.reverse();
        }
        emit(quad, color);
    };
    if let Some([l, t, u, v]) = adjacent {
        let (l, t, u, v) = (l.max(x), t.max(z), u.min(r), v.min(b));
        if l < u && t < v {
            for strip in [[x, z, l, b], [u, z, r, b], [l, z, u, t], [l, v, u, b]] {
                cap(strip);
            }
            return;
        }
    }
    cap(rect);
}

pub(super) fn visit(
    seed: u64,
    profile: Profile,
    _coarse: bool,
    emit: &mut impl FnMut([[f32; 3]; 3], [f32; 3]),
) {
    let base = i32::from(profile.below) * 16;
    let total = base + (1 + i32::from(profile.above)) * 16;
    blades(seed, base, total, 16, 4, 8, &mut |q, rgb| {
        emit([q[0], q[1], q[2]], rgb);
        emit([q[0], q[2], q[3]], rgb);
    });
}

pub(crate) fn rosette(
    variant: usize,
    level: crate::props::visual::PropDetail,
    emit: &mut impl FnMut([[f32; 3]; 4], [f32; 3]),
) {
    let total = 40 + (variant % 2) as i32 * 8;
    let (count, step) = match level {
        crate::props::visual::PropDetail::Near => (5, 4),
        crate::props::visual::PropDetail::Middle => (4, 8),
        crate::props::visual::PropDetail::Far => (3, total),
    };
    blades(
        super::seed([variant as i64, 0, -19]),
        0,
        total,
        total,
        count,
        step,
        emit,
    );
}

fn blades(
    seed: u64,
    base: i32,
    total: i32,
    span: i32,
    count: usize,
    step: i32,
    emit: &mut impl FnMut([[f32; 3]; 4], [f32; 3]),
) {
    let phase = (seed & 1023) as f32 * (std::f32::consts::TAU / 1024.);
    for blade in 0..count {
        let variation = ((seed >> (blade * 7 + 7)) & 63) as f32 / 63.;
        let height = if blade == 0 {
            total
        } else {
            (total as f32 * (0.45 + 0.48 * variation)).round() as i32
        };
        let end = (height - base).min(span);
        if end <= 0 {
            continue;
        }
        let angle = phase + blade as f32 * 2.39996;
        let (z, x) = angle.sin_cos();
        let cross_section = |y: i32| {
            let band = (y + step / 2).div_euclid(step) * step;
            let t = (band as f32 / height as f32).clamp(0., 1.);
            let bend = 2.6 * t + 1.25 * (angle + t * 4.4).sin() * t;
            let cx = (8. + x * (1.7 + bend) + phase.cos() * 1.75 * t * t).round() as i32;
            let cz = (8. + z * (1.7 + bend) + phase.sin() * 1.75 * t * t).round() as i32;
            let width = if t < 0.72 { 3 } else { 2 };
            let (w, d) = if x.abs() < z.abs() {
                (width, 2)
            } else {
                (2, width)
            };
            [cx - 1, cz - 1, cx - 1 + w, cz - 1 + d]
        };
        let mut lo = 0;
        while lo < end {
            let rect = cross_section(base + lo);
            let mut hi = lo + 1;
            while hi < end && cross_section(base + hi) == rect {
                hi += 1;
            }
            let [x, z, r, b] = rect.map(|i| i as f32 / 16.);
            let (y, top) = (lo as f32 / 16., hi as f32 / 16.);
            let tone = 0.56 + 0.27 * (base + hi) as f32 / height as f32 + 0.12 * variation;
            let color = [tone * 0.78, tone, tone * 0.69];
            for (a, c) in [
                ([x, z], [x, b]),
                ([x, b], [r, b]),
                ([r, b], [r, z]),
                ([r, z], [x, z]),
            ] {
                emit(
                    [
                        [a[0], y, a[1]],
                        [c[0], y, c[1]],
                        [c[0], top, c[1]],
                        [a[0], top, a[1]],
                    ],
                    color,
                );
            }
            if base + lo == 0 || lo > 0 {
                shelf(
                    rect,
                    (base + lo > 0).then(|| cross_section(base + lo - 1)),
                    y,
                    false,
                    color,
                    emit,
                );
            }
            if base + hi == height || hi < end {
                shelf(
                    rect,
                    (base + hi < height).then(|| cross_section(base + hi)),
                    top,
                    true,
                    color,
                    emit,
                );
            }
            lo = hi;
        }
    }
}
