use super::DetailLevel;
use super::connectivity::connected_position;
use crate::blocks::definitions::{BlockRenderKind, DetailDensity};
use crate::blocks::{PrimitiveTemplate, TemplateSet};
use bevy::prelude::*;
use std::collections::BTreeMap;
use terra_voxel::blocks::{SECTION_SIZE, SECTION_VOLUME};
use terra_voxel::{SectionPos, meshing::BlockFace};

#[path = "condition_faces.rs"]
pub(super) mod condition_faces;

#[derive(Clone)]
pub(crate) struct RenderOptions {
    pub water_level: Option<f64>,
    pub detail_level: DetailLevel,
    pub terrain: Option<terra_world::TerraGenerator>,
    pub exposure: Option<std::sync::Arc<crate::weather::surfaces::exposure::Exposure>>,
    pub conditions: terra_world::environment::SurfaceSnapshot,
}

#[derive(Default, Debug, PartialEq)]
pub(crate) struct MeshBatch {
    pub shadow_only: bool,
    pub shadow_sources: Vec<usize>,
    pub material: usize,
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    /// Optional UV1 weights: rooted terrain wind, or prop growth timing/ratio.
    pub rooted_wind: Vec<[f32; 2]>,
    pub surface_water: Vec<[f32; 2]>,
    pub growth_anchors: Vec<[f32; 3]>,
    /// CPU-only prop ranges for refreshing growth timing after worker completion.
    pub growth_ranges: Vec<(terra_world::props::PropId, std::ops::Range<usize>)>,
    /// CPU-only source cells; micro geometry must not infer provenance from its surface.
    pub foliage_owners: Vec<FoliageOwner>,
    pub colors: Vec<[f32; 4]>,
    pub indices: Vec<u32>,
    pub compact_indices: Vec<u16>,
    pub packed_normals: Vec<[i8; 4]>,
    pub packed_colors: Vec<[u8; 4]>,
}

impl MeshBatch {
    /// Final worker step, after all geometry edits. Smaller section batches do
    /// not need four-byte indices in ready queues or uploaded GPU buffers.
    pub(crate) fn compact_indices(&mut self) {
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

#[cfg(test)]
#[path = "index_packing_tests.rs"]
mod index_packing_tests;

#[derive(Debug, PartialEq)]
pub(crate) struct FoliageOwner {
    pub vertices: std::ops::Range<usize>,
    pub cell: [u32; 3],
    pub material: u16,
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
        .filter(|_| faces.iter().any(|face| matches!(face.material, 1 | 4 | 5)))
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
            .filter(|_| faces.iter().any(|face| matches!(face.material, 6..=10)))
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
        if face.side == 3
            && matches!(face.material, 1 | 4 | 5)
            && options.detail_level != DetailLevel::Near
            && let Some(ground) = &tint.ground
        {
            tree_faces.extend(super::tint_faces::split(face, ground));
        } else if tint.trees.is_some()
            && options.detail_level != DetailLevel::Near
            && matches!(face.material, 6..=10)
        {
            tree_faces.extend(tint.split(face));
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
        let block = base_block;
        if block.render_kind == BlockRenderKind::Cube
            && matches!(face.material, 7..=9)
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
                    let mut starts = BTreeMap::new();
                    let tree_tint = tint.tree_tint(cell, face.material);
                    if condition != Default::default() || tree_tint != [1.0; 3] {
                        for &slot in &crown.face_materials {
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
                        (
                            face.material,
                            tint.tree(cell, face.material)
                                == Some(terra_world::generation::TreeAppearance::Spruce),
                        ),
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
                || matches!(face.material, 7..=9)
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
                let block = base_block;
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
            let block = base_block;
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
            if block.render_kind == BlockRenderKind::Cube {
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
                && matches!(face.material, 6 | 10 | 36)
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
    let grass = templates
        .blocks
        .get(&1)
        .map(|block| block.face_materials[3]);
    let tree_tint = tint.tree_tint(cell, face.material);
    for (index, &(slot, start)) in starts.iter().enumerate() {
        if index == 1 && slot == starts[0].0 {
            continue;
        }
        let is_grass = face.material == 1 && Some(slot) == grass;
        let is_ground = is_grass || (face.side == 3 && matches!(face.material, 4 | 5));
        if (!is_ground || tint.ground.is_none())
            && condition == Default::default()
            && tree_tint == [1.0; 3]
            && exposure == 0.0
            && !snow_sky_override
        {
            continue;
        }
        let Some(batch) = batches.get_mut(&slot) else {
            continue;
        };
        if (condition.wetness > 0.003
            || condition.snow > 0.0
            || exposure != 0.0
            || snow_sky_override)
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
                    if cave_grass && is_grass {
                        t.cave_grass_at(*position)
                    } else {
                        t.at(*position, face.material)
                    }
                })
            } else {
                [1.0; 3]
            };
            for axis in 0..3 {
                color[axis] *= biome[axis] * factor[axis] * tree_tint[axis];
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
    )
}

fn append_rectangle(
    batches: &mut BTreeMap<usize, MeshBatch>,
    face: BlockFace,
    material: usize,
    minimum: [f32; 3],
    width: f32,
    height: f32,
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
    if matches!(face.material, 7..=9) {
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
        return append_connected(batch, primitive, offset, mask, bevel, cube_side);
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
) -> Result<(), String> {
    // Local six-vertex indexing shares each quad's identical corners without
    // a per-block heap/hash table. Hard normals and UV seams remain distinct.
    for pair in primitive.indices.chunks(6) {
        let mut keys = [[0_u32; 8]; 6];
        let mut count = 0;
        let base =
            u32::try_from(batch.positions.len()).map_err(|_| "Section mesh index overflow")?;
        for triangle in pair.chunks_exact(3) {
            let source = [triangle[0], triangle[1], triangle[2]];
            let points = source.map(|i| {
                Vec3::from_array(connected_position(
                    primitive.positions[i as usize],
                    mask,
                    bevel,
                )) + offset
            });
            let normal = (points[1] - points[0]).cross(points[2] - points[0]);
            if normal.length_squared() < 1e-12 {
                continue;
            }
            let normal = normal.normalize().to_array();
            for (point, index) in points.into_iter().zip(source) {
                let uv = if let Some(side) = cube_side {
                    let (u, v) = match side / 2 {
                        0 => (2, 1),
                        1 => (0, 2),
                        _ => (0, 1),
                    };
                    [point[u], point[v]]
                } else {
                    primitive.uvs[index as usize]
                };
                let p = point.to_array();
                let key = [
                    p[0], p[1], p[2], normal[0], normal[1], normal[2], uv[0], uv[1],
                ]
                .map(f32::to_bits);
                let existing = keys[..count].iter().position(|old| *old == key);
                let local = existing.unwrap_or_else(|| {
                    keys[count] = key;
                    batch.positions.push(p);
                    batch.normals.push(normal);
                    batch.uvs.push(uv);
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

#[cfg(test)]
mod indexing_tests {
    use super::*;
    #[test]
    fn connected_quads_share_vertices_without_changing_triangles_or_hard_edges() {
        let mut primitive = PrimitiveTemplate {
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
        append_connected(&mut batch, &primitive, Vec3::ZERO, 63, 0.0, Some(3)).unwrap();
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
        append_connected(&mut batch, &primitive, Vec3::ZERO, 63, 0.0, None).unwrap();
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
