//! Bounded, derived window joinery. Existing per-cell states remain the save format.
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use terra_core::block_shapes::ShapeBox;

const COMPONENT_LIMIT: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Group {
    pub anchor: [i64; 3],
    pub material: u16,
    pub facing: u8,
    pub width: u8,
    pub height: u8,
}
impl Group {
    pub fn right(self) -> [i64; 3] {
        [[1, 0, 0], [0, 0, -1], [-1, 0, 0], [0, 0, 1]][usize::from(self.facing % 4)]
    }
    pub fn cells(self) -> impl Iterator<Item = [i64; 3]> {
        (0..self.height).flat_map(move |y| {
            (0..self.width).map(move |x| {
                let right = self.right();
                std::array::from_fn(|axis| {
                    self.anchor[axis]
                        + right[axis] * i64::from(x)
                        + if axis == 1 { i64::from(y) } else { 0 }
                })
            })
        })
    }
    pub fn with_open(self, open: bool) -> Self {
        Self {
            facing: self.facing % 4 + u8::from(open) * 4,
            ..self
        }
    }
    pub fn hinge(self, leaf: u8) -> [f64; 3] {
        [
            if leaf == 0 {
                0.15
            } else {
                f64::from(self.width) - 0.15
            },
            0.0,
            0.5,
        ]
    }
    pub fn nearest_cell(self, point: [f64; 3]) -> [i64; 3] {
        let distance = |p: [i64; 3]| {
            (0..3)
                .map(|a| (point[a] - p[a] as f64 - 0.5).powi(2))
                .sum::<f64>()
        };
        self.cells()
            .min_by(|a, b| distance(*a).total_cmp(&distance(*b)))
            .unwrap_or(self.anchor)
    }
    pub fn leaf_angle(leaf: u8) -> f64 {
        if leaf == 0 {
            -std::f64::consts::FRAC_PI_2
        } else {
            std::f64::consts::FRAC_PI_2
        }
    }
}

/// A connected wall larger than 2×2 is tiled deterministically. Components above
/// the fixed budget retain independent windows instead of unbounded history scans.
pub fn group(
    position: [i64; 3],
    material: u16,
    facing: u8,
    mut lookup: impl FnMut([i64; 3]) -> Option<(u16, u8)>,
) -> Group {
    let single = Group {
        anchor: position,
        material,
        facing,
        width: 1,
        height: 1,
    };
    if !super::is_window(material) || facing >= 8 {
        return single;
    }
    let right = single.right();
    let coordinate = |p: [i64; 3]| (p[1], p[0] * right[0] + p[2] * right[2]);
    let mut members = BTreeMap::from([(coordinate(position), position)]);
    let mut pending = VecDeque::from([position]);
    let mut tested = BTreeSet::from([position]);
    while let Some(p) = pending.pop_front() {
        for direction in [right, right.map(|v| -v), [0, 1, 0], [0, -1, 0]] {
            let mut q = p;
            let mut valid = true;
            for axis in 0..3 {
                if let Some(value) = p[axis].checked_add(direction[axis]) {
                    q[axis] = value;
                } else {
                    valid = false;
                }
            }
            if !valid || !tested.insert(q) || lookup(q) != Some((material, facing)) {
                continue;
            }
            if members.len() == COMPONENT_LIMIT {
                return single;
            }
            members.insert(coordinate(q), q);
            pending.push_back(q);
        }
    }
    let target = coordinate(position);
    while let Some((&(y, u), &anchor)) = members.first_key_value() {
        let square = members.contains_key(&(y, u + 1))
            && members.contains_key(&(y + 1, u))
            && members.contains_key(&(y + 1, u + 1));
        let (width, height) = if square {
            (2, 2)
        } else if members.contains_key(&(y, u + 1)) {
            (2, 1)
        } else if members.contains_key(&(y + 1, u)) {
            (1, 2)
        } else {
            (1, 1)
        };
        let result = Group {
            anchor,
            material,
            facing,
            width,
            height,
        };
        for dy in 0..height {
            for dx in 0..width {
                let key = (y + i64::from(dy), u + i64::from(dx));
                if key == target {
                    return result;
                }
                members.remove(&key);
            }
        }
    }
    single
}

#[derive(Clone, Copy, Debug)]
pub struct Piece {
    pub bounds: ShapeBox,
    /// Wood, dark grain, carved trim, metal, glass; all reuse existing materials.
    pub surface: u8,
    pub leaf: Option<u8>,
    pub solid: bool,
}

/// Shared local-space joinery for render meshes and actual physical/picking boxes.
pub fn pieces(width: u8, height: u8) -> Vec<Piece> {
    let width = width.clamp(1, 2);
    let height = height.clamp(1, 2);
    let w = f64::from(width);
    let h = f64::from(height);
    let mut out = Vec::with_capacity(64);
    let mut add = |min: [f64; 3], max: [f64; 3], surface, leaf: Option<u8>, solid| {
        out.push(Piece {
            bounds: ShapeBox {
                minimum: min,
                maximum: max,
                hinged: leaf.is_some(),
            },
            surface,
            leaf,
            solid,
        });
    };
    for x in [0.04, w - 0.15] {
        add([x, 0.0, 0.34], [x + 0.11, h, 0.66], 0, None, true);
        for y in [0.015, h - 0.17] {
            add(
                [x - 0.015, y, 0.32],
                [x + 0.125, y + 0.15, 0.68],
                2,
                None,
                false,
            );
            add(
                [x + 0.03, y + 0.05, 0.685],
                [x + 0.07, y + 0.095, 0.70],
                1,
                None,
                false,
            );
        }
        for i in 0..height * 4 {
            let y = 0.22 + f64::from(i) * 0.23;
            if y < h - 0.18 {
                for z in [0.332, 0.662] {
                    add(
                        [x + 0.02, y, z],
                        [x + 0.035, y + 0.12, z + 0.006],
                        1,
                        None,
                        false,
                    );
                }
            }
        }
    }
    add([0.15, 0.0, 0.34], [w - 0.15, 0.075, 0.66], 0, None, true);
    add([0.15, h - 0.12, 0.34], [w - 0.15, h, 0.66], 0, None, true);
    add(
        [0.015, h - 0.10, 0.30],
        [w - 0.015, h - 0.025, 0.70],
        2,
        None,
        false,
    );
    add([0.01, 0.02, 0.275], [w - 0.01, 0.09, 0.725], 2, None, false);
    for leaf in 0..width.clamp(1, 2) {
        let left = if leaf == 0 { 0.15 } else { w / 2.0 + 0.025 };
        let right = if width == 2 && leaf == 0 {
            w / 2.0 - 0.025
        } else {
            w - 0.15
        };
        let bottom = 0.085;
        let top = h - 0.135;
        for x in [left, right - 0.055] {
            add(
                [x, bottom, 0.455],
                [x + 0.055, top, 0.545],
                0,
                Some(leaf),
                true,
            );
        }
        for y in [bottom, top - 0.055] {
            add(
                [left + 0.055, y, 0.455],
                [right - 0.055, y + 0.055, 0.545],
                2,
                Some(leaf),
                true,
            );
        }
        if height == 2 {
            add(
                [left + 0.055, h / 2.0 - 0.025, 0.455],
                [right - 0.055, h / 2.0 + 0.025, 0.545],
                2,
                Some(leaf),
                true,
            );
        }
        for pane in 0..height.clamp(1, 2) {
            let y0 = if pane == 0 {
                bottom + 0.055
            } else {
                h / 2.0 + 0.025
            };
            let y1 = if height == 2 && pane == 0 {
                h / 2.0 - 0.025
            } else {
                top - 0.055
            };
            add(
                [left + 0.055, y0, 0.488],
                [right - 0.055, y1, 0.512],
                4,
                Some(leaf),
                true,
            );
        }
        let hinge = if leaf == 0 { left } else { right };
        for y in [h * 0.24, h * 0.76] {
            add(
                [hinge - 0.015, y, 0.555],
                [hinge + 0.015, y + 0.13, 0.60],
                3,
                None,
                false,
            );
            let a = if leaf == 0 { left + 0.01 } else { right - 0.18 };
            add(
                [a, y + 0.025, 0.55],
                [a + 0.17, y + 0.10, 0.59],
                3,
                Some(leaf),
                false,
            );
        }
        let handle = if leaf == 0 {
            right - 0.09
        } else {
            left + 0.035
        };
        add(
            [handle, h * 0.45, 0.55],
            [handle + 0.035, h * 0.45 + 0.12, 0.585],
            3,
            Some(leaf),
            false,
        );
        add(
            [handle - 0.01, h * 0.45 + 0.04, 0.58],
            [handle + 0.05, h * 0.45 + 0.065, 0.605],
            3,
            Some(leaf),
            false,
        );
    }
    out
}

pub fn world_box(piece: Piece, group: Group, openness: f64) -> ShapeBox {
    let mut bounds = ShapeBox {
        minimum: [f64::INFINITY; 3],
        maximum: [f64::NEG_INFINITY; 3],
        hinged: false,
    };
    for corner in 0..8 {
        let mut p: [f64; 3] = std::array::from_fn(|a| {
            if corner & (1 << a) == 0 {
                piece.bounds.minimum[a]
            } else {
                piece.bounds.maximum[a]
            }
        });
        if let Some(leaf) = piece.leaf {
            let hinge = group.hinge(leaf);
            let (s, c) = (Group::leaf_angle(leaf) * openness.clamp(0.0, 1.0)).sin_cos();
            let [x, _, z] = std::array::from_fn(|a| p[a] - hinge[a]);
            p[0] = hinge[0] + x * c + z * s;
            p[2] = hinge[2] - x * s + z * c;
        }
        let [x, y, z] = [p[0] - 0.5, p[1], p[2] - 0.5];
        let p = match group.facing % 4 {
            0 => [x, y, z],
            1 => [z, y, -x],
            2 => [-x, y, -z],
            _ => [-z, y, x],
        };
        for (a, coordinate) in p.into_iter().enumerate() {
            let value = coordinate + group.anchor[a] as f64 + if a == 1 { 0.0 } else { 0.5 };
            bounds.minimum[a] = bounds.minimum[a].min(value);
            bounds.maximum[a] = bounds.maximum[a].max(value);
        }
    }
    bounds
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn connected_windows_tile_consistently_in_all_yaws_negative_coordinates_and_load_orders() {
        for facing in 0..8 {
            for (width, height) in [(2, 1), (1, 2), (2, 2)] {
                let expected = Group {
                    anchor: [-16, 15, -16],
                    material: 68,
                    facing,
                    width,
                    height,
                };
                let cells: BTreeSet<_> = expected.cells().collect();
                for &p in cells.iter().rev() {
                    let actual = group(p, 68, facing, |q| {
                        cells.contains(&q).then_some((68, facing))
                    });
                    assert_eq!(actual, expected);
                }
            }
        }
        let single = group([0, 0, 0], 68, 0, |q| (q == [1, 0, 0]).then_some((69, 0)));
        assert_eq!((single.width, single.height), (1, 1));
        let single = group([0, 0, 0], 68, 0, |q| (q == [1, 0, 0]).then_some((68, 4)));
        assert_eq!((single.width, single.height), (1, 1));
        let mut calls = 0;
        let single = group([0, 0, 0], 68, 0, |_| {
            calls += 1;
            Some((68, 0))
        });
        assert_eq!((single.width, single.height), (1, 1));
        assert!(calls <= COMPONENT_LIMIT * 4);
    }
    #[test]
    fn joined_window_has_only_outer_frame_and_opposing_leaves_clear_the_aperture() {
        for (width, height) in [(2, 1), (1, 2), (2, 2)] {
            let group = Group {
                anchor: [0; 3],
                material: 68,
                facing: 0,
                width,
                height,
            };
            let middle = [f64::from(width) * 0.37, f64::from(height) / 2.0 + 0.16, 0.5];
            let tiny = middle.map(|v| v + 0.005);
            let pieces = pieces(width, height);
            assert!(pieces.iter().any(|p| p.leaf.is_some() && world_box(*p,group,0.0).intersects(middle,tiny)));
            assert!(
                pieces
                    .iter()
                    .filter(|p| p.solid)
                    .all(|p| !world_box(*p, group, 1.0).intersects(middle, tiny))
            );
            assert!(pieces.len() < 80);
            if width == 2 {
                let left = pieces.iter().find(|p| p.leaf == Some(0)).unwrap();
                let right = pieces.iter().find(|p| p.leaf == Some(1)).unwrap();
                assert!(world_box(*left, group, 1.0).maximum[0] < 0.25);
                assert!(world_box(*right, group, 1.0).minimum[0] > 1.75);
            }
        }
    }
    #[test]
    fn connected_window_states_roundtrip_existing_section_deltas_and_split_after_removal() {
        use crate::edits::{BlockObjectState, EditJournal};
        for facing in 0..8 {
            let expected = Group {
                anchor: [-16, 15, -16],
                material: 70,
                facing,
                width: 2,
                height: 2,
            };
            let mut journal = EditJournal::default();
            for p in expected.cells() {
                journal.record(p, 70).unwrap();
                journal
                    .set_object_state(
                        p,
                        Some(BlockObjectState {
                            facing,
                            ..Default::default()
                        }),
                    )
                    .unwrap();
            }
            let owners: BTreeSet<_> = expected
                .cells()
                .map(|p| terra_voxel::global_to_section(p).0)
                .collect();
            let mut restored = EditJournal::default();
            for owner in owners.iter().rev() {
                restored
                    .import_replica_delta(journal.section_delta(*owner))
                    .unwrap();
            }
            let lookup = |p| Some((restored.block(p)?, restored.object_state(p)?.facing));
            for p in expected.cells() {
                assert_eq!(group(p, 70, facing, lookup), expected);
            }
            let removed = expected.cells().last().unwrap();
            restored.record(removed, 0).unwrap();
            restored.set_object_state(removed, None).unwrap();
            for p in expected.cells().filter(|p| *p != removed) {
                let actual = group(p, 70, facing, |q| {
                    Some((restored.block(q)?, restored.object_state(q)?.facing))
                });
                assert!(!actual.cells().any(|q| q == removed));
                assert_eq!(actual.facing, facing);
            }
        }
    }
}
