use bevy::{mesh::VertexAttributeValues, prelude::*};

pub(crate) const TERRAIN_VERTICES: usize = 128_000;
pub(crate) const PROP_VERTICES: usize = 96_000;
pub(crate) const CLOUD_VERTICES: usize = 32_000;
pub(crate) const TERRAIN_BYTES: usize = 8 * 1024 * 1024;
pub(crate) const PROP_BYTES: usize = 6 * 1024 * 1024;
pub(crate) const MAX_BATCH_VERTICES: usize = 64_000;
pub(crate) const MAX_BATCH_BYTES: usize = 4 * 1024 * 1024;

#[derive(Clone, Copy)]
pub(crate) enum UploadKind {
    Terrain,
    Props,
    Water,
    Clouds,
}

/// Admitted geometry, not GPU timings or draw calls. Separate quotas prevent a
/// terrain backlog from starving interactive plants or fluid changes.
#[derive(Resource, Default, Debug)]
pub struct MeshUploadStatus {
    pub admitted_vertices: usize,
    pub peak_admitted_vertices: usize,
    pub admitted_bytes: usize,
    pub peak_admitted_bytes: usize,
    pub deferred_batches: u64,
    used: [usize; 4],
    used_bytes: [usize; 4],
    sections: [usize; 4],
}

impl MeshUploadStatus {
    pub(crate) fn reserve_section(&mut self, kind: UploadKind) -> bool {
        let index = kind as usize;
        if self.sections[index] >= [8, 4, 4, 4][index] {
            self.deferred_batches += 1;
            return false;
        }
        self.sections[index] += 1;
        true
    }

    #[cfg(test)]
    fn reserve_batch(&mut self, kind: UploadKind, vertices: usize) -> bool {
        self.reserve_batch_inner(kind, vertices, vertices.saturating_mul(12), false)
    }

    pub(crate) fn reserve_batch_bytes(
        &mut self,
        kind: UploadKind,
        vertices: usize,
        bytes: usize,
    ) -> bool {
        self.reserve_batch_inner(kind, vertices, bytes, true)
    }

    /// The previous detail level remains on screen until the entire replacement
    /// is ready. Spread these refinements over frames without delaying a first
    /// presentation or a player edit. A single legal piece must always progress.
    pub(crate) fn reserve_terrain_batch(
        &mut self,
        vertices: usize,
        bytes: usize,
        refinement: bool,
    ) -> bool {
        let i = UploadKind::Terrain as usize;
        if refinement
            && self.used_bytes[i] != 0
            && (self.used[i].saturating_add(vertices) > MAX_BATCH_VERTICES
                || self.used_bytes[i].saturating_add(bytes) > MAX_BATCH_BYTES / 2)
        {
            self.deferred_batches += 1;
            return false;
        }
        self.reserve_batch_bytes(UploadKind::Terrain, vertices, bytes)
    }

    fn reserve_batch_inner(
        &mut self,
        kind: UploadKind,
        vertices: usize,
        bytes: usize,
        enforce_bytes: bool,
    ) -> bool {
        let index = kind as usize;
        let limit = [TERRAIN_VERTICES, PROP_VERTICES, 16_000, CLOUD_VERTICES][index];
        let byte_limit = [TERRAIN_BYTES, PROP_BYTES, 2 * 1024 * 1024, 2 * 1024 * 1024][index];
        // An indivisible material batch may exceed its quota only while this
        // domain is empty; subsequent batches wait for the next frame.
        if (self.used[index] != 0 && self.used[index].saturating_add(vertices) > limit)
            || (enforce_bytes && self.used_bytes[index].saturating_add(bytes) > byte_limit)
        {
            self.deferred_batches += 1;
            return false;
        }
        self.used[index] = self.used[index].saturating_add(vertices);
        self.admitted_vertices = self.admitted_vertices.saturating_add(vertices);
        self.peak_admitted_vertices = self.peak_admitted_vertices.max(self.admitted_vertices);
        self.record_bytes(index, bytes);
        true
    }

    fn record_bytes(&mut self, index: usize, bytes: usize) {
        self.used_bytes[index] = self.used_bytes[index].saturating_add(bytes);
        self.admitted_bytes = self.admitted_bytes.saturating_add(bytes);
        self.peak_admitted_bytes = self.peak_admitted_bytes.max(self.admitted_bytes);
    }

    pub(crate) fn reserve(&mut self, kind: UploadKind, vertices: usize) -> bool {
        let index = kind as usize;
        let limit = [TERRAIN_VERTICES, PROP_VERTICES, 16_000, CLOUD_VERTICES][index];
        let section_limit = [8, 4, 4, 4][index];
        // Reserve once for the complete section, not once per material. One
        // oversized section may progress alone without fragmenting GPU batches.
        if self.sections[index] >= section_limit
            || (self.sections[index] != 0 && self.used[index].saturating_add(vertices) > limit)
        {
            self.deferred_batches += 1;
            return false;
        }
        self.used[index] = self.used[index].saturating_add(vertices);
        self.sections[index] += 1;
        self.admitted_vertices = self.admitted_vertices.saturating_add(vertices);
        self.peak_admitted_vertices = self.peak_admitted_vertices.max(self.admitted_vertices);
        self.record_bytes(index, vertices.saturating_mul(12));
        true
    }

    pub(crate) fn reset(mut status: ResMut<Self>) {
        status.used = [0; 4];
        status.used_bytes = [0; 4];
        status.sections = [0; 4];
        status.admitted_vertices = 0;
        status.admitted_bytes = 0;
    }
}

/// Queued vertex/index bytes; optional overlapping attributes are counted
/// conservatively even when a specialized upload selects only one of them.
pub(crate) fn batch_bytes(batch: &crate::meshing::builder::MeshBatch) -> usize {
    [
        (batch.positions.len(), 12),
        (batch.normals.len(), 12),
        (batch.tangents.len(), 16),
        (batch.uvs.len(), 8),
        (batch.packed_uvs.len(), 4),
        (batch.rooted_wind.len(), 8),
        (batch.surface_water.len(), 8),
        (batch.bile_motion.len(), 16),
        (batch.growth_anchors.len(), 12),
        (batch.colors.len(), 16),
        (batch.packed_normals.len(), 4),
        (batch.packed_tangents.len(), 4),
        (batch.packed_colors.len(), 4),
        (batch.indices.len(), 4),
        (batch.compact_indices.len(), 2),
    ]
    .into_iter()
    .fold(0_usize, |total, (count, stride)| {
        total.saturating_add(count.saturating_mul(stride))
    })
    .saturating_add(
        batch
            .half_colors
            .as_ref()
            .map_or(0, |colors| colors.get_bytes().len()),
    )
}

fn vertex_stride(batch: &crate::meshing::builder::MeshBatch) -> usize {
    [
        (!batch.positions.is_empty(), 12),
        (!batch.normals.is_empty(), 12),
        (!batch.tangents.is_empty(), 16),
        (!batch.uvs.is_empty(), 8),
        (!batch.packed_uvs.is_empty(), 4),
        (!batch.rooted_wind.is_empty(), 8),
        (!batch.surface_water.is_empty(), 8),
        (!batch.bile_motion.is_empty(), 16),
        (!batch.growth_anchors.is_empty(), 12),
        (!batch.colors.is_empty(), 16),
        (batch.half_colors.is_some(), 8),
        (!batch.packed_normals.is_empty(), 4),
        (!batch.packed_tangents.is_empty(), 4),
        (!batch.packed_colors.is_empty(), 4),
    ]
    .into_iter()
    .filter_map(|(present, stride)| present.then_some(stride))
    .sum()
}

/// Gather each vertex attribute in a single allocation after fixing the part's
/// indices. Relief templates usually reference a contiguous run in index order.
fn gather_vertices<T: Copy + Default>(source: &[T], originals: &[usize], ordered: bool) -> Vec<T> {
    if source.is_empty() {
        return Vec::new();
    }
    if ordered {
        let start = originals[0];
        let end = start + originals.len();
        if let Some(vertices) = source.get(start..end) {
            return vertices.to_vec();
        }
    }
    // Wind and water payloads can be shorter than positions; staging fills
    // their missing tail with zeroes in exactly the same way.
    originals
        .iter()
        .map(|&i| source.get(i).copied().unwrap_or_default())
        .collect()
}

fn finish_part(
    mut part: crate::meshing::builder::MeshBatch,
    source: &crate::meshing::builder::MeshBatch,
    originals: &[usize],
) -> crate::meshing::builder::MeshBatch {
    let ordered = originals.windows(2).all(|pair| pair[1] == pair[0] + 1);
    part.material = source.material;
    part.tiled_uv_range = source.tiled_uv_range;
    part.shadow_only = source.shadow_only;
    part.paired_leaf_shadow = source.paired_leaf_shadow;
    part.profile_leaf_shadow = source.profile_leaf_shadow;
    part.shadow_sources.clone_from(&source.shadow_sources);
    part.flowing_lava = source.flowing_lava;
    macro_rules! gather {
        ($($field:ident),+) => {$(
            part.$field = gather_vertices(&source.$field, originals, ordered);
        )+};
    }
    gather!(
        positions,
        normals,
        tangents,
        uvs,
        packed_uvs,
        growth_anchors,
        colors,
        packed_normals,
        packed_tangents,
        packed_colors,
        rooted_wind,
        surface_water,
        bile_motion
    );
    if let Some(VertexAttributeValues::Float16x4(colors)) = &source.half_colors {
        part.half_colors = Some(VertexAttributeValues::Float16x4(gather_vertices(
            colors, originals, ordered,
        )));
    }
    annotate_part(&mut part, source, originals);
    part
}

fn remap_ranges(
    mapping: &[(usize, usize)],
    original: &std::ops::Range<usize>,
) -> Vec<std::ops::Range<usize>> {
    let first = mapping.partition_point(|&(old, _)| old < original.start);
    let mut vertices: Vec<_> = mapping[first..]
        .iter()
        .take_while(|&&(old, _)| old < original.end)
        .map(|&(_, new)| new)
        .collect();
    vertices.sort_unstable();
    let mut result: Vec<std::ops::Range<usize>> = Vec::new();
    for vertex in vertices {
        if let Some(previous) = result.last_mut().filter(|range| range.end == vertex) {
            previous.end += 1;
        } else {
            result.push(vertex..vertex + 1);
        }
    }
    result
}

fn annotate_part(
    part: &mut crate::meshing::builder::MeshBatch,
    source: &crate::meshing::builder::MeshBatch,
    originals: &[usize],
) {
    if source.growth_ranges.is_empty() && source.foliage_owners.is_empty() {
        return;
    }
    let mut mapping: Vec<_> = originals
        .iter()
        .enumerate()
        .map(|(new, &old)| (old, new))
        .collect();
    mapping.sort_unstable_by_key(|&(old, _)| old);
    for (id, range) in &source.growth_ranges {
        part.growth_ranges.extend(
            remap_ranges(&mapping, range)
                .into_iter()
                .map(|range| (*id, range)),
        );
    }
    for owner in &source.foliage_owners {
        part.foliage_owners
            .extend(
                remap_ranges(&mapping, &owner.vertices)
                    .into_iter()
                    .map(|vertices| crate::meshing::builder::FoliageOwner {
                        vertices,
                        cell: owner.cell,
                        material: owner.material,
                    }),
            );
    }
}

fn validate_half_colors(batch: &crate::meshing::builder::MeshBatch) -> Result<(), String> {
    let Some(colors) = &batch.half_colors else {
        return Ok(());
    };
    let VertexAttributeValues::Float16x4(colors) = colors else {
        return Err("Cannot split mesh with half colors outside Float16x4 format".into());
    };
    if colors.len() != batch.positions.len() {
        return Err(format!(
            "Cannot split mesh with half colors count {} for {} vertices",
            colors.len(),
            batch.positions.len()
        ));
    }
    if !batch.colors.is_empty() || !batch.packed_colors.is_empty() {
        return Err("Cannot split mesh with half colors and another active color buffer".into());
    }
    Ok(())
}

fn validate_split_source(batch: &crate::meshing::builder::MeshBatch) -> Result<(), String> {
    let count = batch.positions.len();
    for (name, length) in [
        ("normals", batch.normals.len()),
        ("tangents", batch.tangents.len()),
        ("uvs", batch.uvs.len()),
        ("packed UVs", batch.packed_uvs.len()),
        ("bile motion", batch.bile_motion.len()),
        ("growth anchors", batch.growth_anchors.len()),
        ("colors", batch.colors.len()),
        ("packed normals", batch.packed_normals.len()),
        ("packed tangents", batch.packed_tangents.len()),
        ("packed colors", batch.packed_colors.len()),
    ] {
        if length != 0 && length != count {
            return Err(format!(
                "Cannot split mesh with {name} count {length} for {count} vertices"
            ));
        }
    }
    if batch.rooted_wind.len() > count
        || batch.surface_water.len() > count
        || batch
            .growth_ranges
            .iter()
            .any(|(_, range)| range.start > range.end || range.end > count)
        || batch
            .foliage_owners
            .iter()
            .any(|owner| owner.vertices.start > owner.vertices.end || owner.vertices.end > count)
    {
        return Err("Cannot split mesh with out-of-range vertex metadata".into());
    }
    if !batch.indices.is_empty() && !batch.compact_indices.is_empty() {
        return Err("Cannot split mesh with two active index buffers".into());
    }
    let indices = if batch.compact_indices.is_empty() {
        batch.indices.len()
    } else {
        batch.compact_indices.len()
    };
    if indices % 3 != 0 {
        return Err("Cannot split an incomplete triangle list".into());
    }
    Ok(())
}

/// Keep folded-leaf pieces local to the cells that generated them. Large
/// canopies otherwise produce budget-sized meshes that span most of a section;
/// neither the camera nor the sun can cull their hidden parts independently.
fn crown_ranges(
    batch: &crate::meshing::builder::MeshBatch,
    index_count: usize,
    index: impl Fn(usize) -> usize,
) -> Vec<Vec<std::ops::Range<usize>>> {
    // Count modeled faces, independent of their current triangle density.
    // A sparse octant stays one mesh. Dense octants split on fixed 4 m planes.
    let group = |[x, y, z]: [u32; 3], shift: u32| {
        (((x >> shift) & 1) | (((y >> shift) & 1) << 1) | (((z >> shift) & 1) << 2)) as usize
    };
    let mut faces = [0; 8];
    for owner in &batch.foliage_owners {
        if owner.vertices.len() >= 256 {
            faces[group(owner.cell, 3)] += 1;
        }
    }
    let mut groups = vec![Vec::<std::ops::Range<usize>>::new(); 65];
    let mut vertex_groups = vec![64_u8; batch.positions.len()];
    for owner in &batch.foliage_owners {
        let octant = group(owner.cell, 3);
        let tile = if faces[octant] >= 48 {
            group(owner.cell, 2)
        } else {
            0
        };
        if let Some(vertices) = vertex_groups.get_mut(owner.vertices.clone()) {
            vertices.fill((octant * 8 + tile) as u8);
        }
    }
    for start in (0..index_count).step_by(6) {
        let key = vertex_groups.get(index(start)).copied().unwrap_or(64);
        let ranges = &mut groups[usize::from(key)];
        let end = (start + 6).min(index_count);
        if let Some(previous) = ranges.last_mut().filter(|r| r.end == start) {
            previous.end = end;
        } else {
            ranges.push(start..end);
        }
    }
    groups.retain(|ranges| !ranges.is_empty());
    groups
}

/// Worker-only: bound each later main-thread handoff without changing geometry.
/// Keep adjacent triangle pairs together, remapping shared boundary vertices
/// and CPU provenance. Call before staging GPU handles and compacting indices.
pub(crate) fn split_batches(
    batches: Vec<crate::meshing::builder::MeshBatch>,
) -> Result<Vec<crate::meshing::builder::MeshBatch>, String> {
    split_worker_batches(batches, None)
}

/// Seasonal and wind tags have already been baked into the vertex streams.
pub(crate) fn split_terrain_batches(
    batches: Vec<crate::meshing::builder::MeshBatch>,
    local_crowns: bool,
) -> Result<Vec<crate::meshing::builder::MeshBatch>, String> {
    split_worker_batches(batches, Some(local_crowns))
}

fn split_worker_batches(
    batches: Vec<crate::meshing::builder::MeshBatch>,
    terrain: Option<bool>,
) -> Result<Vec<crate::meshing::builder::MeshBatch>, String> {
    let mut result = Vec::with_capacity(batches.len());
    for mut source in batches {
        validate_half_colors(&source)?;
        let local_crown = terrain == Some(true)
            && source.positions.len() >= 24_000
            && source
                .foliage_owners
                .iter()
                .any(|owner| owner.vertices.len() >= 256);
        if terrain.is_some() && !local_crown {
            source.foliage_owners = Vec::new();
        }
        if (!local_crown
            && source.positions.len() <= MAX_BATCH_VERTICES
            && batch_bytes(&source) <= MAX_BATCH_BYTES)
            || !source.has_indices()
        {
            result.push(source);
            continue;
        }
        if source.staged.is_some() {
            return Err("Mesh splitting must run on the worker before GPU staging".into());
        }
        // Existing composition can append procedural vertices after tangents.
        // This established finalizer supplies matching frames before remapping.
        source.compact_indices();
        validate_split_source(&source)?;
        let compact = !source.compact_indices.is_empty();
        let index_count = if compact {
            source.compact_indices.len()
        } else {
            source.indices.len()
        };
        let index = |i: usize| {
            if compact {
                usize::from(source.compact_indices[i])
            } else {
                source.indices[i] as usize
            }
        };
        let stride = vertex_stride(&source);
        let index_stride = if compact { 2 } else { 4 };
        let mut remap = vec![u32::MAX; source.positions.len()];
        let mut originals = Vec::with_capacity(MAX_BATCH_VERTICES);
        let mut part = crate::meshing::builder::MeshBatch::default();
        let groups = if local_crown {
            crown_ranges(&source, index_count, index)
        } else {
            vec![vec![0..index_count]]
        };
        if terrain.is_some() {
            source.foliage_owners = Vec::new();
        }
        for group in groups {
            for start in group.into_iter().flat_map(|run| run.step_by(6)) {
                let end = (start + 6).min(index_count);
                let indices = part.indices.len() + part.compact_indices.len() + end - start;
                let over_limit = |vertices| {
                    vertices > MAX_BATCH_VERTICES
                        || vertices * stride + indices * index_stride > MAX_BATCH_BYTES
                };
                // A triangle pair can introduce at most six vertices. Inspect shared
                // indices twice only at the part boundary, where their reuse matters.
                if part.has_indices() && over_limit(originals.len() + end - start) {
                    let mut fresh = [usize::MAX; 6];
                    let mut added = 0;
                    for i in start..end {
                        let old = index(i);
                        if remap.get(old) == Some(&u32::MAX) && !fresh[..added].contains(&old) {
                            fresh[added] = old;
                            added += 1;
                        }
                    }
                    if over_limit(originals.len() + added) {
                        result.push(finish_part(std::mem::take(&mut part), &source, &originals));
                        for old in originals.drain(..) {
                            remap[old] = u32::MAX;
                        }
                    }
                }
                for i in start..end {
                    let old = index(i);
                    let slot = remap
                        .get_mut(old)
                        .ok_or("Cannot split mesh with an out-of-range triangle index")?;
                    if *slot == u32::MAX {
                        *slot = originals.len() as u32;
                        originals.push(old);
                    }
                    let local = *slot;
                    if compact {
                        part.compact_indices.push(local as u16);
                    } else {
                        part.indices.push(local);
                    }
                }
            }
            if part.has_indices() {
                result.push(finish_part(std::mem::take(&mut part), &source, &originals));
                for old in originals.drain(..) {
                    remap[old] = u32::MAX;
                }
            }
        }
    }
    Ok(result)
}

pub(crate) fn release_staged(
    batches: &mut [crate::meshing::builder::MeshBatch],
    meshes: &mut Assets<Mesh>,
) {
    for batch in batches {
        if let Some(staged) = batch.staged.take()
            && batch.borrowed_from.take().is_none()
            && staged.liquid.is_none()
        {
            meshes.remove(staged.handle.id());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn refinements_make_progress_and_leave_upload_room_for_new_geometry() {
        let mut budget = MeshUploadStatus::default();
        assert!(budget.reserve_terrain_batch(MAX_BATCH_VERTICES, MAX_BATCH_BYTES, true));
        assert!(!budget.reserve_terrain_batch(4, 48, true));
        assert!(budget.reserve_terrain_batch(4, 48, false));
        assert!(budget.reserve_batch_bytes(UploadKind::Props, 4, 48));
        let mut next = MeshUploadStatus::default();
        assert!(next.reserve_terrain_batch(4, 48, true));
        assert_eq!(budget.deferred_batches, 1);
    }
    #[test]
    fn quotas_defer_without_starving_other_domains_or_oversized_sections() {
        let mut status = MeshUploadStatus::default();
        assert!(status.reserve(UploadKind::Terrain, TERRAIN_VERTICES - 10_000));
        assert!(!status.reserve(UploadKind::Terrain, 20_000));
        assert!(status.reserve(UploadKind::Props, 40_000));
        assert!(status.reserve(UploadKind::Water, 10_000));
        assert_eq!(status.admitted_vertices, TERRAIN_VERTICES + 40_000);
        assert_eq!(status.deferred_batches, 1);
        let mut app = App::new();
        app.insert_resource(status)
            .add_systems(First, MeshUploadStatus::reset);
        app.update();
        let mut status = app.world_mut().resource_mut::<MeshUploadStatus>();
        assert_eq!(status.admitted_vertices, 0);
        assert!(status.reserve(UploadKind::Terrain, 500_000));
        assert!(!status.reserve(UploadKind::Terrain, 1));
    }

    #[test]
    fn tiny_and_empty_batches_cannot_create_an_unbounded_handoff_frame() {
        let mut status = MeshUploadStatus::default();
        for _ in 0..8 {
            assert!(status.reserve(UploadKind::Terrain, 0));
        }
        assert!(!status.reserve(UploadKind::Terrain, 0));
        assert!(status.reserve(UploadKind::Props, 200_000));
        assert!(!status.reserve(UploadKind::Props, 0));
        assert!(status.reserve(UploadKind::Water, 1));
    }

    #[test]
    fn cloud_banks_share_the_existing_budget_without_starving_terrain_or_exceeding_four_uploads() {
        let mut status = MeshUploadStatus::default();
        assert!(status.reserve(UploadKind::Terrain, TERRAIN_VERTICES));
        assert!(status.reserve(UploadKind::Clouds, CLOUD_VERTICES - 1));
        assert!(!status.reserve(UploadKind::Clouds, 2));
        assert!(status.reserve(UploadKind::Clouds, 1));
        assert!(status.reserve(UploadKind::Props, 1));
        let mut app = App::new();
        app.insert_resource(status)
            .add_systems(First, MeshUploadStatus::reset);
        app.update();
        let mut status = app.world_mut().resource_mut::<MeshUploadStatus>();
        for _ in 0..4 {
            assert!(status.reserve(UploadKind::Clouds, 1));
        }
        assert!(!status.reserve(UploadKind::Clouds, 1));
        assert!(status.reserve(UploadKind::Terrain, 1));
    }

    #[test]
    fn material_batches_share_vertex_quota_without_exhausting_section_slots() {
        let mut status = MeshUploadStatus::default();
        for _ in 0..8 {
            assert!(status.reserve_section(UploadKind::Terrain));
            for _ in 0..12 {
                assert!(status.reserve_batch(UploadKind::Terrain, 3));
            }
        }
        assert_eq!(status.admitted_vertices, 8 * 12 * 3);
        assert!(!status.reserve_section(UploadKind::Terrain));
        assert!(status.reserve_section(UploadKind::Props));
        assert!(status.reserve_batch(UploadKind::Props, PROP_VERTICES));
        assert!(!status.reserve_batch(UploadKind::Props, 1));
        assert_eq!(status.admitted_vertices, 8 * 12 * 3 + PROP_VERTICES);
    }

    #[test]
    fn oversized_material_batches_progress_alone_but_not_as_one_oversized_section() {
        let mut app = App::new();
        app.init_resource::<MeshUploadStatus>()
            .add_systems(First, MeshUploadStatus::reset);
        for _ in 0..3 {
            app.update();
            let mut status = app.world_mut().resource_mut::<MeshUploadStatus>();
            assert!(status.reserve_section(UploadKind::Terrain));
            assert!(status.reserve_batch(UploadKind::Terrain, 500_000));
            assert!(!status.reserve_batch(UploadKind::Terrain, 3));
            assert_eq!(status.admitted_vertices, 500_000);
            assert!(status.reserve_section(UploadKind::Props));
            assert!(status.reserve_batch(UploadKind::Props, 3));
        }
    }

    #[test]
    fn real_byte_quotas_defer_even_the_first_oversized_upload() {
        let mut status = MeshUploadStatus::default();
        assert!(!status.reserve_batch_bytes(UploadKind::Terrain, 3, TERRAIN_BYTES + 1));
        assert_eq!(status.admitted_vertices, 0);
        assert_eq!(status.admitted_bytes, 0);
        assert!(status.reserve_batch_bytes(UploadKind::Terrain, 40_000, MAX_BATCH_BYTES));
        assert!(status.reserve_batch_bytes(UploadKind::Terrain, 40_000, MAX_BATCH_BYTES));
        assert!(!status.reserve_batch_bytes(UploadKind::Terrain, 3, 1));
        assert!(status.reserve_batch_bytes(UploadKind::Props, 30_000, MAX_BATCH_BYTES));
        assert!(status.reserve_batch_bytes(
            UploadKind::Props,
            20_000,
            PROP_BYTES - MAX_BATCH_BYTES
        ));
        assert!(!status.reserve_batch_bytes(UploadKind::Props, 3, 1));
        assert_eq!(status.admitted_bytes, TERRAIN_BYTES + PROP_BYTES);
        let mut app = App::new();
        app.insert_resource(status)
            .add_systems(First, MeshUploadStatus::reset);
        app.update();
        let status = app.world().resource::<MeshUploadStatus>();
        assert_eq!(status.admitted_bytes, 0);
        assert_eq!(status.peak_admitted_bytes, TERRAIN_BYTES + PROP_BYTES);
    }

    fn rich_batch(count: usize) -> crate::meshing::builder::MeshBatch {
        use crate::meshing::builder::{FoliageOwner, MeshBatch};
        MeshBatch {
            shadow_only: true,
            shadow_sources: vec![3, 7, 11],
            material: 19,
            positions: (0..count).map(|i| [i as f32, 1.0, 2.0]).collect(),
            normals: vec![[0.0, 1.0, 0.0]; count],
            tangents: vec![[1.0, 0.0, 0.0, 1.0]; count],
            uvs: (0..count).map(|i| [i as f32 * 0.25, 0.5]).collect(),
            rooted_wind: vec![[3.0, -1.5]; count],
            surface_water: vec![[0.4, 0.8]; count],
            growth_anchors: vec![[2.0, 3.0, 4.0]; count],
            colors: vec![[0.5, 0.6, 0.7, 0.8]; count],
            packed_normals: vec![[0, 127, 0, 0]; count],
            packed_tangents: vec![[127, 0, 0, -127]; count],
            packed_colors: vec![[1, 2, 3, 4]; count],
            indices: (0..count as u32).collect(),
            growth_ranges: vec![
                (terra_world::props::PropId(10), 0..count / 2),
                (terra_world::props::PropId(20), count / 2..count),
            ],
            foliage_owners: vec![
                FoliageOwner {
                    vertices: 0..count / 2,
                    cell: [1, 2, 3],
                    material: 7,
                },
                FoliageOwner {
                    vertices: count / 2..count,
                    cell: [4, 5, 6],
                    material: 9,
                },
            ],
            ..Default::default()
        }
    }

    fn reconstructed_indices(batches: &[crate::meshing::builder::MeshBatch]) -> Vec<usize> {
        batches
            .iter()
            .flat_map(|batch| {
                if batch.compact_indices.is_empty() {
                    batch
                        .indices
                        .iter()
                        .map(|&i| batch.positions[i as usize][0] as usize)
                        .collect::<Vec<_>>()
                } else {
                    batch
                        .compact_indices
                        .iter()
                        .map(|&i| batch.positions[usize::from(i)][0] as usize)
                        .collect()
                }
            })
            .collect()
    }

    fn half_colors(count: usize) -> VertexAttributeValues {
        VertexAttributeValues::Float16x4(
            (0..count)
                .map(|i| {
                    [
                        (i % 251) as u8,
                        ((i / 251) % 241) as u8,
                        (i / (251 * 241)) as u8,
                        1,
                    ]
                    .map(Into::into)
                })
                .collect(),
        )
    }

    fn half_color_batch(count: usize) -> crate::meshing::builder::MeshBatch {
        crate::meshing::builder::MeshBatch {
            positions: (0..count).map(|i| [i as f32, 0.0, 0.0]).collect(),
            indices: (0..count as u32).collect(),
            half_colors: Some(half_colors(count)),
            ..Default::default()
        }
    }

    #[test]
    fn spatial_crowns_keep_exact_triangles_and_isolate_distant_cell_changes() {
        use crate::meshing::builder::{FoliageOwner, MeshBatch};
        let crown = |edit: bool| {
            let mut batch = MeshBatch::default();
            // Interleave the two octants as the face mesher does.
            for i in 0..90 {
                let x = if i % 2 == 0 { 0 } else { 12 };
                let start = batch.positions.len();
                let count = if edit && i == 0 { 288 } else { 282 };
                for j in 0..count {
                    batch
                        .positions
                        .push([x as f32, i as f32 / 90., j as f32 / 282.]);
                    batch.colors.push([0.1, 0.3, 0.2, (i % 8) as f32 / 8.]);
                    batch.indices.push((start + j) as u32);
                }
                batch.foliage_owners.push(FoliageOwner {
                    vertices: start..start + count,
                    cell: [x, 0, 0],
                    material: 7,
                });
            }
            batch
        };
        let mut original = crown(false);
        let expected = |x: f32| {
            original
                .positions
                .iter()
                .zip(&original.colors)
                .filter(|(p, _)| p[0] == x)
                .map(|(p, c)| (*p, *c))
                .collect::<Vec<_>>()
        };
        let streams = [expected(0.), expected(12.)];
        original.compact_indices();
        let parts = split_terrain_batches(vec![original], true).unwrap();
        let edited = split_terrain_batches(vec![crown(true)], true).unwrap();
        assert_eq!(parts.len(), 2);
        for (part, expected) in parts.iter().zip(streams) {
            let actual = part
                .compact_indices
                .iter()
                .map(|&i| (part.positions[i as usize], part.colors[i as usize]))
                .collect::<Vec<_>>();
            assert_eq!(actual, expected);
            assert!(part.foliage_owners.is_empty());
        }
        assert_eq!(parts[1], edited[1]);
        assert_ne!(parts[0], edited[0]);
        assert_eq!(
            split_terrain_batches(vec![crown(false)], false)
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn half_color_byte_accounting_uses_the_actual_attribute_storage() {
        let mut batch = half_color_batch(6);
        assert_eq!(vertex_stride(&batch), 12 + 8);
        assert_eq!(batch_bytes(&batch), 6 * (12 + 8 + 4));
        batch.compact_indices();
        assert_eq!(batch_bytes(&batch), 6 * (12 + 8 + 2));
        batch.half_colors = Some(VertexAttributeValues::Float32x4(vec![[0.0; 4]; 6]));
        assert_eq!(batch_bytes(&batch), 6 * (12 + 16 + 2));
    }

    #[test]
    fn worker_resplit_preserves_half_colors_and_scrambled_triangle_indices() {
        let count = 70_002;
        let mut batch = rich_batch(count);
        batch.colors.clear();
        batch.packed_colors.clear();
        let colors = half_colors(count);
        batch.half_colors = Some(colors.clone());
        batch.indices = (0..count / 2)
            .flat_map(|i| [i as u32, (i + count / 2) as u32])
            .collect();
        let expected = batch
            .indices
            .iter()
            .map(|&index| index as usize)
            .collect::<Vec<_>>();
        let VertexAttributeValues::Float16x4(original_colors) = colors else {
            unreachable!();
        };
        let parts = split_batches(vec![batch]).unwrap();
        assert!(parts.len() > 1);
        assert_eq!(reconstructed_indices(&parts), expected);
        for part in &parts {
            assert!(part.positions.len() <= MAX_BATCH_VERTICES);
            assert!(batch_bytes(part) <= MAX_BATCH_BYTES);
            assert!(part.colors.is_empty() && part.packed_colors.is_empty());
            let Some(VertexAttributeValues::Float16x4(colors)) = &part.half_colors else {
                panic!("split lost half color storage");
            };
            assert_eq!(colors.len(), part.positions.len());
            for (position, color) in part.positions.iter().zip(colors) {
                assert_eq!(*color, original_colors[position[0] as usize]);
            }
        }
        let parts = split_batches(parts).unwrap();
        assert_eq!(reconstructed_indices(&parts), expected);
    }

    #[test]
    fn malformed_or_duplicate_half_colors_are_rejected_before_the_fast_path() {
        let mut invalid = half_color_batch(6);
        invalid.half_colors = Some(VertexAttributeValues::Float32x4(vec![[0.0; 4]; 6]));
        assert!(
            split_batches(vec![invalid])
                .unwrap_err()
                .contains("Float16x4")
        );
        let mut invalid = half_color_batch(6);
        invalid.half_colors = Some(half_colors(5));
        assert!(
            split_batches(vec![invalid])
                .unwrap_err()
                .contains("half colors count")
        );
        let mut invalid = half_color_batch(6);
        invalid.colors = vec![[1.0; 4]; 6];
        assert!(
            split_batches(vec![invalid])
                .unwrap_err()
                .contains("another active color buffer")
        );
        let mut invalid = half_color_batch(6);
        invalid.packed_colors = vec![[255; 4]; 6];
        assert!(
            split_batches(vec![invalid])
                .unwrap_err()
                .contains("another active color buffer")
        );
        let mut invalid = half_color_batch(70_002);
        invalid.indices.clear();
        invalid.half_colors = Some(half_colors(1));
        assert!(
            split_batches(vec![invalid])
                .unwrap_err()
                .contains("half colors count")
        );
    }

    #[test]
    fn worker_split_preserves_all_attributes_indices_and_shadow_metadata() {
        let count = 140_004;
        let mut batch = rich_batch(count);
        batch.flowing_lava = true;
        batch.bile_motion = (0..count).map(|i| [i as f32, 0.5, 0.25, 1.0]).collect();
        let parts = split_batches(vec![batch]).unwrap();
        assert!(parts.len() >= 3);
        assert_eq!(
            reconstructed_indices(&parts),
            (0..count).collect::<Vec<_>>()
        );
        for part in &parts {
            assert!(part.positions.len() <= MAX_BATCH_VERTICES);
            assert!(batch_bytes(part) <= MAX_BATCH_BYTES);
            assert_eq!(part.material, 19);
            assert!(part.shadow_only);
            assert_eq!(part.shadow_sources, [3, 7, 11]);
            assert!(part.flowing_lava);
            for i in 0..part.positions.len() {
                let old = part.positions[i][0] as usize;
                assert_eq!(part.positions[i], [old as f32, 1.0, 2.0]);
                assert_eq!(part.bile_motion[i], [old as f32, 0.5, 0.25, 1.0]);
                assert_eq!(part.normals[i], [0.0, 1.0, 0.0]);
                assert_eq!(part.tangents[i], [1.0, 0.0, 0.0, 1.0]);
                assert_eq!(part.uvs[i], [old as f32 * 0.25, 0.5]);
                assert_eq!(part.rooted_wind[i], [3.0, -1.5]);
                assert_eq!(part.surface_water[i], [0.4, 0.8]);
                assert_eq!(part.growth_anchors[i], [2.0, 3.0, 4.0]);
                assert_eq!(part.colors[i], [0.5, 0.6, 0.7, 0.8]);
                assert_eq!(part.packed_normals[i], [0, 127, 0, 0]);
                assert_eq!(part.packed_tangents[i], [127, 0, 0, -127]);
                assert_eq!(part.packed_colors[i], [1, 2, 3, 4]);
            }
        }
    }

    #[test]
    fn scrambled_shared_vertices_keep_growth_and_foliage_ranges() {
        let count = 70_002;
        let mut batch = rich_batch(count);
        batch.indices.clear();
        for i in 0..count / 2 {
            batch.indices.extend([i as u32, (i + count / 2) as u32]);
        }
        let expected = batch
            .indices
            .iter()
            .map(|&i| i as usize)
            .collect::<Vec<_>>();
        let parts = split_batches(vec![batch]).unwrap();
        assert_eq!(reconstructed_indices(&parts), expected);
        for part in &parts {
            let mut growth = vec![0; part.positions.len()];
            let mut foliage = vec![[0; 3]; part.positions.len()];
            for (id, range) in &part.growth_ranges {
                growth[range.clone()].fill(id.0);
            }
            for owner in &part.foliage_owners {
                foliage[owner.vertices.clone()].fill(owner.cell);
                assert_eq!(owner.material, if owner.cell == [1, 2, 3] { 7 } else { 9 });
            }
            for (i, position) in part.positions.iter().enumerate() {
                let first = (position[0] as usize) < count / 2;
                assert_eq!(growth[i], if first { 10 } else { 20 });
                assert_eq!(foliage[i], if first { [1, 2, 3] } else { [4, 5, 6] });
            }
        }
    }

    #[test]
    fn compact_sources_and_index_heavy_buffers_stay_bounded_without_losing_triangles() {
        let count = 64_500;
        let mut batch = rich_batch(count);
        batch.compact_indices();
        let mut parts = split_batches(vec![batch]).unwrap();
        assert!(parts.len() > 1);
        assert_eq!(
            reconstructed_indices(&parts),
            (0..count).collect::<Vec<_>>()
        );
        for part in &mut parts {
            assert!(!part.compact_indices.is_empty());
            assert!(part.indices.is_empty());
            assert!(batch_bytes(part) <= MAX_BATCH_BYTES);
        }
        let mut heavy = rich_batch(6);
        heavy.indices = [0, 1, 2, 0, 2, 3].repeat(400_000);
        let expected_triangles = heavy.indices.len() / 3;
        let parts = split_batches(vec![heavy]).unwrap();
        assert!(parts.len() > 1);
        assert_eq!(
            parts
                .iter()
                .map(|part| (part.indices.len() + part.compact_indices.len()) / 3)
                .sum::<usize>(),
            expected_triangles
        );
        assert!(
            parts
                .iter()
                .all(|part| batch_bytes(part) <= MAX_BATCH_BYTES)
        );
    }

    #[test]
    fn small_staged_batches_are_untouched_and_malformed_sources_are_rejected() {
        let mut staged = rich_batch(6);
        staged.staged = Some(crate::meshing::staging::StagedMesh {
            handle: Handle::default(),
            liquid: None,
            bounds: None,
            rooted: true,
            damp: false,
            opaque_shadow: true,
            coastal_quads: 3,
            far: None,
        });
        let mut parts = split_batches(vec![staged]).unwrap();
        let staged = parts.pop().unwrap().staged.unwrap();
        assert!(staged.rooted && staged.opaque_shadow);
        assert_eq!(staged.coastal_quads, 3);
        let mut invalid = rich_batch(70_002);
        invalid.normals.pop();
        assert!(
            split_batches(vec![invalid])
                .unwrap_err()
                .contains("normals")
        );
        let mut invalid = rich_batch(70_002);
        invalid.indices[0] = u32::MAX;
        assert!(split_batches(vec![invalid]).unwrap_err().contains("index"));
    }
}
