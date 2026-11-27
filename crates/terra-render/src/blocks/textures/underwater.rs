use super::{TexturePattern, hash};

pub(super) fn sample(pattern: TexturePattern, px: u32, py: u32) -> Option<[f32; 3]> {
    let (px, py) = (px % 512, py % 512);
    let (x, y) = (px / 8, py / 8);
    let seed = hash(x / 3, y / 3);
    let grain = (hash(px / 2, py / 2) % 11) as f32 * 0.002;
    let (base, shade) = if pattern == TexturePattern::Ice {
        // Interpolate the trapped frost on a periodic lattice so neighbouring
        // cubes share their edges even across large greedy faces.
        let frost_knot = |dx, dy| (hash((x / 16 + dx) % 4, (y / 16 + dy) % 4) % 101) as f32 * 0.01;
        let u = (x % 16) as f32 / 16.0;
        let v = (y % 16) as f32 / 16.0;
        let low = frost_knot(0, 0) * (1.0 - u) + frost_knot(1, 0) * u;
        let high = frost_knot(0, 1) * (1.0 - u) + frost_knot(1, 1) * u;
        let frost = low * (1.0 - v) + high * v;
        let seam = (x + y + (frost * 13.0) as u32) % 64;
        let air = hash(x / 5, y / 5).is_multiple_of(9) && x % 5 == 2 && y % 5 == 3;
        let base = if seam <= 1 || air {
            [217.0, 239.0, 249.0]
        } else {
            [
                145.0 + 65.0 * frost,
                192.0 + 39.0 * frost,
                216.0 + 26.0 * frost,
            ]
        };
        (base, 0.97 + grain)
    } else if pattern == TexturePattern::Mud {
        let patch = hash(x / 7, y / 7);
        let pebble = patch % 13 < 2 && (2..=4).contains(&(x % 7)) && (2..=4).contains(&(y % 7));
        let moss = patch % 17 < 2 && (x % 7 + y % 7) < 5;
        let base = if pebble {
            [161.0, 151.0, 132.0]
        } else if moss {
            [101.0, 119.0, 61.0]
        } else {
            [108.0, 83.0, 59.0]
        };
        (base, 0.82 + (seed % 17) as f32 * 0.013 + grain)
    } else {
        let base = match pattern {
            TexturePattern::CoralRed => [212.0, 65.0, 83.0],
            TexturePattern::CoralBlue => [47.0, 116.0, 209.0],
            TexturePattern::CoralYellow => [233.0, 170.0, 39.0],
            TexturePattern::CoralPurple => [147.0, 67.0, 201.0],
            _ => return None,
        };
        let dx = (x % 16).abs_diff(8);
        let dy = (y % 16).abs_diff(8);
        let petal = match pattern {
            TexturePattern::CoralRed => (dx <= 2 && dy <= 6) || (dx <= 6 && dy <= 2),
            TexturePattern::CoralBlue => (3..=5).contains(&dx.max(dy)),
            TexturePattern::CoralYellow => (y + (x / 16) * 3) % 12 <= 2 && dx < 6,
            _ => dx + dy < 7 && dx.max(dy) >= 2,
        };
        let cavity = pattern == TexturePattern::CoralBlue && dx.max(dy) < 3;
        let polyp = dx < 2 && dy < 2 && pattern != TexturePattern::CoralBlue;
        let base = if polyp {
            base.map(|v| v * 0.48 + 239.0 * 0.52)
        } else if petal {
            base.map(|v| v * 0.77 + 241.0 * 0.23)
        } else {
            base
        };
        let edge = if x.is_multiple_of(8) || y.is_multiple_of(8) {
            -0.12
        } else {
            0.0
        };
        let shade = if cavity {
            0.44 + grain
        } else {
            0.80 + (seed % 17) as f32 * 0.014 + grain + edge
        };
        (base, shade)
    };
    Some(base.map(|v| v * shade))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reef_palettes_are_distinct_matte_and_tile_deterministically() {
        let mut palettes = std::collections::HashSet::new();
        for pattern in [
            TexturePattern::CoralRed,
            TexturePattern::CoralBlue,
            TexturePattern::CoralYellow,
            TexturePattern::CoralPurple,
            TexturePattern::Mud,
            TexturePattern::Ice,
        ] {
            let color = sample(pattern, 104, 188).unwrap();
            assert!(
                color
                    .iter()
                    .all(|v| v.is_finite() && (0.0..=255.0).contains(v))
            );
            palettes.insert(color.map(f32::to_bits));
            assert_eq!(
                sample(pattern, 104, 188),
                sample(pattern, 104 + 512, 188 + 512)
            );
        }
        assert_eq!(palettes.len(), 6);
        assert!(sample(TexturePattern::Stone, 1, 2).is_none());
    }
}
