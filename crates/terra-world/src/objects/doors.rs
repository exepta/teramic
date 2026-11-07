//! Derived double doors; existing reciprocal vertical states remain the save format.
use super::windows::{self, Piece};
use crate::edits::BlockObjectState;
use terra_core::block_shapes::ShapeBox;

const ROW_LIMIT: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Group {
    pub anchor: [i64; 3],
    pub material: u16,
    /// Base yaw; each leaf retains its own historic open bit.
    pub facing: u8,
    pub width: u8,
    pub opened: [bool; 2],
}

impl Group {
    pub fn geometry(self) -> windows::Group {
        windows::Group {
            anchor: self.anchor,
            material: self.material,
            facing: self.facing,
            width: self.width,
            height: 2,
        }
    }
    pub fn cells(self) -> impl Iterator<Item = [i64; 3]> {
        self.geometry().cells()
    }
    pub fn hinge(self, leaf: u8) -> [f64; 3] {
        self.geometry().hinge(leaf)
    }
    pub fn nearest_cell(self, point: [f64; 3]) -> [i64; 3] {
        self.geometry().nearest_cell(point)
    }
    pub fn with_open(self, open: bool) -> Self {
        Self {
            opened: [open; 2],
            ..self
        }
    }
    pub fn world_box(self, piece: Piece) -> ShapeBox {
        let open = piece
            .leaf
            .is_some_and(|leaf| self.opened[usize::from(leaf)]);
        windows::world_box(piece, self.geometry(), f64::from(u8::from(open)))
    }
}

fn shifted(p: [i64; 3], d: [i64; 3]) -> Option<[i64; 3]> {
    Some([
        p[0].checked_add(d[0])?,
        p[1].checked_add(d[1])?,
        p[2].checked_add(d[2])?,
    ])
}

/// Only complete reciprocal two-cell doors participate. Unknown neighbors never
/// manufacture a partner, and malformed saves retain their legacy single model.
pub fn group(
    position: [i64; 3],
    material: u16,
    state: BlockObjectState,
    mut lookup: impl FnMut([i64; 3]) -> Option<(u16, BlockObjectState)>,
) -> Option<Group> {
    if !super::is_door(material) || state.facing >= 8 {
        return None;
    }
    let primary = if state.secondary {
        state.peer?
    } else {
        position
    };
    let mut read = |p| {
        if p == position {
            Some((material, state))
        } else {
            lookup(p)
        }
    };
    let mut door = |p| -> Option<BlockObjectState> {
        let (id, bottom) = read(p)?;
        let top = shifted(p, [0, 1, 0])?;
        let (top_id, upper) = read(top)?;
        (id == material
            && top_id == material
            && !bottom.secondary
            && bottom.facing < 8
            && bottom.facing % 4 == state.facing % 4
            && bottom.peer == Some(top)
            && upper.secondary
            && upper.peer == Some(p)
            && upper.facing == bottom.facing)
            .then_some(bottom)
    };
    let bottom = door(primary)?;
    let single = Group {
        anchor: primary,
        material,
        facing: bottom.facing % 4,
        width: 1,
        opened: [bottom.facing >= 4, false],
    };
    let right = single.geometry().right();
    let mut start = primary;
    for count in 0..=ROW_LIMIT {
        let Some(left) = shifted(start, right.map(|v| -v)) else {
            break;
        };
        if door(left).is_none() {
            break;
        }
        if count == ROW_LIMIT {
            return Some(single);
        }
        start = left;
    }
    let mut row = Vec::new();
    let mut cursor = start;
    while let Some(item) = door(cursor) {
        if row.len() == ROW_LIMIT {
            return Some(single);
        }
        row.push((cursor, item));
        let Some(next) = shifted(cursor, right) else {
            break;
        };
        cursor = next;
    }
    let index = row.iter().position(|(p, _)| *p == primary)?;
    let first = index / 2 * 2;
    let (anchor, left) = row[first];
    let other = row.get(first + 1).map(|(_, state)| state.facing >= 4);
    Some(Group {
        anchor,
        material,
        facing: left.facing % 4,
        width: if other.is_some() { 2 } else { 1 },
        opened: [left.facing >= 4, other.unwrap_or(false)],
    })
}

/// A single continuous surround and two plank leaves with outer hinges. All
/// surfaces reuse the corresponding authored door's wood/grain/iron palette.
pub fn pieces() -> Vec<Piece> {
    let mut out = Vec::new();
    let mut add =
        |minimum: [f64; 3], maximum: [f64; 3], surface: u8, leaf: Option<u8>, solid: bool| {
            out.push(Piece {
                bounds: ShapeBox {
                    minimum,
                    maximum,
                    hinged: leaf.is_some(),
                },
                surface,
                leaf,
                solid,
            })
        };
    for x in [0.04, 1.85] {
        add([x, 0.0, 0.34], [x + 0.11, 2.0, 0.66], 0, None, true);
        for y in [0.025, 1.82] {
            add(
                [x - 0.015, y, 0.32],
                [x + 0.125, y + 0.14, 0.68],
                2,
                None,
                false,
            );
        }
    }
    add([0.15, 1.94, 0.34], [1.85, 2.0, 0.66], 0, None, true);
    add([0.025, 1.91, 0.32], [1.975, 1.975, 0.68], 2, None, false);
    for leaf in 0..2 {
        let (left, right) = if leaf == 0 {
            (0.15, 0.995)
        } else {
            (1.005, 1.85)
        };
        add(
            [left, 0.015, 0.47],
            [right, 1.94, 0.53],
            0,
            Some(leaf),
            true,
        );
        for plank in 0..5 {
            let x = left + (right - left) * f64::from(plank) / 5.0;
            for z in [0.458, 0.53] {
                add(
                    [x + 0.002, 0.025, z],
                    [x + (right - left) / 5.0 - 0.002, 1.93, z + 0.012],
                    if plank % 2 == 0 { 0 } else { 1 },
                    Some(leaf),
                    false,
                );
            }
        }
        for y in [0.16, 1.0, 1.76] {
            for z in [0.442, 0.542] {
                add(
                    [left + 0.025, y, z],
                    [right - 0.025, y + 0.075, z + 0.016],
                    2,
                    Some(leaf),
                    false,
                );
            }
        }
        let hinge = if leaf == 0 { left } else { right - 0.04 };
        for y in [0.28, 1.52] {
            add(
                [hinge, y, 0.438],
                [hinge + 0.04, y + 0.2, 0.562],
                3,
                Some(leaf),
                false,
            );
        }
        let handle = if leaf == 0 {
            right - 0.095
        } else {
            left + 0.045
        };
        for z in [0.41, 0.558] {
            add(
                [handle, 0.88, z],
                [handle + 0.05, 1.05, z + 0.032],
                3,
                Some(leaf),
                false,
            );
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    fn row(material: u16, facing: u8, count: u8) -> BTreeMap<[i64; 3], (u16, BlockObjectState)> {
        let mut map = BTreeMap::new();
        let group = windows::Group {
            anchor: [-16, 15, -16],
            material,
            facing,
            width: count,
            height: 2,
        };
        for (index, p) in group.cells().enumerate() {
            let top = index / usize::from(count) == 1;
            map.insert(
                p,
                (
                    material,
                    BlockObjectState {
                        facing,
                        secondary: top,
                        peer: Some(shifted(p, [0, if top { -1 } else { 1 }, 0]).unwrap()),
                        ..Default::default()
                    },
                ),
            );
        }
        map
    }
    #[test]
    fn adjacent_doors_pair_in_all_materials_axes_and_open_states_with_stable_three_leaf_rows() {
        for material in [65, 66, 67, 139] {
            for facing in 0..8 {
                let map = row(material, facing, 3);
                let expected = windows::Group {
                    anchor: [-16, 15, -16],
                    material,
                    facing,
                    width: 3,
                    height: 2,
                };
                let right = expected.right();
                for p in expected.cells().collect::<Vec<_>>().into_iter().rev() {
                    let actual = group(p, material, map[&p].1, |q| map.get(&q).copied()).unwrap();
                    let last = shifted(shifted(expected.anchor, right).unwrap(), right).unwrap();
                    if p[0] == last[0] && p[2] == last[2] {
                        assert_eq!((actual.anchor, actual.width), (last, 1));
                    } else {
                        assert_eq!((actual.anchor, actual.width), (expected.anchor, 2));
                        assert_eq!(actual.cells().count(), 4);
                    }
                }
                let mut changed = map.clone();
                let peer = shifted(expected.anchor, right).unwrap();
                changed.get_mut(&peer).unwrap().0 = if material == 66 { 65 } else { 66 };
                assert_eq!(
                    group(expected.anchor, material, map[&expected.anchor].1, |q| {
                        changed.get(&q).copied()
                    })
                    .unwrap()
                    .width,
                    1
                );
            }
        }
    }
    #[test]
    fn pairing_preserves_mixed_manual_poses_and_bounds_long_rows() {
        let mut map = row(65, 0, 3);
        for p in [[-15, 15, -16], [-15, 16, -16]] {
            map.get_mut(&p).unwrap().1.facing = 4;
        }
        let anchor = [-16, 15, -16];
        let pair = group(anchor, 65, map[&anchor].1, |p| map.get(&p).copied()).unwrap();
        assert_eq!(pair.width, 2);
        assert_eq!(pair.opened, [false, true]);
        let long = row(65, 0, 65);
        for x in [0, 32, 64] {
            let p = [-16 + x, 15, -16];
            assert_eq!(
                group(p, 65, long[&p].1, |q| long.get(&q).copied())
                    .unwrap()
                    .width,
                1
            );
        }
        assert!(group([i64::MAX; 3], 65, BlockObjectState::default(), |_| None).is_none());
    }

    #[test]
    fn opposite_hinges_clear_the_center_without_a_stationary_center_post() {
        let group = Group {
            anchor: [0; 3],
            material: 65,
            facing: 0,
            width: 2,
            opened: [false; 2],
        };
        let panels: Vec<_> = pieces()
            .into_iter()
            .filter(|p| p.solid && p.leaf.is_some())
            .collect();
        assert_eq!(panels.len(), 2);
        for p in &panels {
            assert!(
                group
                    .world_box(*p)
                    .intersects([0.2, 0.1, 0.49], [1.8, 1.9, 0.51])
            );
        }
        let opened = group.with_open(true);
        for p in pieces().into_iter().filter(|p| p.solid) {
            assert!(
                !opened
                    .world_box(p)
                    .intersects([0.25, 0.1, 0.3], [1.75, 1.9, 0.7])
            );
        }
        let a = opened.world_box(panels[0]);
        let b = opened.world_box(panels[1]);
        assert!(a.maximum[0] < 0.2 && b.minimum[0] > 1.8);
        assert!(a.maximum[2] > 1.2 && b.maximum[2] > 1.2);
    }
}
