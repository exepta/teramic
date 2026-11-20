/// Flatten a bevel band against solid neighbors. Free exterior edges retain their
/// modeled shape; joined surfaces meet exactly on the canonical voxel boundary.
pub(crate) fn connected_position(mut point: [f32; 3], visible: u8, bevel: f32) -> [f32; 3] {
    let touches_neighbor = point.iter().enumerate().any(|(axis, &coordinate)| {
        (visible & (1 << (axis * 2)) == 0 && coordinate <= bevel + 0.00001)
            || (visible & (1 << (axis * 2 + 1)) == 0 && coordinate >= 1.0 - bevel - 0.00001)
    });
    if !touches_neighbor {
        return point;
    }
    // A culled interface must have the same complete outline for every material/LOD.
    // Flattening only its normal axis leaves open wedges at concave junctions.
    for coordinate in &mut point {
        if *coordinate <= bevel + 0.00001 {
            *coordinate = 0.0;
        }
        if *coordinate >= 1.0 - bevel - 0.00001 {
            *coordinate = 1.0;
        }
    }
    point
}

/// The imported grass cube has a shallow rounded mound around every tile.
/// A continuous turf surface keeps that rounding only at actual exterior edges;
/// its leaf/soil relief remains in the authored PBR textures.
pub(crate) fn connected_turf_top(mut point: [f32; 3], visible: u8, bevel: f32) -> [f32; 3] {
    let horizontal = 0b11_0011;
    if visible & horizontal == horizontal {
        return connected_position(point, visible, bevel);
    }
    let exterior = [(0, 0), (0, 1), (2, 4), (2, 5)]
        .into_iter()
        .any(|(axis, side)| {
            visible & (1 << side) != 0
                && if side % 2 == 0 {
                    point[axis] <= bevel + 0.00001
                } else {
                    point[axis] >= 1.0 - bevel - 0.00001
                }
        });
    if !exterior {
        point[1] = 1.0;
    }
    connected_position(point, visible, bevel)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joined_edges_have_a_canonical_outline_but_free_edges_keep_their_bevel() {
        let rounded = [0.02, 0.98, 0.5];
        assert_eq!(connected_position(rounded, 63, 0.065), rounded);
        assert_eq!(connected_position(rounded, 63 & !1, 0.065), [0.0, 1.0, 0.5]);
        assert_eq!(connected_position(rounded, 63 & !2, 0.065), rounded);
    }
}
