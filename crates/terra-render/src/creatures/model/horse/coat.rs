//! Stable coat indices; patterns are painted onto the continuous body surface.
pub(super) fn coat(variant: u8, p: [f32; 3]) -> [f32; 3] {
    let base: [f32; 3] = [
        [0.47, 0.29, 0.16],
        [0.10, 0.105, 0.11],
        [0.69, 0.40, 0.21],
        [0.91, 0.90, 0.85],
        [0.48, 0.51, 0.53],
        [0.72, 0.56, 0.32],
        [0.80, 0.66, 0.40],
        [0.58, 0.43, 0.35],
        [0.10, 0.105, 0.11],
        [0.69, 0.40, 0.21],
        [0.50, 0.31, 0.17],
        [0.10, 0.105, 0.11],
        [0.88, 0.86, 0.76],
        [0.65, 0.68, 0.68],
        [0.89, 0.82, 0.61],
        [0.39, 0.25, 0.14],
    ][usize::from(variant.min(15))];
    let cell = p.map(|v| (v * 9.0).floor() as i32);
    let hash = cell[0].wrapping_mul(73_856_093)
        ^ cell[1].wrapping_mul(19_349_663)
        ^ cell[2].wrapping_mul(83_492_791);
    let patch = (p[2] * 5.4 + p[1] * 2.8).sin() + (p[1] * 7.5 - p[0] * 4.0).cos() * 0.65;
    if matches!(variant, 10 | 11) && patch > 0.15 && p[2] > -0.58
        || matches!(variant, 2 | 3 | 6 | 8 | 9 | 10 | 11) && p[1] < 0.29
    {
        return [0.91, 0.90, 0.84];
    }
    if variant == 12 && hash.rem_euclid(7) < 2 {
        return [0.22, 0.12, 0.08];
    }
    // A few broad contiguous patches, not random colors on every voxel. The
    // three levels retain material readability and large greedy rectangles.
    let field = (p[2] * 8.0 + p[0].abs() * 1.5).sin() + (p[1] * 7.5 + p[2] * 2.0).cos() * 0.65;
    let coat_tone = if field > 0.65 {
        1.06
    } else if field < -0.65 {
        0.94
    } else {
        1.0
    };
    let tint = if variant == 4 && hash.rem_euclid(5) < 2 {
        1.33
    } else if variant == 7 && hash.rem_euclid(4) == 0 {
        1.12
    } else {
        coat_tone
    };
    base.map(|v| (v * tint).min(1.0))
}
