use terra_voxel::MaterialId;

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
    Pendant(MaterialId),
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
    ParkCap {
        center: [f32; 3],
        radius: f32,
        rise: f32,
        tilt: [f32; 2],
        phase: f32,
    },
    Frond(WoodSegment, Option<(MaterialId, [f32; 2])>),
    ConiferTier {
        center: [f32; 3],
        radius: [f32; 3],
        phase: f32,
    },
}

impl Crown {
    /// A run of hanging model cells. Its mesh touches both vertical block faces.
    pub fn pendant(x: i32, z: i32, top: i32, length: i32, material: MaterialId) -> Self {
        Self {
            bounds: Bounds {
                min: [x, top + 1 - length, z],
                max: [x + 1, top + 1, z + 1],
            },
            shape: CrownShape::Pendant(material),
        }
    }
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
    /// Flared, ribbed underside and a shallow domed crown. Tilt is in the XY plane,
    /// so the same cap can finish a fallen trunk on its side.
    pub fn park_cap(center: [f32; 3], radius: f32, rise: f32, angle: f32, phase: f32) -> Self {
        let (sin, cos) = angle.sin_cos();
        let extent = [
            radius * 1.05 * cos.abs() + rise * sin.abs(),
            radius * 1.05 * sin.abs() + rise * cos.abs(),
            radius * 1.05,
        ];
        Self {
            bounds: Bounds::around(center, center, extent),
            shape: CrownShape::ParkCap {
                center,
                radius,
                rise,
                tilt: [sin, cos],
                phase,
            },
        }
    }
    pub fn material(&self, point: [f32; 3], skin: MaterialId) -> Option<MaterialId> {
        if !self.bounds.contains(point) {
            return None;
        }
        if let CrownShape::Pendant(material) = self.shape {
            return Some(material);
        }
        if let CrownShape::ParkCap {
            center,
            radius,
            rise,
            tilt: [sin, cos],
            phase,
        } = self.shape
        {
            let dx = point[0] - center[0];
            let dy = point[1] - center[1];
            let x = dx * cos + dy * sin;
            let y = dy * cos - dx * sin;
            let z = point[2] - center[2];
            let r = x.hypot(z) / radius;
            if r > 1.05 || y > rise || y < -rise {
                return None;
            }
            let a = z.atan2(x);
            let rim = 1.0 + 0.035 * (a * 7.0 + phase).sin();
            if r > rim {
                return None;
            }
            let wide = radius >= 10.0;
            let ridge = (a * if wide { 27.0 } else { 19.0 } + phase + r * 0.9).cos();
            let under = -rise * (0.12 + (1.0 - r).powi(2) * 0.78)
                - ridge.max(0.0).powi(2) * (1.0 - r) * if wide { 1.65 } else { 0.70 };
            let dome = rise * (1.0 - (r / rim).powi(2)).sqrt();
            if y < under || y > dome {
                return None;
            }
            if y <= under + 1.08 && r < rim * 0.96 {
                return Some(MaterialId::MushroomGill);
            }
            // Fixed flecks across neighboring cap blocks read at landscape scale.
            let fleck_scale = if wide { 0.68 } else { 1.0 };
            let spot = (x * fleck_scale * 1.53 + phase).sin()
                + (z * fleck_scale * 1.38 - phase).cos()
                + ((x + z) * fleck_scale * 0.55 + phase * 2.).sin() * 0.48;
            return Some(if spot > 1.54 && y > dome - 1.4 {
                MaterialId::WhiteMushroomCap
            } else {
                skin
            });
        }
        if let CrownShape::Frond(_, Some((material, _))) = &self.shape {
            return self.palm_span(point).map(|_| *material);
        }
        self.contains(point).then_some(skin)
    }
    pub fn frond(start: [f32; 3], end: [f32; 3], radii: [f32; 2]) -> Self {
        Self::palm_segment(start, end, radii, None)
    }
    pub fn palm_segment(
        start: [f32; 3],
        end: [f32; 3],
        radii: [f32; 2],
        skin: Option<(MaterialId, [f32; 2])>,
    ) -> Self {
        let segment = WoodSegment::new(start, end, radii);
        Self {
            bounds: segment.bounds,
            shape: CrownShape::Frond(segment, skin),
        }
    }
    /// Clip the centerline to a half-open voxel. Adjacent cells share the exact same seam.
    pub fn palm_span(&self, center: [f32; 3]) -> Option<super::PalmSpan> {
        let CrownShape::Frond(segment, Some((_, progress))) = &self.shape else {
            return None;
        };
        if !self.bounds.contains(center) {
            return None;
        }
        let mut lo = 0.0_f32;
        let mut hi = 1.0_f32;
        let corner = center.map(|c| c - 0.5);
        for (axis, &edge) in corner.iter().enumerate() {
            let d = segment.direction[axis];
            let p = segment.start[axis] - edge;
            if d.abs() < 1e-6 {
                if !(0.0..1.0).contains(&p) {
                    return None;
                }
            } else {
                let a = -p / d;
                let b = (1.0 - p) / d;
                lo = lo.max(a.min(b));
                hi = hi.min(a.max(b));
            }
        }
        if hi - lo < 1e-5 {
            return None;
        }
        let at = |t: f32| {
            std::array::from_fn(|i| segment.start[i] - corner[i] + segment.direction[i] * t)
        };
        Some(super::PalmSpan {
            endpoints: [at(lo), at(hi)],
            progress: [lo, hi].map(|t| progress[0] + (progress[1] - progress[0]) * t),
        })
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
            CrownShape::Pendant(_) => true,
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
            CrownShape::ParkCap { .. } => self.material(point, MaterialId::MushroomGill).is_some(),
            CrownShape::Frond(_, Some(_)) => self.palm_span(point).is_some(),
            CrownShape::Frond(segment, None) => segment.contains(point),
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
