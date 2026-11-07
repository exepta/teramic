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

fn allowed(peer: &Peer, world: &ControlWorld, item: ItemKind) -> bool {
    peer.synced
        && !peer.vitals.dead()
        && peer.chat.teleport.is_none()
        && peer.container.is_none()
        && peer.mining.is_none()
        && match peer.game_mode(&world.config) {
            GameMode::Survival => {
                terra_core::brewing::drink_seconds(item).is_some()
                    || peer.vitals.hunger < terra_core::MAX_VITAL - 0.01
            }
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
    intent_at(slot, revision, peer, sender, world, Instant::now());
}

fn intent_at(
    slot: Option<u8>,
    revision: u64,
    peer: &mut Peer,
    sender: &mut MessageSender<ServerControl>,
    world: &ControlWorld,
    now: Instant,
) {
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
    let Some((item, duration)) = stack.and_then(|stack| {
        terra_core::food(stack.item)
            .map(|food| food.eat_seconds)
            .or_else(|| terra_core::brewing::drink_seconds(stack.item))
            .map(|duration| (stack.item, duration))
    }) else {
        cancel(peer, sender);
        return;
    };
    if !allowed(peer, world, item) {
        cancel(peer, sender);
        return;
    }
    if let Some(state) = peer.consumption.as_mut()
        && state.slot == slot
        && state.item == item
        && state.revision == revision
        && (state.damage_epoch == peer.survival.damage_epoch
            || terra_core::brewing::drink_seconds(state.item).is_some())
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
        duration,
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
    let food = terra_core::food(item);
    let drink = terra_core::brewing::drink_seconds(item).is_some();
    if food.is_none() && !drink {
        return None;
    }
    if slot >= 9
        || !inventory.valid()
        || !vitals.valid()
        || inventory.slot(usize::from(slot))?.item != item
        || vitals.dead()
    {
        return None;
    }
    let mut next = inventory.clone();
    let mut stats = vitals;
    match mode {
        GameMode::Creative => {}
        GameMode::Survival => {
            if drink {
                next.revision.checked_add(2)?;
                if !next.take_from(usize::from(slot), item)
                    || next.insert(ItemKind::GlassBottle, 1) != 0
                {
                    return None;
                }
            } else {
                if !stats.eat(item) {
                    return None;
                }
                next = consume_stack(next, slot, item, food?)?;
            }
        }
        GameMode::Spectator => return None,
    }
    if let ItemKind::Potion(id) = item {
        for dose in &terra_core::brewing::recipe(id)?.effects {
            stats.effects.apply(dose.kind, dose.seconds, dose.level);
        }
    } else if item == ItemKind::WaterBottle {
        stats
            .effects
            .remove(terra_core::effects::EffectKind::OnFire);
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
    if !allowed(peer, world, state.item)
        || state.mode != peer.game_mode(&world.config)
        || now.duration_since(state.pulse) > PULSE_TIMEOUT
        || peer.held_slot != Some(state.slot)
        || state.damage_epoch != peer.survival.damage_epoch
            && terra_core::brewing::drink_seconds(state.item).is_none()
        || world.journal.gameplay().revision == u64::MAX
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
            }
            peer.vitals = stats;
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
    use bevy::ecs::system::SystemState;
    use terra_core::{ItemStack, effects::EffectKind};

    fn pulse(
        app: &mut App,
        peer: &mut Peer,
        sender: &mut MessageSender<ServerControl>,
        now: Instant,
    ) {
        let revision =
            app.world().resource::<EditJournal>().gameplay().inventories[&peer.uuid].revision;
        let mut system = SystemState::<ControlWorld>::new(app.world_mut());
        intent_at(
            Some(0),
            revision,
            peer,
            sender,
            &system.get_mut(app.world_mut()).unwrap(),
            now,
        );
    }

    fn step(
        app: &mut App,
        peer: &mut Peer,
        sender: &mut MessageSender<ServerControl>,
        now: Instant,
    ) {
        let mut system = SystemState::<ControlWorld>::new(app.world_mut());
        tick(
            peer,
            sender,
            &mut system.get_mut(app.world_mut()).unwrap(),
            now,
        );
    }

    #[test]
    fn held_water_and_mixed_potions_finish_through_real_damage_without_restarting_or_healing_it() {
        for item in [ItemKind::WaterBottle, ItemKind::Potion(6)] {
            let (mut app, mut peer) = crate::server::rgb_filter_tests::fixture();
            let mut inventory = Inventory::default();
            assert_eq!(inventory.insert(item, 2), 0);
            app.world_mut()
                .resource_mut::<EditJournal>()
                .gameplay_mut()
                .inventories
                .insert(peer.uuid, inventory);
            peer.vitals.effects.apply(EffectKind::OnFire, 10.0, 1);
            peer.vitals.effects.apply(EffectKind::Poison, 12.0, 2);
            peer.vitals.effects.apply(EffectKind::Speed, 20.0, 1);
            let before = peer.vitals;
            let mut sender = MessageSender::default();
            let started = Instant::now();
            pulse(&mut app, &mut peer, &mut sender, started);
            assert_eq!(peer.consumption.as_ref().unwrap().duration, 1.4);
            for millis in [250, 500, 750, 1000, 1250, 1500] {
                let now = started + Duration::from_millis(millis);
                if millis == 1000 {
                    peer.survival.damage_epoch += 1;
                    peer.vitals.health -= 5.0;
                    step(&mut app, &mut peer, &mut sender, now);
                    assert!(
                        peer.consumption.is_some(),
                        "damage must not cancel {item:?}"
                    );
                }
                pulse(&mut app, &mut peer, &mut sender, now);
                assert_eq!(peer.consumption.as_ref().unwrap().started, started);
                step(&mut app, &mut peer, &mut sender, now);
            }
            assert!(peer.consumption.is_none());
            let inventory =
                &app.world().resource::<EditJournal>().gameplay().inventories[&peer.uuid];
            assert_eq!(inventory.count(item), 1);
            assert_eq!(inventory.count(ItemKind::GlassBottle), 1);
            assert_eq!(peer.vitals.health, before.health - 5.0);
            assert_eq!(peer.vitals.hunger, before.hunger);
            for effect in [EffectKind::Poison, EffectKind::Speed] {
                assert_eq!(
                    peer.vitals.effects.timed(effect),
                    before.effects.timed(effect)
                );
            }
            if let ItemKind::Potion(id) = item {
                assert_eq!(
                    peer.vitals.effects.timed(EffectKind::OnFire),
                    before.effects.timed(EffectKind::OnFire)
                );
                for dose in &terra_core::brewing::recipe(id).unwrap().effects {
                    let actual = peer.vitals.effects.timed(dose.kind).unwrap();
                    assert_eq!(actual.level, dose.level);
                    assert_eq!(actual.remaining_seconds, dose.seconds);
                }
            } else {
                assert!(!peer.vitals.effects.contains(EffectKind::OnFire));
            }
            assert!(inventory.valid() && peer.vitals.valid());
        }
    }

    #[test]
    fn ordinary_food_still_cancels_on_damage_before_consuming_or_restoring_health() {
        let (mut app, mut peer) = crate::server::rgb_filter_tests::fixture();
        let item = ItemKind::Animal(terra_core::creatures::AnimalProduct::CookedBeef);
        let mut inventory = Inventory::default();
        assert_eq!(inventory.insert(item, 2), 0);
        app.world_mut()
            .resource_mut::<EditJournal>()
            .gameplay_mut()
            .inventories
            .insert(peer.uuid, inventory.clone());
        peer.vitals.hunger = 20.0;
        let mut sender = MessageSender::default();
        let started = Instant::now();
        pulse(&mut app, &mut peer, &mut sender, started);
        assert!(peer.consumption.is_some());
        pulse(
            &mut app,
            &mut peer,
            &mut sender,
            started + Duration::from_millis(250),
        );
        peer.survival.damage_epoch += 1;
        peer.vitals.health -= 5.0;
        let injured = peer.vitals;
        step(
            &mut app,
            &mut peer,
            &mut sender,
            started + Duration::from_millis(500),
        );
        assert!(peer.consumption.is_none());
        assert_eq!(peer.vitals, injured);
        assert_eq!(
            app.world().resource::<EditJournal>().gameplay().inventories[&peer.uuid],
            inventory
        );
    }

    #[test]
    fn timed_negative_potions_apply_typed_debuffs_without_erasing_existing_statuses() {
        for id in 7..=12 {
            let recipe = terra_core::brewing::recipe(id).unwrap();
            let dose = recipe.effects[0];
            let item = ItemKind::Potion(id);
            let (mut app, mut peer) = crate::server::rgb_filter_tests::fixture();
            let mut inventory = Inventory::default();
            inventory.insert(item, 2);
            app.world_mut()
                .resource_mut::<EditJournal>()
                .gameplay_mut()
                .inventories
                .insert(peer.uuid, inventory.clone());
            let other = if dose.kind == EffectKind::Poison {
                EffectKind::Slow
            } else {
                EffectKind::Poison
            };
            for kind in [EffectKind::NightVision, EffectKind::Strength, other] {
                peer.vitals.effects.apply(kind, 60.0, 1);
            }
            let before = peer.vitals;
            let started = Instant::now();
            let mut sender = MessageSender::default();
            pulse(&mut app, &mut peer, &mut sender, started);
            for millis in [250, 500, 750, 1000, 1250] {
                let now = started + Duration::from_millis(millis);
                pulse(&mut app, &mut peer, &mut sender, now);
                step(&mut app, &mut peer, &mut sender, now);
                assert!(!peer.vitals.effects.contains(dose.kind));
                assert_eq!(
                    app.world().resource::<EditJournal>().gameplay().inventories[&peer.uuid],
                    inventory
                );
            }
            let finished = started + Duration::from_millis(1500);
            pulse(&mut app, &mut peer, &mut sender, finished);
            step(&mut app, &mut peer, &mut sender, finished);
            assert!(peer.consumption.is_none());
            let actual = peer.vitals.effects.timed(dose.kind).unwrap();
            assert_eq!(
                (actual.kind, actual.level, actual.remaining_seconds),
                (dose.kind, dose.level, dose.seconds)
            );
            for kind in [EffectKind::NightVision, EffectKind::Strength, other] {
                assert_eq!(peer.vitals.effects.timed(kind), before.effects.timed(kind));
            }
            assert_eq!(peer.vitals.health, before.health);
            let inventory =
                &app.world().resource::<EditJournal>().gameplay().inventories[&peer.uuid];
            assert_eq!(inventory.count(item), 1);
            assert_eq!(inventory.count(ItemKind::GlassBottle), 1);
            assert!(inventory.valid() && peer.vitals.valid());
            let mut baseline = before;
            let mut affected = peer.vitals;
            baseline.advance_life(false, 0.25);
            affected.advance_life(false, 0.25);
            let modifiers = terra_core::effects::effect_definition(dose.kind).modifiers;
            if modifiers.damage_per_second > 0.0 {
                assert!(affected.health < baseline.health, "{dose:?}");
            }
            if modifiers.movement_multiplier < 1.0 {
                assert!(
                    affected.effects.movement_multiplier(false) <= modifiers.movement_multiplier
                );
            }
            if modifiers.hunger_per_second > 0.0 {
                assert!(
                    affected.hunger + affected.saturation < baseline.hunger + baseline.saturation
                );
            }
        }
    }

    #[test]
    fn mixed_potions_drink_at_full_hunger_preserve_other_effects_and_return_one_bottle() {
        let recipe = terra_core::brewing::recipe(6).unwrap();
        let item = recipe.output().item;
        let mut bag = Inventory::default();
        bag.insert(item, 2);
        let mut stats = SurvivalVitals::default();
        stats.effects.apply(EffectKind::Poison, 10.0, 2);
        let (next, after) = commit(&bag, stats, 0, item, GameMode::Survival).unwrap();
        assert_eq!(after.hunger, stats.hunger);
        assert_eq!(after.health, stats.health);
        assert_eq!(
            after.effects.timed(EffectKind::Poison),
            stats.effects.timed(EffectKind::Poison)
        );
        for dose in &recipe.effects {
            let actual = after.effects.timed(dose.kind).unwrap();
            assert_eq!(actual.level, dose.level);
            assert_eq!(actual.remaining_seconds, dose.seconds);
        }
        assert_eq!(next.count(item), 1);
        assert_eq!(next.count(ItemKind::GlassBottle), 1);
        assert!(next.valid() && after.valid());
        let (creative, after) = commit(&bag, stats, 0, item, GameMode::Creative).unwrap();
        assert_eq!(creative, bag);
        assert!(
            recipe
                .effects
                .iter()
                .all(|dose| after.effects.contains(dose.kind))
        );
        assert!(commit(&bag, stats, 0, item, GameMode::Spectator).is_none());
    }

    #[test]
    fn drinks_reject_full_bags_and_overflows_atomically_and_water_extinguishes_only_fire() {
        let item = ItemKind::WaterBottle;
        let mut bag = Inventory {
            slots: vec![
                ItemStack {
                    item: ItemKind::Block(3),
                    count: terra_core::STACK_LIMIT,
                };
                terra_core::INVENTORY_SLOTS
            ],
            ..Default::default()
        };
        bag.slots[0] = ItemStack { item, count: 2 };
        let before = bag.clone();
        let mut stats = SurvivalVitals::default();
        stats.effects.apply(EffectKind::OnFire, 10.0, 1);
        stats.effects.apply(EffectKind::Speed, 20.0, 2);
        assert!(commit(&bag, stats, 0, item, GameMode::Survival).is_none());
        assert_eq!(bag, before);
        bag.slots[0].count = 1;
        let (next, after) = commit(&bag, stats, 0, item, GameMode::Survival).unwrap();
        assert_eq!(
            next.slot(0),
            Some(ItemStack {
                item: ItemKind::GlassBottle,
                count: 1
            })
        );
        assert!(!after.effects.contains(EffectKind::OnFire));
        assert_eq!(
            after.effects.timed(EffectKind::Speed),
            stats.effects.timed(EffectKind::Speed)
        );
        bag.revision = u64::MAX - 1;
        assert!(commit(&bag, stats, 0, item, GameMode::Survival).is_none());
        assert!(commit(&before, stats, 9, item, GameMode::Survival).is_none());
    }
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
    #[test]
    fn actual_apple_consumption_leaves_a_non_edible_core_and_respects_full_inventory() {
        let mut bag = Inventory::default();
        bag.insert(ItemKind::Apple, 1);
        let food = terra_core::food(ItemKind::Apple).unwrap();
        let next = consume_stack(bag, 0, ItemKind::Apple, food).unwrap();
        assert_eq!(next.count(ItemKind::Apple), 0);
        assert_eq!(next.count(ItemKind::AppleCore), 1);
        assert!(terra_core::food(ItemKind::AppleCore).is_none());
        let bag = Inventory {
            slots: vec![
                terra_core::ItemStack {
                    item: ItemKind::Block(3),
                    count: terra_core::STACK_LIMIT
                };
                terra_core::INVENTORY_SLOTS
            ],
            ..Default::default()
        };
        let mut bag = bag;
        bag.slots[0] = terra_core::ItemStack {
            item: ItemKind::Apple,
            count: 2,
        };
        assert!(consume_stack(bag, 0, ItemKind::Apple, food).is_none());
    }
}
