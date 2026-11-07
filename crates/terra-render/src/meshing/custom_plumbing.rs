//! Hydraulic profiles reuse authored primitives and the existing per-material batches.
use super::*;
use crate::blocks::PrimitiveTemplate;
use bevy::math::{Quat, Vec3};
#[path = "custom_basin.rs"]
mod basin;
#[path = "custom_pipe.rs"]
pub(crate) mod pipe;

fn side(primitive: &PrimitiveTemplate) -> Option<usize> {
    let mut minimum = Vec3::splat(f32::INFINITY);
    let mut maximum = Vec3::splat(f32::NEG_INFINITY);
    for &point in &primitive.positions {
        let point = Vec3::from_array(point);
        minimum = minimum.min(point);
        maximum = maximum.max(point);
    }
    let delta = (minimum + maximum) * 0.5 - Vec3::splat(0.5);
    let axis = (0..3).max_by(|&a, &b| delta[a].abs().total_cmp(&delta[b].abs()))?;
    (delta[axis].abs() > 0.14).then_some(axis * 2 + usize::from(delta[axis] > 0.0))
}

fn connected_mask(model: &ModelCell, standalone: bool) -> u8 {
    if standalone {
        0
    } else {
        (0..6).fold(0, |mask, side| {
            let connects = model.neighbors[side].is_some_and(|material| {
                terra_core::plumbing::connects(
                    material,
                    model.neighbor_facing[side],
                    terra_core::plumbing::DIRECTIONS[side ^ 1],
                )
            });
            mask | if connects { 1 << side } else { 0 }
        })
    }
}

fn pipe_mask(model: &ModelCell, standalone: bool) -> u8 {
    let connected = connected_mask(model, standalone);
    if connected.count_ones() == 1 {
        let side = connected.trailing_zeros();
        return connected | (1 << (side ^ 1));
    }
    if connected != 0 {
        return connected;
    }
    let axis = terra_core::kinetic::axis(model.state.facing);
    let index = axis.iter().position(|v| *v != 0).unwrap_or(0);
    3 << (index * 2)
}

#[derive(Default)]
pub(super) struct Profiles(std::collections::HashMap<(u16, u8, u8), Vec<PrimitiveTemplate>>);
impl Profiles {
    pub(super) fn get<'a>(
        &'a mut self,
        model: &ModelCell,
        primitives: &[PrimitiveTemplate],
        standalone: bool,
    ) -> std::borrow::Cow<'a, [PrimitiveTemplate]> {
        if !terra_core::plumbing::is_pipe(model.material) {
            return std::borrow::Cow::Owned(profile(model, primitives, standalone));
        }
        let mask = pipe_mask(model, standalone);
        let key = (
            model.material,
            mask,
            mask & !connected_mask(model, standalone),
        );
        let full = self.0.len() >= 64;
        match self.0.entry(key) {
            std::collections::hash_map::Entry::Occupied(entry) => {
                std::borrow::Cow::Borrowed(entry.into_mut())
            }
            std::collections::hash_map::Entry::Vacant(entry) => {
                let parts = profile(model, primitives, standalone);
                if full {
                    std::borrow::Cow::Owned(parts)
                } else {
                    std::borrow::Cow::Borrowed(entry.insert(parts))
                }
            }
        }
    }
}

pub(super) fn profile(
    model: &ModelCell,
    primitives: &[PrimitiveTemplate],
    standalone: bool,
) -> Vec<PrimitiveTemplate> {
    let mask = if terra_core::plumbing::is_pipe(model.material) {
        pipe_mask(model, standalone)
    } else if standalone {
        0
    } else {
        model.state.facing
    };
    if terra_core::plumbing::is_pipe(model.material) {
        let Some(body_material) = primitives
            .iter()
            .find(|part| side(part).is_none())
            .map(|part| part.material)
        else {
            return Vec::new();
        };
        let mut parts: Vec<_> = primitives
            .iter()
            .filter(|part| {
                part.material != body_material
                    && side(part).is_some_and(|side| mask & (1 << side) != 0)
            })
            .cloned()
            .collect();
        parts.push(pipe::body(
            mask,
            body_material,
            mask & !connected_mask(model, standalone),
        ));
        return parts;
    }
    if model.material == terra_core::plumbing::BASIN {
        return basin::profile(model, primitives, standalone);
    }
    primitives
        .iter()
        .filter(|primitive| {
            if terra_core::plumbing::is_pipe(model.material) {
                return side(primitive).is_none_or(|side| mask & (1 << side) != 0);
            }
            true
        })
        .cloned()
        .map(|mut primitive| {
            let handle_angle = if model.material == 166
                && model.state.facing & 8 != 0
                && primitive.positions.iter().all(|p| p[1] >= 0.785)
            {
                std::f32::consts::FRAC_PI_4
            } else if model.material == 167 && primitive.positions.iter().all(|p| p[1] >= 0.70) {
                f32::from(3 - terra_core::plumbing::faucet_level(model.state.facing))
                    * std::f32::consts::FRAC_PI_6
            } else {
                0.0
            };
            if handle_angle != 0.0 {
                let rotation = Quat::from_rotation_y(handle_angle);
                for p in &mut primitive.positions {
                    *p = (rotation * (Vec3::from_array(*p) - Vec3::new(0.5, 0.83, 0.5))
                        + Vec3::new(0.5, 0.83, 0.5))
                    .to_array();
                }
                for n in &mut primitive.normals {
                    *n = (rotation * Vec3::from_array(*n)).to_array();
                }
                for t in &mut primitive.tangents {
                    let v = rotation * Vec3::new(t[0], t[1], t[2]);
                    *t = [v.x, v.y, v.z, t[3]];
                }
            }
            primitive
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn primitive(min: [f32; 3], max: [f32; 3]) -> PrimitiveTemplate {
        PrimitiveTemplate {
            positions: vec![min, max],
            normals: vec![],
            tangents: vec![],
            uvs: vec![],
            indices: vec![],
            material: 0,
        }
    }
    #[test]
    fn authored_arm_classification_uses_all_six_native_axes_not_material_aliases() {
        for side_index in 0..6 {
            let mut min = [0.4; 3];
            let mut max = [0.6; 3];
            min[side_index / 2] = if side_index % 2 == 0 { 0.0 } else { 0.64 };
            max[side_index / 2] = if side_index % 2 == 0 { 0.36 } else { 1.0 };
            assert_eq!(side(&primitive(min, max)), Some(side_index));
        }
        assert_eq!(side(&primitive([0.35; 3], [0.65; 3])), None);
    }
    #[test]
    fn profile_cache_distinguishes_free_pipe_mouths_from_the_same_connected_axis() {
        let mut model = ModelCell {
            cell: [0; 3],
            material: terra_core::plumbing::GLASS_PIPE,
            state: Default::default(),
            neighbors: [None; 6],
            neighbor_facing: [0; 6],
            peer_wood: Default::default(),
        };
        let source = vec![primitive([0.35; 3], [0.65; 3])];
        let mut profiles = Profiles::default();
        model.neighbors[0] = Some(terra_core::plumbing::IRON_PIPE);
        let exposed = profiles.get(&model, &source, false).into_owned();
        model.neighbors[1] = Some(terra_core::plumbing::GLASS_PIPE);
        let connected = profiles.get(&model, &source, false).into_owned();
        let planar = |parts: &[PrimitiveTemplate]| {
            parts
                .iter()
                .map(|part| {
                    part.indices
                        .chunks_exact(3)
                        .filter(|triangle| {
                            triangle
                                .iter()
                                .all(|&i| (part.positions[i as usize][0] - 1.).abs() < 1e-5)
                        })
                        .count()
                })
                .sum::<usize>()
        };
        assert_eq!(planar(&exposed), 32);
        assert_eq!(planar(&connected), 0);
        assert_eq!(profiles.0.len(), 2);
    }

    #[test]
    fn basin_neighbor_mask_opens_shared_wall_and_omits_intermediate_floor() {
        let model = ModelCell {
            cell: [0; 3],
            material: 169,
            state: terra_world::edits::BlockObjectState {
                facing: 1 | 4,
                ..Default::default()
            },
            neighbors: [None; 6],
            neighbor_facing: [0; 6],
            peer_wood: Default::default(),
        };
        let parts = profile(&model, &[], false);
        assert!(!parts.is_empty());
        // Wall end caps can remain when the neighbor is not in this snapshot.
        // The interior floor must be absent, independently of those solid rims.
        assert!(
            parts
                .iter()
                .all(|part| part.indices.chunks_exact(3).all(|triangle| {
                    let points = [triangle[0], triangle[1], triangle[2]]
                        .map(|i| Vec3::from_array(part.positions[i as usize]));
                    let center = (points[0] + points[1] + points[2]) / 3.0;
                    !(part.normals[triangle[0] as usize][1] < -0.9
                        && points.iter().all(|p| p.y == 0.0)
                        && (0.16..0.84).contains(&center.x)
                        && (0.16..0.84).contains(&center.z))
                }))
        );

        assert!(
            parts
                .iter()
                .flat_map(|p| p.positions.iter().zip(&p.normals))
                .any(|(point, normal)| { point[0] == 0.84 && normal[0] < -0.9 })
        );
    }
}
