//! Small shared voxel sculptures, meshed once per profession and rigid body part.
//! Overlapping clothing volumes overwrite cells; interior faces never reach the GPU.
use super::{Builder, *};
use std::collections::BTreeMap;

const UNIT: f32 = 0.025;
type Cell = [i16; 3];
type Tint = [u8; 3];

#[derive(Default)]
pub(super) struct Sculpt(BTreeMap<Cell, Tint>);

impl Sculpt {
    pub(super) fn block(&mut self, at: [f32; 3], size: [f32; 3], color: [f32; 3]) {
        self.rotated(at, size, color, Quat::IDENTITY);
    }

    pub(super) fn rotated(&mut self, at: [f32; 3], size: [f32; 3], color: [f32; 3], rot: Quat) {
        let at = Vec3::from_array(at);
        let half = Vec3::from_array(size) * 0.5;
        let extent = Mat3::from_quat(rot).abs() * half;
        let lo = ((at - extent) / UNIT).floor().as_ivec3();
        let hi = ((at + extent) / UNIT).ceil().as_ivec3();
        let color = color.map(|c| (c.clamp(0.0, 1.0) * 255.0).round() as u8);
        for x in lo.x..hi.x {
            for y in lo.y..hi.y {
                for z in lo.z..hi.z {
                    let center =
                        (Vec3::new(x as f32, y as f32, z as f32) + Vec3::splat(0.5)) * UNIT;
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

    pub(super) fn finish(self) -> Result<Mesh, String> {
        let mut mesh = Builder::default();
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
                    while faces.get(&[cell[0], cell[1], end]) == Some(&color) {
                        faces.remove(&[cell[0], cell[1], end]);
                        end += 1;
                    }
                    let mut top = cell[1] + 1;
                    while (cell[2]..end).all(|x| faces.get(&[cell[0], top, x]) == Some(&color)) {
                        for x in cell[2]..end {
                            faces.remove(&[cell[0], top, x]);
                        }
                        top += 1;
                    }
                    let mut p = Vec3::ZERO;
                    p[axis] = f32::from(cell[0] + i16::from(sign > 0)) * UNIT;
                    p[u] = f32::from(cell[2]) * UNIT;
                    p[v] = f32::from(cell[1]) * UNIT;
                    let mut du = Vec3::ZERO;
                    du[u] = f32::from(end - cell[2]) * UNIT;
                    let mut dv = Vec3::ZERO;
                    dv[v] = f32::from(top - cell[1]) * UNIT;
                    let corners = if sign > 0 {
                        [p, p + du, p + du + dv, p + dv]
                    } else {
                        [p + du, p, p + dv, p + du + dv]
                    };
                    mesh.quad(corners, color.map(|c| f32::from(c) / 255.0))?;
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
