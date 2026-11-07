//! Fine cohesive golden grains and subtle wind ripples; no stones or per-cell assets.
pub(super) const SIZE: u32 = 512;

fn hash(x: u32, y: u32) -> u32 {
    let mut seed = x.wrapping_mul(0x8da6b343) ^ y.wrapping_mul(0xd8163841);
    seed = (seed ^ (seed >> 16)).wrapping_mul(0x7feb352d);
    (seed ^ (seed >> 15)).wrapping_mul(0x846ca68b)
}

fn clusters(x: u32, y: u32) -> f32 {
    const STEP: u32 = 16;
    const PERIOD: u32 = SIZE / STEP;
    let cell = [x / STEP, y / STEP];
    let weight = [x % STEP, y % STEP].map(|value| {
        let t = (value as f32 + 0.5) / STEP as f32;
        t * t * (3.0 - 2.0 * t)
    });
    let sample =
        |dx, dy| (hash((cell[0] + dx) % PERIOD, (cell[1] + dy) % PERIOD) & 255) as f32 / 255.0;
    let corners = [sample(0, 0), sample(1, 0), sample(0, 1), sample(1, 1)];
    let near = corners[0] + (corners[1] - corners[0]) * weight[0];
    let far = corners[2] + (corners[3] - corners[2]) * weight[0];
    near + (far - near) * weight[1] - 0.5
}

pub(super) fn color(px: u32, py: u32) -> [f32; 3] {
    use std::f32::consts::TAU;

    let [x, y] = [px % SIZE, py % SIZE];
    let p = [px % SIZE, py % SIZE].map(|v| (v as f32 + 0.5) / SIZE as f32);
    let seed = hash(x / 2, y / 2);
    let grain = ((seed >> 8) & 255) as f32 / 255.0 - 0.5;
    let packed = (hash(x / 8, y / 8) & 255) as f32 / 255.0 - 0.5;
    let cluster = clusters(x, y);
    let facet = (hash(x, y) & 255) as f32 / 255.0 - 0.5;
    let warmth = ((seed >> 20) & 31) as f32 / 31.0 - 0.5;
    // Integer-frequency waves meet across the repeat boundary. Their subdued
    // lighting leaves fine individual grains visible without rock-sized cells.
    let phase =
        TAU * (p[1] * 5.0 + (p[0] * TAU).sin() * 0.10 + ((p[0] * 3.0 - p[1]) * TAU).sin() * 0.035);
    let dune = phase.sin() * 0.018 + (phase * 2.0).cos() * 0.004;
    let base = [239.0, 205.0, 139.0];
    std::array::from_fn(|axis| {
        (base[axis] * (1.0 + dune)
            + [14.0, 12.0, 8.0][axis] * grain
            + [17.0, 14.0, 10.0][axis] * packed
            + [16.0, 13.0, 9.0][axis] * cluster
            + [4.0, 3.0, 2.0][axis] * facet
            + [2.0, 0.0, 3.0][axis] * warmth)
            .clamp(0.0, 255.0)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn painted_sand_is_cohesive_fine_grained_golden_and_entirely_stone_free() {
        let mut mean = [0.0; 3];
        let mut detail = 0;
        for y in 0..SIZE {
            for x in 0..SIZE {
                let rgb = color(x, y);
                assert!(
                    rgb.iter()
                        .all(|v| v.is_finite() && (0.0..=255.0).contains(v))
                );
                assert!(rgb[0] > 200.0 && rgb[1] > 170.0 && rgb[2] < 165.0);
                assert!(rgb[0] > rgb[1] + 22.0 && rgb[1] > rgb[2] + 45.0);
                for axis in 0..3 {
                    mean[axis] += rgb[axis] / (SIZE * SIZE) as f32;
                }
                assert_eq!(rgb, color(x + SIZE, y + SIZE));
                let adjacent = color(x + 1, y);
                let difference = (rgb[0] - adjacent[0]).abs();
                assert!(difference < 40.0, "grain contrast: {difference}");
                detail += usize::from(difference > 0.5);
            }
        }
        assert!(detail as f32 / (SIZE * SIZE) as f32 > 0.5);
        for (actual, expected) in mean.into_iter().zip([239.0, 205.0, 139.0]) {
            assert!((actual - expected).abs() < 1.5);
        }
    }

    #[test]
    fn packed_fine_grains_stay_readable_after_eight_pixel_mip_filtering() {
        let mut low = f32::INFINITY;
        let mut high = f32::NEG_INFINITY;
        let mut sum = 0.0;
        let mut squares = 0.0;
        for by in 0..SIZE / 8 {
            for bx in 0..SIZE / 8 {
                let mut red = 0.0;
                for y in 0..8 {
                    for x in 0..8 {
                        red += color(bx * 8 + x, by * 8 + y)[0] / 64.0;
                    }
                }
                low = low.min(red);
                high = high.max(red);
                sum += red;
                squares += red * red;
            }
        }
        let count = (SIZE / 8).pow(2) as f32;
        let variance = squares / count - (sum / count).powi(2);
        assert!(high - low > 20.0, "mipped grain range: {}", high - low);
        assert!(variance > 12.0, "mipped grain variance: {variance}");
        assert!(variance < 80.0, "sand must stay cohesive: {variance}");
    }

    #[test]
    fn sand_uses_existing_world_anchored_pbr_grain_without_extra_meshes() {
        let shader = include_str!("../../../../assets/shaders/terra_wet_surface.wesl");
        let sand = shader
            .split("} else if family < 4.5 {")
            .nth(1)
            .unwrap()
            .split("} else {")
            .next()
            .unwrap();
        assert!(sand.contains("wet_noise(p * 64.0"));
        assert!(sand.contains("wet_hash(floor(p * 24.0)"));
        assert!(sand.contains("height = ripple * 0.0018 + grains * 0.0012 + grain * 0.00035"));
        assert!(sand.contains("contact = 0.965 + grains * 0.035"));
        assert!(shader.contains("let detail_fade = 1.0 - smoothstep(0.012,0.11,footprint)"));
        assert!(shader.contains("perceptual_roughness,0.86"));
    }
}
