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
