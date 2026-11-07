//! Revision-eight site spacing is derived from coordinates and seed, never
//! from the order chunks or players arrive in the world.
use super::*;
#[path = "distribution_borders.rs"]
mod borders;

pub const KINGDOM_START_CLEARANCE: i64 = 1024;
pub const OUTPOST_START_CLEARANCE: i64 = 512;

fn eligible(seed: u64, region: [i64; 2]) -> Option<StructureKind> {
    if region.iter().any(|v| v.unsigned_abs() > 2500) {
        return None;
    }
    let ticket = regional_seed(seed, region);
    if !build::royal::selected(ticket) {
        return None;
    }
    let outpost = (ticket >> 8) % 4 == 1;
    // The original 6/7 distribution is only thinned, so unrelated site kinds
    // keep their regional identities. A larger origin refuge protects starts.
    let center = region.map(|v| v * REGION_SIZE + REGION_SIZE / 2);
    let minimum = if outpost { 768_i64 } else { 1536_i64 };
    if center[0] * center[0] + center[1] * center[1] < minimum * minimum {
        return None;
    }
    let salt = if outpost { 0x4f55_5450 } else { 0x4b49_4e47 };
    let chance = if outpost { 2 } else { 4 };
    layer_seed(u64::from(ticket), salt)
        .is_multiple_of(chance)
        .then_some(if outpost {
            StructureKind::Outpost
        } else {
            StructureKind::Kingdom
        })
}

pub(super) fn candidate(seed: u64, region: [i64; 2]) -> Option<StructureKind> {
    let kind = eligible(seed, region)?;
    let rank = |p| (regional_seed(seed ^ 0x7350_4143_494e_4738, p), p);
    let priority = rank(region);
    let radius = if kind == StructureKind::Kingdom {
        2_i64
    } else {
        1_i64
    };
    for z in -radius..=radius {
        for x in -radius..=radius {
            if x == 0 && z == 0 {
                continue;
            }
            let neighbor = [region[0] + x, region[1] + z];
            if eligible(seed, neighbor) == Some(kind) && rank(neighbor) < priority {
                return None;
            }
        }
    }
    Some(kind)
}

pub(super) fn configured_candidate(
    config: &terra_core::WorldConfig,
    region: [i64; 2],
) -> Option<StructureKind> {
    if config.structures_revision >= 9 {
        borders::spaced(config.seed.0, region, config.structure_border_gap)
    } else {
        candidate(config.seed.0, region)
    }
}
/// Conservative selection is only valid while actual authored land stays in
/// this footprint. New revisions fail closed if a future builder exceeds it.
pub(super) fn within_royal_region(region: [i64; 2], bounds: [[i64; 3]; 2]) -> bool {
    [0, 2].into_iter().enumerate().all(|(i, axis)| {
        let center = region[i] * REGION_SIZE + REGION_SIZE / 2;
        bounds[0][axis] >= center - borders::ROYAL_RADIUS
            && bounds[1][axis] <= center + borders::ROYAL_RADIUS
    })
}

impl TerraGenerator {
    /// Cheap seeded eligibility only: biome, terrain fit and bounded authoring
    /// can still reject this region. Does not construct or cache any site.
    pub fn royal_site_candidate(&self, region: [i64; 2]) -> Option<StructureKind> {
        let revision = self.config().structures_revision;
        if revision < 6 || region.iter().any(|v| v.unsigned_abs() > 2500) {
            return None;
        }
        let kind = if revision >= 8 {
            configured_candidate(self.config(), region)?
        } else {
            let seed = regional_seed(self.config().seed.0, region);
            if !build::royal::selected(seed) {
                return None;
            }
            if (seed >> 8) % 4 == 1 {
                StructureKind::Outpost
            } else {
                StructureKind::Kingdom
            }
        };
        let enabled = if revision == 6 {
            self.config().structures.villages
        } else {
            kind.enabled(self.config().structures)
        };
        enabled.then_some(kind)
    }

    /// Enumerate seeded royal candidates in an inclusive region rectangle,
    /// without planning sites. Results are sorted and respect feature toggles.
    ///
    /// The budget bounds unique macrocell choices (each at most 64 tickets) in
    /// revision 9, including cached neighbor choices. Earlier revisions and
    /// gaps up to 12 count individual regions. Exhaustion returns no partial
    /// answer, so callers can fail closed rather than miss a nearby realm.
    #[allow(clippy::result_unit_err)]
    pub fn royal_site_candidates_in_bounds(
        &self,
        low: [i64; 2],
        high: [i64; 2],
        budget: usize,
    ) -> Result<Vec<[i64; 2]>, ()> {
        if (0..2).any(|axis| low[axis] > high[axis]) {
            return Err(());
        }
        let revision = self.config().structures_revision;
        let settings = self.config().structures;
        if revision < 6
            || if revision == 6 {
                !settings.villages
            } else {
                !settings.kingdoms && !settings.outposts
            }
        {
            return Ok(Vec::new());
        }
        let low = low.map(|v| v.max(-2500));
        let high = high.map(|v| v.min(2500));
        if (0..2).any(|axis| low[axis] > high[axis]) {
            return Ok(Vec::new());
        }
        if revision >= 9 && self.config().structure_border_gap > 12 {
            let mut selected = borders::candidates_in_bounds(
                self.config().seed.0,
                low,
                high,
                self.config().structure_border_gap,
                budget,
            )?;
            selected.retain(|&region| {
                candidate(self.config().seed.0, region).is_some_and(|kind| kind.enabled(settings))
            });
            return Ok(selected);
        }
        let area = (high[0] - low[0] + 1) as u64 * (high[1] - low[1] + 1) as u64;
        if area > budget as u64 {
            return Err(());
        }
        let mut selected = Vec::new();
        for z in low[1]..=high[1] {
            for x in low[0]..=high[0] {
                if self.royal_site_candidate([x, z]).is_some() {
                    selected.push([x, z]);
                }
            }
        }
        selected.sort_unstable();
        Ok(selected)
    }

    /// Conservative startup exclusion uses only bounded seed calculations.
    /// It never builds neighboring castles while searching for a dry column.
    pub(crate) fn settlement_start_clear(&self, point: [i64; 2]) -> bool {
        if self.config().structures_revision < 8 {
            return true;
        }
        let settings = self.config().structures;
        if !settings.kingdoms && !settings.outposts {
            return true;
        }
        let region = point.map(|v| v.div_euclid(REGION_SIZE));
        for z in -3..=3 {
            for x in -3..=3 {
                let p = [region[0] + x, region[1] + z];
                let Some(kind) = configured_candidate(self.config(), p) else {
                    continue;
                };
                let clearance = match kind {
                    StructureKind::Kingdom if settings.kingdoms => KINGDOM_START_CLEARANCE,
                    StructureKind::Outpost if settings.outposts => OUTPOST_START_CLEARANCE,
                    _ => continue,
                };
                let center = p.map(|v| v * REGION_SIZE + REGION_SIZE / 2);
                if (center[0] - point[0]).pow(2) + (center[1] - point[1]).pow(2) < clearance.pow(2)
                {
                    // Marine/desert slots cannot host a royal site. Do not
                    // exclude the only dry shoreline for a castle that cannot exist.
                    let biome = self.biome_at(center[0] as f64, center[1] as f64);
                    if biome.weights.ocean <= 0.25
                        && biome.weights.desert + biome.weights.cliffted_desert <= 0.5
                    {
                        return false;
                    }
                }
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn public_candidate_filter_is_cheap_bounded_and_preserves_each_revision_and_toggle() {
        for revision in [5, 6, 7, 8] {
            let mut config = terra_core::WorldConfig {
                structures_revision: revision,
                biomes: false,
                ..Default::default()
            };
            let generator = TerraGenerator::new(config.clone()).unwrap();
            for z in -8..=8 {
                for x in -8..=8 {
                    let region = [x, z];
                    let expected = if revision < 6 {
                        None
                    } else if revision >= 8 {
                        candidate(config.seed.0, region)
                    } else {
                        let seed = regional_seed(config.seed.0, region);
                        build::royal::selected(seed).then_some(if (seed >> 8) % 4 == 1 {
                            StructureKind::Outpost
                        } else {
                            StructureKind::Kingdom
                        })
                    };
                    assert_eq!(generator.royal_site_candidate(region), expected);
                }
            }
            assert!(generator.structures.0.lock().unwrap().slots.is_empty());
            assert_eq!(generator.royal_site_candidate([i64::MAX, 0]), None);
            assert_eq!(generator.royal_site_candidate([0, i64::MIN]), None);
            config.structures.villages = false;
            config.structures.kingdoms = false;
            config.structures.outposts = false;
            let disabled = TerraGenerator::new(config).unwrap();
            for z in -8..=8 {
                for x in -8..=8 {
                    assert_eq!(disabled.royal_site_candidate([x, z]), None);
                }
            }
        }
    }
    #[test]
    fn new_sites_are_rare_spaced_and_keep_a_large_start_refuge() {
        for seed in [42042, 7, 999] {
            let mut old = 0;
            let mut kingdoms = 0;
            let mut outposts = 0;
            for z in -24..=24 {
                for x in -24..=24 {
                    let p = [x, z];
                    old += usize::from(build::royal::selected(regional_seed(seed, p)));
                    let Some(kind) = candidate(seed, p) else {
                        continue;
                    };
                    let center = p.map(|v| v * REGION_SIZE + REGION_SIZE / 2);
                    let (radius, minimum) = if kind == StructureKind::Kingdom {
                        kingdoms += 1;
                        (2, 1536_i64)
                    } else {
                        outposts += 1;
                        (1, 768_i64)
                    };
                    assert!(center[0].pow(2) + center[1].pow(2) >= minimum.pow(2));
                    for dz in -radius..=radius {
                        for dx in -radius..=radius {
                            if dx != 0 || dz != 0 {
                                assert_ne!(candidate(seed, [x + dx, z + dz]), Some(kind));
                            }
                        }
                    }
                }
            }
            assert!(kingdoms > 0 && outposts > 0);
            assert!((kingdoms + outposts) * 3 < old);
        }
    }
    #[test]
    fn biome_spawn_exclusion_is_seeded_and_disabled_features_do_not_reserve_space() {
        let mut config = terra_core::WorldConfig {
            structures_revision: 8,
            biomes: false,
            ..Default::default()
        };
        let p = (-20..=20)
            .flat_map(|z| (-20..=20).map(move |x| [x, z]))
            .find(|&p| candidate(config.seed.0, p) == Some(StructureKind::Kingdom))
            .unwrap();
        let center = p.map(|v| v * REGION_SIZE + REGION_SIZE / 2);
        let generator = TerraGenerator::new(config.clone()).unwrap();
        assert!(!generator.settlement_start_clear(center));
        config.structures.kingdoms = false;
        config.structures.outposts = false;
        assert!(
            TerraGenerator::new(config)
                .unwrap()
                .settlement_start_clear(center)
        );
    }

    #[test]
    fn the_real_spawn_search_leaves_a_kingdom_buffer_even_with_a_biome_preference() {
        let config = terra_core::WorldConfig {
            structures_revision: 8,
            preferred_spawn_biome: Some(terra_core::BiomeKind::Forest),
            biomes: false,
            trees: false,
            water: false,
            caves: false,
            landscape_revision: 0,
            geology_revision: 0,
            ocean_revision: 0,
            base_height: 16.0,
            hill_amplitude: 0.0,
            mountain_amplitude: 0.0,
            ..Default::default()
        };
        let p = (-20..=20)
            .flat_map(|z| (-20..=20).map(move |x| [x, z]))
            .find(|&p| candidate(config.seed.0, p) == Some(StructureKind::Kingdom))
            .unwrap();
        let center = p.map(|v| v * REGION_SIZE + REGION_SIZE / 2);
        let terrain = TerraGenerator::new(config).unwrap();
        let spawn =
            crate::spawn::find_land_spawn(&terrain, &crate::edits::EditJournal::default(), center)
                .unwrap();
        let [x, _, z] = spawn.ground;
        assert!((x - center[0]).pow(2) + (z - center[1]).pow(2) >= KINGDOM_START_CLEARANCE.pow(2));
        assert!(spawn.is_valid(&terrain, &crate::edits::EditJournal::default()));
    }
}
