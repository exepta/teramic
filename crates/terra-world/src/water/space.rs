//! Liquid occupies the complement of the same slab/stair boxes used by collision.
use super::FluidCell;
use crate::edits::EditJournal;
use terra_core::{block_shapes::ShapeBox, construction};

const EPS: f64 = 1e-8;

pub(crate) fn partial(material: u16) -> bool {
    construction::is_slab(material)
        || construction::is_stair(material)
        || material == terra_core::crusher::CRANK_MATERIAL
}

#[derive(Clone, Debug)]
pub(crate) struct FluidSpace {
    pub solids: Vec<ShapeBox>,
}

impl FluidSpace {
    pub fn at(
        journal: &EditJournal,
        p: [i64; 3],
        mut block_at: impl FnMut([i64; 3]) -> Option<u16>,
    ) -> Self {
        let material = block_at(p).unwrap_or(u16::MAX);
        if material == terra_core::crusher::CRANK_MATERIAL {
            let facing = journal.object_state(p).unwrap_or_default().facing;
            let solids = terra_core::block_shapes::builtin_boxes(material)
                .iter()
                .map(|bounds| bounds.rotated_y(facing))
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
        if self.solids.is_empty() && other.solids.is_empty() {
            return cell.amount > 0
                && other_cell.amount > 0
                && (axis != 1
                    || if high {
                        cell.amount == 8
                    } else {
                        other_cell.amount == 8
                    });
        }
        let other_water = other.wet_boxes(other_cell.height());
        self.wet_boxes(cell.height()).iter().any(|a| {
            (if high {
                (a.maximum[axis] - 1.0).abs()
            } else {
                a.minimum[axis].abs()
            }) < EPS
                && other_water.iter().any(|b| {
                    (if high {
                        b.minimum[axis].abs()
                    } else {
                        (b.maximum[axis] - 1.0).abs()
                    }) < EPS
                        && (0..3).filter(|&i| i != axis).all(|i| {
                            a.maximum[i].min(b.maximum[i]) - a.minimum[i].max(b.minimum[i]) > EPS
                        })
                })
        })
    }
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
    #[test]
    fn water_uses_the_rotated_thin_crank_geometry_instead_of_a_solid_cell() {
        use crate::edits::BlockObjectState;
        let p = [-2, 7, -3];
        for facing in 0..4 {
            let state = BlockObjectState {
                facing,
                ..Default::default()
            };
            let anchor = std::array::from_fn(|axis| p[axis] - state.right()[axis] * 2);
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
