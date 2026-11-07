use super::*;
use terra_core::logistics::FilterConfig;

pub(super) fn set_random(
    selection: &mut InventorySelection,
    session: &ClientSession,
    random: bool,
) {
    let Some(container) = session.container.as_ref() else {
        return;
    };
    let Some(confirmed) = container.tunnel_random else {
        return;
    };
    let current = selection
        .queue
        .tunnel_random(container.anchor)
        .unwrap_or(confirmed);
    if current != random {
        selection.request(
            session,
            ServerAction::SetTunnelRandom {
                station: container.anchor,
                random,
            },
        );
    }
}

fn current(
    selection: &InventorySelection,
    session: &ClientSession,
    output: u8,
) -> Option<([i64; 3], Option<u8>, FilterConfig)> {
    let container = session.container.as_ref()?;
    if let Some(distributor) = container.distributor {
        let filter = *distributor.filters.get(usize::from(output))?;
        return Some((
            container.anchor,
            Some(output),
            selection
                .queue
                .distributor_filter(container.anchor, output)
                .unwrap_or(filter),
        ));
    }
    let filter = container.filter?;
    Some((
        container.anchor,
        None,
        selection
            .queue
            .funnel_filter(container.anchor)
            .unwrap_or(filter),
    ))
}

pub(super) fn set_allowed(
    selection: &mut InventorySelection,
    session: &ClientSession,
    output: u8,
    allowed: bool,
) {
    if let Some((station, output, mut filter)) = current(selection, session, output)
        && filter.allowed != allowed
    {
        filter.allowed = allowed;
        request(selection, session, station, output, filter);
    }
}

pub(super) fn set_item(
    selection: &mut InventorySelection,
    session: &ClientSession,
    output: u8,
    item: Option<ItemKind>,
) {
    if let Some((station, output, mut filter)) = current(selection, session, output)
        && filter.item != item
    {
        filter.item = item;
        request(selection, session, station, output, filter);
    }
}

fn request(
    selection: &mut InventorySelection,
    session: &ClientSession,
    station: [i64; 3],
    output: Option<u8>,
    filter: FilterConfig,
) {
    selection.request(
        session,
        match output {
            Some(output) => ServerAction::SetDistributorFilter {
                station,
                output,
                filter,
            },
            None => ServerAction::SetFunnelFilter { station, filter },
        },
    );
}

pub(super) fn select(
    state: &mut InventoryState,
    selection: &mut InventorySelection,
    session: &ClientSession,
    id: &str,
) -> bool {
    if current(selection, session, state.filter_output).is_none() {
        return false;
    }
    if let Some(item) = inspectable_item(session, &selection.palette, id) {
        set_item(selection, session, state.filter_output, Some(item));
        state.focused = Some(item);
        state.filter_selecting = false;
    }
    true
}

pub(super) fn press(
    state: &mut InventoryState,
    selection: &mut InventorySelection,
    session: &ClientSession,
    press: pointer::Press,
) -> bool {
    let pointer::Press {
        command,
        index,
        item,
        secondary,
        ..
    } = press;
    let changing_output = command == "distributor-filter" && index != Some(state.filter_output);
    if command == "distributor-filter" {
        let Some(output) = index.filter(|output| *output < 3) else {
            return true;
        };
        if session
            .container
            .as_ref()
            .is_none_or(|container| container.distributor.is_none())
        {
            return false;
        }
        state.filter_output = output;
    }
    if current(selection, session, state.filter_output).is_none() {
        return false;
    }
    if matches!(command, "filter" | "distributor-filter") {
        if secondary {
            set_item(selection, session, state.filter_output, None);
            state.filter_selecting = false;
        } else if let Some(cursor) = session.inventory.cursor {
            set_item(selection, session, state.filter_output, Some(cursor.item));
            state.filter_selecting = false;
        } else {
            state.filter_selecting = changing_output || !state.filter_selecting;
        }
        return true;
    }
    if (command != "catalog" && !state.filter_selecting)
        || !matches!(command, "catalog" | "slot" | "hotbar")
    {
        return false;
    }
    let selected = if matches!(command, "slot" | "hotbar") {
        index
            .and_then(|index| session.inventory.slot(usize::from(index)))
            .map(|stack| stack.item)
    } else {
        inspectable_item(session, &selection.palette, item)
    };
    if let Some(selected) = selected {
        set_item(selection, session, state.filter_output, Some(selected));
        state.focused = Some(selected);
        state.filter_selecting = false;
    }
    true
}

pub(super) fn output_slot(
    output: u8,
    filter: FilterConfig,
    selecting: bool,
    palette: &BlockPalette,
) -> view::InventoryItemView {
    let mut slot = slot(filter, selecting, palette);
    slot.id = format!("distributor-filter-{output}");
    slot.index = output.to_string();
    slot.command = "distributor-filter".into();
    slot
}

pub(super) fn slot(
    filter: FilterConfig,
    selecting: bool,
    palette: &BlockPalette,
) -> view::InventoryItemView {
    let mut slot = view::InventoryItemView {
        id: "funnel-filter".into(),
        index: "0".into(),
        command: "filter".into(),
        empty: true,
        selected: selecting,
        ..default()
    };
    if let Some(item) = filter.item {
        slot.item = Some(item);
        let item = view::item_view(item, 1, palette);
        slot.ghost_preview = item.preview_key;
        slot.ghost_name = item.name;
        slot.identifier = item.identifier;
        slot.description = item.description;
    }
    slot
}
