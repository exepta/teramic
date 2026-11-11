//! Small shared voxel sculptures, meshed once per profession and rigid body part.
//! Overlapping clothing volumes overwrite cells; interior faces never reach the GPU.
use super::{Builder, *};
use std::collections::BTreeMap;

type Cell = [i16; 3];
type Tint = [u8; 3];

pub(super) struct Sculpt(BTreeMap<Cell, Tint>, f32);
impl Default for Sculpt {
    fn default() -> Self {
        Self(BTreeMap::new(), 0.025)
    }
}

impl Sculpt {
    pub(super) fn detailed_head() -> Self {
        Self(BTreeMap::new(), 0.02)
    }
    pub(super) fn block(&mut self, at: [f32; 3], size: [f32; 3], color: [f32; 3]) {
        self.rotated(at, size, color, Quat::IDENTITY);
    }

    /// Rounded garment corners retain broad planes without a rectangular silhouette.
    pub(super) fn rounded_box(
        &mut self,
        at: [f32; 3],
        size: [f32; 3],
        rounding: f32,
        color: [f32; 3],
    ) {
        let center = Vec3::from_array(at);
        let half = Vec3::from_array(size) * 0.5;
        let radius = rounding.min(half.min_element());
        let core = half - Vec3::splat(radius);
        self.volume(center, half, color, |point| {
            (point.abs() - core).max(Vec3::ZERO).length_squared() <= radius * radius
        });
    }

    /// Elliptical horizontal bands form hat brims, crowns and fitted clothing hems.
    pub(super) fn oval(&mut self, at: [f32; 3], size: [f32; 3], color: [f32; 3]) {
        let center = Vec3::from_array(at);
        let half = Vec3::from_array(size) * 0.5;
        self.volume(center, half, color, |point| {
            (point.x / half.x).powi(2) + (point.z / half.z).powi(2) <= 1.0
                && point.y.abs() <= half.y
        });
    }

    fn volume(&mut self, center: Vec3, half: Vec3, color: [f32; 3], inside: impl Fn(Vec3) -> bool) {
        let low = ((center - half) / self.1).floor().as_ivec3();
        let high = ((center + half) / self.1).ceil().as_ivec3();
        let color = color.map(|c| (c.clamp(0.0, 1.0) * 255.0).round() as u8);
        for x in low.x..high.x {
            for y in low.y..high.y {
                for z in low.z..high.z {
                    let point =
                        (Vec3::new(x as f32, y as f32, z as f32) + Vec3::splat(0.5)) * self.1;
                    if inside(point - center) {
                        self.0.insert([x as i16, y as i16, z as i16], color);
                    }
                }
            }
        }
    }

    /// Keep authored held equipment in the same rigid mesh as its gripping hand.
    pub(super) fn append(&mut self, other: Self, offset: Vec3) {
        let offset = (offset / self.1).round().as_ivec3();
        for (cell, color) in other.0 {
            self.0.insert(
                [
                    cell[0] + offset.x as i16,
                    cell[1] + offset.y as i16,
                    cell[2] + offset.z as i16,
                ],
                color,
            );
        }
    }

    /// Resample an articulated section once around its joint, preserving a closed union.
    pub(super) fn articulated(&mut self, other: Self, joint: Vec3, rotation: Quat) {
        for (cell, color) in other.0 {
            let center = (Vec3::new(cell[0] as f32, cell[1] as f32, cell[2] as f32)
                + Vec3::splat(0.5))
                * self.1;
            let transformed = joint + rotation * (center - joint);
            self.rotated(
                transformed.to_array(),
                [self.1 * 1.12; 3],
                color.map(|component| f32::from(component) / 255.0),
                rotation,
            );
        }
    }

    pub(super) fn rotated(&mut self, at: [f32; 3], size: [f32; 3], color: [f32; 3], rot: Quat) {
        let at = Vec3::from_array(at);
        let half = Vec3::from_array(size) * 0.5;
        let extent = Mat3::from_quat(rot).abs() * half;
        let lo = ((at - extent) / self.1).floor().as_ivec3();
        let hi = ((at + extent) / self.1).ceil().as_ivec3();
        let color = color.map(|c| (c.clamp(0.0, 1.0) * 255.0).round() as u8);
        for x in lo.x..hi.x {
            for y in lo.y..hi.y {
                for z in lo.z..hi.z {
                    let center =
                        (Vec3::new(x as f32, y as f32, z as f32) + Vec3::splat(0.5)) * self.1;
                    if (rot.inverse() * (center - at))
                        .abs()
                        .cmple(half + Vec3::splat(0.00001))
                        .all()
                    {
                        self.0.insert([x as i16, y as i16, z as i16], color);
                    }
                }
            }
        }
    }

    /// Fill a rounded voxel volume while retaining a crisp, stepped silhouette.
    pub(super) fn ellipsoid(&mut self, at: [f32; 3], radii: [f32; 3], color: [f32; 3]) {
        let center = Vec3::from_array(at);
        let radius = Vec3::from_array(radii);
        let lo = ((center - radius) / self.1).floor().as_ivec3();
        let hi = ((center + radius) / self.1).ceil().as_ivec3();
        let color = color.map(|c| (c.clamp(0.0, 1.0) * 255.0).round() as u8);
        for x in lo.x..hi.x {
            for y in lo.y..hi.y {
                for z in lo.z..hi.z {
                    let point =
                        (Vec3::new(x as f32, y as f32, z as f32) + Vec3::splat(0.5)) * self.1;
                    if ((point - center) / radius).length_squared() <= 1.0 {
                        self.0.insert([x as i16, y as i16, z as i16], color);
                    }
                }
            }
        }
    }

    pub(super) fn finish(self) -> Result<Mesh, String> {
        let mut mesh = Builder::default();
        let facet_cells = (0.05 / self.1).round() as i16;
        // Merge coplanar equal-colored cells into rectangles. Variation is deliberately
        // applied per authored garment/hair piece, not noisy per-frame texels.
        for axis in 0..3 {
            let u = (axis + 1) % 3;
            let v = (axis + 2) % 3;
            for sign in [-1_i16, 1] {
                let mut faces = BTreeMap::new();
                for (&cell, &color) in &self.0 {
                    let mut neighbor = cell;
                    neighbor[axis] += sign;
                    if !self.0.contains_key(&neighbor) {
                        faces.insert([cell[axis], cell[v], cell[u]], color);
                    }
                }
                while let Some((cell, color)) = faces.pop_first() {
                    let mut end = cell[2] + 1;
                    while end - cell[2] < facet_cells
                        && faces.get(&[cell[0], cell[1], end]) == Some(&color)
                    {
                        faces.remove(&[cell[0], cell[1], end]);
                        end += 1;
                    }
                    let mut top = cell[1] + 1;
                    while top - cell[1] < facet_cells
                        && (cell[2]..end).all(|x| faces.get(&[cell[0], top, x]) == Some(&color))
                    {
                        for x in cell[2]..end {
                            faces.remove(&[cell[0], top, x]);
                        }
                        top += 1;
                    }
                    let mut p = Vec3::ZERO;
                    p[axis] = f32::from(cell[0] + i16::from(sign > 0)) * self.1;
                    p[u] = f32::from(cell[2]) * self.1;
                    p[v] = f32::from(cell[1]) * self.1;
                    let mut du = Vec3::ZERO;
                    du[u] = f32::from(end - cell[2]) * self.1;
                    let mut dv = Vec3::ZERO;
                    dv[v] = f32::from(top - cell[1]) * self.1;
                    let corners = if sign > 0 {
                        [p, p + du, p + du + dv, p + dv]
                    } else {
                        [p + du, p, p + dv, p + du + dv]
                    };
                    // A shallow four-sided facet breaks large planar garments and skin
                    // into the small carved surfaces of the reference. Boundary vertices
                    // stay on the exact union surface, so adjacent pieces never separate.
                    let hash = u32::from(cell[0] as u16).wrapping_mul(73)
                        ^ u32::from(cell[1] as u16).wrapping_mul(137)
                        ^ u32::from(cell[2] as u16).wrapping_mul(317);
                    let tint = color.map(|c| {
                        (f32::from(c) / 255.0 * (0.965 + (hash & 7) as f32 * 0.010)).min(1.0)
                    });
                    let mut normal = Vec3::ZERO;
                    normal[axis] = f32::from(sign);
                    let relief = if axis == 1 && sign < 0 { 0.0 } else { 0.0015 };
                    let center = (corners[0] + corners[2]) * 0.5 + normal * relief;
                    mesh.faceted_quad(corners, center, tint)?;
                }
            }
        }
        mesh.finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adjoining_and_overlapping_volumes_have_no_internal_or_duplicate_faces() {
        let mut single = Sculpt::default();
        single.block([0.0; 3], [0.4; 3], [0.5; 3]);
        let expected = single.finish().unwrap();
        let mut pieces = Sculpt::default();
        for x in [-0.1, 0.1] {
            pieces.block([x, 0.0, 0.0], [0.2, 0.4, 0.4], [0.5; 3]);
        }
        pieces.block([0.0; 3], [0.15; 3], [0.7; 3]);
        let mesh = pieces.finish().unwrap();
        assert_eq!(
            mesh.attribute(Mesh::ATTRIBUTE_POSITION),
            expected.attribute(Mesh::ATTRIBUTE_POSITION)
        );
        assert_eq!(
            mesh.attribute(Mesh::ATTRIBUTE_NORMAL),
            expected.attribute(Mesh::ATTRIBUTE_NORMAL)
        );
        assert_eq!(mesh.count_vertices(), expected.count_vertices());
    }
}
