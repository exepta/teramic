//! Stage every touched sparse section before publishing parts, cargo and vehicle ownership.
use super::EditJournal;
use crate::{streaming::ChunkStore, trains::AssemblyPlan};
use std::collections::{BTreeMap, BTreeSet};
use terra_core::{train_parts, trains::TrainError};
use terra_voxel::global_to_section;
impl EditJournal {
    pub(crate) fn commit_train_plan(
        &mut self,
        chunks: &mut ChunkStore,
        plan: &AssemblyPlan,
    ) -> Result<u64, TrainError> {
        if self.gameplay.trains.revision != plan.registry_revision
            || self.gameplay.revision == u64::MAX
        {
            return Err(TrainError::Stale);
        }
        let fresh = if plan.assembling {
            crate::trains::plan_assembly_with_properties(
                chunks,
                self,
                plan.actor,
                plan.station,
                plan.start,
                plan.vehicle.kind,
                &plan.properties,
            )?
        } else {
            crate::trains::plan_dismantle(chunks, self, plan.actor, plan.station, plan.vehicle.id)?
        };
        if fresh.source_motion != plan.source_motion
            || fresh.vehicle != plan.vehicle
            || fresh.parts.len() != plan.parts.len()
            || fresh.parts.iter().zip(&plan.parts).any(|(a, b)| {
                a.cell != b.cell
                    || a.expected != b.expected
                    || a.state != b.state
                    || a.cargo != b.cargo
            })
        {
            return Err(TrainError::Stale);
        }
        let mut registry = self.gameplay.trains.clone();
        let id = if plan.assembling {
            registry.insert(plan.vehicle.clone())?
        } else {
            registry.remove(plan.actor, plan.vehicle.id)?.id
        };
        let owners: BTreeSet<_> = plan
            .parts
            .iter()
            .map(|part| global_to_section(part.cell).0)
            .collect();
        if owners.len() > 64 {
            return Err(TrainError::Limit);
        }
        let mut staged = Self::default();
        let mut halo = BTreeSet::new();
        let mut blocks = BTreeMap::new();
        for owner in &owners {
            if let Some(section) = self.sections.get(owner) {
                staged.sections.insert(*owner, section.clone());
            }
            blocks.insert(
                *owner,
                chunks
                    .loaded
                    .get(owner)
                    .ok_or(TrainError::Incomplete)?
                    .data
                    .clone(),
            );
            for x in -1..=1 {
                for y in -1..=1 {
                    for z in -1..=1 {
                        let neighbor = owner.offset([x, y, z]).map_err(|_| TrainError::Overflow)?;
                        halo.insert(neighbor);
                        staged
                            .mesh_epochs
                            .insert(neighbor, self.mesh_epoch(neighbor));
                        staged
                            .water_mesh_epochs
                            .insert(neighbor, self.water_mesh_epoch(neighbor));
                    }
                }
            }
        }
        for (index, part) in plan.parts.iter().enumerate() {
            let material = if plan.assembling {
                0
            } else {
                plan.vehicle.parts[index].material
            };
            let (owner, local) = global_to_section(part.cell);
            staged
                .record(part.cell, material)
                .map_err(|_| TrainError::Overflow)?;
            staged
                .set_object_state(part.cell, if plan.assembling { None } else { part.state })
                .map_err(|_| TrainError::Invalid)?;
            if plan.assembling {
                staged
                    .set_fluid(part.cell, crate::water::FluidCell::default())
                    .map_err(|_| TrainError::Invalid)?;
            }
            blocks
                .get_mut(&owner)
                .unwrap()
                .set_block(local, material)
                .map_err(|_| TrainError::Invalid)?;
        }
        if !plan.assembling {
            for (index, part) in plan.parts.iter().enumerate() {
                if let Some(payload) = plan.vehicle.payloads.get(&(index as u16)) {
                    staged.restore_train_payload(part.cell, payload)?;
                }
                if let Some(cargo) = &part.cargo {
                    staged
                        .set_container(part.cell, cargo.clone())
                        .map_err(|_| TrainError::Invalid)?;
                }
            }
        }
        // Freed machine slots may have been filled elsewhere while mounted.
        // Validate against the live maps, not the isolated staging journal.
        macro_rules! capacity {
            ($field:ident, $limit:expr) => {{
                let removed = plan
                    .parts
                    .iter()
                    .filter(|part| self.$field.contains_key(&part.cell))
                    .count();
                let restored = staged.$field.len();
                if self
                    .$field
                    .len()
                    .saturating_sub(removed)
                    .saturating_add(restored)
                    > $limit
                {
                    return Err(TrainError::Limit);
                }
            }};
        }
        capacity!(plumbing, crate::plumbing::MAX_NODES);
        capacity!(cauldrons, crate::edits::hearths::MAX_HEARTHS);
        capacity!(campfires, crate::edits::hearths::MAX_HEARTHS);
        capacity!(attached_campfires, crate::edits::hearths::MAX_HEARTHS);
        // The registry, all block/pose/epoch writes, cargo and loaded payloads
        // have passed their fallible work. Publication cannot split ownership.
        for (owner, section) in std::mem::take(&mut staged.sections) {
            self.sections.insert(owner, section);
        }
        for owner in &owners {
            self.refresh_object_section(*owner);
        }
        self.mesh_epochs
            .extend(std::mem::take(&mut staged.mesh_epochs));
        self.water_mesh_epochs
            .extend(std::mem::take(&mut staged.water_mesh_epochs));
        for (owner, data) in blocks {
            chunks.loaded.get_mut(&owner).unwrap().data = data;
        }
        for part in &plan.parts {
            self.publish_train_runtime(&mut staged, part.cell);
            self.wake_fluid_neighbors(part.cell);
        }
        if plan
            .vehicle
            .parts
            .iter()
            .any(|part| terra_core::plumbing::is_part(part.material))
        {
            self.invalidate_plumbing_supply();
        }
        self.gameplay.trains = registry;
        self.gameplay.revision += 1;
        for section in halo {
            chunks.mark_dirty_if_wanted(section);
        }
        Ok(id)
    }
}

/// State of a mounted block is private and paused. World ports, automation,
/// fire and fluid simulation see air until the complete object is dismantled.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
struct FrozenCell {
    pose: Option<crate::edits::BlockObjectState>,
    tint: Option<[u8; 3]>,
    condition: crate::environment::SurfaceCondition,
    inventory: Option<terra_core::Inventory>,
    furnace: Option<terra_core::furnace::FurnaceState>,
    crusher: Option<terra_core::crusher::CrusherState>,
    mixer: Option<terra_core::mixer::MixerState>,
    cauldron: Option<terra_core::brewing::CauldronState>,
    campfire: Option<terra_core::campfire::CampfireState>,
    attached_fire: Option<terra_core::campfire::AttachedCampfire>,
    plumbing: Option<crate::plumbing::PlumbingState>,
    filter: Option<terra_core::logistics::FilterConfig>,
    distributor: Option<terra_core::logistics::DistributorConfig>,
    random_tunnel: bool,
    belt: Option<crate::edits::BeltOwner>,
    fluid: Option<crate::water::FluidCell>,
    furniture_order: Option<u64>,
}
impl FrozenCell {
    fn decode(payload: &[u8]) -> Result<Self, TrainError> {
        if payload.len() > terra_core::trains::MAX_PAYLOAD_BYTES {
            return Err(TrainError::Limit);
        }
        serde_json::from_slice(payload).map_err(|_| TrainError::Invalid)
    }
}
impl EditJournal {
    pub(crate) fn train_payload(&self, cell: [i64; 3]) -> Result<Vec<u8>, TrainError> {
        let frozen = FrozenCell {
            pose: self.object_state(cell),
            tint: self.tint_at(cell),
            condition: self.surface_condition(cell),
            inventory: (!matches!(self.block(cell), Some(29 | train_parts::CARGO | 218)))
                .then(|| self.containers.get(&cell).cloned())
                .flatten(),
            furnace: self.furnaces.get(&cell).cloned(),
            crusher: self.crushers.get(&cell).cloned(),
            mixer: self.mixers.get(&cell).cloned(),
            cauldron: self.cauldrons.get(&cell).cloned(),
            campfire: self.campfires.get(&cell).cloned(),
            attached_fire: self.attached_campfires.get(&cell).cloned(),
            plumbing: self.plumbing.get(&cell).cloned(),
            filter: self.logistics_filters.get(&cell).copied(),
            distributor: self.distributors.get(&cell).copied(),
            random_tunnel: self.random_tunnels.contains(&cell),
            belt: self.belt_owner(cell),
            fluid: self.fluid(cell),
            furniture_order: self.furniture_order.get(&cell).copied(),
        };
        let bytes = serde_json::to_vec(&frozen).map_err(|_| TrainError::Invalid)?;
        if bytes.len() > terra_core::trains::MAX_PAYLOAD_BYTES {
            return Err(TrainError::Limit);
        }
        Ok(bytes)
    }
    pub(crate) fn train_payload_pose(
        payload: &[u8],
    ) -> Result<Option<crate::edits::BlockObjectState>, TrainError> {
        Ok(FrozenCell::decode(payload)?.pose)
    }
    pub(crate) fn rebase_train_payload(
        payload: &[u8],
        material: u16,
        new_facing: u8,
        source: [i64; 3],
        target: [i64; 3],
        turns: u8,
        engine_burn: Option<f64>,
    ) -> Result<Vec<u8>, TrainError> {
        let mut frozen = FrozenCell::decode(payload)?;
        let coordinate = |p| crate::train_disassembly::coordinate(source, target, p, turns);
        let flow = |mut water: crate::water::FluidCell| {
            let vector = crate::train_disassembly::turn(
                [i64::from(water.flow[0]), 0, i64::from(water.flow[1])],
                turns,
            );
            water.flow = [vector[0] as i8, vector[2] as i8];
            water
        };
        if let Some(pose) = &mut frozen.pose {
            pose.facing = new_facing;
            pose.peer = pose.peer.map(coordinate);
        }
        if let Some(belt) = &mut frozen.belt {
            belt.start = coordinate(belt.start);
            belt.end = coordinate(belt.end);
        }
        if let Some(state) = &mut frozen.furnace {
            state.outlet = coordinate(state.outlet);
            for cell in &mut state.smoke_cells {
                *cell = coordinate(*cell);
            }
        }
        if let Some(state) = &mut frozen.plumbing {
            state.water = flow(state.water);
            if let Some(basin) = &mut state.basin {
                let far = std::array::from_fn(|i| basin.origin[i] + i64::from(basin.size[i]) - 1);
                let a = coordinate(basin.origin);
                let b = coordinate(far);
                basin.origin = std::array::from_fn(|i| a[i].min(b[i]));
                if turns % 2 == 1 {
                    basin.size.swap(0, 2);
                }
            }
        }
        frozen.fluid = frozen.fluid.map(flow);
        if material == 218
            && let Some(seconds) =
                engine_burn.filter(|seconds| *seconds > 0.0 || frozen.furnace.is_some())
        {
            let state = frozen.furnace.get_or_insert_with(Default::default);
            state.burn_remaining = seconds as f32;
            state.burn_total = state.burn_total.max(seconds as f32);
            state.outlet = target;
        }
        serde_json::to_vec(&frozen).map_err(|_| TrainError::Invalid)
    }
    fn restore_train_payload(&mut self, cell: [i64; 3], payload: &[u8]) -> Result<(), TrainError> {
        let frozen = FrozenCell::decode(payload)?;
        let restore = |result: Result<(), String>| result.map_err(|_| TrainError::Invalid);
        if self.object_state(cell) != frozen.pose {
            return Err(TrainError::Invalid);
        }
        self.set_tint(cell, frozen.tint)
            .map_err(|_| TrainError::Invalid)?;
        self.set_surface_condition(cell, frozen.condition)
            .map_err(|_| TrainError::Invalid)?;
        if let Some(inventory) = frozen.inventory {
            self.set_container(cell, inventory)
                .map_err(|_| TrainError::Invalid)?;
        }
        if let Some(state) = frozen.furnace {
            restore(self.set_furnace(cell, state))?;
        }
        if let Some(state) = frozen.crusher {
            restore(self.set_crusher(cell, state))?;
        }
        if let Some(state) = frozen.mixer {
            restore(self.set_mixer(cell, state))?;
        }
        if let Some(state) = frozen.cauldron {
            restore(self.set_cauldron(cell, state))?;
        }
        if let Some(state) = frozen.campfire {
            restore(self.set_campfire(cell, state))?;
        }
        if let Some(state) = frozen.attached_fire {
            self.commit_attached_campfire(cell, state)
                .map_err(|_| TrainError::Invalid)?;
        }
        if let Some(state) = frozen.plumbing {
            self.set_plumbing(cell, state)
                .map_err(|_| TrainError::Invalid)?;
        }
        if let Some(state) = frozen.filter {
            self.set_logistics_filter(cell, state)
                .map_err(|_| TrainError::Invalid)?;
        }
        if let Some(state) = frozen.distributor {
            self.set_distributor(cell, state)
                .map_err(|_| TrainError::Invalid)?;
        }
        if frozen.random_tunnel {
            self.set_tunnel_random(cell, true)
                .map_err(|_| TrainError::Invalid)?;
        }
        if let Some(state) = frozen.belt {
            self.set_belt_owner(cell, Some(state))
                .map_err(|_| TrainError::Invalid)?;
        }
        if let Some(state) = frozen.fluid {
            self.set_fluid(cell, state)
                .map_err(|_| TrainError::Invalid)?;
        } else if self.fluid(cell).is_some() {
            // Assembly deliberately leaves a dry world override so an implicit
            // reservoir cannot refill the vacant vehicle cells. It is not part
            // of a previously dry block's frozen state. Restore true sparsity,
            // with the same fluid revision/invalidation as an explicit change.
            let (owner, local) = global_to_section(cell);
            let epoch = self
                .fluid_epoch(owner)
                .checked_add(1)
                .ok_or(TrainError::Overflow)?;
            self.invalidate_water(cell)
                .map_err(|_| TrainError::Overflow)?;
            let section = self.sections.get_mut(&owner).ok_or(TrainError::Invalid)?;
            section.fluids.remove(&local);
            section.fluid_epoch = epoch;
            self.wake_fluid_neighbors(cell);
        }
        if let Some(order) = frozen.furniture_order {
            if !matches!(self.block(cell), Some(28 | 29)) {
                return Err(TrainError::Invalid);
            }
            self.furniture_order.insert(cell, order);
            self.next_furniture_order = self.next_furniture_order.max(order);
        }
        Ok(())
    }
    /// Move ownership of private state after every fallible operation succeeded.
    fn publish_train_runtime(&mut self, staged: &mut EditJournal, cell: [i64; 3]) {
        macro_rules! transfer {
            ($($field:ident),* $(,)?) => { $(
                self.$field.remove(&cell);
                if let Some(value) = staged.$field.remove(&cell) { self.$field.insert(cell, value); }
            )* };
        }
        transfer!(
            containers,
            furnaces,
            crushers,
            mixers,
            cauldrons,
            campfires,
            attached_campfires,
            plumbing,
            logistics_filters,
            distributors,
            furniture_order
        );
        self.kinetic_states.remove(&cell);
        self.plumbing_supply.remove(&cell);
        self.infusion_views.remove(&cell);
        self.random_tunnels.remove(&cell);
        if staged.random_tunnels.remove(&cell) {
            self.random_tunnels.insert(cell);
        }
        self.next_furniture_order = self.next_furniture_order.max(staged.next_furniture_order);
        let owner = global_to_section(cell).0;
        // Runtime indexes are rebuilt only for cells in this transaction.
        macro_rules! index {
            ($field:ident, $active:expr) => {{
                if $active {
                    self.$field.entry(owner).or_default().insert(cell);
                } else if let Some(cells) = self.$field.get_mut(&owner) {
                    cells.remove(&cell);
                    if cells.is_empty() {
                        self.$field.remove(&owner);
                    }
                }
            }};
        }
        let material = self.block(cell).unwrap_or(0);
        index!(kinetic_sections, terra_core::kinetic::is_part(material));
        index!(logistics_sections, terra_core::logistics::is_port(material));
        index!(
            extended_kinetic_sections,
            terra_core::kinetic::is_extended(material)
                || matches!(material, 144..=146)
                    && self
                        .object_state(cell)
                        .is_some_and(|state| state.peer.is_some())
        );
        index!(crusher_sections, self.crushers.contains_key(&cell));
        index!(plumbing_sections, self.plumbing.contains_key(&cell));
        index!(hold_sections, material == terra_core::infusion::HOLD_BLOCK);
        self.crusher_driven.remove(&cell);
    }
}
