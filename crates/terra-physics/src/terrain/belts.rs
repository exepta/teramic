use bevy::math::DVec3;

/// Clip surfaces, not their AABBs. Section seams have no artificial vertical caps.
pub(super) fn append_clipped(
    target: &mut Vec<[[f32; 3]; 3]>,
    triangle: [[f64; 3]; 3],
    origin: [i64; 3],
) {
    clip_triangle(triangle, origin, |points| {
        target.push(points.map(|p| p.as_vec3().to_array()))
    });
}

fn clip_triangle(triangle: [[f64; 3]; 3], origin: [i64; 3], mut emit: impl FnMut([DVec3; 3])) {
    let origin = DVec3::from_array(origin.map(|v| v as f64));
    let points = triangle.map(|p| DVec3::from_array(p) - origin);
    for axis in 0..3 {
        if points.iter().all(|p| p[axis] < 0.0) || points.iter().all(|p| p[axis] > 16.0) {
            return;
        }
    }
    if points
        .iter()
        .all(|p| p.cmpge(DVec3::ZERO).all() && p.cmple(DVec3::splat(16.0)).all())
    {
        emit(points);
        return;
    }
    let mut polygon = points.to_vec();
    for axis in 0..3 {
        for upper in [false, true] {
            if polygon.len() < 3 {
                return;
            }
            let boundary = if upper { 16.0 } else { 0.0 };
            let inside = |p: DVec3| {
                if upper {
                    p[axis] <= boundary
                } else {
                    p[axis] >= boundary
                }
            };
            let mut clipped = Vec::with_capacity(polygon.len() + 1);
            let mut previous = *polygon.last().unwrap();
            let mut previous_inside = inside(previous);
            for &point in &polygon {
                let point_inside = inside(point);
                if point_inside != previous_inside {
                    let t = (boundary - previous[axis]) / (point[axis] - previous[axis]);
                    let mut crossing = previous + (point - previous) * t;
                    crossing[axis] = boundary;
                    clipped.push(crossing);
                }
                if point_inside {
                    clipped.push(point);
                }
                previous = point;
                previous_inside = point_inside;
            }
            polygon = clipped;
        }
    }
    if polygon.len() < 3 {
        return;
    }
    for index in 1..polygon.len() - 1 {
        let triangle = [polygon[0], polygon[index], polygon[index + 1]];
        if (triangle[1] - triangle[0])
            .cross(triangle[2] - triangle[0])
            .length_squared()
            > 1e-18
        {
            emit(triangle);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::{build_collider_with_geometry, model_geometry};
    use super::*;
    use terra_voxel::{BlockSection, global_to_section};
    use terra_world::conveyors::Span;
    use terra_world::{
        edits::{BlockObjectState, EditJournal},
        streaming::{ChunkStore, LoadedSection},
    };

    fn span() -> Span {
        Span {
            controller: [4, 14, 14],
            first: [4, 14, 14],
            start: [4, 13, 13],
            end: [4, 18, 18],
            step: [0, 1, 1],
            length: 4,
            facing: 0,
            wood: Default::default(),
            linked: true,
        }
    }

    #[test]
    fn continuous_sloped_run_has_the_analytic_normal_and_no_internal_slice_or_segment_faces() {
        let span = span();
        let triangles = terra_world::interaction_shapes::conveyor_triangles(span);
        assert_eq!(triangles.len(), 144);
        let expected = DVec3::from_array(span.normal().map(f64::from)).normalize();
        let upper_center = DVec3::from_array(span.start.map(|v| v as f64 + 0.5)) + expected * 0.125;
        let mut upper = 0;
        for triangle in triangles {
            let points = triangle.map(DVec3::from_array);
            let normal = (points[1] - points[0])
                .cross(points[2] - points[0])
                .normalize();
            assert!(normal.is_finite());
            if points
                .iter()
                .all(|point| (*point - upper_center).dot(expected).abs() < 1e-7)
            {
                assert!((normal - expected).length() < 1e-7);
                upper += 1;
            }
            let centroid = points.into_iter().sum::<DVec3>() / 3.0;
            if centroid.z > span.start[2] as f64 + 0.7 && centroid.z < span.end[2] as f64 + 0.3 {
                assert!(
                    normal
                        .dot(DVec3::from_array(span.direction().map(f64::from)))
                        .abs()
                        < 1e-7,
                    "no artificial cross-section wall inside the run: {normal:?}"
                );
            }
        }
        assert_eq!(upper, 2);
    }

    #[test]
    fn sloped_profile_clips_at_negative_section_seams_without_adding_vertical_faces() {
        for shift in [[0, 0, 0], [-16, -16, -16], [-32, -32, -32]] {
            let original = span();
            let span = Span {
                start: std::array::from_fn(|i| original.start[i] + shift[i]),
                end: std::array::from_fn(|i| original.end[i] + shift[i]),
                first: std::array::from_fn(|i| original.first[i] + shift[i]),
                controller: std::array::from_fn(|i| original.controller[i] + shift[i]),
                ..original
            };
            let triangles = terra_world::interaction_shapes::conveyor_triangles(span);
            let mut parts = Vec::new();
            let mut exact_parts = Vec::new();
            let mut quantization_bound = 0.0;
            for y in [0, 16] {
                for z in [0, 16] {
                    let origin = [shift[0], shift[1] + y, shift[2] + z];
                    let mut clipped = Vec::new();
                    let mut exact = Vec::new();
                    for &triangle in &triangles {
                        append_clipped(&mut clipped, triangle, origin);
                        clip_triangle(triangle, origin, |points| exact.push(points));
                    }
                    assert_eq!(clipped.len(), exact.len());
                    for (points, rounded) in exact.iter().zip(&clipped) {
                        let rounded = rounded.map(|p| DVec3::from_array(p.map(f64::from)));
                        let errors: [f64; 3] =
                            std::array::from_fn(|i| (points[i] - rounded[i]).length());
                        let u = points[1] - points[0];
                        let v = points[2] - points[0];
                        let du = errors[0] + errors[1];
                        let dv = errors[0] + errors[2];
                        // |(u+du)x(v+dv)-uxv| bounds the area error from real f32 vertex rounding.
                        quantization_bound += (u.length() * dv + v.length() * du + du * dv) * 0.5;
                    }
                    assert!(!clipped.is_empty());
                    for triangle in &clipped {
                        assert!(
                            triangle
                                .iter()
                                .flatten()
                                .all(|v| v.is_finite() && (0.0..=16.0).contains(v))
                        );
                        let points = triangle.map(|p| DVec3::from_array(p.map(f64::from)));
                        let normal = (points[1] - points[0])
                            .cross(points[2] - points[0])
                            .normalize();
                        assert!(normal.is_finite());
                        assert!(
                            !triangle.iter().all(|point| point[1] == 0.0) || normal.y.abs() < 0.999,
                            "clipping must not add a horizontal section cap"
                        );
                        assert!(
                            !triangle.iter().all(|point| point[2] == 0.0) || normal.z.abs() < 0.999,
                            "clipping must not add a vertical section cap"
                        );
                    }
                    parts.extend(clipped.into_iter().map(|triangle| {
                        triangle
                            .map(|p| std::array::from_fn(|i| f64::from(p[i]) + origin[i] as f64))
                    }));
                    let origin = DVec3::from_array(origin.map(|v| v as f64));
                    exact_parts.extend(
                        exact
                            .into_iter()
                            .map(|triangle| triangle.map(|p| (p + origin).to_array())),
                    );
                }
            }
            let area = |triangle: [[f64; 3]; 3]| {
                let p = triangle.map(DVec3::from_array);
                (p[1] - p[0]).cross(p[2] - p[0]).length() * 0.5
            };
            let original_area = triangles.iter().copied().map(area).sum::<f64>();
            let clipped_area = parts.into_iter().map(area).sum::<f64>();
            let exact_area = exact_parts.into_iter().map(area).sum::<f64>();
            assert!(
                (original_area - exact_area).abs() < 1e-10,
                "section clips must partition the same surface: original_area={original_area} exact_area={exact_area} clipped_area={clipped_area} shift={shift:?}"
            );
            assert!(
                (exact_area - clipped_area).abs() <= quantization_bound + 1e-10,
                "only bounded f32 rounding may change area: original_area={original_area} exact_area={exact_area} clipped_area={clipped_area} quantization_bound={quantization_bound} shift={shift:?}"
            );
        }
    }

    #[test]
    fn real_section_colliders_replace_only_sloped_profiles_and_keep_the_axles() {
        for shift in [0, -16] {
            let mut chunks = ChunkStore::default();
            let mut journal = EditJournal::default();
            for y in [shift, shift + 16] {
                for z in [shift, shift + 16] {
                    let section = global_to_section([shift, y, z]).0;
                    chunks.loaded.insert(
                        section,
                        LoadedSection {
                            data: BlockSection::empty(section).unwrap(),
                            mesh: None,
                            mesh_epoch: 0,
                            water_epoch: 0,
                            water: None,
                            props: Vec::new(),
                        },
                    );
                }
            }
            let start = [shift + 4, shift + 13, shift + 13];
            let end = [shift + 4, shift + 18, shift + 18];
            for position in [start, end] {
                let (section, local) = global_to_section(position);
                chunks
                    .loaded
                    .get_mut(&section)
                    .unwrap()
                    .data
                    .set_block(local, 144)
                    .unwrap();
                journal.record(position, 144).unwrap();
                journal
                    .set_object_state(position, Some(BlockObjectState::default()))
                    .unwrap();
            }
            let layout = terra_world::conveyors::layout(
                &chunks,
                &journal,
                start,
                end,
                terra_core::ItemKind::Block(148),
            )
            .unwrap();
            assert!(terra_world::conveyors::commit(&mut chunks, &mut journal, &layout).unwrap());
            let properties = terra_core::block_properties::BlockPropertiesCatalog::default();
            for section in chunks.loaded.values() {
                let geometry =
                    model_geometry(&section.data, &chunks, &journal, &properties).unwrap();
                assert!(
                    !geometry.triangles.is_empty(),
                    "each border section owns its real clipped share"
                );
                let has_axle = [start, end]
                    .into_iter()
                    .any(|position| global_to_section(position).0 == section.data.position());
                assert_eq!(
                    !geometry.boxes.is_empty(),
                    has_axle,
                    "no staircase profile boxes may remain"
                );
                let collider = build_collider_with_geometry(
                    &section.data,
                    &geometry.boxes,
                    &geometry.triangles,
                    &[144, 148],
                )
                .unwrap()
                .unwrap();
                let mesh = collider.shape().as_trimesh().unwrap();
                assert!(mesh.vertices().iter().all(|p| p.is_finite()
                    && p.cmpge(bevy::prelude::Vec3::ZERO).all()
                    && p.cmple(bevy::prelude::Vec3::splat(16.0)).all()));
                assert!(mesh.indices().len() >= geometry.triangles.len());
            }
        }
    }
}
