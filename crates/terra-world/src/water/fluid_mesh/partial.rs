use super::*;
use crate::water::{FluidCell, space::FluidSpace};

/// Only cells touching construction geometry take this exact path. Open water
/// retains the existing smoothed surface and shared, inexpensive channel mesh.
pub(super) fn append(
    patch: &mut Option<WaterPatch>,
    p: [i64; 3],
    cell: FluidCell,
    origin: [i64; 3],
    occluders: &FluidOccluders,
    effects: [f32; 4],
    sample: impl Fn([i64; 3]) -> Option<f64>,
) {
    let space = FluidSpace::from_global(occluders.get(&p).map_or(&[], Vec::as_slice), p);
    let above = [p[0], p[1] + 1, p[2]];
    let covered = sample(above).is_some_and(|height| {
        let upper =
            FluidSpace::from_global(occluders.get(&above).map_or(&[], Vec::as_slice), above);
        space.connected(
            FluidCell { amount: 8, ..cell },
            &upper,
            FluidCell {
                amount: ((height - above[1] as f64) * 8.0).ceil() as u8,
                ..cell
            },
            [0, 1, 0],
        )
    });
    let water = space.wet_boxes(if covered { 1.0 } else { cell.height() });
    let surface =
        water.iter().map(|b| b.maximum[1]).fold(0.0, f64::max) as f32 + (p[1] - origin[1]) as f32;
    for (index, bounds) in water.iter().enumerate() {
        for direction in super::super::fluid::NEIGHBORS {
            let axis = direction.iter().position(|&v| v != 0).unwrap();
            let high = direction[axis] > 0;
            let tangents: Vec<_> = (0..3).filter(|&i| i != axis).collect();
            let mut corners = [[0.0; 3]; 4];
            for (corner, [a, b]) in [[0, 0], [1, 0], [1, 1], [0, 1]].into_iter().enumerate() {
                corners[corner][axis] = if high {
                    bounds.maximum[axis]
                } else {
                    bounds.minimum[axis]
                };
                for (i, upper) in [(tangents[0], a), (tangents[1], b)] {
                    corners[corner][i] = if upper == 0 {
                        bounds.minimum[i]
                    } else {
                        bounds.maximum[i]
                    };
                }
            }
            // The tangent basis has alternating handedness.
            if high != (axis != 1) {
                corners.reverse();
            }
            let face = corners
                .map(|v| std::array::from_fn(|i| (v[i] + p[i] as f64 - origin[i] as f64) as f32));
            let normal = direction.map(|v| v as f32);
            let mut masks = Vec::new();
            let global = |b: &ShapeBox, at: [i64; 3]| ShapeBox {
                minimum: std::array::from_fn(|i| b.minimum[i] + at[i] as f64),
                maximum: std::array::from_fn(|i| b.maximum[i] + at[i] as f64),
                hinged: false,
            };
            masks.extend(
                water
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| *i != index)
                    .map(|(_, b)| global(b, p)),
            );
            masks.extend(space.solids.iter().map(|b| global(b, p)));
            if let Some(q) = super::super::fluid::adjacent(p, direction) {
                masks.extend(occluders.get(&q).into_iter().flatten().copied());
                if let Some(height) = sample(q) {
                    let neighbor =
                        FluidSpace::from_global(occluders.get(&q).map_or(&[], Vec::as_slice), q);
                    let upper_position = [q[0], q[1] + 1, q[2]];
                    let neighbor_covered = sample(upper_position).is_some_and(|upper_height| {
                        let upper = FluidSpace::from_global(
                            occluders.get(&upper_position).map_or(&[], Vec::as_slice),
                            upper_position,
                        );
                        neighbor.connected(
                            FluidCell { amount: 8, ..cell },
                            &upper,
                            FluidCell {
                                amount: ((upper_height - upper_position[1] as f64) * 8.0).ceil()
                                    as u8,
                                ..cell
                            },
                            [0, 1, 0],
                        )
                    });
                    let height = if neighbor_covered {
                        1.0
                    } else {
                        height - q[1] as f64
                    };
                    // Free neighbors share pinned construction corners. Their
                    // rendered edge cannot fall below this waterlogged cell.
                    let height = if neighbor.solids.is_empty() && axis != 1 {
                        height.max(f64::from(surface) - (p[1] - origin[1]) as f64)
                    } else {
                        height
                    };
                    masks.extend(neighbor.wet_boxes(height).iter().map(|b| global(b, q)));
                }
            }
            for fragment in occlusion::fragments(face, normal, origin, &masks) {
                let target = patch.get_or_insert_with(empty_patch);
                target
                    .normals
                    .resize(target.positions.len(), [0.0, 1.0, 0.0]);
                target.flow.resize(target.positions.len(), [0.0; 2]);
                target
                    .surface_effects
                    .resize(target.positions.len(), [0.0; 4]);
                let mut effects = if axis != 1 { effects } else { [0.0; 4] };
                if effects[3] == 5.0 {
                    // Clipping must retain the original curtain's crest/impact
                    // distances rather than restarting its foam on each piece.
                    let top = fragment
                        .iter()
                        .map(|v| v[1])
                        .fold(f32::NEG_INFINITY, f32::max);
                    let bottom = fragment.iter().map(|v| v[1]).fold(f32::INFINITY, f32::min);
                    effects[0] += surface - top;
                    effects[1] += bottom - (p[1] - origin[1]) as f32;
                }
                polygon(
                    target,
                    &fragment,
                    normal,
                    cell.flow.map(f32::from),
                    cell.height() as f32,
                    effects,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::water::{FluidCell, space::FluidSpace};

    #[test]
    fn covered_neighbor_uses_the_same_falling_height_for_occlusion_as_its_actual_mesh() {
        let p = [8, 7, 8];
        let q = [9, 7, 8];
        let occluders = FluidOccluders::from([(
            p,
            vec![ShapeBox {
                minimum: [8.0, 7.0, 8.0],
                maximum: [9.0, 7.5, 9.0],
                hinged: false,
            }],
        )]);
        let mut patch = None;
        append(
            &mut patch,
            p,
            FluidCell {
                amount: 8,
                ..Default::default()
            },
            [0; 3],
            &occluders,
            [0.0, 8.0, 1.0, 5.0],
            |position| {
                if position == p {
                    Some(8.0)
                } else if position == q {
                    Some(7.125)
                } else if position == [9, 8, 8] {
                    Some(8.125)
                } else {
                    None
                }
            },
        );
        let patch = patch.unwrap();
        assert!(
            patch
                .normals
                .iter()
                .all(|normal| *normal != [1.0, 0.0, 0.0]),
            "a connected falling neighbor fills its visible cell; do not emit overlapping internal half-faces"
        );
    }

    #[test]
    fn every_waterlogged_rotation_has_valid_exposed_geometry_without_solid_intersections() {
        for origin in [[0, 0, 0], [-16, -16, -16]] {
            let p = [origin[0] + 15, origin[1] + 8, origin[2] + 8];
            for (material, facings) in [(82, 58), (74, 24)] {
                for facing in 0..facings {
                    let solids =
                        terra_core::construction::placed_boxes(material, facing, [None; 6]);
                    let boxes = solids
                        .iter()
                        .map(|b| ShapeBox {
                            minimum: std::array::from_fn(|i| b.minimum[i] + p[i] as f64),
                            maximum: std::array::from_fn(|i| b.maximum[i] + p[i] as f64),
                            hinged: false,
                        })
                        .collect();
                    let occluders = FluidOccluders::from([(p, boxes)]);
                    let mut patch = None;
                    append(
                        &mut patch,
                        p,
                        FluidCell {
                            amount: 8,
                            ..Default::default()
                        },
                        origin,
                        &occluders,
                        [0.0; 4],
                        |q| (q == p).then_some(p[1] as f64 + 1.0),
                    );
                    let space = FluidSpace { solids };
                    if space.wet_boxes(1.0).is_empty() {
                        assert!(patch.is_none());
                        continue;
                    }
                    let patch = patch.unwrap();
                    assert_eq!(patch.positions.len(), patch.normals.len());
                    assert_eq!(patch.positions.len(), patch.surface_effects.len());
                    assert!(!patch.indices.is_empty());
                    for triangle in patch.indices.chunks_exact(3) {
                        let points: [[f32; 3]; 3] =
                            std::array::from_fn(|i| patch.positions[triangle[i] as usize]);
                        let a: [f32; 3] = std::array::from_fn(|i| points[1][i] - points[0][i]);
                        let b: [f32; 3] = std::array::from_fn(|i| points[2][i] - points[0][i]);
                        let cross = [
                            a[1] * b[2] - a[2] * b[1],
                            a[2] * b[0] - a[0] * b[2],
                            a[0] * b[1] - a[1] * b[0],
                        ];
                        let n = patch.normals[triangle[0] as usize];
                        assert!(
                            (0..3).map(|i| cross[i] * n[i]).sum::<f32>() > 1e-9,
                            "material={material} facing={facing}"
                        );
                        let inside_water: [f64; 3] = std::array::from_fn(|i| {
                            points.iter().map(|v| f64::from(v[i])).sum::<f64>() / 3.0
                                + origin[i] as f64
                                - p[i] as f64
                                - f64::from(n[i]) * 1e-5
                        });
                        assert!(
                            space.contains(
                                FluidCell {
                                    amount: 8,
                                    ..Default::default()
                                },
                                inside_water
                            ),
                            "solid intersection material={material} facing={facing} point={inside_water:?}"
                        );
                    }
                }
            }
        }
    }
}
