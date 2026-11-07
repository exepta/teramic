use super::{WaterPatch, outlets::MIN_LIP_DROP};
use crate::SurfaceColumn;
use terra_voxel::SectionPos;

#[cfg(test)]
thread_local! {
    static CORNER_SAMPLES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Natural reservoirs have horizontal surfaces only. Falling volume belongs to
/// authoritative fluid cells, meshed by the shared liquid-cell renderer.
pub(crate) fn build_channel_patch(
    position: SectionPos,
    origin: [i64; 3],
    columns: &[SurfaceColumn],
    padded: &[u16],
) -> Option<WaterPatch> {
    let mut patch = WaterPatch {
        positions: Vec::new(),
        normals: Vec::new(),
        flow: Vec::new(),
        depth_shore: Vec::new(),
        surface_effects: Vec::new(),
        wave_weights: Vec::new(),
        indices: Vec::new(),
    };
    for z in 0..16 {
        for x in 0..16 {
            let column = columns[x + 1 + 18 * (z + 1)];
            let Some(level) = column.water_level else {
                continue;
            };
            // Most vertical sections never contain this surface. Its corner
            // depths, slopes and normals are useful only for an emitted quad.
            if (level.floor() as i64).div_euclid(16) != position.y {
                continue;
            }
            let iy = (level - origin[1] as f64).floor() as usize + 1;
            let index = x + 1 + 18 * (iy + 18 * (z + 1));
            let volume = if (level - level.floor()).abs() < 1e-8 && iy > 0 {
                padded[index - 18]
            } else {
                padded[index]
            };
            if !super::can_hold_fluid(padded[index])
                || super::space::partial(padded[index])
                || !super::can_hold_fluid(volume)
                || super::space::partial(volume)
            {
                continue;
            }
            let corners = [(x, z), (x, z + 1), (x + 1, z + 1), (x + 1, z)];
            let samples = corners.map(|(cx, cz)| corner(columns, cx, cz, level));
            let points = std::array::from_fn(|i| {
                [
                    corners[i].0 as f32,
                    (samples[i].height - origin[1] as f64) as f32,
                    corners[i].1 as f32,
                ]
            });
            quad(
                &mut patch,
                points,
                samples.map(|s| [s.depth, s.shore]),
                [[0.0; 4]; 4],
                column.water_flow,
            );
        }
    }
    super::geometry::assign_wave_weights(&mut patch, origin, columns, None, padded);
    (!patch.indices.is_empty()).then_some(patch)
}

#[derive(Clone, Copy)]
struct Corner {
    height: f64,
    depth: f32,
    shore: f32,
}

fn corner(columns: &[SurfaceColumn], x: usize, z: usize, reference: f64) -> Corner {
    #[cfg(test)]
    CORNER_SAMPLES.set(CORNER_SAMPLES.get() + 1);
    let adjacent = [
        columns[x + 18 * z],
        columns[x + 1 + 18 * z],
        columns[x + 18 * (z + 1)],
        columns[x + 1 + 18 * (z + 1)],
    ]
    .map(|mut column| {
        if reference < column.water_bed()
            && let Some(water) = column.cave.water
        {
            column.water_level = Some(water.level);
            column.height = water.bed - 0.01;
            column.river_cavity = None;
        }
        column
    });
    let mut levels = [f64::INFINITY; 4];
    let mut wet = 0;
    let mut dry_bed = f64::INFINITY;
    for column in adjacent {
        if let Some(level) = column.water_level {
            levels[wet] = level;
            wet += 1;
        } else {
            dry_bed = dry_bed.min(bed(column));
        }
    }
    levels[..wet].sort_by(f64::total_cmp);
    // Group by actual discontinuities. Comparing each sample with the requesting
    // cell would give different corners when several gentle slopes meet.
    let mut start = 0;
    let mut end = wet;
    for i in 1..wet {
        if levels[i] - levels[i - 1] >= MIN_LIP_DROP {
            if reference < levels[i] {
                end = i;
                break;
            }
            start = i;
        }
    }
    let count = end - start;
    let mean = if count == 0 {
        reference
    } else {
        levels[start..end].iter().sum::<f64>() / count as f64
    };
    // A dry low neighbor is a drainage edge. Meet its solid top instead of
    // leaving a suspended horizontal rim. Raised banks retain the shared level.
    let height = if adjacent
        .iter()
        .any(|c| c.cave.dynamic_water && reference < c.cave.ceiling)
    {
        mean
    } else {
        mean.min(dry_bed)
    };
    let depth = adjacent
        .into_iter()
        .filter(|c| {
            c.water_level
                .is_some_and(|l| count > 0 && l >= levels[start] && l <= levels[end - 1])
        })
        .map(|c| (height - bed(c)).max(0.0))
        .sum::<f64>()
        / count.max(1) as f64;
    Corner {
        height,
        depth: depth as f32,
        shore: 1.0 - wet as f32 * 0.25,
    }
}

#[cfg(any(feature = "bevy", test))]
pub(super) fn shared_corner_height(
    columns: &[SurfaceColumn],
    x: usize,
    z: usize,
    reference: f64,
) -> f64 {
    corner(columns, x, z, reference).height
}

pub(super) fn shared_wave_weight(columns: &[SurfaceColumn], x: usize, z: usize) -> f32 {
    let mut levels = [f64::INFINITY; 4];
    let mut count = 0;
    for column in [
        columns[x + 18 * z],
        columns[x + 1 + 18 * z],
        columns[x + 18 * (z + 1)],
        columns[x + 1 + 18 * (z + 1)],
    ] {
        if let Some(level) = column.water_level {
            levels[count] = level;
            count += 1;
        }
    }
    // A shoreline corner touches dry ground. Displacing it below the block's
    // edge opens a visible air slit, even in deep water beside a steep bank.
    if count < 4 {
        return 0.0;
    }
    levels[..count].sort_by(f64::total_cmp);
    // Both sides of the discontinuity stay pinned where real liquid leaves a lip.
    if levels[..count]
        .windows(2)
        .any(|pair| pair[1] - pair[0] >= MIN_LIP_DROP)
    {
        return 0.0;
    }
    let depth = corner(columns, x, z, levels[0]).depth;
    let t = ((depth - 0.05) / 0.90).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn bed(column: SurfaceColumn) -> f64 {
    column.water_bed()
}

fn quad(
    patch: &mut WaterPatch,
    points: [[f32; 3]; 4],
    depth: [[f32; 2]; 4],
    effects: [[f32; 4]; 4],
    flow: [f32; 2],
) {
    let cross = |a: usize, b: usize, c: usize| {
        let ab: [f32; 3] = std::array::from_fn(|i| points[b][i] - points[a][i]);
        let ac: [f32; 3] = std::array::from_fn(|i| points[c][i] - points[a][i]);
        [
            ab[1] * ac[2] - ab[2] * ac[1],
            ab[2] * ac[0] - ab[0] * ac[2],
            ab[0] * ac[1] - ab[1] * ac[0],
        ]
    };
    let a = cross(0, 1, 2);
    let b = cross(0, 2, 3);
    let sum: [f32; 3] = std::array::from_fn(|i| a[i] + b[i]);
    let length = sum.iter().map(|v| v * v).sum::<f32>().sqrt();
    if length <= f32::EPSILON {
        return;
    }
    let normal = sum.map(|v| v / length);
    let base = patch.positions.len() as u32;
    patch.positions.extend(points);
    patch.normals.extend([normal; 4]);
    patch.depth_shore.extend(depth);
    patch.surface_effects.extend(effects);
    patch.flow.extend([flow; 4]);
    for (triangle, area) in [([0, 1, 2], a), ([0, 2, 3], b)] {
        if area.iter().map(|v| v * v).sum::<f32>() > f32::EPSILON {
            patch.indices.extend(triangle.map(|i| base + i));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_sections_outside_their_water_level_do_not_sample_surface_corners() {
        let terrain = crate::TerraGenerator::new(terra_core::WorldConfig {
            caves: false,
            trees: false,
            biomes: false,
            rivers: false,
            structures_revision: 0,
            landscape_revision: 0,
            ..Default::default()
        })
        .unwrap();
        let padded = vec![0; 18 * 18 * 18];
        for level in [-24.5_f64, -16.0, 8.0, 16.0, 24.5] {
            let mut sample = terrain.surface_column(-16.5, -0.5);
            sample.water_level = Some(level);
            sample.height = level - 4.0;
            sample.water_flow = [0.125, -0.25];
            sample.river_cavity = None;
            sample.cave = crate::generation::CaveStructure::default();
            let columns = vec![sample; 18 * 18];
            let owner = SectionPos::new(-2, (level.floor() as i64).div_euclid(16), -1);
            for dy in [-2, -1, 1, 2] {
                let position = owner.offset([0, dy, 0]).unwrap();
                CORNER_SAMPLES.set(0);
                assert!(
                    build_channel_patch(position, position.origin().unwrap(), &columns, &padded)
                        .is_none()
                );
                assert_eq!(
                    CORNER_SAMPLES.get(),
                    0,
                    "water level {level}, section {position:?}"
                );
            }
            CORNER_SAMPLES.set(0);
            let patch =
                build_channel_patch(owner, owner.origin().unwrap(), &columns, &padded).unwrap();
            assert!(CORNER_SAMPLES.get() > 0);
            assert_eq!(patch.positions.len(), 16 * 16 * 4);
            assert_eq!(patch.indices.len(), 16 * 16 * 6);
            assert!(
                patch
                    .positions
                    .iter()
                    .all(|point| point[1] + owner.origin().unwrap()[1] as f32 == level as f32)
            );
            assert!(patch.flow.iter().all(|flow| *flow == [0.125, -0.25]));
            assert!(
                patch
                    .normals
                    .iter()
                    .all(|normal| *normal == [0.0, 1.0, 0.0])
            );
            CORNER_SAMPLES.set(0);
            assert!(
                build_channel_patch(
                    owner,
                    owner.origin().unwrap(),
                    &columns,
                    &vec![1; 18 * 18 * 18]
                )
                .is_none()
            );
            assert_eq!(
                CORNER_SAMPLES.get(),
                0,
                "occluded reservoir cells use no natural surface corners"
            );
        }
    }

    #[test]
    fn reservoir_lips_split_at_one_block_while_half_block_river_corners_stay_smooth() {
        let terrain = crate::TerraGenerator::new(terra_core::WorldConfig {
            base_height: 4.0,
            sea_level: 8.0,
            hill_amplitude: 0.0,
            mountain_amplitude: 0.0,
            caves: false,
            trees: false,
            biomes: false,
            rivers: false,
            ..Default::default()
        })
        .unwrap();
        for drop in [0.5, 1.0, 2.0, 4.0] {
            for (owner, corner_x, high_side) in [
                (SectionPos::new(-2, 0, -1), 16, true),
                (SectionPos::new(-1, 0, -1), 0, false),
            ] {
                let origin = owner.origin().unwrap();
                let mut columns = vec![terrain.surface_column(0.5, 0.5); 18 * 18];
                for z in 0..18 {
                    for x in 0..18 {
                        columns[x + 18 * z].water_level = Some(if origin[0] + x as i64 - 1 < -16 {
                            8.0 + drop
                        } else {
                            8.0
                        });
                    }
                }
                let height = |reference| corner(&columns, corner_x, 8, reference).height;
                let expected = if drop < MIN_LIP_DROP {
                    8.0 + drop * 0.5
                } else if high_side {
                    8.0 + drop
                } else {
                    8.0
                };
                if drop < MIN_LIP_DROP {
                    assert_eq!(height(8.0), height(8.0 + drop));
                    assert!(shared_wave_weight(&columns, corner_x, 8) > 0.9);
                } else {
                    assert_eq!(height(8.0), 8.0);
                    assert_eq!(height(8.0 + drop), 8.0 + drop);
                    assert_eq!(shared_wave_weight(&columns, corner_x, 8), 0.0);
                }
                let patch =
                    build_channel_patch(owner, origin, &columns, &vec![0; 18 * 18 * 18]).unwrap();
                let edge: Vec<_> = patch
                    .positions
                    .iter()
                    .filter(|p| p[0] + origin[0] as f32 == -16.0)
                    .collect();
                assert!(!edge.is_empty());
                assert!(
                    edge.iter().all(|p| p[1] == expected as f32),
                    "drop={drop} {owner:?}: no averaged diagonal sheet may bridge a real falling lip"
                );
            }
        }
    }
}
