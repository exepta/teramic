use super::{Cell, cell_boxes, cells, complete, object_state, owner_at};
use crate::{
    edits::EditJournal,
    placement::Layout,
    streaming::{BlockEditRequest, ChunkStore},
};
use std::collections::BTreeSet;
use terra_core::{block_shapes::ShapeBox, rails::Piece};
use terra_voxel::global_to_section;

const MAX_MODULE_CELLS: usize = 512;

/// Snap to an actual free endpoint of the clicked module, including its grade.
/// Only module ports and four yaws are inspected; this never searches terrain.
pub fn placement(
    chunks: &ChunkStore,
    journal: &EditJournal,
    hit: terra_voxel::blocks::BlockHit,
    material: u16,
    ray: ([f64; 3], [f64; 3]),
    look: [f32; 2],
) -> Result<Layout, String> {
    placement_candidate(chunks, journal, hit, material, ray, look, true)
}

/// A non-authoritative silhouette candidate, retained when support or space is
/// invalid. Only `placement`, `plan` and `commit` authorize world changes.
pub fn proposal(
    chunks: &ChunkStore,
    journal: &EditJournal,
    hit: terra_voxel::blocks::BlockHit,
    material: u16,
    ray: ([f64; 3], [f64; 3]),
    look: [f32; 2],
) -> Result<Layout, String> {
    placement_candidate(chunks, journal, hit, material, ray, look, false)
}

fn placement_candidate(
    chunks: &ChunkStore,
    journal: &EditJournal,
    hit: terra_voxel::blocks::BlockHit,
    material: u16,
    ray: ([f64; 3], [f64; 3]),
    look: [f32; 2],
    validate: bool,
) -> Result<Layout, String> {
    if ray
        .0
        .iter()
        .chain(ray.1.iter())
        .any(|value| !value.is_finite())
        || look.iter().any(|value| !value.is_finite())
        || !hit.distance.is_finite()
        || hit.distance < 0.0
    {
        return Err("Invalid rail placement ray".into());
    }
    let preferred = crate::edits::BlockObjectState::from_yaw(look[0] + std::f32::consts::PI).facing;
    let Some(existing) = owner_at(journal, hit.position) else {
        return if validate {
            plan(chunks, journal, hit.adjacent(), material, preferred)
        } else {
            candidate(chunks, hit.adjacent(), material, preferred)
        };
    };
    if !complete(journal, existing) {
        return Err("The clicked rail module is incomplete".into());
    }
    let point: [f64; 3] = std::array::from_fn(|axis| ray.0[axis] + ray.1[axis] * hit.distance);
    let distance = |position: [f64; 3], origin: [f64; 3]| {
        (0..3)
            .map(|axis| (position[axis] - origin[axis]).powi(2))
            .sum::<f64>()
    };
    let mut ports = existing.ports();
    ports.retain(|port| {
        port.connectable
            && distance(port.position.position(), ray.0) <= 4.5 * 4.5
            && !port_connected(journal, existing, *port)
    });
    ports.sort_by(|first, second| {
        distance(first.position.position(), point)
            .total_cmp(&distance(second.position.position(), point))
            .then(first.index.cmp(&second.index))
    });
    let mut last_error =
        "No free rail endpoint with a compatible grade is within reach".to_string();
    for port in ports {
        for turn in [0, 1, 3, 2] {
            let yaw = (preferred + turn) % 4;
            let local = Piece::from_facing([0; 3], material, yaw).ok_or("Invalid rail module")?;
            for incoming in local.ports() {
                if !incoming.connectable || port.tangent.map(|value| -value) != incoming.tangent {
                    continue;
                }
                let translation: [i64; 3] =
                    std::array::from_fn(|axis| port.position.0[axis] - incoming.position.0[axis]);
                if translation
                    .iter()
                    .any(|value| value.rem_euclid(terra_core::rails::FIXED_SCALE) != 0)
                {
                    continue;
                }
                let anchor = translation.map(|value| value / terra_core::rails::FIXED_SCALE);
                let layout = if validate {
                    plan(chunks, journal, anchor, material, yaw)
                } else {
                    candidate(chunks, anchor, material, yaw)
                };
                match layout {
                    Ok(layout) => return Ok(layout),
                    Err(error) => last_error = error,
                }
            }
        }
    }
    Err(last_error)
}

fn port_connected(journal: &EditJournal, piece: Piece, port: terra_core::rails::Port) -> bool {
    let center = global_to_section(
        port.position
            .position()
            .map(|coordinate| coordinate.floor() as i64),
    )
    .0;
    journal
        .rail_sections()
        .filter(|section| {
            section.x.abs_diff(center.x) <= 2
                && section.y.abs_diff(center.y) <= 1
                && section.z.abs_diff(center.z) <= 2
        })
        .any(|section| {
            journal
                .object_states_in_section(section)
                .any(|(cell, material, state)| {
                    terra_core::rails::is_rail(material)
                        && !state.secondary
                        && cell != piece.anchor
                        && owner_at(journal, cell).is_some_and(|other| {
                            other
                                .ports()
                                .into_iter()
                                .any(|other_port| port.connects(other_port))
                        })
                })
        })
}

/// A placement reserves only the authored module footprint, never its bounds box.
pub fn plan(
    chunks: &ChunkStore,
    journal: &EditJournal,
    anchor: Cell,
    material: u16,
    facing: u8,
) -> Result<Layout, String> {
    let result = candidate(chunks, anchor, material, facing)?;
    environment(&result, chunks, journal)?;
    Ok(result)
}

fn candidate(
    chunks: &ChunkStore,
    anchor: Cell,
    material: u16,
    facing: u8,
) -> Result<Layout, String> {
    let piece = Piece::from_facing(anchor, material, facing).ok_or("Invalid rail module")?;
    let footprint = cells(piece);
    if footprint.is_empty() || footprint.len() > MAX_MODULE_CELLS {
        return Err("Rail module exceeds its placement budget".into());
    }
    let mut result = empty_layout();
    for cell in footprint {
        if cell.iter().any(|value| value.unsigned_abs() > 990_000) {
            return Err("Rail module exceeds world bounds".into());
        }
        let previous = chunks.block_at(cell).unwrap_or(0);
        result.edits.push(BlockEditRequest {
            position: cell,
            expected_material: previous,
            material,
        });
        result.states.push((cell, Some(object_state(piece, cell))));
    }
    Ok(result)
}

fn empty_layout() -> Layout {
    Layout {
        edits: Vec::new(),
        states: Vec::new(),
        belts: Vec::new(),
    }
}

/// Any slice addresses the whole module; an incomplete save/import fails closed.
pub fn break_layout(
    chunks: &ChunkStore,
    journal: &EditJournal,
    cell: Cell,
) -> Result<Layout, String> {
    let piece = owner_at(journal, cell).ok_or("Rail module has no canonical owner")?;
    if !complete(journal, piece) {
        return Err("Rail module metadata is incomplete".into());
    }
    let mut result = empty_layout();
    for position in cells(piece) {
        if chunks
            .block_at(position)
            .or_else(|| journal.block(position))
            != Some(piece.material)
        {
            return Err("Rail module changed".into());
        }
        result.edits.push(BlockEditRequest {
            position,
            expected_material: piece.material,
            material: 0,
        });
        result.states.push((position, None));
    }
    Ok(result)
}

/// The reserved footprint stays fixed when a switch selects its other route.
pub fn toggle_switch_plan(
    chunks: &ChunkStore,
    journal: &EditJournal,
    cell: Cell,
) -> Result<Layout, String> {
    let old = owner_at(journal, cell).ok_or("Rail module has no canonical owner")?;
    let switched = Piece::from_facing(old.anchor, old.material, old.encoded_facing() ^ 4)
        .ok_or("Only rail switches can change route")?;
    let mut result = break_layout(chunks, journal, cell)?;
    for edit in &mut result.edits {
        edit.material = old.material;
    }
    for (position, state) in &mut result.states {
        *state = Some(object_state(switched, *position));
    }
    Ok(result)
}

fn proposed_piece(plan: &Layout) -> Option<Piece> {
    plan.states.iter().find_map(|(cell, state)| {
        let state = (*state)?;
        if state.secondary || state.peer != Some(*cell) {
            return None;
        }
        let material = plan
            .edits
            .iter()
            .find(|edit| edit.position == *cell)?
            .material;
        Piece::from_facing(*cell, material, state.facing)
    })
}

/// Require a complete top face; lower slabs, furniture and other rails cannot float a module.
fn supports_floor(journal: &EditJournal, cell: Cell, material: u16) -> bool {
    supports_floor_with_state(material, journal.object_state(cell))
}

fn supports_floor_with_state(material: u16, state: Option<crate::edits::BlockObjectState>) -> bool {
    if terra_core::rails::is_rail(material) || material == 14 {
        return false;
    }
    if terra_core::construction::is_slab(material) {
        return state.is_some_and(|state| {
            state.facing == 4 || terra_core::construction::full_slab(state.facing)
        });
    }
    terra_core::beds::supports_bed_floor(material)
}

fn supported(piece: Piece, chunks: &ChunkStore, journal: &EditJournal) -> bool {
    piece.supports().iter().all(|point| {
        let support =
            std::array::from_fn(|axis| piece.anchor[axis] + i64::from(point.support[axis]));
        chunks
            .block_at(support)
            .is_some_and(|material| supports_floor(journal, support, material))
    })
}

fn overlaps(first: &ShapeBox, second: [[f64; 3]; 2]) -> bool {
    (0..3).all(|axis| {
        first.minimum[axis] < second[1][axis] - 1e-7 && first.maximum[axis] > second[0][axis] + 1e-7
    })
}

/// Check support and real occupied prop space using the same shape as collision.
pub fn environment(
    plan: &Layout,
    chunks: &ChunkStore,
    journal: &EditJournal,
) -> Result<(), String> {
    if journal.rail_module_count() >= terra_core::rails::MAX_PIECES {
        return Err("Rail network has reached its module limit".into());
    }
    let piece = proposed_piece(plan).ok_or("Missing canonical rail placement state")?;
    if !supported(piece, chunks, journal) {
        return Err("Every rail sleeper needs a loaded, full-height supporting floor".into());
    }
    let positions: BTreeSet<_> = plan.edits.iter().map(|edit| edit.position).collect();
    if positions != cells(piece).into_iter().collect()
        || plan.edits.len() != positions.len()
        || plan.states.len() != positions.len()
        || !plan.belts.is_empty()
    {
        return Err("Rail layout is incomplete".into());
    }
    let mut sections = BTreeSet::new();
    for edit in &plan.edits {
        if journal.has_extended_kinetics() {
            crate::placement::kinetic_clearance(
                plan,
                chunks,
                journal,
                edit.position,
                edit.material,
                false,
            )?;
        }
        if edit.material != piece.material
            || chunks.block_at(edit.position) != Some(edit.expected_material)
            || !matches!(edit.expected_material, 0 | 14 | 27)
            || journal.object_state(edit.position).is_some()
            || plan
                .states
                .iter()
                .find(|(cell, _)| *cell == edit.position)
                .and_then(|(_, state)| *state)
                != Some(object_state(piece, edit.position))
        {
            return Err("Rail footprint is occupied or changed".into());
        }
        let owner = global_to_section(edit.position).0;
        for x in -1..=1 {
            for y in -1..=1 {
                for z in -1..=1 {
                    sections.insert(owner.offset([x, y, z]).map_err(|error| error.to_string())?);
                }
            }
        }
    }
    let bounds: Vec<_> = plan
        .edits
        .iter()
        .flat_map(|edit| {
            cell_boxes(
                edit.position,
                piece.material,
                object_state(piece, edit.position),
            )
            .iter()
            .copied()
            .map(move |mut shape| {
                for axis in 0..3 {
                    shape.minimum[axis] += edit.position[axis] as f64;
                    shape.maximum[axis] += edit.position[axis] as f64;
                }
                shape
            })
        })
        .collect();
    for section in sections {
        for prop in chunks
            .loaded
            .get(&section)
            .into_iter()
            .flat_map(|section| section.props.iter())
            .chain(journal.placed_props(section))
        {
            if !prop.removed
                && !journal.prop_removed(prop.section, prop.id)
                && bounds
                    .iter()
                    .any(|shape| overlaps(shape, prop.placement_bounds()))
            {
                return Err("Rail geometry intersects a prop".into());
            }
        }
    }
    Ok(())
}

/// Removing a sleeper's floor is rejected until its entire module is removed.
/// The journal lookup also protects a rail whose section is currently unloaded.
pub fn support_edit_allowed(
    _chunks: &ChunkStore,
    journal: &EditJournal,
    position: Cell,
    new_material: u16,
) -> bool {
    support_state_allowed(
        journal,
        position,
        new_material,
        journal.object_state(position),
    )
}

pub(crate) fn support_state_allowed(
    journal: &EditJournal,
    position: Cell,
    new_material: u16,
    state: Option<crate::edits::BlockObjectState>,
) -> bool {
    if supports_floor_with_state(new_material, state) {
        return true;
    }
    let Some(above_y) = position[1].checked_add(1) else {
        return false;
    };
    let above = [position[0], above_y, position[2]];
    let Some(piece) = super::piece_at(journal, above) else {
        return true;
    };
    !piece.supports().iter().any(|point| {
        position == std::array::from_fn(|axis| piece.anchor[axis] + i64::from(point.support[axis]))
    })
}

/// No item ownership changes here: the server consumes/returns one module item.
pub fn commit(
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
    plan: &Layout,
) -> Result<bool, String> {
    journal.commit_rail_layout(chunks, plan)
}

pub(crate) fn validate_commit(
    plan: &Layout,
    chunks: &ChunkStore,
    journal: &EditJournal,
) -> Result<(), String> {
    if plan.edits.is_empty() || plan.edits.len() > MAX_MODULE_CELLS || !plan.belts.is_empty() {
        return Err("Invalid rail transaction size".into());
    }
    let positions: BTreeSet<_> = plan.edits.iter().map(|edit| edit.position).collect();
    let states: BTreeSet<_> = plan.states.iter().map(|(cell, _)| *cell).collect();
    if positions.len() != plan.edits.len()
        || states.len() != plan.states.len()
        || positions != states
    {
        return Err("Rail transaction metadata does not match its cells".into());
    }
    if plan.edits.iter().any(|edit| {
        chunks
            .block_at(edit.position)
            .or_else(|| journal.block(edit.position))
            != Some(edit.expected_material)
    }) {
        return Err("Rail transaction changed".into());
    }
    if plan
        .edits
        .iter()
        .all(|edit| matches!(edit.expected_material, 0 | 14 | 27))
    {
        return environment(plan, chunks, journal);
    }
    let first = &plan.edits[0];
    let current = owner_at(journal, first.position).ok_or("Missing rail owner")?;
    if !complete(journal, current)
        || plan.edits.len() != cells(current).len()
        || plan.states.len() != plan.edits.len()
        || plan
            .edits
            .iter()
            .map(|edit| edit.position)
            .collect::<BTreeSet<_>>()
            != cells(current).into_iter().collect()
    {
        return Err("Rail transaction does not own the complete module".into());
    }
    let removed = plan.edits.iter().all(|edit| edit.material == 0);
    if removed {
        if plan.states.iter().any(|(_, state)| state.is_some()) {
            return Err("Removed rail retains metadata".into());
        }
        return Ok(());
    }
    let next = proposed_piece(plan).ok_or("Missing rail switch owner")?;
    if next.anchor != current.anchor
        || next.material != current.material
        || next.encoded_facing() != (current.encoded_facing() ^ 4)
        || plan.edits.iter().any(|edit| edit.material != next.material)
        || plan
            .states
            .iter()
            .any(|(cell, state)| *state != Some(object_state(next, *cell)))
    {
        return Err("Invalid rail switch transaction".into());
    }
    Ok(())
}
