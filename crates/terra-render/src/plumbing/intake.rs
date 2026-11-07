//! Actual server-admitted intake pulses create a bounded set of rising bubbles.
use super::Geometry;
use bevy::prelude::*;
use std::collections::HashMap;
use terra_core::plumbing::IntakeActivity;
use terra_network::client::ClientSession;
use terra_world::{edits::EditJournal, streaming::ChunkStore};

#[derive(Resource, Default, Debug)]
pub struct IntakeRenderStatus {
    pub active_inlets: usize,
    pub bubbles: usize,
}

pub(super) struct Pulse {
    activity: IntakeActivity,
    last_seen: f32,
}

fn triangle(geometry: &mut Geometry, mut p: [Vec3; 3], normal: Vec3, color: [f32; 4]) {
    if (p[1] - p[0]).cross(p[2] - p[0]).dot(normal) < 0.0 {
        p.swap(1, 2);
    }
    let base = geometry.positions.len() as u32;
    geometry.positions.extend(p.map(|p| p.to_array()));
    geometry.normals.extend([normal.to_array(); 3]);
    geometry.colors.extend([color; 3]);
    geometry.indices.extend([base, base + 1, base + 2]);
}

fn bubble(geometry: &mut Geometry, center: Vec3, radius: f32, alpha: f32) {
    let color = [0.70, 0.87, 0.93, alpha];
    let ring = |latitude: f32, angle: f32| {
        center
            + Vec3::new(
                latitude.cos() * angle.cos(),
                latitude.sin(),
                latitude.cos() * angle.sin(),
            ) * radius
    };
    let levels = [
        -std::f32::consts::FRAC_PI_3,
        0.0,
        std::f32::consts::FRAC_PI_3,
    ];
    for side in 0..8 {
        let a = side as f32 * std::f32::consts::TAU / 8.;
        let b = (side + 1) as f32 * std::f32::consts::TAU / 8.;
        for pair in levels.windows(2) {
            let mut points = [
                ring(pair[0], a),
                ring(pair[1], a),
                ring(pair[1], b),
                ring(pair[0], b),
            ];
            let normal = (points.iter().copied().sum::<Vec3>() / 4. - center).normalize();
            if (points[1] - points[0])
                .cross(points[2] - points[0])
                .dot(normal)
                < 0.
            {
                points.reverse();
            }
            geometry.quad(points, normal, color);
        }
        let bottom = [
            center - Vec3::Y * radius,
            ring(levels[0], a),
            ring(levels[0], b),
        ];
        let top = [
            center + Vec3::Y * radius,
            ring(levels[2], a),
            ring(levels[2], b),
        ];
        triangle(
            geometry,
            bottom,
            (bottom.iter().copied().sum::<Vec3>() / 3. - center).normalize(),
            color,
        );
        triangle(
            geometry,
            top,
            (top.iter().copied().sum::<Vec3>() / 3. - center).normalize(),
            color,
        );
    }
}

const BUBBLE_MIN_RADIUS: f32 = 0.012;
const BUBBLE_MAX_RADIUS: f32 = 0.026;
const BUBBLE_SURFACE_GAP: f32 = 0.012;
const BUBBLE_BED_GAP: f32 = 0.004;
const BUBBLE_WOBBLE: f32 = 0.02;
// The authored flange reaches radius .215. Even the largest bubble plus its
// inward wobble stays outside that ring for the complete animation cycle.
const DOWNWARD_TRACK_RADIUS: f32 = 0.29;

fn rising_bubbles(
    geometry: &mut Geometry,
    start: Vec3,
    ceiling: f32,
    now: f32,
    phase: f32,
    alpha: f32,
) {
    let travel = (ceiling - start.y - BUBBLE_MAX_RADIUS).clamp(0.0, 1.2);
    for i in 0..4 {
        let progress = (now * 1.6 + phase + i as f32 / 4.0).fract();
        let lateral = now * 3.0 + i as f32 * 2.1;
        let radius = BUBBLE_MIN_RADIUS + progress * (BUBBLE_MAX_RADIUS - BUBBLE_MIN_RADIUS);
        let center = start
            + Vec3::new(
                lateral.sin() * BUBBLE_WOBBLE,
                progress * travel,
                lateral.cos() * BUBBLE_WOBBLE,
            );
        bubble(geometry, center, radius, alpha * (0.58 - progress * 0.25));
    }
}

pub(super) fn append(
    geometry: &mut Geometry,
    pulses: &mut HashMap<[i64; 3], Pulse>,
    now: f32,
    session: Option<&ClientSession>,
    eye: Vec3,
    (chunks, journal, terrain): (&ChunkStore, &EditJournal, &terra_world::TerraGenerator),
) -> IntakeRenderStatus {
    let Some(session) = session.filter(|s| s.ready) else {
        pulses.clear();
        return Default::default();
    };
    let mut active: Vec<_> = session
        .plumbing
        .iter()
        .filter_map(|node| {
            let activity = node.intake?;
            let distance = Vec3::from_array(node.position.map(|n| n as f32)).distance_squared(eye);
            (activity.units > 0 && distance < 64.0_f32.powi(2)).then_some((
                distance,
                node.position,
                activity,
            ))
        })
        .collect();
    active.sort_by(|a, b| a.0.total_cmp(&b.0));
    for (_, position, activity) in active.into_iter().take(16) {
        pulses.insert(
            position,
            Pulse {
                activity,
                last_seen: now,
            },
        );
    }
    pulses.retain(|position, pulse| {
        now - pulse.last_seen < 0.45
            && chunks
                .block_at(*position)
                .is_some_and(terra_core::plumbing::is_pipe)
    });
    let mut status = IntakeRenderStatus::default();
    for (&position, pulse) in pulses.iter().take(16) {
        let direction = Vec3::from_array(pulse.activity.direction.map(f32::from));
        let mouth =
            Vec3::from_array(position.map(|n| n as f32)) + Vec3::splat(0.5) + direction * 0.5;
        // Bubbles from a downward mouth rise around its flange, rather than
        // disappearing against the pipe underside on the way to the surface.
        let offsets: &[Vec3] = if direction.y < -0.5 {
            &[
                Vec3::new(DOWNWARD_TRACK_RADIUS, 0., 0.),
                Vec3::new(-DOWNWARD_TRACK_RADIUS, 0., 0.),
                Vec3::new(0., 0., DOWNWARD_TRACK_RADIUS),
                Vec3::new(0., 0., -DOWNWARD_TRACK_RADIUS),
            ]
        } else {
            &[Vec3::ZERO]
        };
        let Some((offset, column)) = offsets
            .iter()
            .filter_map(|&offset| {
                let sample = (mouth + direction * 0.035 + offset)
                    .to_array()
                    .map(f64::from);
                chunks
                    .water_column_at_point(terrain, journal, sample)
                    .map(|column| (offset, column))
            })
            .max_by(|a, b| a.1.level.total_cmp(&b.1.level))
        else {
            continue;
        };
        let ceiling = column.level as f32 - BUBBLE_SURFACE_GAP;
        let minimum = column.bed as f32 + BUBBLE_MAX_RADIUS + BUBBLE_BED_GAP;
        let maximum = ceiling - BUBBLE_MAX_RADIUS;
        if minimum >= maximum {
            continue;
        }
        let mut start = mouth + direction * 0.055 + offset;
        start.y = start.y.clamp(minimum, maximum);
        status.active_inlets += 1;
        let fade = (1.0 - (now - pulse.last_seen) / 0.45).clamp(0., 1.);
        let phase =
            ((position[0].wrapping_mul(17) ^ position[2].wrapping_mul(31)) & 255) as f32 / 256.;
        rising_bubbles(geometry, start, ceiling, now, phase, fade);
        status.bubbles += 4;
        // The actual water shader owns its surface. A second flat translucent
        // ring at level+.004 would cross waves and the intake flange, producing
        // precisely the flickering sheet underneath a running intake.
    }
    status
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn running_downward_intake_has_only_submerged_round_geometry_outside_the_real_flange() {
        use terra_core::plumbing::{IRON_PIPE, IntakeActivity};
        use terra_network::protocol::session::PlumbingVisual;
        use terra_voxel::{BlockSection, global_to_section};
        use terra_world::{edits::BlockObjectState, streaming::LoadedSection};
        let terrain = terra_world::TerraGenerator::new(terra_core::WorldConfig {
            base_height: 4.0,
            sea_level: 8.0,
            hill_amplitude: 0.0,
            mountain_amplitude: 0.0,
            biomes: false,
            caves: false,
            rivers: false,
            trees: false,
            ..Default::default()
        })
        .unwrap();
        // These authored bounds include the .215 flange, whereas the plain tube
        // radius is only .15. The effect must clear the visible flange as well.
        let flange = terra_core::block_shapes::builtin_boxes(IRON_PIPE)
            .iter()
            .find(|bounds| bounds.minimum[1] == 0.0 && bounds.maximum[1] == 1.0)
            .unwrap();
        let flange_radius = (flange.maximum[0] - 0.5) as f32;
        for inlet in [[8, 8, 8], [-8, 8, -8]] {
            let mut chunks = ChunkStore::default();
            let (owner, local) = global_to_section(inlet);
            let mut data = BlockSection::empty(owner).unwrap();
            for x in inlet[0] - 1..=inlet[0] + 1 {
                for z in inlet[2] - 1..=inlet[2] + 1 {
                    data.set_block(global_to_section([x, 6, z]).1, 3).unwrap();
                    data.set_block(global_to_section([x, 7, z]).1, 27).unwrap();
                }
            }
            data.set_block(local, IRON_PIPE).unwrap();
            chunks.loaded.insert(
                owner,
                LoadedSection {
                    data,
                    mesh: None,
                    mesh_epoch: 0,
                    water_epoch: 0,
                    water: None,
                    props: Vec::new(),
                },
            );
            let mut journal = EditJournal::default();
            journal.record(inlet, IRON_PIPE).unwrap();
            journal
                .set_object_state(
                    inlet,
                    Some(BlockObjectState {
                        facing: 4,
                        ..Default::default()
                    }),
                )
                .unwrap();
            let mut session = ClientSession::default();
            session.ready = true;
            session.plumbing.push(PlumbingVisual {
                position: inlet,
                amount: 8,
                capacity: 8,
                open: true,
                flowing: true,
                supplied: false,
                pump_rpm: 0.0,
                basin: None,
                intake: Some(IntakeActivity {
                    source: [inlet[0], 7, inlet[2]],
                    direction: [0, -1, 0],
                    units: 1,
                }),
            });
            let mouth = Vec3::new(inlet[0] as f32 + 0.5, 8.0, inlet[2] as f32 + 0.5);
            let mut pulses = HashMap::new();
            for frame in 0..96 {
                let mut geometry = Geometry::default();
                let status = append(
                    &mut geometry,
                    &mut pulses,
                    frame as f32 * 0.05,
                    Some(&session),
                    mouth + Vec3::new(0.0, 2.0, 4.0),
                    (&chunks, &journal, &terrain),
                );
                assert_eq!(status.active_inlets, 1);
                assert_eq!(status.bubbles, 4);
                for point in &geometry.positions {
                    let point = Vec3::from_array(*point);
                    assert!(
                        point.y < 8.0 - BUBBLE_SURFACE_GAP + 0.00001,
                        "effect crossed water plane at {point:?}"
                    );
                    assert!(point.y > 7.0 + BUBBLE_BED_GAP - 0.00001);
                    assert!(
                        (point.xz() - mouth.xz()).length() > flange_radius + 0.005,
                        "bubble intersected the authored flange at {point:?}"
                    );
                }
                for triangle in geometry.indices.chunks_exact(3) {
                    let heights = [triangle[0], triangle[1], triangle[2]]
                        .map(|index| geometry.positions[index as usize][1]);
                    let minimum = heights.into_iter().fold(f32::INFINITY, f32::min);
                    let maximum = heights.into_iter().fold(f32::NEG_INFINITY, f32::max);
                    assert!(
                        maximum - minimum > 0.000001,
                        "intake emitted a second flat water/ripple sheet"
                    );
                }
            }
        }
    }

    #[test]
    fn bubbles_are_round_closed_outward_and_remain_small_bounded_shared_geometry() {
        let mut geometry = Geometry::default();
        let center = Vec3::new(-17.5, 7.2, -0.5);
        bubble(&mut geometry, center, 0.025, 0.6);
        assert!(geometry.positions.len() < 256);
        for p in &geometry.positions {
            assert!((Vec3::from_array(*p) - center).length() <= 0.02501);
        }
        for t in geometry.indices.chunks_exact(3) {
            let [a, b, c] =
                std::array::from_fn(|i| Vec3::from_array(geometry.positions[t[i] as usize]));
            assert!(
                (b - a)
                    .cross(c - a)
                    .dot(Vec3::from_array(geometry.normals[t[0] as usize]))
                    > 0.
            );
        }
    }
}
