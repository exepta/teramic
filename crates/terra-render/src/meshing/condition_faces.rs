use terra_voxel::meshing::BlockFace;
use terra_world::environment::{SurfaceCondition, SurfaceSnapshot};

/// Keep dry, submerged and exposed parts of a far greedy face distinct. Uniform
/// sky-facing rectangles keep their original geometry and allocate nothing.
pub(super) fn split_weather(
    face: BlockFace,
    conditions: &SurfaceSnapshot,
    exposure: Option<&crate::weather::surfaces::exposure::Exposure>,
    origin_y: i64,
) -> Option<Vec<BlockFace>> {
    let Some(exposure) = exposure else {
        return split(face, conditions);
    };
    let mut grid = [(SurfaceCondition::default(), 0_u8, false); 256];
    let mut different = false;
    for v in 0..face.height {
        for u in 0..face.width {
            let mut cell = face.minimum;
            cell[face.u_axis] += u;
            cell[face.v_axis] += v;
            let payload = exposure.payload(cell, origin_y);
            let weather = if payload < 0.0 {
                0
            } else if payload == 0.0 {
                1
            } else {
                2 + ((payload - 1.0) * 8.0).round() as u8
            };
            let index = (u + v * face.width) as usize;
            grid[index] = (
                conditions
                    .get(&cell.map(|p| p as usize))
                    .copied()
                    .unwrap_or_default(),
                weather,
                exposure.snow_exposed(cell, origin_y),
            );
            different |= grid[index] != grid[0];
        }
    }
    different.then(|| partition(face, &grid))
}

/// Preserve greedy rectangles around sparse appearance edits. The caller validates the face.
pub(super) fn split(face: BlockFace, conditions: &SurfaceSnapshot) -> Option<Vec<BlockFace>> {
    let width = face.width as usize;
    let height = face.height as usize;
    let minimum = face.minimum.map(|v| v as usize);
    let normal_axis = face.side / 2;
    let mut grid = [SurfaceCondition::default(); 16 * 16];
    let mut hit = false;
    for (&cell, &condition) in conditions {
        if cell[normal_axis] != minimum[normal_axis]
            || !(minimum[face.u_axis]..minimum[face.u_axis] + width).contains(&cell[face.u_axis])
            || !(minimum[face.v_axis]..minimum[face.v_axis] + height).contains(&cell[face.v_axis])
        {
            continue;
        }
        let u = cell[face.u_axis] - minimum[face.u_axis];
        let v = cell[face.v_axis] - minimum[face.v_axis];
        grid[u + v * width] = condition;
        hit = true;
    }
    if !hit {
        return None;
    }
    Some(partition(face, &grid))
}

pub(in crate::meshing) fn partition<T: Copy + PartialEq>(
    face: BlockFace,
    grid: &[T],
) -> Vec<BlockFace> {
    let width = face.width as usize;
    let height = face.height as usize;
    let mut covered = [false; 16 * 16];
    let mut result = Vec::new();
    for v in 0..height {
        for u in 0..width {
            let start = u + v * width;
            if covered[start] {
                continue;
            }
            let condition = grid[start];
            let mut run_width = 1;
            while u + run_width < width {
                let next = start + run_width;
                if covered[next] || grid[next] != condition {
                    break;
                }
                run_width += 1;
            }
            let mut run_height = 1;
            while v + run_height < height {
                let row = u + (v + run_height) * width;
                if !(0..run_width).all(|i| !covered[row + i] && grid[row + i] == condition) {
                    break;
                }
                run_height += 1;
            }
            for y in v..v + run_height {
                covered[y * width + u..y * width + u + run_width].fill(true);
            }
            let mut minimum = face.minimum;
            minimum[face.u_axis] += u as u32;
            minimum[face.v_axis] += v as u32;
            result.push(BlockFace {
                minimum,
                width: run_width as u32,
                height: run_height as u32,
                ..face
            });
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn face(side: usize) -> BlockFace {
        let (u_axis, v_axis) = match side / 2 {
            0 => (2, 1),
            1 => (0, 2),
            _ => (0, 1),
        };
        BlockFace {
            edge_mask: 15,
            occupied_neighbor: false,
            minimum: [0; 3],
            width: 16,
            height: 16,
            u_axis,
            v_axis,
            side,
            material: 1,
        }
    }

    fn assert_partition(face: BlockFace, conditions: &SurfaceSnapshot, rectangles: &[BlockFace]) {
        let mut coverage = [[0; 16]; 16];
        for part in rectangles {
            assert_eq!(part.side, face.side);
            assert_eq!(part.material, face.material);
            let condition = conditions
                .get(&part.minimum.map(|v| v as usize))
                .copied()
                .unwrap_or_default();
            for v in 0..part.height {
                for u in 0..part.width {
                    let mut cell = part.minimum.map(|v| v as usize);
                    cell[face.u_axis] += u as usize;
                    cell[face.v_axis] += v as usize;
                    coverage[cell[face.v_axis]][cell[face.u_axis]] += 1;
                    assert_eq!(
                        conditions.get(&cell).copied().unwrap_or_default(),
                        condition
                    );
                }
            }
        }
        assert!(coverage.into_iter().flatten().all(|count| count == 1));
    }

    #[test]
    fn one_changed_cell_leaves_at_most_five_rectangles_on_every_axis() {
        for side in 0..6 {
            let face = face(side);
            for coordinates in [[0, 0], [7, 7], [15, 15]] {
                let mut cell = [0; 3];
                cell[face.u_axis] = coordinates[0];
                cell[face.v_axis] = coordinates[1];
                let conditions = [(cell, SurfaceCondition::new(0.0, 1.0).unwrap())].into();
                let rectangles = split(face, &conditions).unwrap();
                assert!(rectangles.len() <= 5);
                assert_partition(face, &conditions, &rectangles);
            }
        }
    }

    #[test]
    fn different_conditions_stay_separate_and_equal_neighbors_merge() {
        let face = face(3);
        let mut conditions = SurfaceSnapshot::new();
        for v in 0..16 {
            for u in 0..16 {
                let condition = if u < 8 {
                    SurfaceCondition::new(1.0, 0.0).unwrap()
                } else {
                    SurfaceCondition::new(0.0, 0.75).unwrap()
                };
                conditions.insert([u, 0, v], condition);
            }
        }
        let rectangles = split(face, &conditions).unwrap();
        assert_eq!(rectangles.len(), 2);
        assert_partition(face, &conditions, &rectangles);
    }

    #[test]
    fn edits_outside_the_face_do_not_split_or_allocate_rectangles() {
        let condition = SurfaceCondition::new(0.5, 0.5).unwrap();
        let conditions = [([0, 1, 0], condition), ([15, 0, 15], condition)].into();
        let face = BlockFace {
            width: 8,
            height: 8,
            ..face(3)
        };
        assert!(split(face, &conditions).is_none());
    }

    #[test]
    fn distant_greedy_lake_beds_never_inherit_the_dry_bank_rain_payload() {
        let face = face(3);
        let mut exposure = crate::weather::surfaces::exposure::Exposure {
            natural: None,
            roofs: [0; 256],
            snow_roofs: [0; 256],
            coast: [0.0; 256],
            coast_vertices: [0.0; 289],
            water: [f32::NEG_INFINITY; 256],
        };
        assert!(split_weather(face, &SurfaceSnapshot::default(), Some(&exposure), 0).is_none());
        for z in 0..16 {
            for x in 8..16 {
                exposure.water[x + z * 16] = 2.0;
            }
        }
        let parts = split_weather(face, &SurfaceSnapshot::default(), Some(&exposure), 0).unwrap();
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0].width, 8);
        assert_eq!(parts[1].minimum[0], 8);
        assert_eq!(exposure.payload(parts[0].minimum, 0), 1.0);
        assert_eq!(exposure.payload(parts[1].minimum, 0), -1.0);
    }

    #[test]
    fn separate_snow_roofs_partition_a_rain_sheltered_greedy_face() {
        let face = face(3);
        let mut exposure = crate::weather::surfaces::exposure::Exposure {
            natural: None,
            roofs: [20; 256],
            snow_roofs: [20; 256],
            coast: [0.0; 256],
            coast_vertices: [0.0; 289],
            water: [f32::NEG_INFINITY; 256],
        };
        for z in 0..16 {
            for x in 0..8 {
                exposure.snow_roofs[x + z * 16] = 0;
            }
        }
        let parts = split_weather(face, &SurfaceSnapshot::default(), Some(&exposure), 0).unwrap();
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0].width, 8);
        assert_eq!(parts[1].minimum[0], 8);
        assert!(
            parts
                .iter()
                .all(|part| exposure.payload(part.minimum, 0) == 0.0)
        );
        assert!(exposure.snow_exposed(parts[0].minimum, 0));
        assert!(!exposure.snow_exposed(parts[1].minimum, 0));
    }
}
