use super::*;
use serde::Serialize;
use std::collections::VecDeque;
use tilt_ui::html_shared;

fn virtual_index(command: &str, index: Option<u8>) -> Option<u8> {
    match command {
        "slot" | "hotbar" => index.filter(|i| usize::from(*i) < terra_core::INVENTORY_SLOTS),
        "container" => index
            .filter(|i| usize::from(*i) < 2 * terra_core::containers::CHEST_SLOTS)
            .map(|i| i + terra_core::INVENTORY_SLOTS as u8),
        _ => None,
    }
}

fn virtual_slot(session: &ClientSession, index: u8) -> Option<terra_core::ItemStack> {
    let index = usize::from(index);
    if index < terra_core::INVENTORY_SLOTS {
        return session.inventory.slot(index);
    }
    let local = index - terra_core::INVENTORY_SLOTS;
    session
        .container
        .as_ref()?
        .halves
        .get(local / terra_core::containers::CHEST_SLOTS)?
        .inventory
        .slot(local % terra_core::containers::CHEST_SLOTS)
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SlotTarget {
    command: String,
    index: Option<u8>,
    item: String,
}

enum PointerGesture {
    Distribute { item: ItemKind, slots: Vec<u8> },
    Collect { item: ItemKind, visited: Vec<u8> },
    QuickMove { visited: Vec<u8> },
    PlaceOne { item: ItemKind, entered: Option<u8> },
}

impl PointerGesture {
    fn button(&self) -> MouseButton {
        if matches!(self, Self::PlaceOne { .. }) {
            MouseButton::Right
        } else {
            MouseButton::Left
        }
    }
}

#[derive(Resource, Default)]
pub(super) struct InventoryPointer {
    hovered: Option<SlotTarget>,
    gesture: Option<PointerGesture>,
    last_primary: Option<(u8, f64)>,
}

pub(super) struct DragPreview {
    pub(super) player: terra_core::Inventory,
    pub(super) halves: Vec<terra_core::Inventory>,
}

impl InventoryPointer {
    pub(super) fn cancel_gesture(&mut self) {
        self.gesture = None;
        self.last_primary = None;
    }

    pub(super) fn catalog_item(&self) -> Option<&str> {
        self.hovered
            .as_ref()
            .filter(|hovered| hovered.command == "catalog" && !hovered.item.is_empty())
            .map(|hovered| hovered.item.as_str())
    }

    pub(super) fn preview(
        &self,
        session: &ClientSession,
        reservations: &crafting::CraftingState,
        queue: Option<&InventoryQueue>,
    ) -> Option<DragPreview> {
        let (item, destinations) = match self.gesture.as_ref()? {
            PointerGesture::Distribute { item, slots } => (item, Some(slots)),
            PointerGesture::PlaceOne { item, .. } => (item, None),
            _ => return None,
        };
        if session.inventory.cursor?.item != *item {
            return None;
        }
        let mut preview = DragPreview {
            player: session.inventory.clone(),
            halves: session
                .container
                .as_ref()
                .map_or_else(Vec::new, |container| {
                    container
                        .halves
                        .iter()
                        .map(|half| half.inventory.clone())
                        .collect()
                }),
        };
        // Run the same bounded gesture against a view copy; the server remains the owner.
        let Some(slots) = destinations else {
            let queue = queue?;
            let waiting = queue
                .waiting
                .as_ref()
                .filter(|waiting| waiting.response == session.inventory_response)
                .map(|waiting| &waiting.action);
            let mut changed = false;
            for queued in waiting.into_iter().chain(queue.pending.iter()) {
                if let ServerAction::Click {
                    slot,
                    secondary: true,
                } = queued.action
                    && context_matches(*queued, session)
                    && queued.permitted(&preview.player)
                {
                    changed |= if preview.halves.is_empty() {
                        preview.player.click(usize::from(slot), true)
                    } else if session
                        .container
                        .as_ref()
                        .is_some_and(|c| c.crusher.is_some())
                    {
                        terra_core::crusher::apply_gesture_reserved(
                            &mut preview.player,
                            &mut preview.halves[0],
                            terra_core::containers::ContainerGesture::Click {
                                slot,
                                secondary: true,
                            },
                            crafting::reserved_counts(reservations),
                        )
                    } else if session
                        .container
                        .as_ref()
                        .is_some_and(|c| c.furnace.is_some())
                    {
                        terra_core::furnace::apply_gesture_reserved(
                            &mut preview.player,
                            &mut preview.halves[0],
                            terra_core::containers::ContainerGesture::Click {
                                slot,
                                secondary: true,
                            },
                            crafting::reserved_counts(reservations),
                        )
                    } else {
                        terra_core::containers::apply_gesture_reserved(
                            &mut preview.player,
                            &mut preview.halves,
                            terra_core::containers::ContainerGesture::Click {
                                slot,
                                secondary: true,
                            },
                            crafting::reserved_counts(reservations),
                        )
                    };
                }
            }
            return changed.then_some(preview);
        };
        let changed = if preview.halves.is_empty() {
            let mut targets = [None; terra_core::INVENTORY_SLOTS];
            for (target, slot) in targets.iter_mut().zip(slots) {
                *target = Some(*slot);
            }
            preview.player.drag(targets, false)
        } else if session
            .container
            .as_ref()
            .is_some_and(|c| c.crusher.is_some())
        {
            terra_core::crusher::apply_gesture_reserved(
                &mut preview.player,
                &mut preview.halves[0],
                terra_core::containers::ContainerGesture::Drag {
                    slots: slots.clone(),
                    secondary: false,
                },
                crafting::reserved_counts(reservations),
            )
        } else if session
            .container
            .as_ref()
            .is_some_and(|c| c.furnace.is_some())
        {
            terra_core::furnace::apply_gesture_reserved(
                &mut preview.player,
                &mut preview.halves[0],
                terra_core::containers::ContainerGesture::Drag {
                    slots: slots.clone(),
                    secondary: false,
                },
                crafting::reserved_counts(reservations),
            )
        } else {
            terra_core::containers::apply_gesture_reserved(
                &mut preview.player,
                &mut preview.halves,
                terra_core::containers::ContainerGesture::Drag {
                    slots: slots.clone(),
                    secondary: false,
                },
                crafting::reserved_counts(reservations),
            )
        };
        changed.then_some(preview)
    }

    pub(super) fn hover(
        &mut self,
        command: &str,
        index: Option<u8>,
        item: &str,
        shift: bool,
    ) -> Option<QueuedAction> {
        self.hovered = Some(SlotTarget {
            command: command.into(),
            index,
            item: item.into(),
        });
        if !matches!(command, "slot" | "hotbar" | "container") {
            if let Some(PointerGesture::PlaceOne { entered, .. }) = &mut self.gesture {
                *entered = None;
            }
            return None;
        }
        if !shift && matches!(self.gesture, Some(PointerGesture::QuickMove { .. })) {
            self.gesture = None;
        }
        let slot = virtual_index(command, index)?;
        match self.gesture.as_mut()? {
            PointerGesture::Distribute { slots, .. } if !slots.contains(&slot) => slots.push(slot),
            PointerGesture::Collect {
                item: matching,
                visited,
            } if item_id(*matching) == item && !visited.contains(&slot) => {
                visited.push(slot);
                return Some(QueuedAction::held(
                    ServerAction::CollectFrom { slot },
                    *matching,
                ));
            }
            PointerGesture::QuickMove { visited }
                if !item.is_empty() && !visited.contains(&slot) =>
            {
                visited.push(slot);
                return Some(QueuedAction::direct(ServerAction::QuickMove { slot }));
            }
            PointerGesture::PlaceOne {
                item: matching,
                entered,
            } => {
                let previous = entered.replace(slot);
                if previous != Some(slot) && (item.is_empty() || item_id(*matching) == item) {
                    return Some(QueuedAction::held(
                        ServerAction::Click {
                            slot,
                            secondary: true,
                        },
                        *matching,
                    ));
                }
            }
            _ => {}
        }
        None
    }

    pub(super) fn leave(&mut self, command: &str, index: Option<u8>, _item: &str) {
        if self
            .hovered
            .as_ref()
            .is_some_and(|hovered| hovered.command == command && hovered.index == index)
        {
            self.hovered = None;
            if let Some(PointerGesture::PlaceOne { entered, .. }) = &mut self.gesture {
                *entered = None;
            }
        }
    }
}

#[derive(Resource, Default)]
pub(super) struct InventoryQueue {
    pub(super) owner: Option<[i64; 3]>,
    pending: VecDeque<QueuedAction>,
    waiting: Option<PendingResponse>,
}

struct PendingResponse {
    response: u64,
    action: QueuedAction,
    retries: u8,
}

impl InventoryQueue {
    pub(super) fn push(&mut self, action: ServerAction) {
        self.push_queued(QueuedAction::direct(action));
    }

    pub(super) fn push_queued(&mut self, mut action: QueuedAction) {
        action.container = self.owner;
        if self.pending.len() < 64 {
            self.pending.push_back(action);
        }
    }

    pub(super) fn clear(&mut self) {
        self.pending.clear();
        self.waiting = None;
    }

    pub(super) fn is_empty(&self) -> bool {
        self.pending.is_empty() && self.waiting.is_none()
    }

    pub(super) fn has_open_container(&self) -> bool {
        self.waiting.as_ref().is_some_and(|waiting| {
            matches!(waiting.action.action, ServerAction::OpenContainer { .. })
        }) || self
            .pending
            .iter()
            .any(|queued| matches!(queued.action, ServerAction::OpenContainer { .. }))
    }

    pub(super) fn has_container_lifecycle(&self) -> bool {
        let lifecycle = |action| {
            matches!(
                action,
                ServerAction::OpenContainer { .. } | ServerAction::CloseContainer
            )
        };
        self.waiting
            .as_ref()
            .is_some_and(|waiting| lifecycle(waiting.action.action))
            || self.pending.iter().any(|queued| lifecycle(queued.action))
    }
}

#[derive(Clone, Copy)]
enum CursorGuard {
    Any,
    Empty,
    Holding(ItemKind),
}

#[derive(Clone, Copy)]
pub(super) struct QueuedAction {
    container: Option<[i64; 3]>,
    action: ServerAction,
    cursor: CursorGuard,
}

impl QueuedAction {
    pub(super) fn protected(mut self, reservations: &crafting::CraftingState) -> Self {
        self.action = protect(self.action, reservations);
        self
    }
    fn direct(action: ServerAction) -> Self {
        Self {
            container: None,
            action,
            cursor: CursorGuard::Any,
        }
    }
    pub(super) fn held(action: ServerAction, item: ItemKind) -> Self {
        Self {
            container: None,
            action,
            cursor: CursorGuard::Holding(item),
        }
    }
    fn permitted(self, inventory: &terra_core::Inventory) -> bool {
        match self.cursor {
            CursorGuard::Any => true,
            CursorGuard::Empty => inventory.cursor.is_none(),
            CursorGuard::Holding(item) => inventory.cursor.is_some_and(|stack| stack.item == item),
        }
    }
}

pub(super) fn protect(
    action: ServerAction,
    reservations: &crafting::CraftingState,
) -> ServerAction {
    use terra_network::protocol::inventory::ProtectedMoveKind;
    let reserved = crafting::reserved_counts(reservations);
    if reserved.iter().all(|count| *count == 0) {
        return action;
    }
    if let ServerAction::Move { from, to, count } = action {
        return ServerAction::Move {
            from,
            to,
            count: if reserved.get(usize::from(to)).copied().unwrap_or(0) > 0 {
                0
            } else {
                count.saturating_sub(reserved.get(usize::from(from)).copied().unwrap_or(0))
            },
        };
    }
    let (slot, kind) = match action {
        ServerAction::QuickMove { slot } => (slot, ProtectedMoveKind::QuickMove),
        ServerAction::Collect { slot } => (slot, ProtectedMoveKind::Collect),
        ServerAction::CollectFrom { slot } => (slot, ProtectedMoveKind::CollectFrom),
        ServerAction::Wheel {
            slot,
            to_cursor: true,
        } => (slot, ProtectedMoveKind::WheelToCursor),
        _ => return action,
    };
    ServerAction::ProtectedMove {
        slot,
        kind,
        reserved,
    }
}

pub(super) struct Press<'a> {
    pub command: &'a str,
    pub index: Option<u8>,
    pub item: &'a str,
    pub secondary: bool,
    pub shift: bool,
}

pub(super) fn press(
    pointer: &mut InventoryPointer,
    state: &mut InventoryState,
    selection: &mut InventorySelection,
    session: &ClientSession,
    now: f64,
    press: Press,
) {
    let Press {
        command,
        index,
        item,
        secondary,
        shift,
    } = press;
    pointer.gesture = None;
    let _ = pointer.hover(command, index, item, shift);
    if command == "catalog" {
        if session.game_mode != GameMode::Creative {
            return;
        }
        if let Some(item) =
            available_items(GameMode::Creative, &session.inventory, &selection.palette)
                .into_iter()
                .find(|entry| item_id(*entry) == item)
        {
            state.focused = Some(item);
            selection.request(
                session,
                ServerAction::CreativeQuickMove {
                    item,
                    count: if shift { item.max_stack() } else { 1 },
                },
            );
        }
        return;
    }
    if command == "equipment" {
        let Some(slot) =
            index.filter(|index| usize::from(*index) < terra_core::EquipmentSlot::ALL.len())
        else {
            return;
        };
        state.source = None;
        state.focused = session
            .stats
            .equipment
            .slot(usize::from(slot))
            .map(|stack| stack.item)
            .or_else(|| session.inventory.cursor.map(|stack| stack.item));
        selection.request(
            session,
            if shift {
                ServerAction::EquipmentQuickMove { slot }
            } else {
                ServerAction::EquipmentClick { slot, secondary }
            },
        );
        return;
    }
    let Some(slot) = virtual_index(command, index) else {
        return;
    };
    if !matches!(command, "slot" | "hotbar" | "container") {
        return;
    }
    let stack = virtual_slot(session, slot);
    let available = stack.map_or(0, |s| {
        crafting::available_count(&selection.reservations, usize::from(slot), s.count)
    });
    let reserved = stack.map_or(0, |s| s.count.saturating_sub(available));
    if reserved > 0 {
        if available == 0 && session.inventory.cursor.is_none() {
            return;
        }
        if let Some(cursor) = session.inventory.cursor {
            if stack.is_some_and(|s| s.item != cursor.item) {
                return;
            }
        } else if shift {
            if session.container.is_some() {
                selection.request(session, ServerAction::QuickMove { slot });
                return;
            }
            let mut left = available;
            let range = if slot < 9 {
                9..terra_core::INVENTORY_SLOTS
            } else {
                0..9
            };
            for target in range {
                let capacity = match session.inventory.slot(target) {
                    Some(target) if Some(target.item) == stack.map(|s| s.item) => {
                        STACK_LIMIT - target.count
                    }
                    None => STACK_LIMIT,
                    _ => 0,
                };
                let moved = left.min(capacity);
                if moved > 0 {
                    selection.queue.push(ServerAction::Move {
                        from: slot,
                        to: target as u8,
                        count: moved,
                    });
                    left -= moved;
                }
                if left == 0 {
                    break;
                }
            }
            return;
        } else {
            selection.request(
                session,
                ServerAction::TakeToCursor {
                    slot,
                    count: if secondary {
                        available.div_ceil(2)
                    } else {
                        available
                    },
                },
            );
            return;
        }
    }
    state.source = Some(slot);
    state.focused = virtual_slot(session, slot)
        .map(|stack| stack.item)
        .or_else(|| session.inventory.cursor.map(|stack| stack.item));
    if shift {
        selection.request(session, ServerAction::QuickMove { slot });
        if !secondary {
            pointer.gesture = Some(PointerGesture::QuickMove {
                visited: vec![slot],
            });
        }
        return;
    }
    let double = !secondary
        && pointer
            .last_primary
            .is_some_and(|(last, at)| last == slot && now - at <= 0.3);
    if !secondary {
        pointer.last_primary = Some((slot, now));
    }
    if double {
        pointer.last_primary = None;
        selection.request(session, ServerAction::Collect { slot });
    } else if let Some(cursor) = session.inventory.cursor {
        if secondary {
            selection.queue.push_queued(QueuedAction::held(
                ServerAction::Click {
                    slot,
                    secondary: true,
                },
                cursor.item,
            ));
            pointer.gesture = session
                .inventory
                .cursor
                .map(|stack| PointerGesture::PlaceOne {
                    item: stack.item,
                    entered: Some(slot),
                });
        } else {
            pointer.gesture = Some(PointerGesture::Distribute {
                item: cursor.item,
                slots: vec![slot],
            });
        }
    } else {
        selection.request(session, ServerAction::Click { slot, secondary });
        if !secondary && let Some(stack) = virtual_slot(session, slot) {
            pointer.gesture = Some(PointerGesture::Collect {
                item: stack.item,
                visited: vec![slot],
            });
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn finish_gesture(
    state: Res<InventoryState>,
    input: Res<ButtonInput<MouseButton>>,
    mut wheel: MessageReader<MouseWheel>,
    mut pointer: ResMut<InventoryPointer>,
    mut queue: ResMut<InventoryQueue>,
    session: Option<Res<ClientSession>>,
    reservations: Res<crafting::CraftingState>,
    keys: Res<ButtonInput<KeyCode>>,
) {
    if !state.open || crafting::busy(&reservations) {
        wheel.clear();
        pointer.gesture = None;
        return;
    }
    if pointer
        .gesture
        .as_ref()
        .is_some_and(|gesture| !input.pressed(gesture.button()))
        && let Some(PointerGesture::Distribute {
            item,
            slots: destinations,
        }) = pointer.gesture.take()
    {
        if let [slot] = destinations.as_slice() {
            queue.push_queued(QueuedAction::held(
                ServerAction::Click {
                    slot: *slot,
                    secondary: false,
                },
                item,
            ));
        } else if !destinations.is_empty()
            && session.as_ref().is_some_and(|s| s.container.is_some())
        {
            let mut slots = [None; 32];
            let mut extra = [None; 31];
            for (entry, slot) in slots.iter_mut().chain(extra.iter_mut()).zip(destinations) {
                *entry = Some(slot);
            }
            queue.push_queued(QueuedAction::held(
                ServerAction::ViewDrag {
                    slots,
                    extra,
                    secondary: false,
                },
                item,
            ));
        } else if !destinations.is_empty() {
            let mut slots = [None; terra_core::INVENTORY_SLOTS];
            for (destination, slot) in slots.iter_mut().zip(destinations) {
                *destination = Some(slot);
            }
            queue.push_queued(QueuedAction::held(
                ServerAction::Drag {
                    slots,
                    secondary: false,
                },
                item,
            ));
        }
    }
    let delta: f32 = wheel.read().map(|event| event.y).sum();
    if delta.is_finite()
        && (keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight))
        && delta.abs() >= 0.1
        && let Some(hovered) = &pointer.hovered
        && let Some(slot) = virtual_index(&hovered.command, hovered.index)
    {
        for _ in 0..(delta.abs().round() as usize).clamp(1, 8) {
            let cursor = session
                .as_ref()
                .and_then(|session| session.inventory.cursor)
                .map_or(CursorGuard::Empty, |stack| CursorGuard::Holding(stack.item));
            queue.push_queued(
                QueuedAction {
                    container: None,
                    action: ServerAction::Wheel {
                        slot,
                        to_cursor: delta > 0.0,
                    },
                    cursor,
                }
                .protected(&reservations),
            );
        }
    }
}

pub(super) fn send_requests(
    session: Option<Res<ClientSession>>,
    mut queue: ResMut<InventoryQueue>,
    mut requests: MessageWriter<InventoryRequest>,
    mut containers: MessageWriter<ContainerRequest>,
    reservations: Res<crafting::CraftingState>,
) {
    let Some(session) = session.filter(|session| session.ready && !session.teleporting) else {
        queue.clear();
        return;
    };
    if let Some(waiting) = &queue.waiting {
        if session.inventory_response == waiting.response {
            return;
        }
        if session.last_inventory
            == Some(terra_network::protocol::inventory::InventoryOutcome::Conflict)
            && waiting.retries < 2
            && waiting.action.permitted(&session.inventory)
            && !empty_action(waiting.action.action, &session)
            && context_matches(waiting.action, &session)
        {
            let action = waiting.action;
            let retries = waiting.retries + 1;
            dispatch(
                action,
                &session,
                &reservations,
                &mut requests,
                &mut containers,
            );
            queue.waiting = Some(PendingResponse {
                response: session.inventory_response,
                action,
                retries,
            });
            return;
        }
        queue.waiting = None;
    }
    while let Some(action) = queue.pending.pop_front() {
        if !action.permitted(&session.inventory)
            || empty_action(action.action, &session)
            || !context_matches(action, &session)
        {
            continue;
        }
        dispatch(
            action,
            &session,
            &reservations,
            &mut requests,
            &mut containers,
        );
        queue.waiting = Some(PendingResponse {
            response: session.inventory_response,
            action,
            retries: 0,
        });
        break;
    }
}

fn container_gesture(action: ServerAction) -> Option<terra_core::containers::ContainerGesture> {
    use terra_core::containers::ContainerGesture as Gesture;
    use terra_network::protocol::inventory::ProtectedMoveKind;
    Some(match action {
        ServerAction::Click { slot, secondary } => Gesture::Click { slot, secondary },
        ServerAction::Drag { slots, secondary } => Gesture::Drag {
            slots: slots.into_iter().flatten().collect(),
            secondary,
        },
        ServerAction::ViewDrag {
            slots,
            extra,
            secondary,
        } => Gesture::Drag {
            slots: slots.into_iter().chain(extra).flatten().collect(),
            secondary,
        },
        ServerAction::QuickMove { slot } => Gesture::QuickMove { slot },
        ServerAction::Collect { slot } => Gesture::Collect { slot },
        ServerAction::CollectFrom { slot } => Gesture::CollectFrom { slot },
        ServerAction::Wheel { slot, to_cursor } => Gesture::Wheel { slot, to_cursor },
        ServerAction::ProtectedMove { slot, kind, .. } => match kind {
            ProtectedMoveKind::QuickMove => Gesture::QuickMove { slot },
            ProtectedMoveKind::Collect => Gesture::Collect { slot },
            ProtectedMoveKind::CollectFrom => Gesture::CollectFrom { slot },
            ProtectedMoveKind::WheelToCursor => Gesture::Wheel {
                slot,
                to_cursor: true,
            },
        },
        _ => return None,
    })
}

fn context_matches(action: QueuedAction, session: &ClientSession) -> bool {
    if let ServerAction::LoadCrushingRecipe { station, .. } = action.action {
        return session
            .container
            .as_ref()
            .is_some_and(|container| container.anchor == station && container.crusher.is_some());
    }
    if let ServerAction::LoadSmeltingRecipe { station, .. } = action.action {
        return session
            .container
            .as_ref()
            .is_some_and(|container| container.anchor == station && container.furnace.is_some());
    }
    container_gesture(action.action).is_none()
        || action.container == session.container.as_ref().map(|container| container.anchor)
}

fn dispatch(
    queued: QueuedAction,
    session: &ClientSession,
    reservations: &crafting::CraftingState,
    requests: &mut MessageWriter<InventoryRequest>,
    containers: &mut MessageWriter<ContainerRequest>,
) {
    if let (Some(anchor), Some(action), Some(snapshot)) = (
        queued.container,
        container_gesture(queued.action),
        session.container.as_ref(),
    ) {
        containers.write(ContainerRequest(
            terra_network::protocol::inventory::ContainerCommand {
                inventory_revision: session.inventory.revision,
                reserved: crafting::reserved_counts(reservations),
                anchor,
                revisions: snapshot
                    .halves
                    .iter()
                    .map(|half| half.inventory.revision)
                    .collect(),
                action,
            },
        ));
    } else {
        requests.write(InventoryRequest(
            terra_network::protocol::inventory::InventoryCommand {
                revision: session.inventory.revision,
                action: queued.action,
            },
        ));
    }
}

pub(super) fn suppress_transfer_scroll(
    state: Res<InventoryState>,
    keys: Res<ButtonInput<KeyCode>>,
    events: Option<ResMut<Messages<bevy::picking::events::Pointer<bevy::picking::events::Scroll>>>>,
) {
    if state.open
        && (keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight))
        && let Some(mut events) = events
    {
        events.clear();
    }
}

fn empty_action(action: ServerAction, session: &ClientSession) -> bool {
    let inventory = &session.inventory;
    match action {
        ServerAction::Click { slot, .. } => {
            inventory.cursor.is_none() && virtual_slot(session, slot).is_none()
        }
        ServerAction::DropSlot { slot, .. } => inventory.slot(usize::from(slot)).is_none(),
        ServerAction::DropCursor { .. } | ServerAction::ReturnCursor => inventory.cursor.is_none(),
        ServerAction::CreativeTrash { slot } => match slot {
            Some(slot) => inventory.slot(usize::from(slot)).is_none(),
            None => inventory.cursor.is_none(),
        },
        ServerAction::CreativeClear => {
            inventory.cursor.is_none()
                && inventory.occupied_slots() == 0
                && session.stats.equipment.iter().next().is_none()
        }
        _ => false,
    }
}

#[html_shared]
#[derive(Resource, Default, Debug, Serialize, PartialEq)]
pub(super) struct InventoryCursor {
    pub visible: bool,
    pub preview: String,
    pub count: String,
    pub position: String,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn cursor_view(
    state: Res<InventoryState>,
    context: InventoryContext,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    ui_scale: Option<Res<bevy::ui::UiScale>>,
    pointer: Option<Res<InventoryPointer>>,
    crafting: Option<Res<crafting::CraftingState>>,
    queue: Option<Res<InventoryQueue>>,
    mut cursor: ResMut<InventoryCursor>,
) {
    let session = context.session.as_ref();
    let stack = session.and_then(|session| {
        if let Some(preview) = pointer
            .as_ref()
            .zip(crafting.as_ref())
            .and_then(|(pointer, crafting)| pointer.preview(session, crafting, queue.as_deref()))
        {
            return preview.player.cursor;
        }
        let mut stack = session.inventory.cursor?;
        if let Some((_, _, deposited)) = crafting
            .as_ref()
            .and_then(|state| crafting::drag_preview(state, session))
        {
            stack.count = stack.count.saturating_sub(deposited);
        }
        (stack.count > 0).then_some(stack)
    });
    let position = windows.iter().next().and_then(Window::cursor_position);
    let scale = ui_scale.map_or(1.0, |scale| scale.0).max(0.1);
    let position = position.map(|position| position / scale + Vec2::splat(8.0));
    let next = InventoryCursor {
        visible: state.open && context.available() && stack.is_some() && position.is_some(),
        preview: stack
            .map(|stack| preview_key(stack.item))
            .unwrap_or_default(),
        count: stack
            .map(|stack| stack.count.to_string())
            .unwrap_or_default(),
        position: position
            .map(|position| format!("left: {}px; top: {}px;", position.x, position.y))
            .unwrap_or_default(),
    };
    if *cursor != next {
        *cursor = next;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drag_previews_show_each_destination_and_remaining_cursor_without_committing() {
        let item = ItemKind::Block(3);
        let mut session = ClientSession::default();
        session.inventory.cursor = Some(terra_core::ItemStack { item, count: 17 });
        let original = session.inventory.clone();
        let mut pointer = InventoryPointer {
            gesture: Some(PointerGesture::Distribute {
                item,
                slots: vec![10, 11, 12],
            }),
            ..default()
        };
        let reservations = crafting::CraftingState::default();
        let preview = pointer.preview(&session, &reservations, None).unwrap();
        for index in [10, 11, 12] {
            assert_eq!(preview.player.slot(index).unwrap().count, 5);
        }
        assert_eq!(preview.player.cursor.unwrap().count, 2);
        assert_eq!(session.inventory, original);

        pointer.gesture = Some(PointerGesture::PlaceOne {
            item,
            entered: Some(11),
        });
        let mut queue = InventoryQueue::default();
        queue.push(ServerAction::Click {
            slot: 10,
            secondary: true,
        });
        queue.push(ServerAction::Click {
            slot: 11,
            secondary: true,
        });
        let preview = pointer
            .preview(&session, &reservations, Some(&queue))
            .unwrap();
        assert_eq!(preview.player.slot(10).unwrap().count, 1);
        assert_eq!(preview.player.slot(11).unwrap().count, 1);
        assert_eq!(preview.player.cursor.unwrap().count, 15);
        assert_eq!(session.inventory, original);
    }

    #[test]
    fn chest_drag_preview_keeps_physical_half_indices_and_never_changes_the_snapshot() {
        use terra_network::protocol::inventory::{ContainerHalf, ContainerSnapshot};
        let item = ItemKind::Block(3);
        let mut session = ClientSession::default();
        session.inventory.cursor = Some(terra_core::ItemStack { item, count: 9 });
        let anchor = [-4, 60, 8];
        let snapshot = ContainerSnapshot {
            anchor,
            workbench: None,
            furnace: None,
            crusher: None,
            halves: [anchor, [-3, 60, 8]]
                .into_iter()
                .map(|position| ContainerHalf {
                    position,
                    inventory: default(),
                })
                .collect(),
        };
        session.container = Some(snapshot.clone());
        let pointer = InventoryPointer {
            gesture: Some(PointerGesture::Distribute {
                item,
                slots: vec![27, 45],
            }),
            ..default()
        };
        let preview = pointer
            .preview(&session, &crafting::CraftingState::default(), None)
            .unwrap();
        assert_eq!(preview.halves[0].slot(0).unwrap().count, 4);
        assert_eq!(preview.halves[1].slot(0).unwrap().count, 4);
        assert_eq!(preview.player.cursor.unwrap().count, 1);
        assert_eq!(session.container, Some(snapshot));
    }

    #[test]
    fn furnace_drag_preview_uses_restricted_slots_without_mutating_server_state() {
        use terra_network::protocol::inventory::{ContainerHalf, ContainerSnapshot};
        let recipe = terra_core::furnace::recipes().first().unwrap();
        let item = recipe.input;
        let mut session = ClientSession::default();
        session.inventory.cursor = Some(terra_core::ItemStack { item, count: 9 });
        let anchor = [4, 60, 8];
        session.container = Some(ContainerSnapshot {
            anchor,
            workbench: None,
            furnace: Some(terra_core::furnace::FurnaceView {
                progress: 0.0,
                burn_remaining: 0.0,
                burn_total: 0.0,
                recipe: None,
                smoke_cells: 0,
                vented: false,
            }),
            crusher: None,
            halves: vec![ContainerHalf {
                position: anchor,
                inventory: default(),
            }],
        });
        let before = session.container.clone();
        let pointer = InventoryPointer {
            gesture: Some(PointerGesture::Distribute {
                item,
                slots: vec![terra_core::INVENTORY_SLOTS as u8],
            }),
            ..default()
        };
        let preview = pointer
            .preview(&session, &crafting::CraftingState::default(), None)
            .unwrap();
        assert_eq!(preview.halves[0].slot(0).unwrap().count, 9);
        assert!(preview.player.cursor.is_none());
        assert_eq!(session.container, before);
        assert_eq!(session.inventory.cursor.unwrap().count, 9);
        let pointer = InventoryPointer {
            gesture: Some(PointerGesture::Distribute {
                item,
                slots: vec![terra_core::INVENTORY_SLOTS as u8 + 2],
            }),
            ..default()
        };
        assert!(
            pointer
                .preview(&session, &crafting::CraftingState::default(), None)
                .is_none()
        );
    }
}
