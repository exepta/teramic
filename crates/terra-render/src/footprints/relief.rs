//! Real recessed geometry and a matching constant-cost terrain cutout lookup.
use super::*;
use bevy::mesh::{Indices, PrimitiveTopology};

const BUCKETS: usize = 128;
const SLOTS: usize = 4;

#[derive(Resource, Clone)]
pub(crate) struct Imprints(pub [Vec4; BUCKETS * SLOTS * 2]);
impl Default for Imprints {
    fn default() -> Self {
        Self([Vec4::ZERO; BUCKETS * SLOTS * 2])
    }
}
fn bucket(x: i32, z: i32) -> usize {
    ((x as u32).wrapping_mul(0x8da6b343) ^ (z as u32).wrapping_mul(0xd8163841)) as usize % BUCKETS
}

impl Imprints {
    fn insert(&mut self, center: Vec3, side: Vec2, half: Vec2, depth: f32) -> bool {
        let forward = Vec2::new(-side.y, side.x);
        let extent = side.abs() * half.x + forward.abs() * half.y;
        let low = ((center.xz() - extent) * 2.0).floor().as_ivec2();
        let high = ((center.xz() + extent) * 2.0).floor().as_ivec2();
        let mut slots = Vec::with_capacity(4);
        for x in low.x..=high.x {
            for z in low.y..=high.y {
                let base = bucket(x, z) * SLOTS * 2;
                if slots
                    .iter()
                    .any(|index| *index >= base && *index < base + SLOTS * 2)
                {
                    continue;
                }
                let Some(index) = (base..base + SLOTS * 2)
                    .step_by(2)
                    .find(|i| self.0[*i].w == 0.0)
                else {
                    return false;
                };
                slots.push(index);
            }
        }
        // Publish a recess only if every touched cell has its matching cutout.
        for i in slots {
            self.0[i] = Vec4::new(center.x, center.z, center.y, half.x);
            self.0[i + 1] = Vec4::new(side.x, side.y, half.y, depth);
        }
        true
    }
}

#[derive(Default)]
struct Geometry {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    colors: Vec<[f32; 4]>,
    indices: Vec<u32>,
}
impl Geometry {
    fn quad(&mut self, points: [Vec3; 4], rgb: Vec3, alpha: f32) {
        let start = self.positions.len() as u32;
        let normal = (points[2] - points[0])
            .cross(points[1] - points[0])
            .normalize_or(Vec3::Y);
        for p in points {
            self.positions.push(p.to_array());
            self.normals.push(normal.to_array());
            self.colors.push(rgb.extend(alpha).to_array());
        }
        self.indices
            .extend([start, start + 2, start + 1, start, start + 3, start + 2]);
    }
    fn recess(&mut self, center: Vec3, side: Vec2, half: Vec2, depth: f32, snow: bool, alpha: f32) {
        let forward = Vec2::new(-side.y, side.x);
        let corners = [
            Vec2::NEG_ONE,
            Vec2::new(1.0, -1.0),
            Vec2::ONE,
            Vec2::new(-1.0, 1.0),
        ];
        let ring = |scale: f32, y: f32| {
            corners.map(|v| {
                let q = v * half * scale;
                let xz = side * q.x + forward * q.y;
                center + Vec3::new(xz.x, y, xz.y)
            })
        };
        let rim = ring(1.0, 0.0002);
        let bottom = ring(0.75, -depth);
        let rgb = if snow {
            Vec3::new(0.76, 0.82, 0.89)
        } else {
            Vec3::new(0.61, 0.45, 0.23)
        };
        self.quad(bottom, rgb * 0.91, alpha);
        for i in 0..4 {
            let j = (i + 1) % 4;
            self.quad([rim[i], rim[j], bottom[j], bottom[i]], rgb, alpha);
        }
    }
    fn mesh(self) -> Mesh {
        let count = self.positions.len();
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, self.colors)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0, 0.0]; count])
        .with_inserted_indices(Indices::U32(self.indices))
    }
}

#[cfg(test)]
pub(super) fn build(tracks: &[Track], now: f32) -> (Mesh, Imprints) {
    build_checked(tracks, now, |_, _, _, _| true)
}

pub(super) fn build_checked(
    tracks: &[Track],
    now: f32,
    mut supported: impl FnMut(Vec3, Vec2, Vec2, u16) -> bool,
) -> (Mesh, Imprints) {
    let mut geometry = Geometry::default();
    let mut mask = Imprints::default();
    // Newest contacts win bounded bucket pressure; no visible recess exists without its mask.
    for track in tracks.iter().rev() {
        let step = track.step;
        let forward = step.forward.normalize_or(Vec2::Y);
        let side = Vec2::new(forward.y, -forward.x);
        let snow = TrackSurface::from_block(step.material) == Some(TrackSurface::Snow);
        let opacity = alpha(track, now);
        let mut imprint = |offset: Vec2, half: Vec2| {
            let xz = side * offset.x + forward * offset.y;
            let center = step.position + Vec3::new(xz.x, 0.0, xz.y);
            let depth = (if snow { 0.045 } else { 0.026 }) * opacity / 0.36;
            let has_support = supported(center, side, half, step.material);
            let half = half * (opacity / 0.36).sqrt();
            if opacity > 0.0 && has_support && mask.insert(center, side, half, depth) {
                geometry.recess(center, side, half, depth, snow, opacity);
            }
        };
        let sign = if step.alternate { 1.0 } else { -1.0 };
        if matches!(step.walker, Walker::Player | Walker::Stalker) {
            imprint(Vec2::new(sign * 0.13, 0.07), Vec2::new(0.065, 0.105));
            imprint(Vec2::new(sign * 0.13, -0.12), Vec2::new(0.055, 0.055));
        } else if step.walker == Walker::Spider {
            for i in 0..4 {
                let flank = if i % 2 == 0 { sign } else { -sign };
                imprint(
                    Vec2::new(flank * 0.27, (i as f32 - 1.5) * 0.16),
                    Vec2::new(0.023, 0.038),
                );
            }
        } else if step.walker == Walker::Slime {
            imprint(Vec2::ZERO, Vec2::new(0.20, 0.17));
        } else if step.walker == Walker::Dragon {
            let heel = Vec2::new(sign * 0.40, 0.05);
            imprint(heel, Vec2::new(0.13, 0.15));
            for toe in -1..=1 {
                imprint(
                    heel + Vec2::new(toe as f32 * 0.12, 0.23),
                    Vec2::new(0.035, 0.11),
                );
            }
        } else if step.walker == Walker::Bear {
            for (x, z) in [(sign * 0.30, 0.40), (-sign * 0.30, -0.40)] {
                imprint(Vec2::new(x, z), Vec2::new(0.095, 0.10));
                for toe in -2..=2 {
                    imprint(
                        Vec2::new(x + toe as f32 * 0.038, z + 0.13),
                        Vec2::new(0.014, 0.035),
                    );
                }
            }
        } else {
            let scale = if step.walker == Walker::Cow {
                1.15
            } else {
                0.85
            };
            for (x, z) in [(sign * 0.26, 0.35), (-sign * 0.26, -0.35)] {
                for split in [-0.034, 0.034] {
                    imprint(
                        Vec2::new(x + split, z) * scale,
                        Vec2::new(0.024, 0.070) * scale,
                    );
                }
            }
        }
    }
    (geometry.mesh(), mask)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn negative_cells_and_bucket_pressure_never_leave_unpaired_holes() {
        let mut mask = Imprints::default();
        let p = Vec3::new(-0.01, 1.25, -0.01);
        for _ in 0..SLOTS {
            assert!(mask.insert(p, Vec2::X, Vec2::splat(0.09), 0.03));
        }
        let previous = mask.0;
        assert!(!mask.insert(p, Vec2::X, Vec2::splat(0.09), 0.03));
        assert_eq!(mask.0, previous);
        for (x, z) in [(-1, -1), (-1, 0), (0, -1), (0, 0)] {
            assert!(mask.0[bucket(x, z) * SLOTS * 2].w > 0.0);
        }
    }
    #[test]
    fn sand_and_snow_have_a_real_floor_below_the_rim_and_valid_wall_normals() {
        for snow in [false, true] {
            let mut g = Geometry::default();
            g.recess(Vec3::Y, Vec2::X, Vec2::new(0.07, 0.12), 0.04, snow, 0.36);
            assert_eq!(g.positions.len(), 20);
            assert!(g.positions[..4].iter().all(|p| p[1] == 0.96));
            assert!(g.positions[4..].iter().any(|p| p[1] > 1.0));
            assert!(
                g.normals
                    .iter()
                    .all(|n| Vec3::from_array(*n).is_normalized())
            );
        }
    }
}
