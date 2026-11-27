use bevy::prelude::*;
use std::collections::BTreeMap;
use terra_core::fog_blocks::{MATERIAL, RADIUS};
use terra_voxel::SectionPos;
use terra_world::{edits::EditJournal, streaming::ChunkStore};

pub(super) const SIZE: usize = 41;
pub(super) const CELL: i64 = 2;
pub(super) const MAX_SOURCES: usize = 16;
pub(super) const HEIGHT: f32 = 3.2;
pub(super) type Position = [i64; 3];

#[derive(Default)]
pub(super) struct Index(BTreeMap<SectionPos, (u64, u64, Vec<Position>)>);
impl Index {
    pub fn invalidate(&mut self, position: SectionPos) {
        self.0.remove(&position);
    }
    pub fn refresh(&mut self, eye: Vec3, chunks: &ChunkStore, journal: &EditJournal) {
        // nearby() accepts a 32 m cube. Include rounding at either end of each
        // axis, including when a far-world f32 camera cannot distinguish voxels.
        let low = eye
            .to_array()
            .map(|v| ((v - 32.0).next_down() as f64 - 1.0).floor() as i64);
        let high = eye
            .to_array()
            .map(|v| ((v + 32.0).next_up() as f64 + 1.0).ceil() as i64);
        let low = low.map(|v| v.div_euclid(16));
        let high = high.map(|v| v.div_euclid(16));
        let in_range = |p: SectionPos| {
            (low[0]..=high[0]).contains(&p.x)
                && (low[1]..=high[1]).contains(&p.y)
                && (low[2]..=high[2]).contains(&p.z)
        };
        self.0
            .retain(|p, _| in_range(*p) && chunks.loaded.contains_key(p));
        let mut refresh = |p: SectionPos, loaded: &terra_world::streaming::LoadedSection| {
            let revision = loaded.data.revision();
            let epoch = journal.mesh_epoch(p);
            if self
                .0
                .get(&p)
                .is_some_and(|(r, e, _)| (*r, *e) == (revision, epoch))
            {
                return;
            }
            let mut sources = Vec::new();
            if loaded.data.may_contain(MATERIAL) {
                for (i, &raw) in loaded.data.blocks().iter().enumerate() {
                    if raw == MATERIAL
                        && let Ok(global) = p.global_block([i % 16, (i / 16) % 16, i / 256])
                        && journal.block(global).unwrap_or(raw) == MATERIAL
                    {
                        sources.push(global);
                    }
                }
            }
            for (local, raw) in journal.blocks_in_section(p) {
                if raw == MATERIAL
                    && loaded.data.block(local) != Ok(MATERIAL)
                    && let Ok(global) = p.global_block(local)
                {
                    sources.push(global);
                }
            }
            self.0.insert(p, (revision, epoch, sources));
        };
        let volume = (0..3).fold(1u64, |n, a| n.saturating_mul(high[a].abs_diff(low[a]) + 1));
        if volume > chunks.loaded.len() as u64 {
            for (&p, loaded) in &chunks.loaded {
                if in_range(p) {
                    refresh(p, loaded);
                }
            }
        } else {
            for z in low[2]..=high[2] {
                for y in low[1]..=high[1] {
                    for x in low[0]..=high[0] {
                        let p = SectionPos::new(x, y, z);
                        if let Some(loaded) = chunks.loaded.get(&p) {
                            refresh(p, loaded);
                        }
                    }
                }
            }
        }
    }
    pub fn nearby(&self, eye: Vec3) -> Vec<Position> {
        let mut candidates: Vec<_> = self
            .0
            .values()
            .flat_map(|(_, _, positions)| positions)
            .copied()
            .filter(|p| {
                (p[0] as f32 + 0.5 - eye.x).abs() <= 32.0
                    && (p[2] as f32 + 0.5 - eye.z).abs() <= 32.0
                    && (p[1] as f32 - eye.y).abs() <= 32.0
            })
            .collect();
        candidates.sort_by(|a, b| {
            let distance =
                |p: Position| Vec3::from_array(p.map(|v| v as f32)).distance_squared(eye);
            distance(*a).total_cmp(&distance(*b)).then_with(|| a.cmp(b))
        });
        candidates.truncate(MAX_SOURCES);
        candidates
    }
}

fn block(chunks: &ChunkStore, journal: &EditJournal, p: Position) -> Option<u16> {
    chunks
        .block_at(p)
        .map(|raw| journal.block(p).unwrap_or(raw))
}
fn supporting(raw: u16) -> bool {
    terra_voxel::MaterialId::from_raw(raw)
        .is_some_and(|m| m.is_opaque() && !terra_voxel::MaterialId::FOLIAGE.contains(&raw))
}
pub(super) fn source_height(
    chunks: &ChunkStore,
    journal: &EditJournal,
    p: Position,
) -> Option<f32> {
    // A floor may cover the emitter. Walk only the connected loaded cover,
    // never project fog through an arbitrary ceiling or an unloaded section.
    for y in p[1] + 1..=p[1] + 9 {
        let raw = block(chunks, journal, [p[0], y, p[2]])?;
        if !supporting(raw) {
            return Some(y as f32 + 0.045);
        }
    }
    None
}
pub(super) fn radial_weight(x: f32, z: f32, sources: &[Vec4]) -> f32 {
    sources
        .iter()
        .map(|p| {
            let distance = Vec2::new(x - p.x, z - p.z).length();
            let q = ((RADIUS - distance) / 1.8).clamp(0.0, 1.0);
            q * q * (3.0 - 2.0 * q)
        })
        .fold(0.0, f32::max)
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Field {
    pub origin: [i64; 2],
    pub heights: Vec<f32>,
    pub weights: Vec<f32>,
    pub sources: Vec<Vec4>,
}
impl Field {
    pub fn sample(
        eye: Vec3,
        positions: &[Position],
        chunks: &ChunkStore,
        journal: &EditJournal,
    ) -> Self {
        let origin = [eye.x, eye.z].map(|v| (v.floor() as i64).div_euclid(CELL) * CELL - 40);
        let sources: Vec<_> = positions
            .iter()
            .filter_map(|&p| {
                source_height(chunks, journal, p)
                    .map(|height| Vec4::new(p[0] as f32 + 0.5, height, p[2] as f32 + 0.5, 1.0))
            })
            .collect();
        let mut result = Self {
            origin,
            heights: vec![0.0; SIZE * SIZE],
            weights: vec![0.0; SIZE * SIZE],
            sources,
        };
        for z in 0..SIZE {
            for x in 0..SIZE {
                let px = origin[0] + x as i64 * CELL;
                let pz = origin[1] + z as i64 * CELL;
                let weight = radial_weight(px as f32, pz as f32, &result.sources);
                if weight <= 0.0 {
                    continue;
                }
                let hint = result
                    .sources
                    .iter()
                    .filter(|s| Vec2::new(px as f32 - s.x, pz as f32 - s.z).length() <= RADIUS)
                    .map(|s| s.y)
                    .max_by(f32::total_cmp)
                    .unwrap_or(eye.y);
                // Follow the floor nearest the source's air layer. Taking
                // the highest support would move indoor fog onto a roof.
                let mut closest: Option<f32> = None;
                for y in ((hint.floor() as i64 - 6)..=(hint.floor() as i64 + 6)).rev() {
                    if block(chunks, journal, [px, y, pz]).is_some_and(supporting)
                        && block(chunks, journal, [px, y + 1, pz])
                            .is_some_and(|raw| !supporting(raw))
                    {
                        let floor = y as f32 + 1.045;
                        if closest.is_none_or(|old| (floor - hint).abs() < (old - hint).abs()) {
                            closest = Some(floor);
                        }
                    }
                }
                if let Some(height) = closest {
                    let i = x + z * SIZE;
                    result.heights[i] = height;
                    result.weights[i] = weight;
                }
            }
        }
        // Density-zero cells retain a nearby height so bilinear interpolation
        // does not slope fog toward world zero at the edge of a circle.
        let valid: Vec<_> = (0..SIZE * SIZE)
            .filter(|&i| result.weights[i] > 0.0)
            .collect();
        for i in 0..SIZE * SIZE {
            if result.weights[i] == 0.0
                && let Some(&j) = valid.iter().min_by_key(|&&j| {
                    (i % SIZE).abs_diff(j % SIZE).pow(2) + (i / SIZE).abs_diff(j / SIZE).pow(2)
                })
            {
                result.heights[i] = result.heights[j];
            }
        }
        result
    }
    pub fn occupied(&self) -> usize {
        self.weights.iter().filter(|&&v| v > 0.0).count()
    }
    pub fn bounds(&self) -> (Vec3, Vec3) {
        let low = self.heights.iter().copied().fold(f32::INFINITY, f32::min);
        let high = self
            .heights
            .iter()
            .copied()
            .fold(f32::NEG_INFINITY, f32::max)
            + HEIGHT;
        (
            Vec3::new(
                self.origin[0] as f32 + 40.0,
                (low + high) * 0.5,
                self.origin[1] as f32 + 40.0,
            ),
            Vec3::new(40.0, (high - low) * 0.5, 40.0),
        )
    }
    pub fn pixels(&self) -> (Vec<u8>, f32, f32) {
        let base = self.heights.iter().copied().fold(f32::INFINITY, f32::min);
        let span = (self
            .heights
            .iter()
            .copied()
            .fold(f32::NEG_INFINITY, f32::max)
            - base)
            .max(1.0);
        let mut pixels = Vec::with_capacity(SIZE * SIZE * 4);
        for (i, &h) in self.heights.iter().enumerate() {
            let height = (((h - base) / span).clamp(0.0, 1.0) * 65535.0).round() as u16;
            pixels.extend([
                (height >> 8) as u8,
                height as u8,
                (self.weights[i] * 255.0).round() as u8,
                255,
            ]);
        }
        (pixels, base, span)
    }
}
