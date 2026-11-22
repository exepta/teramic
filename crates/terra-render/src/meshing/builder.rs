use super::DetailLevel;
use super::connectivity::{connected_position, connected_turf_top};
use crate::blocks::definitions::{BlockRenderKind, DetailDensity};
use crate::blocks::{PrimitiveTemplate, TemplateSet, projected_tangent};
use bevy::prelude::*;
use std::collections::BTreeMap;
use terra_voxel::blocks::{SECTION_SIZE, SECTION_VOLUME};
use terra_voxel::{SectionPos, meshing::BlockFace};

#[path = "block_tints.rs"]
mod block_tints;
#[path = "condition_faces.rs"]
pub(super) mod condition_faces;

#[derive(Clone)]
pub(crate) struct RenderOptions {
    pub water_level: Option<f64>,
    pub detail_level: DetailLevel,
    pub terrain: Option<terra_world::TerraGenerator>,
    pub exposure: Option<std::sync::Arc<crate::weather::surfaces::exposure::Exposure>>,
    pub conditions: terra_world::environment::SurfaceSnapshot,
    pub tints: std::sync::Arc<std::collections::HashMap<[usize; 3], [u8; 3]>>,
}

#[derive(Default, Debug, PartialEq)]
pub(crate) struct MeshBatch {
    pub staged: Option<super::staging::StagedMesh>,
    pub shadow_only: bool,
    pub shadow_sources: Vec<usize>,
    pub material: usize,
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    /// Empty or aligned to positions; retained for authored normal-mapped meshes.
    pub tangents: Vec<[f32; 4]>,
    pub uvs: Vec<[f32; 2]>,
    /// Optional UV1 weights: rooted terrain wind, or prop growth timing/ratio.
    pub rooted_wind: Vec<[f32; 2]>,
    pub surface_water: Vec<[f32; 2]>,
    pub bile_motion: Vec<[f32; 4]>,
    pub growth_anchors: Vec<[f32; 3]>,
    /// CPU-only prop ranges for refreshing growth timing after worker completion.
    pub growth_ranges: Vec<(terra_world::props::PropId, std::ops::Range<usize>)>,
    /// CPU-only source cells; micro geometry must not infer provenance from its surface.
    pub foliage_owners: Vec<FoliageOwner>,
    pub colors: Vec<[f32; 4]>,
    /// Worker-prepared HDR tints. The main thread only moves this GPU payload.
    pub half_colors: Option<bevy::mesh::VertexAttributeValues>,
    pub indices: Vec<u32>,
    pub compact_indices: Vec<u16>,
    pub packed_normals: Vec<[i8; 4]>,
    pub packed_tangents: Vec<[i8; 4]>,
    pub packed_colors: Vec<[u8; 4]>,
}

impl MeshBatch {
    /// Worker-only, after geometry/AO edits. PBR and rooted-fibre shaders read
    /// normalized frames directly, including exact tangent handedness at +/-1.
    pub(crate) fn pack_static_frame(&mut self) {
        if self.staged.is_some()
            || self.shadow_only
            || (!self.rooted_wind.is_empty() && !rooted_frame_packing_enabled())
        {
            return;
        }
        self.finish_tangents();
        if self.packed_normals.is_empty()
            && self.normals.len() == self.positions.len()
            && self.normals.iter().all(|normal| unit_direction(*normal))
        {
            self.packed_normals = std::mem::take(&mut self.normals)
                .into_iter()
                .map(crate::foliage::packing::normal)
                .collect();
            self.packed_normals.shrink_to_fit();
        }
        if self.packed_tangents.is_empty()
            && self.tangents.len() == self.positions.len()
            && self.tangents.iter().all(|tangent| {
                unit_direction([tangent[0], tangent[1], tangent[2]])
                    && tangent[3].is_finite()
                    && (tangent[3].abs() - 1.0).abs() <= 0.001
            })
        {
            self.packed_tangents = std::mem::take(&mut self.tangents)
                .into_iter()
                .map(|tangent| {
                    let mut packed =
                        crate::foliage::packing::normal([tangent[0], tangent[1], tangent[2]]);
                    packed[3] = if tangent[3] < 0.0 { -127 } else { 127 };
                    packed
                })
                .collect();
            self.packed_tangents.shrink_to_fit();
        }
    }

    pub(crate) fn append_tangents(&mut self, start: usize, values: impl Iterator<Item = [f32; 4]>) {
        let mut values = values.peekable();
        if self.tangents.is_empty() && values.peek().is_none() {
            return;
        }
        self.fill_tangents(start);
        self.tangents.extend(values);
        self.fill_tangents(self.positions.len());
    }

    pub(crate) fn complete_tangents(&self, tangents: &mut Vec<[f32; 4]>, count: usize) {
        // Untextured detail can be appended after a normal-mapped primitive.
        // Bevy truncates the whole mesh to its shortest attribute at extraction.
        tangents.extend((tangents.len()..count).map(|index| {
            let normal = self
                .normals
                .get(index)
                .copied()
                .or_else(|| {
                    self.packed_normals
                        .get(index)
                        .map(|n| std::array::from_fn(|i| f32::from(n[i]) / 127.))
                })
                .unwrap_or([0., 1., 0.]);
            crate::blocks::tangent_frame(Vec3::X, Vec3::from_array(normal), 1.)
        }));
    }

    fn fill_tangents(&mut self, length: usize) {
        if self.tangents.len() < length {
            let mut tangents = std::mem::take(&mut self.tangents);
            self.complete_tangents(&mut tangents, length);
            self.tangents = tangents;
        }
    }

    pub(crate) fn finish_tangents(&mut self) {
        if !self.tangents.is_empty() {
            self.fill_tangents(self.positions.len());
        }
    }

    /// Final worker step, after all geometry edits. Smaller section batches do
    /// not need four-byte indices in ready queues or uploaded GPU buffers.
    pub(crate) fn compact_indices(&mut self) {
        self.finish_tangents();
        if self.indices.is_empty()
            || self.positions.len() > usize::from(u16::MAX) + 1
            || self
                .indices
                .iter()
                .any(|&index| index > u32::from(u16::MAX))
        {
            return;
        }
        self.compact_indices = Vec::with_capacity(self.indices.len());
        self.compact_indices.extend(
            std::mem::take(&mut self.indices)
                .into_iter()
                .map(|index| index as u16),
        );
    }

    pub(crate) fn has_indices(&self) -> bool {
        !self.indices.is_empty() || !self.compact_indices.is_empty()
    }

    pub(crate) fn take_indices(&mut self) -> bevy::mesh::Indices {
        if self.compact_indices.is_empty() {
            bevy::mesh::Indices::U32(std::mem::take(&mut self.indices))
        } else {
            bevy::mesh::Indices::U16(std::mem::take(&mut self.compact_indices))
        }
    }
}

fn unit_direction(direction: [f32; 3]) -> bool {
    direction.iter().all(|value| value.is_finite())
        && (Vec3::from_array(direction).length_squared() - 1.0).abs() <= 0.001
}

pub(super) fn static_frame_packing_enabled() -> bool {
    static ENABLED: std::sync::LazyLock<bool> = std::sync::LazyLock::new(|| {
        !(std::env::args().any(|argument| argument.starts_with("--smoke-test"))
            && std::env::var("TERRA_PACKED_TERRAIN").is_ok_and(|value| value == "0"))
    });
    *ENABLED
}

fn rooted_frame_packing_enabled() -> bool {
    static ENABLED: std::sync::LazyLock<bool> = std::sync::LazyLock::new(|| {
        !(std::env::args().any(|argument| argument.starts_with("--smoke-test"))
            && std::env::var("TERRA_PACKED_ROOTED").is_ok_and(|value| value == "0"))
    });
    *ENABLED
}

#[cfg(test)]
#[path = "index_packing_tests.rs"]
mod index_packing_tests;

#[derive(Debug, PartialEq)]
pub(crate) struct FoliageOwner {
    pub vertices: std::ops::Range<usize>,
    pub cell: [u32; 3],
    pub material: u16,
}

#[derive(Default)]
struct AuthoredFaceCache(BTreeMap<(u16, usize, u8), Vec<PrimitiveTemplate>>);

fn coarse_authored(templates: &TemplateSet, material: u16) -> bool {
    // Dispatch replaces only authored terrain with its validated cube fallback
    // after micro-detail hysteresis. A nearby original remains CustomMesh.
    templates.opaque_cube_lods.contains(&material)
        && templates.cube_lods.contains_key(&material)
        && templates
            .blocks
            .get(&material)
            .is_some_and(|block| block.render_kind == BlockRenderKind::Cube)
}

impl AuthoredFaceCache {
    const LIMIT: usize = 6 * 64;

    fn append(
        &mut self,
        batches: &mut BTreeMap<usize, MeshBatch>,
        material: u16,
        block: &crate::blocks::BlockTemplate,
        side: usize,
        offset: Vec3,
        mask: u8,
    ) -> Result<(), String> {
        let key = (material, side, mask);
        if !self.0.contains_key(&key) {
            if self.0.len() == Self::LIMIT {
                for primitive in &block.faces[side] {
                    let batch = batches
                        .entry(primitive.material)
                        .or_insert_with(|| MeshBatch {
                            material: primitive.material,
                            ..default()
                        });
                    append_connected(
                        batch,
                        primitive,
                        offset,
                        mask,
                        block.bevel_width,
                        None,
                        material == 1 && side == 3,
                    )?;
                }
                return Ok(());
            }
            let prepared = block.faces[side]
                .iter()
                .map(|primitive| {
                    let mut batch = MeshBatch::default();
                    append_connected(
                        &mut batch,
                        primitive,
                        Vec3::ZERO,
                        mask,
                        block.bevel_width,
                        None,
                        material == 1 && side == 3,
                    )?;
                    Ok(PrimitiveTemplate {
                        material: primitive.material,
                        positions: batch.positions,
                        normals: batch.normals,
                        tangents: batch.tangents,
                        uvs: batch.uvs,
                        indices: batch.indices,
                    })
                })
                .collect::<Result<Vec<_>, String>>()?;
            self.0.insert(key, prepared);
        }
        for primitive in &self.0[&key] {
            let batch = batches
                .entry(primitive.material)
                .or_insert_with(|| MeshBatch {
                    material: primitive.material,
                    ..default()
                });
            let start =
                u32::try_from(batch.positions.len()).map_err(|_| "Section mesh index overflow")?;
            batch.positions.extend(
                primitive
                    .positions
                    .iter()
                    .map(|&point| (Vec3::from_array(point) + offset).to_array()),
            );
            batch.normals.extend_from_slice(&primitive.normals);
            batch.uvs.extend_from_slice(&primitive.uvs);
            batch.append_tangents(start as usize, primitive.tangents.iter().copied());
            for &index in &primitive.indices {
                batch.indices.push(
                    start
                        .checked_add(index)
                        .ok_or("Section mesh index overflow")?,
                );
            }
        }
        Ok(())
    }
}

#[cfg(test)]
pub(crate) fn compose_section(
    faces: &[BlockFace],
    templates: &TemplateSet,
    section: SectionPos,
    water_level: Option<f64>,
) -> Result<Vec<MeshBatch>, String> {
    compose_section_with_options(
        faces,
        templates,
        section,
        &RenderOptions {
            water_level,
            detail_level: DetailLevel::Near,
            terrain: None,
            exposure: None,
            tints: Default::default(),
            conditions: Default::default(),
        },
    )
}
pub(crate) fn compose_section_with_options(
    faces: &[BlockFace],
    templates: &TemplateSet,
    section: SectionPos,
    options: &RenderOptions,
) -> Result<Vec<MeshBatch>, String> {
    compose_section_with_growth(faces, templates, section, options, &[])
}

pub(crate) fn compose_section_with_growth(
    faces: &[BlockFace],
    templates: &TemplateSet,
    section: SectionPos,
    options: &RenderOptions,
    growth: &[([u8; 3], u8, f32)],
) -> Result<Vec<MeshBatch>, String> {
    compose_section_with_segments(faces, templates, section, options, growth, &[])
}

pub(crate) fn compose_section_with_segments(
    faces: &[BlockFace],
    templates: &TemplateSet,
    section: SectionPos,
    options: &RenderOptions,
    growth: &[([u8; 3], u8, f32)],
    segments: &[terra_voxel::meshing::BlockSegment],
) -> Result<Vec<MeshBatch>, String> {
    let origin = section.origin().map_err(|e| e.to_string())?;
    let growth: BTreeMap<_, _> = growth
        .iter()
        .map(|&(cell, side, height)| ((cell.map(u32::from), usize::from(side)), height))
        .collect();
    let mut batches: BTreeMap<usize, MeshBatch> = BTreeMap::new();
    // Only authored join poses are cached, and only for this worker composition.
    let mut authored_faces = AuthoredFaceCache::default();
    let mut emitted_segments = [false; SECTION_VOLUME];
    // Partial cells remain visible through adjacent partial cells, even when
    // the cube mesher correctly finds no exposed full-cell interfaces.
    for segment in segments {
        let cell = segment.cell.map(u32::from);
        if cell.iter().any(|&v| v >= 16) || !terra_world::segments::is_segment(segment.material) {
            return Err("Invalid segment metadata".into());
        }
        let index = cell_index(cell);
        if emitted_segments[index] {
            continue;
        }
        let block = templates
            .blocks
            .get(&segment.material)
            .ok_or_else(|| format!("No block template for segment {}", segment.material))?;
        super::segments::append(
            &mut batches,
            block.face_materials[0],
            segment.material,
            std::array::from_fn(|i| origin[i] + i64::from(cell[i])),
            cell,
            terra_world::segments::Profile {
                below: segment.below,
                above: segment.above,
                hanging: segment.hanging,
                joined: segment.joined,
                fan: segment.fan,
            },
            options.detail_level,
        )?;
        emitted_segments[index] = true;
    }
    let mut visibility = [0_u8; SECTION_VOLUME];
    let mut open_neighbors = [0_u8; SECTION_VOLUME];
    let ground = options
        .terrain
        .as_ref()
        .filter(|_| {
            faces
                .iter()
                .any(|face| matches!(face.material, 1 | 4 | 5 | 306 | 400))
        })
        .map(|t| crate::appearance::GroundTint::new(t, origin));
    let tint = super::tree_appearance::SurfaceTint {
        caves: options
            .terrain
            .as_ref()
            .filter(|t| {
                t.config().cave_structure_revision > 0
                    && faces.iter().any(|f| {
                        matches!(f.material, 1 | 2 | 30 | 37 | 39 | 13)
                            || t.config().cave_structure_revision >= 3 && f.material == 3
                    })
            })
            .map(|terrain| {
                Box::new(std::array::from_fn(|i| {
                    let x = (origin[0] + (i % 16) as i64) as f64 + 0.5;
                    let z = (origin[2] + (i / 16) as i64) as f64 + 0.5;
                    (
                        terrain.cave_structure_at(
                            (origin[0] + (i % 16) as i64) as f64 + 0.5,
                            (origin[2] + (i / 16) as i64) as f64 + 0.5,
                        ),
                        terrain.cave_biome_column(x, z),
                    )
                }))
            }),
        ground,
        trees: options
            .terrain
            .as_ref()
            .filter(|_| {
                faces.iter().any(|face| {
                    terra_voxel::MaterialId::FOLIAGE.contains(&face.material)
                        || matches!(face.material, 6 | 10 | 36 | 134 | 313)
                })
            })
            .map(|terrain| terrain.tree_appearance(origin)),
        origin,
    };
    let snow_climate = options
        .terrain
        .as_ref()
        .map(|terrain| crate::weather::storage::snow::SnowClimate::new(terrain, origin));
    // Exposure already includes read-only neighbor padding, including across sections.
    for &face in faces {
        validate_face(face)?;
        for cell in cells(face) {
            visibility[cell_index(cell)] |= 1 << face.side;
            if !face.occupied_neighbor {
                open_neighbors[cell_index(cell)] |= 1 << face.side;
            }
        }
    }
    let mut tree_faces = Vec::with_capacity(faces.len());
    for &face in faces {
        if face.side != 2
            && matches!(face.material, 1 | 4 | 5 | 306 | 400)
            && (options.detail_level != DetailLevel::Near
                || coarse_authored(templates, face.material))
            && let Some(ground) = &tint.ground
        {
            tree_faces.extend(super::tint_faces::split(
                face,
                ground,
                templates.cube_lods.contains_key(&face.material),
            ));
        } else if tint.trees.is_some()
            && options.detail_level != DetailLevel::Near
            && matches!(face.material, 6..=10)
        {
            tree_faces.extend(tint.split(face));
        } else if face.material == 103 && !options.tints.is_empty() {
            tree_faces.extend(block_tints::split(face, &options.tints));
        } else {
            tree_faces.push(face);
        }
    }
    for face in &tree_faces {
        let base_block = templates
            .blocks
            .get(&face.material)
            .ok_or_else(|| format!("No block template for material {}", face.material))?;
        if terra_world::segments::is_segment(face.material) {
            for cell in cells(*face) {
                if emitted_segments[cell_index(cell)] {
                    continue;
                }
                super::segments::append(
                    &mut batches,
                    base_block.face_materials[0],
                    face.material,
                    std::array::from_fn(|i| origin[i] + i64::from(cell[i])),
                    cell,
                    Default::default(),
                    options.detail_level,
                )?;
                emitted_segments[cell_index(cell)] = true;
            }
            continue;
        }
        if let Some(&frame) = templates.frame_materials.get(&face.material) {
            super::glass::append(
                &mut batches,
                *face,
                base_block.face_materials[face.side],
                frame,
            )?;
            continue;
        }
        let block = if options.detail_level == DetailLevel::Near {
            base_block
        } else {
            templates
                .cube_lods
                .get(&face.material)
                .unwrap_or(base_block)
        };
        if block.render_kind == BlockRenderKind::Cube
            && terra_voxel::MaterialId::FOLIAGE.contains(&face.material)
            && options.detail_level != DetailLevel::Far
            && templates.block_details.leaf_geometry_quality != terra_core::BlockDetailQuality::Low
        {
            for cell in cells(*face) {
                let mask = visibility[cell_index(cell)];
                if face.side == mask.trailing_zeros() as usize {
                    let global = std::array::from_fn(|axis| origin[axis] + i64::from(cell[axis]));
                    let condition = options
                        .conditions
                        .get(&cell.map(|v| v as usize))
                        .copied()
                        .unwrap_or_default();
                    let crown = base_block;
                    let cluster = templates.foliage_clusters.get(&face.material).copied();
                    let mut starts = BTreeMap::new();
                    let tree_tint = tint.tree_tint(cell, face.material);
                    if condition != Default::default() || tree_tint != [1.0; 3] {
                        for slot in crown
                            .face_materials
                            .into_iter()
                            .chain(cluster.into_iter().flat_map(|cluster| cluster.slots()))
                        {
                            starts.entry(slot).or_insert_with(|| {
                                batches.get(&slot).map_or(0, |batch| batch.positions.len())
                            });
                        }
                    }
                    super::tree_details::append_crown(
                        &mut batches,
                        crown,
                        global,
                        cell,
                        mask,
                        if templates.block_details.leaf_geometry_quality
                            == terra_core::BlockDetailQuality::High
                        {
                            options.detail_level
                        } else {
                            DetailLevel::Middle
                        },
                        super::tree_details::CrownStyle {
                            material: face.material,
                            species: tint.tree(cell, face.material),
                            cluster,
                        },
                    )?;
                    for (slot, start) in starts {
                        if let Some(batch) = batches.get_mut(&slot) {
                            batch.colors.resize(batch.positions.len(), [1.0; 4]);
                            let tint = crate::appearance::condition_tint(condition);
                            for color in &mut batch.colors[start..] {
                                for axis in 0..3 {
                                    color[axis] *= tint[axis] * tree_tint[axis];
                                }
                            }
                        }
                    }
                }
            }
            continue;
        }
        if block.render_kind == BlockRenderKind::Cube
            && (options.detail_level != DetailLevel::Near
                || coarse_authored(templates, face.material)
                || terra_voxel::MaterialId::FOLIAGE.contains(&face.material)
                    && templates.block_details.leaf_geometry_quality
                        == terra_core::BlockDetailQuality::Low)
        {
            let split = condition_faces::split_weather(
                *face,
                &options.conditions,
                options.exposure.as_deref(),
                origin[1],
            );
            for part in split.as_deref().unwrap_or(std::slice::from_ref(face)) {
                let starts = tint_starts(&batches, block, face.side);
                append_greedy(&mut batches, *part, block)?;
                tint_surface(
                    &mut batches,
                    starts,
                    *part,
                    part.minimum,
                    templates,
                    &tint,
                    options,
                    snow_climate.as_ref(),
                );
                if part.material == terra_voxel::MaterialId::Cactus as u16 {
                    for cell in cells(*part) {
                        if open_neighbors[cell_index(cell)] & (1 << part.side) != 0 {
                            super::cactus_details::append(
                                &mut batches,
                                templates
                                    .blocks
                                    .get(&(terra_voxel::MaterialId::WhiteClay as u16))
                                    .map_or(block.face_materials[part.side], |ivory| {
                                        ivory.face_materials[3]
                                    }),
                                Vec3::from_array(cell.map(|p| p as f32)),
                                std::array::from_fn(|axis| origin[axis] + i64::from(cell[axis])),
                                part.side,
                                options.detail_level,
                            )?;
                        }
                    }
                }
                if matches!(part.material, 20..=26) && options.detail_level == DetailLevel::Middle {
                    for cell in cells(*part) {
                        if open_neighbors[cell_index(cell)] & (1 << part.side) != 0 {
                            super::mushroom_details::append(
                                &mut batches,
                                templates,
                                block,
                                part.material,
                                std::array::from_fn(|axis| origin[axis] + i64::from(cell[axis])),
                                Vec3::from_array(cell.map(|coordinate| coordinate as f32)),
                                part.side,
                                options.detail_level,
                            )?;
                        }
                    }
                }
                if let Some(&material) = templates.ore_materials.get(&part.material) {
                    for cell in cells(*part) {
                        if open_neighbors[cell_index(cell)] & (1 << part.side) != 0 {
                            super::ore_details::append(
                                &mut batches,
                                material,
                                part.material,
                                std::array::from_fn(|axis| origin[axis] + i64::from(cell[axis])),
                                Vec3::from_array(cell.map(|coordinate| coordinate as f32)),
                                part.side,
                                (
                                    options.detail_level,
                                    growth.get(&(cell, part.side)).copied().unwrap_or(0.0),
                                ),
                            )?;
                        }
                    }
                }
            }
            continue;
        }
        for cell in cells(*face) {
            let starts = tint_starts(&batches, block, face.side);
            let translation = Vec3::from_array(cell.map(|i| i as f32));
            let connection = (block.bevel_width > 0.0)
                .then_some((open_neighbors[cell_index(cell)], block.bevel_width));
            if let Some(surface) = &block.surface {
                let mask = open_neighbors[cell_index(cell)];
                let global = std::array::from_fn(|i| origin[i] + i64::from(cell[i]));
                for patch in surface.face(
                    &block.faces[face.side],
                    face.side,
                    (variation(global, face.side as u64) % 4) as usize
                        + match templates.block_details.detail_quality {
                            terra_core::BlockDetailQuality::High => 0,
                            terra_core::BlockDetailQuality::Medium => 4,
                            terra_core::BlockDetailQuality::Low => 8,
                        },
                    mask,
                    |p| Vec3::from_array(connected_position(p.to_array(), mask, block.bevel_width)),
                ) {
                    let start = batches
                        .get(&patch.primitive.material)
                        .map_or(0, |b| b.positions.len());
                    append(
                        &mut batches,
                        &patch.primitive,
                        translation,
                        Quat::IDENTITY,
                        None,
                        Some(face.side),
                    )?;
                    if let Some(batch) = batches.get_mut(&patch.primitive.material) {
                        batch.colors.resize(batch.positions.len(), [1.0; 4]);
                        batch.colors[start..].fill([patch.shade, patch.shade, patch.shade, 1.0]);
                    }
                }
            } else if block.render_kind == BlockRenderKind::CustomMesh
                && templates.cube_lods.contains_key(&face.material)
                && let Some((mask, _)) = connection
            {
                authored_faces.append(
                    &mut batches,
                    face.material,
                    block,
                    face.side,
                    translation,
                    mask,
                )?;
            } else {
                for primitive in &block.faces[face.side] {
                    append(
                        &mut batches,
                        primitive,
                        translation,
                        Quat::IDENTITY,
                        connection,
                        (block.render_kind == BlockRenderKind::Cube).then_some(face.side),
                    )?;
                }
            }
            if block.render_kind == BlockRenderKind::Cube
                || templates.cube_lods.contains_key(&face.material)
            {
                tint_surface(
                    &mut batches,
                    starts,
                    *face,
                    cell,
                    templates,
                    &tint,
                    options,
                    snow_climate.as_ref(),
                );
            }
            if open_neighbors[cell_index(cell)] & (1 << face.side) != 0
                && let Some(&material) = templates.ore_materials.get(&face.material)
            {
                super::ore_details::append(
                    &mut batches,
                    material,
                    face.material,
                    std::array::from_fn(|axis| origin[axis] + i64::from(cell[axis])),
                    translation,
                    face.side,
                    (
                        options.detail_level,
                        growth.get(&(cell, face.side)).copied().unwrap_or(0.0),
                    ),
                )?;
            }
            if face.material == terra_voxel::MaterialId::Cactus as u16
                && open_neighbors[cell_index(cell)] & (1 << face.side) != 0
            {
                super::cactus_details::append(
                    &mut batches,
                    templates
                        .blocks
                        .get(&(terra_voxel::MaterialId::WhiteClay as u16))
                        .map_or(block.face_materials[face.side], |ivory| {
                            ivory.face_materials[3]
                        }),
                    translation,
                    std::array::from_fn(|axis| origin[axis] + i64::from(cell[axis])),
                    face.side,
                    options.detail_level,
                )?;
            }
            if options.detail_level == DetailLevel::Near
                && matches!(face.material, 20..=26)
                && open_neighbors[cell_index(cell)] & (1 << face.side) != 0
            {
                let global = std::array::from_fn(|i| origin[i] + i64::from(cell[i]));
                super::mushroom_details::append(
                    &mut batches,
                    templates,
                    block,
                    face.material,
                    global,
                    translation,
                    face.side,
                    options.detail_level,
                )?;
            }
            if options.detail_level == DetailLevel::Near
                && matches!(face.material, 6 | 10 | 36 | 134 | 313)
                && open_neighbors[cell_index(cell)] & (1 << 3) != 0
            {
                let global = std::array::from_fn(|i| origin[i] + i64::from(cell[i]));
                let seed = variation(global, face.side as u64);
                if seed.is_multiple_of(7)
                    && options.terrain.as_ref().is_some_and(|terrain| {
                        global[1] as f64
                            > terrain.surface_height(global[0] as f64, global[2] as f64) + 4.0
                    })
                {
                    super::tree_details::append_twig(
                        &mut batches,
                        block.face_materials[face.side],
                        translation,
                        face.side,
                        seed,
                    )?;
                }
            }
            if face.side == 3 && options.detail_level != DetailLevel::Far {
                let global = std::array::from_fn(|i| origin[i] + i64::from(cell[i]));
                let cover = (!block.details.is_empty())
                    .then(|| {
                        options
                            .terrain
                            .as_ref()
                            .map(|t| t.ground_cover(global[0] as f64 + 0.5, global[2] as f64 + 0.5))
                    })
                    .flatten();
                for (index, detail) in block.details.iter().enumerate() {
                    if options.detail_level == DetailLevel::Middle && detail.density_field.is_some()
                    {
                        continue;
                    }
                    let clearance = options
                        .water_level
                        .map(|level| global[1] as f64 + 1.0 - level);
                    if clearance.is_some_and(|height| height <= 0.0)
                        || detail.water_clearance.is_some_and(|[low, high]| {
                            clearance.is_none_or(|height| {
                                height < f64::from(low) || height > f64::from(high)
                            })
                        })
                    {
                        continue;
                    }
                    let random = variation(global, index as u64);
                    let density = cover.map_or(1.0, |c| match detail.density_field {
                        Some(DetailDensity::Grass) => c.grass,
                        Some(DetailDensity::Bush) => c.bushes,
                        Some(DetailDensity::Flower) => c.flowers,
                        Some(DetailDensity::Fern) => c.ferns,
                        None => 1.0,
                    });
                    if (random >> 40) as f32 / 16_777_216.0 < detail.probability * density {
                        let rotation = Quat::from_rotation_y(
                            (random & 3) as f32 * std::f32::consts::FRAC_PI_2,
                        );
                        for primitive in &detail.primitives {
                            append(&mut batches, primitive, translation, rotation, None, None)?;
                        }
                    }
                }
            }
        }
    }
    append_coastal_wash(&mut batches, templates, options, origin)?;
    for batch in batches.values_mut() {
        if templates.water_material(batch.material) {
            batch
                .colors
                .resize(batch.positions.len(), [0.0, 0.0, 0.0, 4.0]);
            for (color, uv) in batch.colors.iter_mut().zip(&mut batch.uvs) {
                if color[3] < 5.5 {
                    *color = [0.0, 0.0, 0.0, 4.0];
                    *uv = [1.0, 0.0];
                }
            }
        }
        if !batch.colors.is_empty() {
            batch.colors.resize(batch.positions.len(), [1.0; 4]);
        }
    }
    Ok(batches
        .into_values()
        .filter(|b| !b.indices.is_empty())
        .collect())
}

fn append_coastal_wash(
    batches: &mut BTreeMap<usize, MeshBatch>,
    templates: &TemplateSet,
    options: &RenderOptions,
    origin: [i64; 3],
) -> Result<(), String> {
    let (Some(exposure), Some(water)) = (&options.exposure, templates.blocks.get(&14)) else {
        return Ok(());
    };
    let slot = water.face_materials[3];
    for z in 0..16 {
        for x in 0..16 {
            let i = x + z * 16;
            let reach = exposure.coast[i];
            let roof = i64::from(exposure.roofs[i]);
            if reach <= 0.0 || roof.div_euclid(16) != origin[1].div_euclid(16) {
                continue;
            }
            let batch = batches.entry(slot).or_insert_with(|| MeshBatch {
                material: slot,
                ..Default::default()
            });
            let y = (roof - origin[1]) as f32 + 1.018;
            let steps = if options.detail_level == super::DetailLevel::Near {
                2
            } else {
                1
            };
            let corners = [
                exposure.coast_vertices[x + z * 17],
                exposure.coast_vertices[x + (z + 1) * 17],
                exposure.coast_vertices[x + 1 + (z + 1) * 17],
                exposure.coast_vertices[x + 1 + z * 17],
            ];
            for dz in 0..steps {
                for dx in 0..steps {
                    let first = u32::try_from(batch.positions.len())
                        .map_err(|_| "Coastal wash vertex overflow")?;
                    batch.colors.resize(first as usize, [0.0, 0.0, 0.0, 4.0]);
                    for [cx, cz] in [[dx, dz], [dx, dz + 1], [dx + 1, dz + 1], [dx + 1, dz]] {
                        let u = cx as f32 / steps as f32;
                        let v = cz as f32 / steps as f32;
                        let left = corners[0] + (corners[1] - corners[0]) * v;
                        let right = corners[3] + (corners[2] - corners[3]) * v;
                        let vertex_reach = left + (right - left) * u;
                        batch.positions.push([x as f32 + u, y, z as f32 + v]);
                        batch.normals.push([0.0, 1.0, 0.0]);
                        batch.uvs.push([0.035, reach]);
                        batch.colors.push([1.0 - vertex_reach, 0.0, 0.0, 6.0]);
                    }
                    batch.indices.extend([
                        first,
                        first + 1,
                        first + 2,
                        first,
                        first + 2,
                        first + 3,
                    ]);
                }
            }
        }
    }
    Ok(())
}

fn tint_starts(
    batches: &BTreeMap<usize, MeshBatch>,
    block: &crate::blocks::BlockTemplate,
    side: usize,
) -> [(usize, usize); 2] {
    let material = block.face_materials[side];
    [
        material,
        block.side_cap.map_or(material, |cap| cap.material),
    ]
    .map(|slot| {
        (
            slot,
            batches.get(&slot).map_or(0, |batch| batch.positions.len()),
        )
    })
}

#[allow(clippy::too_many_arguments)]
fn tint_surface(
    batches: &mut BTreeMap<usize, MeshBatch>,
    starts: [(usize, usize); 2],
    face: BlockFace,
    cell: [u32; 3],
    templates: &TemplateSet,
    tint: &super::tree_appearance::SurfaceTint,
    options: &RenderOptions,
    snow_climate: Option<&crate::weather::storage::snow::SnowClimate>,
) {
    if terra_voxel::MaterialId::from_raw(face.material).is_none() {
        return;
    }
    let mut condition = options
        .conditions
        .get(&cell.map(|v| v as usize))
        .copied()
        .unwrap_or_default();
    let mut cave_grass = false;
    if let Some(caves) = &tint.caves {
        let (cave, biome) = caves[cell[0] as usize + 16 * cell[2] as usize];
        let y = tint.origin[1] as f64 + f64::from(cell[1]) + 0.5;
        let regional = options
            .terrain
            .as_ref()
            .is_some_and(|t| t.config().cave_structure_revision >= 3);
        if !regional && cave.active() && (cave.floor - 2.5..=cave.ceiling + 2.5).contains(&y) {
            condition.wetness = condition.wetness.max(cave.dampness);
            cave_grass = cave.kind == terra_world::generation::CaveEcology::Lush;
        }
        if options.terrain.as_ref().is_some_and(|t| {
            regional
                && t.config().cave_biomes.enabled
                && (t.config().cave_biomes.connectors.inherit_biomes
                    || cave.active() && (cave.floor - 2.5..=cave.ceiling + 2.5).contains(&y))
        }) && biome.surface - y > 6.0
        {
            let sample = biome.at(y);
            condition.wetness = condition.wetness.max(sample.moisture * 0.55);
            cave_grass = sample.weights[2] > 0.5;
        }
    }
    let factor = crate::appearance::condition_tint(terra_world::environment::SurfaceCondition {
        wetness: 0.0,
        ..condition
    });
    let exposure = options.exposure.as_ref().map_or_else(
        || if condition.snow > 0.0 { 1.0 } else { 0.0 },
        |e| e.payload(cell, tint.origin[1]),
    );
    let retains_snow = terra_world::weather::retains_snow(face.material);
    if !retains_snow {
        condition.snow = 0.0;
    }
    let snow_sky = retains_snow
        && options
            .exposure
            .as_ref()
            .is_some_and(|e| e.snow_exposed(cell, tint.origin[1]));
    let snow_sky_override = snow_sky && exposure == 0.0;
    // Imprints cut the real surface even when the material is perfectly dry
    // beneath a roof; UV1 selects the shared wet/imprint material, not wetness.
    let imprint_surface = matches!(face.material, 4 | 5);
    let grass = templates
        .blocks
        .get(&1)
        .map(|block| block.face_materials[3]);
    let authored = matches!(face.material, 306 | 400)
        || templates.cube_lods.contains_key(&face.material)
        || templates.blocks.get(&face.material).is_some_and(|block| {
            block
                .face_materials
                .iter()
                .any(|&slot| templates.authored_terrain_material(slot))
        });
    let tree_tint = tint.tree_tint(cell, face.material);
    let placed_tint = if face.material == 103 {
        options
            .tints
            .get(&cell.map(|v| v as usize))
            .copied()
            .map_or([1.0; 3], |rgb| {
                let [r, g, b, _] = crate::dye_tint::rgb(rgb);
                [r, g, b]
            })
    } else {
        [1.0; 3]
    };
    for (index, &(slot, start)) in starts.iter().enumerate() {
        if index == 1 && slot == starts[0].0 {
            continue;
        }
        let is_grass = matches!(face.material, 1 | 306 | 400)
            && face.side != 2
            && (face.material != 1 || templates.cube_lods.contains_key(&1) || Some(slot) == grass);
        let is_ground = is_grass || (face.side == 3 && matches!(face.material, 4 | 5));
        if (!is_ground || tint.ground.is_none())
            && condition == Default::default()
            && tree_tint == [1.0; 3]
            && placed_tint == [1.0; 3]
            && exposure == 0.0
            && !snow_sky_override
            && !imprint_surface
            && !authored
        {
            continue;
        }
        let Some(batch) = batches.get_mut(&slot) else {
            continue;
        };
        if (condition.wetness > 0.003
            || condition.snow > 0.0
            || exposure != 0.0
            || snow_sky_override
            || imprint_surface
            || authored)
            && batch.rooted_wind.is_empty()
        {
            batch.surface_water.resize(batch.positions.len(), [0.0; 2]);
            for water in &mut batch.surface_water[start..] {
                *water = [
                    crate::weather::storage::snow::encode_surface(
                        condition.wetness,
                        condition.snow,
                    ) + if snow_sky_override { 1024.0 } else { 0.0 }
                        + if retains_snow { 0.0 } else { 2048.0 },
                    exposure,
                ];
            }
        } else if !batch.surface_water.is_empty() {
            batch.surface_water.resize(batch.positions.len(), [0.0; 2]);
        }
        batch.colors.resize(batch.positions.len(), [1.0; 4]);
        for (position, color) in batch.positions[start..]
            .iter()
            .zip(&mut batch.colors[start..])
        {
            let biome = if is_ground {
                tint.ground.as_ref().map_or([1.0; 3], |t| {
                    if authored && is_grass {
                        t.authored_grass_at(*position)
                    } else if cave_grass && is_grass {
                        t.cave_grass_at(*position)
                    } else {
                        t.at(*position, face.material)
                    }
                })
            } else {
                [1.0; 3]
            };
            for axis in 0..3 {
                color[axis] *= biome[axis] * factor[axis] * tree_tint[axis] * placed_tint[axis];
            }
            if let Some(climate) = snow_climate {
                color[3] = if snow_sky || options.exposure.is_none() && condition.snow > 0.0 {
                    crate::weather::storage::snow::encode_temperature(
                        climate.temperature(*position),
                    )
                } else {
                    0.0
                };
            }
        }
    }
}

pub(crate) fn append_greedy(
    batches: &mut BTreeMap<usize, MeshBatch>,
    face: BlockFace,
    block: &crate::blocks::BlockTemplate,
) -> Result<(), String> {
    let material = block.face_materials[face.side];
    let tangent = block.faces[face.side]
        .iter()
        .find_map(|p| p.tangents.first())
        .copied();
    if let Some(cap) = block.side_cap.filter(|_| face.side / 2 != 1) {
        let vertical_u = face.u_axis == 1;
        let rows = if vertical_u { face.width } else { face.height };
        for row in 0..rows {
            for (start, height, slot) in [
                (0.0, 1.0 - cap.height, material),
                (1.0 - cap.height, cap.height, cap.material),
            ] {
                let mut minimum = face.minimum.map(|v| v as f32);
                minimum[1] += row as f32 + start;
                append_rectangle(
                    batches,
                    face,
                    slot,
                    minimum,
                    if vertical_u {
                        height
                    } else {
                        face.width as f32
                    },
                    if vertical_u {
                        face.height as f32
                    } else {
                        height
                    },
                    tangent,
                )?;
            }
        }
        return Ok(());
    }
    append_rectangle(
        batches,
        face,
        material,
        face.minimum.map(|v| v as f32),
        face.width as f32,
        face.height as f32,
        tangent,
    )
}

fn append_rectangle(
    batches: &mut BTreeMap<usize, MeshBatch>,
    face: BlockFace,
    material: usize,
    minimum: [f32; 3],
    width: f32,
    height: f32,
    tangent: Option<[f32; 4]>,
) -> Result<(), String> {
    let batch = batches.entry(material).or_insert_with(|| MeshBatch {
        material,
        ..default()
    });
    let mut a = minimum;
    if face.side % 2 == 1 {
        a[face.side / 2] += 1.0;
    }
    let mut b = a;
    b[face.u_axis] += width;
    let mut c = b;
    c[face.v_axis] += height;
    let mut d = a;
    d[face.v_axis] += height;
    let mut normal = [0.0; 3];
    normal[face.side / 2] = if face.side.is_multiple_of(2) {
        -1.0
    } else {
        1.0
    };
    let cross = (Vec3::from_array(b) - Vec3::from_array(a))
        .cross(Vec3::from_array(d) - Vec3::from_array(a));
    let start = u32::try_from(batch.positions.len()).map_err(|_| "Mesh index overflow")?;
    batch.positions.extend([a, b, c, d]);
    if terra_voxel::MaterialId::FOLIAGE.contains(&face.material) {
        batch.foliage_owners.push(FoliageOwner {
            vertices: start as usize..batch.positions.len(),
            cell: face.minimum,
            material: face.material,
        });
    }
    batch.normals.extend([normal; 4]);
    let (u, v) = match face.side / 2 {
        0 => (2, 1),
        1 => (0, 2),
        _ => (0, 1),
    };
    batch.uvs.extend([a, b, c, d].map(|p| [p[u], p[v]]));
    batch.append_tangents(start as usize, tangent.into_iter().flat_map(|t| [t; 4]));
    let indices = if cross.dot(Vec3::from_array(normal)) > 0.0 {
        [0, 1, 2, 0, 2, 3]
    } else {
        [0, 2, 1, 0, 3, 2]
    };
    batch.indices.extend(indices.map(|i| start + i));
    Ok(())
}

fn cell_index(cell: [u32; 3]) -> usize {
    cell[0] as usize + SECTION_SIZE * (cell[1] as usize + SECTION_SIZE * cell[2] as usize)
}

fn cells(face: BlockFace) -> impl Iterator<Item = [u32; 3]> {
    (0..face.width).flat_map(move |u| {
        (0..face.height).map(move |v| {
            let mut cell = face.minimum;
            cell[face.u_axis] += u;
            cell[face.v_axis] += v;
            cell
        })
    })
}

fn validate_face(face: BlockFace) -> Result<(), String> {
    if face.side >= 6
        || face.u_axis >= 3
        || face.v_axis >= 3
        || face.u_axis == face.v_axis
        || face.side / 2 == face.u_axis
        || face.side / 2 == face.v_axis
        || face.width == 0
        || face.width > 16
        || face.height == 0
        || face.height > 16
        || face.minimum.iter().any(|&v| v >= 16)
        || face.minimum[face.u_axis] + face.width > 16
        || face.minimum[face.v_axis] + face.height > 16
    {
        return Err("Invalid exposed face rectangle".into());
    }
    Ok(())
}

pub(super) fn variation(position: [i64; 3], salt: u64) -> u64 {
    let mut h = salt.wrapping_add(0x9e3779b97f4a7c15);
    for coordinate in position {
        h ^= (coordinate as u64).wrapping_mul(0xbf58476d1ce4e5b9);
        h = (h ^ (h >> 30)).wrapping_mul(0x94d049bb133111eb);
        h ^= h >> 27;
    }
    h ^ (h >> 31)
}

fn append(
    batches: &mut BTreeMap<usize, MeshBatch>,
    primitive: &PrimitiveTemplate,
    offset: Vec3,
    rotation: Quat,
    connection: Option<(u8, f32)>,
    cube_side: Option<usize>,
) -> Result<(), String> {
    let batch = batches
        .entry(primitive.material)
        .or_insert_with(|| MeshBatch {
            material: primitive.material,
            ..default()
        });
    if let Some((mask, bevel)) = connection {
        return append_connected(batch, primitive, offset, mask, bevel, cube_side, false);
    }
    let start =
        u32::try_from(batch.positions.len()).map_err(|_| "Section exceeds u32 mesh index limit")?;
    let center = Vec3::new(0.5, 0.0, 0.5);
    batch.positions.extend(
        primitive
            .positions
            .iter()
            .map(|&p| (rotation * (Vec3::from_array(p) - center) + center + offset).to_array()),
    );
    batch.normals.extend(
        primitive
            .normals
            .iter()
            .map(|&n| (rotation * Vec3::from_array(n)).to_array()),
    );
    if let Some(side) = cube_side {
        let (u, v) = match side / 2 {
            0 => (2, 1),
            1 => (0, 2),
            _ => (0, 1),
        };
        batch.uvs.extend(
            batch.positions[start as usize..]
                .iter()
                .map(|p| [p[u], p[v]]),
        );
    } else {
        batch.uvs.extend_from_slice(&primitive.uvs);
    }
    batch.append_tangents(
        start as usize,
        primitive
            .tangents
            .iter()
            .enumerate()
            .map(|(index, tangent)| {
                let normal = rotation * Vec3::from_array(primitive.normals[index]);
                if let Some(side) = cube_side {
                    projected_tangent(normal, side)
                } else {
                    crate::blocks::tangent_frame(
                        rotation * Vec3::from_array([tangent[0], tangent[1], tangent[2]]),
                        normal,
                        tangent[3],
                    )
                }
            }),
    );
    for triangle in primitive.indices.chunks_exact(3) {
        let indices = [triangle[0], triangle[1], triangle[2]].map(|i| start.checked_add(i));
        let [Some(a), Some(b), Some(c)] = indices else {
            return Err("Section mesh index overflow".into());
        };
        batch.indices.extend([a, b, c]);
    }
    Ok(())
}

fn append_connected(
    batch: &mut MeshBatch,
    primitive: &PrimitiveTemplate,
    offset: Vec3,
    mask: u8,
    bevel: f32,
    cube_side: Option<usize>,
    turf_top: bool,
) -> Result<(), String> {
    let has_tangents = !primitive.tangents.is_empty() || !batch.tangents.is_empty();
    // Local six-vertex indexing shares each quad's identical corners without
    // a per-block heap/hash table. Hard normals and UV seams remain distinct.
    for pair in primitive.indices.chunks(6) {
        let mut keys = [[0_u32; 12]; 6];
        let mut count = 0;
        let base =
            u32::try_from(batch.positions.len()).map_err(|_| "Section mesh index overflow")?;
        for triangle in pair.chunks_exact(3) {
            let source = [triangle[0], triangle[1], triangle[2]];
            let points = source.map(|i| {
                let point = primitive.positions[i as usize];
                Vec3::from_array(if turf_top {
                    connected_turf_top(point, mask, bevel)
                } else {
                    connected_position(point, mask, bevel)
                }) + offset
            });
            let normal = (points[1] - points[0]).cross(points[2] - points[0]);
            if normal.length_squared() < 1e-12 {
                continue;
            }
            let normal = normal.normalize();
            let uvs = std::array::from_fn(|corner| {
                let point = points[corner];
                if let Some(side) = cube_side {
                    let (u, v) = match side / 2 {
                        0 => (2, 1),
                        1 => (0, 2),
                        _ => (0, 1),
                    };
                    [point[u], point[v]]
                } else {
                    primitive.uvs[source[corner] as usize]
                }
            });
            let tangent = if has_tangents {
                triangle_tangent(points, uvs, normal)
            } else {
                [0.0; 4]
            };
            let normal = normal.to_array();
            for (point, uv) in points.into_iter().zip(uvs) {
                let p = point.to_array();
                let key = [
                    p[0], p[1], p[2], normal[0], normal[1], normal[2], uv[0], uv[1], tangent[0],
                    tangent[1], tangent[2], tangent[3],
                ]
                .map(f32::to_bits);
                let existing = keys[..count].iter().position(|old| *old == key);
                let local = existing.unwrap_or_else(|| {
                    keys[count] = key;
                    batch.positions.push(p);
                    batch.normals.push(normal);
                    batch.uvs.push(uv);
                    if has_tangents {
                        batch.append_tangents(batch.positions.len() - 1, std::iter::once(tangent));
                    }
                    count += 1;
                    count - 1
                });
                batch.indices.push(
                    base.checked_add(local as u32)
                        .ok_or("Section mesh index overflow")?,
                );
            }
        }
    }
    Ok(())
}

fn triangle_tangent(points: [Vec3; 3], uvs: [[f32; 2]; 3], normal: Vec3) -> [f32; 4] {
    let edge_u = points[1] - points[0];
    let edge_v = points[2] - points[0];
    let uv_u = Vec2::from_array(uvs[1]) - Vec2::from_array(uvs[0]);
    let uv_v = Vec2::from_array(uvs[2]) - Vec2::from_array(uvs[0]);
    let determinant = uv_u.perp_dot(uv_v);
    if determinant.abs() < 1.0e-12 {
        return crate::blocks::tangent_frame(Vec3::X, normal, 1.0);
    }
    let tangent = (edge_u * uv_v.y - edge_v * uv_u.y) / determinant;
    let bitangent = (edge_v * uv_u.x - edge_u * uv_v.x) / determinant;
    crate::blocks::tangent_frame(tangent, normal, normal.cross(tangent).dot(bitangent))
}

#[cfg(test)]
mod indexing_tests {
    use super::*;
    #[test]
    fn connected_quads_share_vertices_without_changing_triangles_or_hard_edges() {
        let mut primitive = PrimitiveTemplate {
            tangents: Vec::new(),
            material: 0,
            positions: vec![
                [0.0, 1.0, 0.0],
                [1.0, 1.0, 0.0],
                [1.0, 1.0, 1.0],
                [0.0, 1.0, 1.0],
            ],
            normals: vec![[0.0, -1.0, 0.0]; 4],
            uvs: vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
            indices: vec![0, 1, 2, 0, 2, 3],
        };
        let mut batch = MeshBatch::default();
        append_connected(&mut batch, &primitive, Vec3::ZERO, 63, 0.0, Some(3), false).unwrap();
        assert_eq!(batch.positions.len(), 4);
        assert_eq!(
            batch
                .indices
                .iter()
                .map(|&i| batch.positions[i as usize])
                .collect::<Vec<_>>(),
            primitive
                .indices
                .iter()
                .map(|&i| primitive.positions[i as usize])
                .collect::<Vec<_>>()
        );
        assert_eq!(batch.normals, vec![[0.0, -1.0, 0.0]; 4]);
        primitive.positions[2][1] += 0.2;
        let mut batch = MeshBatch::default();
        append_connected(&mut batch, &primitive, Vec3::ZERO, 63, 0.0, None, false).unwrap();
        assert_eq!(
            batch.positions.len(),
            6,
            "non-coplanar normals must not be merged"
        );
        assert!(
            batch
                .indices
                .iter()
                .all(|&i| (i as usize) < batch.positions.len())
        );
    }
}

#[cfg(test)]
#[path = "tangent_tests.rs"]
mod tangent_tests;

#[cfg(test)]
#[path = "authored_cache_tests.rs"]
mod authored_cache_tests;
