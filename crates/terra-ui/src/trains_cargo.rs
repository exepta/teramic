//! One stale token refresh may replay an unchanged inventory intent exactly once.
use super::*;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Intent {
    Slot(u8),
    Sort {
        backpack: bool,
        order: terra_core::inventory_sort::SortOrder,
    },
}

#[derive(Clone)]
pub(super) struct Retry {
    identity: (Option<Entity>, Option<terra_core::PlayerUuid>),
    vehicle: u64,
    part: u16,
    material: u16,
    intent: Intent,
    cursor: Option<ItemStack>,
    source: Option<ItemStack>,
    attempted: bool,
    refreshing: bool,
    replaying: bool,
}

impl Retry {
    fn snapshot(world: &World, intent: Intent) -> Option<Self> {
        let session = world.get_resource::<ClientSession>()?;
        let cargo = session.train_cargo.as_ref()?;
        let vehicle = session
            .trains
            .vehicles
            .iter()
            .find(|vehicle| vehicle.id == cargo.vehicle)?;
        if Some(vehicle.owner) != session.uuid {
            return None;
        }
        let source = match intent {
            Intent::Slot(slot) if usize::from(slot) < terra_core::INVENTORY_SLOTS => {
                session.inventory.slot(usize::from(slot))
            }
            Intent::Slot(slot) => cargo
                .inventory
                .slot(usize::from(slot) - terra_core::INVENTORY_SLOTS),
            Intent::Sort { .. } => None,
        };
        Some(Self {
            identity: (session.connection, session.uuid),
            vehicle: cargo.vehicle,
            part: cargo.part,
            material: vehicle.parts.get(usize::from(cargo.part))?.material,
            intent,
            cursor: session.inventory.cursor,
            source,
            attempted: false,
            refreshing: false,
            replaying: false,
        })
    }

    fn unchanged(&self, world: &World, state: &TrainUiState) -> bool {
        if !state.open || !state.cargo {
            return false;
        }
        let Some(current) = Self::snapshot(world, self.intent) else {
            return false;
        };
        if self.identity != current.identity
            || self.vehicle != current.vehicle
            || self.part != current.part
            || self.material != current.material
            || self.cursor != current.cursor
        {
            return false;
        }
        if self.cursor.is_some() {
            // A cursor click must never become an unexpected item swap after
            // another operation changed the destination's material.
            self.source.map(|stack| stack.item) == current.source.map(|stack| stack.item)
        } else {
            self.source == current.source
        }
    }
}

pub(super) fn capture(world: &World, state: &mut TrainUiState, slot: u8) {
    capture_intent(world, state, Intent::Slot(slot));
}

pub(super) fn capture_sort(
    world: &World,
    state: &mut TrainUiState,
    backpack: bool,
    order: terra_core::inventory_sort::SortOrder,
) {
    capture_intent(world, state, Intent::Sort { backpack, order });
}

fn capture_intent(world: &World, state: &mut TrainUiState, intent: Intent) {
    if let Some(retry) = state
        .cargo_retry
        .as_mut()
        .filter(|retry| retry.replaying && retry.intent == intent)
    {
        retry.replaying = false;
    } else {
        state.cargo_retry = Retry::snapshot(world, intent);
    }
}

pub(super) fn refresh_stale(world: &mut World, state: &mut TrainUiState) {
    let Some(mut retry) = state.cargo_retry.take() else {
        return;
    };
    if retry.attempted || !retry.unchanged(world, state) {
        return;
    }
    retry.attempted = true;
    retry.refreshing = true;
    let action = TrainAction::OpenCargo {
        vehicle: retry.vehicle,
        part: retry.part,
    };
    state.cargo_retry = Some(retry);
    request(world, state, action);
}

pub(super) fn refreshed(world: &mut World, state: &mut TrainUiState) {
    let Some(mut retry) = state.cargo_retry.take() else {
        return;
    };
    if !retry.refreshing {
        return;
    }
    if !retry.unchanged(world, state) {
        state.status =
            "Der Stapel oder Zielplatz hat sich geändert. Bitte erneut anklicken.".into();
        return;
    }
    retry.refreshing = false;
    retry.replaying = true;
    let next = match retry.intent {
        Intent::Slot(slot) => format!("slot:{slot}"),
        Intent::Sort { backpack, order } => format!(
            "cargo-sort-{}{}",
            if backpack { "backpack-" } else { "" },
            match order {
                terra_core::inventory_sort::SortOrder::Category => "type",
                terra_core::inventory_sort::SortOrder::Quantity => "quantity",
                terra_core::inventory_sort::SortOrder::Alphabetical => "name",
                terra_core::inventory_sort::SortOrder::Id => "id",
            }
        ),
    };
    state.cargo_retry = Some(retry);
    command(world, state, &next);
}
