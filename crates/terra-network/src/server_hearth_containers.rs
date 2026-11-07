use super::*;
use terra_core::{Inventory, ItemKind};

fn station_access(peer: &Peer, world: &ControlWorld, station: [i64; 3], cauldron: bool) -> bool {
    let correct = |snapshot: &ContainerSnapshot| {
        if cauldron {
            snapshot.cauldron.is_some()
        } else {
            snapshot.campfire.is_some()
        }
    };
    peer.container
        .as_ref()
        .is_some_and(|snapshot| snapshot.anchor == station && correct(snapshot))
        && snapshot_view(world, station, peer.container.as_ref().unwrap().view)
            .is_some_and(|snapshot| correct(&snapshot) && available(peer, world, &snapshot))
}
pub(super) fn set_recipe(
    peer: &Peer,
    world: &mut ControlWorld,
    station: [i64; 3],
    recipe: u16,
) -> InventoryOutcome {
    if terra_core::brewing::recipe(recipe).is_none() {
        return InventoryOutcome::UnknownRecipe;
    }
    if !station_access(peer, world, station, true) {
        return InventoryOutcome::ContainerUnavailable;
    }
    let Some(mut state) = world.journal.cauldron_at(station).cloned() else {
        return InventoryOutcome::ContainerUnavailable;
    };
    if state.recipe != Some(recipe) {
        state.recipe = Some(recipe);
        state.progress_seconds = 0.0;
    }
    match world.journal.set_cauldron(station, state) {
        Ok(()) => InventoryOutcome::Applied,
        Err(_) => InventoryOutcome::Invalid,
    }
}
pub(super) fn set_lit(
    peer: &Peer,
    world: &mut ControlWorld,
    station: [i64; 3],
    lit: bool,
) -> InventoryOutcome {
    if !station_access(peer, world, station, false) {
        return InventoryOutcome::ContainerUnavailable;
    }
    if lit && !terra_world::hearths::can_light(&world.chunks, &world.journal, station) {
        return InventoryOutcome::Invalid;
    }
    if lit
        && terra_world::hearths::rain_exposed(&world.chunks, &world.journal, station)
        && terra_world::weather::SurfaceWeatherProbe::default()
            .sample(
                &world.weather,
                &world.terrain.0,
                &world.chunks,
                station.map(|v| v as f64 + 0.5),
                *world.clock,
            )
            .rain
            > 0.2
    {
        return InventoryOutcome::Invalid;
    }
    let Some(mut state) = world.journal.hearth_at(station).cloned() else {
        return InventoryOutcome::ContainerUnavailable;
    };
    if state.lit == lit {
        return InventoryOutcome::Applied;
    }
    let mut inventory = world
        .journal
        .hearth_inventory_at(station)
        .cloned()
        .unwrap_or_default();
    if !terra_core::campfire::set_lit(&mut state, &mut inventory, lit) {
        return InventoryOutcome::MissingIngredients;
    }
    match world.journal.commit_hearth(station, state, inventory) {
        Ok(_) => InventoryOutcome::Applied,
        Err(_) => InventoryOutcome::Invalid,
    }
}
pub(super) fn fill_bottle(
    peer: &Peer,
    world: &mut ControlWorld,
    inventory: &Inventory,
    slot: u8,
    position: [i64; 3],
) -> InventoryOutcome {
    if slot >= 9
        || peer.held_slot != Some(slot)
        || !peer.synced
        || peer.vitals.dead()
        || peer.chat.teleport.is_some()
        || peer.game_mode(&world.config) == terra_core::GameMode::Spectator
        || peer.container.is_some()
        || !world.config.world.water
        || position.iter().any(|v| v.unsigned_abs() > 990_000)
        || inventory
            .slot(usize::from(slot))
            .is_none_or(|stack| stack.item != ItemKind::GlassBottle)
    {
        return InventoryOutcome::Denied;
    }
    let Ok(body) = world.bodies.get(peer.body) else {
        return InventoryOutcome::Denied;
    };
    let eye = (body.translation + Vec3::Y * terra_physics::character::PLAYER_EYE_OFFSET)
        .to_array()
        .map(f64::from);
    let direction = (Quat::from_euler(EulerRot::YXZ, peer.look[0], peer.look[1], 0.0)
        * Vec3::NEG_Z)
        .to_array()
        .map(f64::from);
    if terra_world::water::buckets::target(
        &world.chunks,
        &world.journal,
        &world.block_properties,
        false,
        eye,
        direction,
    ) != Some(position)
        || terra_world::water::buckets::source(
            &world.chunks,
            &world.terrain.0,
            &world.journal,
            position,
        )
        .is_none_or(|cell| cell.amount == 0)
    {
        return InventoryOutcome::Denied;
    }
    let tap = world.chunks.block_at(position) == Some(terra_core::plumbing::FAUCET);
    if tap && !peer.permissions.build {
        return InventoryOutcome::Denied;
    }
    let Some(revision) = world
        .journal
        .gameplay()
        .revision
        .checked_add(if tap { 2 } else { 1 })
    else {
        return InventoryOutcome::Invalid;
    };
    if inventory.revision > u64::MAX - 2 {
        return InventoryOutcome::Invalid;
    }
    let mut next = inventory.clone();
    if !next.take_from(usize::from(slot), ItemKind::GlassBottle) {
        return InventoryOutcome::Conflict;
    }
    if next.insert(ItemKind::WaterBottle, 1) != 0 {
        return InventoryOutcome::NoSpace;
    }
    if tap
        && terra_world::plumbing::take_tap(&world.chunks, &mut world.journal, position, 1)
            != Ok(true)
    {
        return InventoryOutcome::Conflict;
    }
    // Springs remain source cells; a faucet debits its finite FluidCell first.
    let game = world.journal.gameplay_mut();
    game.inventories.insert(peer.uuid, next);
    game.revision = revision;
    InventoryOutcome::Applied
}

#[cfg(test)]
#[path = "server_hearth_container_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "server_compound_hearth_tests.rs"]
mod compound_tests;
