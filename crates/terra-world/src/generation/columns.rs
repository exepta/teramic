use super::{
    MAX_SAMPLE_COORDINATE, SectionGenerationError, SectionGenerator, SurfaceColumn,
    vegetation::{Tree, trees_in_column},
};
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
    pub(super) maximum: f64,
    pub(super) surface_sections: Option<[i64; 2]>,
}

impl PreparedColumn {
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
    /// Read only the prepared tree primitives; this never generates new trees.
    pub fn tree_appearance_at(
        &self,
        global: [i64; 3],
        material: u16,
    ) -> Option<super::TreeAppearance> {
        self.trees.iter().find_map(|tree| {
            let generated = match material {
                6 | 10 | 36 => tree.wood_at(global),
                7..=9 => tree.leaves_at(global),
                _ => None,
            };
            generated
                .filter(|id| *id as u16 == material)
                .map(|_| super::TreeAppearance::of(tree))
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
        let minimum = columns
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
        let surface_sections = with_mesh.then_some([
            ((minimum - 0.5).floor() as i64).div_euclid(16) - 1,
            (maximum.ceil() as i64).div_euclid(16),
        ]);
        Ok(PreparedColumn {
            config: self.terrain().config().clone(),
            coordinate,
            padding,
            size,
            columns,
            trees,
            tree_props,
            maximum,
            surface_sections,
        })
    }
}
