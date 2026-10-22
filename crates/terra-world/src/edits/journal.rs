use crate::environment::{SurfaceCondition, SurfaceConditionError, SurfaceSnapshot};
use crate::generation::sections::BlockOverrides;
use crate::props::{PropId, WorldProp};
use crate::water::FluidCell;
use std::collections::{HashMap, HashSet};
use terra_voxel::blocks::BlockError;
use terra_voxel::{SectionPos, global_to_section};

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SectionDelta {
    pub position: SectionPos,
    pub revision: u64,
    pub prop_epoch: u64,
    pub appearance_epoch: u64,
    #[serde(default)]
    pub fluid_epoch: u64,
    pub blocks: Vec<([u8; 3], u16)>,
    pub removed_props: Vec<PropId>,
    #[serde(default)]
    pub placed_props: Vec<WorldProp>,
    pub conditions: Vec<([u8; 3], SurfaceCondition)>,
    #[serde(default)]
    pub fluids: Vec<([u8; 3], FluidCell)>,
    #[serde(default)]
    pub objects: Vec<([u8; 3], super::BlockObjectState)>,
}

#[derive(Default, Clone)]
pub(super) struct SectionEdits {
    pub(super) objects: HashMap<[usize; 3], super::BlockObjectState>,
    revision: u64,
    pub(super) blocks: HashMap<[usize; 3], u16>,
    prop_epoch: u64,
    removed_props: HashSet<PropId>,
    placed_props: HashMap<PropId, WorldProp>,
    appearance_epoch: u64,
    surface_conditions: SurfaceSnapshot,
    pub(super) fluid_epoch: u64,
    pub(super) fluids: HashMap<[usize; 3], FluidCell>,
}
/// Session-local authoritative edits survive section eviction. Disk/network storage is separate.
#[derive(Default, Clone)]
#[cfg_attr(feature = "bevy", derive(bevy::prelude::Resource))]
pub struct EditJournal {
    /// World metadata travels with the existing atomic save snapshot.
    pub clock: Option<terra_core::WorldClock>,
    pub weather: Option<terra_core::WorldWeather>,
    pub world_rules: Option<terra_core::WorldRuleSettings>,
    player_state_revision: u64,
    gameplay: terra_core::GameplayState,
    pub(super) containers: HashMap<[i64; 3], terra_core::Inventory>,
    pub(super) furnaces: HashMap<[i64; 3], terra_core::furnace::FurnaceState>,
    pub(super) furniture_order: HashMap<[i64; 3], u64>,
    pub(super) next_furniture_order: u64,
    pub(super) sections: HashMap<SectionPos, SectionEdits>,
    mesh_epochs: HashMap<SectionPos, u64>,
    pub(super) water_mesh_epochs: HashMap<SectionPos, u64>,
    pub(super) fluid_active: std::collections::VecDeque<[i64; 3]>,
    pub(super) fluid_queued: HashSet<[i64; 3]>,
    #[cfg(feature = "bevy")]
    pub(crate) grass_candidates: std::collections::VecDeque<[i64; 3]>,
    #[cfg(feature = "bevy")]
    pub(crate) grass_queued: HashSet<[i64; 3]>,
    #[cfg(feature = "bevy")]
    pub(super) fluid_sleeping: HashMap<SectionPos, HashSet<[i64; 3]>>,
}
impl EditJournal {
    /// Metadata changes must not invalidate terrain, collision or fluid meshes.
    pub fn player_state_revision(&self) -> u64 {
        self.player_state_revision
    }
    pub fn update_player_stats(
        &mut self,
        player: terra_core::PlayerUuid,
        stats: terra_core::PlayerStats,
    ) -> bool {
        if player.is_nil()
            || !stats.survival.valid()
            || (!self.gameplay.player_stats.contains_key(&player)
                && self.gameplay.player_stats.len() >= 4096)
            || self.gameplay.player_stats.get(&player) == Some(&stats)
        {
            return false;
        }
        self.gameplay.player_stats.insert(player, stats);
        self.player_state_revision = self.player_state_revision.saturating_add(1);
        true
    }
    pub fn gameplay(&self) -> &terra_core::GameplayState {
        &self.gameplay
    }
    pub fn gameplay_mut(&mut self) -> &mut terra_core::GameplayState {
        &mut self.gameplay
    }
    pub fn import_gameplay(&mut self, state: terra_core::GameplayState) -> Result<(), String> {
        if !state.valid() {
            return Err("Invalid inventory or resource-drop snapshot".into());
        }
        self.gameplay = state;
        Ok(())
    }
    pub fn changed_sections(&self) -> impl Iterator<Item = SectionPos> + '_ {
        self.sections.keys().copied()
    }

    pub fn section_delta(&self, position: SectionPos) -> SectionDelta {
        let Some(edits) = self.sections.get(&position) else {
            return SectionDelta {
                position,
                ..Default::default()
            };
        };
        let mut blocks: Vec<_> = edits
            .blocks
            .iter()
            .map(|(&p, &id)| (p.map(|n| n as u8), id))
            .collect();
        blocks.sort_unstable_by_key(|(p, _)| *p);
        let mut removed_props: Vec<_> = edits.removed_props.iter().copied().collect();
        removed_props.sort_unstable();
        let mut conditions: Vec<_> = edits
            .surface_conditions
            .iter()
            .map(|(&p, &c)| (p.map(|n| n as u8), c))
            .collect();
        conditions.sort_unstable_by_key(|(p, _)| *p);
        let mut fluids: Vec<_> = edits
            .fluids
            .iter()
            .map(|(&p, &cell)| (p.map(|n| n as u8), cell))
            .collect();
        fluids.sort_unstable_by_key(|(p, _)| *p);
        SectionDelta {
            position,
            revision: edits.revision,
            prop_epoch: edits.prop_epoch,
            appearance_epoch: edits.appearance_epoch,
            fluid_epoch: edits.fluid_epoch,
            blocks,
            removed_props,
            placed_props: {
                let mut props: Vec<_> = edits.placed_props.values().copied().collect();
                props.sort_unstable_by_key(|p| p.id);
                props
            },
            conditions,
            fluids,
            objects: {
                let mut values: Vec<_> = edits
                    .objects
                    .iter()
                    .map(|(p, state)| (p.map(|n| n as u8), *state))
                    .collect();
                values.sort_unstable_by_key(|(p, _)| *p);
                values
            },
        }
    }

    /// Replace a confirmed full section delta, keeping local worker invalidation
    /// epochs independent from the authority's revision counters.
    pub fn import_delta(&mut self, delta: SectionDelta) -> Result<Vec<SectionPos>, String> {
        self.import_delta_with_simulation(delta, true)
    }

    /// Replicas retain confirmed data and render invalidations, but never run
    /// authoritative fluid or grass simulation work derived from those edits.
    #[cfg(feature = "bevy")]
    pub(crate) fn import_replica_delta(
        &mut self,
        delta: SectionDelta,
    ) -> Result<Vec<SectionPos>, String> {
        self.import_delta_with_simulation(delta, false)
    }

    fn import_delta_with_simulation(
        &mut self,
        delta: SectionDelta,
        wake_simulation: bool,
    ) -> Result<Vec<SectionPos>, String> {
        delta.position.origin().map_err(|e| e.to_string())?;
        if delta.blocks.len() > 4096
            || delta.objects.len() > 4096
            || delta.objects.iter().any(|(p, state)| {
                p.iter().any(|v| *v >= 16)
                    || !delta
                        .position
                        .global_block(p.map(usize::from))
                        .is_ok_and(|position| state.valid_at(position))
                    || !delta.blocks.iter().any(|(cell, material)| {
                        cell == p
                            && delta
                                .position
                                .global_block(p.map(usize::from))
                                .is_ok_and(|position| state.valid_for(position, *material))
                    })
            })
            || delta
                .objects
                .iter()
                .map(|(p, _)| p)
                .collect::<HashSet<_>>()
                .len()
                != delta.objects.len()
            || delta.conditions.len() > 4096
            || delta.removed_props.len() > 4096
            || delta.placed_props.len() > 256
            || delta
                .placed_props
                .iter()
                .any(|p| !p.valid() || p.section != delta.position || p.removed)
            || delta
                .placed_props
                .iter()
                .map(|p| p.id)
                .collect::<HashSet<_>>()
                .len()
                != delta.placed_props.len()
            || delta.fluids.len() > 4096
            || delta.blocks.iter().any(|(p, _)| p.iter().any(|&v| v >= 16))
            || delta
                .conditions
                .iter()
                .any(|(p, c)| p.iter().any(|&v| v >= 16) || c.validate().is_err())
            || delta
                .fluids
                .iter()
                .any(|(p, cell)| p.iter().any(|&v| v >= 16) || !cell.validate() || cell.amount == 0)
            || delta
                .fluids
                .iter()
                .map(|(p, _)| p)
                .collect::<HashSet<_>>()
                .len()
                != delta.fluids.len()
        {
            return Err("Invalid section delta".into());
        }
        let p = delta.position;
        if delta.revision < self.revision(p)
            || delta.prop_epoch < self.prop_epoch(p)
            || delta.appearance_epoch < self.appearance_epoch(p)
            || delta.fluid_epoch < self.fluid_epoch(p)
        {
            return Ok(Vec::new());
        }
        let blocks: HashMap<_, _> = delta
            .blocks
            .into_iter()
            .map(|(p, id)| (p.map(usize::from), id))
            .collect();
        let objects: HashMap<_, _> = delta
            .objects
            .into_iter()
            .map(|(p, state)| (p.map(usize::from), state))
            .collect();
        let old = self.sections.get(&p);
        let cells: HashSet<_> = blocks
            .keys()
            .copied()
            .chain(old.into_iter().flat_map(|e| e.blocks.keys().copied()))
            .collect();
        let changed: Vec<_> = cells
            .into_iter()
            .filter(|local| {
                old.and_then(|e| e.blocks.get(local)) != blocks.get(local)
                    || old.and_then(|e| e.objects.get(local)) != objects.get(local)
            })
            .collect();
        let mut affected = HashSet::new();
        let fluids: HashMap<_, _> = delta
            .fluids
            .into_iter()
            .map(|(p, cell)| (p.map(usize::from), cell))
            .collect();
        let fluid_cells: HashSet<_> = fluids
            .keys()
            .copied()
            .chain(old.into_iter().flat_map(|e| e.fluids.keys().copied()))
            .collect();
        let fluid_changes: Vec<_> = fluid_cells
            .into_iter()
            .filter(|local| old.and_then(|e| e.fluids.get(local)) != fluids.get(local))
            .collect();
        for local in changed {
            let global = p.global_block(local).map_err(|e| e.to_string())?;
            affected.extend(
                self.record_with_order(
                    global,
                    blocks.get(&local).copied().unwrap_or(0),
                    false,
                    wake_simulation,
                )
                .map_err(|e| e.to_string())?,
            );
        }
        for local in fluid_changes {
            let global = p.global_block(local).map_err(|e| e.to_string())?;
            self.invalidate_water(global).map_err(|e| e.to_string())?;
        }
        self.sections.insert(
            p,
            SectionEdits {
                objects,
                revision: delta.revision,
                blocks,
                prop_epoch: delta.prop_epoch,
                removed_props: delta.removed_props.into_iter().collect(),
                placed_props: delta.placed_props.into_iter().map(|p| (p.id, p)).collect(),
                appearance_epoch: delta.appearance_epoch,
                fluid_epoch: delta.fluid_epoch,
                fluids,
                surface_conditions: delta
                    .conditions
                    .into_iter()
                    .map(|(p, c)| (p.map(usize::from), c))
                    .collect(),
            },
        );
        Ok(affected.into_iter().collect())
    }
    pub fn revision(&self, p: SectionPos) -> u64 {
        self.sections.get(&p).map_or(0, |s| s.revision)
    }
    pub fn mesh_epoch(&self, p: SectionPos) -> u64 {
        self.mesh_epochs.get(&p).copied().unwrap_or(0)
    }
    pub fn edited_blocks(&self) -> usize {
        self.sections.values().map(|s| s.blocks.len()).sum()
    }
    /// A saved air override is distinct from an unmodified generated cell.
    pub fn block(&self, global: [i64; 3]) -> Option<u16> {
        let (owner, local) = global_to_section(global);
        self.sections.get(&owner)?.blocks.get(&local).copied()
    }
    pub fn appearance_epoch(&self, section: SectionPos) -> u64 {
        self.sections
            .get(&section)
            .map_or(0, |edits| edits.appearance_epoch)
    }
    #[cfg(feature = "bevy")]
    pub(crate) fn blocks_in_section(
        &self,
        section: SectionPos,
    ) -> impl Iterator<Item = ([usize; 3], u16)> + '_ {
        self.sections
            .get(&section)
            .into_iter()
            .flat_map(|s| s.blocks.iter().map(|(&p, &id)| (p, id)))
    }
    pub fn surface_condition(&self, global: [i64; 3]) -> SurfaceCondition {
        let (owner, local) = global_to_section(global);
        self.sections
            .get(&owner)
            .and_then(|edits| edits.surface_conditions.get(&local))
            .copied()
            .unwrap_or_default()
    }
    pub fn surface_snapshot(&self, section: SectionPos) -> SurfaceSnapshot {
        self.sections
            .get(&section)
            .map(|edits| edits.surface_conditions.clone())
            .unwrap_or_default()
    }
    pub fn surface_condition_count(&self) -> usize {
        self.sections
            .values()
            .map(|s| s.surface_conditions.len())
            .sum()
    }
    pub fn active_fires(&self) -> impl Iterator<Item = ([i64; 3], SurfaceCondition)> + '_ {
        self.sections.iter().flat_map(|(&section, edits)| {
            edits
                .surface_conditions
                .iter()
                .filter_map(move |(&local, &condition)| {
                    (condition.fire > 0.0)
                        .then(|| {
                            section
                                .global_block(local)
                                .ok()
                                .map(|position| (position, condition))
                        })
                        .flatten()
                })
        })
    }
    /// Does not invalidate voxel generation, colliders, water or logical props.
    pub fn set_surface_condition(
        &mut self,
        global: [i64; 3],
        condition: SurfaceCondition,
    ) -> Result<bool, SurfaceConditionError> {
        condition.validate()?;
        if self.surface_condition(global) == condition {
            return Ok(false);
        }
        let (owner, local) = global_to_section(global);
        let epoch = self
            .appearance_epoch(owner)
            .checked_add(1)
            .ok_or(SurfaceConditionError::RevisionOverflow)?;
        let edits = self.sections.entry(owner).or_default();
        if condition == SurfaceCondition::default() {
            edits.surface_conditions.remove(&local);
        } else {
            edits.surface_conditions.insert(local, condition);
        }
        edits.appearance_epoch = epoch;
        Ok(true)
    }
    pub fn prop_epoch(&self, section: SectionPos) -> u64 {
        self.sections
            .get(&section)
            .map_or(0, |edits| edits.prop_epoch)
    }
    pub fn removed_props(&self) -> usize {
        self.sections
            .values()
            .map(|edits| edits.removed_props.len())
            .sum()
    }
    pub fn prop_removed(&self, section: SectionPos, id: PropId) -> bool {
        self.sections
            .get(&section)
            .is_some_and(|edits| edits.removed_props.contains(&id))
    }
    /// Validated by the loaded logical prop list. Does not invalidate terrain/water meshes.
    pub fn remove_prop(&mut self, section: SectionPos, id: PropId) -> Result<bool, BlockError> {
        if self.prop_removed(section, id) {
            return Ok(false);
        }
        let epoch = self
            .prop_epoch(section)
            .checked_add(1)
            .ok_or(BlockError::RevisionOverflow)?;
        let edits = self.sections.entry(section).or_default();
        edits.removed_props.insert(id);
        edits.placed_props.remove(&id);
        edits.prop_epoch = epoch;
        Ok(true)
    }
    /// Apply after receiving a worker result, including removals made while it was in flight.
    pub fn apply_prop_removals(&self, section: SectionPos, props: &mut Vec<WorldProp>) {
        if let Some(edits) = self.sections.get(&section) {
            for placed in edits.placed_props.values() {
                if let Some(existing) = props.iter_mut().find(|p| p.id == placed.id) {
                    *existing = *placed;
                } else {
                    props.push(*placed);
                }
            }
            for prop in props.iter_mut() {
                prop.removed = edits.removed_props.contains(&prop.id);
            }
            if !edits.placed_props.is_empty() || !edits.removed_props.is_empty() {
                // Group anchors derive from live identities, not journal insertion order.
                crate::props::placement::reflow(props);
            }
        }
    }
    pub fn place_prop(&mut self, prop: WorldProp) -> Result<bool, BlockError> {
        if !prop.valid() || prop.removed || self.prop_removed(prop.section, prop.id) {
            return Ok(false);
        }
        let edits = self.sections.entry(prop.section).or_default();
        if edits.placed_props.len() >= 256
            || edits.placed_props.len() + edits.removed_props.len() >= 3840
            || edits.placed_props.contains_key(&prop.id)
        {
            return Ok(false);
        }
        let epoch = edits
            .prop_epoch
            .checked_add(1)
            .ok_or(BlockError::RevisionOverflow)?;
        edits.placed_props.insert(prop.id, prop);
        edits.prop_epoch = epoch;
        Ok(true)
    }

    /// Presentation growth updates retain the original identity and save record.
    #[cfg(feature = "bevy")]
    pub(crate) fn update_placed_prop(&mut self, prop: WorldProp) -> Result<bool, BlockError> {
        if !prop.valid() || prop.removed {
            return Ok(false);
        }
        let Some(edits) = self.sections.get_mut(&prop.section) else {
            return Ok(false);
        };
        let Some(previous) = edits.placed_props.get_mut(&prop.id) else {
            return Ok(false);
        };
        if *previous == prop {
            return Ok(false);
        }
        let epoch = edits
            .prop_epoch
            .checked_add(1)
            .ok_or(BlockError::RevisionOverflow)?;
        *previous = prop;
        edits.prop_epoch = epoch;
        Ok(true)
    }
    /// Caller must validate the loaded block and avoid recording no-op writes.
    pub fn record(
        &mut self,
        global: [i64; 3],
        material: u16,
    ) -> Result<Vec<SectionPos>, BlockError> {
        self.record_with_order(global, material, true, true)
    }
    fn record_with_order(
        &mut self,
        global: [i64; 3],
        material: u16,
        track_furniture: bool,
        wake_simulation: bool,
    ) -> Result<Vec<SectionPos>, BlockError> {
        let (owner, local) = global_to_section(global);
        let mut affected = Vec::with_capacity(8);
        // Corner AO, like shoreline vertices, reads diagonal halo cells as well.
        // Only sections whose one-cell halo contains the edit need a rebuild.
        let offsets = local.map(|v| match v {
            0 => -1,
            15 => 1,
            _ => 0,
        });
        for z in [0, offsets[2]]
            .into_iter()
            .take(if offsets[2] == 0 { 1 } else { 2 })
        {
            for y in [0, offsets[1]]
                .into_iter()
                .take(if offsets[1] == 0 { 1 } else { 2 })
            {
                for x in [0, offsets[0]]
                    .into_iter()
                    .take(if offsets[0] == 0 { 1 } else { 2 })
                {
                    affected.push(owner.offset([x, y, z])?);
                }
            }
        }
        let revision = self
            .revision(owner)
            .checked_add(1)
            .ok_or(BlockError::RevisionOverflow)?;
        for &p in &affected {
            self.mesh_epoch(p)
                .checked_add(1)
                .ok_or(BlockError::RevisionOverflow)?;
        }
        let appearance_epoch = if self
            .sections
            .get(&owner)
            .is_some_and(|edits| edits.surface_conditions.contains_key(&local))
        {
            Some(
                self.appearance_epoch(owner)
                    .checked_add(1)
                    .ok_or(BlockError::RevisionOverflow)?,
            )
        } else {
            None
        };
        let new_furniture =
            track_furniture && matches!(material, 28..=29) && self.block(global) != Some(material);
        let furniture_order = if new_furniture {
            Some(
                self.next_furniture_order
                    .checked_add(1)
                    .ok_or(BlockError::RevisionOverflow)?,
            )
        } else {
            None
        };
        let edits = self.sections.entry(owner).or_default();
        let previous = edits.blocks.insert(local, material);
        if !matches!(material, 28..=29 | 40 | 59..=97)
            || previous.is_some_and(|old| old != material)
        {
            edits.objects.remove(&local);
        }
        if !matches!(material, 28..=29) || previous.is_some_and(|previous| previous != material) {
            self.furniture_order.remove(&global);
        }
        if let Some(order) = furniture_order {
            self.furniture_order.insert(global, order);
            self.next_furniture_order = order;
        }
        edits.revision = revision;
        if let Some(epoch) = appearance_epoch {
            edits.surface_conditions.remove(&local);
            edits.appearance_epoch = epoch;
        }
        for &p in &affected {
            *self.mesh_epochs.entry(p).or_default() += 1;
        }
        self.invalidate_water_geometry(global)?;
        if material == terra_voxel::MaterialId::FluidWater as u16 {
            self.write_fluid_with_wake(
                global,
                FluidCell {
                    amount: crate::water::FLUID_CAPACITY,
                    flow: [0, 0],
                    source: true,
                },
                wake_simulation,
            )?;
        } else if crate::water::space::partial(material)
            && previous == Some(terra_voxel::MaterialId::FluidWater as u16)
            && let Some(cell) = self.fluid(global)
        {
            // Replacing the explicit source removes its replenishment, while
            // remaining water can drain through the construction's free space.
            self.write_fluid_with_wake(
                global,
                FluidCell {
                    source: false,
                    ..cell
                },
                wake_simulation,
            )?;
        } else if !(50..=55).contains(&material)
            && !crate::water::space::partial(material)
            && (material != 0 || previous == Some(terra_voxel::MaterialId::FluidWater as u16))
        {
            self.write_fluid_with_wake(global, FluidCell::default(), wake_simulation)?;
        }
        if wake_simulation {
            self.wake_fluid_neighbors(global);
            #[cfg(feature = "bevy")]
            crate::growth::schedule(self, global);
        }
        Ok(affected)
    }
    /// Capture face padding and vertical mineral clearance, never a full world copy.
    pub fn snapshot(&self, p: SectionPos) -> Result<BlockOverrides, BlockError> {
        let origin = p.origin()?;
        let mut lower = [0; 3];
        let mut upper = [0; 3];
        for i in 0..3 {
            lower[i] = origin[i]
                .checked_sub(if i == 1 { 32 } else { 1 })
                .ok_or(BlockError::CoordinateOverflow)?;
            upper[i] = origin[i]
                .checked_add(if i == 1 { 47 } else { 16 })
                .ok_or(BlockError::CoordinateOverflow)?;
        }
        let mut snapshot = BlockOverrides::new();
        for dz in -1..=1 {
            for dy in -2..=2 {
                for dx in -1..=1 {
                    let neighbor = p.offset([dx, dy, dz])?;
                    if let Some(section) = self.sections.get(&neighbor) {
                        for (&local, &material) in &section.blocks {
                            let global = neighbor.global_block(local)?;
                            if (0..3).all(|i| (lower[i]..=upper[i]).contains(&global[i])) {
                                snapshot.insert(global, material);
                            }
                        }
                    }
                }
            }
        }
        Ok(snapshot)
    }
}
