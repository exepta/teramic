use super::{PrimitiveTemplate, SideCap};
use bevy::math::Vec3;

/// One shared rounded-cube generator; all points stay in the logical unit cell.
/// Face borders use identical samples so the existing neighbor flattening closes joins.
pub(crate) fn cube_faces(
    bevel: f32,
    materials: [usize; 6],
    cap: Option<SideCap>,
) -> [Vec<PrimitiveTemplate>; 6] {
    std::array::from_fn(|side| {
        let axis = side / 2;
        let (u, v) = match axis {
            0 => (2, 1),
            1 => (0, 2),
            _ => (0, 1),
        };
        let mut coordinates = if bevel == 0.0 {
            vec![0.0, 1.0]
        } else {
            vec![0.0, bevel, 1.0 - bevel, 1.0]
        };
        if cap.is_some() {
            coordinates.push(0.5);
            coordinates.sort_by(f32::total_cmp);
            coordinates.dedup();
        }
        let vertical = if axis != 1 {
            if let Some(cap) = cap {
                coordinates.push(1.0 - cap.height);
                coordinates.push((1.0 - cap.height - 0.08).max(bevel));
            }
            coordinates.sort_by(f32::total_cmp);
            coordinates.dedup();
            coordinates.clone()
        } else {
            coordinates.clone()
        };
        // A cap split belongs only to the vertical axis; keep other samples shared.
        let mut horizontal = if bevel == 0.0 {
            vec![0.0, 1.0]
        } else {
            vec![0.0, bevel, 1.0 - bevel, 1.0]
        };
        if cap.is_some() {
            horizontal.push(0.5);
            horizontal.sort_by(f32::total_cmp);
            horizontal.dedup();
        }
        let mut primitives = vec![empty_primitive(materials[side])];
        if let Some(cap) = cap.filter(|_| axis != 1) {
            primitives.push(empty_primitive(cap.material));
        }
        for range_v in vertical.windows(2) {
            for range_u in horizontal.windows(2) {
                let mut points = [[0.0; 3]; 4];
                for (point, [x, y]) in points.iter_mut().zip([
                    [range_u[0], range_v[0]],
                    [range_u[1], range_v[0]],
                    [range_u[1], range_v[1]],
                    [range_u[0], range_v[1]],
                ]) {
                    point[axis] = (side % 2) as f32;
                    point[u] = x;
                    point[v] = y;
                    if axis != 1
                        && let Some(cap) = cap
                        && (point[1] - (1.0 - cap.height)).abs() < 0.00001
                    {
                        let along = point[u];
                        let tooth = [0.0, 0.035, -0.025, 0.055, -0.015, 0.025, 0.0]
                            [((along * 6.0).round() as usize).min(6)];
                        point[1] -= tooth * (along * std::f32::consts::PI).sin().max(0.0);
                    }
                    if bevel > 0.0 {
                        let outer = Vec3::from_array(*point);
                        let inner = outer.clamp(Vec3::splat(bevel), Vec3::splat(1.0 - bevel));
                        *point = (inner + (outer - inner).normalize() * bevel).to_array();
                    }
                }
                let material_index = usize::from(
                    axis != 1 && cap.is_some_and(|cap| range_v[0] >= 1.0 - cap.height - 0.00001),
                );
                append_quad(&mut primitives[material_index], points, side, u, v);
            }
        }
        primitives.retain(|p| !p.indices.is_empty());
        primitives
    })
}

fn empty_primitive(material: usize) -> PrimitiveTemplate {
    PrimitiveTemplate {
        positions: Vec::new(),
        normals: Vec::new(),
        uvs: Vec::new(),
        indices: Vec::new(),
        material,
    }
}

pub(crate) fn box_model(
    boxes: &[terra_core::block_shapes::ShapeBox],
    bevel: f32,
    materials: [usize; 6],
) -> Vec<PrimitiveTemplate> {
    let mut model = Vec::with_capacity(boxes.len() * 6);
    for bounds in boxes {
        let low = Vec3::from_array(bounds.minimum.map(|v| v as f32));
        let size = Vec3::from_array(std::array::from_fn(|i| {
            (bounds.maximum[i] - bounds.minimum[i]) as f32
        }));
        for (side, faces) in cube_faces(bevel, materials, None).into_iter().enumerate() {
            let (u, v) = match side / 2 {
                0 => (2, 1),
                1 => (0, 2),
                _ => (0, 1),
            };
            for mut face in faces {
                for position in &mut face.positions {
                    *position = (low + Vec3::from_array(*position) * size).to_array();
                }
                for normal in &mut face.normals {
                    *normal = (Vec3::from_array(*normal) / size)
                        .normalize_or_zero()
                        .to_array();
                }
                for uv in &mut face.uvs {
                    *uv = [low[u] + uv[0] * size[u], low[v] + uv[1] * size[v]];
                }
                model.push(face);
            }
        }
    }
    model
}

fn append_quad(
    primitive: &mut PrimitiveTemplate,
    points: [[f32; 3]; 4],
    side: usize,
    u: usize,
    v: usize,
) {
    let [a, b, _, d] = points.map(Vec3::from_array);
    let normal = (b - a).cross(d - a).normalize();
    let outward = if side.is_multiple_of(2) { -1.0 } else { 1.0 };
    let reverse = normal[side / 2] * outward < 0.0;
    let normal = if reverse { -normal } else { normal };
    let start = primitive.positions.len() as u32;
    primitive.positions.extend(points);
    primitive.normals.extend([normal.to_array(); 4]);
    primitive
        .uvs
        .extend(points.map(|point| [point[u], point[v]]));
    let indices = if reverse {
        [0, 2, 1, 0, 3, 2]
    } else {
        [0, 1, 2, 0, 2, 3]
    };
    primitive.indices.extend(indices.map(|i| start + i));
}

#[cfg(test)]
mod construction_tests {
    use super::*;
    #[test]
    fn box_templates_have_finite_geometry_valid_indices_and_unit_normals() {
        for id in terra_core::construction::FIRST..=terra_core::construction::LAST {
            let boxes = terra_core::construction::boxes(id);
            let model = box_model(boxes, 0.018, [0; 6]);
            assert_eq!(model.len(), boxes.len() * 6);
            for primitive in model {
                assert!(primitive.positions.iter().flatten().all(|x| x.is_finite()));
                assert!(
                    primitive
                        .indices
                        .iter()
                        .all(|i| (*i as usize) < primitive.positions.len())
                );
                assert!(
                    primitive
                        .normals
                        .iter()
                        .all(|n| (Vec3::from_array(*n).length() - 1.0).abs() < 0.0001)
                );
            }
        }
    }
}
