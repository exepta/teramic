use crate::edits::{BlockObjectState, EditJournal};
use terra_core::{
    block_shapes::{ShapeBox, builtin_boxes},
    plumbing::{self as core, Cell, DIRECTIONS},
};

pub fn connection_mask(
    journal: &EditJournal,
    position: Cell,
    material: u16,
    facing: u8,
    mut block_at: impl FnMut(Cell) -> Option<u16>,
) -> u8 {
    let mut mask: u8 = 0;
    for (side, direction) in DIRECTIONS.iter().enumerate() {
        let cell = std::array::from_fn(|axis| position[axis] + direction[axis]);
        if block_at(cell).is_some_and(|other| {
            core::connects(
                other,
                journal.object_state(cell).unwrap_or_default().facing,
                DIRECTIONS[side ^ 1],
            )
        }) {
            mask |= 1 << side;
        }
    }
    if mask.count_ones() == 1 && core::is_pipe(material) {
        let side = mask.trailing_zeros();
        mask |= 1 << (side ^ 1);
    }
    if mask == 0 && core::is_pipe(material) {
        let side = core::direction_index(terra_core::kinetic::axis(facing)).unwrap();
        mask = (1 << side) | (1 << (side ^ 1));
    }
    mask
}

/// Local boxes, shared by selection, placement and the actual physics mesh.
pub fn plumbing_boxes(
    journal: &EditJournal,
    position: Cell,
    material: u16,
    state: BlockObjectState,
    block_at: impl FnMut(Cell) -> Option<u16>,
) -> Vec<ShapeBox> {
    let cube = |minimum, maximum| ShapeBox {
        minimum,
        maximum,
        hinged: false,
    };
    if core::is_pipe(material) {
        let mask = connection_mask(journal, position, material, state.facing, block_at);
        let half = if mask.count_ones() >= 3 {
            0.23 // The round manifold's outer radius.
        } else if (0..3).any(|axis| mask == 3 << (axis * 2)) {
            0.15
        } else {
            0.20 // Bounds of the swept quarter-circle bend.
        };
        let mut boxes = vec![cube([0.5 - half; 3], [0.5 + half; 3])];
        for side in 0..6 {
            if mask & (1 << side) != 0 {
                let mut lo = [0.35; 3];
                let mut hi = [0.65; 3];
                if side % 2 == 0 {
                    lo[side / 2] = 0.;
                    hi[side / 2] = 0.5;
                } else {
                    lo[side / 2] = 0.5;
                    hi[side / 2] = 1.;
                }
                boxes.push(cube(lo, hi));
            }
        }
        return boxes;
    }
    if material == core::BASIN {
        let mask = state.facing;
        let top = if mask & 8 != 0 { 1.0 } else { 0.95 };
        let mut boxes = Vec::with_capacity(5);
        if mask & 4 == 0 {
            boxes.push(cube([0.; 3], [1., 0.13, 1.]));
        }
        if mask & 1 == 0 {
            boxes.push(cube([0.; 3], [0.16, top, 1.]));
        }
        if mask & 2 == 0 {
            boxes.push(cube([0.84, 0., 0.], [1., top, 1.]));
        }
        if mask & 16 == 0 {
            boxes.push(cube([0.; 3], [1., top, 0.16]));
        }
        if mask & 32 == 0 {
            boxes.push(cube([0., 0., 0.84], [1., top, 1.]));
        }
        return boxes;
    }
    builtin_boxes(material)
        .iter()
        .copied()
        .map(|shape| {
            if material == core::VALVE {
                shape.rotated_axis(core::orientation(material, state.facing))
            } else {
                shape.rotated_y(core::orientation(material, state.facing))
            }
        })
        .collect()
}
