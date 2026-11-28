//! Reservoir lips wake the existing liquid scheduler; no generated falling sheets.
use crate::SurfaceColumn;

pub(crate) const HORIZONTAL: [[i64; 2]; 4] = [[-1, 0], [1, 0], [0, -1], [0, 1]];
pub(super) const MIN_LIP_DROP: f64 = 1.0;

/// Wet columns sharing a water vertex, in NW, NE, SW, SE order.
pub(super) fn level_group(
    mut levels: [Option<f64>; 4],
    reference: f64,
) -> std::ops::RangeInclusive<f64> {
    // At a bank one corner of a sloping sheet may be dry. Complete its plane
    // only to classify the slope; it contributes neither depth nor wet area.
    // Two plateaus separated by a cliff extrapolate to the original plateaus.
    if let Some(i) = levels.iter().position(Option::is_none)
        && let (Some(x), Some(z), Some(opposite)) = (levels[i ^ 1], levels[i ^ 2], levels[i ^ 3])
        && x != opposite
        && z != opposite
        && x != z
    {
        levels[i] = Some(x + z - opposite);
    }
    let mut sorted = levels.map(|v| v.unwrap_or(f64::INFINITY));
    sorted.sort_by(f64::total_cmp);
    let mut lo = sorted[0];
    for [a, b] in sorted.array_windows() {
        if b - a >= MIN_LIP_DROP {
            if reference < *b {
                return lo..=*a;
            }
            lo = *b;
        }
    }
    lo..=sorted[3]
}

/// Even when two cell centres differ by a metre, a diagonal river can
/// already share both endpoints of their edge via the adjoining columns.
pub(crate) fn shares_channel_edge(
    [dx, dz]: [i64; 2],
    a: f64,
    b: f64,
    mut level_at: impl FnMut([i64; 2]) -> Option<f64>,
) -> bool {
    [-1, 1].into_iter().all(|sign| {
        let (tx, tz) = (dz * sign, dx * sign);
        let levels = [
            Some(a),
            Some(b),
            level_at([tx, tz]),
            level_at([dx + tx, dz + tz]),
        ];
        level_group(levels, a).contains(&b)
    })
}

pub(crate) fn natural_outlets(
    global: [i64; 2],
    section_y: i64,
    column: SurfaceColumn,
    mut neighbor_at: impl FnMut([i64; 2]) -> SurfaceColumn,
    mut block_at: impl FnMut([i64; 3]) -> u16,
) -> Vec<[i64; 3]> {
    let mut result = Vec::new();
    for (layer, water) in column.water_columns().intervals.into_iter().enumerate() {
        let Some(water) = water else { continue };
        let bottom = water.bed.ceil() as i64;
        let top = water.level.ceil() as i64 - 1;
        let base = section_y * 16;
        let lo = bottom.max(base);
        let hi = top.min(base + 15);
        if lo > hi {
            continue;
        }
        // A chamber or a terrain cut can expose the *underside* of a
        // reservoir far below its visible surface. Wake a paid downward
        // transfer from the last wet cell, even across a section boundary.
        if bottom >= base && bottom <= hi {
            let below = [global[0], bottom - 1, global[1]];
            if column.water_columns().at(bottom - 1).is_none()
                && !column
                    .falls
                    .into_iter()
                    .flatten()
                    .any(|s| s.contains(below[1]))
                && super::can_hold_fluid(block_at(below))
                && super::can_hold_fluid(block_at([global[0], bottom, global[1]]))
            {
                result.push(below);
            }
        }
        for offset in HORIZONTAL {
            let neighbor = neighbor_at(offset);
            let lower = neighbor.water_columns().intervals[layer];
            // A completely wet shared face has no outlet.
            if lower.is_some_and(|w| w.bed <= lo as f64 && w.level > hi as f64) {
                continue;
            }
            let floor = if layer == 0 {
                neighbor.water_bed()
            } else {
                neighbor.cave.floor
            };
            let river_bank = layer == 0
                && column.water_flow != [0.; 2]
                && neighbor.water_flow == [0.; 2]
                && lower.is_none();
            let lip = !river_bank
                && water.level - lower.map_or(floor, |w| w.level) >= MIN_LIP_DROP
                && !(layer == 0
                    && lower.is_some_and(|w| {
                        shares_channel_edge(offset, water.level, w.level, |p| {
                            neighbor_at(p).water_level
                        })
                    }));
            for y in lo..=hi {
                // Ordinary surface rivers stay within their channel; a real
                // opening below the adjacent ground is a separate escape path.
                if !(y == top && lip || layer == 0 && (y as f64 + 0.5) < neighbor.height)
                    || neighbor.water_columns().at(y).is_some()
                {
                    continue;
                }
                let target = [global[0] + offset[0], y, global[1] + offset[1]];
                if !neighbor
                    .falls
                    .into_iter()
                    .flatten()
                    .any(|s| s.contains(target[1]))
                    && super::can_hold_fluid(block_at(target))
                    && super::can_hold_fluid(block_at([global[0], y, global[1]]))
                {
                    result.push(target);
                }
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{TerraGenerator, water::WaterColumn};

    fn natural_outlets(
        global: [i64; 2],
        section_y: i64,
        column: SurfaceColumn,
        neighbors: [SurfaceColumn; 4],
        block_at: impl FnMut([i64; 3]) -> u16,
    ) -> Vec<[i64; 3]> {
        super::natural_outlets(
            global,
            section_y,
            column,
            |offset| {
                HORIZONTAL
                    .iter()
                    .position(|p| *p == offset)
                    .map_or(column, |i| neighbors[i])
            },
            block_at,
        )
    }

    #[test]
    fn a_dry_riverbank_does_not_split_the_neighboring_sloped_sheet() {
        let bank = [Some(25.09), Some(26.19), Some(24.46), None];
        assert!(level_group(bank, 25.09).contains(&26.19));
        assert!(level_group(bank, 26.19).contains(&25.09));
        let cliff = [Some(25.09), Some(26.19), Some(25.09), None];
        assert!(!level_group(cliff, 25.09).contains(&26.19));
    }

    #[test]
    fn connected_steep_river_edges_do_not_seed_a_second_waterfall() {
        let terrain = TerraGenerator::new(terra_core::WorldConfig::default()).unwrap();
        let mut upper = terrain.surface_column(0.5, 0.5);
        upper.height = 4.0;
        upper.water_level = Some(12.8);
        upper.water_flow = [0.6, 1.0];
        upper.cave.water = None;
        upper.river_cavity = None;
        for offset in HORIZONTAL {
            let [dx, dz] = offset;
            let neighbor = |[x, z]: [i64; 2]| crate::SurfaceColumn {
                water_level: Some(
                    12.8 - (x * dx + z * dz) as f64 * 1.2 - (x * dz + z * dx) as f64 * 0.6,
                ),
                ..upper
            };
            let target = [offset[0], 12, offset[1]];
            assert!(shares_channel_edge(offset, 12.8, 11.6, |p| neighbor(p).water_level));
            let wake = super::natural_outlets([0, 0], 0, upper, neighbor, |p| u16::from(p[1] < 5));
            assert!(!wake.contains(&target));
            // Plateaus separated by the same height difference still discharge.
            let cliff = |[x, z]: [i64; 2]| crate::SurfaceColumn {
                water_level: Some(if x * dx + z * dz > 0 { 11.6 } else { 12.8 }),
                ..upper
            };
            let wake = super::natural_outlets([0, 0], 0, upper, cliff, |p| u16::from(p[1] < 5));
            assert!(wake.contains(&target));
        }
    }

    #[test]
    fn a_real_breach_in_the_bed_or_submerged_wall_drains_even_below_the_surface_section() {
        let terrain = TerraGenerator::new(terra_core::WorldConfig::default()).unwrap();
        let mut wet = terrain.surface_column(0.5, 0.5);
        wet.water_level = Some(18.);
        wet.height = 13.;
        wet.river_cavity = None;
        wet.cave.water = None;
        wet.water_flow = [0., 1.];
        let mut bank = wet;
        bank.height = 24.;
        bank.water_flow = [0.; 2];
        bank.water_level = None;
        let adjacent = [wet, bank, wet, wet];
        let result = natural_outlets([15, 15], 0, wet, adjacent, |p| match p {
            [15, 12..=17, 15] | [16, 14, 15] => 0,
            _ => 3,
        });
        assert_eq!(result, vec![[15, 12, 15], [16, 14, 15]]);
        assert!(
            natural_outlets([15, 15], 0, wet, adjacent, |p| {
                if matches!(p, [15, 13..=17, 15]) { 0 } else { 3 }
            })
            .is_empty()
        );
    }

    #[test]
    fn generated_river_lips_wake_only_the_downstream_channel_and_accept_real_lakes() {
        let terrain = TerraGenerator::new(terra_core::WorldConfig::default()).unwrap();
        let mut upper = terrain.surface_column(0.5, 0.5);
        upper.height = 18.0;
        upper.water_level = Some(23.0);
        upper.water_flow = [0.0, 1.0];
        upper.cave.water = None;
        let mut bank = upper;
        bank.height = 10.0;
        bank.water_level = None;
        bank.water_flow = [0.0; 2];
        let mut downstream = bank;
        downstream.water_flow = [0.0, 1.0];
        let mut neighbors = [bank; 4];
        neighbors[3] = downstream;
        assert_eq!(
            natural_outlets([15, 15], 1, upper, neighbors, |p| u16::from(p[1] < 18)),
            vec![[15, 22, 16]]
        );
        downstream.water_flow = [0.0; 2];
        downstream.water_level = Some(15.0);
        neighbors[3] = downstream;
        assert_eq!(
            natural_outlets([15, 15], 1, upper, neighbors, |p| u16::from(p[1] < 18)),
            vec![[15, 22, 16]]
        );
    }

    #[test]
    fn actual_open_lips_only_wake_their_source_section_for_surface_and_legacy_caves() {
        let terrain = TerraGenerator::new(terra_core::WorldConfig::default()).unwrap();
        for base in [[15, 15], [-17, -17]] {
            for (underground, drop) in [false, true].into_iter().flat_map(|cave| {
                [0.5, 1.0, 2.0, 4.0, 8.0]
                    .into_iter()
                    .map(move |drop| (cave, drop))
            }) {
                let mut upper = terrain.surface_column(0.5, 0.5);
                let mut lower = upper;
                upper.water_level = None;
                lower.water_level = None;
                upper.cave.water = None;
                lower.cave.water = None;
                let water = |level| WaterColumn {
                    level,
                    bed: -12.0,
                    depth: level + 12.0,
                };
                if underground {
                    upper.cave.water = Some(water(-1.0));
                    lower.cave.water = Some(water(-1.0 - drop));
                } else {
                    upper.height = 6.0;
                    lower.height = -12.0;
                    upper.water_level = Some(23.0);
                    lower.water_level = Some(23.0 - drop);
                }
                let y: i64 = if underground { -2 } else { 22 };
                let mut neighbors = [upper; 4];
                neighbors[1] = lower;
                let target = [base[0] + 1, y, base[1]];
                let expected = if drop >= 1.0 {
                    vec![target]
                } else {
                    Vec::new()
                };
                assert_eq!(
                    natural_outlets(base, y.div_euclid(16), upper, neighbors, |p| u16::from(
                        p[1] != y
                    )),
                    expected,
                    "cave={underground} drop={drop}: only actual one-block-or-larger lips seed a fall"
                );
                assert!(
                    natural_outlets(base, y.div_euclid(16) + 1, upper, neighbors, |p| u16::from(
                        p[1] != y
                    ))
                    .is_empty()
                );
                assert!(
                    natural_outlets(base, y.div_euclid(16), upper, neighbors, |p| u16::from(
                        p[1] != y || p == target
                    ) * 3)
                    .is_empty(),
                    "a blocked lip cannot emit water through its wall"
                );
                assert!(
                    natural_outlets(base, y.div_euclid(16), upper, neighbors, |p| u16::from(
                        p[1] != y || p == [base[0], y, base[1]]
                    ) * 3)
                    .is_empty(),
                    "an edited-away source cannot seed a fall"
                );
            }
        }
    }
}
