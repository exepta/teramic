#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Bounds {
    pub min: [i32; 3],
    pub max: [i32; 3],
}

impl Bounds {
    fn around(start: [f32; 3], end: [f32; 3], radius: [f32; 3]) -> Self {
        Self {
            min: std::array::from_fn(|i| (start[i].min(end[i]) - radius[i]).floor() as i32),
            max: std::array::from_fn(|i| (start[i].max(end[i]) + radius[i]).ceil() as i32),
        }
    }
    pub fn union(self, other: Self) -> Self {
        Self {
            min: std::array::from_fn(|i| self.min[i].min(other.min[i])),
            max: std::array::from_fn(|i| self.max[i].max(other.max[i])),
        }
    }
    pub fn contains(self, point: [f32; 3]) -> bool {
        (0..3).all(|i| point[i] >= self.min[i] as f32 && point[i] < self.max[i] as f32)
    }
    pub fn intersects(self, origin: [i64; 3], start: [i64; 3], size: usize) -> bool {
        (0..3).all(|i| {
            origin[i] + i64::from(self.max[i]) > start[i]
                && origin[i] + i64::from(self.min[i]) < start[i] + size as i64
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct WoodSegment {
    pub bounds: Bounds,
    start: [f32; 3],
    direction: [f32; 3],
    inverse_length_squared: f32,
    radii: [f32; 2],
}

impl WoodSegment {
    pub fn new(start: [f32; 3], end: [f32; 3], radii: [f32; 2]) -> Self {
        let direction = std::array::from_fn(|i| end[i] - start[i]);
        let squared: f32 = direction.iter().map(|v| v * v).sum();
        Self {
            bounds: Bounds::around(start, end, [radii[0].max(radii[1]); 3]),
            start,
            direction,
            inverse_length_squared: if squared > 0.0 { 1.0 / squared } else { 0.0 },
            radii,
        }
    }
    pub fn contains(&self, point: [f32; 3]) -> bool {
        if !self.bounds.contains(point) {
            return false;
        }
        let delta: [f32; 3] = std::array::from_fn(|i| point[i] - self.start[i]);
        let along = (delta
            .iter()
            .zip(self.direction)
            .map(|(&p, d)| p * d)
            .sum::<f32>()
            * self.inverse_length_squared)
            .clamp(0.0, 1.0);
        let distance: f32 = (0..3)
            .map(|i| (delta[i] - self.direction[i] * along).powi(2))
            .sum();
        let radius = self.radii[0] + (self.radii[1] - self.radii[0]) * along;
        distance <= radius * radius
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Crown {
    pub bounds: Bounds,
    shape: CrownShape,
}

#[derive(Debug, Clone, PartialEq)]
enum CrownShape {
    Ellipsoid {
        center: [f32; 3],
        inverse_radius: [f32; 3],
    },
    MushroomCap {
        center: [f32; 3],
        radius: [f32; 3],
        variant: u8,
        phase: f32,
    },
    Frond(WoodSegment),
    ConiferTier {
        center: [f32; 3],
        radius: [f32; 3],
        phase: f32,
    },
}

impl Crown {
    pub fn new(center: [f32; 3], radius: [f32; 3]) -> Self {
        Self {
            bounds: Bounds::around(center, center, radius),
            shape: CrownShape::Ellipsoid {
                center,
                inverse_radius: radius.map(|r| 1.0 / r),
            },
        }
    }
    pub fn mushroom_cap(center: [f32; 3], radius: [f32; 3], variant: u8, phase: f32) -> Self {
        let mut bounds = Bounds::around(
            center,
            center,
            [radius[0] * 1.12 + 1.2, radius[1], radius[2] * 1.12],
        );
        bounds.min[1] = (center[1] - 0.7).floor() as i32;
        Self {
            bounds,
            shape: CrownShape::MushroomCap {
                center,
                radius,
                variant,
                phase,
            },
        }
    }
    pub fn frond(start: [f32; 3], end: [f32; 3], radii: [f32; 2]) -> Self {
        let segment = WoodSegment::new(start, end, radii);
        Self {
            bounds: segment.bounds,
            shape: CrownShape::Frond(segment),
        }
    }
    pub fn conifer_tier(center: [f32; 3], radius: [f32; 3], phase: f32) -> Self {
        let mut bounds = Bounds::around(center, center, radius);
        bounds.min[1] = (center[1] - 0.6).floor() as i32;
        Self {
            bounds,
            shape: CrownShape::ConiferTier {
                center,
                radius,
                phase,
            },
        }
    }
    pub fn contains(&self, point: [f32; 3]) -> bool {
        if !self.bounds.contains(point) {
            return false;
        }
        match &self.shape {
            CrownShape::Ellipsoid {
                center,
                inverse_radius,
            } => {
                (0..3)
                    .map(|i| ((point[i] - center[i]) * inverse_radius[i]).powi(2))
                    .sum::<f32>()
                    <= 1.0
            }
            CrownShape::MushroomCap {
                center,
                radius,
                variant,
                phase,
            } => {
                let y = point[1] - center[1];
                if y < -0.7 || y > radius[1] {
                    return false;
                }
                let t = (y / radius[1]).clamp(0.0, 1.0);
                let bend = if *variant == 0 { t * t * 1.2 } else { 0.0 };
                let x = (point[0] - center[0] - bend) / radius[0];
                let z = (point[2] - center[2]) / radius[2];
                let angle = z.atan2(x);
                let lobes =
                    1.0 + (angle * 5.0 + phase).sin() * 0.045 + (angle * 9.0 - phase).cos() * 0.025;
                let profile = match variant {
                    0 => (1.0 - t).powf(0.72),
                    2 => 1.0 - t * 0.90,
                    4 => (1.0 - t).powf(0.58),
                    5 => (1.0 - t * t).sqrt() * (1.0 - t * 0.12),
                    _ => (1.0 - t * t).sqrt(),
                };
                let underside = if y < 0.0 { 1.0 + y * 0.22 } else { 1.0 };
                x * x + z * z <= (profile * lobes * underside).powi(2)
            }
            CrownShape::Frond(segment) => segment.contains(point),
            CrownShape::ConiferTier {
                center,
                radius,
                phase,
            } => {
                let y = point[1] - center[1];
                if y < -0.6 || y > radius[1] {
                    return false;
                }
                let x = (point[0] - center[0]) / radius[0];
                let z = (point[2] - center[2]) / radius[2];
                let angle = z.atan2(x);
                let boughs = 0.93 + (angle * 6.0 + phase).cos() * 0.07;
                let taper = (1.0 - (y / radius[1]).max(0.0)).powf(0.72);
                let underside = if y < 0.0 { 1.0 + y * 0.5 } else { 1.0 };
                x * x + z * z <= (taper * boughs * underside).powi(2)
            }
        }
    }
}

/// Rasterize only a shape's intersection with the requested section/padding.
/// Subtract integer world coordinates before converting to local floats.
pub(crate) fn visit_voxels(
    bounds: Bounds,
    origin: [i64; 3],
    start: [i64; 3],
    size: usize,
    mut visit: impl FnMut([f32; 3], usize),
) {
    let minimum: [usize; 3] = std::array::from_fn(|i| {
        (origin[i] - start[i] + i64::from(bounds.min[i])).clamp(0, size as i64) as usize
    });
    let maximum: [usize; 3] = std::array::from_fn(|i| {
        (origin[i] - start[i] + i64::from(bounds.max[i])).clamp(0, size as i64) as usize
    });
    for z in minimum[2]..maximum[2] {
        for y in minimum[1]..maximum[1] {
            for x in minimum[0]..maximum[0] {
                let point = [x, y, z].map(|v| v as i64);
                let point = std::array::from_fn(|i| (start[i] + point[i] - origin[i]) as f32 + 0.5);
                visit(point, x + size * (y + size * z));
            }
        }
    }
}
