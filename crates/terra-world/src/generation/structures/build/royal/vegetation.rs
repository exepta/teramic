//! Authored courtyard greenery uses ordinary voxels and harvestable world props.
use super::*;
use crate::props::WorldProp;
use terra_voxel::SectionPos;

impl Royal<'_, '_> {
    fn authored(&self, p: [i64; 3]) -> Option<u16> {
        let (owner, local) = global_to_section(self.point(p));
        let material = *self
            .builder
            .patches
            .get(&owner)?
            .get(local[0] + 16 * (local[1] + 16 * local[2]))?;
        (material != u16::MAX).then_some(material)
    }

    pub(super) fn plant_courtyard_trees(&mut self) {
        if self.builder.terrain.config().structures_revision < 8
            || !self.builder.terrain.config().trees
        {
            return;
        }
        let radius = if self.outpost { 25_i64 } else { 65_i64 };
        let mut candidates = Vec::new();
        for z in (-radius..=radius).step_by(7) {
            for x in (-radius..=radius).step_by(7) {
                if x * x + z * z > radius * radius {
                    continue;
                }
                let ticket = layer_seed(
                    u64::from(self.builder.seed)
                        ^ (x as u64).rotate_left(13)
                        ^ (z as u64).rotate_left(37),
                    0x6741_5244_454e,
                );
                candidates.push((ticket, x, z));
            }
        }
        candidates.sort_unstable();
        let mut planted = Vec::<[i64; 2]>::new();
        for (_, x, z) in candidates {
            if planted
                .iter()
                .any(|p| (p[0] - x).pow(2) + (p[1] - z).pow(2) < 11 * 11)
            {
                continue;
            }
            let y = self.floor(x, z);
            // Keep the entire future canopy off lanes, doors, buildings and
            // fences, including on the elevated winter terraces.
            if !(-2..=2).all(|dz| {
                (-2..=2).all(|dx| {
                    matches!(self.authored([x + dx, y - 1, z + dz]), Some(1 | 5))
                        && (0..=7).all(|dy| self.authored([x + dx, y + dy, z + dz]) == Some(0))
                })
            }) {
                continue;
            }
            if self.winter {
                for (level, r) in [(2, 2_i64), (3, 2), (4, 1), (5, 1), (6, 0)] {
                    for dz in -r..=r {
                        for dx in -r..=r {
                            if dx.abs() + dz.abs() <= r + 1 {
                                self.set([x + dx, y + level, z + dz], 7);
                            }
                        }
                    }
                }
                self.fill([x, y, z], [x, y + 5, z], 36);
                self.set([x, y + 7, z], 98);
            } else {
                let leaves = if (x + z).rem_euclid(3) == 0 { 8 } else { 7 };
                for level in 3..=6 {
                    let r = if level <= 4 { 2_i64 } else { 1 };
                    for dz in -r..=r {
                        for dx in -r..=r {
                            if dx.abs() + dz.abs() <= r + 1 {
                                self.set([x + dx, y + level, z + dz], leaves);
                            }
                        }
                    }
                }
                self.fill([x, y, z], [x, y + 4, z], 6);
            }
            planted.push([x, z]);
            if planted.len() >= if self.outpost { 3 } else { 10 } {
                break;
            }
        }
    }
}

impl Site {
    /// Only explicit free soil written by the authoring plan can acquire these
    /// props. Paths, fields, rooms and untouched natural terrain are excluded.
    pub(crate) fn garden_supports(
        &self,
        section: SectionPos,
    ) -> impl Iterator<Item = [i64; 3]> + '_ {
        self.sections
            .get(&section)
            .filter(|_| {
                matches!(
                    self.summary.kind,
                    StructureKind::Kingdom | StructureKind::Outpost
                )
            })
            .zip(section.origin().ok())
            .into_iter()
            .flat_map(move |(patch, origin)| {
                patch.cells.iter().filter_map(move |&(index, material)| {
                    if !matches!(material, 1 | 5) {
                        return None;
                    }
                    let i = i64::from(index);
                    let p = [
                        origin[0] + i % 16,
                        origin[1] + i / 16 % 16,
                        origin[2] + i / 256,
                    ];
                    let free = self.block_at([p[0], p[1] + 1, p[2]]) == Some(0)
                        && self.block_at([p[0], p[1] + 2, p[2]]) == Some(0);
                    let occupied = self.summary.buildings.iter().any(|b| {
                        [0, 2]
                            .into_iter()
                            .all(|a| (b.bounds[0][a] - 2..=b.bounds[1][a] + 2).contains(&p[a]))
                    });
                    (free && !occupied).then_some(p)
                })
            })
    }

    pub(crate) fn garden_props(
        &self,
        terrain: &TerraGenerator,
        support: [i64; 3],
        section: SectionPos,
    ) -> impl Iterator<Item = WorldProp> {
        let mut column = terrain.surface_column(support[0] as f64 + 0.5, support[2] as f64 + 0.5);
        column.height = support[1] as f64 + 1.0;
        column.water_level = None;
        let winter = self.summary.style == VillageStyle::Highland;
        let cover = crate::generation::GroundCover {
            grass: 0.72,
            bushes: 0.32,
            flowers: if winter { 0.08 } else { 0.65 },
            ferns: 0.25,
            path_clearance: 1.0,
        };
        terrain
            .ground_props(support, section, 1, cover, column)
            .into_iter()
            .flatten()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_core::{WorldConfig, WorldSeed};
    fn terrain(trees: bool, revision: u8, winter: bool) -> TerraGenerator {
        TerraGenerator::new(WorldConfig {
            seed: WorldSeed(42042),
            structures_revision: revision,
            base_height: 16.0,
            sea_level: 0.0,
            hill_amplitude: 0.0,
            mountain_amplitude: 0.0,
            landscape_revision: 0,
            geology_revision: 0,
            ocean_revision: 0,
            water: false,
            rivers: false,
            caves: false,
            biomes: false,
            trees,
            snow_height: if winter { 8.0 } else { 112.0 },
            ..Default::default()
        })
        .unwrap()
    }
    #[test]
    fn new_courtyards_have_real_trees_and_lanterns_without_touching_legacy_plans() {
        for (seed, winter) in [(522, false), (522, true), (261, false)] {
            let old = plan(&terrain(true, 7, winter), [0, 0], seed).unwrap();
            let new = plan(&terrain(true, 8, winter), [0, 0], seed).unwrap();
            let count = |site: &Site, m: u16| {
                site.sections
                    .values()
                    .flat_map(|p| &p.cells)
                    .filter(|(_, material)| *material == m)
                    .count()
            };
            assert_eq!(count(&old, 7) + count(&old, 8), 0);
            assert!(
                count(&new, 7) + count(&new, 8) > 10,
                "Both stronghold layouts need planted trees"
            );
            assert!(count(&old, 40) > 0);
            assert_eq!(count(&new, 40), 0);
            let lamps = new
                .sections
                .values()
                .flat_map(|p| &p.objects)
                .filter(|o| o.material == terra_core::lantern::LANTERN)
                .collect::<Vec<_>>();
            assert!(!lamps.is_empty());
            for lamp in lamps {
                let support = terra_core::lantern::support(
                    lamp.position,
                    lamp.material,
                    lamp.state.unwrap().facing,
                )
                .unwrap();
                assert!(new.block_at(support).is_some_and(|m| m != 0));
            }
            assert!(new.sections.len() <= MAX_SITE_SECTIONS);
            assert!(new.sections.values().map(|s| s.cells.len()).sum::<usize>() <= MAX_SITE_CELLS);
            let no_trees = plan(&terrain(false, 8, winter), [0, 0], seed).unwrap();
            assert_eq!(count(&no_trees, 7) + count(&no_trees, 8), 0);
        }
    }
    #[test]
    fn authored_gardens_generate_the_same_harvestable_props_with_or_without_mesh_and_respect_edits()
    {
        let terrain = terrain(false, 8, false);
        let site = std::sync::Arc::new(plan(&terrain, [0, 0], 522).unwrap());
        let (position, support) = site
            .sections
            .keys()
            .find_map(|&section| {
                site.garden_supports(section)
                    .find(|&p| site.garden_props(&terrain, p, section).next().is_some())
                    .map(|p| (section, p))
            })
            .unwrap();
        let generator = crate::SectionGenerator::new(terrain.clone());
        let build = |mesh, edits: &crate::generation::sections::BlockOverrides| {
            let mut column = generator
                .prepare_column([position.x, position.z], mesh)
                .unwrap();
            column.structures = vec![site.clone()];
            column.maximum = column.maximum.max(site.summary.bounds[1][1] as f64 + 2.0);
            generator
                .build_prepared(position, 0, edits, &column)
                .unwrap()
        };
        let plain = build(false, &Default::default());
        let meshed = build(true, &Default::default());
        assert_eq!(plain.props, meshed.props);
        let mut ids = std::collections::BTreeSet::new();
        for prop in &plain.props {
            assert!(prop.valid());
            assert!(ids.insert(prop.id));
        }
        assert!(
            plain
                .props
                .iter()
                .any(|p| crate::props::placement::support(p) == support)
        );
        let changed = build(false, &std::collections::HashMap::from([(support, 0)]));
        assert!(
            !changed
                .props
                .iter()
                .any(|p| crate::props::placement::support(p) == support)
        );
    }
}
