//! Held use is timed by the authority; inventory and nutrition commit together.
use super::*;
use terra_core::{FoodDefinition, GameMode, Inventory, ItemKind, SurvivalVitals};

const PULSE_TIMEOUT: Duration = Duration::from_millis(400);

pub(super) struct EatingState {
    slot: u8,
    item: ItemKind,
    revision: u64,
    damage_epoch: u64,
    started: Instant,
    pulse: Instant,
    reported: Instant,
    duration: f32,
    mode: GameMode,
}
impl EatingState {
    pub(super) fn progress(&self, now: Instant) -> ConsumptionProgress {
        ConsumptionProgress {
            slot: self.slot,
            item: self.item,
            duration: self.duration,
            progress: (now.duration_since(self.started).as_secs_f32() / self.duration).min(1.0),
        }
    }
}

pub(super) fn cancel(peer: &mut Peer, sender: &mut MessageSender<ServerControl>) {
    if peer.consumption.take().is_some() {
        sender.send::<ControlChannel>(ServerControl::Consumption(None));
    }
}

fn allowed(peer: &Peer, world: &ControlWorld) -> bool {
    peer.synced
        && !peer.vitals.dead()
        && peer.chat.teleport.is_none()
        && peer.container.is_none()
        && peer.mining.is_none()
        && match peer.game_mode(&world.config) {
            GameMode::Survival => peer.vitals.hunger < terra_core::MAX_VITAL - 0.01,
            GameMode::Creative => true,
            GameMode::Spectator => false,
        }
}

pub(super) fn intent(
    slot: Option<u8>,
    revision: u64,
    peer: &mut Peer,
    sender: &mut MessageSender<ServerControl>,
    world: &ControlWorld,
) {
    let now = Instant::now();
    let Some(slot) = slot.filter(|slot| *slot < 9 && peer.held_slot == Some(*slot)) else {
        cancel(peer, sender);
        return;
    };
    let stack = world
        .journal
        .gameplay()
        .inventories
        .get(&peer.uuid)
        .filter(|bag| bag.revision == revision)
        .and_then(|bag| bag.slot(usize::from(slot)));
    let Some((item, food)) =
        stack.and_then(|stack| terra_core::food(stack.item).map(|food| (stack.item, food)))
    else {
        cancel(peer, sender);
        return;
    };
    if !allowed(peer, world) {
        cancel(peer, sender);
        return;
    }
    if let Some(state) = peer.consumption.as_mut()
        && state.slot == slot
        && state.item == item
        && state.revision == revision
        && state.damage_epoch == peer.survival.damage_epoch
        && now.duration_since(state.pulse) <= PULSE_TIMEOUT
    {
        state.pulse = now;
        return;
    }
    cancel(peer, sender);
    let state = EatingState {
        slot,
        item,
        revision,
        damage_epoch: peer.survival.damage_epoch,
        started: now,
        pulse: now,
        reported: now,
        duration: food.eat_seconds,
        mode: peer.game_mode(&world.config),
    };
    sender.send::<ControlChannel>(ServerControl::Consumption(Some(state.progress(now))));
    peer.consumption = Some(state);
}

fn commit(
    inventory: &Inventory,
    vitals: SurvivalVitals,
    slot: u8,
    item: ItemKind,
    mode: GameMode,
) -> Option<(Inventory, SurvivalVitals)> {
    let food = terra_core::food(item)?;
    if slot >= 9 || inventory.slot(usize::from(slot))?.item != item || vitals.dead() {
        return None;
    }
    let mut next = inventory.clone();
    let mut stats = vitals;
    match mode {
        GameMode::Creative => {}
        GameMode::Survival => {
            if !stats.eat(item) {
                return None;
            }
            next = consume_stack(next, slot, item, food)?;
        }
        GameMode::Spectator => return None,
    }
    Some((next, stats))
}

fn consume_stack(
    mut inventory: Inventory,
    slot: u8,
    item: ItemKind,
    food: &FoodDefinition,
) -> Option<Inventory> {
    inventory
        .revision
        .checked_add(1 + u64::from(food.remainder.is_some()))?;
    if slot >= 9
        || !inventory.valid()
        || !food.valid()
        || food.remainder == Some(item)
        || !inventory.take_from(usize::from(slot), item)
    {
        return None;
    }
    if let Some(remainder) = food.remainder
        && inventory.insert(remainder, 1) != 0
    {
        return None;
    }
    Some(inventory)
}

pub(super) fn tick(
    peer: &mut Peer,
    sender: &mut MessageSender<ServerControl>,
    world: &mut ControlWorld,
    now: Instant,
) {
    let Some(state) = peer.consumption.as_ref() else {
        return;
    };
    let bag = world.journal.gameplay().inventories.get(&peer.uuid);
    if !allowed(peer, world)
        || state.mode != peer.game_mode(&world.config)
        || now.duration_since(state.pulse) > PULSE_TIMEOUT
        || peer.held_slot != Some(state.slot)
        || state.damage_epoch != peer.survival.damage_epoch
        || bag.is_none_or(|bag| {
            bag.revision != state.revision
                || bag
                    .slot(usize::from(state.slot))
                    .is_none_or(|stack| stack.item != state.item)
        })
    {
        cancel(peer, sender);
        return;
    }
    let progress = state.progress(now);
    if progress.progress >= 1.0 {
        if let Some((next, stats)) =
            bag.and_then(|bag| commit(bag, peer.vitals, state.slot, state.item, state.mode))
        {
            if state.mode == GameMode::Survival {
                let gameplay = world.journal.gameplay_mut();
                gameplay.inventories.insert(peer.uuid, next.clone());
                gameplay.revision = gameplay.revision.saturating_add(1);
                peer.vitals = stats;
            }
            sender.send::<ControlChannel>(ServerControl::ConsumptionFinished { inventory: next });
        }
        cancel(peer, sender);
    } else if now.duration_since(state.reported) >= Duration::from_millis(80) {
        sender.send::<ControlChannel>(ServerControl::Consumption(Some(progress)));
        if let Some(state) = peer.consumption.as_mut() {
            state.reported = now;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn consumption_commits_one_item_and_creative_does_not_spend_or_restore() {
        let item = ItemKind::Animal(terra_core::creatures::AnimalProduct::CookedBeef);
        let mut bag = Inventory::default();
        assert_eq!(bag.insert(item, 2), 0);
        let stats = SurvivalVitals {
            hunger: 20.0,
            ..Default::default()
        };
        let (next, fed) = commit(&bag, stats, 0, item, GameMode::Survival).unwrap();
        assert_eq!(next.slot(0).unwrap().count, 1);
        assert!(fed.hunger > stats.hunger && next.revision > bag.revision);
        assert_eq!(
            commit(&bag, stats, 0, item, GameMode::Creative).unwrap(),
            (bag.clone(), stats)
        );
        assert!(commit(&bag, SurvivalVitals::default(), 0, item, GameMode::Survival).is_none());
        assert!(commit(&bag, stats, 9, item, GameMode::Survival).is_none());
        assert!(commit(&bag, stats, 0, item, GameMode::Spectator).is_none());
    }

    #[test]
    fn food_remainders_require_space_or_merge_without_losing_the_original_stack() {
        use terra_core::{INVENTORY_SLOTS, ItemStack, STACK_LIMIT};

        let item = ItemKind::Animal(terra_core::creatures::AnimalProduct::CookedBeef);
        let food = FoodDefinition {
            remainder: Some(ItemKind::Stick),
            ..*terra_core::food(item).unwrap()
        };
        let mut bag = Inventory {
            slots: vec![
                ItemStack {
                    item: ItemKind::Block(3),
                    count: STACK_LIMIT,
                };
                INVENTORY_SLOTS
            ],
            revision: 17,
            ..Default::default()
        };
        bag.slots[0] = ItemStack { item, count: 2 };
        let before = bag.clone();
        assert!(consume_stack(bag.clone(), 0, item, &food).is_none());
        assert_eq!(bag, before);

        bag.slots[0].count = 1;
        let next = consume_stack(bag.clone(), 0, item, &food).unwrap();
        assert_eq!(
            next.slot(0),
            Some(ItemStack {
                item: ItemKind::Stick,
                count: 1,
            })
        );
        assert_eq!(next.count(item), 0);
        assert_eq!(next.revision, bag.revision + 2);
        assert!(next.valid());

        bag.slots[0].count = 2;
        bag.slots[1] = ItemStack {
            item: ItemKind::Stick,
            count: STACK_LIMIT - 1,
        };
        let next = consume_stack(bag.clone(), 0, item, &food).unwrap();
        assert_eq!(next.slot(0), Some(ItemStack { item, count: 1 }));
        assert_eq!(next.slot(1).unwrap().count, STACK_LIMIT);
        assert_eq!(next.occupied_slots(), INVENTORY_SLOTS);
        assert!(next.valid());
        for remainder in [ItemKind::Block(0), item] {
            assert!(
                consume_stack(
                    bag.clone(),
                    0,
                    item,
                    &FoodDefinition {
                        remainder: Some(remainder),
                        ..food
                    },
                )
                .is_none()
            );
        }
        bag.revision = u64::MAX - 1;
        assert!(consume_stack(bag, 0, item, &food).is_none());
    }
}
