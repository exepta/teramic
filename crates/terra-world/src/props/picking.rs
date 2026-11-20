use super::{
    MAX_WEATHER_WIND, PropKind, PropPose, WIND_AMPLITUDE, WIND_FLUTTER, WorldProp,
    visual::{self, PropDetail, PropPalette},
    wind_displacement, wind_parameters,
};
use std::sync::OnceLock;
mod outline;
#[cfg(test)]
mod performance_tests;

struct PickMesh {
    positions: Vec<[f32; 3]>,
    indices: Vec<u32>,
    outline: Vec<[u32; 2]>,
}
static MESHES: [[OnceLock<Result<PickMesh, String>>; 256]; PropKind::ALL.len()] =
    [const { [const { OnceLock::new() }; 256] }; PropKind::ALL.len()];

fn mesh(prop: &WorldProp) -> Option<&'static PickMesh> {
    MESHES[prop.kind as usize][usize::from(prop.template_variant())]
        .get_or_init(|| {
            let mut mesh = PickMesh {
                positions: Vec::new(),
                indices: Vec::new(),
                outline: Vec::new(),
            };
            for part in visual::template(
                prop.kind,
                prop.template_variant(),
                PropDetail::Near,
                &PropPalette::default(),
            )? {
                let base = mesh.positions.len() as u32;
                mesh.positions.extend(part.positions);
                mesh.indices
                    .extend(part.indices.into_iter().map(|i| i + base));
            }
            mesh.outline = outline::from_triangles(&mesh.positions, &mesh.indices);
            Ok(mesh)
        })
        .as_ref()
        .ok()
}

fn point(prop: &WorldProp, p: [f32; 3], pose: PropPose) -> [f64; 3] {
    point_scaled(prop, p, pose, 1.0)
}

fn point_scaled(prop: &WorldProp, p: [f32; 3], pose: PropPose, factor: f32) -> [f64; 3] {
    let (sin, cos) = f64::from(prop.rotation).sin_cos();
    let scale = f64::from(prop.visual_scale() * factor);
    let [x, y, z] = p.map(f64::from);
    let mut world = [
        prop.position[0] + (cos * x + sin * z) * scale,
        prop.position[1] + y * scale,
        prop.position[2] + (-sin * x + cos * z) * scale,
    ];
    let wind = match pose {
        PropPose::Wind(time) => Some((time, [0.86, 0.51, 1.0, 1.0])),
        PropPose::WeatherWind { time, frame } => Some((time, frame)),
        PropPose::Seasonal {
            time: Some(time),
            frame,
            swept: false,
            ..
        } => Some((time, frame)),
        _ => None,
    };
    if let Some((time, frame)) = wind {
        let weights = wind_parameters(prop.kind, prop.variant, p[1], prop.visual_scale() * factor);
        let displacement = wind_displacement(world, time, weights, frame);
        world[0] += displacement[0];
        world[2] += displacement[1];
    }
    world
}

pub(super) fn edges(prop: &WorldProp, pose: PropPose, mut visit: impl FnMut([[f64; 3]; 2])) {
    let Some(mesh) = mesh(prop) else { return };
    let world = |p| point(prop, p, pose);
    for &[a, b] in &mesh.outline {
        visit([
            world(mesh.positions[a as usize]),
            world(mesh.positions[b as usize]),
        ]);
    }
}

pub(super) fn hit(
    prop: &WorldProp,
    origin: [f64; 3],
    direction: [f64; 3],
    reach: f64,
    pose: PropPose,
) -> Option<f64> {
    hit_scaled(prop, origin, direction, reach, pose, 1.0)
}

pub(super) fn hit_scaled(
    prop: &WorldProp,
    origin: [f64; 3],
    direction: [f64; 3],
    reach: f64,
    pose: PropPose,
    factor: f32,
) -> Option<f64> {
    use bevy_math::DVec3;
    let mesh = mesh(prop)?;
    let origin = DVec3::from_array(origin) - DVec3::from_array(prop.position);
    let direction = DVec3::from_array(direction);
    let positions: Vec<_> = mesh
        .positions
        .iter()
        .map(|&p| {
            DVec3::from_array(point_scaled(prop, p, pose, factor))
                - DVec3::from_array(prop.position)
        })
        .collect();
    let offsets: Vec<_> = if matches!(
        pose,
        PropPose::Swept | PropPose::WeatherSwept { .. } | PropPose::Seasonal { swept: true, .. }
    ) {
        mesh.positions
            .iter()
            .map(|p| {
                let [h, f] =
                    wind_parameters(prop.kind, prop.variant, p[1], prop.visual_scale() * factor);
                if f < 0.0 {
                    return DVec3::new(f64::from(0.86_f32), 0.0, f64::from(0.51_f32))
                        * f64::from(h * f.abs() * (WIND_AMPLITUDE + WIND_FLUTTER));
                }
                match pose {
                    PropPose::WeatherSwept { frame }
                    | PropPose::Seasonal {
                        frame, swept: true, ..
                    } => {
                        DVec3::new(f64::from(frame[0]), 0.0, f64::from(frame[1]))
                            * f64::from(
                                h * f
                                    * (WIND_AMPLITUDE + WIND_FLUTTER)
                                    * frame[2].clamp(0.0, MAX_WEATHER_WIND),
                            )
                    }
                    _ => {
                        DVec3::new(f64::from(0.86_f32), 0.0, f64::from(0.51_f32))
                            * f64::from(h * f * (WIND_AMPLITUDE + WIND_FLUTTER))
                    }
                }
            })
            .collect()
    } else {
        Vec::new()
    };
    let mut closest = None;
    let mut intersect = |[a, b, c]: [DVec3; 3]| {
        let e1 = b - a;
        let e2 = c - a;
        let cross = direction.cross(e2);
        let determinant = e1.dot(cross);
        if determinant.abs() < 1e-12 {
            return;
        }
        let t = origin - a;
        let u = t.dot(cross) / determinant;
        let q = t.cross(e1);
        let v = direction.dot(q) / determinant;
        let distance = e2.dot(q) / determinant;
        if u >= -1e-9
            && v >= -1e-9
            && u + v <= 1.0 + 1e-9
            && distance >= 0.0
            && distance <= closest.unwrap_or(reach)
        {
            closest = Some(distance);
        }
    };
    for triangle in mesh.indices.chunks_exact(3) {
        let vertices = [triangle[0], triangle[1], triangle[2]].map(|i| positions[i as usize]);
        if matches!(
            pose,
            PropPose::Swept
                | PropPose::WeatherSwept { .. }
                | PropPose::Seasonal { swept: true, .. }
        ) {
            // Only the actual current wind can occlude another target. Reserving
            // the maximum storm envelope in calm weather creates phantom hits.
            let high: [DVec3; 3] =
                std::array::from_fn(|i| vertices[i] + offsets[triangle[i] as usize]);
            let low: [DVec3; 3] =
                std::array::from_fn(|i| vertices[i] - offsets[triangle[i] as usize]);
            intersect(high);
            intersect(low);
            for i in 0..3 {
                let j = (i + 1) % 3;
                intersect([high[i], high[j], low[j]]);
                intersect([high[i], low[j], low[i]]);
            }
        } else {
            intersect(vertices);
        }
    }
    closest
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::props::{PropId, raycast_props};
    use terra_voxel::SectionPos;
    #[test]
    fn mushroom_air_beside_stem_misses_but_cap_and_stem_hit_after_rotation_and_scale() {
        let prop = WorldProp {
            id: PropId(1),
            kind: PropKind::Mushroom,
            position: [-1.5, 0.0, -0.5],
            rotation: 0.71,
            scale: 1.7,
            variant: 0,
            section: SectionPos::new(-1, -1, -1),
            removed: false,
        };
        let ray = |x, y| raycast_props([x, y, 2.0], [0.0, 0.0, -1.0], 8.0, [&prop]);
        let height = f64::from(prop.visual_scale());
        assert!(ray(-1.5, height * 0.13 / 0.3825).is_some());
        assert!(ray(-1.5, height * 0.28 / 0.3825).is_some());
        assert!(
            ray(-1.42, height * 0.10).is_none(),
            "air between the two thick stems must not become a solid picking box"
        );
    }

    #[test]
    fn animated_surface_hits_fit_the_triangle_wind_sweep_without_clock_synchronization() {
        use bevy_math::DVec3;
        for kind in [
            PropKind::Grass,
            PropKind::Flower,
            PropKind::Bush,
            PropKind::Fern,
        ] {
            let prop = WorldProp {
                id: PropId(7),
                kind,
                position: [-17.5, 16.0, -0.5],
                rotation: 1.2,
                scale: 2.0,
                variant: 2,
                section: SectionPos::new(-2, 0, -1),
                removed: false,
            };
            let mesh = mesh(&prop).unwrap();
            for time in [0.0, 0.7, 3.0, 19.0, 3599.5] {
                for pose in [
                    PropPose::Wind(time),
                    PropPose::WeatherWind {
                        time,
                        frame: super::super::weather_wind_frame([-7.0, 3.4]),
                    },
                ] {
                    for triangle in mesh.indices.chunks_exact(3).step_by(17) {
                        let [a, b, c] = [triangle[0], triangle[1], triangle[2]].map(|i| {
                            DVec3::from_array(point(&prop, mesh.positions[i as usize], pose))
                        });
                        let normal = (b - a).cross(c - a).normalize_or_zero();
                        if normal == DVec3::ZERO {
                            continue;
                        }
                        let origin = ((a + b + c) / 3.0 + normal * 0.5).to_array();
                        let direction = (-normal).to_array();
                        assert!(hit(&prop, origin, direction, 1.0, pose).is_some());
                        if let PropPose::WeatherWind { frame, .. } = pose {
                            assert!(
                                hit(
                                    &prop,
                                    origin,
                                    direction,
                                    1.0,
                                    PropPose::WeatherSwept { frame }
                                )
                                .is_some(),
                                "current-weather sweep missed {kind:?} at {time}"
                            );
                        }
                        if matches!(pose, PropPose::Wind(_)) {
                            assert!(
                                hit(&prop, origin, direction, 1.0, PropPose::Swept).is_some(),
                                "{kind:?} at {time}"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn calm_authority_does_not_occlude_with_hypothetical_storm_geometry() {
        use bevy_math::DVec3;
        let prop = WorldProp {
            id: PropId(74),
            kind: PropKind::Flower,
            position: [-17.5, 0.0, -0.5],
            rotation: 0.0,
            scale: 1.0,
            variant: 0,
            section: SectionPos::new(-2, 0, -1),
            removed: false,
        };
        let geometry = mesh(&prop).unwrap();
        let storm = super::super::weather_wind_frame([-7.0, 3.4]);
        let calm = super::super::weather_wind_frame([0.35, 0.15]);
        let mut outside_calm = false;
        for triangle in geometry.indices.chunks_exact(3) {
            let posed = PropPose::WeatherWind {
                time: 19.0,
                frame: storm,
            };
            let [a, b, c] = [triangle[0], triangle[1], triangle[2]]
                .map(|i| DVec3::from_array(point(&prop, geometry.positions[i as usize], posed)));
            let normal = (b - a).cross(c - a).normalize_or_zero();
            if normal == DVec3::ZERO {
                continue;
            }
            let origin = ((a + b + c) / 3.0 + normal * 0.5).to_array();
            let direction = (-normal).to_array();
            if hit(&prop, origin, direction, 0.51, posed).is_some()
                && hit(
                    &prop,
                    origin,
                    direction,
                    0.51,
                    PropPose::WeatherSwept { frame: storm },
                )
                .is_some()
                && hit(
                    &prop,
                    origin,
                    direction,
                    0.51,
                    PropPose::WeatherSwept { frame: calm },
                )
                .is_none()
            {
                outside_calm = true;
                break;
            }
        }
        assert!(
            outside_calm,
            "a calm-world picker must exclude displaced storm-only surfaces"
        );
    }
}
