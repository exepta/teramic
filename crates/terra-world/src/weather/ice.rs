//! Budgeted seasonal crust on still, exposed natural water. Player ice/tanks stay authored.
use crate::{
    edits::{BlockObjectState, EditJournal},
    streaming::{BlockEditRequest, ChunkStore, WorldAccess, WorldTerrain, commit_block_edit},
};
use bevy::prelude::*;
use std::collections::{BTreeMap, BTreeSet};
use terra_core::{WorldClock, WorldWeather, block_properties::BlockPropertiesCatalog};
use terra_voxel::{MaterialId, global_to_section};

const MAX_PROGRESS: usize = 16_384;
const EDIT_BUDGET: usize = 4;
const FREEZE_SECONDS: f64 = 45.;
const THAW_SECONDS: f64 = 30.;

#[derive(Clone, Copy)]
struct Exposure {
    last: f64,
    seconds: f64,
    freezing: bool,
}
#[derive(Default)]
pub(crate) struct IceWork {
    next: f64,
    cursor: usize,
    progress: BTreeMap<[i64; 3], Exposure>,
}

#[derive(Resource, Default, Debug)]
pub struct IceStatus {
    pub tracked_cells: usize,
    pub scanned_cells: usize,
    pub edits_this_tick: usize,
    pub freezes: u64,
    pub thaws: u64,
}

#[derive(Clone, Copy)]
struct Change {
    position: [i64; 3],
    material: u16,
    water: u8,
}
impl Change {
    fn freezing(self) -> bool {
        self.material == 0
    }
}

fn exposed(chunks: &ChunkStore, journal: &EditJournal, p: [i64; 3], height: f64) -> bool {
    let top = (height.ceil() as i64 + 2).max(p[1] + 16);
    top - p[1] <= 64
        && (p[1] + 1..=top).all(|y| {
            let q = [p[0], y, p[2]];
            chunks.block_at(q).is_some_and(|id| {
                id == 0 || journal.block(q).is_none() && MaterialId::FOLIAGE.contains(&id)
            })
        })
}

fn candidate(
    chunks: &ChunkStore,
    journal: &EditJournal,
    x: i64,
    z: i64,
    temperature: f32,
) -> Option<Change> {
    let surface = chunks.surface_column_at(x, z)?;
    let level = surface.water_level?;
    let p = [x, level.ceil() as i64 - 1, z];
    let material = chunks.block_at(p)?;
    if material == 49 {
        let source = journal.object_state(p)?.frozen_water()?;
        // Thaw even when the player subsequently put a roof above the ice.
        return (temperature >= 2.).then_some(Change {
            position: p,
            material,
            water: source.amount,
        });
    }
    if material != 0 || temperature > -2. || !exposed(chunks, journal, p, surface.height) {
        return None;
    }
    let natural = crate::water::fluid::from_columns(surface.water_columns(), p[1]);
    let actual = journal.fluid(p).unwrap_or(natural);
    (natural.amount > 0 && actual == natural && actual.source && actual.flow == [0, 0]).then_some(
        Change {
            position: p,
            material,
            water: actual.amount,
        },
    )
}

fn apply(
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
    properties: &BlockPropertiesCatalog,
    change: Change,
) -> Result<bool, String> {
    // Metadata must fit after the block commit. Natural ice never owns an inventory.
    let owner = global_to_section(change.position).0;
    if journal.revision(owner) > u64::MAX - 8 || journal.fluid_epoch(owner) > u64::MAX - 8 {
        return Err("Seasonal ice revision exhausted".into());
    }
    for dz in -1..=1 {
        for dy in -1..=1 {
            for dx in -1..=1 {
                let p = owner.offset([dx, dy, dz]).map_err(|e| e.to_string())?;
                if journal.mesh_epoch(p) > u64::MAX - 8
                    || journal.water_mesh_epoch(p) > u64::MAX - 8
                {
                    return Err("Seasonal ice mesh revision exhausted".into());
                }
            }
        }
    }
    if !commit_block_edit(
        chunks,
        journal,
        &BlockEditRequest {
            position: change.position,
            expected_material: change.material,
            material: if change.freezing() { 49 } else { 0 },
        },
        properties,
    )? {
        return Ok(false);
    }
    if change.freezing() {
        journal.set_object_state(
            change.position,
            Some(BlockObjectState {
                facing: change.water,
                ..Default::default()
            }),
        )?;
    }
    // Melting/breaking the marked ice restores the source in EditJournal::record.
    Ok(true)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn tick(
    time: Res<Time>,
    access: Res<WorldAccess>,
    clock: Res<WorldClock>,
    weather: Res<WorldWeather>,
    terrain: Res<WorldTerrain>,
    mut chunks: ResMut<ChunkStore>,
    mut journal: ResMut<EditJournal>,
    properties: Res<BlockPropertiesCatalog>,
    mut status: ResMut<IceStatus>,
    mut work: Local<IceWork>,
) {
    if !access.authoritative() || !terrain.0.config().water {
        return;
    }
    if clock.paused {
        work.progress.clear();
        return;
    }
    let now = time.elapsed_secs_f64();
    if now < work.next {
        return;
    }
    work.next = now + 0.5;
    work.progress.retain(|_, p| now - p.last < 600.);
    status.edits_this_tick = 0;
    status.scanned_cells = 0;
    // One prepared column per half-second. No chunk requests or generation.
    let columns: BTreeSet<_> = chunks
        .loaded
        .keys()
        .map(|p| [p.x, p.z])
        .filter(|p| chunks.prepared_column(*p).is_some())
        .collect();
    let Some(&column) = columns.iter().nth(work.cursor % columns.len().max(1)) else {
        return;
    };
    work.cursor = work.cursor.wrapping_add(1);
    let season = clock.season(terrain.0.config().seasons);
    for z in 0..16 {
        for x in 0..16 {
            let x = column[0] * 16 + x;
            let z = column[1] * 16 + z;
            status.scanned_cells += 1;
            let Some(surface) = chunks.surface_column_at(x, z) else {
                continue;
            };
            let local =
                super::from_surface(&weather, surface, terrain.0.config().sea_level, season);
            let Some(change) = candidate(&chunks, &journal, x, z, local.temperature) else {
                // A warm/cold reversal discards accumulated dwell time.
                if let Some(level) = surface.water_level {
                    work.progress.remove(&[x, level.ceil() as i64 - 1, z]);
                }
                continue;
            };
            if work.progress.len() >= MAX_PROGRESS && !work.progress.contains_key(&change.position)
            {
                continue;
            }
            let exposure = work.progress.entry(change.position).or_insert(Exposure {
                last: now,
                seconds: 0.,
                freezing: change.freezing(),
            });
            if exposure.freezing != change.freezing() {
                exposure.seconds = 0.;
                exposure.freezing = change.freezing();
            } else {
                exposure.seconds += (now - exposure.last).clamp(0., 120.);
            }
            exposure.last = now;
            if exposure.seconds
                < if change.freezing() {
                    FREEZE_SECONDS
                } else {
                    THAW_SECONDS
                }
                || status.edits_this_tick >= EDIT_BUDGET
            {
                continue;
            }
            match apply(&mut chunks, &mut journal, &properties, change) {
                Ok(true) => {
                    work.progress.remove(&change.position);
                    status.edits_this_tick += 1;
                    if change.freezing() {
                        status.freezes += 1;
                    } else {
                        status.thaws += 1;
                    }
                }
                Ok(false) => {
                    work.progress.remove(&change.position);
                }
                Err(error) => tracing::warn!(%error, "Seasonal ice update rejected"),
            }
        }
    }
    status.tracked_cells = work.progress.len();
}

#[cfg(test)]
mod tests;
