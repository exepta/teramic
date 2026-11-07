//! Continuous, closed water volume between the real nozzle and its held vessel.
use super::Geometry;
use bevy::prelude::*;

const SIDES: usize = 12;
const RINGS: usize = 16;
const MOUTH_RADIUS: f32 = 0.019;
const BODY_RADIUS: f32 = 0.043;
const EFFECT: f32 = 8.;

#[derive(Resource, Default, Debug)]
pub struct InfuserJetRenderStatus {
    pub streams: usize,
    pub vertices: usize,
    /// Accepted network snapshot used to select the real active filling item.
    pub plumbing_sequence: u64,
}

pub(super) fn target(
    session: &terra_network::client::ClientSession,
    node: &terra_network::protocol::session::PlumbingVisual,
    journal: &terra_world::edits::EditJournal,
) -> Option<Vec3> {
    use terra_core::infusion;
    if !node.supplied {
        return None;
    }
    let hold = infusion::hold_below(node.position);
    let active = session
        .infusion
        .iter()
        .find(|view| view.position == hold && view.phase == infusion::HoldPhase::Filling)?
        .active_drop?;
    // Filling confirms enough fluid across the entire upstream component.
    // Its terminal buffer can be empty while upstream pipes supply the item.
    let item = session.drops.iter().find(|drop| {
        drop.id == active
            && drop.stack.count == 1
            && infusion::fillable(drop.stack.item)
            && terra_world::conveyors::infusion_carrying(drop, journal) == Some(hold)
            && (drop.position[0] - hold[0] as f64 - 0.5).abs() < 0.31
            && (drop.position[2] - hold[2] as f64 - 0.5).abs() < 0.31
    })?;
    Some(Vec3::from_array(item.position.map(|v| v as f32)) + Vec3::Y * 0.08)
}

pub(super) fn append(mesh: &mut Geometry, mouth: Vec3, target: Vec3, time: f32) -> usize {
    let length = mouth.distance(target);
    if !mouth.is_finite() || !target.is_finite() || !time.is_finite() || length < 0.01 {
        return 0;
    }
    let initial = mesh.positions.len();
    let axis = (target - mouth) / length;
    let radial = axis.any_orthonormal_vector();
    let cross = axis.cross(radial);
    let ring = |index: usize| {
        let t = index as f32 / RINGS as f32;
        // The first rim fits inside the authored 0.020-radius nozzle bore.
        // Gentle travelling necks retain a connected core for every frame.
        let expansion = (t * length / 0.13).clamp(0., 1.);
        let ripple = 1. + (t * length * 26. - time * 19.).sin() * 0.065 * expansion;
        let radius = (MOUTH_RADIUS + (BODY_RADIUS - MOUTH_RADIUS) * expansion) * ripple;
        let center = mouth.lerp(target, t);
        std::array::from_fn::<_, SIDES, _>(|side| {
            let angle = side as f32 * std::f32::consts::TAU / SIDES as f32;
            let normal = radial * angle.cos() + cross * angle.sin();
            (center + normal * radius, normal, radius * 2., t)
        })
    };
    for index in 0..RINGS {
        let a = ring(index);
        let b = ring(index + 1);
        for side in 0..SIDES {
            let next = (side + 1) % SIDES;
            let base = mesh.positions.len() as u32;
            for (point, normal, diameter, t) in [a[side], a[next], b[next], b[side]] {
                mesh.positions.push(point.to_array());
                mesh.normals.push(normal.to_array());
                mesh.colors.push([diameter, t, 1., EFFECT]);
            }
            mesh.indices
                .extend([base, base + 1, base + 2, base, base + 2, base + 3]);
        }
    }
    // End caps meet the nozzle and item exactly; no floating droplets/gaps.
    for (index, normal) in [(0, -axis), (RINGS, axis)] {
        let rim = ring(index);
        let center = mouth.lerp(target, index as f32 / RINGS as f32);
        let base = mesh.positions.len() as u32;
        mesh.positions.push(center.to_array());
        mesh.normals.push(normal.to_array());
        mesh.colors.push([rim[0].2, rim[0].3, 1., EFFECT]);
        for &(point, _, diameter, t) in &rim {
            mesh.positions.push(point.to_array());
            mesh.normals.push(normal.to_array());
            mesh.colors.push([diameter, t, 1., EFFECT]);
        }
        for side in 0..SIDES {
            let a = base + 1 + side as u32;
            let b = base + 1 + ((side + 1) % SIDES) as u32;
            mesh.indices.extend(if index == 0 {
                [base, b, a]
            } else {
                [base, a, b]
            });
        }
    }
    mesh.positions.len() - initial
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confirmed_filling_uses_upstream_supply_even_when_the_terminal_buffer_is_empty() {
        use terra_core::{ItemKind, ItemStack, ToolKind, WorldDrop, infusion};
        let hold = [-17, 4, 16];
        let mut journal = terra_world::edits::EditJournal::default();
        journal.record(hold, infusion::HOLD_BLOCK).unwrap();
        journal
            .set_object_state(hold, Some(Default::default()))
            .unwrap();
        let mut session = terra_network::client::ClientSession::default();
        session.infusion.push(infusion::HoldVisual {
            position: hold,
            active_drop: Some(7),
            phase: infusion::HoldPhase::Filling,
            gates: true,
            progress: 400,
            ..Default::default()
        });
        session.drops.push(WorldDrop {
            id: 7,
            position: [-16.5, 4. + infusion::DROP_HEIGHT, 16.5],
            stack: ItemStack {
                item: ItemKind::Tool(ToolKind::Bucket),
                count: 1,
            },
            velocity: [0.; 3],
            age_seconds: 0.,
            rotation: [0., 0., 0., 1.],
            angular_velocity: [0.; 3],
            grounded: true,
            pickup_delay_seconds: 0.,
        });
        let mut node = terra_network::protocol::session::PlumbingVisual {
            position: infusion::infuser_above(hold),
            amount: 0,
            capacity: 8,
            open: true,
            flowing: true,
            supplied: true,
            pump_rpm: 0.,
            basin: None,
            intake: None,
        };
        assert!(target(&session, &node, &journal).is_some());
        node.supplied = false;
        assert!(target(&session, &node, &journal).is_none());
        node.supplied = true;
        for phase in [
            infusion::HoldPhase::Approaching,
            infusion::HoldPhase::WaitingForFluid,
            infusion::HoldPhase::Releasing,
        ] {
            session.infusion[0].phase = phase;
            assert!(target(&session, &node, &journal).is_none());
        }
        session.infusion[0].phase = infusion::HoldPhase::Filling;
        session.drops[0].stack.item = ItemKind::Tool(ToolKind::WaterBucket);
        assert!(target(&session, &node, &journal).is_none());
        session.drops[0].stack.item = ItemKind::GlassBottle;
        session.drops[0].position[0] += 0.4;
        assert!(target(&session, &node, &journal).is_none());
    }

    #[test]
    fn continuous_jet_meets_nozzle_and_item_and_has_closed_outward_finite_faces() {
        for (mouth, target) in [
            (Vec3::new(10.5, 19.012, 0.5), Vec3::new(10.5, 17.835, 0.5)),
            (
                Vec3::new(-16.5, 4.012, -0.5),
                Vec3::new(-16.48, 2.835, -0.52),
            ),
        ] {
            let mut previous = None;
            for time in [0., 0.125, 0.31, 13.7] {
                let mut mesh = Geometry::default();
                let vertices = append(&mut mesh, mouth, target, time);
                assert_eq!(vertices, SIDES * RINGS * 4 + 2 * (SIDES + 1));
                assert_eq!(mesh.indices.len(), (SIDES * RINGS * 2 + SIDES * 2) * 3);
                let axis = (target - mouth).normalize();
                let length = mouth.distance(target);
                for (point, color) in mesh.positions.iter().zip(&mesh.colors) {
                    let offset = Vec3::from_array(*point) - mouth;
                    let t = offset.dot(axis) / length;
                    assert!((-1e-5..=1.00001).contains(&t));
                    assert!((offset - axis * offset.dot(axis)).length() <= BODY_RADIUS * 1.066);
                    assert_eq!(color[3], EFFECT);
                    assert!(point.iter().all(|p| p.is_finite()));
                }
                for triangle in mesh.indices.chunks_exact(3) {
                    let [a, b, c] = std::array::from_fn(|i| {
                        Vec3::from_array(mesh.positions[triangle[i] as usize])
                    });
                    let normal = Vec3::from_array(mesh.normals[triangle[0] as usize]);
                    assert!((b - a).cross(c - a).dot(normal) > 0.);
                }
                let step = length / RINGS as f32;
                let precision =
                    mouth.abs().max(target.abs()).max_element().max(1.) * f32::EPSILON * 4.;
                for index in 0..RINGS {
                    let first = Vec3::from_array(mesh.positions[index * SIDES * 4]);
                    let last = Vec3::from_array(mesh.positions[index * SIDES * 4 + 3]);
                    assert!(((last - first).dot(axis) - step).abs() <= precision);
                    if index + 1 < RINGS {
                        assert_eq!(
                            mesh.positions[index * SIDES * 4 + 3],
                            mesh.positions[(index + 1) * SIDES * 4],
                            "adjacent rings must meet exactly"
                        );
                    }
                }
                if let Some(previous) = previous {
                    assert_ne!(mesh.positions, previous);
                }
                previous = Some(mesh.positions);
            }
        }
    }

    #[test]
    fn jet_uses_own_thickness_and_axial_highlights_in_the_shared_biome_water_material() {
        use crate::water::material::ATTRIBUTE_WAVE_WEIGHT;
        use bevy::mesh::VertexAttributeValues;
        let mut geometry = Geometry::default();
        append(&mut geometry, Vec3::Y * 2., Vec3::ZERO, 0.);
        let terrain = terra_world::TerraGenerator::new(Default::default()).unwrap();
        let mesh = geometry.water_mesh(&terrain);
        let Some(VertexAttributeValues::Float32x2(uv)) = mesh.attribute(Mesh::ATTRIBUTE_UV_0)
        else {
            panic!("jet optical coordinates");
        };
        assert!(
            uv.iter()
                .all(|p| p[0] > 0. && p[0] < BODY_RADIUS * 2.14 && (0. ..=1.).contains(&p[1]))
        );
        assert!(uv.iter().any(|p| p[1] == 0.));
        assert!(uv.iter().any(|p| p[1] == 1.));
        assert!(
            matches!(mesh.attribute(Mesh::ATTRIBUTE_COLOR), Some(VertexAttributeValues::Float32x4(colors)) if colors.iter().all(|c| c[3] == EFFECT))
        );
        assert!(
            matches!(mesh.attribute(ATTRIBUTE_WAVE_WEIGHT), Some(VertexAttributeValues::Float32(weights)) if weights.iter().all(|&w| w == 0.))
        );
    }
}
