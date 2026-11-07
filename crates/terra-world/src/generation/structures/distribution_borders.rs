//! Revision-nine land-border spacing; all earlier tickets remain frozen.
use super::*;
const MAX_PROBES: usize = 64;
const REGION_CHUNKS: i64 = REGION_SIZE / 16;
/// Royal authoring is bounded to origin±95. Origin is chunk-aligned at +192,
/// so even the widest foundation occupies at most twelve chunk columns.
const FOOTPRINT_CHUNKS: i64 = 12;
pub(super) const ROYAL_RADIUS: i64 = 95;

#[cfg(test)]
thread_local! {static PROBES:std::cell::Cell<usize>=const {std::cell::Cell::new(0)};}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Choice {
    region: [i64; 2],
    kind: StructureKind,
}
fn priority(seed: u64, p: [i64; 2]) -> (u32, [i64; 2]) {
    (regional_seed(seed ^ 0x424f_5244_4552_5339, p), p)
}
fn tile_size(gap: u16) -> i64 {
    i64::from(
        (u32::from(gap) + FOOTPRINT_CHUNKS as u32)
            .div_ceil(REGION_CHUNKS as u32)
            .saturating_sub(1),
    )
    .max(1)
}
/// At most 64 existing rare tickets are considered, even for 4096 free chunks.
/// Small tiles visit every slot; large tiles use seeded, duplicate-free probes.
fn choice(seed: u64, tile: [i64; 2], size: i64) -> Option<Choice> {
    let area = (size * size) as u64;
    let mut seen = std::collections::BTreeSet::new();
    let mut best: Option<Choice> = None;
    for i in 0..(area as usize).min(MAX_PROBES) {
        let slot = if area <= MAX_PROBES as u64 {
            i as u64
        } else {
            u64::from(regional_seed(
                seed ^ (i as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15),
                tile,
            )) % area
        };
        if !seen.insert(slot) {
            continue;
        }
        let region = [
            tile[0] * size + (slot % size as u64) as i64,
            tile[1] * size + (slot / size as u64) as i64,
        ];
        #[cfg(test)]
        PROBES.set(PROBES.get() + 1);
        let Some(kind) = candidate(seed, region) else {
            continue;
        };
        if best.is_none_or(|old| priority(seed, region) < priority(seed, old.region)) {
            best = Some(Choice { region, kind });
        }
    }
    best
}
pub(super) fn spaced(seed: u64, region: [i64; 2], gap: u16) -> Option<StructureKind> {
    let kind = candidate(seed, region)?;
    // Even neighboring384m regions already leave twelve free chunk columns.
    if i64::from(gap) <= REGION_CHUNKS - FOOTPRINT_CHUNKS {
        return Some(kind);
    }
    let size = tile_size(gap);
    let tile = region.map(|v| v.div_euclid(size));
    let selected = choice(seed, tile, size)?;
    if selected.region != region || selected.kind != kind {
        return None;
    }
    let rank = priority(seed, region);
    for z in -1..=1 {
        for x in -1..=1 {
            if x == 0 && z == 0 {
                continue;
            }
            if choice(seed, [tile[0] + x, tile[1] + z], size)
                .is_some_and(|other| priority(seed, other.region) < rank)
            {
                return None;
            }
        }
    }
    Some(kind)
}

/// Each cached choice has a fixed upper bound of 64 candidate probes. Sharing
/// these choices across the rectangle prevents repeated neighbor work and
/// lets a 4096-chunk gap query visit macrotiles instead of raw regions.
pub(super) fn candidates_in_bounds(
    seed: u64,
    low: [i64; 2],
    high: [i64; 2],
    gap: u16,
    budget: usize,
) -> Result<Vec<[i64; 2]>, ()> {
    let size = tile_size(gap);
    let low_tile = low.map(|v| v.div_euclid(size));
    let high_tile = high.map(|v| v.div_euclid(size));
    let area = (high_tile[0] - low_tile[0] + 1) as u64 * (high_tile[1] - low_tile[1] + 1) as u64;
    if area > budget as u64 {
        return Err(());
    }
    let mut cache = std::collections::BTreeMap::new();
    let mut result = Vec::new();
    for z in low_tile[1]..=high_tile[1] {
        for x in low_tile[0]..=high_tile[0] {
            let Some(selected) = cached_choice(seed, [x, z], size, budget, &mut cache)? else {
                continue;
            };
            if (0..2)
                .any(|axis| selected.region[axis] < low[axis] || selected.region[axis] > high[axis])
            {
                continue;
            }
            let mut accepted = true;
            'neighbors: for dz in -1..=1 {
                for dx in -1..=1 {
                    if dx == 0 && dz == 0 {
                        continue;
                    }
                    if cached_choice(seed, [x + dx, z + dz], size, budget, &mut cache)?.is_some_and(
                        |other| priority(seed, other.region) < priority(seed, selected.region),
                    ) {
                        accepted = false;
                        break 'neighbors;
                    }
                }
            }
            if accepted {
                result.push(selected.region);
            }
        }
    }
    result.sort_unstable();
    Ok(result)
}

fn cached_choice(
    seed: u64,
    tile: [i64; 2],
    size: i64,
    budget: usize,
    cache: &mut std::collections::BTreeMap<[i64; 2], Option<Choice>>,
) -> Result<Option<Choice>, ()> {
    if let Some(&cached) = cache.get(&tile) {
        return Ok(cached);
    }
    if cache.len() >= budget {
        return Err(());
    }
    let selected = choice(seed, tile, size);
    cache.insert(tile, selected);
    Ok(selected)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rectangular_lookup_matches_individual_candidates_across_negative_macro_boundaries() {
        for seed in [42042, 999] {
            for gap in [16, 64] {
                let generator = TerraGenerator::new(terra_core::WorldConfig {
                    seed: terra_core::WorldSeed(seed),
                    structures_revision: 9,
                    structure_border_gap: gap,
                    ..Default::default()
                })
                .unwrap();
                for (low, high) in [([-13, -11], [12, 14]), ([-14, -14], [-1, -2])] {
                    let mut expected = Vec::new();
                    for z in low[1]..=high[1] {
                        for x in low[0]..=high[0] {
                            if generator.royal_site_candidate([x, z]).is_some() {
                                expected.push([x, z]);
                            }
                        }
                    }
                    expected.sort_unstable();
                    assert_eq!(
                        generator.royal_site_candidates_in_bounds(low, high, 1024),
                        Ok(expected),
                        "seed{seed}/gap{gap}/{low:?}/{high:?}"
                    );
                }
                assert!(generator.structures.0.lock().unwrap().slots.is_empty());
            }
        }
    }

    #[test]
    fn maximum_gap_rectangle_uses_cached_macro_choices_within_the_server_budget() {
        let gap = 4096;
        let size = tile_size(gap);
        let generator = TerraGenerator::new(terra_core::WorldConfig {
            structures_revision: 9,
            structure_border_gap: gap,
            ..Default::default()
        })
        .unwrap();
        for (low, high) in [
            ([-size, -size], [size, size]),
            ([-3 * size, -3 * size], [-size - 1, -size - 1]),
        ] {
            let mut expected = Vec::new();
            let low_tile = low.map(|v| v.div_euclid(size));
            let high_tile = high.map(|v| v.div_euclid(size));
            for z in low_tile[1]..=high_tile[1] {
                for x in low_tile[0]..=high_tile[0] {
                    if let Some(c) = choice(generator.config().seed.0, [x, z], size)
                        && (0..2).all(|axis| (low[axis]..=high[axis]).contains(&c.region[axis]))
                        && generator.royal_site_candidate(c.region).is_some()
                    {
                        expected.push(c.region);
                    }
                }
            }
            expected.sort_unstable();
            PROBES.set(0);
            let actual = generator
                .royal_site_candidates_in_bounds(low, high, 1024)
                .expect("a valid fresh gap4096 lookup must fit the server's work budget");
            assert_eq!(actual, expected);
            assert!(
                PROBES.get() <= 25 * MAX_PROBES,
                "each of at most25 central/halo macro choices must be sampled once"
            );
            PROBES.set(0);
            assert_eq!(
                generator.royal_site_candidates_in_bounds(low, high, 0),
                Err(())
            );
            assert_eq!(
                PROBES.get(),
                0,
                "oversized grids reject before any candidate work"
            );
        }
        assert!(generator.structures.0.lock().unwrap().slots.is_empty());
    }

    #[test]
    fn rectangular_lookup_preserves_legacy_flags_clips_world_bounds_and_fails_closed() {
        for revision in [6, 7, 8, 9] {
            let config = terra_core::WorldConfig {
                structures_revision: revision,
                structure_border_gap: 0,
                ..Default::default()
            };
            let generator = TerraGenerator::new(config.clone()).unwrap();
            let mut expected = Vec::new();
            for z in -6..=6 {
                for x in -6..=6 {
                    if generator.royal_site_candidate([x, z]).is_some() {
                        expected.push([x, z]);
                    }
                }
            }
            expected.sort_unstable();
            assert_eq!(
                generator.royal_site_candidates_in_bounds([-6; 2], [6; 2], 169),
                Ok(expected)
            );
            assert_eq!(
                generator.royal_site_candidates_in_bounds([-6; 2], [6; 2], 168),
                Err(())
            );
            assert_eq!(
                generator.royal_site_candidates_in_bounds([1, 0], [0, 1], 1024),
                Err(())
            );
            assert_eq!(
                generator.royal_site_candidates_in_bounds([2501; 2], [i64::MAX; 2], 0),
                Ok(Vec::new())
            );
            assert_eq!(
                generator.royal_site_candidates_in_bounds([i64::MIN; 2], [i64::MAX; 2], 1024),
                Err(())
            );
            let mut disabled_config = config;
            disabled_config.structures.villages = false;
            disabled_config.structures.kingdoms = false;
            disabled_config.structures.outposts = false;
            let disabled = TerraGenerator::new(disabled_config).unwrap();
            assert_eq!(
                disabled.royal_site_candidates_in_bounds([i64::MIN; 2], [i64::MAX; 2], 0),
                Ok(Vec::new())
            );
        }
    }

    fn gap(a: [[i64; 3]; 2], b: [[i64; 3]; 2]) -> i64 {
        [0, 2]
            .map(|axis| {
                let low_a = a[0][axis].div_euclid(16);
                let high_a = a[1][axis].div_euclid(16);
                let low_b = b[0][axis].div_euclid(16);
                let high_b = b[1][axis].div_euclid(16);
                (low_b - high_a - 1).max(low_a - high_b - 1).max(0)
            })
            .into_iter()
            .max()
            .unwrap()
    }
    fn footprint(region: [i64; 2]) -> [[i64; 3]; 2] {
        let c = region.map(|v| v * REGION_SIZE + REGION_SIZE / 2);
        [
            [c[0] - ROYAL_RADIUS, 0, c[1] - ROYAL_RADIUS],
            [c[0] + ROYAL_RADIUS, 0, c[1] + ROYAL_RADIUS],
        ]
    }
    #[test]
    fn every_selected_type_pair_has_the_requested_number_of_free_land_chunks() {
        for seed in [42042, 7, 999] {
            for requested in [0_u16, 16, 37, 96] {
                let mut found = Vec::new();
                for z in -32..=32 {
                    for x in -32..=32 {
                        if let Some(kind) = spaced(seed, [x, z], requested) {
                            assert_eq!(
                                Some(kind),
                                candidate(seed, [x, z]),
                                "new spacing cannot invent commoner/near-start tickets"
                            );
                            for &(old, _) in &found {
                                assert!(
                                    gap(footprint(old), footprint([x, z])) >= i64::from(requested)
                                );
                            }
                            found.push(([x, z], kind));
                        }
                    }
                }
                assert!(found.iter().any(|(_, k)| *k == StructureKind::Kingdom));
                assert!(found.iter().any(|(_, k)| *k == StructureKind::Outpost));
                for &(p, kind) in found.iter().rev() {
                    assert_eq!(spaced(seed, p, requested), Some(kind));
                }
            }
        }
    }
    #[test]
    fn maximum_gap_uses_constant_probes_and_negative_euclidean_tiles() {
        let size = tile_size(4096);
        assert_eq!(size, 171);
        let mut checked = 0;
        for tile in [[0, 0], [-1, -1], [-2, 3], [4, -4]] {
            let Some(c) = choice(42042, tile, size) else {
                continue;
            };
            assert_eq!(c.region.map(|v| v.div_euclid(size)), tile);
            PROBES.set(0);
            let _ = spaced(42042, c.region, 4096);
            assert!(
                PROBES.get() <= MAX_PROBES * 9,
                "spacing performed an area-dependent scan"
            );
            checked += 1;
        }
        assert!(checked > 0);
    }
    #[test]
    fn real_fresh_site_bounds_respect_mixed_kingdom_outpost_spacing() {
        for requested in [16_u16, 64] {
            let terrain = TerraGenerator::new(terra_core::WorldConfig {
                structures_revision: 9,
                structure_border_gap: requested,
                biomes: false,
                water: false,
                trees: false,
                caves: false,
                hill_amplitude: 0.0,
                mountain_amplitude: 0.0,
                ..Default::default()
            })
            .unwrap();
            let mut sites = Vec::new();
            for ring in 0_i64..=24 {
                for z in -ring..=ring {
                    for x in -ring..=ring {
                        if x.abs().max(z.abs()) != ring
                            || terrain.royal_site_candidate([x, z]).is_none()
                        {
                            continue;
                        }
                        let Some(site) = terrain.structure_site([x, z]) else {
                            continue;
                        };
                        assert!(super::super::within_royal_region([x, z], site.bounds));
                        for old in &sites {
                            assert!(gap(*old, site.bounds) >= i64::from(requested));
                        }
                        sites.push(site.bounds);
                        if sites.len() >= 12 {
                            break;
                        }
                    }
                    if sites.len() >= 12 {
                        break;
                    }
                }
                if sites.len() >= 12 {
                    break;
                }
            }
            assert!(sites.len() >= 6);
        }
    }
}

#[cfg(test)]
mod legacy_tests {
    use super::*;
    #[test]
    fn revisions_zero_through_eight_ignore_new_border_snapshot_without_changing_site_bytes_or_seed()
    {
        for revision in 0..=8 {
            let config = terra_core::WorldConfig {
                structures_revision: revision,
                structure_border_gap: 0,
                biomes: false,
                water: false,
                trees: false,
                caves: false,
                hill_amplitude: 0.0,
                mountain_amplitude: 0.0,
                ..Default::default()
            };
            let mut other = config.clone();
            other.structure_border_gap = 4096;
            let a = TerraGenerator::new(config).unwrap();
            let b = TerraGenerator::new(other).unwrap();
            for p in [[0, 0], [-1, -1], [0, -1], [1, 4]] {
                assert_eq!(a.royal_site_candidate(p), b.royal_site_candidate(p));
                match (a.planned_site(p), b.planned_site(p)) {
                    (None, None) => {}
                    (Some(a), Some(b)) => {
                        assert_eq!(a.summary, b.summary);
                        assert_eq!(a.settlement, b.settlement);
                        assert_eq!(a.sections.len(), b.sections.len());
                        for (section, patch) in &a.sections {
                            let other = &b.sections[section];
                            assert_eq!(
                                patch.cells, other.cells,
                                "legacy voxel bytes changed: revision{revision}/{p:?}"
                            );
                            assert_eq!(
                                patch.objects, other.objects,
                                "legacy furniture/loot changed: revision{revision}/{p:?}"
                            );
                        }
                    }
                    _ => panic!("legacy availability changed: revision{revision}/{p:?}"),
                }
            }
        }
    }
}
