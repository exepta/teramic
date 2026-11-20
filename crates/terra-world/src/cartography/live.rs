use super::{DetailTile, MapLayer, MapSample, TILE_SIZE};
use crate::{PreparedColumn, edits::EditJournal, streaming::ChunkStore, water::FluidCell};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use terra_voxel::{BlockSection, MaterialId};

/// Only loaded cells are copied. Meshing, generation and renderer assets are not
/// touched; the relatively long vertical scan runs on the caller's worker pool.
pub struct DetailTileSnapshot {
    coordinate: [i64; 2],
    layer: MapLayer,
    revision: u64,
    sections: HashMap<i64, BlockSection>,
    fluids: HashMap<[i64; 3], FluidCell>,
    column: Option<Arc<PreparedColumn>>,
}

/// Loaded-section metadata retained by the atlas. Publications refresh only
/// their columns; no block payloads or renderer assets are retained here.
pub struct DetailTileIndex {
    layer: MapLayer,
    positions: HashMap<[i64; 2], Vec<terra_voxel::SectionPos>>,
    revisions: HashMap<[i64; 2], u64>,
    sections: HashMap<terra_voxel::SectionPos, u64>,
    journal_sections: HashSet<terra_voxel::SectionPos>,
    columns: HashMap<[i64; 2], Arc<PreparedColumn>>,
    generations: HashMap<[i64; 2], u64>,
    generation: u64,
}

impl DetailTileIndex {
    pub fn new(chunks: &ChunkStore, journal: &EditJournal, layer: MapLayer) -> Self {
        let _scope = terra_core::profiling::ScopeTimer::new("mapping.index");
        let mut index = Self {
            layer,
            positions: HashMap::new(),
            revisions: HashMap::new(),
            sections: if layer == MapLayer::Surface {
                HashMap::with_capacity(chunks.loaded.len())
            } else {
                HashMap::new()
            },
            journal_sections: HashSet::new(),
            columns: HashMap::new(),
            generations: HashMap::new(),
            generation: 1,
        };
        // Bulk construction is a single loaded-table traversal, not an
        // incremental remove/reinsert for every initially absent section.
        for (&position, section) in &chunks.loaded {
            if !in_layer(position.y, layer) {
                continue;
            }
            let column = [position.x, position.z];
            let revision = section_revision(position, section.data.revision(), journal);
            index.sections.insert(position, revision);
            if journal_contributed(journal, position) {
                index.journal_sections.insert(position);
            }
            index.positions.entry(column).or_default().push(position);
            *index
                .revisions
                .entry(column)
                .or_insert_with(|| initial_revision(layer)) ^= revision;
        }
        for (&column, revision) in &mut index.revisions {
            if let Some(prepared) = chunks.prepared_column(column) {
                *revision ^= prepared_revision(&prepared);
                index.columns.insert(column, prepared);
            }
            index.generations.insert(column, index.generation);
        }
        index
    }

    pub fn layer(&self) -> MapLayer {
        self.layer
    }

    /// Conservative recovery for edits, source changes or mutations without a
    /// lifecycle message. Unchanged journal metadata does not invalidate tiles.
    pub fn rebuild(
        &mut self,
        chunks: &ChunkStore,
        journal: &EditJournal,
        layer: MapLayer,
        invalidate: bool,
    ) {
        let mut next = Self::new(chunks, journal, layer);
        next.generation = self.generation.wrapping_add(1);
        for (&column, generation) in &mut next.generations {
            *generation = if !invalidate
                && self.layer == layer
                && self.revisions.get(&column) == next.revisions.get(&column)
            {
                self.generations
                    .get(&column)
                    .copied()
                    .unwrap_or(next.generation)
            } else {
                next.generation
            };
        }
        *self = next;
    }

    pub fn refresh(
        &mut self,
        chunks: &ChunkStore,
        journal: &EditJournal,
        positions: impl IntoIterator<Item = terra_voxel::SectionPos>,
        columns: impl IntoIterator<Item = [i64; 2]>,
    ) {
        let _scope = terra_core::profiling::ScopeTimer::new("mapping.index.incremental");
        let mut changed: std::collections::HashSet<_> = columns.into_iter().collect();
        for position in positions {
            self.journal_sections.remove(&position);
            let column = [position.x, position.z];
            if let Some(previous) = self.sections.remove(&position) {
                if let Some(revision) = self.revisions.get_mut(&column) {
                    *revision ^= previous;
                }
                if let Some(positions) = self.positions.get_mut(&column) {
                    positions.retain(|p| *p != position);
                }
                changed.insert(column);
            }
            if !in_layer(position.y, self.layer) {
                continue;
            }
            let Some(section) = chunks.loaded.get(&position) else {
                continue;
            };
            let revision = section_revision(position, section.data.revision(), journal);
            self.sections.insert(position, revision);
            if journal_contributed(journal, position) {
                self.journal_sections.insert(position);
            }
            self.positions.entry(column).or_default().push(position);
            *self
                .revisions
                .entry(column)
                .or_insert_with(|| initial_revision(self.layer)) ^= revision;
            changed.insert(column);
        }
        for column in changed {
            if self.positions.get(&column).is_none_or(Vec::is_empty) {
                self.positions.remove(&column);
                self.revisions.remove(&column);
                self.columns.remove(&column);
                self.generations.remove(&column);
                continue;
            }
            if let Some(previous) = self.columns.remove(&column)
                && let Some(revision) = self.revisions.get_mut(&column)
            {
                *revision ^= prepared_revision(&previous);
            }
            if let Some(prepared) = chunks.prepared_column(column) {
                if let Some(revision) = self.revisions.get_mut(&column) {
                    *revision ^= prepared_revision(&prepared);
                }
                self.columns.insert(column, prepared);
            }
            self.generation = self.generation.wrapping_add(1);
            self.generations.insert(column, self.generation);
        }
    }

    /// Journal metadata and edits outside loaded map cells require no loaded
    /// world scan. Previously journal-backed sections also cover journal reset.
    pub fn refresh_journal(&mut self, chunks: &ChunkStore, journal: &EditJournal) -> usize {
        let _scope = terra_core::profiling::ScopeTimer::new("mapping.index.journal");
        let (candidates, _) = self.journal_candidates(journal);
        let changed: Vec<_> = candidates
            .into_iter()
            .filter(|position| {
                chunks.loaded.get(position).is_some_and(|section| {
                    self.sections.get(position).copied()
                        != Some(section_revision(
                            *position,
                            section.data.revision(),
                            journal,
                        ))
                })
            })
            .collect();
        let count = changed.len();
        if count != 0 {
            self.refresh(chunks, journal, changed, []);
        }
        count
    }

    fn journal_candidates(
        &self,
        journal: &EditJournal,
    ) -> (HashSet<terra_voxel::SectionPos>, usize) {
        let sections = journal.changed_sections();
        let historical = sections.size_hint().0;
        if historical > self.sections.len() {
            // Long exploration histories must not make a small loaded view
            // increasingly expensive. This domain already includes old epochs.
            (self.sections.keys().copied().collect(), self.sections.len())
        } else {
            let candidates: HashSet<_> = sections
                .chain(self.journal_sections.iter().copied())
                .filter(|position| self.sections.contains_key(position))
                .collect();
            let inspected = candidates.len();
            (candidates, inspected)
        }
    }

    pub fn revisions(&self) -> &HashMap<[i64; 2], u64> {
        &self.revisions
    }

    /// Scheduler token also distinguishes equal-revision section replacements
    /// and immutable column swaps while an older worker is still running.
    pub fn source_revision(&self, coordinate: [i64; 2]) -> Option<u64> {
        Some(mix(
            *self.revisions.get(&coordinate)? ^ self.generations.get(&coordinate)?.rotate_left(19)
        ))
    }

    pub fn capture(
        &self,
        chunks: &ChunkStore,
        journal: &EditJournal,
        coordinate: [i64; 2],
    ) -> Option<DetailTileSnapshot> {
        let _scope = terra_core::profiling::ScopeTimer::new("mapping.snapshot");
        let revision = *self.revisions.get(&coordinate)?;
        Some(DetailTileSnapshot::capture_sections(
            chunks,
            journal,
            coordinate,
            self.layer,
            revision,
            self.positions.get(&coordinate)?.iter().copied(),
        ))
    }
}

fn journal_contributed(journal: &EditJournal, position: terra_voxel::SectionPos) -> bool {
    journal.revision(position) != 0 || journal.fluid_epoch(position) != 0
}

fn prepared_revision(column: &Arc<PreparedColumn>) -> u64 {
    mix(Arc::as_ptr(column) as usize as u64)
}

fn in_layer(y: i64, layer: MapLayer) -> bool {
    match layer {
        MapLayer::Surface => true,
        MapLayer::Cave { level } => {
            let level = i64::from(level);
            (level - 25).div_euclid(16) <= y && y <= (level + 5).div_euclid(16)
        }
    }
}

fn mix(mut value: u64) -> u64 {
    value ^= value >> 30;
    value = value.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value ^= value >> 27;
    value = value.wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

fn initial_revision(layer: MapLayer) -> u64 {
    match layer {
        MapLayer::Surface => 0x7375_7266_6163_6500,
        MapLayer::Cave { level } => mix(level as i64 as u64),
    }
}

fn section_revision(
    position: terra_voxel::SectionPos,
    revision: u64,
    journal: &EditJournal,
) -> u64 {
    let key = mix(position.y as u64);
    mix(key ^ revision.wrapping_mul(0x9e37_79b9))
        ^ mix(key.rotate_left(17) ^ journal.revision(position).wrapping_add(2))
        ^ mix(key.rotate_left(31) ^ journal.fluid_epoch(position).wrapping_add(3))
}

/// Batch variant for map schedulers. Reads the loaded-section table once,
/// instead of scanning it for every tile in the visible map window.
pub fn detail_revisions(
    chunks: &ChunkStore,
    journal: &EditJournal,
    layer: MapLayer,
) -> HashMap<[i64; 2], u64> {
    let mut revisions = HashMap::new();
    for (&position, section) in &chunks.loaded {
        if !in_layer(position.y, layer) {
            continue;
        }
        let revision = revisions
            .entry([position.x, position.z])
            .or_insert_with(|| initial_revision(layer));
        *revision ^= section_revision(position, section.data.revision(), journal);
    }
    for (&coordinate, revision) in &mut revisions {
        if let Some(column) = chunks.prepared_column(coordinate) {
            *revision ^= prepared_revision(&column);
        }
    }
    revisions
}

/// Cheap dirty check. Includes loading, edits and liquid updates, while leaving
/// animation and mesh LOD changes out of the cartography invalidation path.
pub fn detail_revision(
    chunks: &ChunkStore,
    journal: &EditJournal,
    coordinate: [i64; 2],
    layer: MapLayer,
) -> Option<u64> {
    let mut any = false;
    let mut revision = initial_revision(layer);
    for (&position, section) in &chunks.loaded {
        if [position.x, position.z] != coordinate || !in_layer(position.y, layer) {
            continue;
        }
        any = true;
        revision ^= section_revision(position, section.data.revision(), journal);
    }
    any.then(|| {
        if let Some(column) = chunks.prepared_column(coordinate) {
            revision ^= prepared_revision(&column);
        }
        revision
    })
}

impl DetailTileSnapshot {
    pub fn capture(
        chunks: &ChunkStore,
        journal: &EditJournal,
        coordinate: [i64; 2],
        layer: MapLayer,
    ) -> Option<Self> {
        let revision = detail_revision(chunks, journal, coordinate, layer)?;
        let positions = chunks.loaded.keys().copied().filter(|position| {
            [position.x, position.z] == coordinate && in_layer(position.y, layer)
        });
        Some(Self::capture_sections(
            chunks, journal, coordinate, layer, revision, positions,
        ))
    }

    fn capture_sections(
        chunks: &ChunkStore,
        journal: &EditJournal,
        coordinate: [i64; 2],
        layer: MapLayer,
        revision: u64,
        positions: impl Iterator<Item = terra_voxel::SectionPos>,
    ) -> Self {
        let mut sections = HashMap::new();
        let mut fluids = HashMap::new();
        for position in positions {
            let Some(loaded) = chunks.loaded.get(&position) else {
                continue;
            };
            let mut section = loaded.data.clone();
            if journal.revision(position) != 0 {
                for (local, material) in journal.section_delta(position).blocks {
                    let _ = section.set_block(local.map(usize::from), material);
                }
            }
            sections.insert(position.y, section);
            fluids.extend(journal.fluid_cells_in(position));
        }
        Self {
            coordinate,
            layer,
            revision,
            sections,
            fluids,
            column: chunks.prepared_column(coordinate),
        }
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn render(self) -> DetailTile {
        let mut tile = DetailTile {
            coordinate: self.coordinate,
            layer: self.layer,
            revision: self.revision,
            cells: [MapSample::default(); TILE_SIZE * TILE_SIZE],
        };
        let mut heights: Vec<_> = self.sections.keys().copied().collect();
        heights.sort_unstable_by(|a, b| b.cmp(a));
        for z in 0..TILE_SIZE {
            for x in 0..TILE_SIZE {
                let global = [
                    self.coordinate[0] * 16 + x as i64,
                    0,
                    self.coordinate[1] * 16 + z as i64,
                ];
                let sample = match self.layer {
                    MapLayer::Surface => self.surface(global, &heights),
                    MapLayer::Cave { level } => self.cave(global, i64::from(level)),
                };
                tile.cells[z * TILE_SIZE + x] = sample;
            }
        }
        tile
    }

    fn block(&self, position: [i64; 3]) -> Option<u16> {
        self.sections
            .get(&position[1].div_euclid(16))?
            .block(position.map(|v| v.rem_euclid(16) as usize))
            .ok()
    }

    fn fluid(&self, position: [i64; 3], material: u16) -> FluidCell {
        if !crate::water::can_hold_fluid(material) {
            return FluidCell::default();
        }
        if let Some(cell) = self.fluids.get(&position) {
            return *cell;
        }
        if material == MaterialId::FluidWater as u16 {
            return FluidCell {
                amount: crate::water::FLUID_CAPACITY,
                source: true,
                ..Default::default()
            };
        }
        let natural = self
            .column
            .as_ref()
            .and_then(|column| column.surface_at(position[0], position[2]))
            .and_then(|surface| {
                surface.water_level.map(|level| crate::water::WaterColumn {
                    level,
                    bed: surface.water_bed(),
                    depth: level - surface.water_bed(),
                })
            });
        crate::water::fluid::from_column(natural, position[1])
    }

    fn sample(&self, position: [i64; 3], material: u16, depth: u8) -> MapSample {
        let mut sample = MapSample {
            material,
            height: (position[1] + 1).clamp(i64::from(i16::MIN), i64::from(i16::MAX)) as i16,
            water_depth: depth,
            known: true,
            canopy: terra_voxel::MaterialId::FOLIAGE.contains(&material),
            ..Default::default()
        };
        if let Some(surface) = self
            .column
            .as_ref()
            .and_then(|column| column.surface_at(position[0], position[2]))
        {
            sample.biome_weights = surface.biome.weights.values().map(|v| v as f32);
            sample.shore = surface.shore_cover as f32;
            sample.snow = if surface.biome.aridity() <= 0.5 {
                surface.snow_cover as f32
            } else {
                0.0
            };
        }
        sample
    }

    fn surface(&self, mut position: [i64; 3], heights: &[i64]) -> MapSample {
        for &section_y in heights {
            for local_y in (0..16).rev() {
                position[1] = section_y * 16 + local_y;
                let Some(material) = self.block(position) else {
                    continue;
                };
                if self.fluid(position, material).amount > 0 {
                    return self.water_sample(position);
                }
                if material != MaterialId::Air as u16 && material != MaterialId::Torch as u16 {
                    return self.sample(position, material, 0);
                }
            }
        }
        MapSample::default()
    }

    fn water_sample(&self, position: [i64; 3]) -> MapSample {
        let mut depth = 1;
        for down in 1..=32 {
            let below = [position[0], position[1] - down, position[2]];
            let Some(material) = self.block(below) else {
                break;
            };
            if self.fluid(below, material).amount == 0 {
                break;
            }
            depth += 1;
        }
        self.sample(position, MaterialId::FluidWater as u16, depth)
    }

    fn cave(&self, mut position: [i64; 3], level: i64) -> MapSample {
        // Follow a continuous air/fluid column down from this slice. This never
        // jumps through a roof into an unrelated room on another cave storey.
        let mut start = None;
        for up in 0..=3 {
            position[1] = level + up;
            let Some(material) = self.block(position) else {
                return MapSample::default();
            };
            if !opaque_floor(material) {
                start = Some(position[1]);
                break;
            }
        }
        let Some(start) = start else {
            return MapSample::default();
        };
        let mut water_top = None;
        for y in (level - 24..=start).rev() {
            position[1] = y;
            let Some(material) = self.block(position) else {
                return MapSample::default();
            };
            if self.fluid(position, material).amount > 0 && water_top.is_none() {
                water_top = Some(position);
            }
            if opaque_floor(material) {
                let headroom = [y + 1, y + 2].into_iter().all(|above| {
                    self.block([position[0], above, position[2]])
                        .is_some_and(|block| !opaque_floor(block))
                });
                if !headroom {
                    return MapSample::default();
                }
                return water_top.map_or_else(
                    || self.sample(position, material, 0),
                    |water| self.water_sample(water),
                );
            }
        }
        MapSample::default()
    }
}

fn opaque_floor(material: u16) -> bool {
    MaterialId::from_raw(material).is_some_and(|material| {
        material.is_solid()
            && !matches!(
                material,
                MaterialId::Leaves
                    | MaterialId::AutumnLeaves
                    | MaterialId::SilverLeaves
                    | MaterialId::CherryLeaves
                    | MaterialId::WhiteCherryLeaves
            )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::streaming::LoadedSection;
    use terra_voxel::SectionPos;

    fn insert(chunks: &mut ChunkStore, data: BlockSection) {
        chunks.loaded.insert(
            data.position(),
            LoadedSection {
                data,
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            },
        );
    }

    #[test]
    fn indexed_snapshots_match_direct_capture_across_layers_and_edits() {
        let mut chunks = ChunkStore::default();
        let mut journal = EditJournal::default();
        for x in [-2, 0, 3] {
            for y in [-3, 0, 2] {
                let mut data = BlockSection::empty(SectionPos::new(x, y, -1)).unwrap();
                data.set_block([4, 2, 7], MaterialId::Stone as u16).unwrap();
                insert(&mut chunks, data);
            }
        }
        journal
            .record([-28, 2, -9], MaterialId::Dirt as u16)
            .unwrap();
        for layer in [
            MapLayer::Surface,
            MapLayer::Cave { level: -30 },
            MapLayer::Cave { level: 8 },
        ] {
            let index = DetailTileIndex::new(&chunks, &journal, layer);
            assert_eq!(
                index.revisions(),
                &detail_revisions(&chunks, &journal, layer)
            );
            for coordinate in [[-2, -1], [0, -1], [3, -1]] {
                let indexed = index
                    .capture(&chunks, &journal, coordinate)
                    .unwrap()
                    .render();
                let direct = DetailTileSnapshot::capture(&chunks, &journal, coordinate, layer)
                    .unwrap()
                    .render();
                assert_eq!(indexed.revision, direct.revision);
                assert_eq!(indexed.cells, direct.cells);
            }
            assert!(index.capture(&chunks, &journal, [9, 9]).is_none());
        }
    }

    fn assert_index_matches_world(
        index: &DetailTileIndex,
        chunks: &ChunkStore,
        journal: &EditJournal,
    ) {
        assert_eq!(
            index.revisions(),
            &detail_revisions(chunks, journal, index.layer())
        );
        assert_eq!(
            index.sections.len(),
            chunks
                .loaded
                .keys()
                .filter(|p| in_layer(p.y, index.layer()))
                .count()
        );
        for &column in index.revisions().keys() {
            let indexed = index.capture(chunks, journal, column).unwrap().render();
            let direct = DetailTileSnapshot::capture(chunks, journal, column, index.layer())
                .unwrap()
                .render();
            assert_eq!(indexed.revision, direct.revision);
            assert_eq!(indexed.cells, direct.cells);
        }
    }

    #[test]
    fn incremental_index_matches_new_replaced_removed_and_negative_cave_sections() {
        let mut chunks = ChunkStore::default();
        let journal = EditJournal::default();
        let first = SectionPos::new(-2, -2, -1);
        let second = SectionPos::new(-2, -1, -1);
        let distant = SectionPos::new(41, 80, -17);
        for layer in [MapLayer::Surface, MapLayer::Cave { level: -16 }] {
            chunks.loaded.clear();
            let mut index = DetailTileIndex::new(&chunks, &journal, layer);
            for position in [first, second, distant] {
                let mut data = BlockSection::empty(position).unwrap();
                data.set_block([3, 4, 5], MaterialId::Stone as u16).unwrap();
                insert(&mut chunks, data);
                index.refresh(&chunks, &journal, [position, position], []);
                assert_index_matches_world(&index, &chunks, &journal);
            }
            let before = index.source_revision([-2, -1]);
            // Equal storage revisions still represent a newly published payload.
            let mut data = BlockSection::empty(first).unwrap();
            data.set_block([3, 4, 5], MaterialId::Dirt as u16).unwrap();
            insert(&mut chunks, data);
            index.refresh(&chunks, &journal, [first], []);
            assert_ne!(index.source_revision([-2, -1]), before);
            assert_index_matches_world(&index, &chunks, &journal);
            for position in [first, second, distant] {
                chunks.loaded.remove(&position);
                index.refresh(&chunks, &journal, [position], []);
                assert_index_matches_world(&index, &chunks, &journal);
            }
            assert!(index.positions.is_empty());
            assert!(index.columns.is_empty());
            assert!(index.generations.is_empty());
        }
    }

    #[test]
    fn immutable_column_swaps_and_evictions_update_only_their_indexed_column() {
        let generator = crate::SectionGenerator::new(
            crate::TerraGenerator::new(terra_core::WorldConfig::default()).unwrap(),
        );
        let mut chunks = ChunkStore::default();
        let journal = EditJournal::default();
        for x in [-2, -1] {
            insert(
                &mut chunks,
                BlockSection::empty(SectionPos::new(x, -1, -1)).unwrap(),
            );
            chunks.set_prepared_column_for_test(
                [x, -1],
                Some(Arc::new(generator.prepare_column([x, -1], false).unwrap())),
            );
        }
        let mut index = DetailTileIndex::new(&chunks, &journal, MapLayer::Surface);
        let unrelated = index.source_revision([-2, -1]);
        let previous = index.source_revision([-1, -1]);
        chunks.set_prepared_column_for_test(
            [-1, -1],
            Some(Arc::new(generator.prepare_column([-1, -1], false).unwrap())),
        );
        index.refresh(&chunks, &journal, [], [[-1, -1]]);
        assert_ne!(index.source_revision([-1, -1]), previous);
        assert_eq!(index.source_revision([-2, -1]), unrelated);
        assert_index_matches_world(&index, &chunks, &journal);
        chunks.set_prepared_column_for_test([-1, -1], None);
        index.refresh(&chunks, &journal, [], [[-1, -1]]);
        assert_index_matches_world(&index, &chunks, &journal);
        assert_eq!(index.columns.len(), 1);
    }

    #[test]
    fn journal_recovery_handles_unannounced_fluids_without_reinvalidating_unchanged_tiles() {
        let mut chunks = ChunkStore::default();
        let mut journal = EditJournal::default();
        for x in [-1, 0] {
            insert(
                &mut chunks,
                BlockSection::empty(SectionPos::new(x, -1, -1)).unwrap(),
            );
        }
        let layer = MapLayer::Cave { level: -8 };
        let mut index = DetailTileIndex::new(&chunks, &journal, layer);
        let before = index.source_revision([-1, -1]);
        let unrelated = index.source_revision([0, -1]);
        journal
            .set_fluid(
                [-1, -9, -1],
                FluidCell {
                    amount: 8,
                    source: true,
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(index.refresh_journal(&chunks, &journal), 1);
        assert_ne!(index.source_revision([-1, -1]), before);
        assert_eq!(index.source_revision([0, -1]), unrelated);
        assert_index_matches_world(&index, &chunks, &journal);
        let stable = index.source_revision([-1, -1]);
        assert_eq!(index.refresh_journal(&chunks, &journal), 0);
        assert_eq!(index.source_revision([-1, -1]), stable);
        index.rebuild(&chunks, &journal, layer, true);
        assert_ne!(
            index.source_revision([-1, -1]),
            stable,
            "unexplained store and generator changes invalidate equal revision numbers"
        );
        let stable = index.source_revision([-1, -1]);
        index.rebuild(&chunks, &journal, MapLayer::Surface, false);
        assert_ne!(index.source_revision([-1, -1]), stable);
        assert_index_matches_world(&index, &chunks, &journal);
    }

    #[test]
    fn sparse_journal_refresh_ignores_metadata_unloaded_edits_and_restores_cleared_epochs() {
        let mut chunks = ChunkStore::default();
        let mut journal = EditJournal::default();
        for x in -8..8 {
            insert(
                &mut chunks,
                BlockSection::empty(SectionPos::new(x, -1, -1)).unwrap(),
            );
        }
        for layer in [MapLayer::Surface, MapLayer::Cave { level: -8 }] {
            let mut index = DetailTileIndex::new(&chunks, &journal, layer);
            let original = index.revisions().clone();
            let tokens: HashMap<_, _> = original
                .keys()
                .map(|&column| (column, index.source_revision(column)))
                .collect();
            journal.clock = Some(Default::default());
            assert_eq!(index.refresh_journal(&chunks, &journal), 0);
            journal
                .record([800, -4, -1], MaterialId::Dirt as u16)
                .unwrap();
            assert_eq!(index.refresh_journal(&chunks, &journal), 0);
            assert_eq!(index.revisions(), &original);
            for (&column, &token) in &tokens {
                assert_eq!(index.source_revision(column), token);
            }
            journal
                .record([-1, -4, -1], MaterialId::Dirt as u16)
                .unwrap();
            journal
                .set_fluid(
                    [-17, -5, -1],
                    FluidCell {
                        amount: 8,
                        source: true,
                        ..Default::default()
                    },
                )
                .unwrap();
            assert_eq!(index.refresh_journal(&chunks, &journal), 2);
            assert_eq!(index.journal_sections.len(), 2);
            assert_index_matches_world(&index, &chunks, &journal);
            let before_reset = index.source_revision([-1, -1]);
            journal = EditJournal::default();
            assert_eq!(index.refresh_journal(&chunks, &journal), 2);
            assert!(index.journal_sections.is_empty());
            assert_ne!(index.source_revision([-1, -1]), before_reset);
            assert_eq!(index.revisions(), &original);
            assert_index_matches_world(&index, &chunks, &journal);
            assert_eq!(index.refresh_journal(&chunks, &journal), 0);
        }
    }

    #[test]
    fn distant_edit_history_cannot_exceed_the_loaded_journal_refresh_domain() {
        let mut chunks = ChunkStore::default();
        let mut journal = EditJournal::default();
        insert(
            &mut chunks,
            BlockSection::empty(SectionPos::new(-1, -1, -1)).unwrap(),
        );
        for x in 1..=4096 {
            journal
                .record([x * 16, 1, 0], MaterialId::Dirt as u16)
                .unwrap();
        }
        for layer in [MapLayer::Surface, MapLayer::Cave { level: -8 }] {
            let mut index = DetailTileIndex::new(&chunks, &journal, layer);
            assert!(index.journal_sections.is_empty());
            let (candidates, inspected) = index.journal_candidates(&journal);
            assert_eq!(inspected, 1);
            assert_eq!(candidates.len(), 1);
            assert_eq!(index.refresh_journal(&chunks, &journal), 0);
            journal
                .record([-1, -4, -1], MaterialId::Dirt as u16)
                .unwrap();
            assert_eq!(index.refresh_journal(&chunks, &journal), 1);
            assert_index_matches_world(&index, &chunks, &journal);
            let reset = EditJournal::default();
            let (_, inspected) = index.journal_candidates(&reset);
            assert_eq!(
                inspected, 1,
                "cleared journal still revisits its old loaded contribution"
            );
            assert_eq!(index.refresh_journal(&chunks, &reset), 1);
            assert_index_matches_world(&index, &chunks, &reset);
            journal = reset;
            for x in 1..=4096 {
                journal
                    .record([x * 16, 1, 0], MaterialId::Dirt as u16)
                    .unwrap();
            }
        }
    }

    #[test]
    fn live_surface_uses_canopies_constructions_and_negative_local_coordinates() {
        let position = SectionPos::new(-1, 0, -1);
        let mut data = BlockSection::empty(position).unwrap();
        data.set_block([15, 2, 15], MaterialId::Grass as u16)
            .unwrap();
        data.set_block([15, 7, 15], MaterialId::Leaves as u16)
            .unwrap();
        let mut chunks = ChunkStore::default();
        insert(&mut chunks, data);
        let mut journal = EditJournal::default();
        let snapshot = || {
            DetailTileSnapshot::capture(&chunks, &journal, [-1, -1], MapLayer::Surface)
                .unwrap()
                .render()
        };
        let before = snapshot();
        let canopy = before.cells[255];
        assert_eq!(canopy.material, MaterialId::Leaves as u16);
        assert_eq!(canopy.height, 8);
        assert!(canopy.canopy && canopy.known);
        assert!(!before.cells[0].known);
        // A confirmed change is visible before the asynchronous terrain remesh.
        journal.record([-1, 7, -1], 0).unwrap();
        journal
            .record([-1, 5, -1], MaterialId::OakPlanks as u16)
            .unwrap();
        let after = DetailTileSnapshot::capture(&chunks, &journal, [-1, -1], MapLayer::Surface)
            .unwrap()
            .render();
        assert_ne!(after.revision, before.revision);
        assert_eq!(after.cells[255].material, MaterialId::OakPlanks as u16);
        assert_eq!(after.cells[255].height, 6);
        assert!(!after.cells[255].canopy);
    }

    #[test]
    fn cave_slice_does_not_map_roofs_hidden_storeys_or_unloaded_space() {
        let position = SectionPos::new(0, 0, 0);
        let mut data = BlockSection::empty(position).unwrap();
        for z in 0..16 {
            for x in 0..16 {
                data.set_block([x, 1, z], MaterialId::Stone as u16).unwrap();
                data.set_block([x, 9, z], MaterialId::Stone as u16).unwrap();
                data.set_block([x, 14, z], MaterialId::Grass as u16)
                    .unwrap();
            }
        }
        for y in 2..9 {
            data.set_block([8, y, 8], MaterialId::IronOre as u16)
                .unwrap();
        }
        let mut chunks = ChunkStore::default();
        insert(&mut chunks, data);
        let journal = EditJournal::default();
        let tile =
            DetailTileSnapshot::capture(&chunks, &journal, [0, 0], MapLayer::Cave { level: 4 })
                .unwrap()
                .render();
        assert_eq!(tile.cells[0].material, MaterialId::Stone as u16);
        assert_eq!(tile.cells[0].height, 2);
        assert!(!tile.cells[8 + 8 * 16].known, "solid walls stay occluded");
        assert!(
            DetailTileSnapshot::capture(&chunks, &journal, [1, 0], MapLayer::Cave { level: 4 })
                .is_none()
        );
        let below =
            DetailTileSnapshot::capture(&chunks, &journal, [0, 0], MapLayer::Cave { level: -3 })
                .unwrap()
                .render();
        assert!(below.cells.iter().all(|cell| !cell.known));
    }

    #[test]
    fn fluid_changes_invalidate_live_tiles_without_terrain_generation() {
        let mut data = BlockSection::empty(SectionPos::new(0, 0, 0)).unwrap();
        data.set_block([3, 1, 4], MaterialId::Clay as u16).unwrap();
        let mut chunks = ChunkStore::default();
        insert(&mut chunks, data);
        let mut journal = EditJournal::default();
        let before = detail_revision(&chunks, &journal, [0, 0], MapLayer::Surface).unwrap();
        journal
            .set_fluid(
                [3, 2, 4],
                FluidCell {
                    amount: 8,
                    source: true,
                    ..Default::default()
                },
            )
            .unwrap();
        let tile = DetailTileSnapshot::capture(&chunks, &journal, [0, 0], MapLayer::Surface)
            .unwrap()
            .render();
        assert_ne!(before, tile.revision);
        assert_eq!(
            tile.cells[3 + 4 * 16].material,
            MaterialId::FluidWater as u16
        );
        assert_eq!(tile.cells[3 + 4 * 16].water_depth, 1);
        assert_eq!(tile.cells[3 + 4 * 16].height, 3);
    }
}
