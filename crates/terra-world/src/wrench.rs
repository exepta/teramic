//! Bounded, inventory-preserving rotations shared by prediction and authority.
use crate::{
    edits::{BlockObjectState, EditJournal},
    placement::{self, Layout},
    streaming::{BlockEditRequest, ChunkStore},
};
use std::collections::{BTreeMap, BTreeSet};
use terra_core::{construction, kinetic};

type Cell = [i64; 3];

pub fn supported(material: u16) -> bool {
    matches!(material, 28..=29 | 59..=73 | 99..=100 | 139..=160 | 163..=168 | 170..=172)
        || construction::is_stair(material)
        || construction::is_slab(material)
        || terra_core::roads::is_shape(material)
        || terra_core::castle::is_castle(material)
        || terra_core::train_parts::is_train(material)
}

fn cycle(value: u8, count: u8, reverse: bool) -> u8 {
    (value + if reverse { count - 1 } else { 1 }) % count
}

fn turn([x, y, z]: Cell, reverse: bool) -> Cell {
    if reverse { [-z, y, x] } else { [z, y, -x] }
}

fn relative(position: Cell, anchor: Cell) -> Cell {
    std::array::from_fn(|axis| position[axis] - anchor[axis])
}

fn add(position: Cell, direction: Cell) -> Cell {
    std::array::from_fn(|axis| position[axis] + direction[axis])
}

fn facing(material: u16, state: BlockObjectState, reverse: bool) -> Result<u8, String> {
    if terra_core::plumbing::is_pipe(material) || material == terra_core::plumbing::VALVE {
        return Ok((state.facing & 8) | cycle(state.facing & 7, 6, reverse));
    }
    if construction::is_stair(material) || terra_core::castle::is_castle(material) {
        return Ok(cycle(state.facing, 24, reverse));
    }
    if construction::is_slab(material) {
        const HALVES: [u8; 6] = [0, 4, 5, 6, 7, 8];
        let halves = construction::slab_halves(material, state.facing);
        let half = halves.map_or(state.facing, |halves| halves[0].1);
        let index = HALVES
            .iter()
            .position(|&facing| facing == half)
            .unwrap_or(0);
        let next = HALVES[usize::from(cycle(index as u8, 6, reverse))];
        return Ok(if let Some(halves) = halves {
            construction::merged_slab(material, next, halves[1].0)
                .ok_or("Invalid combined slab rotation")?
        } else {
            next
        });
    }
    if kinetic::is_rotary(material) {
        if material == kinetic::WATER_WHEEL && state.facing >= 4 {
            return Err("Water wheels require a horizontal axle".into());
        }
        return Ok(cycle(
            state.facing,
            if material == kinetic::WATER_WHEEL {
                4
            } else {
                6
            },
            reverse,
        ));
    }
    if material == 60 && state.facing >= 8 {
        let ports =
            crate::objects::chimney_ports(material, state.facing).ok_or("Invalid chimney ports")?;
        return crate::objects::chimney_pose(ports.map(|port| turn(port, reverse)))
            .map(|pose| pose + if state.facing >= 32 { 32 } else { 0 })
            .ok_or_else(|| "Invalid rotated chimney ports".into());
    }
    // Color/open state is encoded above the low two yaw bits.
    Ok((state.facing / 4) * 4 + cycle(state.facing % 4, 4, reverse))
}

fn existing(
    chunks: &ChunkStore,
    journal: &EditJournal,
    position: Cell,
    material: u16,
) -> Result<BlockObjectState, String> {
    if chunks.block_at(position) != Some(material) {
        return Err("Rotating object is not loaded".into());
    }
    let state = journal.object_state(position).unwrap_or_default();
    if !state.valid_for(position, material) {
        return Err("Invalid object attachment".into());
    }
    Ok(state)
}

/// Keep a fixed primary and rotate its complete peer/crank footprint around it.
pub fn propose(
    chunks: &ChunkStore,
    journal: &EditJournal,
    position: Cell,
    reverse: bool,
) -> Result<Layout, String> {
    if position
        .iter()
        .any(|coordinate| coordinate.unsigned_abs() > 990_000)
    {
        return Err("Invalid rotation coordinates".into());
    }
    let material = chunks
        .block_at(position)
        .ok_or("Rotating object is not loaded")?;
    if material == 148
        && journal
            .object_state(position)
            .is_some_and(|s| s.peer.is_some())
        || matches!(material, 144..=146)
            && !crate::conveyors::connected_at(journal, position).is_empty()
    {
        return Err("Remove the connected conveyor before rotating its shafts".into());
    }
    if !supported(material) {
        return Err("This block has no rotatable pose".into());
    }
    if material == terra_core::crusher::CRANK_MATERIAL {
        let owner = journal
            .crank_anchor(position)
            .ok_or("Crank has no valid owner")?;
        return propose(chunks, journal, owner, reverse);
    }
    let selected = existing(chunks, journal, position, material)?;
    if terra_core::logistics::is_port(material) {
        let plan = Layout {
            edits: vec![BlockEditRequest {
                position,
                expected_material: material,
                material,
            }],
            states: vec![(
                position,
                Some(BlockObjectState {
                    facing: if terra_core::logistics::is_router(material) {
                        (selected.facing + if reverse { 3 } else { 1 }) % 4
                    } else {
                        selected.facing ^ 4
                    },
                    ..selected
                }),
            )],
            belts: Vec::new(),
        };
        validate(&plan, chunks, journal)?;
        placement::environment(&plan, chunks, journal, false)?;
        return Ok(plan);
    }
    let anchor = if selected.secondary {
        selected.peer.ok_or("Missing primary object")?
    } else {
        position
    };
    let primary = existing(chunks, journal, anchor, material)?;
    if primary.secondary {
        return Err("Invalid primary object".into());
    }
    let next = facing(material, primary, reverse)?;
    let mut old = BTreeMap::from([(anchor, (material, primary))]);
    let mut new = BTreeMap::from([(
        anchor,
        (
            material,
            BlockObjectState {
                facing: next,
                ..primary
            },
        ),
    )]);
    if let Some(peer) = primary.peer {
        let other = existing(chunks, journal, peer, material)?;
        if !other.secondary || other.peer != Some(anchor) || other.facing != primary.facing {
            return Err("Object pair is incomplete".into());
        }
        let target = add(anchor, turn(relative(peer, anchor), reverse));
        old.insert(peer, (material, other));
        new.get_mut(&anchor).unwrap().1.peer = Some(target);
        new.insert(
            target,
            (
                material,
                BlockObjectState {
                    facing: next,
                    peer: Some(anchor),
                    ..other
                },
            ),
        );
    } else if matches!(material, 28 | 59 | 65..=67 | 71..=73 | 139) {
        return Err("Object pair is incomplete".into());
    }
    if material == 99 || kinetic::is_crank_mount(material) {
        let candidates: Vec<_> = if material == 99 {
            journal.crusher_crank_position(anchor).into_iter().collect()
        } else {
            kinetic::DIRECTIONS
                .into_iter()
                .map(|direction| add(anchor, direction))
                .collect()
        };
        for cell in candidates {
            if journal.block(cell) != Some(100) && chunks.block_at(cell) != Some(100) {
                continue;
            }
            let crank = journal.object_state(cell).ok_or("Invalid mounted crank")?;
            if crank.peer != Some(anchor) {
                continue;
            }
            if journal.crank_anchor(cell) != Some(anchor) || chunks.block_at(cell) != Some(100) {
                return Err("Mounted crank is not loaded or valid".into());
            }
            let (target, crank_facing) = if material == 99 {
                (add(anchor, turn(relative(cell, anchor), reverse)), next)
            } else if kinetic::is_gearbox(material) {
                let direction = turn(relative(cell, anchor), reverse);
                (
                    add(anchor, direction),
                    kinetic::facing(direction).ok_or("Invalid gearbox crank port")?,
                )
            } else {
                let old_axis = kinetic::axis(primary.facing);
                let crank_axis = kinetic::axis(crank.facing);
                let sign: i64 = old_axis
                    .into_iter()
                    .zip(crank_axis)
                    .map(|(a, b)| a * b)
                    .sum();
                let direction = kinetic::axis(next).map(|value| value * sign);
                (
                    add(anchor, direction),
                    kinetic::facing(direction).ok_or("Invalid crank axis")?,
                )
            };
            old.insert(cell, (100, crank));
            if new
                .insert(
                    target,
                    (
                        100,
                        BlockObjectState {
                            facing: crank_facing,
                            ..crank
                        },
                    ),
                )
                .is_some()
            {
                return Err("Rotated object attachments overlap".into());
            }
        }
    }
    let positions: BTreeSet<_> = old.keys().chain(new.keys()).copied().collect();
    let mut plan = Layout {
        edits: Vec::with_capacity(positions.len()),
        states: Vec::with_capacity(positions.len()),
        belts: Vec::new(),
    };
    for cell in std::iter::once(anchor).chain(positions.into_iter().filter(|cell| *cell != anchor))
    {
        let expected = chunks
            .block_at(cell)
            .ok_or("Rotation footprint is not loaded")?;
        if !old.contains_key(&cell) && expected != 0 {
            return Err("Rotation is obstructed".into());
        }
        let proposed = new.get(&cell).copied();
        plan.edits.push(BlockEditRequest {
            position: cell,
            expected_material: expected,
            material: proposed.map_or(0, |(material, _)| material),
        });
        plan.states.push((cell, proposed.map(|(_, state)| state)));
    }
    validate(&plan, chunks, journal)?;
    placement::environment(&plan, chunks, journal, false)?;
    Ok(plan)
}

fn validate(plan: &Layout, chunks: &ChunkStore, journal: &EditJournal) -> Result<(), String> {
    let anchor = plan.edits.first().ok_or("Empty rotation")?.position;
    if plan.edits.len() > 7 || plan.states.len() != plan.edits.len() {
        return Err("Invalid rotation footprint".into());
    }
    let positions: BTreeSet<_> = plan.edits.iter().map(|edit| edit.position).collect();
    if positions.len() != plan.edits.len()
        || plan
            .states
            .iter()
            .map(|(position, _)| *position)
            .collect::<BTreeSet<_>>()
            != positions
    {
        return Err("Invalid rotation cells".into());
    }
    let inventory = |after: bool| {
        let mut counts = BTreeMap::new();
        for edit in &plan.edits {
            let material = if after {
                edit.material
            } else {
                edit.expected_material
            };
            if material != 0 {
                *counts.entry(material).or_insert(0_u8) += 1;
            }
        }
        counts
    };
    if plan.edits[0].material == 0
        || plan.edits[0].expected_material != plan.edits[0].material
        || inventory(false) != inventory(true)
    {
        return Err("Rotation must preserve the primary and all parts".into());
    }
    for edit in &plan.edits {
        if edit
            .position
            .iter()
            .any(|coordinate| coordinate.unsigned_abs() > 990_000)
            || relative(edit.position, anchor)
                .iter()
                .any(|delta| delta.abs() > 2)
            || chunks.block_at(edit.position) != Some(edit.expected_material)
            || edit.material != 0 && !supported(edit.material)
            || edit.expected_material != 0 && !supported(edit.expected_material)
        {
            return Err("Rotation footprint changed or is invalid".into());
        }
        let state = plan
            .states
            .iter()
            .find(|(position, _)| *position == edit.position)
            .unwrap()
            .1;
        if state.is_some_and(|state| !state.valid_for(edit.position, edit.material))
            || edit.material != 0 && state.is_none()
            || edit.material == 0 && state.is_some()
        {
            return Err("Invalid rotated object state".into());
        }
        if edit.expected_material != 0 && edit.material == 0 {
            let above = add(edit.position, [0, 1, 0]);
            if !positions.contains(&above) && chunks.block_at(above) != Some(0) {
                return Err("Remove the supported block before rotating this part".into());
            }
        }
    }
    // A moved chest half must never overwrite unrelated private storage.
    for (from, to) in relocated_furniture(plan, journal) {
        if from != to && journal.container_at(to).is_some() {
            return Err("Rotation destination already has container state".into());
        }
    }
    Ok(())
}

pub(crate) fn relocated_furniture(plan: &Layout, journal: &EditJournal) -> Vec<(Cell, Cell)> {
    plan.edits
        .iter()
        .filter(|edit| matches!(edit.expected_material, 28..=29) && edit.material == 0)
        .filter_map(|edit| {
            let primary = journal.object_state(edit.position)?.peer?;
            let next = plan
                .states
                .iter()
                .find(|(position, _)| *position == primary)?
                .1?
                .peer?;
            Some((edit.position, next))
        })
        .collect()
}

/// Commit only validated rotations. No mining, drops or support reconciliation.
pub fn commit(
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
    plan: &Layout,
) -> Result<bool, String> {
    validate(plan, chunks, journal)?;
    placement::environment(plan, chunks, journal, false)?;
    journal.commit_rotation(chunks, plan)
}

#[cfg(test)]
mod tests;
