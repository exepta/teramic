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
}
impl SectionGenerator {
    pub fn new(terrain: TerraGenerator) -> Self {
        Self { terrain }
    }
    pub fn terrain(&self) -> &TerraGenerator {
        &self.terrain
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
        if column.coordinate != [position.x, position.z]
            || !column.config.same_terrain(self.terrain.config())
        {
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
        if edits.is_empty() && start[1] as f64 + 0.5 > column.maximum {
            return Ok(GeneratedSection {
                data: BlockSection::from_blocks(position, revision, vec![0; SECTION_VOLUME])?,
                mesh: with_mesh.then(BlockMesh::default),
                water: None,
                props: Vec::new(),
                fluid_wake: Vec::new(),
                surface_sections: column.surface_sections,
            });
        }
        let mut padded = vec![0; size * size * size];
        for z in 0..size {
            for y in 0..size {
                for x in 0..size {
                    let global = [
                        start[0] + x as i64,
                        start[1] + y as i64,
                        start[2] + z as i64,
                    ];
                    padded[x + size * (y + size * z)] =
                        self.terrain_block(global, columns[x + z * size]);
                }
            }
        }
        self.terrain.stamp_ores(start, size, columns, &mut padded);
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
                        trees
                            .iter()
                            .find_map(|tree| tree.wood_at(above))
                            .or_else(|| trees.iter().find_map(|tree| tree.leaves_at(above)))
                            .map_or(0, |material| material as u16)
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
        for z in 0..SECTION_SIZE {
            for y in 0..SECTION_SIZE {
                let source = padding + size * (y + padding + size * (z + padding));
                let target = SECTION_SIZE * (y + SECTION_SIZE * z);
                blocks[target..target + SECTION_SIZE]
                    .copy_from_slice(&padded[source..source + SECTION_SIZE]);
            }
        }
        let mesh = if with_mesh {
            let mut mesh = mesh_section(&padded)?;
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
                        let surface = columns[x + padding + (z + padding) * size];
                        let profile = self.segment_profile(global, material, surface, edits);
                        mesh.segments.push(terra_voxel::meshing::BlockSegment {
                            material,
                            cell,
                            below: profile.below,
                            above: profile.above,
                            hanging: profile.hanging,
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
        let mut props = Vec::new();
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
                        if padded[index] == 0
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
                                .unwrap_or_else(|| self.terrain_block(p, column));
                            if material != 0 || self.terrain_block(p, column) != 0 {
                                clear = false;
                                break;
                            }
                        }
                        if clear
                            && edits.get(&support).is_none_or(|m| *m != 0)
                            && self.terrain_block(support, column) != 0
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
                {
                    continue;
                }
                let above = [global[0], global[1] + 1, global[2]];
                let exposed = if y as usize + padding + 1 < size {
                    padded[index + size] == 0
                } else {
                    // Headless jobs omit mesh padding but must generate the same logical props.
                    edits.get(&above).copied().unwrap_or_else(|| {
                        trees
                            .iter()
                            .find_map(|tree| tree.wood_at(above))
                            .or_else(|| trees.iter().find_map(|tree| tree.leaves_at(above)))
                            .map_or(0, |material| material as u16)
                    }) == 0
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
        if self.terrain.config().cave_structure_revision >= 4 && self.terrain.config().water {
            for z in padding..size - padding {
                for x in padding..size - padding {
                    let c = columns[x + size * z].cave;
                    if let Some(p) = c.fluid_outlet(
                        (start[0] + x as i64) as f64 + 0.5,
                        (start[2] + z as i64) as f64 + 0.5,
                    ) && p[1].div_euclid(16) == position.y
                    {
                        fluid_wake.push(p);
                    }
                }
            }
        }
        Ok(GeneratedSection {
            data: BlockSection::from_blocks(position, revision, blocks)?,
            mesh,
            surface_sections,
            water,
            props,
            fluid_wake,
        })
    }
    /// Cached horizontal data keeps remote profile refreshes off the main thread.
    pub fn segment_profile(
        &self,
        global: [i64; 3],
        material: u16,
        column: SurfaceColumn,
        edits: &BlockOverrides,
    ) -> crate::segments::Profile {
        crate::segments::profile(global, material, |p| {
            Some(
                edits
                    .get(&p)
                    .copied()
                    .unwrap_or_else(|| self.terrain_block(p, column)),
            )
        })
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
        let column = self
            .terrain
            .surface_column(global[0] as f64 + 0.5, global[2] as f64 + 0.5);
        let terrain = self.terrain_block(global, column);
        let terrain = if (terrain == 3
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
    fn terrain_block(&self, global: [i64; 3], column: SurfaceColumn) -> u16 {
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
