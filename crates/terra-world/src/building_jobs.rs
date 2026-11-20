//! Authoritative, material-backed and physically reachable settlement construction.
mod catalog;
mod movement;
mod planning;
mod preparation;
mod repair;
mod resources;
mod simulation;
mod site;
mod site_path;
mod supplies;
mod template_export;
pub use template_export::export_default_house_template;
#[cfg(test)]
mod tests;
use crate::{
    TerraGenerator,
    creatures::{Habitat, navigation::Navigation},
    edits::EditJournal,
    streaming::ChunkStore,
};
pub use catalog::{reload_template_catalog, template_catalog, template_catalog_error};
pub use planning::{plan_house, plan_template, plan_template_detailed, preview_house};
pub use resources::town_stock;
pub use simulation::{assign_craftsman, queue_house, set_residential, set_storage, tick};
pub use site::{BuildSiteCause, BuildSiteIssue};
use std::collections::BTreeMap;
use terra_core::{
    Inventory, ItemKind, PlayerUuid,
    building_jobs::*,
    creatures::{Creature, CreatureBehavior, CreatureKind},
    kingdom::{ClaimKind, KingdomId, KingdomPermission, claim_chunk},
};

#[derive(Default)]
pub struct BuildRuntime {
    navigation: Navigation,
    suspended: std::collections::BTreeSet<u64>,
    creative_owners: std::collections::BTreeSet<PlayerUuid>,
    routes: BTreeMap<u64, movement::Route>,
    prepared: std::collections::BTreeSet<u64>,
    repairs: BTreeMap<u64, repair::Progress>,
    auto_cursor: BTreeMap<KingdomId, usize>,
    door_requests: BTreeMap<[i64; 3], crate::npc_work::WorkerDoorRequest>,
    player_bodies: Vec<[f64; 3]>,
    cursor: usize,
    return_cursor: usize,
    auto_town_cursor: usize,
}
impl BuildRuntime {
    /// Transient locomotion diagnostics; never part of the saved world or protocol.
    pub fn inspect_route(&self, npc: u64) -> Option<impl std::fmt::Debug + '_> {
        self.routes.get(&npc)
    }

    /// Only the authoritative server supplies this set from saved/current game modes.
    pub fn set_creative_owners(&mut self, owners: impl IntoIterator<Item = PlayerUuid>) {
        self.creative_owners = owners
            .into_iter()
            .filter(|id| !id.is_nil())
            .take(MAX_SETTLEMENTS)
            .collect();
    }

    /// Defense temporarily owns the actor without cancelling its house or bag.
    pub fn set_suspended(&mut self, ids: impl IntoIterator<Item = u64>) {
        let next: std::collections::BTreeSet<_> = ids
            .into_iter()
            .filter(|id| *id > 0)
            .take(terra_core::creatures::MAX_CREATURES)
            .collect();
        // Defense can relocate an actor. Discard only transient locomotion and
        // pending opening requests; saved work and tree reservations survive.
        for id in self.suspended.symmetric_difference(&next) {
            self.navigation.invalidate(*id);
            self.navigation.begin_tick(*id);
            self.routes.remove(id);
            self.door_requests.retain(|_, request| request.npc != *id);
        }
        self.suspended = next;
    }
    pub fn drain_door_requests(
        &mut self,
    ) -> impl Iterator<Item = crate::npc_work::WorkerDoorRequest> {
        std::mem::take(&mut self.door_requests).into_values()
    }
    pub fn set_player_bodies(&mut self, positions: impl IntoIterator<Item = [f64; 3]>) {
        self.player_bodies = positions
            .into_iter()
            .filter(|p| p.iter().all(|v| v.is_finite()))
            .take(128)
            .collect();
    }
}

pub fn build_access(
    chunks: &ChunkStore,
    journal: &EditJournal,
    kingdom: KingdomId,
    owner: PlayerUuid,
    p: [i64; 3],
) -> bool {
    let registry = &journal.gameplay().kingdoms;
    registry.claim_at(claim_chunk(p)).is_some_and(|claim| {
        claim.kind == ClaimKind::ProtectedBuilding
            && registry.realm_id(claim.kingdom) == registry.realm_id(kingdom)
    }) && crate::kingdom_access::can_use(registry, chunks, owner, KingdomPermission::NpcWork, p)
        && crate::kingdom_access::can_use(registry, chunks, owner, KingdomPermission::Build, p)
}
