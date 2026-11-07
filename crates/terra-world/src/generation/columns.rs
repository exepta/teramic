use super::{
    MAX_SAMPLE_COORDINATE, SectionGenerationError, SectionGenerator, SurfaceColumn,
    vegetation::{Tree, trees_in_column},
};
use std::sync::Arc;
use terra_core::WorldConfig;
use terra_voxel::{SECTION_SIZE, SectionPos};

/// Immutable per-column data shared by all vertical worker jobs. No edits are cached here.
pub struct PreparedColumn {
    pub(super) config: WorldConfig,
    pub(super) coordinate: [i64; 2],
    pub(super) padding: usize,
    pub(super) size: usize,
    pub(super) columns: Vec<SurfaceColumn>,
    pub(super) trees: Vec<Tree>,
    pub(super) tree_props: Vec<crate::props::WorldProp>,
    pub(super) structures: Vec<Arc<super::structures::Site>>,
    pub(super) maximum: f64,
    pub(super) surface_sections: Option<[i64; 2]>,
    #[cfg(feature = "bevy")]
    pub(super) cave_eye_bounds: Option<[f64; 2]>,
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
                config: WorldConfig::default(),
                coordinate: [-1, -2],
                padding: 0,
                size: 0,
                columns: Vec::new(),
                trees: Vec::new(),
                tree_props: Vec::new(),
                structures: vec![Arc::new(Site {
                    summary: SiteSummary {
                        id: 1,
                        kind: StructureKind::Village,
                        style: VillageStyle::Temperate,
                        center,
                        bounds: [center, center],
                        buildings: Vec::new(),
                        containers: Vec::new(),
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
    #[cfg(test)]
    pub(crate) fn with_settlement_for_test(
        mut self,
        settlement: Arc<super::structures::VillageSettlement>,
    ) -> Self {
        use super::structures::{Site, SiteSummary, StructureKind};
        self.structures.push(Arc::new(Site {
            summary: SiteSummary {
                id: settlement.id,
                kind: StructureKind::Village,
                style: settlement.style,
                center: settlement.center,
                bounds: settlement.bounds,
                buildings: Vec::new(),
                containers: Vec::new(),
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
            .collect()
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
        Some((tree.wood_material as u16, tree.species.leaves() as u16))
    }

    #[cfg(feature = "bevy")]
    pub(crate) fn felling_shape(&self, cell: [i64; 3]) -> Option<crate::tree_felling::Shape> {
        self.trees
            .iter()
            .find(|tree| {
                matches!(tree.wood_material as u16, 6 | 10 | 36 | 134)
                    && tree.wood_at(cell).is_some()
            })?
            .felling_shape()
    }

    #[cfg(feature = "bevy")]
    pub(crate) fn felling_foot(&self, cell: [i64; 3]) -> bool {
        self.trees.iter().any(|tree| {
            matches!(tree.wood_material as u16, 6 | 10 | 36 | 134) && tree.wood_at(cell).is_some()
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
        (tree.leaves_at(cell)? as u16 == material && crowns.next().is_none()).then_some(tree.origin)
    }

    /// Exact upper bound of generated terrain, water and trees in this prepared column.
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
                6 | 10 | 36 | 134 => tree.wood_at(global),
                7..=9 | 132 | 133 | 195 => tree.leaves_at(global),
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
        let mut columns = Vec::with_capacity(size * size);
        for z in 0..size {
            for x in 0..size {
                columns.push(self.terrain().surface_column(
                    (start[0] + x as i64) as f64 + 0.5,
                    (start[2] + z as i64) as f64 + 0.5,
                ));
            }
        }
        // Long attached props may probe anchors beyond the meshing halo.
        let structures = self
            .terrain()
            .structures_for_column(coordinate, padding + 20);
        let trees: Vec<_> = trees_in_column(self.terrain(), start, size)
            .into_iter()
            .filter(|tree| {
                !self
                    .terrain()
                    .cave_structure_at(tree.origin[0] as f64 + 0.5, tree.origin[2] as f64 + 0.5)
                    .open_to_sky()
            })
            .collect();
        // Attachment searches are performed once per immutable column, not for
        // every vertical section or every terrain remesh.
        let tree_props = trees
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
        let mut minimum = columns
            .iter()
            .map(|c| {
                c.river_cavity
                    .into_iter()
                    .chain(c.cave_cavity)
                    .fold(c.height, |height, cavity| height.min(cavity[0]))
            })
            .fold(f64::INFINITY, f64::min);
        let mut maximum = columns
            .iter()
            .map(|c| c.height)
            .fold(f64::NEG_INFINITY, f64::max);
        for tree in &trees {
            maximum = maximum.max((tree.origin[1] + i64::from(tree.bounds.max[1])) as f64);
        }
        if self.terrain().config().water && minimum < self.terrain().config().sea_level {
            maximum = maximum.max(self.terrain().config().sea_level);
        }
        for level in columns.iter().filter_map(|column| column.water_level) {
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
        let surface_sections = with_mesh.then_some([
            ((minimum - 0.5).floor() as i64).div_euclid(16) - 1,
            (maximum.ceil() as i64).div_euclid(16),
        ]);
        // Horizontal flight reuses this immutable column across many interest
        // changes. Reject impossible cave views before inspecting 256 cells.
        #[cfg(feature = "bevy")]
        let cave_eye_bounds = cave_eye_bounds(&columns, padding, size);
        Ok(PreparedColumn {
            config: self.terrain().config().clone(),
            coordinate,
            padding,
            size,
            columns,
            trees,
            tree_props,
            structures,
            maximum,
            surface_sections,
            #[cfg(feature = "bevy")]
            cave_eye_bounds,
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
                column.config.bounded_world = bounded;
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
}
