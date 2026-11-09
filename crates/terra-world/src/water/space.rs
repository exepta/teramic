//! Liquid occupies the complement of the same slab/stair boxes used by collision.
use super::FluidCell;
use crate::edits::EditJournal;
use terra_core::{block_shapes::ShapeBox, construction};

const EPS: f64 = 1e-8;

pub(crate) fn partial(material: u16) -> bool {
    // These reservoirs own their internal liquid separately. Other authored
    // shapes expose their actual outer free space instead of a dry unit cube.
    !matches!(
        material,
        terra_core::plumbing::BASIN | terra_core::brewing::MATERIAL
    ) && (terra_core::rails::is_rail(material)
        || construction::is_shape(material)
        || !terra_core::block_shapes::builtin_boxes(material).is_empty())
}

#[derive(Clone, Debug)]
pub(crate) struct FluidSpace {
    pub solids: Vec<ShapeBox>,
}

impl FluidSpace {
    /// Actual solids plus the dry side of a water-containing fence. Collision
    /// continues to use `at`; liquid queries and meshing use this same region.
    pub fn wet_at(
        journal: &EditJournal,
        p: [i64; 3],
        mut block_at: impl FnMut([i64; 3]) -> Option<u16>,
        water_at: impl FnMut([i64; 3], u16) -> FluidCell,
    ) -> Self {
        let fence = block_at(p).is_some_and(construction::is_fence);
        let mut space = Self::at(journal, p, &mut block_at);
        if fence {
            space
                .solids
                .extend(super::fence::dry_regions(journal, p, block_at, water_at));
        }
        space
    }

    pub fn at(
        journal: &EditJournal,
        p: [i64; 3],
        mut block_at: impl FnMut([i64; 3]) -> Option<u16>,
    ) -> Self {
        let mut space = Self::base(journal, p, &mut block_at);
        journal.visit_extended_kinetics_near(p, |owner| {
            if owner == p {
                return;
            }
            let Some(material) = block_at(owner).filter(|m| terra_core::kinetic::is_extended(*m))
            else {
                return;
            };
            let facing = journal.object_state(owner).unwrap_or_default().facing;
            for bounds in terra_core::block_shapes::builtin_boxes(material) {
                let rotated = bounds.rotated_axis(facing);
                let clipped = ShapeBox {
                    minimum: std::array::from_fn(|i| {
                        (rotated.minimum[i] + (owner[i] - p[i]) as f64).max(0.0)
                    }),
                    maximum: std::array::from_fn(|i| {
                        (rotated.maximum[i] + (owner[i] - p[i]) as f64).min(1.0)
                    }),
                    hinged: false,
                };
                if (0..3).all(|i| clipped.maximum[i] - clipped.minimum[i] > EPS) {
                    space.solids.push(clipped);
                }
            }
        });
        space
    }

    fn base(
        journal: &EditJournal,
        p: [i64; 3],
        mut block_at: impl FnMut([i64; 3]) -> Option<u16>,
    ) -> Self {
        let material = block_at(p).unwrap_or(u16::MAX);
        if terra_core::rails::is_rail(material) {
            return Self {
                solids: crate::rails::cell_boxes(
                    p,
                    material,
                    journal.object_state(p).unwrap_or_default(),
                )
                .to_vec(),
            };
        }
        if terra_core::lantern::is_part(material) {
            return Self {
                solids: terra_core::lantern::boxes(
                    material,
                    journal.object_state(p).unwrap_or_default().facing,
                ),
            };
        }
        if terra_core::castle::is_castle(material) {
            let facing = journal.object_state(p).unwrap_or_default().facing;
            let neighbors = std::array::from_fn(|side| {
                let mut cell = p;
                cell[side / 2] += if side % 2 == 0 { -1 } else { 1 };
                block_at(cell).map(|id| (id, journal.object_state(cell).unwrap_or_default().facing))
            });
            return Self {
                solids: terra_core::castle::connections::placed_boxes(material, facing, neighbors),
            };
        }
        // Pipe contents are stored separately; external water occupies only the
        // free space around the same connected geometry used for collision.
        if terra_core::plumbing::is_pipe(material)
            || matches!(
                material,
                terra_core::plumbing::VALVE..=terra_core::plumbing::PUMP
            )
        {
            return Self {
                solids: crate::plumbing::plumbing_boxes(
                    journal,
                    p,
                    material,
                    journal.object_state(p).unwrap_or_default(),
                    &mut block_at,
                ),
            };
        }
        if material == terra_core::crusher::CRANK_MATERIAL
            || matches!(material, 141..=151)
            || terra_core::kinetic::is_gearbox(material)
        {
            let state = journal.object_state(p).unwrap_or_default();
            let facing = state.facing;
            let solids = terra_core::block_shapes::builtin_boxes(material)
                .iter()
                .map(|bounds| {
                    let mut bounds = *bounds;
                    if material == terra_core::crusher::CRANK_MATERIAL {
                        for (i, offset) in terra_core::kinetic::crank_local_offset(p, state.peer)
                            .into_iter()
                            .enumerate()
                        {
                            bounds.minimum[i] += f64::from(offset);
                            bounds.maximum[i] += f64::from(offset);
                        }
                    }
                    if material == 148 {
                        bounds.rotated_y(facing)
                    } else {
                        bounds.rotated_axis(facing)
                    }
                })
                .collect();
            return Self { solids };
        }
        if !partial(material) {
            return Self {
                solids: if super::can_hold_fluid(material) {
                    vec![]
                } else {
                    vec![unit()]
                },
            };
        }
        let facing = journal.object_state(p).unwrap_or_default().facing;
        if construction::is_fence(material) {
            return Self {
                solids: construction::boxes(material)
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| {
                        construction::fence_arm(*index).is_none_or(|arm| {
                            let side = construction::fence_neighbor_side(arm, facing);
                            let mut neighbor = p;
                            neighbor[side / 2] += if side.is_multiple_of(2) { -1 } else { 1 };
                            construction::connects(block_at(neighbor))
                        })
                    })
                    .map(|(_, bounds)| bounds.rotated_y(facing))
                    .collect(),
            };
        }
        if !construction::is_slab(material) && !construction::is_stair(material) {
            #[cfg(feature = "bevy")]
            {
                let mut solids = Vec::new();
                if crate::interaction_shapes::visit_boxes_with(
                    journal,
                    &terra_core::block_properties::BlockPropertiesCatalog::default(),
                    p,
                    &mut block_at,
                    |mut bounds| {
                        for (axis, coordinate) in p.iter().enumerate() {
                            bounds.minimum[axis] -= *coordinate as f64;
                            bounds.maximum[axis] -= *coordinate as f64;
                        }
                        solids.push(bounds);
                    },
                ) {
                    return Self { solids };
                }
            }
            #[cfg(not(feature = "bevy"))]
            {
                return Self {
                    solids: builtin_placed_boxes(journal, p, material, &mut block_at),
                };
            }
        }
        let neighbors = std::array::from_fn(|side| {
            let mut q = p;
            q[side / 2] += if side % 2 == 0 { -1 } else { 1 };
            block_at(q).map(|id| (id, journal.object_state(q).unwrap_or_default().facing))
        });
        Self {
            solids: construction::placed_boxes(material, facing, neighbors),
        }
    }

    #[cfg(any(feature = "bevy", test))]
    pub fn from_global(solids: &[ShapeBox], p: [i64; 3]) -> Self {
        Self {
            solids: solids
                .iter()
                .map(|b| ShapeBox {
                    minimum: std::array::from_fn(|i| b.minimum[i] - p[i] as f64),
                    maximum: std::array::from_fn(|i| b.maximum[i] - p[i] as f64),
                    hinged: false,
                })
                .collect(),
        }
    }

    pub fn wet_boxes(&self, height: f64) -> Vec<ShapeBox> {
        if height <= 0.0 {
            return vec![];
        }
        let mut water = vec![ShapeBox {
            maximum: [1.0, height.min(1.0), 1.0],
            ..unit()
        }];
        for solid in &self.solids {
            water = water
                .into_iter()
                .flat_map(|b| subtract(b, *solid))
                .collect();
        }
        water
    }

    pub fn clamp(&self, mut cell: FluidCell) -> FluidCell {
        if self.solids.is_empty() || cell.amount == 0 {
            return cell;
        }
        let boxes = self.wet_boxes(cell.height());
        let top = boxes.iter().map(|b| b.maximum[1]).fold(0.0, f64::max);
        if top <= EPS {
            return FluidCell::default();
        }
        cell.amount = cell.amount.min((top * 8.0).ceil() as u8);
        cell
    }

    #[cfg(any(feature = "bevy", test))]
    pub fn contains(&self, cell: FluidCell, point: [f64; 3]) -> bool {
        (0..3).all(|i| point[i] >= 0.0 && point[i] < 1.0)
            && point[1] >= 0.0
            && point[1] < cell.height()
            && !self
                .solids
                .iter()
                .any(|b| (0..3).all(|i| point[i] >= b.minimum[i] && point[i] < b.maximum[i]))
    }

    #[cfg(any(feature = "bevy", test))]
    pub fn connected(
        &self,
        cell: FluidCell,
        other: &Self,
        other_cell: FluidCell,
        direction: [i64; 3],
    ) -> bool {
        let axis = direction.iter().position(|&v| v != 0).unwrap();
        let high = direction[axis] > 0;
        if cell.amount == 0
            || other_cell.amount == 0
            || axis == 1
                && if high {
                    cell.amount != 8
                } else {
                    other_cell.amount != 8
                }
        {
            return false;
        }
        let _scope = terra_core::profiling::ScopeTimer::new("water.space.connected");
        // Only the shared face matters. Extrude its free rectangles along the
        // normal so the existing subtraction routine performs two-dimensional
        // cuts. Cutting both entire rail volumes creates thousands of interior
        // fragments that cannot influence a connection across this face.
        let mut face = unit();
        if axis != 1 {
            face.maximum[1] = cell.height().min(other_cell.height());
        }
        let mut open = vec![face];
        for (space, positive) in [(self, high), (other, !high)] {
            for solid in &space.solids {
                let touches = if positive {
                    solid.maximum[axis] >= 1.0 - EPS && solid.minimum[axis] < 1.0 - EPS
                } else {
                    solid.minimum[axis] <= EPS && solid.maximum[axis] > EPS
                };
                if !touches {
                    continue;
                }
                let mut shadow = *solid;
                shadow.minimum[axis] = 0.0;
                shadow.maximum[axis] = 1.0;
                open = open
                    .into_iter()
                    .flat_map(|box_| subtract(box_, shadow))
                    .collect();
                if open.is_empty() {
                    return false;
                }
            }
        }
        true
    }
}

/// Persistence-only builds have no runtime section/assembly model. Reuse the
/// same authored poses for their sub-cell sampling; runtime builds above use
/// the complete existing placement/collision visitor, including joined parts.
#[cfg(not(feature = "bevy"))]
fn builtin_placed_boxes(
    journal: &EditJournal,
    p: [i64; 3],
    material: u16,
    mut block_at: impl FnMut([i64; 3]) -> Option<u16>,
) -> Vec<ShapeBox> {
    let selected = journal.object_state(p).unwrap_or_default();
    if crate::objects::is_door(material)
        && let Some(group) = crate::objects::doors::group(p, material, selected, |cell| {
            Some((
                block_at(cell).or_else(|| journal.block(cell))?,
                journal.object_state(cell)?,
            ))
        })
        && group.width == 2
    {
        return crate::objects::doors::pieces()
            .into_iter()
            .filter(|piece| piece.solid)
            .map(|piece| {
                let mut bounds = group.world_box(piece);
                for (axis, coordinate) in p.into_iter().enumerate() {
                    bounds.minimum[axis] -= coordinate as f64;
                    bounds.maximum[axis] -= coordinate as f64;
                }
                bounds
            })
            .collect();
    }
    let owner = if selected.secondary {
        selected.peer.unwrap_or(p)
    } else {
        p
    };
    let state = if owner == p {
        selected
    } else {
        journal.object_state(owner).unwrap_or_default()
    };
    let template = if material == 60 {
        crate::objects::chimney_basis(state.facing).map_or(material, |(id, _)| id)
    } else {
        material
    };
    terra_core::block_shapes::builtin_boxes(template)
        .iter()
        .map(|source| {
            let mut bounds = ShapeBox {
                minimum: [f64::INFINITY; 3],
                maximum: [f64::NEG_INFINITY; 3],
                hinged: false,
            };
            if crate::objects::is_opening(material) {
                bounds = crate::objects::opening_box(*source, owner, state);
            } else {
                let width = if matches!(material, 28 | 29 | 99) && state.peer.is_some() {
                    2.0
                } else {
                    1.0
                };
                for corner in 0..8 {
                    let point = std::array::from_fn(|axis| {
                        if corner & (1 << axis) == 0 {
                            source.minimum[axis]
                        } else {
                            source.maximum[axis]
                        }
                    });
                    let point = if material == 40 {
                        terra_core::block_shapes::torch_point(state.facing, point)
                    } else if material == 60 && state.facing >= 8 {
                        crate::objects::chimney_point(state.facing, point)
                    } else {
                        let [x, y, z] = [point[0] * width - 0.5, point[1], point[2] - 0.5];
                        let [x, y, z] = match state.facing % 4 {
                            0 => [x, y, z],
                            1 => [z, y, -x],
                            2 => [-x, y, -z],
                            _ => [-z, y, x],
                        };
                        [x + 0.5, y, z + 0.5]
                    };
                    for axis in 0..3 {
                        bounds.minimum[axis] =
                            bounds.minimum[axis].min(point[axis] + owner[axis] as f64);
                        bounds.maximum[axis] =
                            bounds.maximum[axis].max(point[axis] + owner[axis] as f64);
                    }
                }
            }
            for (axis, coordinate) in p.iter().enumerate() {
                bounds.minimum[axis] -= *coordinate as f64;
                bounds.maximum[axis] -= *coordinate as f64;
            }
            bounds
        })
        .collect()
}

fn unit() -> ShapeBox {
    ShapeBox {
        minimum: [0.0; 3],
        maximum: [1.0; 3],
        hinged: false,
    }
}

fn subtract(mut water: ShapeBox, solid: ShapeBox) -> Vec<ShapeBox> {
    let low: [f64; 3] = std::array::from_fn(|i| water.minimum[i].max(solid.minimum[i]));
    let high: [f64; 3] = std::array::from_fn(|i| water.maximum[i].min(solid.maximum[i]));
    if (0..3).any(|i| high[i] - low[i] <= EPS) {
        return vec![water];
    }
    let mut result = Vec::with_capacity(6);
    for i in 0..3 {
        if water.minimum[i] < low[i] - EPS {
            let mut piece = water;
            piece.maximum[i] = low[i];
            result.push(piece);
            water.minimum[i] = low[i];
        }
        if water.maximum[i] > high[i] + EPS {
            let mut piece = water;
            piece.minimum[i] = high[i];
            result.push(piece);
            water.maximum[i] = high[i];
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn volume_connection(
        a: &FluidSpace,
        cell: FluidCell,
        b: &FluidSpace,
        other: FluidCell,
        direction: [i64; 3],
    ) -> bool {
        let axis = direction.iter().position(|&value| value != 0).unwrap();
        let positive = direction[axis] > 0;
        let right = b.wet_boxes(other.height());
        a.wet_boxes(cell.height()).iter().any(|left| {
            (if positive {
                (left.maximum[axis] - 1.0).abs()
            } else {
                left.minimum[axis].abs()
            }) < EPS
                && right.iter().any(|right| {
                    (if positive {
                        right.minimum[axis].abs()
                    } else {
                        (right.maximum[axis] - 1.0).abs()
                    }) < EPS
                        && (0..3).filter(|&i| i != axis).all(|i| {
                            left.maximum[i].min(right.maximum[i])
                                - left.minimum[i].max(right.minimum[i])
                                > EPS
                        })
                })
        })
    }

    #[test]
    fn face_connections_match_volume_subtraction_for_overlapping_partial_solids() {
        let mut random = 0x51a7_u64;
        let mut next = || {
            random = random.wrapping_mul(6364136223846793005).wrapping_add(1);
            (random >> 32) as usize
        };
        for _ in 0..160 {
            let spaces = std::array::from_fn::<_, 2, _>(|_| {
                let count = next() % 7;
                FluidSpace {
                    solids: (0..count)
                        .map(|_| {
                            let mut bounds = unit();
                            for axis in 0..3 {
                                let a = (next() % 13) as f64 * 0.125 - 0.25;
                                let b = (next() % 13) as f64 * 0.125 - 0.25;
                                bounds.minimum[axis] = a.min(b);
                                bounds.maximum[axis] = a.max(b) + 0.125;
                            }
                            bounds
                        })
                        .collect(),
                }
            });
            for direction in [
                [1, 0, 0],
                [-1, 0, 0],
                [0, 1, 0],
                [0, -1, 0],
                [0, 0, 1],
                [0, 0, -1],
            ] {
                for amount in [0, 1, 4, 8] {
                    for other_amount in [0, 1, 4, 8] {
                        let a = FluidCell {
                            amount,
                            ..Default::default()
                        };
                        let b = FluidCell {
                            amount: other_amount,
                            ..Default::default()
                        };
                        assert_eq!(
                            spaces[0].connected(a, &spaces[1], b, direction),
                            volume_connection(&spaces[0], a, &spaces[1], b, direction),
                            "{spaces:?}/{direction:?}/{amount}/{other_amount}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn dense_rail_face_connections_preserve_the_real_water_passages() {
        let piece = terra_core::rails::Piece::seed([0; 3], 0).unwrap();
        let air = FluidSpace { solids: Vec::new() };
        let full = FluidCell {
            amount: 8,
            ..Default::default()
        };
        for offset in piece.footprint().iter().take(3) {
            let rail = FluidSpace {
                solids: piece.cell_boxes(*offset).to_vec(),
            };
            for direction in [
                [1, 0, 0],
                [-1, 0, 0],
                [0, 1, 0],
                [0, -1, 0],
                [0, 0, 1],
                [0, 0, -1],
            ] {
                assert_eq!(
                    rail.connected(full, &air, full, direction),
                    volume_connection(&rail, full, &air, full, direction)
                );
            }
        }
    }

    #[test]
    fn stacked_pillars_water_fills_the_shaft_surround_without_hidden_interior_friezes() {
        use terra_core::castle::{self, connections};
        for facing in 0..24 {
            let center = [15, 15, 15];
            let up = terra_core::construction::stair_basis(facing)[1];
            let mut journal = EditJournal::default();
            for delta in [-1, 0, 1] {
                let cell = std::array::from_fn(|i| center[i] + i64::from(up[i]) * delta);
                journal.record(cell, castle::BATTLEMENT_PILLAR).unwrap();
                journal
                    .set_object_state(
                        cell,
                        Some(crate::edits::BlockObjectState {
                            facing,
                            ..Default::default()
                        }),
                    )
                    .unwrap();
            }
            let space = FluidSpace::at(&journal, center, |p| Some(journal.block(p).unwrap_or(0)));
            let expected = connections::profile_boxes(
                castle::BATTLEMENT_PILLAR,
                connections::STACK_ABOVE | connections::STACK_BELOW,
            )
            .iter()
            .map(|b| castle::rotated_box(*b, facing))
            .collect::<Vec<_>>();
            assert_eq!(space.solids, expected);
            let water = space.wet_boxes(1.0);
            let volume: f64 = water
                .iter()
                .map(|b| (0..3).map(|i| b.maximum[i] - b.minimum[i]).product::<f64>())
                .sum();
            assert!(
                (volume - 0.64).abs() < 1e-8,
                "hidden interior decoration displaced water: {facing}/{volume}"
            );
            assert!(water.iter().all(|b| {
                space
                    .solids
                    .iter()
                    .all(|solid| !solid.intersects(b.minimum, b.maximum))
            }));
        }
    }

    #[test]
    fn connected_battlements_water_matches_neighbor_derived_solids_in_all_poses() {
        use terra_core::castle::{self, connections};
        for facing in 0..24 {
            for mask in 1_u8..16 {
                let center = [15, 15, 15];
                let mut journal = EditJournal::default();
                journal.record(center, castle::BATTLEMENT_CORNER).unwrap();
                journal
                    .set_object_state(
                        center,
                        Some(crate::edits::BlockObjectState {
                            facing,
                            ..Default::default()
                        }),
                    )
                    .unwrap();
                let mut neighbors = [None; 6];
                for port in 0..4 {
                    if mask & (1 << port) == 0 {
                        continue;
                    }
                    let side = connections::port_side(port, facing);
                    let mut p = center;
                    p[side / 2] += if side.is_multiple_of(2) { -1 } else { 1 };
                    journal.record(p, castle::BATTLEMENT_STRAIGHT).unwrap();
                    journal
                        .set_object_state(
                            p,
                            Some(crate::edits::BlockObjectState {
                                facing,
                                ..Default::default()
                            }),
                        )
                        .unwrap();
                    neighbors[side] = Some((castle::BATTLEMENT_STRAIGHT, facing));
                }
                let space =
                    FluidSpace::at(&journal, center, |p| Some(journal.block(p).unwrap_or(0)));
                assert_eq!(
                    space.solids,
                    connections::placed_boxes(castle::BATTLEMENT_CORNER, facing, neighbors)
                );
                let water = space.wet_boxes(1.0);
                for wet in &water {
                    assert!(
                        space
                            .solids
                            .iter()
                            .all(|solid| !solid.intersects(wet.minimum, wet.maximum))
                    );
                }
                let volume: f64 = water
                    .iter()
                    .map(|b| (0..3).map(|i| b.maximum[i] - b.minimum[i]).product::<f64>())
                    .sum();
                assert!(
                    (0.1..0.9).contains(&volume),
                    "crenels must remain water-filled: {facing}/{mask}/{volume}"
                );
            }
        }
    }

    #[test]
    fn double_door_water_uses_the_joined_outer_frame_and_opposing_leaves() {
        use crate::edits::BlockObjectState;
        for material in [65, 66, 67, 139] {
            let mut journal = EditJournal::default();
            for x in 0..2 {
                for y in 0..2 {
                    let p = [x, y, 0];
                    journal.record(p, material).unwrap();
                    journal
                        .set_object_state(
                            p,
                            Some(BlockObjectState {
                                peer: Some([x, 1 - y, 0]),
                                secondary: y == 1,
                                ..Default::default()
                            }),
                        )
                        .unwrap();
                }
            }
            let full = FluidCell {
                amount: 8,
                source: true,
                flow: [0; 2],
            };
            for opened in [false, true] {
                for x in 0..2 {
                    for y in 0..2 {
                        let p = [x, y, 0];
                        let state = journal.object_state(p).unwrap();
                        journal
                            .set_object_state(
                                p,
                                Some(BlockObjectState {
                                    facing: if opened { 4 } else { 0 },
                                    ..state
                                }),
                            )
                            .unwrap();
                    }
                }
                for x in 0..2 {
                    let p = [x, 0, 0];
                    let space = FluidSpace::at(&journal, p, |q| journal.block(q).or(Some(0)));
                    assert_eq!(space.contains(full, [0.5; 3]), opened);
                    assert!(space.contains(full, [0.5, 0.5, 0.1]));
                    assert!(
                        space.contains(full, [if x == 0 { 0.98 } else { 0.02 }, 0.5, 0.4]),
                        "no stationary doubled middle post"
                    );
                }
            }
        }
    }

    #[test]
    fn every_fence_rotation_uses_only_its_actual_connected_rails() {
        use crate::edits::BlockObjectState;
        let p = [-1, -16, 15];
        for material in (90..=97).chain([138]) {
            assert!(partial(material));
            assert!(crate::water::can_hold_fluid(material));
            for facing in 0..4 {
                for mask in 0..16_u8 {
                    let mut journal = EditJournal::default();
                    journal.record(p, material).unwrap();
                    journal
                        .set_object_state(
                            p,
                            Some(BlockObjectState {
                                facing,
                                ..Default::default()
                            }),
                        )
                        .unwrap();
                    let block_at = |q: [i64; 3]| {
                        if q == p {
                            return Some(material);
                        }
                        let delta: [i64; 3] = std::array::from_fn(|axis| q[axis] - p[axis]);
                        let arm = [[-1, 0, 0], [1, 0, 0], [0, 0, -1], [0, 0, 1]]
                            .iter()
                            .position(|direction| *direction == delta);
                        Some(if arm.is_some_and(|arm| mask & (1 << arm) != 0) {
                            material
                        } else {
                            0
                        })
                    };
                    let space = FluidSpace::at(&journal, p, block_at);
                    assert_eq!(space.solids.len(), 3 + mask.count_ones() as usize * 2);
                    let water = space.wet_boxes(1.0);
                    assert!(!water.is_empty());
                    assert!(space.contains(
                        FluidCell {
                            amount: 8,
                            ..Default::default()
                        },
                        [0.15, 0.8, 0.15]
                    ));
                    assert!(!space.contains(
                        FluidCell {
                            amount: 8,
                            ..Default::default()
                        },
                        [0.5, 0.8, 0.5]
                    ));
                    #[cfg(feature = "bevy")]
                    {
                        let mut collision = Vec::new();
                        assert!(crate::interaction_shapes::visit_boxes_with(
                            &journal,
                            &terra_core::block_properties::BlockPropertiesCatalog::default(),
                            p,
                            block_at,
                            |mut bounds| {
                                for (axis, coordinate) in p.iter().enumerate() {
                                    bounds.minimum[axis] -= *coordinate as f64;
                                    bounds.maximum[axis] -= *coordinate as f64;
                                }
                                collision.push(bounds);
                            }
                        ));
                        assert_eq!(space.solids.len(), collision.len());
                        for (fluid, collision) in space.solids.iter().zip(collision) {
                            for axis in 0..3 {
                                assert!(
                                    (fluid.minimum[axis] - collision.minimum[axis]).abs() < 1e-12
                                );
                                assert!(
                                    (fluid.maximum[axis] - collision.maximum[axis]).abs() < 1e-12
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    #[cfg(feature = "bevy")]
    #[test]
    fn registered_authored_partial_shapes_share_the_existing_runtime_pose_and_never_flood_reservoir_internals()
     {
        use crate::edits::BlockObjectState;
        assert!(!partial(terra_core::plumbing::BASIN));
        assert!(!partial(terra_core::brewing::MATERIAL));
        assert!(!crate::water::can_hold_fluid(terra_core::plumbing::BASIN));
        assert!(!crate::water::can_hold_fluid(terra_core::brewing::MATERIAL));
        let p = [-1, -16, -1];
        let mut examined = 0;
        for material in 1..=terra_voxel::MaterialId::LAST {
            if !partial(material) || matches!(material, 100 | 148) {
                continue;
            }
            let mut state = BlockObjectState::default();
            if !state.valid_for(p, material) {
                state.peer = Some([p[0], p[1] + 1, p[2]]);
                if !state.valid_for(p, material) {
                    state.peer = Some([p[0], p[1], p[2] + 1]);
                }
            }
            if !state.valid_for(p, material) {
                continue;
            }
            let mut journal = EditJournal::default();
            journal.record(p, material).unwrap();
            journal.set_object_state(p, Some(state)).unwrap();
            if let Some(peer) = state.peer {
                let other = BlockObjectState {
                    peer: Some(p),
                    secondary: true,
                    ..state
                };
                journal.record(peer, material).unwrap();
                journal.set_object_state(peer, Some(other)).unwrap();
            }
            let block_at = |q| Some(journal.block(q).unwrap_or(0));
            let space = FluidSpace::at(&journal, p, block_at);
            let mut actual = Vec::new();
            assert!(
                crate::interaction_shapes::visit_boxes_with(
                    &journal,
                    &terra_core::block_properties::BlockPropertiesCatalog::default(),
                    p,
                    block_at,
                    |bounds| actual.push(bounds)
                ),
                "material={material}"
            );
            for x in 0..8 {
                for y in 0..8 {
                    for z in 0..8 {
                        let point = [x, y, z].map(|value| (f64::from(value) + 0.5) / 8.0);
                        let global: [f64; 3] =
                            std::array::from_fn(|axis| p[axis] as f64 + point[axis]);
                        let solid = actual.iter().any(|bounds| {
                            (0..3).all(|axis| {
                                global[axis] >= bounds.minimum[axis]
                                    && global[axis] < bounds.maximum[axis]
                            })
                        });
                        assert_eq!(
                            space.contains(
                                FluidCell {
                                    amount: 8,
                                    ..Default::default()
                                },
                                point
                            ),
                            !solid,
                            "material={material}, point={point:?}"
                        );
                    }
                }
            }
            examined += 1;
        }
        assert!(
            examined >= 50,
            "exercise the existing authored registry, not a handpicked new whitelist"
        );
    }

    #[test]
    fn extended_wheels_clip_the_same_collision_boxes_into_adjacent_negative_cells() {
        let owner = [-1, 3, -1];
        let neighbor = [-1, 3, 0];
        let mut journal = EditJournal::default();
        journal.record(owner, 147).unwrap();
        journal
            .set_object_state(owner, Some(Default::default()))
            .unwrap();
        let loaded = |p| Some(if p == owner { 147 } else { 0 });
        let space = FluidSpace::at(&journal, neighbor, loaded);
        assert!(!space.solids.is_empty());
        assert!(
            space
                .solids
                .iter()
                .all(|b| (0..3).all(|i| b.minimum[i] >= 0.0
                    && b.maximum[i] <= 1.0
                    && b.maximum[i] > b.minimum[i]))
        );
        let wet = space.wet_boxes(1.0);
        assert!(!wet.is_empty());
        for water in &wet {
            for solid in &space.solids {
                assert!((0..3).any(|i| water.maximum[i] <= solid.minimum[i] + EPS
                    || water.minimum[i] >= solid.maximum[i] - EPS));
            }
        }
        let unloaded = FluidSpace::at(&journal, neighbor, |p| (p != owner).then_some(0));
        assert!(unloaded.solids.is_empty());
        journal.record(owner, 0).unwrap();
        assert!(
            FluidSpace::at(&journal, neighbor, |_| Some(0))
                .solids
                .is_empty()
        );
    }
    #[test]
    fn water_uses_the_rotated_thin_crank_geometry_instead_of_a_solid_cell() {
        use crate::edits::BlockObjectState;
        let p = [-2, 7, -3];
        for (facing, distance) in
            (0..6).flat_map(|facing| [1, 2].map(|distance| (facing, distance)))
        {
            let state = BlockObjectState {
                facing,
                ..Default::default()
            };
            let anchor = std::array::from_fn(|axis| {
                p[axis] - terra_core::kinetic::axis(facing)[axis] * distance
            });
            let mut journal = EditJournal::default();
            journal.record(p, 100).unwrap();
            journal
                .set_object_state(
                    p,
                    Some(BlockObjectState {
                        peer: Some(anchor),
                        ..state
                    }),
                )
                .unwrap();
            let space = FluidSpace::at(&journal, p, |cell| Some(if cell == p { 100 } else { 0 }));
            assert_eq!(space.solids.len(), 4);
            #[cfg(feature = "bevy")]
            {
                let mut collision = Vec::new();
                assert!(crate::interaction_shapes::visit_boxes_with(
                    &journal,
                    &terra_core::block_properties::BlockPropertiesCatalog::default(),
                    p,
                    |q| Some(if q == p { 100 } else { 0 }),
                    |mut shape| {
                        for (axis, value) in p.iter().enumerate() {
                            shape.minimum[axis] -= *value as f64;
                            shape.maximum[axis] -= *value as f64;
                        }
                        collision.push(shape);
                    }
                ));
                assert_eq!(space.solids.len(), collision.len());
                for (water, collision) in space.solids.iter().zip(&collision) {
                    assert_eq!(water.hinged, collision.hinged);
                    for axis in 0..3 {
                        assert!(
                            (water.minimum[axis] - collision.minimum[axis]).abs() < 1e-12
                                && (water.maximum[axis] - collision.maximum[axis]).abs() < 1e-12,
                            "pure water geometry must match authoritative runtime collision: {water:?} != {collision:?}"
                        );
                    }
                }
            }
            let water = FluidCell {
                amount: 8,
                source: false,
                flow: [0; 2],
            };
            assert!(space.contains(water, [0.5, 0.99, 0.5]));
            let plate = space.solids[0];
            let center =
                std::array::from_fn(|axis| (plate.minimum[axis] + plate.maximum[axis]) * 0.5);
            assert!(!space.contains(water, center));
            let empty = FluidSpace { solids: Vec::new() };
            for direction in [
                [1, 0, 0],
                [-1, 0, 0],
                [0, 1, 0],
                [0, -1, 0],
                [0, 0, 1],
                [0, 0, -1],
            ] {
                assert!(space.connected(water, &empty, water, direction));
            }
        }
    }

    #[test]
    fn every_construction_rotation_excludes_exact_solid_volume() {
        for (id, facings) in [(82, 58), (74, 24)] {
            for facing in 0..facings {
                let space = FluidSpace {
                    solids: construction::placed_boxes(id, facing, [None; 6]),
                };
                for amount in 1..=8 {
                    let cell = FluidCell {
                        amount,
                        ..Default::default()
                    };
                    let water = space.wet_boxes(cell.height());
                    let volume: f64 = water
                        .iter()
                        .map(|b| (0..3).map(|i| b.maximum[i] - b.minimum[i]).product::<f64>())
                        .sum();
                    let mut count = 0;
                    for x in 0..24 {
                        for y in 0..24 {
                            for z in 0..24 {
                                let point = [x, y, z].map(|v| (f64::from(v) + 0.5) / 24.0);
                                let expected = point[1] < cell.height()
                                    && !space.solids.iter().any(|b| {
                                        (0..3).all(|i| {
                                            point[i] >= b.minimum[i] && point[i] < b.maximum[i]
                                        })
                                    });
                                assert_eq!(space.contains(cell, point), expected);
                                let containing = water
                                    .iter()
                                    .filter(|b| {
                                        (0..3).all(|i| {
                                            point[i] >= b.minimum[i] && point[i] < b.maximum[i]
                                        })
                                    })
                                    .count();
                                assert_eq!(
                                    containing,
                                    usize::from(expected),
                                    "id={id} facing={facing} amount={amount} point={point:?}"
                                );
                                count += containing;
                            }
                        }
                    }
                    assert!((volume - count as f64 / 13824.0).abs() < 1e-8);
                    assert_eq!(space.clamp(cell).amount == 0, volume == 0.0);
                }
            }
        }
    }

    #[test]
    fn gearbox_water_uses_rotated_authored_housings_instead_of_solid_cubes() {
        use crate::edits::BlockObjectState;
        for material in [154, 155] {
            assert!(partial(material));
            for facing in 0..4 {
                let position = [-1, 7, -17];
                let mut journal = EditJournal::default();
                journal.record(position, material).unwrap();
                journal
                    .set_object_state(
                        position,
                        Some(BlockObjectState {
                            facing,
                            ..Default::default()
                        }),
                    )
                    .unwrap();
                let space = FluidSpace::at(&journal, position, |p| journal.block(p));
                let authored: Vec<_> = terra_core::block_shapes::builtin_boxes(material)
                    .iter()
                    .map(|bounds| bounds.rotated_axis(facing))
                    .collect();
                assert!(!authored.is_empty());
                assert_eq!(space.solids, authored);
                let cell = FluidCell {
                    amount: 8,
                    source: true,
                    flow: [0; 2],
                };
                assert_eq!(space.clamp(cell), cell);
                assert!(space.contains(cell, [0.03, 0.5, 0.03]));
                assert!(!space.contains(cell, [0.5; 3]));
                let volume: f64 = space
                    .wet_boxes(1.0)
                    .iter()
                    .map(|b| (0..3).map(|i| b.maximum[i] - b.minimum[i]).product::<f64>())
                    .sum();
                assert!(volume > 0.0 && volume < 1.0);
            }
        }
    }

    #[test]
    fn only_matching_free_faces_transmit_water() {
        let full = FluidCell {
            amount: 8,
            ..Default::default()
        };
        let open = FluidSpace { solids: vec![] };
        let lower = FluidSpace {
            solids: vec![construction::slab_bounds(0)],
        };
        let upper = FluidSpace {
            solids: vec![construction::slab_bounds(4)],
        };
        let west = FluidSpace {
            solids: vec![construction::slab_bounds(5)],
        };
        assert!(open.connected(full, &lower, full, [1, 0, 0]));
        assert!(!open.connected(full, &west, full, [1, 0, 0]));
        assert!(!lower.connected(full, &upper, full, [1, 0, 0]));
        assert!(!lower.connected(full, &open, full, [0, -1, 0]));
        assert!(upper.connected(full, &open, full, [0, -1, 0]));
        assert_eq!(
            lower.clamp(FluidCell { amount: 4, ..full }),
            FluidCell::default()
        );
        assert_eq!(upper.clamp(full).amount, 4);
    }
}
