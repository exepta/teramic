//! Periodic angular ballast stones: broad facets and recessed, nonsparkling joints.
use super::TexturePattern;

const SIZE: u32 = 512;
const STONES: i32 = 16;

fn hash(x: i32, y: i32, salt: u32) -> u32 {
    let mut value = (x.rem_euclid(STONES) as u32).wrapping_mul(0x9e3779b9)
        ^ (y.rem_euclid(STONES) as u32).wrapping_mul(0x85ebca6b)
        ^ salt;
    value ^= value >> 16;
    value = value.wrapping_mul(0x7feb352d);
    value ^= value >> 15;
    value.wrapping_mul(0x846ca68b) ^ (value >> 16)
}

pub(super) fn sample(pattern: TexturePattern, px: u32, py: u32) -> Option<[f32; 3]> {
    if pattern != TexturePattern::Ballast {
        return None;
    }
    let x = (px % SIZE) as f32 * STONES as f32 / SIZE as f32;
    let y = (py % SIZE) as f32 * STONES as f32 / SIZE as f32;
    let cell = [x.floor() as i32, y.floor() as i32];
    let mut seeds = [([0.0; 2], 0_u32, 0.0_f32); 9];
    let mut index = 0;
    for dy in -1..=1 {
        for dx in -1..=1 {
            let owner = [cell[0] + dx, cell[1] + dy];
            let seed = hash(owner[0], owner[1], 0x62416c6c);
            let center = [
                owner[0] as f32 + 0.22 + (seed & 255) as f32 / 255.0 * 0.56,
                owner[1] as f32 + 0.22 + ((seed >> 8) & 255) as f32 / 255.0 * 0.56,
            ];
            seeds[index] = (
                center,
                seed,
                (x - center[0]).powi(2) + (y - center[1]).powi(2),
            );
            index += 1;
        }
    }
    let nearest = seeds
        .iter()
        .min_by(|first, second| first.2.total_cmp(&second.2))
        .unwrap();
    // Voronoi bisectors make irregular, genuinely straight angular stone edges.
    let border = seeds
        .iter()
        .filter(|seed| seed.1 != nearest.1)
        .map(|seed| {
            (seed.2 - nearest.2)
                / (2.0 * (seed.0[0] - nearest.0[0]).hypot(seed.0[1] - nearest.0[1]))
        })
        .fold(f32::INFINITY, f32::min);
    if border < 0.045 {
        return Some([65.0, 62.0, 59.0]);
    }
    let palettes = [
        [139.0, 135.0, 130.0],
        [165.0, 155.0, 145.0],
        [193.0, 183.0, 169.0],
        [119.0, 122.0, 126.0],
        [151.0, 146.0, 140.0],
    ];
    let base = palettes[(nearest.1 >> 16) as usize % palettes.len()];
    let local = [x - nearest.0[0], y - nearest.0[1]];
    let facet = if local[0] * 0.64 + local[1] * 0.77 > 0.08 {
        0.96
    } else {
        1.05
    };
    let bevel = if border < 0.12 {
        0.73 + 0.20 * (border - 0.045) / 0.075
    } else {
        1.0
    };
    let grain = hash((px % SIZE / 2) as i32, (py % SIZE / 2) as i32, 0x63) % 5;
    Some(base.map(|channel| channel * facet * bevel + grain as f32 - 2.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn angular_ballast_is_periodic_opaque_deterministic_and_distinct_from_loose_gravel() {
        let mut colors = std::collections::BTreeSet::new();
        let mut joints = 0;
        for x in (0..SIZE).step_by(7) {
            for y in (0..SIZE).step_by(7) {
                let pixel = sample(TexturePattern::Ballast, x, y).unwrap();
                assert_eq!(
                    Some(pixel),
                    sample(TexturePattern::Ballast, x + SIZE, y + SIZE)
                );
                assert!(
                    pixel
                        .iter()
                        .all(|value| value.is_finite() && (0.0..=255.0).contains(value))
                );
                joints += usize::from(pixel == [65.0, 62.0, 59.0]);
                colors.insert(pixel.map(|channel| channel.round() as u8));
            }
        }
        assert!(
            colors.len() > 30,
            "broad stone facets need more than a flat grey fill"
        );
        assert!(
            (100..2000).contains(&joints),
            "joints should separate tightly packed stones"
        );
        assert!(sample(TexturePattern::Gravel, 0, 0).is_none());
    }
}
