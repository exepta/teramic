//! The fixed reference coupling stays visible; optional fittings follow connected pipes.
use super::ModelCell;
use crate::blocks::PrimitiveTemplate;
use bevy::math::{Quat, Vec3};
use terra_core::plumbing;

fn inlet(part: &PrimitiveTemplate) -> Option<usize> {
    for side in [0, 1, 3, 4, 5] {
        let axis = side / 2;
        let positive = side % 2 == 1;
        if part.positions.iter().all(|p| {
            if positive {
                p[axis] >= 0.7999
            } else {
                p[axis] <= 0.2001
            }
        }) {
            return Some(side);
        }
    }
    None
}

pub(super) fn profile(
    model: &ModelCell,
    source: &[PrimitiveTemplate],
    standalone: bool,
) -> Vec<PrimitiveTemplate> {
    let rotation =
        Quat::from_rotation_y(f32::from(model.state.facing % 4) * std::f32::consts::FRAC_PI_2);
    source
        .iter()
        .filter(|part| {
            let Some(local) = inlet(part) else {
                return true;
            };
            if standalone {
                return local == 1;
            }
            let direction = (rotation
                * Vec3::from_array(plumbing::DIRECTIONS[local].map(|v| v as f32)))
            .to_array()
            .map(|v| v.round() as i64);
            let Some(side) = plumbing::direction_index(direction) else {
                return false;
            };
            model.neighbors[side].is_some_and(|material| {
                plumbing::connects(material, model.neighbor_facing[side], direction.map(|v| -v))
            })
        })
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actual_head_keeps_the_reference_coupling_nozzle_and_connected_optional_inlet_at_all_yaws() {
        let source = crate::blocks::test_static_scene(
            "terra/blocks/models/fluid_infuser.gltf",
            terra_core::infusion::FLUID_INFUSER,
        );
        for facing in 0..4 {
            let mut model = ModelCell {
                material: terra_core::infusion::FLUID_INFUSER,
                state: terra_world::edits::BlockObjectState {
                    facing,
                    ..Default::default()
                },
                cell: [0; 3],
                neighbors: [None; 6],
                neighbor_facing: [0; 6],
                peer_wood: Default::default(),
            };
            let rotation = Quat::from_rotation_y(f32::from(facing) * std::f32::consts::FRAC_PI_2);
            let direction = (rotation * Vec3::X).to_array().map(|v| v.round() as i64);
            let side = plumbing::direction_index(direction).unwrap();
            model.neighbors[side] = Some(plumbing::IRON_PIPE);
            let parts = profile(&model, &source, false);
            assert!(parts.iter().all(|part| inlet(part).is_none()));
            // This hollow +X connector belongs to the housing even without a pipe.
            model.neighbors = [None; 6];
            let unconnected = profile(&model, &source, false);
            assert!(
                unconnected
                    .iter()
                    .flat_map(|part| &part.positions)
                    .any(|p| {
                        (p[0] - 1.).abs() < 1e-5
                            && (p[1] - 0.5).abs() < 0.17
                            && (p[2] - 0.5).abs() < 0.17
                    })
            );
            model.neighbors[3] = Some(plumbing::IRON_PIPE);
            let with_top = profile(&model, &source, false);
            assert!(with_top.iter().any(|part| inlet(part) == Some(3)));
            assert!(
                with_top
                    .iter()
                    .all(|part| inlet(part).is_none_or(|side| side == 3))
            );
            assert!(
                parts
                    .iter()
                    .flat_map(|part| &part.positions)
                    .any(|p| p[1].abs() < 1e-5
                        && (p[0] - 0.5).abs() < 0.1
                        && (p[2] - 0.5).abs() < 0.1)
            );
        }
    }
}
