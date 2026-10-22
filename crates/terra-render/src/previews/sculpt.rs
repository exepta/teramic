//! Small cached item solids. Interior voxel faces never reach the GPU.
use super::*;
use std::collections::BTreeMap;

pub(super) struct Sculpt {
    cells: BTreeMap<[i16; 3], [f32; 3]>,
    step: f32,
    stride: i16,
}

impl Sculpt {
    pub fn new(resolution: u16, stride: i16) -> Self {
        Self {
            cells: BTreeMap::new(),
            step: f32::from(stride) / f32::from(resolution),
            stride,
        }
    }

    pub fn cell(&mut self, p: [i16; 3], color: [f32; 3]) {
        self.cells
            .insert(p.map(|v| v.div_euclid(self.stride)), color);
    }

    pub fn cuboid(&mut self, min: [i16; 3], max: [i16; 3], color: [f32; 3]) {
        for x in min[0]..=max[0] {
            for y in min[1]..=max[1] {
                for z in min[2]..=max[2] {
                    self.cell([x, y, z], shade(color, [x, y, z], 0.055));
                }
            }
        }
    }

    pub fn mesh(self, material: usize) -> Result<MeshBatch, String> {
        self.build(material, true)
    }

    /// Flat cast surfaces retain their outer voxel silhouette without a raised
    /// grid of tiny specular bevels across every metal face.
    pub fn faceted_mesh(self, material: usize) -> Result<MeshBatch, String> {
        self.build(material, false)
    }

    fn build(self, material: usize, relief: bool) -> Result<MeshBatch, String> {
        let mut out = MeshBatch {
            material,
            ..default()
        };
        let offsets = [
            [-1, 0, 0],
            [1, 0, 0],
            [0, -1, 0],
            [0, 1, 0],
            [0, 0, -1],
            [0, 0, 1],
        ];
        for (&p, &color) in &self.cells {
            for (side, d) in offsets.iter().enumerate() {
                let neighbor = std::array::from_fn(|i| p[i] + d[i]);
                if self.cells.contains_key(&neighbor) {
                    continue;
                }
                let axis = side / 2;
                let normal = Vec3::from_array(d.map(f32::from));
                let center = (Vec3::from_array(p.map(f32::from)) + Vec3::splat(0.5)) * self.step
                    + normal * self.step * 0.5;
                let mut u = Vec3::ZERO;
                let mut v = Vec3::ZERO;
                u[(axis + 1) % 3] = self.step * 0.5;
                v[(axis + 2) % 3] = self.step * 0.5;
                if side % 2 == 0 {
                    u = -u;
                }
                let corners = [-u - v, u - v, u + v, -u + v];
                // The border stays on the shared voxel edge; the shallow inset
                // relief adds highlights without opening cracks in the solid.
                if relief && axis != 0 && side != 2 {
                    let inner = corners.map(|q| center + q * 0.87 + normal * self.step * 0.055);
                    quad(&mut out, inner, normal, color)?;
                    for i in 0..4 {
                        let j = (i + 1) % 4;
                        let points = [center + corners[i], center + corners[j], inner[j], inner[i]];
                        let n = (points[1] - points[0])
                            .cross(points[2] - points[0])
                            .normalize();
                        quad(&mut out, points, n, color)?;
                    }
                } else {
                    quad(&mut out, corners.map(|q| center + q), normal, color)?;
                }
            }
        }
        Ok(out)
    }
}

pub(super) fn quad(
    batch: &mut MeshBatch,
    points: [Vec3; 4],
    normal: Vec3,
    color: [f32; 3],
) -> Result<(), String> {
    let base = u32::try_from(batch.positions.len()).map_err(|_| "Item mesh overflow")?;
    base.checked_add(3).ok_or("Item mesh overflow")?;
    batch.positions.extend(points.map(|p| p.to_array()));
    batch.normals.extend([normal.to_array(); 4]);
    batch
        .uvs
        .extend([[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]);
    batch
        .colors
        .extend([Color::srgb_from_array(color).to_linear().to_f32_array(); 4]);
    batch.indices.extend([0, 1, 2, 0, 2, 3].map(|i| base + i));
    Ok(())
}

pub(super) fn shade(color: [f32; 3], p: [i16; 3], amount: f32) -> [f32; 3] {
    let hash = (i32::from(p[0]) * 73 + i32::from(p[1]) * 31 + i32::from(p[2]) * 53).rem_euclid(13);
    let gain = 1.0 + (hash as f32 / 6.0 - 1.0) * amount;
    color.map(|c| (c * gain).clamp(0.0, 1.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solid_faces_are_outward_wound_and_shared_faces_are_removed() {
        let mut sculpt = Sculpt::new(8, 1);
        sculpt.cuboid([1, 1, 1], [2, 1, 1], [0.8; 3]);
        let mesh = sculpt.mesh(0).unwrap();
        for triangle in mesh.indices.chunks_exact(3) {
            let [a, b, c] = [triangle[0], triangle[1], triangle[2]]
                .map(|i| Vec3::from_array(mesh.positions[i as usize]));
            let n = Vec3::from_array(mesh.normals[triangle[0] as usize]);
            assert!((b - a).cross(c - a).dot(n) > 0.0);
        }
        assert!(
            !mesh
                .positions
                .iter()
                .zip(&mesh.normals)
                .any(|(p, n)| { (p[0] - 0.25).abs() < 1e-6 && n[0].abs() > 0.99 })
        );
    }
}
