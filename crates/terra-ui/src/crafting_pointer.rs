//! MouseTweaks gestures over virtual cells, backed by authoritative inventory quantities.
use super::*;
use std::collections::VecDeque;

#[derive(Default)]
pub(super) struct GridPointer {
    hovered: Option<usize>,
    gesture: Option<Gesture>,
    edits: VecDeque<Edit>,
    last_primary: Option<(usize, f64)>,
}
enum Gesture {
    Distribute {
        item: ItemKind,
        cells: Vec<usize>,
    },
    PlaceOne {
        item: ItemKind,
        entered: Option<usize>,
    },
    Collect {
        item: ItemKind,
        visited: Vec<usize>,
    },
    QuickMove {
        visited: Vec<usize>,
    },
}
enum Edit {
    Deposit {
        item: ItemKind,
        cells: Vec<usize>,
        one: bool,
    },
    Take {
        item: ItemKind,
        cells: Vec<usize>,
        amount: Amount,
    },
    Release {
        index: usize,
        one: bool,
    },
}
#[derive(Clone, Copy)]
enum Amount {
    All,
    Half,
    One,
}

impl GridPointer {
    fn push(&mut self, edit: Edit) {
        if self.edits.len() < 64 {
            self.edits.push_back(edit);
        }
    }
}
fn grid_index(command: &str, index: Option<u8>, inventory: &InventoryState) -> Option<usize> {
    matches!(command, "craft-cell" | "bench-cell")
        .then_some(index)
        .flatten()
        .map(usize::from)
        .filter(|&i| i < size(inventory).pow(2))
}
fn item_at(state: &CraftingState, session: &ClientSession, index: usize) -> Option<ItemKind> {
    state.cells[index]
        .and_then(|c| session.inventory.slot(usize::from(c.slot)))
        .map(|s| s.item)
}

pub(super) fn handle(
    state: &mut CraftingState,
    inventory: &InventoryState,
    session: &ClientSession,
    event: &InventoryAction,
    shift: bool,
    now: f64,
) -> bool {
    match event {
        InventoryAction::Pointer {
            command,
            index,
            secondary,
            ..
        } => {
            let Some(index) = grid_index(command, *index, inventory) else {
                state.pointer.gesture = None;
                return false;
            };
            if state.recipe_open {
                return true;
            }
            state.pointer.hovered = Some(index);
            state.pointer.gesture = None;
            let item = item_at(state, session, index);
            if shift {
                state.pointer.push(Edit::Release { index, one: false });
                if !secondary {
                    state.pointer.gesture = Some(Gesture::QuickMove {
                        visited: vec![index],
                    });
                }
                return true;
            }
            let double = !secondary
                && state
                    .pointer
                    .last_primary
                    .is_some_and(|(last, at)| last == index && now - at <= 0.3);
            if !secondary {
                state.pointer.last_primary = Some((index, now));
            }
            if double && let Some(item) = session.inventory.cursor.map(|s| s.item).or(item) {
                let cells = (0..size(inventory).pow(2))
                    .filter(|&i| item_at(state, session, i) == Some(item))
                    .collect();
                state.pointer.push(Edit::Take {
                    item,
                    cells,
                    amount: Amount::All,
                });
                state.pointer.last_primary = None;
            } else if let Some(cursor) = session.inventory.cursor {
                if *secondary {
                    state.pointer.push(Edit::Deposit {
                        item: cursor.item,
                        cells: vec![index],
                        one: true,
                    });
                    state.pointer.gesture = Some(Gesture::PlaceOne {
                        item: cursor.item,
                        entered: Some(index),
                    });
                } else {
                    state.pointer.gesture = Some(Gesture::Distribute {
                        item: cursor.item,
                        cells: vec![index],
                    });
                }
            } else if let Some(item) = item {
                state.pointer.push(Edit::Take {
                    item,
                    cells: vec![index],
                    amount: if *secondary {
                        Amount::Half
                    } else {
                        Amount::All
                    },
                });
                if !secondary {
                    state.pointer.gesture = Some(Gesture::Collect {
                        item,
                        visited: vec![index],
                    });
                }
            }
            true
        }
        InventoryAction::Hover { command, index, .. } => {
            let index = grid_index(command, *index, inventory);
            state.pointer.hovered = index;
            if state.recipe_open {
                return false;
            }
            let Some(index) = index else {
                if let Some(Gesture::PlaceOne { entered, .. }) = &mut state.pointer.gesture {
                    *entered = None;
                }
                return false;
            };
            let matching = item_at(state, session, index);
            let edit = match &mut state.pointer.gesture {
                Some(Gesture::Distribute { cells, .. }) => {
                    if !cells.contains(&index) {
                        cells.push(index);
                    }
                    None
                }
                Some(Gesture::PlaceOne { item, entered }) => {
                    let previous = entered.replace(index);
                    (previous != Some(index)).then(|| Edit::Deposit {
                        item: *item,
                        cells: vec![index],
                        one: true,
                    })
                }
                Some(Gesture::Collect { item, visited })
                    if matching == Some(*item) && !visited.contains(&index) =>
                {
                    visited.push(index);
                    Some(Edit::Take {
                        item: *item,
                        cells: vec![index],
                        amount: Amount::All,
                    })
                }
                Some(Gesture::QuickMove { visited }) if shift && !visited.contains(&index) => {
                    visited.push(index);
                    Some(Edit::Release { index, one: false })
                }
                _ => None,
            };
            if let Some(edit) = edit {
                state.pointer.push(edit);
            }
            false
        }
        InventoryAction::Leave { command, index, .. } => {
            if grid_index(command, *index, inventory) == state.pointer.hovered {
                state.pointer.hovered = None;
                if let Some(Gesture::PlaceOne { entered, .. }) = &mut state.pointer.gesture {
                    *entered = None;
                }
            }
            false
        }
        _ => false,
    }
}

pub(super) fn finish(
    state: &mut CraftingState,
    session: &ClientSession,
    queue: &mut pointer::InventoryQueue,
    mouse: &ButtonInput<MouseButton>,
    wheel: &mut MessageReader<MouseWheel>,
    transfer_wheel: bool,
) {
    let delta: f32 = wheel.read().map(|event| event.y).sum();
    if state.recipe_open {
        state.pointer = default();
        return;
    }
    let button = if matches!(state.pointer.gesture, Some(Gesture::PlaceOne { .. })) {
        MouseButton::Right
    } else {
        MouseButton::Left
    };
    if !mouse.pressed(button)
        && let Some(Gesture::Distribute { item, cells }) = state.pointer.gesture.take()
    {
        state.pointer.push(Edit::Deposit {
            item,
            cells,
            one: false,
        });
    }
    if delta.is_finite()
        && transfer_wheel
        && delta.abs() >= 0.1
        && let Some(index) = state.pointer.hovered
        && let Some(item) = session
            .inventory
            .cursor
            .map(|s| s.item)
            .or_else(|| item_at(state, session, index))
    {
        for _ in 0..(delta.abs().round() as usize).clamp(1, 8) {
            state.pointer.push(if delta > 0.0 {
                Edit::Take {
                    item,
                    cells: vec![index],
                    amount: Amount::One,
                }
            } else if session.inventory.cursor.is_none() {
                Edit::Release { index, one: true }
            } else {
                Edit::Deposit {
                    item,
                    cells: vec![index],
                    one: true,
                }
            });
        }
    }
    if state.pending.is_some() || !queue.is_empty() {
        return;
    }
    while let Some(edit) = state.pointer.edits.pop_front() {
        let transfer = match edit {
            Edit::Release { index, one } => {
                if one {
                    if let Some(cell) = &mut state.cells[index] {
                        cell.count -= 1;
                        if cell.count == 0 {
                            state.cells[index] = None;
                        }
                    }
                } else {
                    state.cells[index] = None;
                }
                None
            }
            Edit::Deposit { item, cells, one } => deposit(state, session, item, cells, one),
            Edit::Take {
                item,
                cells,
                amount,
            } => take(state, session, item, cells, amount),
        };
        if let Some((action, cells)) = transfer {
            queue.push(action);
            state.pending = Some(PendingGrid {
                response: session.inventory_response,
                cells,
                retries: 0,
            });
            state.error.clear();
            break;
        }
    }
}

type Transfer = (ServerAction, [Option<CraftGridCell>; 9]);
pub(super) fn preview(
    state: &CraftingState,
    session: &ClientSession,
) -> Option<(ItemKind, [Option<CraftGridCell>; 9], u16)> {
    if state.recipe_open {
        return None;
    }
    if let Some(pending) = &state.pending {
        let before: u16 = state.cells.iter().flatten().map(|cell| cell.count).sum();
        let after: u16 = pending.cells.iter().flatten().map(|cell| cell.count).sum();
        if after > before {
            return Some((
                session.inventory.cursor?.item,
                pending.cells,
                after - before,
            ));
        }
        return None;
    }
    let Gesture::Distribute { item, cells } = state.pointer.gesture.as_ref()? else {
        return None;
    };
    let (ServerAction::TransferCursor { counts, .. }, cells) =
        deposit(state, session, *item, cells.clone(), false)?
    else {
        return None;
    };
    Some((*item, cells, counts.iter().sum()))
}

fn deposit(
    state: &CraftingState,
    session: &ClientSession,
    item: ItemKind,
    indices: Vec<usize>,
    one: bool,
) -> Option<Transfer> {
    let cursor = session.inventory.cursor.filter(|s| s.item == item)?;
    let indices: Vec<_> = indices
        .into_iter()
        .filter(|&i| item_at(state, session, i).is_none_or(|v| v == item))
        .collect();
    if indices.is_empty() {
        return None;
    }
    let each = if one {
        1
    } else {
        cursor.count / indices.len() as u16
    };
    if each == 0 {
        return None;
    }
    let mut counts = [0_u16; terra_core::INVENTORY_SLOTS];
    let mut cells = state.cells;
    let mut left = cursor.count;
    for index in indices {
        let present = cells[index].map_or(0, |c| c.count);
        let source = cells[index].map(|c| usize::from(c.slot)).or_else(|| {
            (0..counts.len()).find(|&i| {
                session.inventory.slot(i).is_none_or(|s| s.item == item)
                    && session.inventory.slot(i).map_or(0, |s| s.count) + counts[i] < STACK_LIMIT
            })
        })?;
        let capacity = STACK_LIMIT
            .saturating_sub(session.inventory.slot(source).map_or(0, |s| s.count) + counts[source]);
        let count = each.min(left).min(STACK_LIMIT - present).min(capacity);
        if count == 0 {
            continue;
        }
        counts[source] += count;
        cells[index] = Some(CraftGridCell {
            slot: source as u8,
            count: present + count,
        });
        left -= count;
    }
    (left < cursor.count).then_some((
        ServerAction::TransferCursor {
            item,
            counts,
            to_cursor: false,
        },
        cells,
    ))
}
fn take(
    state: &CraftingState,
    session: &ClientSession,
    item: ItemKind,
    indices: Vec<usize>,
    amount: Amount,
) -> Option<Transfer> {
    if session.inventory.cursor.is_some_and(|s| s.item != item) {
        return None;
    }
    let mut capacity = STACK_LIMIT - session.inventory.cursor.map_or(0, |s| s.count);
    let mut counts = [0_u16; terra_core::INVENTORY_SLOTS];
    let mut cells = state.cells;
    for index in indices {
        if item_at(state, session, index) != Some(item) {
            continue;
        }
        let Some(cell) = &mut cells[index] else {
            continue;
        };
        let count = capacity.min(match amount {
            Amount::All => cell.count,
            Amount::Half => cell.count.div_ceil(2),
            Amount::One => 1,
        });
        counts[usize::from(cell.slot)] += count;
        capacity -= count;
        cell.count -= count;
        if cell.count == 0 {
            cells[index] = None;
        }
    }
    let sources: Vec<_> = counts.iter().enumerate().filter(|(_, n)| **n > 0).collect();
    if sources.is_empty() {
        return None;
    }
    let action = if sources.len() == 1 && session.inventory.cursor.is_none() {
        ServerAction::TakeToCursor {
            slot: sources[0].0 as u8,
            count: *sources[0].1,
        }
    } else {
        ServerAction::TransferCursor {
            item,
            counts,
            to_cursor: true,
        }
    };
    Some((action, cells))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workbench_drag_reserves_nine_cells_with_one_exact_transfer() {
        let mut session = ClientSession::default();
        let item = ItemKind::Block(16);
        session.inventory.cursor = Some(terra_core::ItemStack { item, count: 128 });
        let mut state = CraftingState::default();
        let (action, cells) = deposit(&state, &session, item, (0..9).collect(), false).unwrap();
        let ServerAction::TransferCursor {
            item,
            counts,
            to_cursor,
        } = action
        else {
            panic!("Expected one exact transfer")
        };
        assert_eq!(counts[0], 126);
        assert_eq!(counts.iter().sum::<u16>(), 126);
        assert!(session.inventory.transfer_cursor(item, counts, to_cursor));
        state.cells = cells;
        assert!(
            cells
                .iter()
                .all(|c| c.is_some_and(|c| c.count == 14 && c.slot == 0))
        );
        assert_eq!(session.inventory.cursor.unwrap().count, 2);
        let (action, remaining) =
            take(&state, &session, item, (0..9).collect(), Amount::All).unwrap();
        let ServerAction::TransferCursor {
            item,
            counts,
            to_cursor,
        } = action
        else {
            panic!("Expected one exact transfer")
        };
        assert!(session.inventory.transfer_cursor(item, counts, to_cursor));
        assert!(remaining.iter().all(Option::is_none));
        assert_eq!(session.inventory.cursor.unwrap().count, 128);
        assert_eq!(session.inventory.count(item), 0);
    }

    #[test]
    fn mismatched_cells_and_full_stacks_do_not_consume_the_cursor() {
        let item = ItemKind::Block(16);
        let mut session = ClientSession::default();
        session.inventory.insert(item, 128);
        session.inventory.insert(ItemKind::Block(2), 3);
        session.inventory.cursor = Some(terra_core::ItemStack { item, count: 3 });
        let mut state = CraftingState::default();
        state.cells[0] = Some(CraftGridCell {
            slot: 0,
            count: 128,
        });
        state.cells[1] = Some(CraftGridCell { slot: 1, count: 3 });
        assert!(deposit(&state, &session, item, vec![0, 1], false).is_none());
        assert_eq!(session.inventory.cursor.unwrap().count, 3);
        let (_, cells) = deposit(&state, &session, item, vec![1, 2], false).unwrap();
        assert_eq!(cells[1], state.cells[1]);
        assert_eq!(cells[2], Some(CraftGridCell { slot: 2, count: 3 }));
    }

    #[test]
    fn held_grid_preview_distributes_visually_without_creating_inventory_items() {
        let item = ItemKind::Block(16);
        let mut session = ClientSession::default();
        session.inventory.cursor = Some(terra_core::ItemStack { item, count: 19 });
        let original = session.inventory.clone();
        let mut state = CraftingState::default();
        state.pointer.gesture = Some(Gesture::Distribute {
            item,
            cells: vec![0, 1, 2],
        });
        let (shown, cells, deposited) = preview(&state, &session).unwrap();
        assert_eq!(shown, item);
        assert_eq!(deposited, 18);
        assert!(
            cells[..3]
                .iter()
                .all(|cell| cell.is_some_and(|cell| cell.count == 6))
        );
        assert_eq!(state.cells, [None; 9]);
        assert_eq!(session.inventory, original);
    }
}
