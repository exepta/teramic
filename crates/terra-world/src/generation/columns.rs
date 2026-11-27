mod falls;
use super::{
    MAX_SAMPLE_COORDINATE, SectionGenerationError, SectionGenerator, SurfaceColumn,
    vegetation::{Tree, trees_in_column},
};
use std::sync::Arc;
use terra_core::WorldConfig;
use terra_voxel::{SECTION_SIZE, SectionPos};

/// Immutable per-column data shared by all vertical worker jobs. No edits are cached here.
pub struct PreparedColumn {
    pub(crate) fall_columns: Vec<usize>,
    pub(super) config: Arc<WorldConfig>,
    pub(super) coordinate: [i64; 2],
    pub(super) padding: usize,
    pub(super) size: usize,
    pub(super) columns: Vec<SurfaceColumn>,
    pub(super) trees: Vec<Tree>,
    pub(super) tree_props: Vec<crate::props::WorldProp>,
    pub(super) structures: Vec<Arc<super::structures::Site>>,
    pub(super) natural_features: Vec<Arc<super::natural_features::NaturalPatch>>,
    pub(super) maximum: f64,
    pub(super) surface_sections: Option<[i64; 2]>,
    #[cfg(feature = "bevy")]
    pub(super) cave_eye_bounds: Option<[f64; 2]>,
    #[cfg(feature = "bevy")]
    pub(crate) gallery_skyline: bool,
}

#[cfg(all(test, feature = "bevy"))]
mod snow_tests {
    use super::*;
    use crate::generation::structures::{
        Site, SiteSummary, StructureKind, StructurePatch, VillageStyle,
    };
    use std::collections::BTreeMap;

    #[test]
    fn cached_plain_structure_paths_without_object_metadata_enter_the_snow_round_only_while_loaded()
    {
        let section = SectionPos::new(-1, 0, -2);
        let center = [-1, 4, -17];
        for material in [102, 78, 3] {
            let prepared = PreparedColumn {
                fall_columns: Vec::new(),
                config: Arc::default(),
                coordinate: [-1, -2],
                padding: 0,
                size: 0,
                columns: Vec::new(),
                trees: Vec::new(),
                tree_props: Vec::new(),
                natural_features: Vec::new(),
                structures: vec![Arc::new(Site {
                    tidal_nest: false,
                    summary: SiteSummary {
                        id: 1,
                        kind: StructureKind::Village,
                        style: VillageStyle::Temperate,
                        center,
                        bounds: [center, center],
                        buildings: Vec::new(),
                        containers: Vec::new(),
                        grave: None,
                    },
                    sections: BTreeMap::from([(
                        section,
                        StructurePatch {
                            cells: vec![(15 + 16 * (4 + 16 * 15), material)],
                            objects: Vec::new(),
                        },
                    )]),
                    settlement: None,
                })],
                maximum: 0.0,
                surface_sections: None,
                cave_eye_bounds: None,
                gallery_skyline: false,
            };
            assert_eq!(
                prepared.has_snow_coatable_structure(&[section]),
                crate::snow::coatable(material)
            );
            assert!(!prepared.has_snow_coatable_structure(&[]));
            assert!(!prepared.has_snow_coatable_structure(&[SectionPos::new(-1, 1, -2)]));
        }
    }
}

impl PreparedColumn {
    pub(super) fn matches_terrain(&self, terrain: &super::TerraGenerator) -> bool {
        Arc::ptr_eq(&self.config, &terrain.config) || self.config.same_terrain(terrain.config())
    }

    #[cfg(all(test, feature = "bevy"))]
    pub(crate) fn with_spruce_for_test(mut self, origin: [i64; 3], seed: u64) -> Self {
        self.trees = vec![super::vegetation::spruce_for_test(origin, seed)];
        self
    }
    pub(crate) fn is_structure_block(&self, cell: [i64; 3]) -> bool {
        self.structures
            .iter()
            .any(|site| site.block_at(cell).is_some())
    }

    /// Includes the air carved for the chamber and its passages. The space
    /// above the roof and neighbouring natural caves use their own atmosphere.
    pub fn in_dragon_lair(&self, cell: [i64; 3]) -> bool {
        self.dragon_lair_top(cell).is_some()
    }

    /// Upper search bound for a real voxel roof, including lairs built above
    /// the natural heightmap. Open passages still require an actual roof hit.
    pub fn dragon_lair_top(&self, cell: [i64; 3]) -> Option<i64> {
        self.structures
            .iter()
            .filter(|site| site.summary.kind.dragon().is_some() && site.block_at(cell).is_some())
            .map(|site| site.summary.bounds[1][1])
            .max()
    }

    #[cfg(test)]
    pub(crate) fn with_settlement_for_test(
        mut self,
        settlement: Arc<super::structures::VillageSettlement>,
    ) -> Self {
        use super::structures::{Site, SiteSummary, StructureKind};
        self.structures.push(Arc::new(Site {
            tidal_nest: false,
            summary: SiteSummary {
                id: settlement.id,
                kind: StructureKind::Village,
                style: settlement.style,
                center: settlement.center,
                bounds: settlement.bounds,
                buildings: Vec::new(),
                containers: Vec::new(),
                grave: None,
            },
            sections: Default::default(),
            settlement: Some(settlement),
        }));
        self
    }

    /// Read cached structural cells: village paths deliberately have no object
    /// metadata, but must still receive priority winter surface visits.
    #[cfg(feature = "bevy")]
    pub(crate) fn has_snow_coatable_structure(&self, sections: &[SectionPos]) -> bool {
        self.structures.iter().any(|site| {
            sections.iter().any(|section| {
                site.sections.get(section).is_some_and(|patch| {
                    patch
                        .cells
                        .iter()
                        .any(|(_, material)| crate::snow::coatable(*material))
                })
            })
        })
    }

    /// Cached immutable settlement metadata. Never plans regions or samples noise.
    pub fn village_settlements(
        &self,
    ) -> impl Iterator<Item = &Arc<super::structures::VillageSettlement>> {
        self.structures
            .iter()
            .filter_map(|site| site.settlement.as_ref())
    }
    pub fn village_residents(&self) -> impl Iterator<Item = &super::structures::ResidentSite> {
        self.village_settlements()
            .flat_map(|village| village.residents.iter())
    }
    pub fn structure_objects(
        &self,
        section: SectionPos,
    ) -> Vec<super::structures::StructureObject> {
        self.structures
            .iter()
            .filter_map(|site| site.sections.get(&section))
            .flat_map(|patch| patch.objects.iter().cloned())
            .chain(
                self.natural_features
                    .iter()
                    .flat_map(|patch| &patch.objects)
                    .filter(|object| terra_voxel::global_to_section(object.position).0 == section)
                    .cloned(),
            )
            .collect()
    }

    /// Authored poses for visual prediction, including the neighboring cells used
    /// to join stairs and crystal facets before their server sections arrive.
    pub fn model_objects_in_halo(
        &self,
        section: SectionPos,
    ) -> impl Iterator<Item = &super::structures::StructureObject> {
        let origin = section.origin().ok();
        let adjacent = [
            [0, 0, 0],
            [-1, 0, 0],
            [1, 0, 0],
            [0, -1, 0],
            [0, 1, 0],
            [0, 0, -1],
            [0, 0, 1],
        ];
        self.structures
            .iter()
            .flat_map(move |site| {
                adjacent
                    .into_iter()
                    .filter_map(move |offset| site.sections.get(&section.offset(offset).ok()?))
                    .flat_map(|patch| &patch.objects)
            })
            .chain(
                self.natural_features
                    .iter()
                    .flat_map(|patch| &patch.objects),
            )
            .filter(move |object| {
                object.state.is_some()
                    && origin.is_some_and(|start| {
                        (0..3).all(|axis| {
                            (start[axis].saturating_sub(1)..=start[axis].saturating_add(16))
                                .contains(&object.position[axis])
                        })
                    })
            })
    }
    #[cfg(feature = "bevy")]
    pub(crate) fn felling_species(&self, root: [i64; 3]) -> Option<terra_core::TreeSpecies> {
        self.trees
            .iter()
            .find(|tree| tree.origin == root)?
            .sapling_species()
    }
    #[cfg(feature = "bevy")]
    pub(crate) fn felling_materials(&self, root: [i64; 3]) -> Option<(u16, u16)> {
        let tree = self.trees.iter().find(|tree| tree.origin == root)?;
        Some((tree.wood_material as u16, tree.leaves_material as u16))
    }

    #[cfg(feature = "bevy")]
    pub(crate) fn felling_shape(&self, cell: [i64; 3]) -> Option<crate::tree_felling::Shape> {
        self.trees
            .iter()
            .find(|tree| {
                (matches!(
                    tree.wood_material as u16,
                    6 | 10 | 36 | 134 | 643 | 646 | 679 | 680
                ) || matches!(tree.leaves_material as u16, 606..=607))
                    && tree.wood_at(cell).is_some()
            })?
            .felling_shape()
    }

    #[cfg(feature = "bevy")]
    pub(crate) fn felling_foot(&self, cell: [i64; 3]) -> bool {
        self.trees.iter().any(|tree| {
            (matches!(
                tree.wood_material as u16,
                6 | 10 | 36 | 134 | 643 | 646 | 679 | 680
            ) || matches!(tree.leaves_material as u16, 606..=607))
                && tree.wood_at(cell).is_some()
        })
    }

    #[cfg(feature = "bevy")]
    pub(crate) fn felling_owner(&self, cell: [i64; 3], material: u16) -> Option<[i64; 3]> {
        // Section generation stamps every trunk before any crown. Match the
        // same spatial priority even where differently owned crowns overlap.
        if let Some(tree) = self.trees.iter().find(|tree| tree.wood_at(cell).is_some()) {
            return (tree.wood_material as u16 == material).then_some(tree.origin);
        }
        let mut crowns = self
            .trees
            .iter()
            .filter(|tree| tree.leaves_at(cell).is_some());
        let tree = crowns.next()?;
        // A neighbouring living tree retains shared crown cells.
        (terra_voxel::MaterialId::same_tree_material(material, tree.leaves_at(cell)? as u16)
            && crowns.next().is_none())
        .then_some(tree.origin)
    }

    /// Exact upper bound of generated terrain, water and trees in this prepared column.
    /// One sample per falling column at the listener's elevation. Fluid
    /// occupancy at the selected cell remains authoritative for sound.
    pub fn nearest_waterfall(&self, section: SectionPos, ear: [f64; 3]) -> Option<[i64; 3]> {
        let origin = section.origin().ok()?;
        let mut nearest = None;
        let mut distance = f64::INFINITY;
        for &i in &self.fall_columns {
            let x = origin[0] + (i % self.size) as i64 - self.padding as i64;
            let z = origin[2] + (i / self.size) as i64 - self.padding as i64;
            if x.div_euclid(16) != section.x || z.div_euclid(16) != section.z {
                continue;
            }
            for span in self.columns[i].falls.into_iter().flatten() {
                let low = i64::from(span.bottom).max(origin[1]);
                let high = (i64::from(span.top) - 1).min(origin[1] + 15);
                if low > high {
                    continue;
                }
                let y = (ear[1].floor() as i64).clamp(low, high);
                let point = [x, y, z];
                let d = (0..3)
                    .map(|j| (point[j] as f64 + 0.5 - ear[j]).powi(2))
                    .sum();
                if d < distance {
                    distance = d;
                    nearest = Some(point);
                }
            }
        }
        nearest
    }
    pub fn maximum_height(&self) -> f64 {
        self.maximum
    }
    pub fn local_surface(&self, x: usize, z: usize) -> Option<&SurfaceColumn> {
        if x >= 16 || z >= 16 {
            return None;
        }
        self.columns
            .get(x + self.padding + (z + self.padding) * self.size)
    }
    pub fn cave_structures_enabled(&self) -> bool {
        self.config.cave_structure_revision > 0
    }
    /// Visible room envelope, not the entire solid column above a cave player.
    /// All values are already prepared; streaming never resamples cave noise.
    #[cfg(feature = "bevy")]
    pub(crate) fn cave_view_sections(&self, eye_y: f64) -> Option<[i64; 2]> {
        if self
            .cave_eye_bounds
            .is_none_or(|[low, high]| eye_y < low || eye_y > high)
        {
            return None;
        }
        let mut envelope: Option<[f64; 2]> = None;
        for z in 0..16 {
            for x in 0..16 {
                let Some(column) = self.local_surface(x, z) else {
                    continue;
                };
                let room = column.cave;
                if !room.active()
                    || eye_y > column.height - 6.0
                    || eye_y < room.floor - 16.0
                    || eye_y > room.ceiling + 16.0
                {
                    continue;
                }
                let bounds = envelope.get_or_insert([room.floor, room.ceiling]);
                bounds[0] = bounds[0].min(room.floor);
                bounds[1] = bounds[1].max(room.ceiling);
            }
        }
        envelope.map(|[floor, ceiling]| {
            let bounds = [
                ((floor - 1.0).floor() as i64).div_euclid(16),
                ((ceiling + 1.0).ceil() as i64).div_euclid(16),
            ];
            if self.config.bounded_world {
                bounds.map(|y| y.clamp(-8, 22))
            } else {
                bounds
            }
        })
    }
    /// Read only the prepared tree primitives; this never generates new trees.
    pub fn tree_appearance_at(
        &self,
        global: [i64; 3],
        material: u16,
    ) -> Option<super::TreeAppearance> {
        self.trees.iter().find_map(|tree| {
            let generated = match material {
                6 | 10 | 36 | 134 | 313 | 405 | 643 | 646 | 679 | 680 => tree.wood_at(global),
                7..=9
                | 132
                | 133
                | 195
                | 310..=312
                | 606..=607
                | 406
                | 537..=540
                | 585..=586
                | 644
                | 647
                | 682
                | 683 => tree.leaves_at(global),
                _ => None,
            };
            generated
                .filter(|id| *id as u16 == material)
                .map(|_| super::TreeAppearance::of(tree))
        })
    }

    /// Apple roots already prepared for this loaded column; never generates or
    /// explores another region. Neighbor canopy overlap does not duplicate roots.
    pub fn natural_apples(&self) -> impl Iterator<Item = ([i64; 3], u64)> + '_ {
        self.trees
            .iter()
            .filter(|tree| {
                super::TreeAppearance::of(tree) == super::TreeAppearance::Apple
                    && [tree.origin[0].div_euclid(16), tree.origin[2].div_euclid(16)]
                        == self.coordinate
            })
            .map(|tree| {
                (
                    tree.origin,
                    crate::fruit::generation_seed(self.config.seed.0, tree.origin),
                )
            })
    }
    /// Only the mineral basin around a generated vent receives thermal-water
    /// optics. Nearby rivers and underground pools retain their own palette.
    pub fn geothermal_water(&self, pos: [f64; 3]) -> bool {
        self.natural_features
            .iter()
            .filter_map(|p| p.geyser)
            .any(|[x, y, z]| {
                let dy = pos[1] - y as f64;
                let dx = pos[0] - x as f64 - 0.5;
                let dz = pos[2] - z as f64 - 0.5;
                (-5.0..=0.5).contains(&dy) && dx * dx + dz * dz <= 81.
            })
    }
    /// Existing immutable biome and surface data; this never resamples noise.
    pub fn surface_at(&self, x: i64, z: i64) -> Option<&SurfaceColumn> {
        if [x.div_euclid(16), z.div_euclid(16)] != self.coordinate {
            return None;
        }
        self.columns.get(
            (z.rem_euclid(16) as usize + self.padding) * self.size
                + x.rem_euclid(16) as usize
                + self.padding,
        )
    }
    #[cfg(feature = "bevy")]
    pub(crate) fn water_columns(&self) -> &[SurfaceColumn] {
        &self.columns
    }
}

#[cfg(feature = "bevy")]
fn cave_eye_bounds(columns: &[SurfaceColumn], padding: usize, size: usize) -> Option<[f64; 2]> {
    let mut bounds: Option<[f64; 2]> = None;
    for z in padding..padding + SECTION_SIZE {
        for x in padding..padding + SECTION_SIZE {
            let column = &columns[x + z * size];
            let room = column.cave;
            if !room.active() {
                continue;
            }
            let low = room.floor - 16.0;
            let high = (column.height - 6.0).min(room.ceiling + 16.0);
            if low > high {
                continue;
            }
            let bounds = bounds.get_or_insert([low, high]);
            bounds[0] = bounds[0].min(low);
            bounds[1] = bounds[1].max(high);
        }
    }
    bounds
}

#[cfg(all(test, feature = "bevy"))]
mod felling_ownership_tests {
    use super::*;

    #[test]
    fn natural_savanna_trunks_resolve_owned_wood_for_felling() {
        let terrain = crate::TerraGenerator::new(WorldConfig {
            biome_definitions: Some(terra_core::BiomeDefinitions::default()),
            ..Default::default()
        })
        .unwrap();
        let roots = terrain.tree_appearance([-96, 0, 1152]);
        let (foot, _) = roots
            .tree_roots()
            .find(|(_, kind)| {
                matches!(
                    kind,
                    super::super::TreeAppearance::Acacia | super::super::TreeAppearance::Baobab
                )
            })
            .unwrap();
        let column = SectionGenerator::new(terrain)
            .prepare_column([foot[0].div_euclid(16), foot[2].div_euclid(16)], false)
            .unwrap();
        assert!(column.felling_foot(foot));
        let shape = column.felling_shape(foot).unwrap();
        assert_eq!(shape.origin, foot);
        assert_eq!(column.felling_owner(foot, shape.wood_material), Some(foot));
    }

    #[test]
    fn tree_attribution_preserves_neighbor_wood_and_shared_crowns_in_generation_priority() {
        let terrain = crate::TerraGenerator::new(WorldConfig {
            trees: false,
            caves: false,
            water: false,
            rivers: false,
            biomes: false,
            ..Default::default()
        })
        .unwrap();
        let mut column = SectionGenerator::new(terrain)
            .prepare_column([0, 0], false)
            .unwrap();
        let first = Tree::new([0, 12, 0], 42, [0.0; 4]);
        let second = Tree::new([2, 12, 0], 42, [0.0; 4]);
        let a = first.felling_shape().unwrap();
        let b = second.felling_shape().unwrap();
        column.trees = vec![first, second];
        let shared_wood = a.wood.iter().find(|p| b.wood.contains(p)).unwrap();
        assert_eq!(
            column.felling_owner(*shared_wood, a.wood_material),
            Some(a.origin)
        );
        let neighbor = b.wood.iter().find(|p| !a.wood.contains(p)).unwrap();
        assert_eq!(
            column.felling_owner(*neighbor, b.wood_material),
            Some(b.origin)
        );
        let shared_leaves = a
            .leaves
            .iter()
            .find(|p| {
                b.leaves.contains(p) && column.trees.iter().all(|tree| tree.wood_at(**p).is_none())
            })
            .unwrap();
        assert_eq!(
            column.felling_owner(*shared_leaves, a.leaves_material),
            None
        );
    }
}

impl SectionGenerator {
    pub fn prepare_column(
        &self,
        coordinate: [i64; 2],
        with_mesh: bool,
    ) -> Result<PreparedColumn, SectionGenerationError> {
        let p = SectionPos::new(coordinate[0], 0, coordinate[1]);
        let origin = p.origin()?;
        if [origin[0], origin[2]]
            .iter()
            .any(|v| !(-MAX_SAMPLE_COORDINATE + 1..=MAX_SAMPLE_COORDINATE - 17).contains(v))
        {
            return Err(SectionGenerationError::PrecisionLimit(p));
        }
        let padding = usize::from(with_mesh);
        let size = SECTION_SIZE + padding * 2;
        let start = [origin[0] - padding as i64, 0, origin[2] - padding as i64];
        let mut columns = self
            .terrain()
            .surface_column_grid([start[0], start[2]], size);
        // Long attached props may probe anchors beyond the meshing halo.
        let structures = self
            .terrain()
            .structures_for_column(coordinate, padding + 20);
        for site in structures
            .iter()
            .filter(|s| s.summary.kind.dragon().is_some())
        {
            for (i, column) in columns.iter_mut().enumerate() {
                let x = start[0] + (i % size) as i64;
                let z = start[2] + (i / size) as i64;
                site.clip_water(column, x, z);
            }
        }
        let trees: Vec<_> = trees_in_column(self.terrain(), start, size)
            .into_iter()
            .filter(|tree| {
                !self
                    .terrain()
                    .cave_room_at(tree.origin[0] as f64 + 0.5, tree.origin[2] as f64 + 0.5)
                    .open_to_sky()
            })
            .collect();
        // Attachment searches are performed once per immutable column, not for
        // every vertical section or every terrain remesh.
        let mut tree_props: Vec<_> = trees
            .iter()
            .filter(|tree| {
                [0, 2].into_iter().all(|axis| {
                    (origin[axis] - 3..origin[axis] + SECTION_SIZE as i64 + 3)
                        .contains(&tree.origin[axis])
                })
            })
            .filter_map(|tree| {
                super::vegetation::attached_mushroom(tree, self.terrain().config().seed.0)
            })
            .filter(|prop| [prop.section.x, prop.section.z] == coordinate)
            .collect();
        if self.terrain().config().ecology_revision >= 15 {
            for tree in &trees {
                tree_props.extend(
                    super::vegetation::palm_coconuts(tree, self.terrain().config().seed.0)
                        .into_iter()
                        .filter(|prop| [prop.section.x, prop.section.z] == coordinate),
                );
            }
        }
        let natural_features = self.terrain().natural_patches(start, size);
        self.prepare_falls(
            start,
            size,
            &mut columns,
            &structures,
            &natural_features,
            &trees,
        );
        let mut minimum = columns
            .iter()
            .map(|c| {
                c.river_cavity
                    .into_iter()
                    .chain(c.cave_cavity)
                    .map(|cavity| cavity[0])
                    .chain(c.cave.surface_view_floor())
                    .fold(c.height, f64::min)
            })
            .fold(f64::INFINITY, f64::min);
        let mut maximum = columns
            .iter()
            .map(|c| c.height)
            .fold(f64::NEG_INFINITY, f64::max);
        if self.terrain().config().spirit_blossom_revision > 0 {
            maximum += 2.0;
        }
        for tree in &trees {
            maximum = maximum.max((tree.origin[1] + i64::from(tree.bounds.max[1])) as f64);
        }
        for patch in &natural_features {
            if let Some([bottom, top]) = patch.vertical_bounds() {
                minimum = minimum.min(bottom as f64);
                maximum = maximum.max(top as f64 + 1.0);
            }
        }
        if self.terrain().config().water && minimum < self.terrain().config().sea_level {
            maximum = maximum.max(self.terrain().config().sea_level);
        }
        for span in columns.iter().flat_map(|c| c.falls.into_iter().flatten()) {
            maximum = maximum.max(f64::from(span.top) + 1.);
            minimum = minimum.min(f64::from(span.bottom));
        }
        for level in columns
            .iter()
            .filter_map(|column| column.water_level.or(column.lava_level))
        {
            maximum = maximum.max(level);
        }
        for column in columns.iter().filter(|column| column.ocean.ice_depth > 0) {
            maximum = maximum.max(column.water_level.unwrap_or(column.height).floor() + 1.0);
        }
        for section in structures.iter().flat_map(|site| site.sections.keys()) {
            if (section.x - coordinate[0]).abs() <= i64::from(padding != 0)
                && (section.z - coordinate[1]).abs() <= i64::from(padding != 0)
            {
                minimum = minimum.min((section.y * 16) as f64);
                maximum = maximum.max((section.y * 16 + 15) as f64);
            }
        }
        let surface_sections = with_mesh.then(|| {
            let range = [
                ((minimum - 0.5).floor() as i64).div_euclid(16) - 1,
                (maximum.ceil() as i64).div_euclid(16),
            ];
            if self.terrain().config().bounded_world {
                range.map(|y| y.clamp(-8, 22))
            } else {
                range
            }
        });
        // Horizontal flight reuses this immutable column across many interest
        // changes. Reject impossible cave views before inspecting 256 cells.
        #[cfg(feature = "bevy")]
        let cave_eye_bounds = cave_eye_bounds(&columns, padding, size);
        #[cfg(feature = "bevy")]
        let gallery_skyline = with_mesh
            && columns
                .iter()
                .any(|c| c.cave.surface_view_floor().is_some());
        Ok(PreparedColumn {
            fall_columns: columns
                .iter()
                .enumerate()
                .filter(|(_, c)| c.falls.iter().any(Option::is_some))
                .map(|(i, _)| i)
                .collect(),
            config: Arc::clone(&self.terrain().config),
            coordinate,
            padding,
            size,
            columns,
            trees,
            tree_props,
            structures,
            natural_features,
            maximum,
            surface_sections,
            #[cfg(feature = "bevy")]
            cave_eye_bounds,
            #[cfg(feature = "bevy")]
            gallery_skyline,
        })
    }
}

#[cfg(all(test, feature = "bevy"))]
mod cave_streaming_tests {
    use super::*;

    fn reference(column: &PreparedColumn, eye_y: f64) -> Option<[i64; 2]> {
        let mut eligible = (0..16)
            .flat_map(|z| (0..16).map(move |x| (x, z)))
            .filter_map(|(x, z)| {
                let surface = column.local_surface(x, z)?;
                let room = surface.cave;
                (room.active()
                    && eye_y <= surface.height - 6.0
                    && eye_y >= room.floor - 16.0
                    && eye_y <= room.ceiling + 16.0)
                    .then_some([room.floor, room.ceiling])
            });
        let first = eligible.next()?;
        let [floor, ceiling] = eligible.fold(first, |a, b| [a[0].min(b[0]), a[1].max(b[1])]);
        let bounds = [
            ((floor - 1.0).floor() as i64).div_euclid(16),
            ((ceiling + 1.0).ceil() as i64).div_euclid(16),
        ];
        Some(if column.config.bounded_world {
            bounds.map(|y| y.clamp(-8, 22))
        } else {
            bounds
        })
    }

    #[test]
    fn cave_view_aggregate_matches_exact_cells_at_height_limits_and_ignores_halo_rooms() {
        let terrain = crate::TerraGenerator::new(WorldConfig {
            trees: false,
            caves: false,
            structures_revision: 0,
            ..Default::default()
        })
        .unwrap();
        for with_mesh in [false, true] {
            let mut column = SectionGenerator::new(terrain.clone())
                .prepare_column([-3, -2], with_mesh)
                .unwrap();
            for (index, surface) in column.columns.iter_mut().enumerate() {
                let x = index % column.size;
                let z = index / column.size;
                let halo = x < column.padding
                    || x >= column.padding + 16
                    || z < column.padding
                    || z >= column.padding + 16;
                surface.height = if halo { 500.0 } else { 32.0 + (x % 9) as f64 };
                surface.cave = super::super::CaveStructure::default();
                surface.cave.kind = if index % 7 == 0 {
                    super::super::CaveEcology::None
                } else {
                    super::super::CaveEcology::Salt
                };
                surface.cave.floor = if halo {
                    -500.0
                } else {
                    -95.0 + (z % 11) as f64
                };
                surface.cave.ceiling = if halo { 400.0 } else { 10.0 + (x % 7) as f64 };
                surface.cave.radial = if index % 5 == 0 { 1.2 } else { 0.25 };
            }
            column.cave_eye_bounds = cave_eye_bounds(&column.columns, column.padding, column.size);
            assert!(column.cave_eye_bounds.is_some());
            for bounded in [false, true] {
                Arc::make_mut(&mut column.config).bounded_world = bounded;
                for half_y in -1_200..=1_200 {
                    let eye_y = half_y as f64 * 0.5;
                    assert_eq!(column.cave_view_sections(eye_y), reference(&column, eye_y));
                }
            }
            let [low, high] = column.cave_eye_bounds.unwrap();
            for eye_y in [
                low - 0.001,
                low,
                low + 0.001,
                high - 0.001,
                high,
                high + 0.001,
            ] {
                assert_eq!(column.cave_view_sections(eye_y), reference(&column, eye_y));
            }
        }
    }

    #[test]
    fn tall_cave_view_requests_the_closed_envelope_but_never_surface_columns_or_noise() {
        let terrain = crate::TerraGenerator::new(WorldConfig {
            cave_structure_revision: 4,
            ..Default::default()
        })
        .unwrap();
        let mut column = SectionGenerator::new(terrain.clone())
            .prepare_column([-1, -2], true)
            .unwrap();
        let mut surface = terrain.surface_column(-0.5, -16.5);
        surface.height = 128.0;
        surface.cave = super::super::CaveStructure::default();
        surface.cave.kind = super::super::CaveEcology::Stone;
        surface.cave.floor = -80.0;
        surface.cave.ceiling = 96.0;
        surface.cave.radial = 0.1;
        column.columns.fill(surface);
        column.cave_eye_bounds = cave_eye_bounds(&column.columns, column.padding, column.size);
        assert_eq!(column.cave_view_sections(-24.0), Some([-6, 6]));
        assert!(column.cave_view_sections(140.0).is_none());
        assert!(column.cave_view_sections(-120.0).is_none());
        column
            .columns
            .iter_mut()
            .for_each(|c| c.cave.kind = super::super::CaveEcology::None);
        column.cave_eye_bounds = cave_eye_bounds(&column.columns, column.padding, column.size);
        assert!(column.cave_view_sections(-24.0).is_none());
    }

    #[test]
    fn flying_over_a_gallery_window_requests_the_floor_with_the_first_surface_batch() {
        let terrain = crate::TerraGenerator::new(WorldConfig {
            trees: false,
            structures_revision: 0,
            cave_structure_revision: 11,
            ..Default::default()
        })
        .unwrap();
        // Locate an actual opening where the airborne camera's ordinary depth
        // budget cannot reach the floor, in seed coordinates on either side of zero.
        let fixture = std::iter::once((-33, -96))
            .chain((-96..=0).flat_map(|z| (-96..=96).map(move |x| (x, z))))
            .find_map(|(x, z)| {
                let center = terrain
                    .cave_chamber_center(f64::from(x * 192 + 96), f64::from(z * 192 + 96))?;
                let room = terrain.cave_structure_at(center[0], center[2]);
                if !matches!(
                    room.kind,
                    super::super::CaveEcology::Dripstone | super::super::CaveEcology::Salt
                ) {
                    return None;
                }
                (-3..=3)
                    .flat_map(|i| (-3..=3).map(move |j| (i, j)))
                    .find_map(|(i, j)| {
                        let x = (center[0] + f64::from(i) * room.radius * 0.16).floor();
                        let z = (center[2] + f64::from(j) * room.radius * 0.16).floor();
                        let c = terrain.surface_column(x + 0.5, z + 0.5);
                        (c.cave.active() && c.cave.open_to_sky() && c.height - c.cave.floor > 50.)
                            .then_some(([x as i64, z as i64], c))
                    })
            })
            .expect("deep natural window");
        let coord = fixture.0.map(|v| v.div_euclid(16));
        let eye = fixture.1.height + 48.;
        let floor = (fixture.1.cave.floor.floor() as i64).div_euclid(16);
        let generator = SectionGenerator::new(terrain);
        let column = generator.prepare_column(coord, true).unwrap();
        assert!(column.cave_view_sections(eye).is_none());
        let first = generator
            .build_prepared(
                SectionPos::new(coord[0], (eye as i64).div_euclid(16), coord[1]),
                0,
                &Default::default(),
                &column,
            )
            .unwrap();
        let [low, high] = first.surface_sections.unwrap();
        assert!((low..=high).contains(&floor));
        assert!(
            low >= floor - 3,
            "scope the apron to the local room, not the world bottom"
        );
    }
}
