use super::{
    MAX_SAMPLE_COORDINATE, PreparedColumn, SurfaceColumn, TerraGenerator,
    vegetation::trees_in_region,
};
use std::collections::HashMap;
use terra_voxel::blocks::BlockError;
use terra_voxel::meshing::{BlockMesh, MeshError, mesh_section};
use terra_voxel::{BlockSection, MaterialId, SECTION_SIZE, SECTION_VOLUME, SectionPos};
use thiserror::Error;

pub type BlockOverrides = HashMap<[i64; 3], u16>;
#[derive(Debug, Error)]
pub enum SectionGenerationError {
    #[error(transparent)]
    Block(#[from] BlockError),
    #[error(transparent)]
    Mesh(#[from] MeshError),
    #[error("section {0:?} outside the generator precision budget")]
    PrecisionLimit(SectionPos),
    #[error("prepared column does not match the section or generator configuration")]
    ColumnMismatch,
}

#[cfg(test)]
mod loaded_remesh_tests {
    use super::*;

    #[test]
    fn terrain_fill_matches_per_voxel_sampling_across_sky_plant_water_and_bedrock_limits() {
        const SIZE: usize = 6;
        for (revision, bounded_world) in [(2, true), (10, true), (10, false)] {
            let generator = SectionGenerator::new(
                TerraGenerator::new(terra_core::WorldConfig {
                    cave_structure_revision: revision,
                    bounded_world,
                    ..Default::default()
                })
                .unwrap(),
            );
            let mut columns = Vec::new();
            for z in 0..SIZE {
                for x in 0..SIZE {
                    let mut column = generator
                        .terrain
                        .surface_column(x as f64 - 16.5, z as f64 - 0.5);
                    column.height = if z == 0 { -142.0 } else { z as f64 - 3.5 };
                    column.seagrass_height = if x == 1 { 7 } else { 0 };
                    column.water_level = (x == 2 || x == 3).then_some(8.25);
                    column.ocean.ice_depth = if x == 3 { 3 } else { 0 };
                    column.lava_level = (x == 4).then_some(12.75);
                    columns.push(column);
                }
            }
            for bottom in [-132, -124, -7, -1, 5, 11, 17] {
                let start = [-17, bottom, -1];
                let blocks = generator.terrain_voxels(start, SIZE, &columns);
                for (i, &material) in blocks.iter().enumerate() {
                    let (x, y, z) = (i % SIZE, i / SIZE % SIZE, i / (SIZE * SIZE));
                    let p = [start[0] + x as i64, bottom + y as i64, start[2] + z as i64];
                    assert_eq!(
                        material,
                        generator.terrain_block(p, columns[x + SIZE * z]),
                        "{p:?}, revision {revision}, bounded {bounded_world}"
                    );
                }
            }
        }
    }

    #[test]
    fn a_connector_cutting_a_mineral_root_removes_the_entire_generated_spike() {
        use super::super::CaveEcology;
        let terrain = TerraGenerator::new(terra_core::WorldConfig {
            cave_structure_revision: 10,
            landscape_revision: 0,
            rivers: false,
            spirit_blossom_revision: 1,
            trees: false,
            ..Default::default()
        })
        .unwrap();
        let generator = SectionGenerator::new(terrain);
        // Use an actual R10 room descriptor so decoration revision, seed and
        // geology agree, while forcing a short corridor through its floor.
        let mut found = false;
        'scan: for rz in -12..12 {
            for rx in -12..12 {
                let Some(center) = generator
                    .terrain
                    .cave_chamber_center(f64::from(rx) * 192. + 96., f64::from(rz) * 192. + 96.)
                else {
                    continue;
                };
                let room = generator.terrain.cave_structure_at(center[0], center[2]);
                if room.kind != CaveEcology::Dripstone || !room.active() {
                    continue;
                }
                let [cx, _, cz] = center.map(|n| n.floor() as i64);
                for x in cx - 14..=cx + 14 {
                    for z in cz - 14..=cz + 14 {
                        let mut column = generator
                            .terrain
                            .surface_column(x as f64 + 0.5, z as f64 + 0.5);
                        let root = (column.cave.floor - 0.5).floor() as i64;
                        let tip = [x, root + 2, z];
                        if generator.terrain_block(tip, column) != 52
                            || generator.terrain_block([x, root + 1, z], column) != 52
                        {
                            continue;
                        }
                        // Excise the root and first voxel but preserve the spike above.
                        column.cave_cavity = Some([root as f64 - 1., root as f64 + 2.]);
                        if generator.unchecked_terrain_block(tip, column) == 52
                            && generator.unchecked_terrain_block([x, root + 1, z], column) == 0
                            && generator.terrain_block(tip, column) == 0
                        {
                            found = true;
                            break 'scan;
                        }
                    }
                }
            }
        }
        assert!(
            found,
            "a real R10 stalagmite must disappear when its support enters a passage"
        );
    }

    #[test]
    fn real_cave_foliage_requires_a_contiguous_ceiling_instead_of_per_voxel_noise_islands() {
        use noise::NoiseFn;
        let mut config = terra_core::WorldConfig {
            trees: false,
            water: false,
            cave_structure_revision: 5,
            ..Default::default()
        };
        config.cave_biomes.distribution = terra_core::config::CaveDistribution {
            normal_weight: 0.0,
            salt_weight: 0.0,
            lush_weight: 1.0,
            cascade_weight: 0.0,
        };
        config.cave_biomes.lush_density = 1.0;
        let mut supported = None;
        let mut island = None;
        'search: for seed in [42042, 99991, 1] {
            config.seed = terra_core::WorldSeed(seed);
            let generator = SectionGenerator::new(TerraGenerator::new(config.clone()).unwrap());
            for z in (-64..64).step_by(4) {
                for x in (-64..64).step_by(4) {
                    let column = generator
                        .terrain
                        .surface_column(x as f64 + 0.5, z as f64 + 0.5);
                    let h = super::super::seed::layer_seed(
                        seed ^ (x as u64).wrapping_mul(0x9e3779b97f4a7c15)
                            ^ (z as u64).wrapping_mul(0xbf58476d1ce4e5b9),
                        304,
                    );
                    if !h.is_multiple_of(11) {
                        continue;
                    }
                    let roll = ((h >> 10) & 65535) as f32 / 65535.0;
                    let probability = |y: i64| {
                        let w = column.cave_biome.weights_at(y as f64 + 0.5);
                        let patch = (generator.terrain.valleys.get([
                            (x as f64 + 0.5) * 0.06,
                            (y as f64 + 0.5) * 0.025,
                            (z as f64 + 0.5) * 0.06,
                        ]) + 0.5)
                            .clamp(0.0, 1.0) as f32;
                        (w[2] * 0.65 + w[3] * 0.15 + w[0] * 0.025) * patch / 0.72
                    };
                    for y in -112..48 {
                        let p = [x, y, z];
                        let material = generator.unchecked_terrain_block(p, column);
                        if !matches!(material, 54 | 55) {
                            continue;
                        }
                        if crate::segments::hanging_support(p, |p| {
                            Some(generator.unchecked_terrain_block(p, column))
                        }) == Some(true)
                        {
                            supported = Some((generator.clone(), p, material));
                        }
                        let above = [x, y + 1, z];
                        if !matches!(generator.unchecked_terrain_block(above, column), 54 | 55) {
                            continue;
                        }
                        let lower = probability(y);
                        let upper = probability(y + 1);
                        if lower <= upper || upper <= 0.0 || roll <= 0.0 {
                            continue;
                        }
                        // A valid configured density between two actual neighbouring
                        // noise probabilities reproduces a gap without invented blocks.
                        let density = (roll / lower + (roll / upper).min(1.0)) * 0.5;
                        if !(0.0..1.0).contains(&density) {
                            continue;
                        }
                        let mut tuned = config.clone();
                        tuned.cave_biomes.lush_density = f64::from(density);
                        let tuned = SectionGenerator::new(TerraGenerator::new(tuned).unwrap());
                        let c = tuned.terrain.surface_column(x as f64 + 0.5, z as f64 + 0.5);
                        if matches!(tuned.unchecked_terrain_block(p, c), 54 | 55)
                            && tuned.unchecked_terrain_block(above, c) == 0
                            && (2..=crate::segments::MAX_CHAIN).any(|dy| {
                                crate::props::placement::ground_support(
                                    tuned.unchecked_terrain_block([x, y + dy, z], c),
                                )
                            })
                        {
                            island = Some((tuned, p));
                        }
                        if supported.is_some() && island.is_some() {
                            break 'search;
                        }
                    }
                }
            }
        }
        let (positive, p, material) =
            supported.expect("actual anchored foliage is retained, not globally disabled");
        let c = positive
            .terrain
            .surface_column(p[0] as f64 + 0.5, p[2] as f64 + 0.5);
        assert_eq!(positive.terrain_block(p, c), material);
        let (generator, island) = island.expect("reproduces the original per-voxel foliage gap");
        let c = generator
            .terrain
            .surface_column(island[0] as f64 + 0.5, island[2] as f64 + 0.5);
        assert_eq!(
            crate::segments::hanging_support(island, |p| Some(
                generator.unchecked_terrain_block(p, c)
            )),
            Some(false)
        );
        assert_eq!(generator.terrain_block(island, c), 0);
        let (owner, local) = terra_voxel::global_to_section(island);
        let plain = generator
            .build(owner, 0, &Default::default(), false)
            .unwrap();
        let meshed = generator
            .build(owner, 0, &Default::default(), true)
            .unwrap();
        assert_eq!(plain.data, meshed.data);
        assert_eq!(plain.props, meshed.props);
        assert_eq!(plain.data.block(local).unwrap(), 0);
        for prop in &plain.props {
            assert_eq!(
                crate::props::placement::supported(prop, |p| generator.block_at(p).ok()),
                Some(true),
                "a generated cave prop needs an actual supporting voxel: {prop:?}"
            );
        }
    }

    #[test]
    fn loaded_remesh_matches_fresh_generation_including_halos_and_segment_details() {
        for (seed, coordinate, cave_structure_revision) in
            [(1, [-1, -1], 0), (42042, [2, 3], 5), (99991, [-8, 7], 5)]
        {
            let terrain = TerraGenerator::new(terra_core::WorldConfig {
                seed: terra_core::WorldSeed(seed),
                cave_structure_revision,
                ..Default::default()
            })
            .unwrap();
            let height = terrain.surface_height(
                (coordinate[0] * 16 + 8) as f64,
                (coordinate[1] * 16 + 8) as f64,
            );
            let owner = SectionPos::new(
                coordinate[0],
                ((height - 8.0).floor() as i64).div_euclid(16),
                coordinate[1],
            );
            let generator = SectionGenerator::new(terrain);
            let prepared = generator.prepare_column(coordinate, true).unwrap();
            let edits = HashMap::from([
                (owner.global_block([15, 9, 15]).unwrap(), 3),
                (owner.global_block([3, 7, 3]).unwrap(), 3),
                (owner.global_block([3, 8, 3]).unwrap(), 51),
                (owner.global_block([3, 9, 3]).unwrap(), 51),
                (owner.global_block([3, 10, 3]).unwrap(), 0),
            ]);
            let full = generator
                .build_prepared(owner, 7, &edits, &prepared)
                .unwrap();
            let mut known = vec![None; terra_voxel::meshing::PADDED_VOLUME];
            for z in 0..16 {
                for y in 0..16 {
                    for x in 0..16 {
                        known[x + 1 + 18 * (y + 1 + 18 * (z + 1))] =
                            Some(full.data.block([x, y, z]).unwrap());
                    }
                }
            }
            let remeshed = generator
                .remesh_loaded(&full.data, 7, &edits, &prepared, &known)
                .unwrap();
            assert_eq!(remeshed.data, full.data);
            assert_eq!(remeshed.water, full.water);
            assert!(
                remeshed.fluid_wake.is_empty(),
                "geometry remeshing must not restart sleeping natural falls"
            );
            let mesh = remeshed.mesh.unwrap();
            let original = full.mesh.unwrap();
            assert_eq!(mesh.positions, original.positions);
            assert_eq!(mesh.normals, original.normals);
            assert_eq!(mesh.materials, original.materials);
            assert_eq!(mesh.uvs, original.uvs);
            assert_eq!(mesh.indices, original.indices);
            assert_eq!(mesh.occluders, original.occluders);
            assert_eq!(mesh.mineral_growth, original.mineral_growth);
            let segments = |m: &BlockMesh| {
                m.segments
                    .iter()
                    .map(|s| (s.material, s.cell, s.below, s.above, s.hanging))
                    .collect::<Vec<_>>()
            };
            assert_eq!(segments(&mesh), segments(&original));
        }
    }
}
pub struct GeneratedSection {
    pub data: BlockSection,
    pub mesh: Option<BlockMesh>,
    /// Surface/crown envelope computed on the same worker as the voxel data.
    pub surface_sections: Option<[i64; 2]>,
    pub water: Option<crate::water::WaterPatch>,
    pub props: Vec<crate::props::WorldProp>,
    pub fluid_wake: Vec<[i64; 3]>,
}
#[derive(Clone)]
pub struct SectionGenerator {
    terrain: TerraGenerator,
    faces_only: bool,
}
impl SectionGenerator {
    pub fn new(terrain: TerraGenerator) -> Self {
        Self {
            terrain,
            faces_only: false,
        }
    }
    #[cfg(feature = "bevy")]
    pub(crate) fn for_streaming(terrain: TerraGenerator) -> Self {
        Self {
            terrain,
            faces_only: true,
        }
    }
    fn section_mesh(&self, padded: &[u16]) -> Result<BlockMesh, MeshError> {
        if self.faces_only {
            terra_voxel::meshing::mesh_section_faces(padded)
        } else {
            mesh_section(padded)
        }
    }
    pub fn terrain(&self) -> &TerraGenerator {
        &self.terrain
    }
    /// Remesh an existing section without regenerating its terrain or props.
    /// Unknown halo cells use the same prepared column as initial generation.
    pub(crate) fn remesh_loaded(
        &self,
        data: &BlockSection,
        revision: u64,
        edits: &BlockOverrides,
        column: &PreparedColumn,
        known: &[Option<u16>],
    ) -> Result<GeneratedSection, SectionGenerationError> {
        let position = data.position();
        if column.coordinate != [position.x, position.z]
            || !column.matches_terrain(&self.terrain)
            || column.padding != 1
        {
            return Err(SectionGenerationError::ColumnMismatch);
        }
        let origin = position.origin()?;
        let size = terra_voxel::meshing::PADDED_SIZE;
        if known.len() != terra_voxel::meshing::PADDED_VOLUME {
            return Err(MeshError::InvalidPadding(known.len()).into());
        }
        let start = origin.map(|v| v - 1);
        let mut padded = vec![0; known.len()];
        for z in 0..size {
            for y in 0..size {
                for x in 0..size {
                    let index = x + size * (y + size * z);
                    if known[index].is_some() {
                        continue;
                    }
                    let global = [
                        start[0] + x as i64,
                        start[1] + y as i64,
                        start[2] + z as i64,
                    ];
                    let surface = column.columns[x + z * size];
                    padded[index] = self.terrain_block(global, surface);
                }
            }
        }
        if known.iter().any(Option::is_none) {
            self.terrain
                .stamp_ores(start, size, &column.columns, &mut padded);
            for patch in &column.natural_features {
                patch.stamp(start, size, &mut padded);
            }
            for tree in &column.trees {
                tree.stamp_wood(start, size, &mut padded);
            }
            for tree in &column.trees {
                tree.stamp_leaves(start, size, &mut padded);
            }
            for site in &column.structures {
                site.stamp(start, size, &mut padded);
            }
        }
        for z in 0..size {
            for y in 0..size {
                for x in 0..size {
                    let index = x + size * (y + size * z);
                    let global = [
                        start[0] + x as i64,
                        start[1] + y as i64,
                        start[2] + z as i64,
                    ];
                    if let Some(material) = edits.get(&global).copied().or(known[index]) {
                        padded[index] = material;
                    }
                    if self.terrain.config().bounded_world && !(-128..=356).contains(&global[1]) {
                        padded[index] = 0;
                    }
                }
            }
        }
        let mut blocks = Vec::with_capacity(SECTION_VOLUME);
        for z in 1..=SECTION_SIZE {
            for y in 1..=SECTION_SIZE {
                let start = 1 + size * (y + size * z);
                blocks.extend_from_slice(&padded[start..start + SECTION_SIZE]);
            }
        }
        let mut mesh = self.section_mesh(&padded)?;
        for (index, &material) in blocks.iter().enumerate() {
            if !crate::segments::is_segment(material) {
                continue;
            }
            let cell = [index % 16, index / 16 % 16, index / 256];
            let global = std::array::from_fn(|axis| origin[axis] + cell[axis] as i64);
            let profile =
                self.mesh_segment_profile(global, material, edits, column, start, &padded);
            mesh.segments.push(terra_voxel::meshing::BlockSegment {
                material,
                cell: cell.map(|v| v as u8),
                below: profile.below,
                above: profile.above,
                hanging: profile.hanging,
                joined: profile.joined,
                fan: profile.fan,
            });
        }
        if self.terrain.config().cave_structure_revision == 0 {
            mesh.mineral_growth =
                crate::ore_shapes::salt_growth_snapshot(&mesh.faces, origin, |p| {
                    if let Some(&material) = edits.get(&p) {
                        return Some(material);
                    }
                    let cell = std::array::from_fn::<_, 3, _>(|axis| p[axis] - start[axis]);
                    if cell.iter().all(|v| (0..size as i64).contains(v)) {
                        Some(
                            padded[cell[0] as usize
                                + size * (cell[1] as usize + size * cell[2] as usize)],
                        )
                    } else {
                        self.block_at(p).ok()
                    }
                });
        }
        // Natural shoreline geometry belongs to this exact edited halo. Active
        // fluid overrides still use the existing water worker/publication path.
        let water = if self.terrain.config().water {
            crate::water::build_natural_patch(
                position,
                origin,
                &column.columns,
                &padded,
                self.terrain.config().sea_level,
                self.terrain.config().rivers
                    || crate::water::local_water_levels(
                        &column.columns,
                        self.terrain.config().sea_level,
                    ),
            )
        } else {
            None
        };
        Ok(GeneratedSection {
            data: BlockSection::from_blocks(position, revision, blocks)?,
            mesh: Some(mesh),
            water,
            props: Vec::new(),
            fluid_wake: Vec::new(),
            surface_sections: None,
        })
    }
    pub fn generate(&self, position: SectionPos) -> Result<BlockSection, SectionGenerationError> {
        Ok(self.build(position, 0, &BlockOverrides::new(), false)?.data)
    }
    /// Heights and tree candidates are cached per job; no generator work runs in the render loop.
    pub fn build(
        &self,
        position: SectionPos,
        revision: u64,
        edits: &BlockOverrides,
        with_mesh: bool,
    ) -> Result<GeneratedSection, SectionGenerationError> {
        let column = self.prepare_column([position.x, position.z], with_mesh)?;
        self.build_prepared(position, revision, edits, &column)
    }
    pub fn build_prepared(
        &self,
        position: SectionPos,
        revision: u64,
        edits: &BlockOverrides,
        column: &PreparedColumn,
    ) -> Result<GeneratedSection, SectionGenerationError> {
        if column.coordinate != [position.x, position.z] || !column.matches_terrain(&self.terrain) {
            return Err(SectionGenerationError::ColumnMismatch);
        }
        let origin = position.origin()?;
        if origin
            .iter()
            .any(|v| !(-MAX_SAMPLE_COORDINATE + 1..=MAX_SAMPLE_COORDINATE - 17).contains(v))
        {
            return Err(SectionGenerationError::PrecisionLimit(position));
        }
        let padding = column.padding;
        let with_mesh = padding != 0;
        let size = column.size;
        let start = origin.map(|v| v - padding as i64);
        let columns = &column.columns;
        let structure_at = |p| {
            column
                .structures
                .iter()
                .find_map(|site| site.block_at(p))
                .or_else(|| {
                    if (start[0]..start[0] + size as i64).contains(&p[0])
                        && (start[2]..start[2] + size as i64).contains(&p[2])
                    {
                        column
                            .natural_features
                            .iter()
                            .find_map(|patch| patch.block_at(p))
                    } else {
                        self.terrain.natural_material_at(p)
                    }
                })
        };
        if edits.is_empty() && start[1] as f64 + 0.5 > column.maximum {
            return Ok(GeneratedSection {
                data: BlockSection::empty_at_revision(position, revision)?,
                mesh: with_mesh.then(BlockMesh::default),
                water: None,
                props: Vec::new(),
                fluid_wake: Vec::new(),
                surface_sections: column.surface_sections,
            });
        }
        let mut padded = self.terrain_voxels(start, size, columns);
        self.terrain.stamp_ores(start, size, columns, &mut padded);
        for patch in &column.natural_features {
            patch.stamp(start, size, &mut padded);
        }
        let trees = &column.trees;
        let surface_sections = column.surface_sections;
        // Terrain wins; every tree's wood wins over every crown; edits win last.
        // Rasterize clipped primitives once, not all branches for every voxel.
        for tree in trees {
            tree.stamp_wood(start, size, &mut padded);
        }
        for tree in trees {
            tree.stamp_leaves(start, size, &mut padded);
        }
        for site in &column.structures {
            site.stamp(start, size, &mut padded);
        }
        if self.terrain.config().bounded_world {
            for z in 0..size {
                for y in 0..size {
                    for x in 0..size {
                        let world_y = start[1] + y as i64;
                        if !(-128..=356).contains(&world_y) {
                            padded[x + size * (y + size * z)] = 0;
                        }
                    }
                }
            }
        }
        // Edits can hide or remove original vegetation, never create candidates
        // under a tree that occupied this space when the world was generated.
        let original_ground: Option<[[u16; 2]; SECTION_SIZE * SECTION_SIZE]> = (!edits.is_empty())
            .then(|| {
                std::array::from_fn(|cell| {
                    let x = cell % SECTION_SIZE;
                    let z = cell / SECTION_SIZE;
                    let ground =
                        (columns[x + padding + (z + padding) * size].height - 0.5).floor() as i64;
                    let y = ground - origin[1];
                    if !(0..SECTION_SIZE as i64).contains(&y) {
                        return [0; 2];
                    }
                    let index = x + padding + size * (y as usize + padding + size * (z + padding));
                    let above = [origin[0] + x as i64, ground + 1, origin[2] + z as i64];
                    let obstruction = if y as usize + padding + 1 < size {
                        padded[index + size]
                    } else {
                        structure_at(above).unwrap_or_else(|| {
                            trees
                                .iter()
                                .find_map(|tree| tree.wood_at(above))
                                .or_else(|| trees.iter().find_map(|tree| tree.leaves_at(above)))
                                .map_or(0, |material| material as u16)
                        })
                    };
                    [padded[index], obstruction]
                })
            });
        for (&global, &material) in edits {
            let local: [Option<i64>; 3] = std::array::from_fn(|i| global[i].checked_sub(start[i]));
            let [Some(x), Some(y), Some(z)] = local else {
                continue;
            };
            if [x, y, z].iter().all(|&v| (0..size as i64).contains(&v)) {
                padded[x as usize + size * (y as usize + size * z as usize)] = material;
            }
        }
        let mut blocks = vec![0; SECTION_VOLUME];
        let mut props = Vec::new();
        if self.terrain.config().ecology_revision >= 16
            && self.terrain.config().water
            && columns.iter().any(|c| {
                c.water_columns()
                    .intervals
                    .into_iter()
                    .flatten()
                    .any(|w| w.level > origin[1] as f64 - 1. && w.bed < origin[1] as f64 + 17.)
                    && c.biome_kind(self.terrain.config().sea_level)
                        == super::biomes::BiomeKind::Ocean
            })
        {
            let surface = |p: [i64; 3]| {
                let (x, z) = (p[0] - start[0], p[2] - start[2]);
                if [x, z].iter().all(|v| (0..size as i64).contains(v)) {
                    columns[x as usize + z as usize * size]
                } else {
                    self.terrain
                        .surface_column(p[0] as f64 + 0.5, p[2] as f64 + 0.5)
                }
            };
            crate::props::reef_flora::colonize(
                origin,
                self.terrain.config().seed.0,
                |p| {
                    let c = surface(p);
                    c.biome_kind(self.terrain.config().sea_level) == super::biomes::BiomeKind::Ocean
                        && crate::water::fluid::from_columns(c.water_columns(), p[1]).amount >= 7
                },
                |p| {
                    let [x, y, z] = std::array::from_fn(|i| p[i] - start[i]);
                    if [x, y, z].iter().all(|v| (0..size as i64).contains(v)) {
                        padded[x as usize + size * (y as usize + size * z as usize)]
                    } else {
                        edits
                            .get(&p)
                            .copied()
                            .or_else(|| structure_at(p))
                            .unwrap_or_else(|| self.terrain_block(p, surface(p)))
                    }
                },
                &mut props,
            );
        }
        if self.terrain.config().cave_structure_revision >= 4 {
            props.extend(super::webs::generate(
                position,
                self.terrain.config().seed.0,
                self.terrain.config().bounded_world,
                |x, z| {
                    let local = [(x - start[0]) as usize, (z - start[2]) as usize];
                    let surface = columns[local[0] + local[1] * size];
                    (surface.height, surface.biome.weights.forest > 0.35)
                },
                |global| {
                    if self.terrain.config().bounded_world && !(-128..=356).contains(&global[1]) {
                        return None;
                    }
                    let local = std::array::from_fn::<_, 3, _>(|axis| global[axis] - start[axis]);
                    if local.iter().all(|&v| (0..size as i64).contains(&v)) {
                        return Some(
                            padded[local[0] as usize
                                + size * (local[1] as usize + size * local[2] as usize)],
                        );
                    }
                    if let Some(material) =
                        edits.get(&global).copied().or_else(|| structure_at(global))
                    {
                        return Some(material);
                    }
                    let surface = self
                        .terrain
                        .surface_column(global[0] as f64 + 0.5, global[2] as f64 + 0.5);
                    Some(edits.get(&global).copied().unwrap_or_else(|| {
                        let terrain = self.terrain_block(global, surface);
                        if terrain != 0 {
                            return terrain;
                        }
                        trees
                            .iter()
                            .find_map(|tree| tree.wood_at(global))
                            .or_else(|| trees.iter().find_map(|tree| tree.leaves_at(global)))
                            .map_or(0, |m| m as u16)
                    }))
                },
            ));
        }
        let mut garden_supports = std::collections::BTreeSet::new();
        if self.terrain.config().structures_revision >= 8 {
            for site in &column.structures {
                for support in site.garden_supports(position) {
                    garden_supports.insert(support);
                    if edits.contains_key(&support)
                        || edits.contains_key(&[support[0], support[1] + 1, support[2]])
                        || edits.contains_key(&[support[0], support[1] + 2, support[2]])
                    {
                        continue;
                    }
                    props.extend(site.garden_props(&self.terrain, support, position));
                }
            }
        }
        for z in 0..SECTION_SIZE {
            for y in 0..SECTION_SIZE {
                let source = padding + size * (y + padding + size * (z + padding));
                let target = SECTION_SIZE * (y + SECTION_SIZE * z);
                blocks[target..target + SECTION_SIZE]
                    .copy_from_slice(&padded[source..source + SECTION_SIZE]);
            }
        }
        let mesh = if with_mesh {
            let mut mesh = self.section_mesh(&padded)?;
            for z in 0..16 {
                for y in 0..16 {
                    for x in 0..16 {
                        let material = blocks[x + 16 * (y + 16 * z)];
                        if !crate::segments::is_segment(material) {
                            continue;
                        }
                        let cell = [x as u8, y as u8, z as u8];
                        let global = [
                            origin[0] + x as i64,
                            origin[1] + y as i64,
                            origin[2] + z as i64,
                        ];
                        let profile = self
                            .mesh_segment_profile(global, material, edits, column, start, &padded);
                        mesh.segments.push(terra_voxel::meshing::BlockSegment {
                            material,
                            cell,
                            below: profile.below,
                            above: profile.above,
                            hanging: profile.hanging,
                            joined: profile.joined,
                            fan: profile.fan,
                        });
                    }
                }
            }
            for face in &mesh.faces {
                if self.terrain.config().cave_structure_revision > 0 {
                    continue;
                }
                if !matches!(face.material, 30 | 37)
                    || !matches!(face.side, 2 | 3)
                    || face.occupied_neighbor
                {
                    continue;
                }
                for v in 0..face.height {
                    for u in 0..face.width {
                        let mut cell = face.minimum;
                        cell[face.u_axis] += u;
                        cell[face.v_axis] += v;
                        let global =
                            std::array::from_fn(|axis| origin[axis] + i64::from(cell[axis]));
                        let surface = columns
                            [cell[0] as usize + padding + (cell[2] as usize + padding) * size];
                        let height = crate::ore_shapes::mineral_growth_height(
                            face.material,
                            global,
                            face.side,
                            |p| {
                                if let Some(&edited) = edits.get(&p) {
                                    return Some(edited);
                                }
                                let y = p[1] - start[1];
                                if (0..size as i64).contains(&y) {
                                    return Some(
                                        padded[cell[0] as usize
                                            + padding
                                            + size
                                                * (y as usize
                                                    + size * (cell[2] as usize + padding))],
                                    );
                                }
                                if let Some(material) = structure_at(p) {
                                    return Some(material);
                                }
                                let material = self.terrain_block(p, surface);
                                Some(if material == 0 {
                                    trees
                                        .iter()
                                        .find_map(|tree| tree.wood_at(p))
                                        .or_else(|| trees.iter().find_map(|tree| tree.leaves_at(p)))
                                        .map_or(0, |material| material as u16)
                                } else {
                                    material
                                })
                            },
                        );
                        if height > 0.0 {
                            mesh.mineral_growth.push((
                                cell.map(|v| v as u8),
                                face.side as u8,
                                height,
                            ));
                        }
                    }
                }
            }
            Some(mesh)
        } else {
            None
        };
        let water = if with_mesh && self.terrain.config().water {
            crate::water::build_natural_patch(
                position,
                origin,
                columns,
                &padded,
                self.terrain.config().sea_level,
                self.terrain.config().rivers
                    || crate::water::local_water_levels(columns, self.terrain.config().sea_level),
            )
        } else {
            None
        };
        for z in 0..16 {
            for x in 0..16 {
                let column = columns[x + padding + (z + padding) * size];
                let floor = (column.cave.floor - 0.5).floor() as i64;
                let floors = if self.terrain.config().cave_structure_revision >= 3 {
                    if !self
                        .terrain
                        .cave_prop_column_candidate(origin[0] + x as i64, origin[2] + z as i64)
                    {
                        continue;
                    }
                    origin[1]..origin[1] + 16
                } else {
                    floor..floor + 1
                };
                for floor in floors {
                    if !(origin[1]..origin[1] + 16).contains(&floor) {
                        continue;
                    }
                    if self.terrain.config().cave_structure_revision >= 4 {
                        let y = (floor - start[1]) as usize;
                        let index = x + padding + size * (y + size * (z + padding));
                        // Reuse generated occupancy before evaluating plant noise.
                        // Missing top-border cells still use the world-space path.
                        if !crate::props::placement::ground_support(padded[index])
                            || (y + 1 < size && padded[index + size] != 0)
                            || (y + 2 < size && padded[index + size * 2] != 0)
                        {
                            continue;
                        }
                    }
                    let support = [origin[0] + x as i64, floor, origin[2] + z as i64];
                    if let Some(prop) = self.terrain.cave_prop(support, column, position) {
                        let mut clear = true;
                        for py in floor + 1..=floor + 2 {
                            let p = [support[0], py, support[2]];
                            let material = edits
                                .get(&p)
                                .copied()
                                .or_else(|| structure_at(p))
                                .unwrap_or_else(|| self.terrain_block(p, column));
                            if material != 0 || self.terrain_block(p, column) != 0 {
                                clear = false;
                                break;
                            }
                        }
                        if clear
                            && crate::props::placement::ground_support(
                                edits
                                    .get(&support)
                                    .copied()
                                    .or_else(|| structure_at(support))
                                    .unwrap_or_else(|| self.terrain_block(support, column)),
                            )
                        {
                            props.push(prop);
                        }
                    }
                }
            }
        }
        for &prop in column
            .tree_props
            .iter()
            .filter(|prop| prop.section == position)
        {
            let support = crate::props::placement::support(&prop);
            let outside = prop.position.map(|value| value.floor() as i64);
            let material_at = |global: [i64; 3]| {
                edits.get(&global).copied().unwrap_or_else(|| {
                    let local = std::array::from_fn::<_, 3, _>(|axis| global[axis] - start[axis]);
                    if local.iter().all(|&v| (0..size as i64).contains(&v)) {
                        return padded[local[0] as usize
                            + size * (local[1] as usize + size * local[2] as usize)];
                    }
                    if let Some(material) = structure_at(global) {
                        return material;
                    }
                    let column = self
                        .terrain
                        .surface_column(global[0] as f64 + 0.5, global[2] as f64 + 0.5);
                    let terrain = self.terrain_block(global, column);
                    if terrain != 0 {
                        return terrain;
                    }
                    trees
                        .iter()
                        .find_map(|tree| tree.wood_at(global))
                        .or_else(|| trees.iter().find_map(|tree| tree.leaves_at(global)))
                        .map_or(0, |material| material as u16)
                })
            };
            if matches!(
                MaterialId::from_raw(material_at(support)),
                Some(MaterialId::Wood | MaterialId::WeatheredWood | MaterialId::SpruceLog)
            ) && material_at(outside) == 0
            {
                props.push(prop);
            }
        }
        for z in 0..SECTION_SIZE {
            for x in 0..SECTION_SIZE {
                let column = columns[x + padding + (z + padding) * size];
                let ground = (column.height - 0.5).floor() as i64;
                let y = ground - origin[1];
                if !(0..SECTION_SIZE as i64).contains(&y) {
                    continue;
                }
                let global = [origin[0] + x as i64, ground, origin[2] + z as i64];
                if garden_supports.contains(&global) {
                    continue;
                }
                let index = x + padding + size * (y as usize + padding + size * (z + padding));
                let material = padded[index];
                if original_ground.as_ref().is_some_and(|original| {
                    let [support, obstruction] = original[x + z * SECTION_SIZE];
                    support != material || obstruction != 0
                }) {
                    continue;
                }
                if ![
                    MaterialId::Grass as u16,
                    MaterialId::Sand as u16,
                    MaterialId::Stone as u16,
                    MaterialId::Dirt as u16,
                    MaterialId::Clay as u16,
                    MaterialId::Gravel as u16,
                    MaterialId::Mud as u16,
                    MaterialId::CoralRed as u16,
                    MaterialId::CoralBlue as u16,
                    MaterialId::CoralYellow as u16,
                    MaterialId::CoralPurple as u16,
                ]
                .contains(&material)
                    && !(matches!(material, 541..=544) && column.water_level.is_some())
                {
                    continue;
                }
                let above = [global[0], global[1] + 1, global[2]];
                let exposed = if y as usize + padding + 1 < size {
                    padded[index + size] == 0
                } else {
                    // Headless jobs omit mesh padding but must generate the same logical props.
                    edits
                        .get(&above)
                        .copied()
                        .or_else(|| structure_at(above))
                        .unwrap_or_else(|| {
                            trees
                                .iter()
                                .find_map(|tree| tree.wood_at(above))
                                .or_else(|| trees.iter().find_map(|tree| tree.leaves_at(above)))
                                .map_or(0, |material| material as u16)
                        })
                        == 0
                };
                if !exposed {
                    continue;
                }
                let cover = if column
                    .water_level
                    .is_some_and(|level| global[1] as f64 + 1.0 <= level)
                {
                    // Marine habitat fields are already in the prepared column.
                    super::GroundCover::default()
                } else {
                    let cover = self.terrain.ground_cover_with_biome(
                        global[0] as f64 + 0.5,
                        global[2] as f64 + 0.5,
                        column.biome.weights,
                    );
                    super::vegetation::understory_cover(trees, global, cover)
                };
                for prop in self
                    .terrain
                    .ground_props(global, position, material, cover, column)
                    .into_iter()
                    .flatten()
                {
                    if prop.kind.aquatic() {
                        // Tall fronds must also clear neighbouring ledges and player edits.
                        // The narrow footprint keeps this bounded, using cached columns locally.
                        let [min, max] = prop.bounds();
                        let mut clear = true;
                        'footprint: for pz in min[2].floor() as i64..=max[2].floor() as i64 {
                            for px in min[0].floor() as i64..=max[0].floor() as i64 {
                                let lx = px - start[0];
                                let lz = pz - start[2];
                                let surface = if (0..size as i64).contains(&lx)
                                    && (0..size as i64).contains(&lz)
                                {
                                    columns[lx as usize + lz as usize * size]
                                } else {
                                    self.terrain
                                        .surface_column(px as f64 + 0.5, pz as f64 + 0.5)
                                };
                                for py in prop.position[1].floor() as i64..max[1].ceil() as i64 {
                                    let cell = [px, py, pz];
                                    let material = edits.get(&cell).copied().unwrap_or_else(|| {
                                        let ly = py - start[1];
                                        if [lx, ly, lz].iter().all(|v| (0..size as i64).contains(v))
                                        {
                                            padded[lx as usize
                                                + size * (ly as usize + size * lz as usize)]
                                        } else {
                                            self.terrain_block(cell, surface)
                                        }
                                    });
                                    if MaterialId::from_raw(material)
                                        .is_some_and(MaterialId::is_solid)
                                    {
                                        clear = false;
                                        break 'footprint;
                                    }
                                }
                            }
                        }
                        if !clear {
                            continue;
                        }
                    }
                    props.push(prop);
                }
            }
        }
        let mut fluid_wake = Vec::new();
        // A pristine prepared column already owns all of its grounded flow.
        // Only a modified channel can expose a new unplanned opening.
        if self.terrain.config().water && !edits.is_empty() {
            for z in padding..size - padding {
                for x in padding..size - padding {
                    let c = columns[x + size * z];
                    if !c
                        .water_columns()
                        .intervals
                        .into_iter()
                        .flatten()
                        .any(|water| {
                            water.level > (position.y * 16) as f64
                                && water.bed < (position.y * 16 + 16) as f64
                        })
                    {
                        continue;
                    }
                    let global = [start[0] + x as i64, start[2] + z as i64];
                    let neighbors = crate::water::outlets::HORIZONTAL.map(|[dx, dz]| {
                        let lx = x as i64 + dx;
                        let lz = z as i64 + dz;
                        if (0..size as i64).contains(&lx) && (0..size as i64).contains(&lz) {
                            columns[lx as usize + size * lz as usize]
                        } else {
                            let [nx, nz] = [global[0] + dx, global[1] + dz];
                            let mut outside = self
                                .terrain
                                .surface_column(nx as f64 + 0.5, nz as f64 + 0.5);
                            for site in column
                                .structures
                                .iter()
                                .filter(|site| site.summary.kind.dragon().is_some())
                            {
                                site.clip_water(&mut outside, nx, nz);
                            }
                            outside
                        }
                    });
                    fluid_wake.extend(crate::water::natural_outlets(
                        global,
                        position.y,
                        c,
                        neighbors,
                        |p| {
                            edits.get(&p).copied().unwrap_or_else(|| {
                                let local: [i64; 3] = std::array::from_fn(|i| p[i] - start[i]);
                                if local.iter().all(|v| (0..size as i64).contains(v)) {
                                    padded[local[0] as usize
                                        + size * (local[1] as usize + size * local[2] as usize)]
                                } else {
                                    // Unpadded server jobs and vertical boundary probes
                                    // must see the same lair lining as the adjacent job.
                                    structure_at(p).unwrap_or_else(|| {
                                        let surface = self
                                            .terrain
                                            .surface_column(p[0] as f64 + 0.5, p[2] as f64 + 0.5);
                                        self.terrain_block(p, surface)
                                    })
                                }
                            })
                        },
                    ));
                }
            }
        }
        fluid_wake.sort_unstable();
        fluid_wake.dedup();
        Ok(GeneratedSection {
            data: BlockSection::from_blocks(position, revision, blocks)?,
            mesh,
            surface_sections,
            water,
            props,
            fluid_wake,
        })
    }
    fn mesh_segment_profile(
        &self,
        global: [i64; 3],
        material: u16,
        edits: &BlockOverrides,
        column: &PreparedColumn,
        start: [i64; 3],
        padded: &[u16],
    ) -> crate::segments::Profile {
        let size = column.size;
        crate::segments::profile(global, material, |p| {
            let [x, y, z] = std::array::from_fn(|a| (p[a] - start[a]) as usize);
            if y < size {
                Some(padded[x + size * (y + size * z)])
            } else {
                Some(edits.get(&p).copied().unwrap_or_else(|| {
                    self.segment_terrain_block(p, material, column.columns[x + size * z])
                }))
            }
        })
    }

    /// Render refreshes reuse their column cache while sampling across section boundaries.
    pub fn segment_profile(
        &self,
        global: [i64; 3],
        material: u16,
        edits: &BlockOverrides,
        mut column_at: impl FnMut([i64; 2]) -> SurfaceColumn,
    ) -> crate::segments::Profile {
        crate::segments::profile(global, material, |p| {
            Some(edits.get(&p).copied().unwrap_or_else(|| {
                self.segment_terrain_block(p, material, column_at([p[0], p[2]]))
            }))
        })
    }

    fn terrain_voxels(&self, start: [i64; 3], size: usize, columns: &[SurfaceColumn]) -> Vec<u16> {
        let mut blocks = vec![0; size * size * size];
        let config = self.terrain.config();
        let end_y = start[1] + size as i64;
        for (index, column) in columns.iter().enumerate() {
            let x = index % size;
            let z = index / size;
            // Trees and structures are stamped later. Modern cave openings reach two
            // metres above the ground, aquatic plants and lilies can reach higher.
            // Legacy cave decorations may follow chamber ceilings independently.
            let until = if config.cave_structure_revision >= 3 {
                let top = column.height + f64::from(column.seagrass_height.max(2));
                let top = column.water_level.map_or(top, |level| top.max(level + 1.0));
                let top = top.max(column.lava_level.unwrap_or(top));
                let top = if config.bounded_world {
                    top.max(-120.0)
                } else {
                    top
                };
                end_y.min(top.ceil() as i64 + 1)
            } else {
                end_y
            };
            let mut offset = x + size * size * z;
            for y in start[1]..until {
                blocks[offset] =
                    self.terrain_block([start[0] + x as i64, y, start[2] + z as i64], *column);
                offset += size;
            }
        }
        blocks
    }

    fn segment_terrain_block(&self, p: [i64; 3], material: u16, column: SurfaceColumn) -> u16 {
        if crate::segments::is_crystal(material) {
            self.unchecked_terrain_block(p, column)
        } else {
            self.terrain_block(p, column)
        }
    }

    pub fn block_at(&self, global: [i64; 3]) -> Result<u16, SectionGenerationError> {
        if global
            .iter()
            .any(|v| !(-MAX_SAMPLE_COORDINATE..=MAX_SAMPLE_COORDINATE).contains(v))
        {
            return Err(SectionGenerationError::PrecisionLimit(
                terra_voxel::global_to_section(global).0,
            ));
        }
        if self.terrain.config().bounded_world && !(-128..=356).contains(&global[1]) {
            return Ok(0);
        }
        if let Some(material) = self.terrain.structure_block(global) {
            return Ok(material);
        }
        if let Some(material) = self.terrain.natural_material_at(global) {
            return Ok(material);
        }
        let column = self
            .terrain
            .surface_column(global[0] as f64 + 0.5, global[2] as f64 + 0.5);
        let terrain = self.terrain_block(global, column);
        let terrain = if (matches!(terrain, 3 | 308)
            || self.terrain.config().cave_structure_revision >= 4
                && matches!(terrain, 13 | 57 | 58))
            && self.terrain.config().ores.enabled
        {
            self.terrain
                .ore_at(global, column)
                .map_or(terrain, |ore| ore.material())
        } else {
            terrain
        };
        if terrain != 0 {
            return Ok(terrain);
        }
        let trees = trees_in_region(&self.terrain, global, 1);
        Ok(trees
            .iter()
            .find_map(|tree| tree.wood_at(global))
            .or_else(|| trees.iter().find_map(|tree| tree.leaves_at(global)))
            .map_or(0, |material| material as u16))
    }
    pub(super) fn terrain_block(&self, global: [i64; 3], column: SurfaceColumn) -> u16 {
        let material = self.unchecked_terrain_block(global, column);
        // Validate after all cave, connector, roof and surface clipping, against
        // unfiltered terrain. This also repairs isolated growth in old seeds.
        if crate::segments::is_crystal(material)
            && crate::segments::mineral_support(global, material, |p| {
                Some(self.unchecked_terrain_block(p, column))
            }) != Some(true)
        {
            return 0;
        }
        if let Some(dy) = crate::segments::fixed_mineral_root(material)
            && !crate::segments::is_crystal(material)
            && !crate::segments::root_block(
                self.unchecked_terrain_block([global[0], global[1] + dy, global[2]], column),
            )
        {
            return 0;
        }
        if matches!(material, 54 | 55)
            && crate::segments::hanging_support(global, |p| {
                Some(self.unchecked_terrain_block(p, column))
            }) != Some(true)
        {
            return 0;
        }
        material
    }

    fn unchecked_terrain_block(&self, global: [i64; 3], column: SurfaceColumn) -> u16 {
        let p = global.map(|v| v as f64 + 0.5);
        let sample = self.terrain.sample_with_column(p, column);
        if sample.density <= 0.0 {
            let surface = (column.height - 0.5).floor() as i64;
            return if sample.material == MaterialId::Grass as u16
                && self.terrain.config().cave_structure_revision < 4
                && global[1] < surface
                && column.cave.kind != super::CaveEcology::Lush
            {
                MaterialId::Dirt as u16
            } else {
                sample.material
            };
        }
        let ground = (column.height - 0.5).floor() as i64;
        if global[1] > ground && global[1] <= ground + i64::from(column.seagrass_height) {
            return MaterialId::Seagrass as u16;
        }
        0
    }
}
