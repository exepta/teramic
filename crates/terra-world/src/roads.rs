//! Finite, viscous tar coats prepared paths without becoming a water source.
//!
//! The prepared earth stays in its original cell. Its thin liquid coating has
//! 64 conserved fine volume units per bucket and a persisted two-second drying
//! stage, so unloading a road never refills it or restarts its drying timer.
use crate::{
    edits::{BlockObjectState, EditJournal, LiquidState},
    streaming::{
        BlockEditRequest, ChunkStore, WorldAccess, commit_block_edit, commit_object_state,
    },
};
use bevy::prelude::*;
use terra_core::{block_properties::BlockPropertiesCatalog, roads};
use terra_voxel::global_to_section;

pub use terra_core::roads::PATH;
pub const TICK_SECONDS: f64 = 2.0;
pub const CELL_BUDGET: usize = 1024;
const DIRECTIONS: [[i64; 3]; 4] = [[1, 0, 0], [0, 0, 1], [-1, 0, 0], [0, 0, -1]];

pub(crate) fn coating(
    position: [i64; 3],
    material: u16,
    volume: u8,
    journal: &EditJournal,
    mut block_at: impl FnMut([i64; 3]) -> Option<u16>,
) -> terra_core::block_shapes::ShapeBox {
    roads::tar_coating(material, volume, |side| {
        let mut p = position;
        p[side / 2] += if side % 2 == 0 { -1 } else { 1 };
        let id = block_at(p);
        let volume = if id == Some(roads::WET_TAR) {
            let state = journal.object_state(p).unwrap_or_default();
            state
                .liquid
                .map_or(roads::tar_amount(state.facing) * 8, |liquid| liquid.volume)
        } else {
            0
        };
        (id, volume)
    })
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct TarStep {
    pub visited: usize,
    pub flowed: usize,
    pub dried: usize,
}

fn exposed(chunks: &ChunkStore, journal: &EditJournal, position: [i64; 3]) -> bool {
    if position.iter().any(|p| p.unsigned_abs() > 990_000) {
        return false;
    }
    let above = [position[0], position[1] + 1, position[2]];
    chunks
        .block_at(above)
        .is_some_and(|id| id == 0 || terra_core::street::is_arc(id))
        && journal.crop_at(position).is_none()
        && [position, above].into_iter().all(|p| {
            journal
                .fluid(p)
                .unwrap_or_else(|| {
                    chunks
                        .surface_column_at(p[0], p[2])
                        .map_or_else(Default::default, |surface| {
                            crate::water::fluid::from_columns(surface.water_columns(), p[1])
                        })
                })
                .amount
                == 0
        })
        && chunks
            .loaded
            .get(&global_to_section(position).0)
            .is_some_and(|section| {
                !section
                    .props
                    .iter()
                    .any(|prop| !prop.removed && crate::props::placement::support(prop) == position)
            })
}

fn open_edge(
    chunks: &ChunkStore,
    journal: &EditJournal,
    p: [i64; 3],
    q: [i64; 3],
    direction: [i64; 3],
) -> bool {
    [(p, direction), (q, direction.map(|d| -d))]
        .into_iter()
        .all(|([x, y, z], side)| {
            let above = [x, y + 1, z];
            match chunks.block_at(above) {
                Some(0) => true,
                Some(id) if terra_core::street::is_arc(id) => {
                    let facing = journal.object_state(above).map_or(0, |s| s.facing);
                    terra_core::street::arc_opens_to(id, facing, side)
                }
                _ => false,
            }
        })
}

pub fn can_make_path(chunks: &ChunkStore, journal: &EditJournal, position: [i64; 3]) -> bool {
    chunks.block_at(position).is_some_and(roads::is_path_base) && exposed(chunks, journal, position)
}

pub fn can_pour(chunks: &ChunkStore, journal: &EditJournal, position: [i64; 3]) -> bool {
    chunks.block_at(position) == Some(PATH) && exposed(chunks, journal, position)
}

/// Select the visible top face. Props in front of the ground must be cleared
/// first, and a side click never prepares or coats a hidden neighbour.
pub fn target(
    chunks: &ChunkStore,
    journal: &EditJournal,
    properties: &BlockPropertiesCatalog,
    origin: [f64; 3],
    direction: [f64; 3],
    pouring: bool,
) -> Option<[i64; 3]> {
    let (_, selected) = crate::targeting::pick_world_with_context(
        chunks,
        journal,
        properties,
        origin,
        direction,
        crate::props::PropPose::Rest,
    );
    let crate::targeting::WorldTarget::Block(hit) = selected? else {
        return None;
    };
    (hit.normal == [0, 1, 0]
        && if pouring {
            can_pour(chunks, journal, hit.position)
        } else {
            can_make_path(chunks, journal, hit.position)
        })
    .then_some(hit.position)
}

pub fn make_path(
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
    position: [i64; 3],
) -> Result<bool, String> {
    if !can_make_path(chunks, journal, position) {
        return Ok(false);
    }
    commit_block_edit(
        chunks,
        journal,
        &BlockEditRequest {
            position,
            expected_material: chunks.block_at(position).unwrap(),
            material: PATH,
        },
        &BlockPropertiesCatalog::default(),
    )
}

pub fn pour(
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
    position: [i64; 3],
) -> Result<bool, String> {
    if !can_pour(chunks, journal, position) {
        return Ok(false);
    }
    preflight(chunks, journal, &[position])?;
    set_film(
        chunks,
        journal,
        position,
        roads::WET_TAR,
        LiquidState {
            origin: position,
            volume: roads::TAR_FINE_UNITS,
            age: 0,
            source: true,
        },
    )
}

fn preflight(
    chunks: &ChunkStore,
    journal: &EditJournal,
    positions: &[[i64; 3]],
) -> Result<(), String> {
    // One flow changes two cells and their halos. Check ample shared headroom
    // before changing either half of the transfer, including section borders.
    for &position in positions {
        let section = global_to_section(position).0;
        if chunks
            .loaded
            .get(&section)
            .is_none_or(|loaded| loaded.data.revision() > u64::MAX - 4096)
            || journal.revision(section) > u64::MAX - 4096
            || journal.fluid_epoch(section) > u64::MAX - 4096
            || journal.appearance_epoch(section) > u64::MAX - 4096
        {
            return Err("Tar update exceeds section revision capacity".into());
        }
        for dz in -1..=1 {
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let neighbor = section.offset([dx, dy, dz]).map_err(|e| e.to_string())?;
                    if journal.mesh_epoch(neighbor) > u64::MAX - 4096
                        || journal.water_mesh_epoch(neighbor) > u64::MAX - 4096
                    {
                        return Err("Tar update exceeds mesh revision capacity".into());
                    }
                }
            }
        }
    }
    Ok(())
}

fn film(position: [i64; 3], state: BlockObjectState) -> LiquidState {
    state.liquid.unwrap_or(LiquidState {
        origin: position,
        volume: roads::tar_amount(state.facing) * 8,
        age: roads::tar_stage(state.facing),
        source: true,
    })
}

#[cfg(test)]
fn set_coating(
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
    p: [i64; 3],
    material: u16,
    amount: u8,
    stage: u8,
) -> Result<bool, String> {
    set_film(
        chunks,
        journal,
        p,
        material,
        LiquidState {
            origin: p,
            volume: amount * 8,
            age: stage,
            source: true,
        },
    )
}

fn set_film(
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
    p: [i64; 3],
    material: u16,
    liquid: LiquidState,
) -> Result<bool, String> {
    let Some(before) = chunks.block_at(p) else {
        return Ok(false);
    };
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
    commit_object_state(
        chunks,
        journal,
        p,
        Some(BlockObjectState {
            facing: roads::tar_state(liquid.volume.div_ceil(8), liquid.age),
            liquid: Some(liquid),
            ..Default::default()
        }),
    )?;
    Ok(true)
}

/// Advance two seconds using ten small, paid neighbor-flux steps. Fine conserved
/// units let a single bucket form a level film across the four-block radius.
pub fn advance(chunks: &mut ChunkStore, journal: &mut EditJournal) -> Result<TarStep, String> {
    let mut cursor = 0;
    let mut total = TarStep::default();
    for step in 0..10 {
        let cells = wet_tar_cells(chunks, journal);
        if cells.is_empty() {
            break;
        }
        let part = advance_from(chunks, journal, &mut cursor, step == 9, cells)?;
        total.visited += part.visited;
        total.flowed += part.flowed;
        total.dried += part.dried;
    }
    Ok(total)
}

fn wet_tar_cells(chunks: &ChunkStore, journal: &EditJournal) -> Vec<[i64; 3]> {
    journal
        .object_sections()
        .flat_map(|p| journal.object_states_in_section(p))
        .filter(|(p, id, _)| *id == roads::WET_TAR && chunks.block_at(*p) == Some(roads::WET_TAR))
        .map(|(p, _, _)| p)
        .collect()
}

fn advance_from(
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
    cursor: &mut usize,
    drying: bool,
    mut positions: Vec<[i64; 3]>,
) -> Result<TarStep, String> {
    use std::collections::{BTreeMap, BTreeSet, VecDeque};
    positions.sort_unstable();
    let count = positions.len();
    if count > 0 {
        positions.rotate_left(*cursor % count);
    }
    let mut result = TarStep::default();
    let mut processed = BTreeSet::new();
    for start in positions {
        if result.visited >= CELL_BUDGET {
            break;
        }
        if processed.contains(&start) {
            continue;
        }
        let mut pool = BTreeMap::new();
        let mut queue = VecDeque::from([start]);
        while let Some(p) = queue.pop_front() {
            if processed.contains(&p) || pool.contains_key(&p) {
                continue;
            }
            if pool.len() + result.visited >= CELL_BUDGET {
                break;
            }
            if chunks.block_at(p) != Some(roads::WET_TAR) {
                continue;
            }
            let Some(state) = journal.object_state(p) else {
                continue;
            };
            pool.insert(p, film(p, state));
            if exposed(chunks, journal, p) {
                for d in DIRECTIONS {
                    let q = std::array::from_fn(|i| p[i] + d[i]);
                    if exposed(chunks, journal, q) && open_edge(chunks, journal, p, q, d) {
                        queue.push_back(q);
                    }
                }
            }
        }
        if pool.is_empty() {
            continue;
        }
        let age = pool
            .values()
            .map(|s| s.age)
            .max()
            .unwrap()
            .saturating_add(u8::from(drying))
            .min(31);
        let origins: BTreeSet<_> = pool.values().map(|s| s.origin).collect();
        let before = pool.len();
        let active: Vec<_> = pool.keys().copied().collect();
        if age < 31 {
            for p in active {
                if !exposed(chunks, journal, p) {
                    continue;
                }
                let turn =
                    (p[0].wrapping_add(p[2]).wrapping_add(*cursor as i64)).rem_euclid(4) as usize;
                for side in 0..4 {
                    let d = DIRECTIONS[(side + turn) % 4];
                    let q = std::array::from_fn(|i| p[i] + d[i]);
                    if !exposed(chunks, journal, q)
                        || !matches!(chunks.block_at(q), Some(PATH | roads::WET_TAR))
                        || !open_edge(chunks, journal, p, q, d)
                    {
                        continue;
                    }
                    if !origins
                        .iter()
                        .any(|o| q[0].abs_diff(o[0]) + q[2].abs_diff(o[2]) <= roads::TAR_RANGE)
                    {
                        continue;
                    }
                    let own = pool[&p].volume;
                    let other = pool.get(&q).map_or(0, |s| s.volume);
                    if own.saturating_sub(other) < 2 || pool.len() + result.visited >= CELL_BUDGET {
                        continue;
                    }
                    let origin = *origins
                        .iter()
                        .filter(|o| q[0].abs_diff(o[0]) + q[2].abs_diff(o[2]) <= roads::TAR_RANGE)
                        .min_by_key(|o| q[0].abs_diff(o[0]) + q[2].abs_diff(o[2]))
                        .unwrap();
                    pool.get_mut(&p).unwrap().volume -= 1;
                    pool.entry(q)
                        .or_insert(LiquidState {
                            origin,
                            volume: 0,
                            age,
                            source: false,
                        })
                        .volume += 1;
                }
            }
        }
        // Project one unit of hydraulic pressure through this connected film.
        // Intermediate edge fluxes cancel, so no water teleports across dry
        // paths: a newly wetted target must touch the existing film. This also
        // avoids a permanent staircase caused by rounding every local head to
        // one of the 64 stored heights.
        if age < 31 {
            let mut candidates = BTreeSet::new();
            for &p in pool.keys() {
                if !exposed(chunks, journal, p) {
                    continue;
                }
                candidates.insert(p);
                for d in DIRECTIONS {
                    let q = std::array::from_fn(|i| p[i] + d[i]);
                    if exposed(chunks, journal, q)
                        && matches!(chunks.block_at(q), Some(PATH | roads::WET_TAR))
                        && open_edge(chunks, journal, p, q, d)
                        && origins
                            .iter()
                            .any(|o| q[0].abs_diff(o[0]) + q[2].abs_diff(o[2]) <= roads::TAR_RANGE)
                    {
                        candidates.insert(q);
                    }
                }
            }
            let target = candidates
                .into_iter()
                .min_by_key(|p| (pool.get(p).map_or(0, |s| s.volume), *p));
            let donor = pool
                .iter()
                .filter(|(p, _)| exposed(chunks, journal, **p))
                .max_by_key(|(_, s)| s.volume)
                .map(|(&p, s)| (p, s.volume));
            if let (Some(q), Some((p, own))) = (target, donor)
                && own.saturating_sub(pool.get(&q).map_or(0, |s| s.volume)) >= 2
                && (pool.contains_key(&q) || pool.len() + result.visited < CELL_BUDGET)
            {
                let origin = *origins
                    .iter()
                    .filter(|o| q[0].abs_diff(o[0]) + q[2].abs_diff(o[2]) <= roads::TAR_RANGE)
                    .min_by_key(|o| q[0].abs_diff(o[0]) + q[2].abs_diff(o[2]))
                    .unwrap();
                pool.get_mut(&p).unwrap().volume -= 1;
                pool.entry(q)
                    .or_insert(LiquidState {
                        origin,
                        volume: 0,
                        age,
                        source: false,
                    })
                    .volume += 1;
            }
        }
        let cells: Vec<_> = pool.keys().copied().collect();
        // Preflight the whole connected edit before publishing its first cell.
        preflight(chunks, journal, &cells)?;
        let length = pool.len();
        for (p, mut state) in pool {
            processed.insert(p);
            result.visited += 1;
            state.age = if age == 31 { 0 } else { age };
            set_film(
                chunks,
                journal,
                p,
                if age == 31 {
                    roads::DRIED_TAR
                } else {
                    roads::WET_TAR
                },
                state,
            )?;
            result.dried += usize::from(age == 31);
        }
        result.flowed += length - before;
    }
    *cursor = if count == 0 {
        0
    } else {
        (*cursor + result.visited + 1) % count
    };
    Ok(result)
}

pub(crate) fn tick(
    time: Res<Time>,
    access: Res<WorldAccess>,
    mut chunks: ResMut<ChunkStore>,
    mut journal: ResMut<EditJournal>,
    mut work: Local<(f64, usize, f64)>,
) {
    let now = time.elapsed_secs_f64();
    if !access.authoritative() || now < work.0 {
        return;
    }
    work.0 = now + 0.2;
    let drying = now - work.2 >= TICK_SECONDS;
    if drying {
        work.2 = now;
    }
    let cells = wet_tar_cells(&chunks, &journal);
    if cells.is_empty() {
        work.1 = 0;
        return;
    }
    if let Err(error) = advance_from(&mut chunks, &mut journal, &mut work.1, drying, cells) {
        tracing::error!(%error, "Tar simulation update rejected");
    }
}

#[cfg(test)]
mod tests;
