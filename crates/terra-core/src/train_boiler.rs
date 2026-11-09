//! The placed pose records a three-by-three boiler on a three-wide frame.
use crate::block_shapes::ShapeBox;
pub const fn is_boiler(material: u16) -> bool {
    matches!(material, 219..=223)
}
pub const fn is_wide(material: u16, facing: u8) -> bool {
    is_boiler(material) && facing >= 4 && facing < 8
}
pub fn wide_support(
    position: [i64; 3],
    facing: u8,
    mut block: impl FnMut([i64; 3]) -> Option<u16>,
) -> bool {
    let side = if facing.is_multiple_of(2) { 0 } else { 2 };
    let mut frame_found = false;
    let supported = [-1, 0, 1].into_iter().all(|offset| {
        let mut cell = position;
        cell[1] -= 1;
        cell[side] += offset;
        let Some(material) = block(cell) else {
            return false;
        };
        frame_found |= matches!(material, 210 | 212 | 241 | 242);
        matches!(material, 210 | 212 | 241 | 242) || crate::beds::supports_bed_floor(material)
    });
    if !frame_found {
        let mut below = position;
        below[1] -= 2;
        frame_found =
            block(below).is_some_and(|material| matches!(material, 210 | 212 | 241 | 242));
    }
    supported && frame_found
}
pub fn boxes(material: u16, facing: u8) -> Vec<ShapeBox> {
    if !is_boiler(material) || facing >= 8 {
        return Vec::new();
    }
    crate::locomotive_parts::base_boxes(material)
        .iter()
        .map(|shape| {
            let mut shape = *shape;
            if is_wide(material, facing) {
                shape.minimum[0] = (shape.minimum[0] - 0.5) * 3.0 + 0.5;
                shape.maximum[0] = (shape.maximum[0] - 0.5) * 3.0 + 0.5;
                shape.minimum[1] *= 3.0;
                shape.maximum[1] *= 3.0;
            }
            shape.rotated_y(facing % 4)
        })
        .collect()
}

/// Canonical occupied-voxel contacts connect fittings to an extended boiler.
pub fn attachment_offsets(material: u16, facing: u8) -> &'static [[i16; 3]] {
    use std::sync::OnceLock;
    static CONTACTS: [OnceLock<Vec<[i16; 3]>>; 20] = [const { OnceLock::new() }; 20];
    if !is_wide(material, facing) {
        return &[];
    }
    CONTACTS[usize::from(material - 219) * 4 + usize::from(facing - 4)]
        .get_or_init(|| {
            let mut contacts = std::collections::BTreeSet::new();
            for shape in boxes(material, facing) {
                let low = shape.minimum.map(|value| (value + 1e-6).floor() as i16);
                let high = shape.maximum.map(|value| (value - 1e-6).floor() as i16);
                for x in low[0]..=high[0] {
                    for y in low[1]..=high[1] {
                        for z in low[2]..=high[2] {
                            for delta in [
                                [0, 0, 0],
                                [1, 0, 0],
                                [-1, 0, 0],
                                [0, 1, 0],
                                [0, -1, 0],
                                [0, 0, 1],
                                [0, 0, -1],
                            ] {
                                contacts.insert([x + delta[0], y + delta[1], z + delta[2]]);
                            }
                        }
                    }
                }
            }
            contacts.into_iter().collect::<Vec<_>>()
        })
        .as_slice()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn three_wide_frames_choose_an_oriented_three_by_three_boiler() {
        for facing in 0..4 {
            let axis = if facing % 2 == 0 { 0 } else { 2 };
            assert!(wide_support([0, 1, 0], facing, |p| (p[1] == 0
                && p[2 - axis] == 0
                && p[axis].abs() <= 1)
                .then_some(212)));
            assert!(!wide_support([0, 1, 0], facing, |p| (p == [0; 3]).then_some(212)));
            for material in 219..=223 {
                let wide = boxes(material, facing | 4);
                let plain = boxes(material, facing);
                let span = |shapes: &[ShapeBox], axis: usize| {
                    shapes
                        .iter()
                        .map(|b| b.maximum[axis])
                        .fold(f64::NEG_INFINITY, f64::max)
                        - shapes
                            .iter()
                            .map(|b| b.minimum[axis])
                            .fold(f64::INFINITY, f64::min)
                };
                assert!((span(&wide, axis) - span(&plain, axis) * 3.0).abs() < 1e-8);
                assert!((span(&wide, 1) - span(&plain, 1) * 3.0).abs() < 1e-8);
                assert!(crate::locomotive_parts::valid_facing(material, facing | 4));
            }
        }
    }
}
