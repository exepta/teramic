//! Measure connected air from the viewer through the cave and out of a real
//! aperture. The renderer also needs the first obstruction along the sun ray.
use bevy::prelude::*;
use std::{cell::RefCell, collections::HashMap};
use terra_voxel::blocks::raycast_blocks;
use terra_world::{edits::EditJournal, streaming::ChunkStore};

pub(super) struct Opening {
    pub window: Vec4,
    pub transmission: f32,
    pub depths: Vec4,
    pub radii: Vec4,
    pub cross_radii: Vec4,
}

const OFFSETS: [(f32, f32); 4] = [(0., 0.), (0.60, 0.), (-0.30, 0.52), (-0.30, -0.52)];

/// Unknown cells, fluids and roofs all end the participating column of air.
fn clear_length(
    from: Vec3,
    direction: Vec3,
    reach: f32,
    air: &mut impl FnMut([i64; 3]) -> bool,
) -> f32 {
    raycast_blocks(
        from.as_dvec3().to_array(),
        direction.as_dvec3().to_array(),
        reach.into(),
        |cell| Some(u16::from(!air(cell))),
    )
    .map_or(reach, |hit| hit.distance as f32)
}

fn connected(eye: Vec3, lip: Vec3, sky: f32, air: &mut impl FnMut([i64; 3]) -> bool) -> bool {
    // A bank or bridge can hide the floor under the window while its upper
    // shaft is still visible. Both candidate paths begin at the actual eye.
    [eye.y, eye.y + (lip.y - eye.y) * 0.48]
        .into_iter()
        .any(|y| {
            let inside = lip.with_y(y);
            let delta = inside - eye;
            let distance = delta.length();
            let rise = sky - inside.y;
            distance <= 126.
                && (0.0..=126.).contains(&rise)
                && clear_length(eye, delta.normalize_or(Vec3::Y), distance, air) >= distance
                && clear_length(inside, Vec3::Y, rise, air) >= rise
        })
}

/// Keep each projected axis as wide as the actual sunlit mouth. A short
/// obstruction in one direction clips that axis independently of the other.
fn beam_footprint(
    lip: Vec3,
    depth: f32,
    reach: f32,
    mut incident: impl FnMut(Vec3) -> f32,
) -> (Vec2, f32) {
    let mut widths = Vec2::splat(0.40);
    let mut end = depth;
    for (i, axis) in [Vec3::X, Vec3::Z].into_iter().enumerate() {
        for scale in [0.30, 0.62, 1.0] {
            let span = reach * scale;
            let limit = incident(lip + axis * span).min(incident(lip - axis * span));
            if limit < (depth * 0.30).max(0.65) {
                break;
            }
            widths[i] = span.max(0.40);
            end = end.min(limit);
        }
    }
    (widths, end)
}

pub(super) fn measure(
    chunks: &ChunkStore,
    journal: &EditJournal,
    eye: Vec3,
    previous: Vec4,
    sun: Vec3,
) -> Option<Opening> {
    let cell = eye.floor().as_i64vec3();
    let column = chunks.surface_column_at(cell.x, cell.z)?;
    if column.height < f64::from(eye.y + 2.) {
        return None;
    }
    let bounds = RefCell::new(HashMap::new());
    let top = |x: i64, z: i64| {
        let key = [x.div_euclid(16), z.div_euclid(16)];
        *bounds.borrow_mut().entry(key).or_insert_with(|| {
            let natural = chunks.prepared_column(key)?.maximum_height();
            super::lights::known_top(chunks, journal, key, natural).map(|v| v as f32 + 1.5)
        })
    };
    let mut air = |p: [i64; 3]| match chunks.block_at(p) {
        Some(raw) => {
            journal.block(p).unwrap_or(raw) == 0
                && journal.fluid(p).is_none_or(|liquid| liquid.amount == 0)
        }
        None => top(p[0], p[2]).is_some_and(|sky| p[1] as f32 > sky - 1.5),
    };
    let mut sample = |x: i64, z: i64, radius: Option<f32>| {
        let roof = chunks.surface_column_at(x, z)?.height.ceil() as f32 + 0.5;
        if roof < eye.y + 2. {
            return None;
        }
        let mut lip = Vec3::new(x as f32 + 0.5, roof, z as f32 + 0.5);
        let radius = if let Some(hint) = radius {
            hint
        } else {
            let sky = top(x, z)?;
            if !connected(eye, lip, sky, &mut air) {
                return None;
            }
            // Hand-dug windows and natural entrances can be arbitrarily wide.
            // Walk in the roof plane, stopping at the first unbroken column.
            let mut rim = [0.; 4];
            for (distance, axis) in rim
                .iter_mut()
                .zip([Vec3::X, Vec3::NEG_X, Vec3::Z, Vec3::NEG_Z])
            {
                for step in 1..=14 {
                    let point = lip + axis * step as f32;
                    let local_sky = top(point.x.floor() as i64, point.z.floor() as i64);
                    let start = point - Vec3::Y * 2.;
                    if !local_sky.is_some_and(|sky| {
                        let rise = (sky - start.y).max(4.);
                        clear_length(start, Vec3::Y, rise, &mut air) >= rise
                    }) {
                        break;
                    }
                    *distance = step as f32;
                }
            }
            lip.x += ((rim[0] - rim[1]) * 0.5).round();
            lip.z += ((rim[2] - rim[3]) * 0.5).round();
            (0.5 + 0.5 * (rim[0] + rim[1]).max(rim[2] + rim[3])).max(0.55)
        };
        let window = lip.extend(radius);
        let mut depths = Vec4::ZERO;
        let mut radii = Vec4::ZERO;
        let mut cross_radii = Vec4::ZERO;
        let mut open = 0;
        for (i, (dx, dz)) in OFFSETS.into_iter().enumerate() {
            // Center each probe in a voxel; the GPU uses the same coordinates.
            let lip = (window.truncate() + Vec3::new(dx * radius, 0., dz * radius)).floor()
                + Vec3::splat(0.5);
            let Some(sky) = top(lip.x.floor() as i64, lip.z.floor() as i64) else {
                continue;
            };
            if !connected(eye, lip, sky, &mut air) {
                continue;
            }
            open += 1;
            if sun.y <= 0.06 {
                continue;
            }
            // A sloping ray can hit the lip or a bridge even under a vertical opening.
            let incident = |p: Vec3, air: &mut _| {
                let reach = ((sky - p.y).max(2.) / sun.y).min(100.);
                if clear_length(p, sun, reach, air) < reach {
                    return 0.;
                }
                clear_length(p, -sun, 100., air) * sun.y
            };
            let depth = incident(lip, &mut air);
            if depth <= 0. {
                continue;
            }
            let reach = radius * if i == 0 { 0.90 } else { 0.67 };
            let (span, end) = beam_footprint(lip, depth, reach, |p| incident(p, &mut air));
            depths[i] = end;
            radii[i] = span.x;
            cross_radii[i] = span.y;
        }
        (open > 0).then_some(Opening {
            window,
            transmission: open as f32 / 4.,
            depths,
            radii,
            cross_radii,
        })
    };
    if let Some(([x, z], coupling, radius)) = column.cave.skylight(f64::from(eye.y))
        && let Some(mut opening) = sample(x.floor() as i64, z.floor() as i64, Some(radius))
    {
        opening.transmission *= coupling;
        return Some(opening);
    }
    // Anchor unplanned windows to a stable grid, reusing the last valid mouth.
    let grid = |n: i64| (n + 4).div_euclid(8) * 8;
    let previous =
        (previous.w > 0.).then_some((previous.x.floor() as i64, previous.z.floor() as i64));
    previous
        .into_iter()
        .filter(|&(x, z)| (x - cell.x).abs() <= 24 && (z - cell.z).abs() <= 24)
        .chain(
            [
                (0, 0),
                (-1, 0),
                (1, 0),
                (0, -1),
                (0, 1),
                (-1, -1),
                (1, -1),
                (-1, 1),
                (1, 1),
                (-2, 0),
                (2, 0),
                (0, -2),
                (0, 2),
            ]
            .map(|(dx, dz)| (grid(cell.x) + dx * 8, grid(cell.z) + dz * 8)),
        )
        .chain(std::iter::once((cell.x, cell.z)))
        .find_map(|(x, z)| sample(x, z, None))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_fracture_lights_its_full_open_axis_even_if_the_cross_axis_is_blocked() {
        let (span, end) = beam_footprint(Vec3::ZERO, 24., 8., |p| {
            if p.x.abs() < 9. && p.z.abs() < 3. {
                20.
            } else {
                0.
            }
        });
        assert_eq!(span.x, 8.);
        assert!((2.0..3.0).contains(&span.y));
        assert_eq!(end, 20.);
    }

    #[test]
    fn a_rock_rib_between_two_openings_stops_the_first_beam_at_that_rib() {
        let (span, _) = beam_footprint(Vec3::ZERO, 12., 8., |p| {
            if (4.0..6.0).contains(&p.x.abs()) {
                0.
            } else {
                12.
            }
        });
        assert!(span.x < 4.);
        assert_eq!(span.y, 8.);
    }

    #[test]
    fn window_requires_a_continuous_visible_path_through_the_roof() {
        let eye = Vec3::new(0.5, 2.5, 0.5);
        let lip = Vec3::new(7.5, 11.5, 0.5);
        let roof = |[x, y, _]: [i64; 3]| !(5..=10).contains(&y) || x == 7;
        assert!(connected(eye, lip, 17., &mut |p| roof(p)));
        assert!(!connected(eye, lip, 17., &mut |p| roof(p) && p[1] != 7));
        assert!(!connected(eye, lip, 17., &mut |p| roof(p) && p[0] != 3));
        assert!(!connected(eye, lip, 17., &mut |p| roof(p) && p[1] < 14));
        let sun = Vec3::new(1., 1., 0.).normalize();
        assert!(clear_length(lip, -sun, 50., &mut |p| roof(p)) < 2.);
        assert_eq!(clear_length(lip, Vec3::NEG_Y, 20., &mut |p| roof(p)), 20.);
    }
}
