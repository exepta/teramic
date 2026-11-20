//! One small bilinear height/biome field replaces overlapping fog volumes.
use bevy::prelude::*;

pub(super) const SIZE: usize = 11;
pub(super) const CELL: i64 = 16;
pub(super) const RANGE: f32 = 80.0;
pub(super) const HEIGHT: f32 = 8.0;

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Field {
    pub origin: [i64; 2],
    pub heights: [f32; SIZE * SIZE],
    pub weights: [f32; SIZE * SIZE],
}
impl Default for Field {
    fn default() -> Self {
        Self {
            origin: [0; 2],
            heights: [0.; SIZE * SIZE],
            weights: [0.; SIZE * SIZE],
        }
    }
}
impl Field {
    pub fn sample(eye: Vec3, mut sample: impl FnMut(i64, i64) -> Option<(f32, f32)>) -> Self {
        let mut field = Self::default();
        if !eye.is_finite() {
            return field;
        }
        field.origin = [eye.x, eye.z].map(|v| {
            (f64::from(v).floor() as i64)
                .div_euclid(CELL)
                .saturating_sub(5)
                .saturating_mul(CELL)
                .saturating_add(CELL / 2)
        });
        for z in 0..SIZE {
            for x in 0..SIZE {
                let px = field.origin[0].saturating_add(x as i64 * CELL);
                let pz = field.origin[1].saturating_add(z as i64 * CELL);
                if let Some((height, weight)) = sample(px, pz)
                    && height.is_finite()
                    && weight.is_finite()
                {
                    let i = x + SIZE * z;
                    field.heights[i] = height;
                    let q = ((weight - 0.05) / 0.75).clamp(0., 1.);
                    field.weights[i] = q * q * (3. - 2. * q);
                }
            }
        }
        // Missing/foreign-biome cells must not pull the interpolated floor to
        // sea level. Extend nearest valid heights, while their density stays zero.
        for i in 0..SIZE * SIZE {
            if field.weights[i] == 0.
                && let Some(j) = (0..SIZE * SIZE)
                    .filter(|&j| field.weights[j] > 0.)
                    .min_by_key(|&j| {
                        (i % SIZE).abs_diff(j % SIZE).pow(2) + (i / SIZE).abs_diff(j / SIZE).pow(2)
                    })
            {
                field.heights[i] = field.heights[j];
            }
        }
        field
    }
    pub fn occupied(&self) -> usize {
        self.weights.iter().filter(|&&w| w > 0.002).count()
    }
    pub fn bounds(&self) -> (Vec3, Vec3) {
        let min = self.heights.iter().copied().fold(f32::INFINITY, f32::min);
        let max = self
            .heights
            .iter()
            .copied()
            .fold(f32::NEG_INFINITY, f32::max)
            + HEIGHT;
        let half = (SIZE - 1) as f32 * CELL as f32 * 0.5;
        (
            Vec3::new(
                self.origin[0] as f32 + half,
                (min + max) * 0.5,
                self.origin[1] as f32 + half,
            ),
            Vec3::new(half, (max - min) * 0.5, half),
        )
    }
    pub fn pixels(&self) -> (Vec<u8>, f32, f32) {
        let base = self.heights.iter().copied().fold(f32::INFINITY, f32::min);
        let span = (self
            .heights
            .iter()
            .copied()
            .fold(f32::NEG_INFINITY, f32::max)
            - base)
            .max(1.);
        let mut pixels = Vec::with_capacity(SIZE * SIZE * 4);
        for i in 0..SIZE * SIZE {
            let height = (((self.heights[i] - base) / span).clamp(0., 1.) * 65535.).round() as u16;
            // Decode after bilinear filtering: both channels are linear terms.
            pixels.extend([
                (height >> 8) as u8,
                height as u8,
                (self.weights[i] * 255.).round() as u8,
                255,
            ]);
        }
        (pixels, base, span)
    }
}
