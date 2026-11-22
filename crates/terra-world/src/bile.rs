//! Loaded-cell-only acid flow. Sources are persistent and spread at half water's tick rate.
use crate::{
    edits::{BlockObjectState, EditJournal, LiquidState},
    streaming::{
        BlockEditRequest, ChunkStore, WorldAccess, commit_block_edit, commit_object_state,
    },
};
use bevy::prelude::*;
use std::collections::{HashMap, VecDeque};
use terra_core::{bile, block_properties::BlockPropertiesCatalog};
use terra_voxel::global_to_section;

mod surface;
pub use surface::BileSurface;
pub(crate) use surface::height_at;

pub const CELL_BUDGET: usize = 512;
pub const MAX_CELLS: usize = 16_384;
const SIDES: [[i64; 3]; 4] = [[-1, 0, 0], [0, 0, -1], [1, 0, 0], [0, 0, 1]];

pub fn liquid(chunks: &ChunkStore, journal: &EditJournal, p: [i64; 3]) -> Option<LiquidState> {
    (chunks.block_at(p) == Some(bile::FLUID))
        .then(|| journal.object_state(p)?.liquid)
        .flatten()
}

fn can_occupy(chunks: &ChunkStore, journal: &EditJournal, p: [i64; 3]) -> bool {
    p.iter().all(|v| v.unsigned_abs() <= 990_000)
        && (-128..=356).contains(&p[1])
        && matches!(chunks.block_at(p), Some(0 | bile::FLUID))
        && journal.fluid(p).map_or_else(
            || {
                chunks.surface_column_at(p[0], p[2]).is_none_or(|c| {
                    crate::water::fluid::from_columns(c.water_columns(), p[1]).amount == 0
                })
            },
            |cell| cell.amount == 0,
        )
}

fn preflight(chunks: &ChunkStore, journal: &EditJournal, p: [i64; 3]) -> Result<(), String> {
    let section = global_to_section(p).0;
    if chunks
        .loaded
        .get(&section)
        .is_none_or(|s| s.data.revision() > u64::MAX - 64)
        || journal.revision(section) > u64::MAX - 64
        || journal.appearance_epoch(section) > u64::MAX - 64
        || (-1..=1).any(|z| {
            (-1..=1).any(|y| {
                (-1..=1).any(|x| {
                    section
                        .offset([x, y, z])
                        .map_or(true, |p| journal.mesh_epoch(p) > u64::MAX - 64)
                })
            })
        })
    {
        return Err("Acid update revision exhausted".into());
    }
    Ok(())
}

fn set(
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
    p: [i64; 3],
    state: Option<LiquidState>,
) -> Result<bool, String> {
    let Some(before) = chunks.block_at(p) else {
        return Ok(false);
    };
    preflight(chunks, journal, p)?;
    let material = if state.is_some() { bile::FLUID } else { 0 };
    if before != material
        && !commit_block_edit(
            chunks,
            journal,
            &BlockEditRequest {
                position: p,
                expected_material: before,
                material,
            },
            &BlockPropertiesCatalog::default(),
        )?
    {
        return Ok(false);
    }
    let object = state.map(|liquid| BlockObjectState {
        facing: bile::facing(liquid.volume, liquid.age),
        liquid: Some(liquid),
        ..Default::default()
    });
    commit_object_state(chunks, journal, p, object)?;
    Ok(true)
}

/// The impact cell is loaded air immediately outside the struck solid face.
/// Spit sources last five seconds of active simulation and remain collectable.
pub fn deposit_from_spit(
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
    target: [i64; 3],
) -> Result<bool, String> {
    deposit(chunks, journal, target, bile::SPIT_STEPS)
}
/// A paid bucket creates a persistent reservoir; it never inherits a spit timer.
pub fn deposit_from_bucket(
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
    target: [i64; 3],
) -> Result<bool, String> {
    deposit(chunks, journal, target, 0)
}
fn deposit(
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
    target: [i64; 3],
    lifetime: u8,
) -> Result<bool, String> {
    if !can_occupy(chunks, journal, target)
        || liquid(chunks, journal, target).is_some_and(|s| s.source)
        || journal
            .object_states()
            .filter(|(_, id, _)| *id == bile::FLUID)
            .take(MAX_CELLS)
            .count()
            >= MAX_CELLS
    {
        return Ok(false);
    }
    set(
        chunks,
        journal,
        target,
        Some(LiquidState {
            origin: target,
            volume: bile::CAPACITY,
            age: if lifetime == 0 {
                bile::FINE
            } else {
                bile::FINE | bile::TIMER_8HZ | lifetime
            },
            source: true,
        }),
    )
}
fn refined(mut state: LiquidState) -> LiquidState {
    state.volume = bile::fine_volume(state.volume, state.age);
    if state.age & bile::FINE == 0 {
        state.age = bile::FINE;
    } else if state.source
        && state.age & bile::TIMER_8HZ == 0
        && state.age & bile::LIFETIME_MASK != 0
    {
        let old_ticks = state.age & bile::LIFETIME_MASK;
        state.age = bile::FINE | bile::TIMER_8HZ | (old_ticks * 8).div_ceil(5);
    }
    state
}

pub fn target(
    chunks: &ChunkStore,
    journal: &EditJournal,
    origin: [f64; 3],
    direction: [f64; 3],
    filled: bool,
) -> Option<[i64; 3]> {
    let hit = terra_voxel::blocks::raycast_blocks(origin, direction, 6.0, |p| chunks.block_at(p))?;
    if !filled {
        return liquid(chunks, journal, hit.position)
            .filter(|s| s.source)
            .map(|_| hit.position);
    }
    let p = std::array::from_fn(|i| hit.position[i] + i64::from(hit.normal[i]));
    can_occupy(chunks, journal, p).then_some(p)
}

pub fn take_source(
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
    p: [i64; 3],
) -> Result<bool, String> {
    if !liquid(chunks, journal, p).is_some_and(|s| s.source) {
        return Ok(false);
    }
    set(chunks, journal, p, None)
}

fn in_range(p: [i64; 3], origin: [i64; 3]) -> bool {
    p[0].abs_diff(origin[0]) + p[2].abs_diff(origin[2]) <= bile::RANGE
        && p[1].abs_diff(origin[1]) <= 512
}

#[derive(Default, Debug, PartialEq, Eq)]
pub struct BileStep {
    pub visited: usize,
    pub spread: usize,
    pub removed: usize,
}

/// Find actual wet paths from reservoirs. Saved cells over an unloaded boundary
/// participate in the proof without waking or generating their section.
fn connected_supplies(journal: &EditJournal) -> (usize, HashMap<[i64; 3], [i64; 3]>) {
    let wet: HashMap<_, _> = journal
        .object_states()
        .filter_map(|(p, id, s)| (id == bile::FLUID).then_some((p, s.liquid?)))
        .collect();
    let mut sources: Vec<_> = wet
        .iter()
        .filter_map(|(&p, s)| s.source.then_some(p))
        .collect();
    sources.sort_unstable();
    let mut supply = HashMap::new();
    let mut next: VecDeque<_> = sources.into_iter().map(|p| (p, p)).collect();
    for &(p, origin) in &next {
        supply.insert(p, origin);
    }
    while let Some((p, origin)) = next.pop_front() {
        for d in SIDES.into_iter().chain([[0, -1, 0]]) {
            let q = std::array::from_fn(|i| p[i] + d[i]);
            if !wet.contains_key(&q) || !in_range(q, origin) || supply.contains_key(&q) {
                continue;
            }
            supply.insert(q, origin);
            next.push_back((q, origin));
        }
    }
    (wet.len(), supply)
}

fn bile_cells(journal: &EditJournal) -> Vec<([i64; 3], LiquidState)> {
    journal
        .object_sections()
        .flat_map(|p| journal.object_states_in_section(p))
        .filter_map(|(p, id, s)| (id == bile::FLUID).then_some((p, s.liquid?)))
        .collect()
}

pub fn advance(chunks: &mut ChunkStore, journal: &mut EditJournal) -> Result<BileStep, String> {
    let cells = bile_cells(journal);
    advance_from(chunks, journal, &mut 0, 1, cells)
}

fn advance_from(
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
    cursor: &mut usize,
    elapsed_ticks: u8,
    all: Vec<([i64; 3], LiquidState)>,
) -> Result<BileStep, String> {
    if all.is_empty() {
        return Ok(BileStep::default());
    }
    let mut cells: Vec<_> = all
        .iter()
        .copied()
        .filter(|(p, _)| chunks.block_at(*p) == Some(bile::FLUID))
        .collect();
    cells.sort_unstable_by_key(|(p, _)| *p);
    let length = cells.len();
    if length > 0 {
        cells.rotate_left(*cursor % length);
    }
    let mut result = BileStep::default();
    // Timed sources expire even when unloaded or outside this step’s flow budget.
    for &(p, old) in &all {
        let mut state = refined(old);
        let left = state.age & bile::LIFETIME_MASK;
        if state.source && left > 0 {
            if left <= elapsed_ticks {
                if chunks.block_at(p).is_some() {
                    set(chunks, journal, p, None)?;
                } else {
                    journal.record(p, 0).map_err(|e| e.to_string())?;
                }
                result.removed += 1;
            } else {
                state.age = bile::FINE | bile::TIMER_8HZ | (left - elapsed_ticks);
                if chunks.block_at(p).is_some() {
                    set(chunks, journal, p, Some(state))?;
                } else {
                    journal.set_object_state(
                        p,
                        Some(BlockObjectState {
                            facing: bile::facing(state.volume, state.age),
                            liquid: Some(state),
                            ..Default::default()
                        }),
                    )?;
                }
            }
        }
    }
    let (mut count, mut supply) = connected_supplies(journal);
    for (p, _) in cells.into_iter().take(CELL_BUDGET) {
        result.visited += 1;
        let Some(raw) = liquid(chunks, journal, p) else {
            continue;
        };
        let mut state = refined(raw);
        if !state.source {
            if let Some(&origin) = supply.get(&p) {
                state.origin = origin;
            } else {
                state.volume = state.volume.saturating_sub(8);
                if state.volume == 0 {
                    set(chunks, journal, p, None)?;
                    count = count.saturating_sub(1);
                    result.removed += 1;
                } else {
                    set(chunks, journal, p, Some(state))?;
                }
                continue;
            }
        }
        let down = [p[0], p[1] - 1, p[2]];
        if chunks.block_at(down).is_none() {
            continue;
        }
        let below = liquid(chunks, journal, down).map(refined);
        if !state.source && below.is_some_and(|cell| cell.source) {
            // A falling jet rejoins a reservoir at its surface. Keep the
            // narrow, supplied inlet just as the water solver does.
            let absorbed = terra_core::fluid_surface::discharge(
                state.volume,
                0,
                bile::CAPACITY,
                false,
                true,
                8,
                8,
            );
            if absorbed > 0 {
                state.volume -= absorbed;
                set(chunks, journal, p, Some(state))?;
            }
            continue;
        }
        let falling = can_occupy(chunks, journal, down)
            && in_range(down, state.origin)
            && below.is_none_or(|s| !s.source && s.volume < bile::CAPACITY);
        let directions: Vec<_> = if falling {
            vec![[0, -1, 0]]
        } else {
            SIDES.to_vec()
        };
        for d in directions {
            let q = std::array::from_fn(|i| p[i] + d[i]);
            if !can_occupy(chunks, journal, q) || !in_range(q, state.origin) {
                continue;
            }
            let old = liquid(chunks, journal, q).map(refined);
            if old.is_some_and(|s| s.source) || old.is_none() && count >= MAX_CELLS {
                continue;
            }
            let other = old.map_or(0, |s| s.volume);
            let retained = if falling && !state.source { 8 } else { 0 };
            let transfer = terra_core::fluid_surface::discharge(
                state.volume,
                other,
                bile::CAPACITY - other,
                state.source,
                falling,
                retained,
                8,
            );
            if transfer == 0 {
                continue;
            }
            preflight(chunks, journal, p)?;
            preflight(chunks, journal, q)?;
            let target = LiquidState {
                volume: other + transfer,
                age: bile::FINE,
                source: false,
                origin: supply.get(&q).copied().unwrap_or(state.origin),
            };
            set(chunks, journal, q, Some(target))?;
            supply.insert(q, target.origin);
            if old.is_none() {
                count += 1;
                result.spread += 1;
            }
            if !state.source {
                state.volume -= transfer;
                if state.volume == 0 {
                    set(chunks, journal, p, None)?;
                    count = count.saturating_sub(1);
                    break;
                }
                set(chunks, journal, p, Some(state))?;
            }
        }
    }
    *cursor = if length == 0 {
        0
    } else {
        (*cursor + result.visited) % length
    };
    Ok(result)
}

pub fn tick(
    time: Res<Time>,
    access: Res<WorldAccess>,
    mut chunks: ResMut<ChunkStore>,
    mut journal: ResMut<EditJournal>,
    mut work: Local<(f64, usize)>,
) {
    let now = time.elapsed_secs_f64();
    if !access.authoritative() || now < work.0 {
        return;
    }
    if work.0 == 0.0 {
        work.0 = now;
    }
    let ticks = ((now - work.0) / bile::FLOW_SECONDS).floor() + 1.0;
    work.0 += ticks * bile::FLOW_SECONDS;
    let elapsed_ticks = ticks.min(f64::from(bile::SPIT_STEPS)) as u8;
    let cells = bile_cells(&journal);
    if cells.is_empty() {
        return;
    }
    if let Err(error) = advance_from(&mut chunks, &mut journal, &mut work.1, elapsed_ticks, cells) {
        tracing::error!(%error,"Acid flow rejected");
    }
}

/// Acid uses the published anatomy, including a worm's animated airborne body.
pub fn hitbox_contact(
    body: terra_core::creatures::hitboxes::Hitbox,
    chunks: &ChunkStore,
    journal: &EditJournal,
) -> bool {
    if body
        .center
        .iter()
        .any(|v| !v.is_finite() || v.abs() > 990_000.)
        || body
            .size
            .iter()
            .any(|v| !v.is_finite() || !(0.0..=24.0).contains(v))
    {
        return false;
    }
    let corners = body.corners();
    let minimum = std::array::from_fn::<_, 3, _>(|i| {
        corners.iter().map(|p| p[i]).fold(f64::INFINITY, f64::min) - 0.015
    });
    let maximum = std::array::from_fn::<_, 3, _>(|i| {
        corners
            .iter()
            .map(|p| p[i])
            .fold(f64::NEG_INFINITY, f64::max)
            + 0.015
    });
    let low = minimum.map(|v| v.floor() as i64);
    let high = maximum.map(|v| v.floor() as i64);
    for z in low[2]..=high[2] {
        for y in low[1]..=high[1] {
            for x in low[0]..=high[0] {
                let p = [x, y, z];
                let Some(id) = chunks.block_at(p).map(|id| journal.block(p).unwrap_or(id)) else {
                    continue;
                };
                let height = if id == bile::BLOCK {
                    1.0
                } else if id == bile::FLUID {
                    let samples = |axis: usize| {
                        let base = p[axis] as f64;
                        let low = minimum[axis].clamp(base + 1e-6, base + 1. - 1e-6);
                        let high = maximum[axis].clamp(base + 1e-6, base + 1. - 1e-6);
                        [low, (low + high) * 0.5, high]
                    };
                    let probe_y = body.center[1].clamp(y as f64 + 1e-6, y as f64 + 1. - 1e-6);
                    let mut top = None::<f64>;
                    for px in samples(0) {
                        for pz in samples(2) {
                            if let Some(h) = height_at(chunks, journal, [px, probe_y, pz]) {
                                top = Some(top.map_or(h, |old| old.max(h)));
                            }
                        }
                    }
                    let Some(top) = top else {
                        continue;
                    };
                    top - y as f64
                } else {
                    continue;
                };
                let center = [x as f64 + 0.5, y as f64 + height * 0.5, z as f64 + 0.5];
                let half = [0.515, height * 0.5 + 0.015, 0.515];
                let delta = std::array::from_fn::<_, 3, _>(|i| body.center[i] - center[i]);
                let unit = [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]];
                let dot = |a: [f64; 3], b: [f64; 3]| {
                    a.into_iter().zip(b).map(|(a, b)| a * b).sum::<f64>()
                };
                let axes = unit
                    .into_iter()
                    .chain(body.axes)
                    .chain(unit.into_iter().flat_map(|a| {
                        body.axes.map(|b| {
                            [
                                a[1] * b[2] - a[2] * b[1],
                                a[2] * b[0] - a[0] * b[2],
                                a[0] * b[1] - a[1] * b[0],
                            ]
                        })
                    }));
                if axes.filter(|a| dot(*a, *a) > 1e-10).all(|axis| {
                    dot(delta, axis).abs()
                        <= (0..3)
                            .map(|i| {
                                half[i] * axis[i].abs()
                                    + body.size[i] * 0.5 * dot(body.axes[i], axis).abs()
                            })
                            .sum::<f64>()
                }) {
                    return true;
                }
            }
        }
    }
    false
}

#[cfg(test)]
mod tests;
