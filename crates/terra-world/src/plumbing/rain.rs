//! Slowly collect local rain through the open top of loaded basin assemblies.
use crate::{TerraGenerator, edits::EditJournal, streaming::ChunkStore};
use std::collections::{BTreeMap, BTreeSet};
use terra_core::{
    WorldClock, WorldWeather,
    plumbing::{BASIN, BasinShape, Cell},
};

/// One eighth-block unit per thirty seconds per square metre at full rain.
pub const RAIN_UNITS_PER_SECOND: f32 = 1.0 / 30.0;
#[derive(Default)]
pub struct RainRuntime {
    elapsed: f32,
    wet_seconds: f64,
    cursor: Option<Cell>,
    credits: BTreeMap<BasinShape, (f64, f32)>,
}

fn sky_open(
    chunks: &ChunkStore,
    journal: &EditJournal,
    terrain: &TerraGenerator,
    top: Cell,
) -> bool {
    let height = chunks.surface_column_at(top[0], top[2]).map_or_else(
        || terrain.surface_height(top[0] as f64 + 0.5, top[2] as f64 + 0.5),
        |surface| surface.height,
    );
    let prepared = chunks.prepared_column([top[0].div_euclid(16), top[2].div_euclid(16)]);
    let ceiling = 356.max(top[1] + 64);
    for y in top[1] + 1..=ceiling {
        let cell = [top[0], y, top[2]];
        if let Some(material) = journal.block(cell).or_else(|| chunks.block_at(cell)) {
            if terra_voxel::MaterialId::from_raw(material).is_some_and(|id| id.is_solid()) {
                return false;
            }
        } else if y as f64 <= height {
            return false; // An unloaded underground ceiling is never assumed open.
        } else if y as f64 <= height + 32.0
            && prepared.as_ref().is_some_and(|column| {
                [6, 10, 36, 134, 7, 8, 9, 132, 133]
                    .into_iter()
                    .any(|material| column.tree_appearance_at(cell, material).is_some())
            })
        {
            return false;
        }
    }
    true
}

#[allow(clippy::too_many_arguments)]
pub fn collect_rain(
    runtime: &mut RainRuntime,
    chunks: &ChunkStore,
    journal: &mut EditJournal,
    terrain: &TerraGenerator,
    weather: &WorldWeather,
    clock: WorldClock,
    elapsed: f32,
) -> Result<u32, String> {
    if !elapsed.is_finite() || elapsed <= 0.0 || weather.intensity() <= 0.0 {
        return Ok(0);
    }
    runtime.elapsed += elapsed.min(1.0);
    runtime.wet_seconds += f64::from(elapsed.min(1.0));
    if runtime.elapsed < 1.0 {
        return Ok(0);
    }
    let sampled_elapsed = std::mem::take(&mut runtime.elapsed);
    let mut candidates: Vec<_> = journal.plumbing_after(runtime.cursor).take(64).collect();
    if candidates.is_empty() {
        runtime.cursor = None;
        candidates = journal.plumbing_positions().take(64).collect();
    }
    let mut seen = BTreeSet::new();
    let mut collected = 0;
    for cell in candidates {
        runtime.cursor = Some(cell);
        if chunks.block_at(cell) != Some(BASIN) {
            continue;
        }
        let Some(shape) = journal.plumbing_at(cell).and_then(|state| state.basin) else {
            continue;
        };
        if !seen.insert(shape) || !shape.cells().all(|p| chunks.block_at(p) == Some(BASIN)) {
            continue;
        }
        let mut rain = 0.0;
        for x in 0..shape.size[0] {
            for z in 0..shape.size[2] {
                let top = [
                    shape.origin[0] + i64::from(x),
                    shape.origin[1] + i64::from(shape.size[1]) - 1,
                    shape.origin[2] + i64::from(z),
                ];
                if sky_open(chunks, journal, terrain, top) {
                    rain += crate::weather::sample_at(
                        weather,
                        terrain,
                        top.map(|p| p as f64 + 0.5),
                        clock,
                    )
                    .rain;
                }
            }
        }
        if runtime.credits.len() >= super::MAX_NODES && !runtime.credits.contains_key(&shape) {
            runtime.credits.pop_first();
        }
        let (last, credit) = runtime
            .credits
            .entry(shape)
            .or_insert((runtime.wet_seconds - f64::from(sampled_elapsed), 0.0));
        let delta = (runtime.wet_seconds - *last).clamp(0.0, 30.0) as f32;
        *last = runtime.wet_seconds;
        *credit = (*credit + rain * delta * RAIN_UNITS_PER_SECOND).min(8.0);
        let total: u16 = shape
            .cells()
            .map(|p| u16::from(journal.plumbing_at(p).map_or(0, |state| state.water.amount)))
            .sum();
        let units = (*credit)
            .floor()
            .min(f32::from(shape.capacity().saturating_sub(total))) as u8;
        if units > 0 && super::fill_basin(chunks, journal, cell, units)? {
            *credit -= f32::from(units);
            collected += u32::from(units);
        }
        if total == shape.capacity() {
            *credit = 0.0;
        }
    }
    Ok(collected)
}
