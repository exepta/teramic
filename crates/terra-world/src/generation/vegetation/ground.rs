use super::{mix, random};
use crate::{
    SurfaceColumn, TerraGenerator,
    generation::biomes::BiomeWeights,
    props::{PropId, PropKind, WorldProp},
};
use terra_voxel::{MaterialId, SectionPos};

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct GroundCover {
    pub grass: f32,
    pub bushes: f32,
    pub flowers: f32,
    pub ferns: f32,
    pub path_clearance: f32,
}

impl TerraGenerator {
    pub fn ground_cover(&self, x: f64, z: f64) -> GroundCover {
        let biome = self.biome_at(x, z).weights;
        self.ground_cover_with_biome(x, z, biome)
    }

    pub(crate) fn ground_cover_with_biome(
        &self,
        x: f64,
        z: f64,
        biome: BiomeWeights,
    ) -> GroundCover {
        let (patch, path) = self.climate.ground_masks(x, z);
        let [glade, meadow, shrubs, tufts] = self.climate.vegetation_masks(x, z);
        let land = (1.0 - biome.ocean - biome.desert - biome.cliffted_desert).clamp(0.0, 1.0);
        let cover = ((0.24 + patch * 0.76) * path * land * glade * (0.65 + tufts * 0.55)) as f32;
        GroundCover {
            grass: (cover * (0.88 + biome.plains as f32 * 0.12)).clamp(0.0, 1.0),
            bushes: cover * shrubs as f32 * (0.18 + biome.forest as f32 * 0.82),
            flowers: cover
                * meadow as f32
                * (0.12 + biome.plains as f32 * 0.88)
                * (1.0 - biome.mushroom_park as f32 * 0.8),
            ferns: cover * biome.forest as f32 * (0.35 + shrubs as f32 * 0.65),
            path_clearance: (path * glade) as f32,
        }
    }

    pub(crate) fn ground_prop(
        &self,
        support: [i64; 3],
        section: SectionPos,
        material: u16,
        cover: GroundCover,
        column: SurfaceColumn,
    ) -> Option<WorldProp> {
        let mut seed = mix(self.config().seed.0 ^ 0x5072_6f70_5465_7272);
        for coordinate in support {
            seed = mix(seed ^ (coordinate as u64).wrapping_mul(0x9e3779b97f4a7c15));
        }
        let level = column.water_level.unwrap_or_else(|| {
            if column.oasis_moisture > 0.0 {
                self.oasis_at(support[0] as f64 + 0.5, support[2] as f64 + 0.5)
                    .map_or(self.config().sea_level, |oasis| oasis.level)
            } else {
                self.river_at(support[0] as f64 + 0.5, support[2] as f64 + 0.5)
                    .map_or(self.config().sea_level, |river| river.level)
            }
        });
        let clearance = support[1] as f64 + 1.0 - level;
        let underwater = self.config().water && clearance <= 0.0;
        if underwater {
            return self.aquatic_prop(support, section, material, column, -clearance);
        }
        let ticket = random(seed, 1);
        let species = random(seed, 2);
        let desert = column.biome.aridity() > 0.5;
        let park = column.biome.weights.mushroom_park as f32;
        let reed = self.config().water
            && (0.0..=2.5).contains(&clearance)
            && ticket < cover.path_clearance * 0.16;
        let kind = if desert
            && !underwater
            && [MaterialId::Grass as u16, MaterialId::Sand as u16].contains(&material)
        {
            if column.oasis_moisture > 0.12 && ticket < 0.12 + column.oasis_moisture as f32 * 0.35 {
                PropKind::Grass
            } else if ticket < 0.045 {
                PropKind::Bush
            } else {
                return None;
            }
        } else if material == MaterialId::Grass as u16 && !underwater {
            if ticket >= (cover.grass * 0.95).clamp(0.0, 0.95) {
                return None;
            }
            // The same continuous ground masks drive all species and preserve wide clearings.
            let bush = cover.bushes * 0.42;
            let fern = cover.ferns * 0.34;
            let flower = (cover.flowers * 0.95 + cover.grass * 0.025).min(0.88 - bush - fern);
            if species > 1.0 - park * 0.80 {
                PropKind::Mushroom
            } else if species < bush {
                PropKind::Bush
            } else if species < bush + fern {
                PropKind::Fern
            } else if species < bush + fern + flower {
                PropKind::Flower
            } else if species > 1.0 - cover.ferns * 0.07 {
                PropKind::Mushroom
            } else {
                PropKind::Grass
            }
        } else if material == MaterialId::Sand as u16 || material == MaterialId::Stone as u16 {
            if reed || (ticket < 0.035 && (!underwater || clearance >= -4.0)) {
                PropKind::SmallPlant
            } else {
                return None;
            }
        } else {
            return None;
        };
        let stone = kind == PropKind::SmallPlant && !reed;
        let scale = if desert {
            0.55 + random(seed, 6) * 0.32
        } else if kind == PropKind::Grass {
            0.64 + random(seed, 6) * 0.40
        } else {
            0.58 + random(seed, 6) * 0.36
        };
        let variant = if (kind == PropKind::Mushroom && park > 0.15)
            || (kind == PropKind::Flower && self.config().biomes && random(seed, 8) < 0.82)
        {
            let patch = mix(self.config().seed.0
                ^ (support[0].div_euclid(24) as u64).wrapping_mul(0x9e3779b97f4a7c15)
                ^ (support[2].div_euclid(24) as u64).wrapping_mul(0xbf58476d1ce4e5b9));
            let colors = if kind == PropKind::Mushroom { 6.0 } else { 8.0 };
            (random(patch, 7) * colors) as u8
        } else {
            (random(seed, 7) * 8.0) as u8 + if stone || desert { 128 } else { 0 }
        };
        let mut prop = WorldProp {
            id: PropId(seed),
            kind,
            position: [
                support[0] as f64 + 0.5,
                support[1] as f64 + 1.0,
                support[2] as f64 + 0.5,
            ],
            rotation: random(seed, 5) * std::f32::consts::TAU,
            scale,
            variant,
            section,
            removed: false,
        };
        let bounds = prop.placement_bounds();
        let radius = (bounds[1][0] - bounds[0][0]) * 0.5;
        let travel = (0.5 - radius - 0.025).max(0.0);
        prop.position[0] += f64::from(random(seed, 3) * 2.0 - 1.0) * travel;
        prop.position[2] += f64::from(random(seed, 4) * 2.0 - 1.0) * travel;
        Some(prop)
    }

    pub(crate) fn ground_props(
        &self,
        support: [i64; 3],
        section: SectionPos,
        material: u16,
        cover: GroundCover,
        column: SurfaceColumn,
    ) -> [Option<WorldProp>; 4] {
        let mut props = self.ground_prop_candidates(support, section, material, cover, column);
        // Thin after choosing every companion so an omitted primary never removes
        // unrelated species or promotes a replacement into the vacant slot.
        for prop in &mut props {
            if prop.as_ref().is_some_and(|prop| {
                matches!(prop.kind, PropKind::Flower | PropKind::Mushroom)
                    && mix(prop.id.0 ^ 0x7468_696e_5072_6f70) & 3 != 0
            }) {
                *prop = None;
            }
        }
        crate::props::placement::reflow_group(props.iter_mut().flatten());
        props
    }

    fn ground_prop_candidates(
        &self,
        support: [i64; 3],
        section: SectionPos,
        material: u16,
        cover: GroundCover,
        column: SurfaceColumn,
    ) -> [Option<WorldProp>; 4] {
        let mut props = [None; 4];
        let Some(mut primary) = self.ground_prop(support, section, material, cover, column) else {
            return props;
        };
        let seed = primary.id.0;
        if primary.kind.aquatic() {
            props[0] = Some(primary);
            return props;
        }
        let grass = primary.kind == PropKind::Grass;
        let desert = column.biome.aridity() > 0.5;
        let park = column.biome.weights.mushroom_park as f32;
        let strength = (cover.grass * 0.42 + cover.flowers * 0.12 + park * 0.08).clamp(0.0, 0.55)
            * if desert { 0.25 } else { 1.0 };
        let slots = if grass { 3 } else { 4 };
        for (slot, target) in props.iter_mut().enumerate().take(slots).skip(1) {
            // Slot identities do not depend on accepted neighbors or later removals.
            let companion_seed = mix(seed ^ (slot as u64).wrapping_mul(0x736c_6f74_5072_6f70));
            if random(companion_seed, 1) >= strength * (1.0 - (slot - 1) as f32 * 0.16) {
                continue;
            }
            let kind = companion_kind(companion_seed, primary.kind, cover, park, desert);
            let mut companion = primary;
            companion.id = PropId(companion_seed);
            companion.kind = kind;
            companion.scale = 0.58 + random(companion_seed, 6) * 0.36;
            companion.variant = if kind == primary.kind {
                primary.variant
            } else {
                (random(companion_seed, 7) * if kind == PropKind::Mushroom { 6.0 } else { 8.0 })
                    as u8
                    + if desert { 128 } else { 0 }
            };
            crate::props::placement::align(
                &mut companion,
                support,
                crate::props::placement::generated_slot(if grass { slot - 1 } else { slot }),
            );
            *target = Some(companion);
        }
        if grass {
            primary.scale = if desert { 0.88 } else { 0.96 } + random(seed, 6) * 0.06;
        }
        crate::props::placement::align(&mut primary, support, 0);
        props[0] = Some(primary);
        props
    }
}

fn companion_kind(
    seed: u64,
    primary: PropKind,
    cover: GroundCover,
    park: f32,
    desert: bool,
) -> PropKind {
    if desert {
        return PropKind::Bush;
    }
    if primary == PropKind::SmallPlant || (primary != PropKind::Grass && random(seed, 2) < 0.65) {
        return primary;
    }
    let weights = [
        (PropKind::Flower, 0.08 + cover.flowers * 0.9),
        (PropKind::Fern, cover.ferns * 0.8),
        (PropKind::Bush, cover.bushes * 0.5),
        (PropKind::Mushroom, park * 1.2 + cover.ferns * 0.08),
    ];
    let mut ticket = random(seed, 3) * weights.iter().map(|(_, weight)| weight).sum::<f32>();
    for (kind, weight) in weights {
        if ticket < weight {
            return kind;
        }
        ticket -= weight;
    }
    PropKind::Flower
}

pub(crate) fn understory_cover(
    trees: &[super::Tree],
    global: [i64; 3],
    mut cover: GroundCover,
) -> GroundCover {
    let shade = trees
        .iter()
        .filter(|tree| {
            [0, 2].into_iter().all(|axis| {
                let local = global[axis] - tree.origin[axis];
                local >= i64::from(tree.bounds.min[axis])
                    && local < i64::from(tree.bounds.max[axis])
            })
        })
        .map(|tree| {
            let dx = (global[0] - tree.origin[0]) as f32;
            let dz = (global[2] - tree.origin[2]) as f32;
            (1.0 - (dx * dx + dz * dz) / 64.0).clamp(0.0, 1.0)
        })
        .fold(0.0_f32, f32::max);
    cover.ferns = (cover.ferns * (1.0 + shade * 1.1)).min(0.9);
    cover.flowers *= 1.0 - shade * 0.85;
    cover.bushes *= 1.0 - shade * 0.55;
    cover
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SectionGenerator, generation::sections::BlockOverrides};
    use std::collections::HashSet;
    use terra_core::WorldConfig;

    fn generator() -> SectionGenerator {
        SectionGenerator::new(
            TerraGenerator::new(WorldConfig {
                base_height: 16.0,
                hill_amplitude: 0.0,
                mountain_amplitude: 0.0,
                water: false,
                caves: false,
                trees: false,
                biomes: false,
                ..WorldConfig::default()
            })
            .unwrap(),
        )
    }

    #[test]
    fn logical_props_are_deterministic_unique_and_equal_in_headless_generation() {
        let generator = generator();
        let mut ids = HashSet::new();
        let mut count = 0;
        for coordinate in [[-3, -2], [-1, -1], [0, 0], [1, 2]] {
            let position = SectionPos::new(coordinate[0], 0, coordinate[1]);
            let client = generator
                .build(position, 0, &BlockOverrides::new(), true)
                .unwrap();
            let server = generator
                .build(position, 0, &BlockOverrides::new(), false)
                .unwrap();
            let reloaded = generator
                .build(position, 0, &BlockOverrides::new(), true)
                .unwrap();
            assert_eq!(client.props, server.props);
            assert_eq!(client.props, reloaded.props);
            for prop in client.props {
                count += 1;
                assert!(ids.insert(prop.id));
                assert_eq!(prop.section, position);
                assert_eq!(prop.position[1], 16.0);
                let [minimum, maximum] = prop.placement_bounds();
                for axis in [0, 2] {
                    assert!(minimum[axis] >= prop.position[axis].floor());
                    assert!(maximum[axis] <= prop.position[axis].floor() + 1.0);
                }
            }
        }
        assert!(count > 10);
    }

    #[test]
    fn edited_support_and_obstruction_remove_only_affected_candidates() {
        let generator = generator();
        let section = SectionPos::new(-1, 0, -1);
        let original = generator
            .build(section, 0, &BlockOverrides::new(), true)
            .unwrap();
        let prop = original.props.first().unwrap();
        let support = [
            prop.position[0].floor() as i64,
            15,
            prop.position[2].floor() as i64,
        ];
        let affected = original
            .props
            .iter()
            .filter(|candidate| crate::props::placement::support(candidate) == support)
            .count();
        for (position, material) in [
            (support, 0),
            ([support[0], 16, support[2]], MaterialId::Wood as u16),
        ] {
            let edits = BlockOverrides::from([(position, material)]);
            let changed = generator.build(section, 1, &edits, true).unwrap();
            assert!(
                !changed
                    .props
                    .iter()
                    .any(|candidate| candidate.id == prop.id)
            );
            assert_eq!(changed.props.len(), original.props.len() - affected);
            assert!(
                changed
                    .props
                    .iter()
                    .all(|candidate| original.props.contains(candidate))
            );
        }
    }

    #[test]
    fn clearing_tree_bases_never_creates_new_props_including_across_section_ceiling() {
        let generator = SectionGenerator::new(
            TerraGenerator::new(WorldConfig {
                base_height: 16.0,
                hill_amplitude: 0.0,
                mountain_amplitude: 0.0,
                water: false,
                caves: false,
                biomes: false,
                trees: true,
                ..WorldConfig::default()
            })
            .unwrap(),
        );
        let mut cleared_tree_cells = 0;
        for z in -2..=2 {
            for x in -2..=2 {
                let section = SectionPos::new(x, 0, z);
                let upper = generator.generate(SectionPos::new(x, 1, z)).unwrap();
                let mut edits = BlockOverrides::new();
                for local_z in 0..16 {
                    for local_x in 0..16 {
                        if matches!(upper.block([local_x, 0, local_z]).unwrap(), 6..=10) {
                            edits.insert([x * 16 + local_x as i64, 16, z * 16 + local_z as i64], 0);
                        }
                    }
                }
                if edits.is_empty() {
                    continue;
                }
                cleared_tree_cells += edits.len();
                for with_mesh in [false, true] {
                    let before = generator
                        .build(section, 0, &BlockOverrides::new(), with_mesh)
                        .unwrap();
                    let mined = generator.build(section, 1, &edits, with_mesh).unwrap();
                    assert_eq!(
                        before.props, mined.props,
                        "cleared tree at {section:?} must not sprout vegetation"
                    );
                    let reloaded = generator.build(section, 1, &edits, with_mesh).unwrap();
                    assert_eq!(reloaded.props, before.props);
                }
            }
        }
        assert!(cleared_tree_cells > 10, "exercise actual tree obstructions");
    }

    #[test]
    fn replacing_natural_support_does_not_generate_a_new_plant_kind() {
        let generator = generator();
        let section = SectionPos::new(-1, 0, -1);
        let before = generator
            .build(section, 0, &BlockOverrides::new(), true)
            .unwrap();
        let edits: BlockOverrides = (0..16)
            .flat_map(|z| (0..16).map(move |x| ([-16 + x, 15, -16 + z], 3)))
            .collect();
        let changed = generator.build(section, 1, &edits, true).unwrap();
        assert!(!before.props.is_empty());
        assert!(
            changed.props.is_empty(),
            "stone replacements cannot invent pebbles or shore plants"
        );
    }

    #[test]
    fn support_ids_stay_compatible_when_groups_replace_anchor_jitter() {
        let generator = generator();
        let cover = GroundCover {
            grass: 1.0,
            bushes: 0.0,
            flowers: 0.0,
            ferns: 0.0,
            path_clearance: 1.0,
        };
        for (support, expected) in [
            ([-1, 15, -1], 9_596_517_175_111_475_306),
            ([15, 15, 16], 7_043_264_594_181_193_183),
            ([-17, 0, -17], 9_575_492_046_619_981_186),
        ] {
            let terrain = generator.terrain();
            let section = terra_voxel::global_to_section(support).0;
            let column = terrain.surface_column(support[0] as f64 + 0.5, support[2] as f64 + 0.5);
            let original = terrain
                .ground_prop(support, section, MaterialId::Grass as u16, cover, column)
                .unwrap();
            let props =
                terrain.ground_props(support, section, MaterialId::Grass as u16, cover, column);
            assert_eq!(original.id, PropId(expected));
            if let Some(primary) = props[0] {
                assert_eq!(primary.id, original.id);
                assert_eq!(primary.kind, original.kind);
                assert_eq!(primary.variant, original.variant);
            } else {
                assert!(matches!(
                    original.kind,
                    PropKind::Flower | PropKind::Mushroom
                ));
            }
            for prop in props.into_iter().flatten() {
                assert!(prop.valid());
                assert_eq!(crate::props::placement::support(&prop), support);
                for axis in [0, 2] {
                    let offset = (prop.position[axis] - support[axis] as f64 - 0.5).abs();
                    if prop.kind == PropKind::Grass {
                        assert_eq!(offset, 0.0);
                    } else {
                        assert!(offset <= 0.25001);
                        let [low, high] = prop.bounds();
                        assert!(low[axis] >= support[axis] as f64 - 0.00001);
                        assert!(high[axis] <= support[axis] as f64 + 1.00001);
                    }
                }
            }
        }
    }

    #[test]
    fn grouped_props_use_four_units_with_centered_grass_and_stable_ids() {
        let generator = generator();
        let terrain = generator.terrain();
        let cover = GroundCover {
            grass: 1.0,
            bushes: 0.3,
            flowers: 0.4,
            ferns: 0.3,
            path_clearance: 1.0,
        };
        let column = terrain.surface_column(0.5, 0.5);
        let mut ids = HashSet::new();
        let mut four = 0;
        let mut grass_with_two = 0;
        let mut generated = 0;
        let mut occupied = 0;
        for z in -32..32 {
            for x in -32..32 {
                let support = [x, 15, z];
                let section = terra_voxel::global_to_section(support).0;
                let props =
                    terrain.ground_props(support, section, MaterialId::Grass as u16, cover, column);
                assert_eq!(
                    props,
                    terrain.ground_props(support, section, MaterialId::Grass as u16, cover, column)
                );
                let props: Vec<_> = props.into_iter().flatten().collect();
                if props.is_empty() {
                    continue;
                }
                occupied += 1;
                generated += props.len();
                let primary = terrain
                    .ground_prop(support, section, MaterialId::Grass as u16, cover, column)
                    .unwrap();
                if let Some(retained) = props.iter().find(|prop| prop.id == primary.id) {
                    assert_eq!(retained.kind, primary.kind);
                    assert_eq!(retained.variant, primary.variant);
                }
                assert!(
                    props
                        .iter()
                        .map(|prop| crate::props::placement::cost(prop.kind))
                        .sum::<usize>()
                        <= 4
                );
                let grass: Vec<_> = props
                    .iter()
                    .filter(|prop| prop.kind == PropKind::Grass)
                    .collect();
                assert!(grass.len() <= 1);
                four += usize::from(props.len() == 4);
                grass_with_two += usize::from(props.len() == 3 && grass.len() == 1);
                let lone = props.len() == 1;
                let has_grass = !grass.is_empty();
                for prop in props {
                    assert!(prop.valid());
                    assert!(ids.insert(prop.id));
                    assert_eq!(crate::props::placement::support(&prop), support);
                    assert_eq!(prop.section, section);
                    if prop.kind == PropKind::Grass {
                        assert_eq!(prop.position, [x as f64 + 0.5, 16.0, z as f64 + 0.5]);
                        assert!((0.96..=1.02).contains(&prop.scale));
                    } else {
                        let [low, high] = prop.bounds();
                        for axis in [0, 2] {
                            assert!(low[axis] >= support[axis] as f64 - 0.00001);
                            assert!(high[axis] <= support[axis] as f64 + 1.00001);
                            assert!(high[axis] - low[axis] <= 0.50001);
                            let offset = (prop.position[axis] - support[axis] as f64 - 0.5).abs();
                            assert!(offset <= 0.25001);
                            if lone {
                                assert_eq!(offset, 0.0);
                            } else if has_grass {
                                assert_eq!(offset, 0.25);
                            }
                        }
                        match prop.kind {
                            PropKind::Flower => {
                                assert!((0.64999..=0.95001).contains(&prop.visual_scale()))
                            }
                            PropKind::Mushroom => {
                                assert!((0.34999..=0.55001).contains(&prop.visual_scale()))
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
        assert!(four > 20, "four-slot groups={four}");
        assert!(
            grass_with_two > 20,
            "grass with both companions={grass_with_two}"
        );
        assert!(
            generated < occupied * 5 / 2,
            "dense patches must not quadruple every candidate"
        );
    }

    #[test]
    fn natural_flowers_and_mushrooms_keep_one_quarter_without_replacing_neighbors() {
        let generator = generator();
        let terrain = generator.terrain();
        let mut totals = [[0_usize; 2]; 2];
        let mut surviving_companions = 0;
        for (name, forest, plains, park, cover) in [
            (
                "forest",
                1.0,
                0.0,
                0.0,
                GroundCover {
                    grass: 0.95,
                    bushes: 0.35,
                    flowers: 0.08,
                    ferns: 0.7,
                    path_clearance: 1.0,
                },
            ),
            (
                "plains",
                0.0,
                1.0,
                0.0,
                GroundCover {
                    grass: 0.95,
                    bushes: 0.08,
                    flowers: 0.65,
                    ferns: 0.2,
                    path_clearance: 1.0,
                },
            ),
            (
                "mushroom park",
                0.0,
                0.0,
                1.0,
                GroundCover {
                    grass: 0.95,
                    bushes: 0.1,
                    flowers: 0.08,
                    ferns: 0.2,
                    path_clearance: 1.0,
                },
            ),
        ] {
            let mut column = terrain.surface_column(0.5, 0.5);
            column.biome.weights = BiomeWeights {
                ocean: 0.0,
                plains,
                forest,
                desert: 0.0,
                highland: 0.0,
                mushroom_park: park,
                cliffted_desert: 0.0,
            };
            let mut counts = [[0_usize; 2]; 2];
            for z in -64..64 {
                for x in -64..64 {
                    let support = [x, 15, z];
                    let section = terra_voxel::global_to_section(support).0;
                    let before = terrain.ground_prop_candidates(
                        support,
                        section,
                        MaterialId::Grass as u16,
                        cover,
                        column,
                    );
                    let after = terrain.ground_props(
                        support,
                        section,
                        MaterialId::Grass as u16,
                        cover,
                        column,
                    );
                    assert_eq!(
                        after,
                        terrain.ground_props(
                            support,
                            section,
                            MaterialId::Grass as u16,
                            cover,
                            column
                        )
                    );
                    surviving_companions += usize::from(
                        before[0].is_some()
                            && after[0].is_none()
                            && after.iter().flatten().any(|prop| {
                                !matches!(prop.kind, PropKind::Flower | PropKind::Mushroom)
                            }),
                    );
                    for (candidate, retained) in before.into_iter().zip(after) {
                        let Some(candidate) = candidate else {
                            assert!(retained.is_none(), "vacant slots must not be refilled");
                            continue;
                        };
                        let kind = match candidate.kind {
                            PropKind::Flower => Some(0),
                            PropKind::Mushroom => Some(1),
                            _ => None,
                        };
                        if let Some(kind) = kind {
                            counts[kind][0] += 1;
                            counts[kind][1] += usize::from(retained.is_some());
                        } else {
                            assert!(retained.is_some(), "other species must remain present");
                        }
                        if let Some(retained) = retained {
                            assert_eq!(
                                WorldProp {
                                    position: candidate.position,
                                    ..retained
                                },
                                candidate,
                                "only the group layout may change after thinning"
                            );
                        }
                    }
                }
            }
            for (kind, counts) in counts.into_iter().enumerate() {
                totals[kind][0] += counts[0];
                totals[kind][1] += counts[1];
                if counts[0] >= 100 {
                    let ratio = counts[1] as f64 / counts[0] as f64;
                    assert!((0.18..=0.32).contains(&ratio), "{name}: {counts:?}");
                }
                eprintln!("{name} kind {kind}: {} -> {}", counts[0], counts[1]);
            }
        }
        for counts in totals {
            let ratio = counts[1] as f64 / counts[0] as f64;
            assert!((0.23..=0.27).contains(&ratio), "overall: {counts:?}");
        }
        assert!(surviving_companions > 100);
    }

    #[test]
    fn removing_one_group_member_survives_reload_without_replacing_its_neighbors() {
        let generator = generator();
        let section = SectionPos::new(-1, 0, -1);
        let original = generator
            .build(section, 0, &BlockOverrides::new(), false)
            .unwrap();
        let prop = original
            .props
            .iter()
            .find(|prop| {
                original
                    .props
                    .iter()
                    .filter(|neighbor| {
                        crate::props::placement::support(neighbor)
                            == crate::props::placement::support(prop)
                    })
                    .count()
                    > 1
            })
            .unwrap();
        let mut journal = crate::edits::EditJournal::default();
        journal.remove_prop(section, prop.id).unwrap();
        let mut restored = crate::edits::EditJournal::default();
        restored
            .import_delta(journal.section_delta(section))
            .unwrap();
        let mut reloaded = generator
            .build(section, 0, &BlockOverrides::new(), false)
            .unwrap()
            .props;
        restored.apply_prop_removals(section, &mut reloaded);
        assert_eq!(reloaded.len(), original.props.len());
        for before in &original.props {
            let after = reloaded.iter().find(|after| after.id == before.id).unwrap();
            assert_eq!(after.removed, after.id == prop.id);
            assert_eq!(
                WorldProp {
                    removed: false,
                    position: before.position,
                    ..*after
                },
                *before
            );
            assert_eq!(
                crate::props::placement::support(before),
                crate::props::placement::support(after)
            );
            if crate::props::placement::support(before) != crate::props::placement::support(prop) {
                assert_eq!(after.position, before.position);
            }
        }
        let first_reload = reloaded.clone();
        restored.apply_prop_removals(section, &mut reloaded);
        assert_eq!(reloaded, first_reload);
    }

    #[test]
    fn ecology_creates_dense_patches_wide_clearings_and_correlated_species_groups() {
        let generator = generator();
        let terrain = generator.terrain();
        let forest = BiomeWeights {
            forest: 1.0,
            ocean: 0.0,
            plains: 0.0,
            desert: 0.0,
            highland: 0.0,
            mushroom_park: 0.0,
            cliffted_desert: 0.0,
        };
        let mut dense = 0;
        let mut clear = 0;
        let mut nearby = 0.0;
        let mut distant = 0.0;
        let mut dense_ratio = 0.0;
        let mut dense_samples = 0;
        for z in (-128..128).step_by(4) {
            for x in (-128..128).step_by(4) {
                let (x, z) = (f64::from(x), f64::from(z));
                let a = terrain.ground_cover_with_biome(x, z, forest);
                let b = terrain.ground_cover_with_biome(x + 1.0, z, forest);
                let c = terrain.ground_cover_with_biome(x + 23.0, z + 11.0, forest);
                nearby += (a.bushes - b.bushes).powi(2) + (a.flowers - b.flowers).powi(2);
                distant += (a.bushes - c.bushes).powi(2) + (a.flowers - c.flowers).powi(2);
                dense += usize::from(a.grass > 0.65);
                clear += usize::from(a.grass < 0.025);
                let (patch, path) = terrain.climate.ground_masks(x, z);
                if a.path_clearance > 0.95 && patch > 0.35 {
                    dense_ratio += f64::from(a.grass) / (patch * path * 0.65);
                    dense_samples += 1;
                }
            }
        }
        assert!(dense > 40 && clear > 40, "dense={dense}, clear={clear}");
        assert!(
            nearby < distant * 0.35,
            "neighbor={nearby}, distant={distant}"
        );
        assert!(dense_ratio / f64::from(dense_samples) > 1.25);
    }

    #[test]
    fn understory_favors_ferns_and_preserves_client_server_candidates() {
        let tree = super::super::Tree::new([0, 16, 0], 0, [0.0; 4]);
        let cover = GroundCover {
            grass: 0.7,
            bushes: 0.4,
            flowers: 0.4,
            ferns: 0.4,
            path_clearance: 1.0,
        };
        let shaded = understory_cover(&[tree], [2, 15, 2], cover);
        assert!(shaded.ferns > cover.ferns);
        assert!(shaded.flowers < cover.flowers);
        let mut config = generator().terrain().config().clone();
        config.trees = true;
        config.tree_density = 1.0;
        let generator = SectionGenerator::new(TerraGenerator::new(config).unwrap());
        for x in -1..=1 {
            let section = SectionPos::new(x, 0, -1);
            let client = generator
                .build(section, 0, &BlockOverrides::new(), true)
                .unwrap();
            let server = generator
                .build(section, 0, &BlockOverrides::new(), false)
                .unwrap();
            assert_eq!(client.props, server.props);
        }
    }

    #[test]
    fn desert_has_dead_bushes_and_dry_grass_only_in_the_oasis_margin() {
        let generator = generator();
        let terrain = generator.terrain();
        let cover = GroundCover {
            grass: 1.0,
            bushes: 1.0,
            flowers: 1.0,
            ferns: 1.0,
            path_clearance: 1.0,
        };
        let mut column = terrain.surface_column(0.5, 0.5);
        column.biome.weights = BiomeWeights {
            ocean: 0.0,
            plains: 0.0,
            forest: 0.0,
            desert: 1.0,
            highland: 0.0,
            mushroom_park: 0.0,
            cliffted_desert: 0.0,
        };
        let mut bushes = 0;
        let mut grasses = 0;
        for z in -16..16 {
            for x in -16..16 {
                let support = [x, 15, z];
                let section = terra_voxel::global_to_section(support).0;
                column.oasis_moisture = 0.0;
                if let Some(prop) =
                    terrain.ground_prop(support, section, MaterialId::Sand as u16, cover, column)
                {
                    assert_eq!(prop.kind, PropKind::Bush);
                    assert!(prop.variant >= 128);
                    bushes += 1;
                }
                column.oasis_moisture = 0.8;
                if let Some(prop) =
                    terrain.ground_prop(support, section, MaterialId::Sand as u16, cover, column)
                {
                    assert!(matches!(prop.kind, PropKind::Grass | PropKind::Bush));
                    assert!(prop.variant >= 128);
                    grasses += usize::from(prop.kind == PropKind::Grass);
                }
            }
        }
        assert!(bushes > 20 && grasses > 150);
    }

    #[test]
    fn mushroom_park_understory_forms_repeatable_colored_groups() {
        let generator = generator();
        let terrain = generator.terrain();
        let cover = GroundCover {
            grass: 1.0,
            bushes: 0.0,
            flowers: 0.0,
            ferns: 0.0,
            path_clearance: 1.0,
        };
        let mut column = terrain.surface_column(0.5, 0.5);
        column.biome.weights = BiomeWeights {
            ocean: 0.0,
            plains: 0.0,
            forest: 0.0,
            desert: 0.0,
            highland: 0.0,
            mushroom_park: 1.0,
            cliffted_desert: 0.0,
        };
        let mut colors = HashSet::new();
        let mut count = 0;
        for z in (-96..96).step_by(3) {
            for x in (-96..96).step_by(3) {
                let support = [x, 15, z];
                let section = terra_voxel::global_to_section(support).0;
                let prop =
                    terrain.ground_prop(support, section, MaterialId::Grass as u16, cover, column);
                assert_eq!(
                    prop,
                    terrain.ground_prop(support, section, MaterialId::Grass as u16, cover, column)
                );
                if let Some(prop) = prop.filter(|prop| prop.kind == PropKind::Mushroom) {
                    colors.insert(prop.variant);
                    count += 1;
                }
            }
        }
        assert_eq!(colors, HashSet::from([0, 1, 2, 3, 4, 5]));
        assert!(count > 2000);
    }
}
