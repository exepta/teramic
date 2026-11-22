//! Reservoir lips wake the existing liquid scheduler; no generated falling sheets.
use crate::SurfaceColumn;

pub(crate) const HORIZONTAL: [[i64; 2]; 4] = [[-1, 0], [1, 0], [0, -1], [0, 1]];
pub(super) const MIN_LIP_DROP: f64 = 1.0;

pub(crate) fn natural_outlets(
    global: [i64; 2],
    section_y: i64,
    column: SurfaceColumn,
    neighbors: [SurfaceColumn; 4],
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
                && super::can_hold_fluid(block_at(below))
                && super::can_hold_fluid(block_at([global[0], bottom, global[1]]))
            {
                result.push(below);
            }
        }
        for (offset, neighbor) in HORIZONTAL.into_iter().zip(neighbors) {
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
            let lip = !river_bank && water.level - lower.map_or(floor, |w| w.level) >= MIN_LIP_DROP;
            for y in lo..=hi {
                // Ordinary surface rivers stay within their channel; a real
                // opening below the adjacent ground is a separate escape path.
                if !(y == top && lip || layer == 0 && (y as f64 + 0.5) < neighbor.height)
                    || neighbor.water_columns().at(y).is_some()
                {
                    continue;
                }
                let target = [global[0] + offset[0], y, global[1] + offset[1]];
                if super::can_hold_fluid(block_at(target))
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
