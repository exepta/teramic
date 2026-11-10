//! Budgeted transfers between real FluidCells, never a second terrain-fluid solver.
#[cfg(feature = "bevy")]
mod intake;
#[cfg(feature = "bevy")]
mod network;
#[cfg(feature = "bevy")]
mod rain;
#[cfg(feature = "bevy")]
mod reservoirs;
#[cfg(feature = "bevy")]
pub use rain::{RAIN_UNITS_PER_SECOND, RainRuntime, collect_rain};
#[cfg(feature = "bevy")]
mod basin_water;
#[cfg(feature = "bevy")]
pub use basin_water::{basin_water_column, basin_water_level};
mod shapes;
use crate::water::FluidCell;
#[cfg(feature = "bevy")]
use crate::{edits::EditJournal, streaming::ChunkStore};
#[cfg(feature = "bevy")]
pub use network::{Runtime, WorkCounts, advance};
#[cfg(feature = "bevy")]
pub use reservoirs::basin_shape;
use serde::{Deserialize, Serialize};
pub use shapes::{connection_mask, plumbing_boxes};
use std::sync::Arc;
use terra_core::plumbing::{self as core, BasinShape, Cell};

pub const MAX_NODES: usize = 8192;

#[cfg(feature = "bevy")]
pub fn toggle(
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
    position: Cell,
    expected: u16,
    facing: u8,
) -> Result<bool, String> {
    let Some(next) = core::toggled(expected, facing) else {
        return Ok(false);
    };
    if chunks.block_at(position) != Some(expected)
        || journal
            .object_state(position)
            .is_none_or(|state| state.facing != facing)
    {
        return Ok(false);
    }
    let mut state = journal.object_state(position).unwrap();
    state.facing = next;
    let affected = journal.set_object_state(position, Some(state))?;
    chunks.invalidate_sections(affected);
    Ok(true)
}

#[cfg(feature = "bevy")]
pub fn adjust_faucet(
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
    position: Cell,
    facing: u8,
    closing: bool,
) -> Result<bool, String> {
    if !core::valid_facing(core::FAUCET, facing)
        || chunks.block_at(position) != Some(core::FAUCET)
        || journal
            .object_state(position)
            .is_none_or(|state| state.facing != facing)
    {
        return Ok(false);
    }
    let mut state = journal.object_state(position).unwrap();
    state.facing = core::adjusted_faucet(facing, closing);
    if state.facing == facing {
        return Ok(true);
    }
    let affected = journal.set_object_state(position, Some(state))?;
    chunks.invalidate_sections(affected);
    Ok(true)
}

#[cfg(all(test, feature = "bevy"))]
mod tests;

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlumbingState {
    pub water: FluidCell,
    /// Water recovered during disassembly; pressure is vented before storage.
    #[serde(skip)]
    pub train_reserve: Option<terra_core::train_power::SteamState>,
    pub pump_credit: f32,
    pub basin: Option<BasinShape>,
    #[serde(skip)]
    pub intake: Option<core::IntakeActivity>,
    /// Confirmed transfer during the latest hydraulic step, including vertical
    /// movement and water consumed in the same step. Never persisted as water.
    #[serde(skip)]
    pub flowing: bool,
}
impl PlumbingState {
    pub fn valid(&self, material: u16) -> bool {
        core::is_part(material)
            && self.train_reserve.is_none_or(|reserve| {
                material == core::TRAIN_DOCK
                    && reserve.valid()
                    && reserve.water > 0.0
                    && reserve.pressure_bar == 0.0
            })
            && self.water.validate()
            && !self.water.source
            && self.pump_credit.is_finite()
            && (0.0..=8.0).contains(&self.pump_credit)
            && self
                .basin
                .is_none_or(|shape| material == core::BASIN && shape.valid())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum PressureSource {
    Pump(Cell, i8),
    Basin(BasinShape),
    Natural(Cell, u16),
    Stored(Cell),
}
#[derive(Debug, Clone)]
pub(crate) struct SupplyToken {
    pub epoch: u64,
    pub source: PressureSource,
    pub route: Arc<Vec<(Cell, u16, u8)>>,
    pub draw: Arc<Vec<Cell>>,
    pub pump_directions: Vec<(Cell, i8)>,
}

#[cfg(feature = "bevy")]
pub fn tap_available(
    chunks: &ChunkStore,
    journal: &EditJournal,
    position: Cell,
    units: u8,
) -> bool {
    supply_available(chunks, journal, position, units, core::FAUCET)
}

/// Terminal devices share route validation and finite withdrawal, without discharging into terrain.
#[cfg(feature = "bevy")]
pub fn infuser_available(
    chunks: &ChunkStore,
    journal: &EditJournal,
    position: Cell,
    units: u8,
) -> bool {
    supply_available(chunks, journal, position, units, core::FLUID_INFUSER)
}

#[cfg(feature = "bevy")]
fn supply_available(
    chunks: &ChunkStore,
    journal: &EditJournal,
    position: Cell,
    units: u8,
    terminal: u16,
) -> bool {
    if units == 0
        || chunks.block_at(position) != Some(terminal)
        || !journal
            .object_state(position)
            .is_some_and(|state| core::open(terminal, state.facing))
        || units > 8
    {
        return false;
    }
    let Some(token) = journal.plumbing_supply_at(position) else {
        return false;
    };
    if token
        .route
        .windows(2)
        .any(|edge| !crate::kingdom_access::can_transport(chunks, journal, edge[0].0, edge[1].0))
        || token
            .draw
            .iter()
            .any(|&p| !crate::kingdom_access::can_transport(chunks, journal, p, position))
        || drawable(journal, token) < u16::from(units)
    {
        return false;
    }
    if token.epoch != journal.plumbing_epoch()
        || token.route.len() > core::MAX_COMPONENT
        || token.pump_directions.iter().any(|(cell, sign)| {
            !journal.kinetic_state(*cell).running()
                || journal.kinetic_state(*cell).rpm.signum() as i8 != *sign
        })
        || token.route.iter().any(|(cell, material, facing)| {
            chunks.block_at(*cell) != Some(*material)
                || journal
                    .object_state(*cell)
                    .is_none_or(|state| state.facing != *facing)
                || !core::open(*material, *facing)
                || *material == core::PUMP && !journal.kinetic_state(*cell).running()
        })
    {
        return false;
    }
    match token.source {
        PressureSource::Pump(pump, sign) => {
            journal.kinetic_state(pump).running()
                && journal.kinetic_state(pump).rpm.signum() as i8 == sign
        }
        // The source quantity was already withdrawn into finite pipe storage.
        // A small pool can now be empty while that admitted water remains usable.
        PressureSource::Natural(cell, material) => chunks.block_at(cell) == Some(material),
        // This is already admitted finite water. The route and draw list above
        // still have to be live, and withdrawing it never replenishes storage.
        PressureSource::Stored(cell) => chunks.block_at(cell).is_some_and(core::is_part),
        PressureSource::Basin(shape) => {
            let amount: u32 = shape
                .cells()
                .map(|cell| {
                    u32::from(
                        journal
                            .plumbing_at(cell)
                            .map_or(0, |state| state.water.amount),
                    )
                })
                .sum();
            let head = shape.origin[1] as f64
                + f64::from(amount) / (f64::from(shape.size[0]) * f64::from(shape.size[2]) * 8.0);
            shape.cells().all(|cell| {
                chunks.block_at(cell) == Some(core::BASIN)
                    && journal
                        .plumbing_at(cell)
                        .is_some_and(|state| state.basin == Some(shape))
            }) && head > position[1] as f64 + 0.275
        }
    }
}

#[cfg(feature = "bevy")]
fn drawable(journal: &EditJournal, token: &SupplyToken) -> u16 {
    token
        .draw
        .iter()
        .map(|&cell| {
            u16::from(
                journal
                    .plumbing_at(cell)
                    .map_or(0, |state| state.water.amount),
            )
        })
        .sum::<u16>()
        .min(8)
}

#[cfg(feature = "bevy")]
#[derive(Debug, Clone, Copy)]
pub struct VisualData {
    pub amount: u16,
    pub capacity: u16,
    pub open: bool,
    pub flowing: bool,
    pub pump_rpm: f32,
    pub basin: Option<BasinShape>,
    pub supplied: bool,
    pub intake: Option<core::IntakeActivity>,
}
#[cfg(feature = "bevy")]
pub fn visual_data(
    chunks: &ChunkStore,
    journal: &EditJournal,
    position: Cell,
) -> Option<VisualData> {
    let material = chunks.block_at(position)?;
    if !core::is_part(material) {
        return None;
    }
    let state = journal.plumbing_at(position)?;
    let amount = if matches!(material, core::FAUCET | core::FLUID_INFUSER) {
        journal
            .plumbing_supply_at(position)
            .filter(|_| supply_available(chunks, journal, position, 1, material))
            .map_or(0, |token| drawable(journal, token))
    } else {
        state.basin.map_or(u16::from(state.water.amount), |shape| {
            shape
                .cells()
                .map(|cell| {
                    u16::from(
                        journal
                            .plumbing_at(cell)
                            .map_or(0, |state| state.water.amount),
                    )
                })
                .sum()
        })
    };
    Some(VisualData {
        amount,
        capacity: state.basin.map_or(8, BasinShape::capacity),
        open: core::open(material, journal.object_state(position)?.facing),
        flowing: state.flowing,
        pump_rpm: if material == core::PUMP {
            journal.kinetic_state(position).rpm
        } else {
            0.
        },
        basin: state.basin,
        supplied: matches!(material, core::FAUCET | core::FLUID_INFUSER)
            && supply_available(chunks, journal, position, 1, material),
        intake: state.intake,
    })
}

#[cfg(feature = "bevy")]
pub fn take_tap(
    chunks: &ChunkStore,
    journal: &mut EditJournal,
    position: Cell,
    units: u8,
) -> Result<bool, String> {
    if !tap_available(chunks, journal, position, units) {
        return Ok(false);
    }
    take_supply(journal, position, units)
}

#[cfg(feature = "bevy")]
pub fn take_infuser(
    chunks: &ChunkStore,
    journal: &mut EditJournal,
    position: Cell,
    units: u8,
) -> Result<bool, String> {
    if !infuser_available(chunks, journal, position, units) {
        return Ok(false);
    }
    take_supply(journal, position, units)
}

#[cfg(feature = "bevy")]
fn take_supply(journal: &mut EditJournal, position: Cell, units: u8) -> Result<bool, String> {
    let draw = journal.plumbing_supply_at(position).unwrap().draw.clone();
    let mut remaining = units;
    let mut updates = Vec::new();
    for cell in
        std::iter::once(position).chain(draw.iter().copied().filter(|&cell| cell != position))
    {
        let Some(mut state) = journal.plumbing_at(cell).cloned() else {
            continue;
        };
        let amount = remaining.min(state.water.amount);
        if amount == 0 {
            continue;
        }
        state.water.amount -= amount;
        state.flowing = true;
        remaining -= amount;
        if state.water.amount == 0 {
            state.water.flow = [0; 2];
        }
        updates.push((cell, state));
        if remaining == 0 {
            break;
        }
    }
    if remaining != 0 {
        return Ok(false);
    }
    // A successful draw crosses the validated route even when intermediate
    // buffers are empty at publication time. Mark only the path from the actual
    // withdrawn buffer to the terminal, not upstream of that buffer. Preserve
    // confirmed transit until the next hydraulic step.
    let route = journal.plumbing_supply_at(position).unwrap().route.clone();
    let furthest = updates
        .iter()
        .map(|(drawn, _)| {
            route
                .iter()
                .position(|(cell, _, _)| cell == drawn)
                .unwrap_or(route.len().saturating_sub(1))
        })
        .max()
        .unwrap_or(0);
    for (cell, _, _) in route.iter().take(furthest + 1) {
        if updates.iter().any(|(updated, _)| updated == cell) {
            continue;
        }
        if let Some(mut state) = journal.plumbing_at(*cell).cloned()
            && !state.flowing
        {
            state.flowing = true;
            updates.push((*cell, state));
        }
    }
    journal
        .gameplay()
        .revision
        .checked_add(updates.len() as u64)
        .ok_or("Faucet transaction revision exhausted")?;
    for (cell, state) in updates {
        journal.set_plumbing(cell, state)?;
    }
    Ok(true)
}

/// A poured bucket is finite. Preflight the complete reservoir before changing any module.
#[cfg(feature = "bevy")]
pub fn fill_basin(
    chunks: &ChunkStore,
    journal: &mut EditJournal,
    position: Cell,
    units: u8,
) -> Result<bool, String> {
    let Some(shape) = journal.plumbing_at(position).and_then(|state| state.basin) else {
        return Ok(false);
    };
    if units == 0
        || reservoirs::basin_shape(chunks, journal, position) != Some(shape)
        || !shape.cells().all(|cell| {
            chunks.block_at(cell) == Some(core::BASIN)
                && crate::kingdom_access::can_transport(chunks, journal, position, cell)
                && journal
                    .plumbing_at(cell)
                    .is_some_and(|state| state.basin == Some(shape))
        })
    {
        return Ok(false);
    }
    let total: u16 = shape
        .cells()
        .map(|cell| u16::from(journal.plumbing_at(cell).unwrap().water.amount))
        .sum();
    let total = total + u16::from(units);
    if total > shape.capacity() {
        return Ok(false);
    }
    let updates: Vec<_> = shape
        .levelled_cells(total)
        .filter_map(|(cell, amount)| {
            let mut state = journal.plumbing_at(cell).unwrap().clone();
            if state.water.amount == amount {
                None
            } else {
                state.water.amount = amount;
                Some((cell, state))
            }
        })
        .collect();
    journal
        .gameplay()
        .revision
        .checked_add(updates.len() as u64)
        .ok_or("Basin fill revision exhausted")?;
    for (cell, state) in updates {
        journal.set_plumbing(cell, state)?;
    }
    Ok(true)
}
