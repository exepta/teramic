//! Bounded triangle clipping for one imported mining model, not voxel substitutes.
use crate::meshing::builder::MeshBatch;
use bevy::prelude::*;

#[derive(Clone, Copy)]
struct Vertex {
    point: Vec3,
    normal: Vec3,
    uv: Vec2,
    color: Vec4,
}
impl Vertex {
    fn lerp(self, other: Self, t: f32) -> Self {
        Self {
            point: self.point.lerp(other.point, t),
            normal: self.normal.lerp(other.normal, t).normalize_or_zero(),
            uv: self.uv.lerp(other.uv, t),
            color: self.color.lerp(other.color, t),
        }
    }
}

pub(super) fn bounds(batches: &[MeshBatch]) -> Option<(Vec3, Vec3)> {
    let mut points = batches.iter().flat_map(|batch| &batch.positions);
    let first = Vec3::from_array(*points.next()?);
    Some(points.fold((first, first), |(low, high), point| {
        let point = Vec3::from_array(*point);
        (low.min(point), high.max(point))
    }))
}

pub(super) fn clip(
    batches: &[MeshBatch],
    axis: usize,
    plane: f32,
    less: bool,
) -> Result<Vec<MeshBatch>, String> {
    let mut result = Vec::new();
    for source in batches {
        let mut batch = MeshBatch {
            material: source.material,
            ..default()
        };
        for indices in source.indices.chunks_exact(3) {
            let mut triangle = [Vertex {
                point: Vec3::ZERO,
                normal: Vec3::ZERO,
                uv: Vec2::ZERO,
                color: Vec4::ONE,
            }; 3];
            for (corner, &index) in indices.iter().enumerate() {
                let index = index as usize;
                triangle[corner] = Vertex {
                    point: Vec3::from_array(
                        *source
                            .positions
                            .get(index)
                            .ok_or("Invalid mining model position")?,
                    ),
                    normal: Vec3::from_array(
                        *source
                            .normals
                            .get(index)
                            .ok_or("Invalid mining model normal")?,
                    ),
                    uv: Vec2::from_array(*source.uvs.get(index).ok_or("Invalid mining model UV")?),
                    color: Vec4::from_array(source.colors.get(index).copied().unwrap_or([1.0; 4])),
                };
            }
            // A surface lying on the cut belongs to exactly one side.
            if !less && triangle.iter().all(|vertex| vertex.point[axis] == plane) {
                continue;
            }
            let inside = |v: Vertex| {
                if less {
                    v.point[axis] <= plane
                } else {
                    v.point[axis] >= plane
                }
            };
            let mut polygon = [triangle[0]; 4];
            let mut length = 0;
            let mut previous = triangle[2];
            for current in triangle {
                if inside(previous) != inside(current) {
                    polygon[length] = previous.lerp(
                        current,
                        (plane - previous.point[axis])
                            / (current.point[axis] - previous.point[axis]),
                    );
                    length += 1;
                }
                if inside(current) {
                    polygon[length] = current;
                    length += 1;
                }
                previous = current;
            }
            for corner in 1..length.saturating_sub(1) {
                let vertices = [polygon[0], polygon[corner], polygon[corner + 1]];
                if (vertices[1].point - vertices[0].point)
                    .cross(vertices[2].point - vertices[0].point)
                    .length_squared()
                    < 1.0e-14
                {
                    continue;
                }
                let base = u32::try_from(batch.positions.len())
                    .map_err(|_| "Mining model index overflow")?;
                for vertex in vertices {
                    batch.positions.push(vertex.point.to_array());
                    batch.normals.push(vertex.normal.to_array());
                    batch.uvs.push(vertex.uv.to_array());
                    if !source.colors.is_empty() {
                        batch.colors.push(vertex.color.to_array());
                    }
                }
                batch.indices.extend([base, base + 1, base + 2]);
            }
        }
        if !batch.indices.is_empty() {
            result.push(batch);
        }
    }
    Ok(result)
}

/// At most eight pieces per material. Their union is precisely the imported
/// surface, including narrow legs, handles, gaps, tint and texture coordinates.
pub(super) fn partition(batches: Vec<MeshBatch>) -> Result<Vec<(Vec3, Vec<MeshBatch>)>, String> {
    let Some((low, high)) = bounds(&batches) else {
        return Ok(Vec::new());
    };
    let midpoint = (low + high) * 0.5;
    let mut pieces = vec![batches];
    for axis in 0..3 {
        let mut next = Vec::with_capacity(pieces.len() * 2);
        for piece in pieces {
            for less in [true, false] {
                let split = clip(&piece, axis, midpoint[axis], less)?;
                if !split.is_empty() {
                    next.push(split);
                }
            }
        }
        pieces = next;
    }
    Ok(pieces
        .into_iter()
        .filter_map(|mut piece| {
            let (low, high) = bounds(&piece)?;
            let center = (low + high) * 0.5;
            for point in piece.iter_mut().flat_map(|batch| &mut batch.positions) {
                *point = (Vec3::from_array(*point) - center).to_array();
            }
            Some((center, piece))
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn surface_area(batches: &[MeshBatch]) -> f32 {
        batches
            .iter()
            .flat_map(|batch| {
                batch.indices.chunks_exact(3).map(|indices| {
                    let [a, b, c] = std::array::from_fn(|corner| {
                        Vec3::from_array(batch.positions[indices[corner] as usize])
                    });
                    (b - a).cross(c - a).length() * 0.5
                })
            })
            .sum()
    }
    #[test]
    fn partition_preserves_exact_imported_surface_without_filling_model_gaps() {
        let mut batches = std::collections::BTreeMap::new();
        for x in [-0.7, 0.7] {
            crate::props::add_box(
                &mut batches,
                3,
                Vec3::new(x, 0.4, 0.0),
                Vec3::new(0.12, 0.8, 0.16),
                Quat::IDENTITY,
            )
            .unwrap();
        }
        let mut batches: Vec<_> = batches.into_values().collect();
        for batch in &mut batches {
            batch
                .colors
                .resize(batch.positions.len(), [0.4, 0.7, 0.2, 1.0]);
        }
        let area = surface_area(&batches);
        let parts = partition(batches).unwrap();
        assert!(parts.len() <= 8 && parts.len() >= 2);
        let mut restored = Vec::new();
        for (center, mut piece) in parts {
            for batch in &mut piece {
                assert_eq!(batch.material, 3);
                assert_eq!(batch.normals.len(), batch.positions.len());
                assert_eq!(batch.uvs.len(), batch.positions.len());
                assert_eq!(batch.colors.len(), batch.positions.len());
                assert!(
                    batch
                        .indices
                        .iter()
                        .all(|&i| (i as usize) < batch.positions.len())
                );
                for point in &mut batch.positions {
                    *point = (Vec3::from_array(*point) + center).to_array();
                    assert!(point[0].abs() > 0.63, "model gap was filled");
                }
            }
            restored.extend(piece);
        }
        assert!((surface_area(&restored) - area).abs() < 1.0e-5);
    }
    #[test]
    fn chest_half_clipping_is_disjoint_and_preserves_shared_plane_once() {
        let mut batches = std::collections::BTreeMap::new();
        crate::props::add_box(
            &mut batches,
            7,
            Vec3::new(1.0, 0.4, 0.5),
            Vec3::new(1.8, 0.8, 0.8),
            Quat::IDENTITY,
        )
        .unwrap();
        let batches: Vec<_> = batches.into_values().collect();
        let a = clip(&batches, 0, 1.0, true).unwrap();
        let b = clip(&batches, 0, 1.0, false).unwrap();
        assert!(a.iter().flat_map(|b| &b.positions).all(|p| p[0] <= 1.0));
        assert!(b.iter().flat_map(|b| &b.positions).all(|p| p[0] >= 1.0));
        assert!((surface_area(&a) + surface_area(&b) - surface_area(&batches)).abs() < 1.0e-5);
    }
}
