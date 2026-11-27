//! UV-local geometry shared by the sculpted block families. The third
//! coordinate is signed depth, with outward relief at negative values.
use crate::blocks::{PrimitiveTemplate, tangent_frame};
use bevy::math::{Vec2, Vec3};

pub(super) struct FacePatch {
    pub(super) side: usize,
    pub(super) variant: usize,
    pub(super) meshes: Vec<PrimitiveTemplate>,
}

type Point = [f32; 3];

impl FacePatch {
    pub(super) fn new(side: usize, variant: usize) -> Self {
        Self {
            side,
            variant: variant % 4,
            meshes: Vec::new(),
        }
    }

    /// Keep the supplied counterclockwise winding, including undercuts.
    pub(super) fn wound_polygon<const N: usize>(&mut self, slot: usize, points: [Point; N]) {
        self.write_polygon(slot, points, None);
    }

    /// Projected surface relief may be authored mirrored or rotated.
    pub(super) fn surface_polygon<const N: usize>(&mut self, slot: usize, points: [Point; N]) {
        self.write_polygon(slot, points, Some(true));
    }

    fn write_polygon<const N: usize>(
        &mut self,
        slot: usize,
        points: [Point; N],
        outward: Option<bool>,
    ) {
        let side = self.side;
        let axis = side / 2;
        let (u_axis, v_axis) = match axis {
            0 => (2, 1),
            1 => (0, 2),
            _ => (0, 1),
        };
        let world = points.map(|[u, v, depth]| {
            let mut p = Vec3::ZERO;
            p[axis] = if side % 2 == 0 { depth } else { 1.0 - depth };
            p[u_axis] = u;
            p[v_axis] = v;
            p
        });
        let slot_index = self
            .meshes
            .iter()
            .position(|p| p.material == slot)
            .unwrap_or_else(|| {
                self.meshes.push(PrimitiveTemplate {
                    positions: Vec::new(),
                    normals: Vec::new(),
                    tangents: Vec::new(),
                    uvs: Vec::new(),
                    indices: Vec::new(),
                    material: slot,
                });
                self.meshes.len() - 1
            });
        let mesh = &mut self.meshes[slot_index];
        let mut shared = [None; N];
        let mut first_normal = None;
        for j in 1..N - 1 {
            let Some(mut normal) = (world[j] - world[0])
                .cross(world[j + 1] - world[0])
                .try_normalize()
            else {
                continue;
            };
            let reverse = match outward {
                None => matches!(side, 1 | 3 | 4),
                Some(front) => ((normal[axis] > 0.0) == (side % 2 != 0)) != front,
            };
            if reverse {
                normal = -normal;
            }
            let reference = *first_normal.get_or_insert(normal);
            let planar = normal.dot(reference) >= 0.9999;
            let normal = if planar { reference } else { normal };
            // On a steep bevel the face projection can collapse an entire wall
            // to one texel. Unfold its height into the adjoining tile.
            let fold_u = normal[u_axis].abs() > normal[v_axis].abs();
            let uv = |i: usize| {
                let [u, v, depth] = points[i];
                let mut uv = Vec2::new(u, v);
                if normal[axis].abs() < 0.5 {
                    if fold_u {
                        uv.x += normal[u_axis].signum() * depth;
                    } else {
                        uv.y += normal[v_axis].signum() * depth;
                    }
                }
                uv
            };
            let (du, dv) = (uv(j) - uv(0), uv(j + 1) - uv(0));
            let (ab, ac) = (world[j] - world[0], world[j + 1] - world[0]);
            let sign = du.perp_dot(dv).signum();
            let along_u = (ab * dv.y - ac * du.y) * sign;
            let along_v = (ac * du.x - ab * dv.x) * sign;
            let tangent = tangent_frame(along_u, normal, normal.cross(along_u).dot(along_v));
            let tri = if reverse {
                [0, j + 1, j]
            } else {
                [0, j, j + 1]
            };
            for i in tri {
                let index = if let Some(index) = shared[i].filter(|_| planar) {
                    index
                } else {
                    let index = mesh.positions.len() as u32;
                    mesh.positions.push(world[i].to_array());
                    mesh.normals.push(normal.to_array());
                    mesh.tangents.push(tangent);
                    mesh.uvs.push(uv(i).to_array());
                    if planar {
                        shared[i] = Some(index);
                    }
                    index
                };
                mesh.indices.push(index);
            }
        }
    }
}
