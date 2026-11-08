#[cfg(feature = "bevy")]
#[path = "sapling_placement.rs"]
mod sapling_placement;
#[cfg(feature = "bevy")]
#[path = "tree_growth.rs"]
pub(crate) mod tree_growth;
use crate::environment::{SurfaceCondition, SurfaceConditionError, SurfaceSnapshot};
use crate::generation::sections::BlockOverrides;
use crate::props::{PropId, WorldProp};
use crate::water::FluidCell;
use std::collections::{HashMap, HashSet};
use terra_voxel::blocks::BlockError;
use terra_voxel::{SectionPos, global_to_section};
#[cfg(test)]
#[path = "import_performance_tests.rs"]
mod import_performance_tests;
#[cfg(feature = "bevy")]
#[path = "rails.rs"]
mod rails;
#[cfg(feature = "bevy")]
#[path = "rotation.rs"]
mod rotation;
#[cfg(feature = "bevy")]
#[path = "trains.rs"]
mod trains;

/// Valid network deltas normally contain unique cells. Keep historical
/// duplicate-block validation (any matching value, last stored value) intact.
fn delta_block_matches(
    indexed: &HashMap<[usize; 3], u16>,
    original: &[([u8; 3], u16)],
    local: [u8; 3],
    valid: impl Fn(u16) -> bool,
) -> bool {
    if indexed.len() == original.len() {
        indexed
            .get(&local.map(usize::from))
            .copied()
            .is_some_and(valid)
    } else {
        original
            .iter()
            .any(|(cell, material)| *cell == local && valid(*material))
    }
}

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
    #[serde(default)]
    pub tints: Vec<([u8; 3], [u8; 3])>,
    #[serde(default)]
    pub belts: Vec<([u8; 3], super::BeltOwner)>,
}

#[derive(Default, Clone)]
pub(super) struct SectionEdits {
    pub(super) belts: HashMap<[usize; 3], super::BeltOwner>,
    tints: HashMap<[usize; 3], [u8; 3]>,
    pub(super) objects: HashMap<[usize; 3], super::BlockObjectState>,
    revision: u64,
    pub(super) blocks: HashMap<[usize; 3], u16>,
    pub(super) prop_epoch: u64,
    pub(super) removed_props: HashSet<PropId>,
    pub(super) placed_props: HashMap<PropId, WorldProp>,
    appearance_epoch: u64,
    surface_conditions: SurfaceSnapshot,
    pub(super) fluid_epoch: u64,
    pub(super) fluids: HashMap<[usize; 3], FluidCell>,
}

impl SectionEdits {
    pub(super) fn commit_belt_owner(
        &mut self,
        local: [usize; 3],
        owner: Option<super::BeltOwner>,
        appearance_revision: Option<u64>,
        appearance_epoch: u64,
    ) {
        if let Some(owner) = owner {
            self.belts.insert(local, owner);
        } else {
            self.belts.remove(&local);
        }
        if let Some(revision) = appearance_revision {
            self.revision = revision;
        }
        self.appearance_epoch = appearance_epoch;
    }

    pub(super) fn clear_pose_tint(&mut self, local: [usize; 3], preflighted_epoch: u64) {
        self.tints.remove(&local);
        self.appearance_epoch = preflighted_epoch;
    }
}

/// Session-local authoritative edits survive section eviction. Disk/network storage is separate.
#[derive(Default, Clone)]
#[cfg_attr(feature = "bevy", derive(bevy::prelude::Resource))]
pub struct EditJournal {
    /// World metadata travels with the existing atomic save snapshot.
    pub clock: Option<terra_core::WorldClock>,
    pub weather: Option<terra_core::WorldWeather>,
    pub world_rules: Option<terra_core::WorldRuleSettings>,
    /// Runtime-only shared flower animation; excluded from save/network deltas.
    pub flowers: crate::props::seasonal::FlowerLifecycle,
    #[cfg(feature = "bevy")]
    pub(crate) planted_appearances:
        std::sync::Arc<std::sync::Mutex<crate::saplings::AppearanceCache>>,
    player_state_revision: u64,
    player_locations: HashMap<terra_core::PlayerUuid, terra_core::PlayerLocation>,
    gameplay: terra_core::GameplayState,
    pub(super) containers: HashMap<[i64; 3], terra_core::Inventory>,
    pub(super) furnaces: HashMap<[i64; 3], terra_core::furnace::FurnaceState>,
    pub(super) crushers: HashMap<[i64; 3], terra_core::crusher::CrusherState>,
    pub(super) mixers: HashMap<[i64; 3], terra_core::mixer::MixerState>,
    pub(super) cauldrons: HashMap<[i64; 3], terra_core::brewing::CauldronState>,
    pub(super) campfires: HashMap<[i64; 3], terra_core::campfire::CampfireState>,
    pub(super) attached_campfires: HashMap<[i64; 3], terra_core::campfire::AttachedCampfire>,
    pub(super) plumbing: std::collections::BTreeMap<[i64; 3], crate::plumbing::PlumbingState>,
    pub(super) plumbing_sections: HashMap<SectionPos, std::collections::BTreeSet<[i64; 3]>>,
    pub(super) plumbing_supply: HashMap<[i64; 3], crate::plumbing::SupplyToken>,
    pub(super) plumbing_epoch: u64,
    pub(super) hold_sections: HashMap<SectionPos, std::collections::BTreeSet<[i64; 3]>>,
    /// Runtime presentation only; no item ownership, fluid storage or persisted progress.
    pub(super) infusion_views:
        std::collections::BTreeMap<[i64; 3], terra_core::infusion::HoldVisual>,
    pub(super) logistics_filters: HashMap<[i64; 3], terra_core::logistics::FilterConfig>,
    pub(super) distributors: HashMap<[i64; 3], terra_core::logistics::DistributorConfig>,
    pub(super) random_tunnels: HashSet<[i64; 3]>,
    pub(super) crusher_sections: HashMap<SectionPos, HashSet<[i64; 3]>>,
    pub(super) crusher_driven: HashSet<[i64; 3]>,
    pub(super) kinetic_sections: HashMap<SectionPos, std::collections::BTreeSet<[i64; 3]>>,
    pub(super) logistics_sections: HashMap<SectionPos, std::collections::BTreeSet<[i64; 3]>>,
    pub(super) extended_kinetic_sections: HashMap<SectionPos, std::collections::BTreeSet<[i64; 3]>>,
    pub(super) kinetic_states:
        std::collections::BTreeMap<[i64; 3], terra_core::kinetic::KineticState>,
    pub(super) crops: HashMap<[i64; 3], terra_core::farming::CropState>,
    pub(super) crop_sections: HashMap<SectionPos, HashSet<[i64; 3]>>,
    pub(super) crop_props: HashMap<PropId, [i64; 3]>,
    pub(crate) forage_regrowth: HashMap<PropId, crate::farming::ForageRegrowth>,
    pub(super) furniture_order: HashMap<[i64; 3], u64>,
    pub(super) next_furniture_order: u64,
    pub(super) sections: HashMap<SectionPos, SectionEdits>,
    pub(super) object_sections: HashSet<SectionPos>,
    pub(super) rail_sections: HashSet<SectionPos>,
    pub(super) rail_anchor_counts: HashMap<SectionPos, usize>,
    pub(super) rail_epoch: u64,
    pub(super) rail_identity: std::sync::Arc<()>,
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
    pub fn player_location(
        &self,
        player: terra_core::PlayerUuid,
    ) -> Option<terra_core::PlayerLocation> {
        self.player_locations.get(&player).copied()
    }

    pub fn player_locations(
        &self,
    ) -> impl Iterator<Item = (terra_core::PlayerUuid, terra_core::PlayerLocation)> + '_ {
        self.player_locations
            .iter()
            .map(|(&id, &location)| (id, location))
    }

    pub fn update_player_location(
        &mut self,
        player: terra_core::PlayerUuid,
        location: terra_core::PlayerLocation,
    ) -> Result<bool, String> {
        if player.is_nil() || !location.valid() {
            return Err("Invalid player location".into());
        }
        if self.player_locations.get(&player) == Some(&location) {
            return Ok(false);
        }
        if !self.player_locations.contains_key(&player) && self.player_locations.len() >= 4096 {
            return Err("Player location capacity reached".into());
        }
        let next = self
            .player_state_revision
            .checked_add(1)
            .ok_or("Player metadata revision overflow")?;
        self.player_locations.insert(player, location);
        self.player_state_revision = next;
        Ok(true)
    }

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
            || !stats.equipment.valid()
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
            tints: {
                let mut values: Vec<_> = edits
                    .tints
                    .iter()
                    .map(|(p, rgb)| (p.map(|v| v as u8), *rgb))
                    .collect();
                values.sort_unstable_by_key(|(p, _)| *p);
                values
            },
            belts: {
                let mut values: Vec<_> = edits
                    .belts
                    .iter()
                    .map(|(p, owner)| (p.map(|v| v as u8), *owner))
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
        let _scope = terra_core::profiling::ScopeTimer::new("world.import_section_delta");
        delta.position.origin().map_err(|e| e.to_string())?;
        if delta.blocks.len() > 4096
            || delta.objects.len() > 4096
            || delta.belts.len() > 4096
            || delta.tints.len() > 4096
            || delta.conditions.len() > 4096
            || delta.removed_props.len() > 4096
            || delta.placed_props.len() > 256
            || delta.fluids.len() > 4096
            || delta
                .blocks
                .iter()
                .any(|(local, _)| local.iter().any(|value| *value >= 16))
        {
            return Err("Invalid section delta".into());
        }
        // Reuse the final sparse maps for validation as well as installation.
        // Dense construction packets must not scan all blocks for every object.
        let blocks: HashMap<_, _> = delta
            .blocks
            .iter()
            .map(|(local, material)| (local.map(usize::from), *material))
            .collect();
        let objects: HashMap<_, _> = delta
            .objects
            .iter()
            .map(|(local, state)| (local.map(usize::from), *state))
            .collect();
        if objects.len() != delta.objects.len()
            || delta.belts.iter().any(|(local, owner)| {
                local.iter().any(|value| *value >= 16)
                    || !delta
                        .position
                        .global_block(local.map(usize::from))
                        .is_ok_and(|position| owner.valid_at(position))
                    || !delta_block_matches(&blocks, &delta.blocks, *local, |material| {
                        matches!(material, 144..=146 | 148)
                    })
                    || !objects.contains_key(&local.map(usize::from))
            })
            || delta
                .belts
                .iter()
                .map(|(local, _)| local)
                .collect::<HashSet<_>>()
                .len()
                != delta.belts.len()
            || delta.tints.iter().any(|(local, _)| {
                local.iter().any(|value| *value >= 16)
                    || !delta_block_matches(&blocks, &delta.blocks, *local, |material| {
                        terra_core::tints::cell_dyeable(
                            material,
                            objects
                                .get(&local.map(usize::from))
                                .map_or(0, |state| state.facing),
                        )
                    })
            })
            || delta
                .tints
                .iter()
                .map(|(local, _)| local)
                .collect::<HashSet<_>>()
                .len()
                != delta.tints.len()
            || delta.objects.iter().any(|(local, state)| {
                local.iter().any(|value| *value >= 16)
                    || !delta_block_matches(&blocks, &delta.blocks, *local, |material| {
                        delta
                            .position
                            .global_block(local.map(usize::from))
                            .is_ok_and(|position| state.valid_for(position, material))
                    })
            })
            || delta
                .placed_props
                .iter()
                .any(|prop| !prop.valid() || prop.section != delta.position || prop.removed)
            || delta
                .placed_props
                .iter()
                .map(|prop| prop.id)
                .collect::<HashSet<_>>()
                .len()
                != delta.placed_props.len()
            || delta.conditions.iter().any(|(local, condition)| {
                local.iter().any(|value| *value >= 16) || condition.validate().is_err()
            })
            || delta.fluids.iter().any(|(local, cell)| {
                local.iter().any(|value| *value >= 16)
                    || !cell.validate()
                    || cell.amount == 0 && !blocks.contains_key(&local.map(usize::from))
            })
            || delta
                .fluids
                .iter()
                .map(|(local, _)| local)
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
        drop(delta.blocks);
        drop(delta.objects);
        let tints: HashMap<_, _> = delta
            .tints
            .into_iter()
            .map(|(p, rgb)| (p.map(usize::from), rgb))
            .collect();
        let belts: HashMap<_, _> = delta
            .belts
            .into_iter()
            .map(|(p, owner)| (p.map(usize::from), owner))
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
                    || old
                        .and_then(|e| e.belts.get(local))
                        .map(|owner| (owner.start, owner.end))
                        != belts.get(local).map(|owner| (owner.start, owner.end))
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
        // No consumer observes the intermediate imported object poses. Rebuild
        // their sparse indexes once after the final metadata is installed.
        // On a bounded-revision failure, resync against the actual partial state
        // before returning, matching normal record() failure behavior.
        for local in changed {
            let global = p.global_block(local).map_err(|error| {
                self.refresh_object_section(p);
                error.to_string()
            })?;
            affected.extend(
                self.record_with_order(
                    global,
                    blocks.get(&local).copied().unwrap_or(0),
                    false,
                    wake_simulation,
                    false,
                    false,
                )
                .map_err(|error| {
                    self.refresh_object_section(p);
                    error.to_string()
                })?,
            );
        }
        for local in fluid_changes {
            let global = p.global_block(local).map_err(|error| {
                self.refresh_object_section(p);
                error.to_string()
            })?;
            self.invalidate_water(global).map_err(|error| {
                self.refresh_object_section(p);
                error.to_string()
            })?;
        }
        self.sections.insert(
            p,
            SectionEdits {
                belts,
                tints,
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
        let edits = &self.sections[&p];
        let extended: std::collections::BTreeSet<_> = edits
            .blocks
            .iter()
            .filter_map(|(local, material)| {
                (terra_core::kinetic::is_extended(*material)
                    || matches!(material, 144..=146)
                        && edits
                            .objects
                            .get(local)
                            .is_some_and(|state| state.peer.is_some()))
                .then(|| p.global_block(*local).ok())
                .flatten()
            })
            .collect();
        if extended.is_empty() {
            self.extended_kinetic_sections.remove(&p);
        } else {
            self.extended_kinetic_sections.insert(p, extended);
        }
        self.refresh_object_section(p);
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
    pub fn tint_at(&self, position: [i64; 3]) -> Option<[u8; 3]> {
        let (section, local) = global_to_section(position);
        self.sections.get(&section)?.tints.get(&local).copied()
    }
    pub fn tint_snapshot(&self, section: SectionPos) -> HashMap<[usize; 3], [u8; 3]> {
        self.sections
            .get(&section)
            .map(|edits| edits.tints.clone())
            .unwrap_or_default()
    }
    pub fn tints(&self) -> impl Iterator<Item = ([i64; 3], [u8; 3])> + '_ {
        self.sections.iter().flat_map(|(&section, edits)| {
            edits
                .tints
                .iter()
                .filter_map(move |(&p, &rgb)| section.global_block(p).ok().map(|p| (p, rgb)))
        })
    }
    /// Sparse appearance metadata never changes block identity, collision or water.
    pub fn set_tint(&mut self, position: [i64; 3], color: Option<[u8; 3]>) -> Result<bool, String> {
        if self.tint_at(position) == color {
            return Ok(false);
        }
        if color.is_some()
            && !self.block(position).is_some_and(|material| {
                terra_core::tints::cell_dyeable(
                    material,
                    self.object_state(position).unwrap_or_default().facing,
                )
            })
        {
            return Err("RGB tint requires a recorded dyeable block".into());
        }
        let (section, local) = global_to_section(position);
        let revision = self
            .revision(section)
            .checked_add(1)
            .ok_or("Tint revision overflow")?;
        let appearance = self
            .appearance_epoch(section)
            .checked_add(1)
            .ok_or("Tint appearance overflow")?;
        let edits = self.sections.entry(section).or_default();
        if let Some(rgb) = color {
            edits.tints.insert(local, rgb);
        } else {
            edits.tints.remove(&local);
        }
        edits.revision = revision;
        edits.appearance_epoch = appearance;
        Ok(true)
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

    /// Sparse saved props remain available when their owning section is unloaded.
    pub fn placed_props(&self, section: SectionPos) -> impl Iterator<Item = &WorldProp> {
        self.sections
            .get(&section)
            .into_iter()
            .flat_map(|edits| edits.placed_props.values())
    }

    /// Loaded prop lists have unique IDs, but may be a fresh generated snapshot
    /// even when the journal already knows this epoch. Inspect without reflowing.
    pub(crate) fn props_need_sync(&self, section: SectionPos, props: &[WorldProp]) -> bool {
        let Some(edits) = self.sections.get(&section) else {
            return false;
        };
        if edits.placed_props.is_empty() && edits.removed_props.is_empty() {
            return false;
        }
        let mut placed = 0;
        for prop in props {
            if prop.removed != edits.removed_props.contains(&prop.id) {
                return true;
            }
            let Some(saved) = edits.placed_props.get(&prop.id) else {
                continue;
            };
            placed += 1;
            let mut logical = *prop;
            logical.position = saved.position;
            if logical != *saved {
                return true;
            }
            if prop.position != saved.position {
                // Ground vegetation can acquire an ID-derived group offset.
                // Reuse the layout rules; unknown placements need hydration.
                let support = crate::props::placement::support(saved);
                if prop.growing_grass()
                    || prop.position[1] != saved.position[1]
                    || crate::props::placement::support(prop) != support
                    || !crate::props::placement::compatible(prop, support)
                    || !crate::props::placement::compatible(saved, support)
                {
                    return true;
                }
            }
        }
        placed != edits.placed_props.len()
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
        if let Some(tree) = self.gameplay.trees.get_mut(id.0)
            && tree.stage == terra_core::TreeStage::Sapling
        {
            tree.stage = terra_core::TreeStage::Felled;
            tree.revision = tree.revision.saturating_add(1);
            self.gameplay.trees.changed();
        }
        if let Some(position) = self.crop_props.get(&id).copied() {
            self.remove_crop(position);
        }
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
        self.record_with_order(global, material, true, true, true, true)
    }

    /// Persist metadata for a block already present in a freshly generated section.
    /// This is not a terrain edit: workers and simulation retain their valid snapshots.
    pub(super) fn register_generated_block(
        &mut self,
        global: [i64; 3],
        material: u16,
        state: Option<super::BlockObjectState>,
    ) -> Result<bool, BlockError> {
        if self.block(global).is_some() {
            return Ok(false);
        }
        let (owner, local) = global_to_section(global);
        let revision = self
            .revision(owner)
            .checked_add(1)
            .ok_or(BlockError::RevisionOverflow)?;
        let furniture_order = matches!(material, 28 | 29)
            .then(|| {
                self.next_furniture_order
                    .checked_add(1)
                    .ok_or(BlockError::RevisionOverflow)
            })
            .transpose()?;
        let edits = self.sections.entry(owner).or_default();
        edits.blocks.insert(local, material);
        if let Some(state) = state {
            edits.objects.insert(local, state);
        }
        edits.revision = revision;
        self.refresh_object_section(owner);
        if terra_core::kinetic::is_part(material) {
            self.kinetic_sections
                .entry(owner)
                .or_default()
                .insert(global);
        }
        if let Some(order) = furniture_order {
            self.furniture_order.insert(global, order);
            self.next_furniture_order = order;
        }
        Ok(true)
    }

    /// Only loaded neighbors need new geometry when generated joinery metadata arrives.
    #[cfg(feature = "bevy")]
    pub(crate) fn invalidate_structure_meshes(
        &mut self,
        sections: &std::collections::BTreeSet<SectionPos>,
    ) -> Result<(), BlockError> {
        for &section in sections {
            self.mesh_epoch(section)
                .checked_add(1)
                .ok_or(BlockError::RevisionOverflow)?;
        }
        for &section in sections {
            *self.mesh_epochs.entry(section).or_default() += 1;
        }
        Ok(())
    }

    pub(super) fn invalidate_generated_water_sections(
        &mut self,
        sections: &std::collections::BTreeSet<SectionPos>,
    ) -> Result<(), BlockError> {
        for &section in sections {
            self.water_mesh_epoch(section)
                .checked_add(1)
                .ok_or(BlockError::RevisionOverflow)?;
        }
        for &section in sections {
            *self.water_mesh_epochs.entry(section).or_default() += 1;
        }
        Ok(())
    }

    /// The caller preflights the complete generated batch and coalesces its
    /// water halo. Only real new sources wake fluid work, never ordinary roofs.
    pub(super) fn register_generated_source(&mut self, global: [i64; 3]) -> Result<(), BlockError> {
        let cell = FluidCell {
            amount: crate::water::FLUID_CAPACITY,
            flow: [0; 2],
            source: true,
        };
        if self.fluid(global) == Some(cell) {
            return Ok(());
        }
        let (owner, local) = global_to_section(global);
        let epoch = self
            .fluid_epoch(owner)
            .checked_add(1)
            .ok_or(BlockError::RevisionOverflow)?;
        let edits = self.sections.entry(owner).or_default();
        edits.fluids.insert(local, cell);
        edits.fluid_epoch = epoch;
        self.wake_fluid_neighbors(global);
        Ok(())
    }
    fn record_with_order(
        &mut self,
        global: [i64; 3],
        material: u16,
        track_furniture: bool,
        wake_simulation: bool,
        refresh_objects: bool,
        relinquish_tree: bool,
    ) -> Result<Vec<SectionPos>, BlockError> {
        if material != terra_core::castle::BERGFRIED
            && self.gameplay.kingdoms.active_at(global).is_some()
        {
            return Err(BlockError::ProtectedStructure);
        }
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
        // Roof-corner profiles sampled by pipe mortar reach two cells. Expand
        // only for actual recorded chimneys, not every terrain edit's halo.
        if local.iter().any(|&v| matches!(v, 0 | 1 | 14 | 15)) {
            for first in crate::objects::DIRECTIONS {
                for second in crate::objects::DIRECTIONS {
                    let position =
                        std::array::from_fn(|axis| global[axis] + first[axis] + second[axis]);
                    if self.block(position).is_some_and(|id| {
                        id == 60
                            || id == 59
                                && self
                                    .object_state(position)
                                    .is_some_and(|state| state.secondary)
                    }) {
                        let dependent = global_to_section(position).0;
                        if !affected.contains(&dependent) {
                            affected.push(dependent);
                        }
                    }
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
        let rail_epoch = if terra_core::rails::is_rail(material)
            || self.block(global).is_some_and(terra_core::rails::is_rail)
        {
            Some(
                self.rail_epoch
                    .checked_add(1)
                    .ok_or(BlockError::RevisionOverflow)?,
            )
        } else {
            None
        };
        let furniture_order = if new_furniture {
            Some(
                self.next_furniture_order
                    .checked_add(1)
                    .ok_or(BlockError::RevisionOverflow)?,
            )
        } else {
            None
        };
        // Confirmed snapshots carry independent authoritative tree ownership.
        // Installing their section cells is not a new local replacement edit.
        if relinquish_tree {
            self.gameplay.trees.relinquish(global);
        }
        let edits = self.sections.entry(owner).or_default();
        let previous = edits.blocks.insert(local, material);
        if let Some(epoch) = rail_epoch {
            self.rail_epoch = epoch;
            std::sync::Arc::make_mut(&mut self.rail_identity);
        }
        if !matches!(material, 144..=146 | 148) || previous.is_some_and(|old| old != material) {
            edits.belts.remove(&local);
        }
        if !terra_core::tints::cell_dyeable(
            material,
            edits.objects.get(&local).map_or(0, |state| state.facing),
        ) || previous.is_some_and(|old| old != material)
        {
            edits.tints.remove(&local);
        }
        if !(matches!(material, 28..=29 | 40 | 59..=73 | 99..=100 | 131 | 141..=172)
            || crate::objects::is_opening(material)
            || terra_core::construction::is_shape(material)
            || terra_core::castle::is_castle(material)
            || terra_core::rails::is_rail(material)
            || terra_core::lantern::is_part(material))
            || previous.is_some_and(|old| old != material)
        {
            edits.objects.remove(&local);
        }
        if terra_core::kinetic::is_part(material) {
            self.kinetic_sections
                .entry(owner)
                .or_default()
                .insert(global);
        } else if let Some(positions) = self.kinetic_sections.get_mut(&owner) {
            positions.remove(&global);
            if positions.is_empty() {
                self.kinetic_sections.remove(&owner);
            }
        }
        if terra_core::logistics::is_port(material) {
            self.logistics_sections
                .entry(owner)
                .or_default()
                .insert(global);
        } else if let Some(positions) = self.logistics_sections.get_mut(&owner) {
            positions.remove(&global);
            if positions.is_empty() {
                self.logistics_sections.remove(&owner);
            }
        }
        self.kinetic_states.remove(&global);
        if material != terra_core::brewing::MATERIAL || previous.is_some_and(|old| old != material)
        {
            self.attached_campfires.remove(&global);
        }
        if (material != terra_core::brewing::MATERIAL
            || previous.is_some_and(|old| old != material))
            && self.cauldrons.remove(&global).is_some()
        {
            self.containers.remove(&global);
        }
        if (material != terra_core::campfire::MATERIAL
            || previous.is_some_and(|old| old != material))
            && self.campfires.remove(&global).is_some()
        {
            self.containers.remove(&global);
        }
        if !terra_core::logistics::is_tunnel(material)
            || previous.is_some_and(|old| old != material)
        {
            self.random_tunnels.remove(&global);
        }
        if !terra_core::logistics::supports_filter(material)
            || previous.is_some_and(|old| old != material)
        {
            self.logistics_filters.remove(&global);
        }
        if material != terra_core::logistics::BELT_DISTRIBUTOR
            || previous.is_some_and(|old| old != material)
        {
            self.distributors.remove(&global);
        }
        if terra_core::kinetic::is_extended(material)
            || matches!(material, 144..=146)
                && edits
                    .objects
                    .get(&local)
                    .is_some_and(|state| state.peer.is_some())
        {
            self.extended_kinetic_sections
                .entry(owner)
                .or_default()
                .insert(global);
        } else if let Some(positions) = self.extended_kinetic_sections.get_mut(&owner) {
            positions.remove(&global);
            if positions.is_empty() {
                self.extended_kinetic_sections.remove(&owner);
            }
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
        if terra_core::plumbing::is_part(material) {
            self.invalidate_plumbing_supply();
        } else if previous.is_some_and(terra_core::plumbing::is_part) {
            self.remove_plumbing(global);
        }
        if refresh_objects {
            self.refresh_object_section(owner);
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
