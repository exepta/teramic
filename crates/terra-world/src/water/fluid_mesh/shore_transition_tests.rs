use super::*;
use crate::{
    TerraGenerator,
    edits::{BlockObjectState, EditJournal},
};
use std::collections::BTreeMap;
use terra_core::{WorldConfig, plumbing};

fn gradient(coordinate: i64, edge: i64) -> f64 {
    (7.2 + (coordinate - edge + 2) as f64 * 0.075).clamp(7.2, 7.5)
}

fn border_edges(patch: &WaterPatch, origin: [i64; 3], axis: usize, edge: i64) -> Vec<[f32; 4]> {
    let tangent = 2 - axis;
    let mut edges = Vec::new();
    for triangle in patch.indices.chunks_exact(3) {
        if patch.normals[triangle[0] as usize][1] < 0.9 {
            continue;
        }
        for [a, b] in [[0, 1], [1, 2], [2, 0]] {
            let a = patch.positions[triangle[a] as usize];
            let b = patch.positions[triangle[b] as usize];
            if a[axis] + origin[axis] as f32 != edge as f32
                || b[axis] + origin[axis] as f32 != edge as f32
            {
                continue;
            }
            let a = [a[tangent] + origin[tangent] as f32, a[1] + origin[1] as f32];
            let b = [b[tangent] + origin[tangent] as f32, b[1] + origin[1] as f32];
            if a[0] == b[0] || a[0].min(b[0]) < 6.0 || a[0].max(b[0]) > 11.0 {
                continue;
            }
            let [a, b] = if a[0] < b[0] { [a, b] } else { [b, a] };
            edges.push([a[0], b[0], a[1], b[1]]);
        }
    }
    edges.sort_by(|a, b| {
        a.iter()
            .zip(b)
            .find_map(|(a, b)| {
                let c = a.total_cmp(b);
                (c != std::cmp::Ordering::Equal).then_some(c)
            })
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    edges.dedup();
    edges
}

const HEIGHT_EPSILON: f64 = 1e-6;
type BorderWaves = BTreeMap<(u32, u32), u32>;

fn interpolate(segment: &[f32; 4], coordinate: f64) -> f64 {
    let [start, end, height_start, height_end] = segment.map(f64::from);
    height_start + (height_end - height_start) * (coordinate - start) / (end - start)
}

fn curve_height(segments: &[[f32; 4]], coordinate: f64) -> Option<f64> {
    let mut height: Option<f64> = None;
    for segment in segments {
        if coordinate < f64::from(segment[0]) || coordinate > f64::from(segment[1]) {
            continue;
        }
        let next = interpolate(segment, coordinate);
        if let Some(previous) = height {
            assert!(
                (previous - next).abs() <= HEIGHT_EPSILON,
                "overlapping boundary segments disagree at {coordinate}: {previous} vs {next}"
            );
        }
        height = Some(next);
    }
    height
}

fn assert_same_boundary_curve(left: &[[f32; 4]], right: &[[f32; 4]], axis: usize, edge: i64) {
    // Clipping may split a collinear edge. The union of all endpoints partitions
    // both piecewise-linear curves; checking each knot and interval midpoint
    // detects both differing heights and uncovered intervals on either side.
    let mut knots = vec![6.0, 11.0];
    for segment in left.iter().chain(right) {
        knots.extend([f64::from(segment[0]), f64::from(segment[1])]);
    }
    knots.sort_by(f64::total_cmp);
    knots.dedup();
    let mut samples = knots.clone();
    samples.extend(knots.windows(2).map(|pair| (pair[0] + pair[1]) * 0.5));
    for coordinate in samples {
        let left = curve_height(left, coordinate).unwrap_or_else(|| {
            panic!("axis={axis} edge={edge}: left water boundary has a gap at {coordinate}")
        });
        let right = curve_height(right, coordinate).unwrap_or_else(|| {
            panic!("axis={axis} edge={edge}: right water boundary has a gap at {coordinate}")
        });
        assert!(
            (left - right).abs() <= HEIGHT_EPSILON,
            "axis={axis} edge={edge}: boundary height mismatch at {coordinate}: {left} vs {right}"
        );
    }
}

fn assert_wave_vertices_match(
    own: &BorderWaves,
    other: &BorderWaves,
    other_segments: &[[f32; 4]],
    axis: usize,
    edge: i64,
) {
    for (&vertex, &weight) in own {
        if let Some(&other_weight) = other.get(&vertex) {
            assert_eq!(
                weight, other_weight,
                "axis={axis} edge={edge}: coincident water vertices must have identical displacement weights"
            );
            continue;
        }
        let coordinate = f64::from(f32::from_bits(vertex.0));
        let height = f64::from(f32::from_bits(vertex.1));
        let segment = other_segments.iter().find(|segment| {
            coordinate > f64::from(segment[0])
                && coordinate < f64::from(segment[1])
                && (height - interpolate(segment, coordinate)).abs() <= HEIGHT_EPSILON
        }).unwrap_or_else(|| panic!("axis={axis} edge={edge}: extra vertex {coordinate}/{height} does not subdivide the opposite boundary"));
        // Extra vertices change where the wave function is sampled. They are
        // harmless only when the whole opposite edge and the new vertex are
        // pinned, rather than merely having interpolated nonzero weights.
        assert_eq!(
            f32::from_bits(weight),
            0.0,
            "axis={axis} edge={edge}: an extra moving vertex would reopen the geometric seam"
        );
        for endpoint in [(segment[0], segment[2]), (segment[1], segment[3])] {
            let key = (endpoint.0.to_bits(), endpoint.1.to_bits());
            let &endpoint_weight = other
                .get(&key)
                .expect("each boundary segment endpoint has a wave weight");
            assert_eq!(
                f32::from_bits(endpoint_weight),
                0.0,
                "axis={axis} edge={edge}: subdivided opposite edge must be fully pinned"
            );
        }
    }
}

#[test]
fn sloping_river_pipe_and_one_cell_natural_fringe_share_exact_top_edges_across_xz_sections() {
    let terrain = TerraGenerator::new(WorldConfig {
        base_height: 3.,
        sea_level: 8.,
        hill_amplitude: 0.,
        mountain_amplitude: 0.,
        biomes: false,
        caves: false,
        rivers: false,
        trees: false,
        ..Default::default()
    })
    .unwrap();
    let column = terrain.surface_column(0.5, 0.5);
    for axis in [0, 2] {
        for edge in [-16i64, 16] {
            let mut pipe = [8, 7, 8];
            pipe[axis] = edge;
            let mut journal = EditJournal::default();
            journal.record(pipe, plumbing::WOOD_PIPE).unwrap();
            let pose = BlockObjectState {
                facing: 4,
                ..Default::default()
            };
            journal.set_object_state(pipe, Some(pose)).unwrap();
            let boxes =
                crate::plumbing::plumbing_boxes(&journal, pipe, plumbing::WOOD_PIPE, pose, |_| {
                    None
                })
                .into_iter()
                .map(|b| ShapeBox {
                    minimum: std::array::from_fn(|i| b.minimum[i] + pipe[i] as f64),
                    maximum: std::array::from_fn(|i| b.maximum[i] + pipe[i] as f64),
                    hinged: false,
                })
                .collect();
            let occluders = FluidOccluders::from([(pipe, boxes)]);
            let mut fluids = FluidSnapshot::new();
            let mut precise = HashMap::new();
            // These are the actual natural water cells surrounding construction,
            // not invented constant-height water used just to close a seam.
            for dz in -1..=1 {
                for dx in -1..=1 {
                    let p = [pipe[0] + dx, 7, pipe[2] + dz];
                    let level = gradient(p[axis], edge);
                    fluids.insert(
                        p,
                        super::super::FluidCell {
                            amount: ((level - 7.) * 8.).ceil() as u8,
                            source: true,
                            flow: [0; 2],
                        },
                    );
                    precise.insert(p, level);
                }
            }
            let mut left = [0, 0, 0];
            left[axis] = edge.div_euclid(16) - 1;
            let mut right = left;
            right[axis] += 1;
            let mut borders = Vec::new();
            let mut waves = Vec::new();
            for owner in [left, right] {
                let position = SectionPos::new(owner[0], owner[1], owner[2]);
                let origin = position.origin().unwrap();
                let mut columns = vec![column; 18 * 18];
                for z in 0..18 {
                    for x in 0..18 {
                        let p = [origin[0] + x as i64 - 1, 7, origin[2] + z as i64 - 1];
                        columns[x + 18 * z].height = 3.;
                        columns[x + 18 * z].water_level = Some(gradient(p[axis], edge));
                        columns[x + 18 * z].water_flow = [0.2, 0.];
                    }
                }
                let mut padded = vec![0; 18 * 18 * 18];
                let local: [i64; 3] = std::array::from_fn(|i| pipe[i] - origin[i] + 1);
                let [x, y, z] = local.map(|v| v as usize);
                padded[x + 18 * (y + 18 * z)] = plumbing::WOOD_PIPE;
                let mut natural = padded.clone();
                for p in fluids.keys() {
                    let local: [i64; 3] = std::array::from_fn(|i| p[i] - origin[i] + 1);
                    if local.iter().all(|v| (0..18).contains(v)) {
                        let [x, y, z] = local.map(|v| v as usize);
                        natural[x + 18 * (y + 18 * z)] = u16::MAX;
                    }
                }
                let mut patch = super::super::channels::build_channel_patch(
                    position, origin, &columns, &natural,
                );
                append_fluid_patch(
                    &mut patch,
                    position,
                    &columns,
                    &padded,
                    &fluids,
                    (true, &occluders, &precise),
                );
                let patch = patch.unwrap();
                let edges = border_edges(&patch, origin, axis, edge);
                assert!(!edges.is_empty());
                assert!(
                    edges.iter().any(|e| e[2] != e[3]),
                    "the test must retain a real sloped transition"
                );
                borders.push(edges);
                let tangent = 2 - axis;
                let mut border_weights = BTreeMap::new();
                for ((p, n), weight) in patch
                    .positions
                    .iter()
                    .zip(&patch.normals)
                    .zip(&patch.wave_weights)
                {
                    let t = p[tangent] + origin[tangent] as f32;
                    if n[1] >= 0.9
                        && p[axis] + origin[axis] as f32 == edge as f32
                        && (6.0..=11.0).contains(&t)
                    {
                        let key = (t.to_bits(), (p[1] + origin[1] as f32).to_bits());
                        if let Some(previous) = border_weights.insert(key, weight.to_bits()) {
                            assert_eq!(
                                previous,
                                weight.to_bits(),
                                "coincident top vertices must displace equally"
                            );
                        }
                    }
                }
                waves.push(border_weights);
            }
            assert_same_boundary_curve(&borders[0], &borders[1], axis, edge);
            assert_wave_vertices_match(&waves[0], &waves[1], &borders[1], axis, edge);
            assert_wave_vertices_match(&waves[1], &waves[0], &borders[0], axis, edge);
        }
    }
}
