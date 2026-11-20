//! Passive ports operate on loaded inventories and the existing visible drops.
use super::*;
#[cfg(test)]
use std::collections::BTreeMap;
use terra_core::logistics;
use terra_world::conveyors::{
    DropIndex, drop_space_clear, drop_step_clear, index_drops, move_indexed_drop,
    remove_indexed_drop,
};
type Cell = [i64; 3];

#[derive(Default)]
pub(super) struct Runtime {
    elapsed: f32,
    cursor: Option<Cell>,
    crusher_cursor: Option<Cell>,
}

fn filter_at(journal: &EditJournal, position: Cell) -> logistics::FilterConfig {
    journal
        .logistics_filter_at(position)
        .copied()
        .unwrap_or_default()
}

fn source(peer: Cell, chunks: &ChunkStore, journal: &EditJournal) -> Option<Cell> {
    let material = chunks.block_at(peer)?;
    if terra_core::containers::is_chest(material) {
        return chest_loaded(peer, chunks, journal).then_some(peer);
    }
    logistics::source_output(material)?;
    let state = journal.object_state(peer)?;
    let anchor = if state.secondary { state.peer? } else { peer };
    let loaded = match material {
        59 => super::furnaces::loaded(anchor, chunks, journal),
        99 => super::crushers::loaded(anchor, chunks, journal),
        156 => chunks.block_at(anchor) == Some(156) && journal.valid_mixer(anchor),
        161 => chunks.block_at(anchor) == Some(161) && journal.valid_cauldron(anchor),
        162 => chunks.block_at(anchor) == Some(162) && journal.valid_campfire(anchor),
        _ => false,
    };
    loaded.then_some(anchor)
}

fn chest_loaded(peer: Cell, chunks: &ChunkStore, journal: &EditJournal) -> bool {
    let Some(material) = chunks
        .block_at(peer)
        .filter(|m| terra_core::containers::is_chest(*m))
    else {
        return false;
    };
    journal.block(peer) == Some(material)
        && journal.object_state(peer).is_none_or(|state| {
            state.valid_for(peer, material)
                && state.peer.is_none_or(|other| {
                    material == 29
                        && chunks.block_at(other) == Some(29)
                        && journal.object_state(other).is_some_and(|s| {
                            s.peer == Some(peer)
                                && s.facing == state.facing
                                && s.secondary != state.secondary
                        })
                })
        })
}

fn inventory_target(
    position: Cell,
    material: u16,
    state: terra_world::edits::BlockObjectState,
    chunks: &ChunkStore,
    journal: &EditJournal,
) -> Option<(Cell, bool)> {
    if let Some(peer) = state.peer {
        let inventory =
            terra_world::placement::logistics_inventory_at(chunks, journal, position, peer)?;
        return source(inventory, chunks, journal)
            .map(|anchor| (anchor, logistics::extracts(material, state.facing)));
    }
    let direction = logistics::flow(state.facing);
    // Unbound ports follow the visible arrow: insert ahead, extract from behind.
    // Height changes cannot reach through a solid cell on that inventory side.
    [1, -1].into_iter().find_map(|sign| {
        [0, -1, 1].into_iter().find_map(|dy| {
            let p = [
                position[0] + direction[0] * sign,
                position[1] + dy,
                position[2] + direction[2] * sign,
            ];
            if dy != 0
                && chunks
                    .block_at([p[0], position[1], p[2]])
                    .and_then(terra_voxel::MaterialId::from_raw)
                    .is_none_or(terra_voxel::MaterialId::is_solid)
            {
                return None;
            }
            let inventory =
                terra_world::placement::logistics_inventory_at(chunks, journal, position, p)?;
            source(inventory, chunks, journal).map(|anchor| (anchor, sign < 0))
        })
    })
}

fn belt_drop_position(
    position: Cell,
    facing: u8,
    chunks: &ChunkStore,
    journal: &EditJournal,
) -> Option<[f64; 3]> {
    let direction = logistics::flow(facing);
    [
        [0, -1, 0],
        [direction[0], -1, direction[2]],
        [direction[0], 0, direction[2]],
        [direction[0], 1, direction[2]],
    ]
    .into_iter()
    .find_map(|delta| {
        let belt = std::array::from_fn(|i| position[i] + delta[i]);
        let segment = terra_world::conveyors::segment_at(journal, belt)?;
        (segment.span.loaded(chunks, journal) && journal.kinetic_state(belt).running())
            .then(|| segment.drop_center())
            .flatten()
    })
}

fn nearby_drops(position: Cell, index: &DropIndex) -> Vec<u64> {
    let mut result = Vec::new();
    for x in -1..=1 {
        for y in -2..=1 {
            for z in -1..=1 {
                result.extend(
                    index
                        .get(&[position[0] + x, position[1] + y, position[2] + z])
                        .into_iter()
                        .flatten()
                        .map(|(id, _)| *id),
                );
            }
        }
    }
    result
}

fn outlet_clear(
    chunks: &ChunkStore,
    journal: &EditJournal,
    properties: &terra_core::block_properties::BlockPropertiesCatalog,
    point: [f64; 3],
    port: Cell,
    carrier: Option<terra_world::conveyors::Span>,
) -> bool {
    match carrier {
        Some(carrier) => {
            outlet_clear_with_carriers(chunks, journal, properties, point, port, &[carrier])
        }
        None => outlet_clear_with_carriers(chunks, journal, properties, point, port, &[]),
    }
}

fn outlet_clear_with_carriers(
    chunks: &ChunkStore,
    journal: &EditJournal,
    properties: &terra_core::block_properties::BlockPropertiesCatalog,
    point: [f64; 3],
    port: Cell,
    carriers: &[terra_world::conveyors::Span],
) -> bool {
    let minimum = point.map(|v| v - 0.12);
    let maximum = point.map(|v| v + 0.12);
    let low = minimum.map(|v| v.floor() as i64);
    let high = maximum.map(|v| v.floor() as i64);
    let mut owners = std::collections::BTreeSet::new();
    for x in low[0]..=high[0] {
        for y in low[1]..=high[1] {
            for z in low[2]..=high[2] {
                let p = [x, y, z];
                if chunks.block_at(p).is_none() {
                    return false;
                }
                owners.insert(p);
                journal.visit_extended_kinetics_near(p, |p| {
                    owners.insert(p);
                });
            }
        }
    }
    for owner in owners {
        if owner == port
            || carriers.iter().any(|span| {
                owner == span.start || owner == span.end || span.cells().any(|p| p == owner)
            })
        {
            continue;
        }
        let mut blocked = false;
        let modeled = terra_world::interaction_shapes::visit_collision_boxes(
            chunks,
            journal,
            properties,
            owner,
            |bounds| blocked |= bounds.intersects(minimum, maximum),
        );
        if blocked
            || !modeled
                && chunks
                    .block_at(owner)
                    .and_then(terra_voxel::MaterialId::from_raw)
                    .is_none_or(terra_voxel::MaterialId::is_solid)
        {
            return false;
        }
    }
    true
}

fn funnel_outlet(
    position: Cell,
    chunks: &ChunkStore,
    journal: &EditJournal,
) -> Option<([f64; 3], Option<terra_world::conveyors::Span>)> {
    let below = [position[0], position[1] - 1, position[2]];
    chunks.block_at(below)?;
    if let Some(segment) = terra_world::conveyors::segment_at(journal, below) {
        if !segment.span.loaded(chunks, journal) {
            return None;
        }
        if let Some(center) = segment.drop_center() {
            return Some((center, Some(segment.span)));
        }
    }
    Some((
        [
            position[0] as f64 + 0.5,
            position[1] as f64 - 0.16,
            position[2] as f64 + 0.5,
        ],
        None,
    ))
}

fn outlet_occupied(index: &DropIndex, point: [f64; 3], except: Option<u64>) -> bool {
    !drop_space_clear(index, point, except)
}

fn in_funnel_mouth(drop: &terra_core::WorldDrop, position: Cell) -> bool {
    (drop.position[0] - position[0] as f64 - 0.5).abs() <= 0.32
        && (drop.position[2] - position[2] as f64 - 0.5).abs() <= 0.32
        && (0.08..=1.2).contains(&(drop.position[1] - position[1] as f64))
        && drop.velocity[1] <= 0.5
}

fn release_funnel_drop(
    position: Cell,
    id: u64,
    chunks: &ChunkStore,
    journal: &mut EditJournal,
    properties: &terra_core::block_properties::BlockPropertiesCatalog,
    index: &mut DropIndex,
) -> bool {
    let Some(drop) = journal.gameplay().drops.iter().find(|d| d.id == id) else {
        return false;
    };
    if !in_funnel_mouth(drop, position)
        || !filter_at(journal, position).accepts(drop.stack.item)
        || journal.gameplay().revision == u64::MAX
    {
        return false;
    }
    let Some((outlet, carrier)) = funnel_outlet(position, chunks, journal) else {
        return false;
    };
    if outlet_occupied(index, outlet, Some(id)) {
        return false;
    }
    let start = drop.position;
    let mut previous = start;
    // The bounded neck sweep ignores only this funnel and its destination belt.
    for step in 1..=30 {
        let point = std::array::from_fn(|axis| {
            start[axis] + (outlet[axis] - start[axis]) * step as f64 / 30.0
        });
        if !terra_world::kingdom_access::can_move_drop(chunks, journal, previous, point)
            || !drop_step_clear(index, previous, point, id)
            || !outlet_clear(chunks, journal, properties, point, position, carrier)
        {
            return false;
        }
        previous = point;
    }
    let drop = journal
        .gameplay_mut()
        .drops
        .iter_mut()
        .find(|d| d.id == id)
        .unwrap();
    drop.position = outlet;
    drop.velocity = [0.0, -0.5, 0.0];
    drop.grounded = false;
    drop.rotation = [0.0, 0.0, 0.0, 1.0];
    drop.angular_velocity = [0.0; 3];
    move_indexed_drop(index, id, start, outlet);
    journal.gameplay_mut().revision += 1;
    true
}

fn carried_funnel_outlet(
    position: Cell,
    facing: u8,
    drop: &terra_core::WorldDrop,
    chunks: &ChunkStore,
    journal: &EditJournal,
) -> Option<(terra_world::conveyors::Span, [f64; 3])> {
    let segment = terra_world::conveyors::carrying_segment(drop, journal)?;
    let flow = logistics::flow(facing);
    let towards = (0..3)
        .map(|i| drop.velocity[i] * flow[i] as f32)
        .sum::<f32>();
    let drive = journal.kinetic_state(segment.position());
    let raw = journal
        .object_state(segment.position())
        .unwrap_or_default()
        .facing;
    let shaft_facing = if journal.block(segment.position()) == Some(148) {
        terra_core::kinetic::belt_facing(raw)
    } else {
        raw
    };
    let sign = (0..3)
        .map(|i| {
            terra_core::kinetic::axis(shaft_facing)[i]
                * terra_core::kinetic::axis(segment.span.facing)[i]
        })
        .sum::<i64>() as f32;
    let belt_towards = if drive.running() {
        (0..3)
            .map(|i| segment.span.direction()[i] * flow[i] as f32)
            .sum::<f32>()
            * drive.rpm
            * sign
            * segment.span.flow_sign()
    } else {
        0.0
    };
    let relative: [f64; 3] = std::array::from_fn(|i| drop.position[i] - position[i] as f64 - 0.5);
    let along = (0..3).map(|i| relative[i] * flow[i] as f64).sum::<f64>();
    let sideways = relative[0] * flow[2] as f64 - relative[2] * flow[0] as f64;
    if !segment.span.loaded(chunks, journal)
        || towards <= 0.0 && belt_towards <= 0.0
        || !(-0.65..=0.25).contains(&along)
        || sideways.abs() > 0.44
        || !(-1.0..=0.3).contains(&relative[1])
    {
        return None;
    }
    let outlet = [
        position[0] as f64 + 0.5 + flow[0] as f64 * 0.8,
        drop.position[1],
        position[2] as f64 + 0.5 + flow[2] as f64 * 0.8,
    ];
    let delta: [f64; 3] = std::array::from_fn(|i| outlet[i] - drop.position[i]);
    if delta.iter().map(|v| v * v).sum::<f64>() > 2.25 {
        return None;
    }
    Some((segment.span, outlet))
}

fn funnel_candidates(
    position: Cell,
    facing: u8,
    chunks: &ChunkStore,
    journal: &EditJournal,
    index: &DropIndex,
    allow_carried: bool,
) -> Vec<u64> {
    nearby_drops(position, index)
        .into_iter()
        .filter(|id| {
            journal
                .gameplay()
                .drops
                .iter()
                .find(|drop| drop.id == *id)
                .is_some_and(|drop| {
                    filter_at(journal, position).accepts(drop.stack.item)
                        && (in_funnel_mouth(drop, position)
                            || allow_carried
                                && carried_funnel_outlet(position, facing, drop, chunks, journal)
                                    .is_some())
                })
        })
        .take(4)
        .collect()
}

fn eject_carried_drop(
    position: Cell,
    facing: u8,
    id: u64,
    chunks: &ChunkStore,
    journal: &mut EditJournal,
    properties: &terra_core::block_properties::BlockPropertiesCatalog,
    index: &mut DropIndex,
) -> bool {
    let Some(drop) = journal.gameplay().drops.iter().find(|d| d.id == id) else {
        return false;
    };
    if !filter_at(journal, position).accepts(drop.stack.item) {
        return false;
    }
    let Some((carrier, outlet)) = carried_funnel_outlet(position, facing, drop, chunks, journal)
    else {
        return false;
    };
    let delta: [f64; 3] = std::array::from_fn(|i| outlet[i] - drop.position[i]);
    let start = drop.position;
    if outlet_occupied(index, outlet, Some(id)) {
        return false;
    }
    let mut previous = start;
    for step in 1..=30 {
        let p = std::array::from_fn(|i| start[i] + delta[i] * step as f64 / 30.0);
        if !terra_world::kingdom_access::can_move_drop(chunks, journal, previous, p)
            || !drop_step_clear(index, previous, p, id)
            || !outlet_clear(chunks, journal, properties, p, position, Some(carrier))
        {
            return false;
        }
        previous = p;
    }
    if journal.gameplay().revision == u64::MAX {
        return false;
    }
    let drop = journal
        .gameplay_mut()
        .drops
        .iter_mut()
        .find(|d| d.id == id)
        .unwrap();
    drop.position = outlet;
    drop.velocity = logistics::flow(facing).map(|v| v as f32);
    drop.grounded = false;
    move_indexed_drop(index, id, start, outlet);
    journal.gameplay_mut().revision += 1;
    true
}

pub(super) fn advance(
    runtime: &mut Runtime,
    chunks: &ChunkStore,
    journal: &mut EditJournal,
    properties: &terra_core::block_properties::BlockPropertiesCatalog,
    elapsed: f32,
) {
    if !elapsed.is_finite() || elapsed <= 0.0 {
        return;
    }
    runtime.elapsed += elapsed.min(1.0);
    if runtime.elapsed < logistics::TRANSFER_SECONDS {
        return;
    }
    runtime.elapsed = 0.0;
    if !journal.has_logistics() && journal.crushers().next().is_none() {
        runtime.cursor = None;
        runtime.crusher_cursor = None;
        return;
    }
    let mut ports: Vec<_> = chunks
        .loaded
        .keys()
        .flat_map(|s| journal.logistics_in_section(*s))
        .filter(|p| chunks.block_at(*p).is_some_and(logistics::is_port))
        .collect();
    ports.sort_unstable();
    let start = runtime
        .cursor
        .map_or(0, |cursor| ports.partition_point(|p| *p <= cursor))
        % ports.len().max(1);
    let mut drops = index_drops(&journal.gameplay().drops);
    for offset in 0..ports.len().min(logistics::PORT_BUDGET) {
        let position = ports[(start + offset) % ports.len()];
        runtime.cursor = Some(position);
        let material = chunks.block_at(position).unwrap_or(0);
        let Some(state) = journal
            .object_state(position)
            .filter(|s| s.valid_for(position, material))
        else {
            continue;
        };
        if logistics::is_router(material) {
            continue;
        }
        let Some((anchor, extracting)) =
            inventory_target(position, material, state, chunks, journal)
        else {
            if state.peer.is_none() && logistics::is_funnel(material) {
                for id in funnel_candidates(position, state.facing, chunks, journal, &drops, true) {
                    if release_funnel_drop(position, id, chunks, journal, properties, &mut drops)
                        || eject_carried_drop(
                            position,
                            state.facing,
                            id,
                            chunks,
                            journal,
                            properties,
                            &mut drops,
                        )
                    {
                        break;
                    }
                }
            }
            continue;
        };
        if extracting {
            // A physical mouth stack takes this port's transfer turn before an
            // inventory export. Insertion mode still sends it into the inventory.
            if logistics::is_funnel(material)
                && funnel_candidates(position, state.facing, chunks, journal, &drops, false)
                    .into_iter()
                    .any(|id| {
                        release_funnel_drop(position, id, chunks, journal, properties, &mut drops)
                    })
            {
                continue;
            }
            let destination = if logistics::is_funnel(material) {
                funnel_outlet(position, chunks, journal)
            } else {
                belt_drop_position(position, state.facing, chunks, journal)
                    .map(|point| (point, None))
            };
            let Some((output, carrier)) = destination else {
                continue;
            };
            let cell = output.map(|v| v.floor() as i64);
            if !terra_world::kingdom_access::can_transport_inventory(chunks, journal, cell, anchor)
                || !terra_world::kingdom_access::can_transport(chunks, journal, position, cell)
                || outlet_occupied(&drops, output, None)
                || if logistics::is_funnel(material) {
                    !outlet_clear(chunks, journal, properties, output, position, carrier)
                } else {
                    !super::gameplay::drop_bounds_clear_with_context(
                        chunks, journal, properties, output, [0.12; 3],
                    )
                }
            {
                continue;
            }
            match journal.eject_inventory_output_selected(
                anchor,
                output,
                filter_at(journal, position),
                logistics::is_tunnel(material) && journal.tunnel_random_at(position),
            ) {
                Ok(true) => {
                    drops
                        .entry(cell)
                        .or_default()
                        .push((journal.gameplay().drops.last().unwrap().id, output));
                }
                Ok(false) => {}
                Err(error) => debug!(?position,%error,"Output transfer deferred"),
            }
        } else {
            let center = [
                position[0] as f64 + 0.5,
                position[1] as f64,
                position[2] as f64 + 0.5,
            ];
            let candidates = nearby_drops(position, &drops)
                .into_iter()
                .filter(|id| {
                    journal
                        .gameplay()
                        .drops
                        .iter()
                        .find(|d| d.id == *id)
                        .is_some_and(|drop| {
                            terra_world::kingdom_access::can_transport_inventory(
                                chunks,
                                journal,
                                drop.position.map(|v| v.floor() as i64),
                                anchor,
                            ) && filter_at(journal, position).accepts(drop.stack.item)
                                && (drop.position[0] - center[0]).abs() <= 0.8
                                && (drop.position[2] - center[2]).abs() <= 0.8
                                && (-1.0..=1.8).contains(&(drop.position[1] - center[1]))
                                && (terra_world::conveyors::carrying_segment(drop, journal)
                                    .is_some_and(|carried| carried.span.loaded(chunks, journal))
                                    || logistics::is_funnel(material)
                                        && in_funnel_mouth(drop, position))
                        })
                })
                .take(1)
                .collect::<Vec<_>>();
            for id in candidates {
                let previous = journal
                    .gameplay()
                    .drops
                    .iter()
                    .find(|drop| drop.id == id)
                    .map(|drop| drop.position);
                if let Err(error) = journal.import_drop_to_inventory_count(
                    anchor,
                    id,
                    logistics::ITEMS_PER_TRANSFER,
                ) {
                    debug!(?position,%error,"Chest transfer deferred");
                }
                if let Some(previous) = previous
                    && !journal.gameplay().drops.iter().any(|drop| drop.id == id)
                {
                    remove_indexed_drop(&mut drops, id, previous);
                }
            }
        }
    }
    if ports.is_empty() {
        runtime.cursor = None;
    }
    let mut crushers: Vec<_> = chunks
        .loaded
        .keys()
        .flat_map(|s| journal.crushers_in_section(*s))
        .filter(|p| terra_world::crushers::loaded(*p, chunks, journal))
        .collect();
    crushers.sort_unstable();
    let start = runtime
        .crusher_cursor
        .map_or(0, |last| crushers.partition_point(|p| *p <= last))
        % crushers.len().max(1);
    for offset in 0..crushers.len().min(logistics::PORT_BUDGET) {
        let primary = crushers[(start + offset) % crushers.len()];
        runtime.crusher_cursor = Some(primary);
        let mut inlets = vec![primary];
        inlets.extend(journal.object_state(primary).and_then(|s| s.peer));
        for inlet in inlets {
            for id in nearby_drops(inlet, &drops).into_iter().take(4) {
                if journal
                    .gameplay()
                    .drops
                    .iter()
                    .find(|d| d.id == id)
                    .is_some_and(|d| {
                        terra_world::crushers::carrying(d, journal) == Some(primary)
                            && terra_world::kingdom_access::can_transport(
                                chunks,
                                journal,
                                d.position.map(|v| v.floor() as i64),
                                primary,
                            )
                    })
                    && let Err(error) = journal.import_drop_to_inventory(primary, id)
                {
                    debug!(?primary,%error,"Crusher inlet transfer deferred");
                }
            }
        }
    }
    if crushers.is_empty() {
        runtime.crusher_cursor = None;
    }
}

#[cfg(test)]
#[path = "server_logistics_route_tests.rs"]
mod route_tests;

#[cfg(test)]
#[path = "server_logistics_filter_merger_tests.rs"]
mod filter_merger_tests;

#[cfg(test)]
#[path = "server_logistics_backpressure_tests.rs"]
mod backpressure_tests;

#[cfg(test)]
#[path = "server_logistics_linked_junction_tests.rs"]
mod linked_junction_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use terra_core::{Inventory, ItemKind, ItemStack, MineralItem, kinetic};
    use terra_voxel::{BlockSection, SectionPos, global_to_section};
    use terra_world::{edits::BlockObjectState, streaming::LoadedSection};

    pub(super) fn place(
        chunks: &mut ChunkStore,
        journal: &mut EditJournal,
        p: Cell,
        m: u16,
        state: BlockObjectState,
    ) {
        let (s, local) = global_to_section(p);
        chunks
            .loaded
            .entry(s)
            .or_insert_with(|| LoadedSection {
                data: BlockSection::empty(s).unwrap(),
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: vec![],
            })
            .data
            .set_block(local, m)
            .unwrap();
        journal.record(p, m).unwrap();
        journal.set_object_state(p, Some(state)).unwrap();
    }
    pub(super) fn fixture() -> (ChunkStore, EditJournal, Cell, Cell) {
        let mut chunks = ChunkStore::default();
        let mut journal = EditJournal::default();
        let source = [2, 2, 2];
        let upper = [2, 3, 2];
        place(
            &mut chunks,
            &mut journal,
            source,
            59,
            BlockObjectState {
                peer: Some(upper),
                ..Default::default()
            },
        );
        place(
            &mut chunks,
            &mut journal,
            upper,
            59,
            BlockObjectState {
                peer: Some(source),
                secondary: true,
                ..Default::default()
            },
        );
        journal
            .set_container(
                source,
                Inventory {
                    slots: vec![
                        ItemStack::EMPTY,
                        ItemStack::EMPTY,
                        ItemStack {
                            item: ItemKind::Mineral(MineralItem::IronIngot),
                            count: 8,
                        },
                    ],
                    ..Default::default()
                },
            )
            .unwrap();
        place(
            &mut chunks,
            &mut journal,
            [2, 2, 3],
            logistics::HOPPER,
            BlockObjectState {
                peer: Some(source),
                ..Default::default()
            },
        );
        let belt = [2, 1, 3];
        place(
            &mut chunks,
            &mut journal,
            belt,
            148,
            BlockObjectState::default(),
        );
        journal.replace_kinetic_states(BTreeMap::from([(
            belt,
            kinetic::KineticState {
                rpm: 30.0,
                overloaded: false,
                ..Default::default()
            },
        )]));
        (chunks, journal, source, belt)
    }
    #[test]
    fn ornate_chest_is_a_loaded_port_source_at_every_yaw() {
        for facing in 0..4 {
            let (mut chunks, mut journal, _, _) = fixture();
            let chest = [8, 3, 7];
            let port = [8, 3, 8];
            place(
                &mut chunks,
                &mut journal,
                chest,
                terra_core::grave_blocks::ORNATE_CHEST,
                BlockObjectState {
                    facing,
                    ..Default::default()
                },
            );
            let state = BlockObjectState {
                peer: Some(chest),
                ..Default::default()
            };
            place(&mut chunks, &mut journal, port, logistics::HOPPER, state);
            assert_eq!(source(chest, &chunks, &journal), Some(chest));
            assert_eq!(
                inventory_target(port, logistics::HOPPER, state, &chunks, &journal),
                Some((chest, true))
            );
            let (section, local) = terra_voxel::global_to_section(chest);
            chunks
                .loaded
                .get_mut(&section)
                .unwrap()
                .data
                .set_block(local, 0)
                .unwrap();
            assert_eq!(
                source(chest, &chunks, &journal),
                None,
                "Stale journal storage must not route items"
            );
        }
    }

    #[test]
    fn tunnel_routes_chests_at_belt_height_and_one_cell_above_or_below_without_a_saved_peer() {
        for dy in [-1, 0, 1] {
            let (mut chunks, mut journal, _, _) = fixture();
            let port = [8, 3, 8];
            let chest = [8, 3 + dy, 7];
            let belt = [8, 2, 8];
            place(
                &mut chunks,
                &mut journal,
                chest,
                29,
                BlockObjectState::default(),
            );
            place(
                &mut chunks,
                &mut journal,
                port,
                152,
                BlockObjectState::default(),
            );
            place(
                &mut chunks,
                &mut journal,
                belt,
                148,
                BlockObjectState::default(),
            );
            let item = ItemKind::Mineral(MineralItem::IronIngot);
            journal
                .set_container(
                    chest,
                    Inventory {
                        slots: vec![ItemStack { item, count: 9 }],
                        ..Default::default()
                    },
                )
                .unwrap();
            journal.replace_kinetic_states(BTreeMap::from([(
                belt,
                kinetic::KineticState {
                    rpm: 30.0,
                    ..Default::default()
                },
            )]));
            let mut runtime = Runtime::default();
            advance(
                &mut runtime,
                &chunks,
                &mut journal,
                &Default::default(),
                0.5,
            );
            let exported: Vec<_> = journal
                .gameplay()
                .drops
                .iter()
                .filter(|d| d.stack.item == item)
                .collect();
            assert_eq!(exported.len(), 1, "height {dy}");
            assert_eq!(exported[0].stack.count, 1);
            assert_eq!(
                journal.container_at(chest).unwrap().slot(0).unwrap().count,
                8
            );
            journal
                .set_object_state(
                    port,
                    Some(BlockObjectState {
                        facing: 4,
                        ..Default::default()
                    }),
                )
                .unwrap();
            advance(
                &mut runtime,
                &chunks,
                &mut journal,
                &Default::default(),
                0.5,
            );
            assert!(journal.gameplay().drops.is_empty());
            assert_eq!(
                journal.container_at(chest).unwrap().slot(0).unwrap().count,
                9
            );
            if dy != 0 {
                let blocked = [chest[0], port[1], chest[2]];
                let (section, local) = global_to_section(blocked);
                chunks
                    .loaded
                    .get_mut(&section)
                    .unwrap()
                    .data
                    .set_block(local, 3)
                    .unwrap();
                journal.record(blocked, 3).unwrap();
                journal
                    .set_object_state(port, Some(Default::default()))
                    .unwrap();
                advance(
                    &mut runtime,
                    &chunks,
                    &mut journal,
                    &Default::default(),
                    0.5,
                );
                assert!(journal.gameplay().drops.is_empty());
                assert_eq!(
                    journal.container_at(chest).unwrap().slot(0).unwrap().count,
                    9
                );
            }
            assert!(journal.gameplay().valid());
        }
    }

    #[test]
    fn free_end_funnel_releases_the_same_carried_stack_but_never_through_a_wall() {
        for blocked in [false, true] {
            let (mut chunks, mut journal, _, _) = fixture();
            journal.replace_kinetic_states(Default::default());
            let belt = [8, 2, 8];
            let port = [8, 3, 8];
            place(
                &mut chunks,
                &mut journal,
                belt,
                148,
                BlockObjectState::default(),
            );
            place(
                &mut chunks,
                &mut journal,
                port,
                153,
                BlockObjectState::default(),
            );
            journal.replace_kinetic_states(BTreeMap::from([(
                belt,
                kinetic::KineticState {
                    rpm: 30.0,
                    ..Default::default()
                },
            )]));
            if blocked {
                let (_, local) = global_to_section([8, 2, 9]);
                chunks
                    .loaded
                    .get_mut(&SectionPos::default())
                    .unwrap()
                    .data
                    .set_block(local, 3)
                    .unwrap();
                journal.record([8, 2, 9], 3).unwrap();
            }
            let center = terra_world::conveyors::segment_at(&journal, belt)
                .unwrap()
                .drop_center()
                .unwrap();
            assert!(journal.gameplay_mut().drop_stack(
                center,
                ItemStack {
                    item: ItemKind::Block(3),
                    count: 7
                },
                [0.0; 3],
                0.0
            ));
            let original = journal.gameplay().drops.last().unwrap().clone();
            advance(
                &mut Runtime::default(),
                &chunks,
                &mut journal,
                &Default::default(),
                0.5,
            );
            let drop = journal
                .gameplay()
                .drops
                .iter()
                .find(|d| d.id == original.id)
                .unwrap();
            assert_eq!(drop.stack, original.stack);
            if blocked {
                assert_eq!(drop.position, original.position);
            } else {
                assert_eq!(drop.position, [8.5, center[1], 9.3]);
                assert!(!drop.grounded);
            }
            assert_eq!(journal.gameplay().drops.len(), 1);
            assert!(journal.gameplay().valid());
        }
    }

    #[test]
    fn single_cell_crusher_takes_physical_input_and_preserves_rejected_or_full_stacks() {
        for (compatible, full) in [(true, false), (true, true), (false, false)] {
            let mut chunks = ChunkStore::default();
            let mut journal = EditJournal::default();
            let crusher = [8, 2, 8];
            place(
                &mut chunks,
                &mut journal,
                crusher,
                99,
                BlockObjectState::default(),
            );
            journal.set_crusher(crusher, Default::default()).unwrap();
            let input = terra_core::crusher::recipes()[0].input;
            let item = if compatible {
                input
            } else {
                let dirt = ItemKind::Block(2);
                assert!(terra_core::crusher::recipe_for(dirt).is_none());
                dirt
            };
            if full {
                journal
                    .set_container(
                        crusher,
                        Inventory {
                            slots: vec![ItemStack {
                                item: input,
                                count: terra_core::crusher::slot_capacity(0, input),
                            }],
                            ..Default::default()
                        },
                    )
                    .unwrap();
            }
            assert!(journal.gameplay_mut().drop_stack(
                [8.5, 3.13, 8.5],
                ItemStack { item, count: 3 },
                [0.0; 3],
                0.0
            ));
            let original = journal.gameplay().drops[0].clone();
            advance(
                &mut Runtime::default(),
                &chunks,
                &mut journal,
                &Default::default(),
                0.5,
            );
            if compatible && !full {
                assert!(journal.gameplay().drops.is_empty());
                assert_eq!(
                    journal
                        .container_at(crusher)
                        .unwrap()
                        .slot(0)
                        .unwrap()
                        .count,
                    3
                );
            } else {
                assert_eq!(journal.gameplay().drops, [original]);
                assert_eq!(
                    terra_world::crushers::carrying(&journal.gameplay().drops[0], &journal),
                    Some(crusher)
                );
            }
            assert!(journal.gameplay().valid());
        }
    }

    #[test]
    fn tunnel_extracts_only_single_crusher_output_then_reverses_into_its_recipe_input() {
        let mut chunks = ChunkStore::default();
        let mut journal = EditJournal::default();
        let crusher = [8, 2, 7];
        let port = [8, 2, 8];
        let belt = [8, 1, 8];
        place(
            &mut chunks,
            &mut journal,
            crusher,
            99,
            BlockObjectState::default(),
        );
        journal.set_crusher(crusher, Default::default()).unwrap();
        place(
            &mut chunks,
            &mut journal,
            port,
            152,
            BlockObjectState {
                peer: Some(crusher),
                ..Default::default()
            },
        );
        place(
            &mut chunks,
            &mut journal,
            belt,
            148,
            BlockObjectState::default(),
        );
        journal.replace_kinetic_states(BTreeMap::from([(
            belt,
            kinetic::KineticState {
                rpm: 30.0,
                ..Default::default()
            },
        )]));
        let recipe = &terra_core::crusher::recipes()[0];
        journal
            .set_container(
                crusher,
                Inventory {
                    slots: vec![
                        ItemStack {
                            item: recipe.input,
                            count: 2,
                        },
                        ItemStack {
                            item: recipe.output.item,
                            count: 8,
                        },
                    ],
                    ..Default::default()
                },
            )
            .unwrap();
        let mut runtime = Runtime::default();
        advance(
            &mut runtime,
            &chunks,
            &mut journal,
            &Default::default(),
            0.5,
        );
        assert_eq!(
            journal
                .container_at(crusher)
                .unwrap()
                .slot(0)
                .unwrap()
                .count,
            2
        );
        assert_eq!(
            journal
                .container_at(crusher)
                .unwrap()
                .slot(1)
                .unwrap()
                .count,
            7
        );
        assert_eq!(
            journal.gameplay().drops[0].stack,
            ItemStack {
                item: recipe.output.item,
                count: 1
            }
        );
        journal.gameplay_mut().drops.clear();
        let center = terra_world::conveyors::segment_at(&journal, belt)
            .unwrap()
            .drop_center()
            .unwrap();
        assert!(journal.gameplay_mut().drop_stack(
            center,
            ItemStack {
                item: recipe.input,
                count: 3
            },
            [0.0; 3],
            0.0
        ));
        journal
            .set_object_state(
                port,
                Some(BlockObjectState {
                    facing: 4,
                    peer: Some(crusher),
                    ..Default::default()
                }),
            )
            .unwrap();
        advance(
            &mut runtime,
            &chunks,
            &mut journal,
            &Default::default(),
            0.5,
        );
        assert_eq!(journal.gameplay().drops[0].stack.count, 2);
        assert_eq!(
            journal
                .container_at(crusher)
                .unwrap()
                .slot(0)
                .unwrap()
                .count,
            3
        );
        assert_eq!(
            journal
                .container_at(crusher)
                .unwrap()
                .slot(1)
                .unwrap()
                .count,
            7
        );
        assert!(journal.gameplay().valid());
    }

    #[test]
    fn hopper_extracts_only_output_and_backpressures_at_a_blocked_belt() {
        let (chunks, mut journal, source, _) = fixture();
        let mut runtime = Runtime::default();
        let properties = Default::default();
        advance(&mut runtime, &chunks, &mut journal, &properties, 0.5);
        assert_eq!(journal.gameplay().drops.len(), 1);
        assert_eq!(journal.gameplay().drops[0].stack.count, 1);
        assert_eq!(
            journal.container_at(source).unwrap().slot(2).unwrap().count,
            7
        );
        for _ in 0..10 {
            advance(&mut runtime, &chunks, &mut journal, &properties, 0.5);
        }
        assert_eq!(journal.gameplay().drops.len(), 1);
        assert_eq!(
            journal.container_at(source).unwrap().slot(2).unwrap().count,
            7
        );
    }
    #[test]
    fn importer_stores_real_belt_stacks_and_unloaded_sources_do_no_work() {
        let (mut chunks, mut journal, source, belt) = fixture();
        let mut runtime = Runtime::default();
        let properties = Default::default();
        advance(&mut runtime, &chunks, &mut journal, &properties, 0.5);
        journal.record([2, 2, 3], 0).unwrap();
        chunks
            .loaded
            .get_mut(&SectionPos::default())
            .unwrap()
            .data
            .set_block([2, 2, 3], 0)
            .unwrap();
        let chest = [2, 2, 4];
        place(
            &mut chunks,
            &mut journal,
            chest,
            29,
            BlockObjectState::default(),
        );
        place(
            &mut chunks,
            &mut journal,
            [2, 2, 3],
            logistics::IMPORTER,
            BlockObjectState {
                peer: Some(chest),
                ..Default::default()
            },
        );
        advance(&mut runtime, &chunks, &mut journal, &properties, 0.5);
        assert!(journal.gameplay().drops.is_empty());
        assert_eq!(
            journal.container_at(chest).unwrap().slot(0).unwrap().count,
            1
        );
        chunks.loaded.clear();
        advance(&mut runtime, &chunks, &mut journal, &properties, 1.0);
        assert_eq!(
            journal.container_at(source).unwrap().slot(2).unwrap().count,
            7
        );
        assert!(runtime.cursor.is_none());
        assert_eq!(journal.kinetic_state(belt).rpm, 30.0);
    }

    #[test]
    fn reversing_a_tunnel_exports_a_chest_then_reversing_again_imports_without_loss() {
        let (mut chunks, mut journal, source, _) = fixture();
        let port = [2, 2, 3];
        place(
            &mut chunks,
            &mut journal,
            source,
            29,
            BlockObjectState::default(),
        );
        let item = ItemKind::Mineral(MineralItem::IronIngot);
        journal
            .set_container(
                source,
                Inventory {
                    slots: vec![ItemStack { item, count: 9 }],
                    ..Default::default()
                },
            )
            .unwrap();
        let mut runtime = Runtime::default();
        let properties = Default::default();
        advance(&mut runtime, &chunks, &mut journal, &properties, 0.5);
        assert_eq!(
            journal.container_at(source).unwrap().slot(0).unwrap().count,
            8
        );
        assert_eq!(journal.gameplay().drops[0].stack.count, 1);
        let state = journal.object_state(port).unwrap();
        journal
            .set_object_state(
                port,
                Some(BlockObjectState {
                    facing: state.facing ^ 4,
                    ..state
                }),
            )
            .unwrap();
        advance(&mut runtime, &chunks, &mut journal, &properties, 0.5);
        assert!(journal.gameplay().drops.is_empty());
        assert_eq!(
            journal.container_at(source).unwrap().slot(0).unwrap().count,
            9
        );
    }

    #[test]
    fn cranked_owned_belt_delivers_machine_output_into_a_real_chest() {
        let (mut chunks, mut journal, source, old_belt) = fixture();
        chunks
            .loaded
            .get_mut(&SectionPos::default())
            .unwrap()
            .data
            .set_block(global_to_section(old_belt).1, 0)
            .unwrap();
        journal.record(old_belt, 0).unwrap();
        let start = [2, 1, 2];
        let end = [2, 1, 9];
        let crank = [3, 1, 2];
        for p in [start, end] {
            place(
                &mut chunks,
                &mut journal,
                p,
                144,
                BlockObjectState::default(),
            );
        }
        place(
            &mut chunks,
            &mut journal,
            crank,
            100,
            BlockObjectState {
                peer: Some(start),
                ..Default::default()
            },
        );
        let plan =
            terra_world::conveyors::layout(&chunks, &journal, start, end, ItemKind::Block(148))
                .unwrap();
        for edit in &plan.edits {
            let state = plan
                .states
                .iter()
                .find(|(p, _)| *p == edit.position)
                .unwrap()
                .1
                .unwrap();
            place(
                &mut chunks,
                &mut journal,
                edit.position,
                edit.material,
                state,
            );
        }
        let chest = [3, 2, 8];
        place(
            &mut chunks,
            &mut journal,
            chest,
            29,
            BlockObjectState::default(),
        );
        place(
            &mut chunks,
            &mut journal,
            [2, 2, 8],
            logistics::IMPORTER,
            BlockObjectState {
                peer: Some(chest),
                facing: 1,
                ..Default::default()
            },
        );
        let terrain = terra_world::TerraGenerator::new(terra_core::WorldConfig::default()).unwrap();
        let mut kinetic_runtime = super::super::kinetics::Runtime::default();
        let mut runtime = Runtime::default();
        let properties = Default::default();
        let active = std::collections::HashSet::from([crank]);
        for _ in 0..120 {
            super::super::kinetics::advance(
                &mut kinetic_runtime,
                &terrain,
                &chunks,
                &mut journal,
                &active,
            );
            super::super::kinetics::transport(
                &mut kinetic_runtime,
                &chunks,
                &mut journal,
                &properties,
                0.1,
            );
            advance(&mut runtime, &chunks, &mut journal, &properties, 0.1);
        }
        assert_eq!(journal.kinetic_state(start).rpm, 30.0);
        assert!(journal.container_at(source).unwrap().slot(2).is_none());
        assert!(journal.gameplay().drops.is_empty());
        assert_eq!(
            journal.container_at(chest).unwrap().slot(0).unwrap().count,
            8
        );
        assert!(journal.gameplay().valid());
    }
}
