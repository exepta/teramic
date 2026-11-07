//! One portable lantern item, with floor, wall and suspended mounting poses.
use crate::block_shapes::ShapeBox;

pub const LANTERN: u16 = 193;
pub const CHAIN: u16 = 194;
pub const TABLE: u8 = 0;
pub const HANGING: u8 = 8;
pub const MAX_CHAIN: usize = 128;

pub const fn is_part(material: u16) -> bool {
    matches!(material, LANTERN | CHAIN)
}
pub fn valid_facing(material: u16, facing: u8) -> bool {
    if material == CHAIN {
        facing == 0
    } else {
        material == LANTERN && (facing == TABLE || (4..=HANGING).contains(&facing))
    }
}
pub fn facing(material: u16, normal: [i32; 3]) -> Option<u8> {
    if material == CHAIN {
        return (normal == [0, -1, 0]).then_some(0);
    }
    (material == LANTERN).then_some(match normal {
        [0, 1, 0] => TABLE,
        [1, 0, 0] => 4,
        [-1, 0, 0] => 5,
        [0, 0, 1] => 6,
        [0, 0, -1] => 7,
        [0, -1, 0] => HANGING,
        _ => return None,
    })
}
pub fn support(position: [i64; 3], material: u16, facing: u8) -> Option<[i64; 3]> {
    if !valid_facing(material, facing) {
        return None;
    }
    let normal = if material == CHAIN || facing == HANGING {
        [0.0, -1.0, 0.0]
    } else {
        crate::block_shapes::torch_normal(facing)
    };
    Some(std::array::from_fn(|i| position[i] - normal[i] as i64))
}
/// Authored wall variant faces +Z; its bracket touches the support at Z=0.
pub fn yaw(facing: u8) -> u8 {
    match facing {
        4 => 1,
        5 => 3,
        7 => 2,
        _ => 0,
    }
}
pub fn variant(facing: u8) -> u8 {
    if facing == HANGING {
        2
    } else if facing >= 4 {
        1
    } else {
        0
    }
}
pub fn emission(facing: u8) -> [f64; 3] {
    let point = if variant(facing) == 1 {
        [0.5, 0.43, 0.66]
    } else {
        [0.5, 0.43, 0.5]
    };
    rotate_point(point, facing)
}
pub fn rotate_point([x, y, z]: [f64; 3], facing: u8) -> [f64; 3] {
    let [x, z] = [x - 0.5, z - 0.5];
    let [x, z] = match yaw(facing) {
        1 => [z, -x],
        2 => [-x, -z],
        3 => [-z, x],
        _ => [x, z],
    };
    [x + 0.5, y, z + 0.5]
}
pub fn boxes(material: u16, facing: u8) -> Vec<ShapeBox> {
    let shape = |minimum, maximum| ShapeBox {
        minimum,
        maximum,
        hinged: false,
    };
    if material == CHAIN {
        // Selection follows each hollow link; empty centers stay pick-through.
        let mut result = Vec::with_capacity(20);
        for link in 0..4 {
            let y = link as f64 * 0.25;
            let mut pieces = [
                shape([0.40, y, 0.465], [0.46, y + 0.30, 0.535]),
                shape([0.54, y, 0.465], [0.60, y + 0.30, 0.535]),
                shape([0.40, y, 0.465], [0.60, y + 0.06, 0.535]),
                shape(
                    [0.40, (y + 0.24).min(0.94), 0.465],
                    [0.60, (y + 0.30).min(1.0), 0.535],
                ),
            ];
            for piece in &mut pieces {
                piece.maximum[1] = piece.maximum[1].min(1.0);
                if link % 2 == 1 {
                    *piece = piece.rotated_y(1);
                }
            }
            result.extend(pieces);
        }
        return result;
    }
    if material != LANTERN {
        return Vec::new();
    }
    let wall = variant(facing) == 1;
    let z = if wall { 0.66 } else { 0.5 };
    let mut result = vec![
        shape([0.20, 0.12, z - 0.30], [0.80, 0.845, z + 0.30]),
        shape([0.38, 0.825, z - 0.05], [0.62, 0.945, z + 0.05]),
    ];
    match variant(facing) {
        0 => result.push(shape([0.15, 0.0, 0.15], [0.85, 0.12, 0.85])),
        1 => {
            result.push(shape([0.35, 0.40, 0.0], [0.65, 1.0, 0.12]));
            result.push(shape([0.45, 0.92, 0.04], [0.55, 1.0, 0.70]));
            result.push(shape([0.45, 0.70, 0.07], [0.55, 0.93, 0.35]));
        }
        _ => result.push(shape([0.45, 0.86, 0.45], [0.55, 1.0, 0.55])),
    }
    result
        .into_iter()
        .map(|s| s.rotated_y(yaw(facing)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn poses_touch_their_support_face_and_keep_authored_shapes_inside_one_cell() {
        for (normal, expected) in [
            ([0, 1, 0], 0),
            ([1, 0, 0], 4),
            ([-1, 0, 0], 5),
            ([0, 0, 1], 6),
            ([0, 0, -1], 7),
            ([0, -1, 0], 8),
        ] {
            let pose = facing(LANTERN, normal).unwrap();
            assert_eq!(pose, expected);
            assert_eq!(
                support([0; 3], LANTERN, pose),
                Some(normal.map(|v| -i64::from(v)))
            );
            assert!(boxes(LANTERN, pose).iter().all(ShapeBox::valid));
            let e = emission(pose);
            assert!(e.into_iter().all(|v| (0.0..=1.0).contains(&v)));
        }
        assert!(boxes(CHAIN, 0).iter().all(ShapeBox::valid));
        assert_eq!(facing(CHAIN, [0, 1, 0]), None);
        assert_eq!(support([0; 3], CHAIN, 0), Some([0, 1, 0]));
    }
}
