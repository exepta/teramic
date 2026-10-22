/// A ray hit addresses an individual block, regardless of greedy mesh quad size.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BlockHit {
    pub position: [i64; 3],
    pub normal: [i32; 3],
    pub distance: f64,
}
impl BlockHit {
    pub fn adjacent(self) -> [i64; 3] {
        std::array::from_fn(|i| self.position[i] + i64::from(self.normal[i]))
    }
}
/// Recenter the library's i32 traversal at the camera to preserve i64 world coordinates.
pub fn raycast_blocks(
    origin: [f64; 3],
    direction: [f64; 3],
    reach: f64,
    mut block: impl FnMut([i64; 3]) -> Option<u16>,
) -> Option<BlockHit> {
    raycast_blocks_with(origin, direction, reach, &mut block, |hit, _| Some(hit))
}

/// Refine occupied cells against authored sub-cell geometry without stopping at
/// empty parts of a model. Candidates may extend beyond their owner cell.
pub fn raycast_blocks_with(
    origin: [f64; 3],
    direction: [f64; 3],
    reach: f64,
    mut block: impl FnMut([i64; 3]) -> Option<u16>,
    mut refine: impl FnMut(BlockHit, u16) -> Option<BlockHit>,
) -> Option<BlockHit> {
    if origin
        .iter()
        .any(|v| !v.is_finite() || v.abs() > (1_i64 << 40) as f64)
        || direction.iter().any(|v| !v.is_finite())
        || !reach.is_finite()
        || !(0.0..=128.0).contains(&reach)
    {
        return None;
    }
    let length = direction.iter().map(|v| v * v).sum::<f64>().sqrt();
    if !length.is_finite() || length <= f64::EPSILON {
        return None;
    }
    let direction = direction.map(|v| v / length);
    let anchor = origin.map(|v| v.floor() as i64);
    let start = std::array::from_fn::<_, 3, _>(|i| origin[i] - anchor[i] as f64);
    let end = std::array::from_fn::<_, 3, _>(|i| start[i] + direction[i] * reach);
    let mut closest: Option<BlockHit> = None;
    for (p, n) in voxel_traversal::VoxelRaycastF64::new(start, end) {
        let p = [p.0, p.1, p.2];
        let normal = [n.0, n.1, n.2];
        let distance = if let Some(axis) = normal.iter().position(|&v| v != 0) {
            let boundary = f64::from(p[axis]) + if normal[axis] > 0 { 1.0 } else { 0.0 };
            ((boundary - start[axis]) / direction[axis]).max(0.0)
        } else {
            0.0
        };
        if distance > reach + 1e-9 {
            break;
        }
        if closest.is_some_and(|hit| distance > hit.distance + 1.0e-9) {
            break;
        }
        let position = std::array::from_fn(|i| anchor[i] + i64::from(p[i]));
        // Unloaded sections are opaque to targeting, so edits cannot pass through them.
        let Some(material) = block(position) else {
            return closest;
        };
        if material != 0
            && let Some(hit) = refine(
                BlockHit {
                    position,
                    normal,
                    distance,
                },
                material,
            )
            && hit.distance.is_finite()
            && hit.distance >= 0.0
            && hit.distance <= reach
            && closest.is_none_or(|old| hit.distance < old.distance)
        {
            closest = Some(hit);
        }
    }
    closest
}
