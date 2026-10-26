use super::{TexturePattern, hash};

pub(super) fn sample(pattern: TexturePattern, px: u32, py: u32) -> Option<[f32; 3]> {
    let (x, y) = (px % 512, py % 512);
    let grain = hash(x, y) as f32 / u32::MAX as f32;
    let (base, shade) = match pattern {
        TexturePattern::Wool => {
            let row = y / 16;
            let lane = (x + (row % 2) * 8) % 512;
            let u = lane % 16;
            let v = y % 16;
            let seed = hash(lane / 16, row);
            let seam = u < 2 || v < 2;
            let loop_ridge = (3..=5).contains(&u) || (9..=12).contains(&u);
            let valley = (7..=8).contains(&u) && (3..=12).contains(&v);
            let strand = (u + v / 4).is_multiple_of(4);
            let shade = 0.92
                + (seed % 7) as f32 * 0.008
                + grain * 0.025
                + if loop_ridge && v > 3 && v < 13 {
                    0.075
                } else {
                    0.0
                }
                - if seam {
                    0.15
                } else if valley {
                    0.105
                } else {
                    0.0
                }
                - if strand { 0.025 } else { 0.0 };
            ([246.0, 243.0, 234.0], shade)
        }
        TexturePattern::PathTop | TexturePattern::PathSide => {
            let cell = [x / 32, y / 32];
            let seed = hash(cell[0], cell[1]);
            let offset_x = 5 + seed % 13;
            let offset_y = 6 + seed.rotate_left(9) % 11;
            let dx = (x % 32) as i32 - offset_x as i32;
            let dy = (y % 32) as i32 - offset_y as i32;
            let pebble = seed.is_multiple_of(7)
                && dx.abs() <= 6
                && dy.abs() <= 5
                && dx.abs() + dy.abs() <= 9;
            let side = pattern == TexturePattern::PathSide;
            let crack = hash(x / 24, y / 20).is_multiple_of(9) && (x % 24 < 2 || y % 20 < 2);
            let base = if pebble {
                [181.0, 176.0, 155.0]
            } else if side && y > 96 {
                [137.0, 94.0, 54.0]
            } else {
                [201.0, 151.0, 76.0]
            };
            (
                base,
                0.9 + (seed % 9) as f32 * 0.012 + grain * 0.06 - if crack { 0.16 } else { 0.0 }
                    + if pebble && dy < -2 { 0.1 } else { 0.0 },
            )
        }
        _ => return None,
    };
    Some(base.map(|c| (c * shade).clamp(0.0, 255.0)))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn woven_loops_and_compacted_path_are_deterministic_tileable_and_not_the_same_material() {
        for pattern in [
            TexturePattern::Wool,
            TexturePattern::PathTop,
            TexturePattern::PathSide,
        ] {
            let mut low: f32 = 255.0;
            let mut high: f32 = 0.0;
            for y in (0..512).step_by(7) {
                for x in (0..512).step_by(7) {
                    let color = sample(pattern, x, y).unwrap();
                    assert_eq!(sample(pattern, x + 512, y + 512), Some(color));
                    assert!(
                        color
                            .iter()
                            .all(|v| v.is_finite() && (0.0..=255.0).contains(v))
                    );
                    low = low.min(color[0]);
                    high = high.max(color[0]);
                    if pattern == TexturePattern::Wool {
                        assert!((color[0] - color[2]).abs() < 15.0);
                    }
                }
            }
            assert!(high - low > 25.0);
        }
        assert_ne!(
            sample(TexturePattern::Wool, 99, 211),
            sample(TexturePattern::PathTop, 99, 211)
        );
    }
}
