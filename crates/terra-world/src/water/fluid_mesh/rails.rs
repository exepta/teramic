//! Clip liquid faces against the authored rail solids, rather than their voxel AABBs.
use std::{
    collections::{BTreeMap, VecDeque},
    sync::{Arc, Mutex, OnceLock},
};
use terra_core::rails::{Beam, Piece};
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Profile {
    bounds: terra_core::block_shapes::ShapeBox,
    center: [f64; 3],
    planes: Vec<([f64; 3], f64)>,
}
type Tiles = BTreeMap<[i32; 3], Arc<[Profile]>>;
type ProfileCache = Mutex<VecDeque<(Piece, Arc<Tiles>)>>;
pub(crate) fn profiles(mut piece: Piece, cell: [i64; 3]) -> Arc<[Profile]> {
    static CACHE: OnceLock<ProfileCache> = OnceLock::new();
    let offset = std::array::from_fn(|i| (cell[i] - piece.anchor[i]) as i32);
    piece.anchor = [0; 3];
    let cache = CACHE.get_or_init(Default::default);
    if let Some((_, tiles)) = cache
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .find(|(key, _)| *key == piece)
    {
        return tiles.get(&offset).cloned().unwrap_or_default();
    }
    let mut tiles = BTreeMap::<[i32; 3], Vec<Profile>>::new();
    for &beam in piece.local_geometry().iter() {
        let bounds = beam.bounds();
        let low = bounds.minimum.map(|v| (v + 1e-8).floor() as i32);
        let high = bounds.maximum.map(|v| (v - 1e-8).floor() as i32);
        for x in low[0]..=high[0] {
            for y in low[1]..=high[1] {
                for z in low[2]..=high[2] {
                    let cell = [x, y, z];
                    tiles
                        .entry(cell)
                        .or_default()
                        .push(profile(beam.translated(cell.map(|v| -f64::from(v)))));
                }
            }
        }
    }
    let tiles: Arc<Tiles> = Arc::new(
        tiles
            .into_iter()
            .map(|(cell, p)| (cell, Arc::from(p)))
            .collect(),
    );
    let result = tiles.get(&offset).cloned().unwrap_or_default();
    let mut cache = cache.lock().unwrap_or_else(|e| e.into_inner());
    if cache.len() >= 64 {
        cache.pop_front();
    }
    cache.push_back((piece, tiles));
    result
}
fn profile(beam: Beam) -> Profile {
    let corners = beam.corners();
    let planes = [
        [0, 2, 6],
        [1, 5, 7],
        [0, 4, 5],
        [2, 3, 7],
        [0, 1, 3],
        [4, 6, 7],
    ]
    .map(|[a, b, c]| {
        let u: [f64; 3] = std::array::from_fn(|i| corners[b][i] - corners[a][i]);
        let v: [f64; 3] = std::array::from_fn(|i| corners[c][i] - corners[a][i]);
        let mut n = [
            u[1] * v[2] - u[2] * v[1],
            u[2] * v[0] - u[0] * v[2],
            u[0] * v[1] - u[1] * v[0],
        ];
        let norm = n.iter().map(|v| v * v).sum::<f64>().sqrt();
        n = n.map(|v| v / norm);
        let mut distance = (0..3)
            .map(|i| n[i] * (corners[a][i] - beam.center[i]))
            .sum::<f64>();
        if distance < 0.0 {
            n = n.map(|v| -v);
            distance = -distance;
        }
        (n, distance)
    });
    Profile {
        bounds: beam.bounds(),
        center: beam.center,
        planes: planes.to_vec(),
    }
}
pub(super) fn fragments(
    mut visible: Vec<Vec<[f32; 3]>>,
    profiles: &[Profile],
    cell: [f32; 3],
) -> Vec<Vec<[f32; 3]>> {
    for profile in profiles {
        // Avoid subdividing an untouched face above/below every sleeper and rail head.
        if let Some(face) = visible.first()
            && let Some(point) = face.first()
            && let Some(axis) =
                (0..3).find(|&axis| face.iter().all(|p| (p[axis] - point[axis]).abs() < 1e-7))
        {
            let plane = f64::from(point[axis] - cell[axis]);
            if plane < profile.bounds.minimum[axis] - 1e-8
                || plane > profile.bounds.maximum[axis] + 1e-8
            {
                continue;
            }
        }
        let mut remaining = Vec::new();
        for polygon in visible {
            let mut inside = polygon;
            for &(normal, distance) in &profile.planes {
                if inside.len() < 3 {
                    break;
                }
                let signed = |p: [f32; 3]| {
                    (0..3)
                        .map(|i| normal[i] * (f64::from(p[i] - cell[i]) - profile.center[i]))
                        .sum::<f64>()
                        - distance
                };
                let outside = clip(&inside, &signed, false);
                if area(&outside) > 1e-10 {
                    remaining.push(outside);
                }
                inside = clip(&inside, &signed, true);
            }
        }
        visible = remaining;
        if visible.is_empty() {
            break;
        }
    }
    visible
}
fn clip(polygon: &[[f32; 3]], signed: &impl Fn([f32; 3]) -> f64, inside: bool) -> Vec<[f32; 3]> {
    let mut result = Vec::new();
    let Some(&last) = polygon.last() else {
        return result;
    };
    let mut previous = last;
    let mut old = signed(last);
    for &point in polygon {
        let next = signed(point);
        let old_in = if inside { old <= 1e-8 } else { old >= -1e-8 };
        let next_in = if inside { next <= 1e-8 } else { next >= -1e-8 };
        if old_in != next_in {
            let t = (old / (old - next)) as f32;
            result.push(std::array::from_fn(|i| {
                previous[i] + t * (point[i] - previous[i])
            }));
        }
        if next_in {
            result.push(point);
        }
        previous = point;
        old = next;
    }
    result
}
fn area(polygon: &[[f32; 3]]) -> f64 {
    let Some(&first) = polygon.first() else {
        return 0.;
    };
    polygon
        .windows(2)
        .map(|p| {
            let u: [f64; 3] = std::array::from_fn(|i| f64::from(p[0][i] - first[i]));
            let v: [f64; 3] = std::array::from_fn(|i| f64::from(p[1][i] - first[i]));
            let cross = [
                u[1] * v[2] - u[2] * v[1],
                u[2] * v[0] - u[0] * v[2],
                u[0] * v[1] - u[1] * v[0],
            ];
            cross.iter().map(|v| v * v).sum::<f64>().sqrt() * 0.5
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_core::rails::PartMaterial;
    #[test]
    fn diagonal_sleepers_leave_their_real_water_corners_instead_of_a_square_cutout() {
        let angle = std::f64::consts::FRAC_PI_4;
        let (s, c) = angle.sin_cos();
        let beam = Beam {
            center: [0.5, 0.1, 0.5],
            right: [c, 0., -s],
            up: [0., 1., 0.],
            forward: [s, 0., c],
            half_extents: [0.1, 0.1, 0.6],
            material: PartMaterial::Wood,
            cross_sections: None,
        };
        let face = vec![[0., 0.1, 0.], [0., 0.1, 1.], [1., 0.1, 1.], [1., 0.1, 0.]];
        let visible = fragments(vec![face], &[profile(beam)], [0.; 3]);
        let surface = visible.iter().map(|p| area(p)).sum::<f64>();
        assert!(
            (0.70..0.82).contains(&surface),
            "visible water area={surface}"
        );
        let aabb = beam.bounds();
        assert!((aabb.maximum[0] - aabb.minimum[0]) * (aabb.maximum[2] - aabb.minimum[2]) > 0.95);
    }
    #[test]
    fn full_water_above_both_drawn_bends_and_shallow_switches_has_no_missing_tiles() {
        use terra_core::rails::{CurveSpec, SWITCH_LEFT, SWITCH_RIGHT};
        for anchor in [[0; 3], [-31, 16, -33]] {
            for piece in [
                Piece::new_curve(
                    anchor,
                    CurveSpec::new([[8, 2, 0], [8, 2, 128], [136, 2, 256], [264, 2, 256]]).unwrap(),
                )
                .unwrap(),
                Piece::for_build(anchor, SWITCH_LEFT, 0).unwrap(),
                Piece::for_build(anchor, SWITCH_RIGHT, 1).unwrap(),
            ] {
                for offset in piece.footprint().iter().filter(|p| p[1] == 0) {
                    let cell = std::array::from_fn(|i| anchor[i] + i64::from(offset[i]));
                    let masks = profiles(piece, cell);
                    let face = vec![[0., 0.7, 0.], [0., 0.7, 1.], [1., 0.7, 1.], [1., 0.7, 0.]];
                    let visible = fragments(vec![face], &masks, [0.; 3]);
                    let surface = visible.iter().map(|p| area(p)).sum::<f64>();
                    assert!(
                        (surface - 1.0).abs() < 1e-6,
                        "{piece:?} offset={offset:?} area={surface}"
                    );
                }
            }
        }
    }
}
