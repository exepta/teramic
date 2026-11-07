use super::EditJournal;
use crate::plumbing::{MAX_NODES, PlumbingState, SupplyToken};
use terra_core::plumbing::{self as core, Cell};
use terra_voxel::{SectionPos, global_to_section};

impl EditJournal {
    /// Replace only confirmed reservoir samples on a client replica. Aggregate
    /// network quantities become the same per-module state used by server water
    /// probes; this is not a local edit or inventory/gameplay transaction.
    pub fn replace_confirmed_basins(
        &mut self,
        basins: impl IntoIterator<Item = (core::BasinShape, u16)>,
    ) -> Result<(), String> {
        let mut shapes = std::collections::BTreeMap::new();
        for (shape, total) in basins {
            if !shape.valid() || total > shape.capacity() {
                return Err("Invalid confirmed basin quantity".into());
            }
            if shapes.insert(shape, total).is_some_and(|old| old != total) {
                return Err("Inconsistent confirmed basin quantities".into());
            }
        }
        let mut next = std::collections::BTreeMap::new();
        for (shape, total) in shapes {
            for (cell, amount) in shape.levelled_cells(total) {
                let state = PlumbingState {
                    basin: Some(shape),
                    water: crate::water::FluidCell {
                        amount,
                        ..Default::default()
                    },
                    ..Default::default()
                };
                if next.get(&cell).is_some_and(|old| old != &state) {
                    return Err("Overlapping or inconsistent confirmed basins".into());
                }
                next.insert(cell, state);
                if next.len() > MAX_NODES {
                    return Err("Confirmed basin budget exceeded".into());
                }
            }
        }
        let other = self
            .plumbing
            .values()
            .filter(|state| state.basin.is_none())
            .count();
        if other + next.len() > MAX_NODES {
            return Err("Confirmed plumbing budget exceeded".into());
        }
        self.plumbing.retain(|_, state| state.basin.is_none());
        self.plumbing.extend(next);
        self.plumbing_sections.clear();
        for &cell in self.plumbing.keys() {
            self.plumbing_sections
                .entry(global_to_section(cell).0)
                .or_default()
                .insert(cell);
        }
        Ok(())
    }
    pub fn plumbing(&self) -> impl Iterator<Item = (Cell, &PlumbingState)> + '_ {
        self.plumbing.iter().map(|(&cell, state)| (cell, state))
    }
    pub fn plumbing_at(&self, position: Cell) -> Option<&PlumbingState> {
        self.plumbing.get(&position)
    }
    pub fn plumbing_positions(&self) -> impl Iterator<Item = Cell> + '_ {
        self.plumbing.keys().copied()
    }
    pub fn plumbing_after(&self, after: Option<Cell>) -> impl Iterator<Item = Cell> + '_ {
        use std::ops::Bound::{Excluded, Unbounded};
        self.plumbing
            .range((after.map_or(Unbounded, Excluded), Unbounded))
            .map(|(&cell, _)| cell)
    }
    pub fn plumbing_in_section(&self, section: SectionPos) -> impl Iterator<Item = Cell> + '_ {
        self.plumbing_sections
            .get(&section)
            .into_iter()
            .flat_map(|cells| cells.iter().copied())
    }
    pub fn plumbing_slot_available(&self, position: Cell) -> bool {
        self.plumbing.contains_key(&position) || self.plumbing.len() < MAX_NODES
    }
    pub fn set_plumbing(&mut self, position: Cell, state: PlumbingState) -> Result<bool, String> {
        let material = self.block(position).ok_or("Unrecorded plumbing node")?;
        if !state.valid(material)
            || state
                .intake
                .is_some_and(|activity| !activity.valid_for(position))
            || !self
                .object_state(position)
                .is_some_and(|pose| pose.valid_for(position, material))
            || state.basin.is_some_and(|shape| !shape.contains(position))
            || !self.plumbing_slot_available(position)
        {
            return Err("Invalid, orphaned or over-capacity plumbing state".into());
        }
        if self.plumbing_at(position) == Some(&state) {
            return Ok(false);
        }
        let revision = self
            .gameplay()
            .revision
            .checked_add(1)
            .ok_or("Plumbing revision overflow")?;
        self.plumbing.insert(position, state);
        self.plumbing_sections
            .entry(global_to_section(position).0)
            .or_default()
            .insert(position);
        self.gameplay_mut().revision = revision;
        Ok(true)
    }
    pub(crate) fn invalidate_plumbing_supply(&mut self) {
        self.plumbing_epoch = self.plumbing_epoch.wrapping_add(1);
    }
    pub fn plumbing_epoch(&self) -> u64 {
        self.plumbing_epoch
    }
    pub(crate) fn plumbing_supply_at(&self, position: Cell) -> Option<&SupplyToken> {
        self.plumbing_supply.get(&position)
    }
    pub(crate) fn set_plumbing_supply(&mut self, position: Cell, token: SupplyToken) {
        if self
            .block(position)
            .is_some_and(|m| matches!(m, core::FAUCET | core::FLUID_INFUSER))
            && (self.plumbing_supply.contains_key(&position)
                || self.plumbing_supply.len() < MAX_NODES)
        {
            self.plumbing_supply.insert(position, token);
        }
    }
    pub(crate) fn clear_plumbing_supply_at(&mut self, position: Cell) {
        self.plumbing_supply.remove(&position);
    }
    pub(super) fn remove_plumbing(&mut self, position: Cell) {
        self.plumbing.remove(&position);
        self.plumbing_supply.remove(&position);
        let owner = global_to_section(position).0;
        if let Some(cells) = self.plumbing_sections.get_mut(&owner) {
            cells.remove(&position);
            if cells.is_empty() {
                self.plumbing_sections.remove(&owner);
            }
        }
        self.invalidate_plumbing_supply();
    }
}
