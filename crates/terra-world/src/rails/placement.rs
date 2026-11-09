use super::{Cell, cells, complete, object_state_with_wood, owner_at, piece_from_state};
use crate::{
    edits::EditJournal,
    placement::Layout,
    streaming::{BlockEditRequest, ChunkStore},
};
use std::collections::BTreeSet;
use terra_core::rails::{Piece, RAIL_BUILD_REACH};
use terra_voxel::global_to_section;

const MAX_MODULE_CELLS: usize = 1024;

pub fn set_wood(plan: &mut Layout, wood: terra_core::WoodSpecies) {
    for (_, state) in &mut plan.states {
        if let Some(state) = state {
            state.wood = wood;
        }
    }
}

pub fn cost(plan: &Layout) -> Option<u16> {
    proposed_piece(plan).map(super::piece_cost)
}

/// Place a new one-metre starter on the actually hit face. Existing modules
/// are selected as connection sources instead of receiving implicit extensions.
pub fn seed(
    chunks: &ChunkStore,
    journal: &EditJournal,
    hit: terra_voxel::blocks::BlockHit,
    ray: ([f64; 3], [f64; 3]),
    look: [f32; 2],
    wood: terra_core::WoodSpecies,
) -> Result<Layout, String> {
    let plan = proposal_seed(chunks, journal, hit, ray, look, wood)?;
    environment(&plan, chunks, journal)?;
    Ok(plan)
}

/// Keep a real one-by-three starter silhouette visible when its support or
/// occupied cells are invalid; only `seed` and `commit` authorize publication.
pub fn proposal_seed(
    chunks: &ChunkStore,
    journal: &EditJournal,
    hit: terra_voxel::blocks::BlockHit,
    ray: ([f64; 3], [f64; 3]),
    look: [f32; 2],
    wood: terra_core::WoodSpecies,
) -> Result<Layout, String> {
    if ray
        .0
        .iter()
        .chain(ray.1.iter())
        .any(|value| !value.is_finite())
        || look.iter().any(|value| !value.is_finite())
        || !hit.distance.is_finite()
        || !(0.0..=RAIL_BUILD_REACH).contains(&hit.distance)
        || hit
            .position
            .iter()
            .any(|value| value.unsigned_abs() > 990_000)
        || !matches!(
            hit.normal,
            [1, 0, 0] | [-1, 0, 0] | [0, 1, 0] | [0, -1, 0] | [0, 0, 1] | [0, 0, -1]
        )
    {
        return Err("Invalid track starter placement ray".into());
    }
    if chunks
        .block_at(hit.position)
        .or_else(|| journal.block(hit.position))
        .is_some_and(terra_core::rails::is_rail)
    {
        return Err("Select the free rail end, then choose its destination".into());
    }
    let facing = crate::edits::BlockObjectState::from_yaw(look[0] + std::f32::consts::PI).facing;
    let piece = Piece::seed(hit.adjacent(), facing).ok_or("Invalid track starter pose")?;
    candidate_piece(chunks, piece, wood)
}

/// Stable starter intent without copying its layout or trusting client points.
pub fn seed_signature(plan: &Layout) -> u64 {
    fn add(hash: &mut u64, value: u64) {
        for byte in value.to_le_bytes() {
            *hash ^= u64::from(byte);
            *hash = hash.wrapping_mul(0x100000001b3);
        }
    }
    let mut hash = 0xcbf29ce484222325;
    add(&mut hash, 0x73656564);
    if let Some(piece) = proposed_piece(plan) {
        add(&mut hash, 1);
        for coordinate in piece.anchor {
            add(&mut hash, coordinate as u64);
        }
        add(&mut hash, u64::from(piece.material));
        add(&mut hash, u64::from(piece.encoded_facing()));
        if let Some(spec) = piece.curve {
            add(&mut hash, 1);
            for coordinate in spec.points.into_iter().flatten() {
                add(&mut hash, coordinate as u64);
            }
        } else {
            add(&mut hash, 0);
        }
        if let Some(state) = plan
            .states
            .iter()
            .find(|(cell, _)| *cell == piece.anchor)
            .and_then(|(_, state)| *state)
        {
            add(&mut hash, state.wood as u64);
        }
    } else {
        add(&mut hash, 0);
    }
    hash
}

/// The canonical outward destination of a starter or a newly built connection.
pub fn next_endpoint(plan: &Layout) -> Option<terra_core::rails::Endpoint> {
    let piece = proposed_piece(plan)?;
    piece
        .ports()
        .into_iter()
        .find(|port| port.index == 1 && port.connectable)
        .map(|port| terra_core::rails::Endpoint {
            anchor: piece.anchor,
            port: port.index,
        })
}

/// Validate an applied receipt against complete replicated geometry and a free
/// exact endpoint. A joined existing route intentionally has no continuation.
pub fn continuation_endpoint(
    journal: &EditJournal,
    piece: Piece,
    destination: terra_core::rails::FixedPoint,
) -> Option<terra_core::rails::Endpoint> {
    if owner_at(journal, piece.anchor) != Some(piece) || !complete(journal, piece) {
        return None;
    }
    piece
        .ports()
        .into_iter()
        .find(|port| {
            port.connectable
                && port.position == destination
                && !port_connected(journal, piece, *port)
        })
        .map(|port| terra_core::rails::Endpoint {
            anchor: piece.anchor,
            port: port.index,
        })
}

/// Snapping consumes an existing module's free port, even when the new piece
/// occupies only unclaimed cells. Callers must authorize its complete footprint.
pub fn placement_sites(
    journal: &EditJournal,
    hit: terra_voxel::blocks::BlockHit,
) -> Option<Vec<Cell>> {
    if !journal
        .block(hit.position)
        .is_some_and(terra_core::rails::is_rail)
    {
        return Some(Vec::new());
    }
    let target = owner_at(journal, hit.position)?;
    complete(journal, target).then(|| cells(target))
}

/// Connecting consumes a free port on each existing module, so callers must
/// authorize every owned cell of those modules as well as the new curve.
pub fn connection_sites(
    journal: &EditJournal,
    start: terra_core::rails::Endpoint,
    hit: terra_voxel::blocks::BlockHit,
) -> Option<Vec<Cell>> {
    let source = owner_at(journal, start.anchor)?;
    if source.anchor != start.anchor
        || !complete(journal, source)
        || !source
            .ports()
            .iter()
            .any(|port| port.index == start.port && port.connectable)
    {
        return None;
    }
    let mut sites: BTreeSet<_> = cells(source).into_iter().collect();
    sites.extend(placement_sites(journal, hit)?);
    Some(sites.into_iter().collect())
}

/// The terminal physical port, independent of the clicked voxel's distance.
pub fn connection_target(plan: &Layout) -> Option<terra_core::rails::Port> {
    proposed_piece(plan)
        .filter(|piece| piece.curve.is_some())?
        .ports()
        .into_iter()
        .find(|port| port.index == 1)
}

/// Resolve the nearest free physical port of the actually hit module.
pub fn select_endpoint(
    journal: &EditJournal,
    hit: terra_voxel::blocks::BlockHit,
    ray: ([f64; 3], [f64; 3]),
) -> Option<terra_core::rails::Endpoint> {
    if ray
        .0
        .iter()
        .chain(ray.1.iter())
        .any(|value| !value.is_finite())
    {
        return None;
    }
    let piece = owner_at(journal, hit.position)?;
    if !complete(journal, piece) {
        return None;
    }
    let point = std::array::from_fn(|axis| ray.0[axis] + ray.1[axis] * hit.distance);
    nearest_free_port(journal, piece, point, Some(ray.0)).map(|port| terra_core::rails::Endpoint {
        anchor: piece.anchor,
        port: port.index,
    })
}

fn distance_squared(first: [f64; 3], second: [f64; 3]) -> f64 {
    (0..3)
        .map(|axis| (first[axis] - second[axis]).powi(2))
        .sum()
}

fn nearest_free_port(
    journal: &EditJournal,
    piece: Piece,
    point: [f64; 3],
    eye: Option<[f64; 3]>,
) -> Option<terra_core::rails::Port> {
    piece
        .ports()
        .into_iter()
        .filter(|port| {
            port.connectable
                && eye.is_none_or(|eye| {
                    distance_squared(port.position.position(), eye)
                        <= RAIL_BUILD_REACH * RAIL_BUILD_REACH
                })
                && !port_connected(journal, piece, *port)
        })
        .min_by(|first, second| {
            distance_squared(first.position.position(), point)
                .total_cmp(&distance_squared(second.position.position(), point))
                .then(first.index.cmp(&second.index))
        })
}

pub fn connection(
    chunks: &ChunkStore,
    journal: &EditJournal,
    start: terra_core::rails::Endpoint,
    hit: terra_voxel::blocks::BlockHit,
    look: [f32; 2],
    wood: terra_core::WoodSpecies,
) -> Result<Layout, String> {
    let mut first_error = None;
    for plan in connection_candidates(chunks, journal, start, hit, look, wood)? {
        match environment(&plan, chunks, journal) {
            Ok(()) => return Ok(plan),
            Err(error) => {
                first_error.get_or_insert(error);
            }
        }
    }
    Err(first_error.unwrap_or_else(|| "No free rail connection fits these ends".into()))
}

/// Keep genuine curve geometry visible when its floor or corridor is blocked.
/// A floor destination derives its tangent from the route, never the camera.
pub fn proposal_connection(
    chunks: &ChunkStore,
    journal: &EditJournal,
    start: terra_core::rails::Endpoint,
    hit: terra_voxel::blocks::BlockHit,
    look: [f32; 2],
    wood: terra_core::WoodSpecies,
) -> Result<Layout, String> {
    connection_candidates(chunks, journal, start, hit, look, wood)?
        .into_iter()
        .next()
        .ok_or_else(|| "No rail curve candidate".into())
}

/// Continuous construction follows the actual ray hit. Lateral endpoints
/// snap to half blocks; the forward plane still belongs to the real hit cell.
pub fn connection_aim(
    chunks: &ChunkStore,
    journal: &EditJournal,
    start: terra_core::rails::Endpoint,
    hit: terra_voxel::blocks::BlockHit,
    ray: ([f64; 3], [f64; 3]),
    look: [f32; 2],
    wood: terra_core::WoodSpecies,
) -> Result<Layout, String> {
    let mut first_error = None;
    for plan in connection_candidates_with_ray(chunks, journal, start, hit, look, wood, Some(ray))?
    {
        match environment(&plan, chunks, journal) {
            Ok(()) => return Ok(plan),
            Err(error) => {
                first_error.get_or_insert(error);
            }
        }
    }
    Err(first_error.unwrap_or_else(|| "No free rail connection fits these ends".into()))
}

/// Keep only genuine ray-snapped geometry when support or occupancy is invalid.
pub fn proposal_connection_aim(
    chunks: &ChunkStore,
    journal: &EditJournal,
    start: terra_core::rails::Endpoint,
    hit: terra_voxel::blocks::BlockHit,
    ray: ([f64; 3], [f64; 3]),
    look: [f32; 2],
    wood: terra_core::WoodSpecies,
) -> Result<Layout, String> {
    connection_candidates_with_ray(chunks, journal, start, hit, look, wood, Some(ray))?
        .into_iter()
        .next()
        .ok_or_else(|| "No rail curve candidate".into())
}

/// A signed continuation binds the original free endpoint and exact new geometry.
pub fn connection_signature(plan: &Layout, start: terra_core::rails::Endpoint) -> u64 {
    let mut hash = seed_signature(plan);
    for value in std::iter::once(0x73747265746368_u64)
        .chain(start.anchor.map(|coordinate| coordinate as u64))
        .chain(std::iter::once(u64::from(start.port)))
    {
        for byte in value.to_le_bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
    }
    hash
}

pub(super) fn connection_candidates(
    chunks: &ChunkStore,
    journal: &EditJournal,
    start: terra_core::rails::Endpoint,
    hit: terra_voxel::blocks::BlockHit,
    look: [f32; 2],
    wood: terra_core::WoodSpecies,
) -> Result<Vec<Layout>, String> {
    connection_candidates_with_ray(chunks, journal, start, hit, look, wood, None)
}

fn connection_candidates_with_ray(
    chunks: &ChunkStore,
    journal: &EditJournal,
    start: terra_core::rails::Endpoint,
    hit: terra_voxel::blocks::BlockHit,
    look: [f32; 2],
    wood: terra_core::WoodSpecies,
    ray: Option<([f64; 3], [f64; 3])>,
) -> Result<Vec<Layout>, String> {
    use terra_core::rails::{CurveError, CurveSpec, FixedPoint, Port};
    if look.iter().any(|value| !value.is_finite())
        || hit
            .position
            .iter()
            .any(|value| value.unsigned_abs() > 990_000)
        || !hit.distance.is_finite()
        || hit.distance < 0.0
    {
        return Err("Invalid rail connection target".into());
    }
    let piece = owner_at(journal, start.anchor).ok_or("Rail start no longer exists")?;
    if !complete(journal, piece) {
        return Err("Rail start is incomplete".into());
    }
    let start_port = piece
        .ports()
        .into_iter()
        .find(|port| port.index == start.port && port.connectable)
        .filter(|port| !port_connected(journal, piece, *port))
        .ok_or("Rail start is already connected")?;
    let position = start_port.position.position();
    let aimed = if let Some(ray) = ray {
        if ray
            .0
            .iter()
            .chain(ray.1.iter())
            .any(|value| !value.is_finite())
            || hit.distance > RAIL_BUILD_REACH
            || distance_squared(position, ray.0) > RAIL_BUILD_REACH * RAIL_BUILD_REACH + 1e-8
        {
            return Err(
                "The rail source or destination is beyond the 32-block construction reach".into(),
            );
        }
        Some(std::array::from_fn(|axis| {
            ray.0[axis] + ray.1[axis] * hit.distance
        }))
    } else {
        None
    };
    let end_ports = if let Some(target) = owner_at(journal, hit.position) {
        if !complete(journal, target) {
            return Err("Rail destination is incomplete".into());
        }
        let point = aimed.unwrap_or_else(|| hit.position.map(|coordinate| coordinate as f64 + 0.5));
        let mut ports: Vec<_> = target
            .ports()
            .into_iter()
            .filter(|port| {
                port.connectable
                    && port.position != start_port.position
                    && !port_connected(journal, target, *port)
            })
            .collect();
        ports.sort_by(|first, second| {
            distance_squared(first.position.position(), point)
                .total_cmp(&distance_squared(second.position.position(), point))
                .then(first.index.cmp(&second.index))
        });
        if ports.is_empty() {
            return Err("Rail destination has no free end; use a switch for a side branch".into());
        }
        ports
    } else {
        let cell = if hit.normal == [0, 1, 0] {
            hit.adjacent()
        } else if matches!(hit.normal, [1, 0, 0] | [-1, 0, 0] | [0, 0, 1] | [0, 0, -1]) {
            [hit.position[0], hit.position[1] + 1, hit.position[2]]
        } else {
            return Err("A free rail destination needs a floor top or a terrain-step side".into());
        };
        let elevated = cell[1] as f64 + terra_core::rails::DECK_HEIGHT > position[1] + 1e-8;
        let delta: [f64; 3] = std::array::from_fn(|axis| {
            aimed.map_or(cell[axis] as f64 + 0.5, |point| point[axis]) - position[axis]
        });
        let mut headings = [[0_i64, 0, 1], [1, 0, 0], [0, 0, -1], [-1, 0, 0]];
        let score = |heading: [i64; 3]| delta[0] * heading[0] as f64 + delta[2] * heading[2] as f64;
        let continuation = |heading: [i64; 3]| {
            i64::from(start_port.tangent[0]) * heading[0]
                + i64::from(start_port.tangent[2]) * heading[2]
        };
        headings.sort_by(|first, second| {
            score(*second)
                .total_cmp(&score(*first))
                .then(continuation(*second).cmp(&continuation(*first)))
        });
        headings
            .into_iter()
            .filter(|heading| score(*heading) > 1e-8)
            .map(|forward| {
                let local = [
                    if forward[0] == 0 {
                        8
                    } else if (forward[0] > 0) != elevated {
                        16
                    } else {
                        0
                    },
                    2,
                    if forward[2] == 0 {
                        8
                    } else if (forward[2] > 0) != elevated {
                        16
                    } else {
                        0
                    },
                ];
                Port {
                    index: 0,
                    connectable: true,
                    position: FixedPoint(std::array::from_fn(|axis| {
                        if axis != 1
                            && forward[axis] == 0
                            && let Some(point) = aimed
                        {
                            (point[axis] * 2.0).round() as i64 * 8
                        } else {
                            cell[axis] * 16 + local[axis]
                        }
                    })),
                    tangent: forward.map(|value| -value as i16),
                }
            })
            .collect()
    };
    let anchor = std::array::from_fn(|axis| {
        let step = if axis == 1 {
            0.0
        } else {
            f64::from(start_port.tangent[axis]).signum() * 1e-6
        };
        (position[axis] + step).floor() as i64
    });
    let mut candidates = Vec::new();
    let mut first_error = None;
    for end_port in end_ports {
        let result = CurveSpec::between_for_build(start_port, end_port, anchor).map_err(|error| match error {
            CurveError::Reverses => "The selected ends face away from this route; choose a free end facing the route or use a switch for a side branch".to_string(),
            CurveError::TooTight => "Rail ends are too close for the 16 m turning radius; move the destination farther away".to_string(),
            other => other.to_string(),
        }).and_then(|curve| Piece::new_curve(anchor, curve).ok_or_else(|| "Invalid rail curve".into()))
            .and_then(|curve| candidate_piece(chunks, curve, wood));
        match result {
            Ok(plan) => candidates.push(plan),
            Err(error) => {
                first_error.get_or_insert(error);
            }
        }
    }
    if candidates.is_empty() {
        Err(first_error
            .unwrap_or_else(|| "Rail destination needs horizontal room for a curve".into()))
    } else {
        Ok(candidates)
    }
}

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
            && distance(port.position.position(), ray.0) <= RAIL_BUILD_REACH * RAIL_BUILD_REACH
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

pub(super) fn port_connected(
    journal: &EditJournal,
    piece: Piece,
    port: terra_core::rails::Port,
) -> bool {
    let center = global_to_section(
        port.position
            .position()
            .map(|coordinate| coordinate.floor() as i64),
    )
    .0;
    for x in -2..=2 {
        for y in -1..=1 {
            for z in -2..=2 {
                let Ok(section) = center.offset([x, y, z]) else {
                    continue;
                };
                if journal
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
                {
                    return true;
                }
            }
        }
    }
    false
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
    candidate_piece(chunks, piece, terra_core::WoodSpecies::Oak)
}

pub(super) fn candidate_piece(
    chunks: &ChunkStore,
    piece: Piece,
    wood: terra_core::WoodSpecies,
) -> Result<Layout, String> {
    let material = piece.material;
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
        result
            .states
            .push((cell, Some(object_state_with_wood(piece, cell, wood))));
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
/// Rotate a complete turntable without consuming or dropping its one item.
pub fn turntable_plan(
    chunks: &ChunkStore,
    journal: &EditJournal,
    cell: Cell,
    clockwise: bool,
) -> Result<Layout, String> {
    let old = owner_at(journal, cell).ok_or("Missing turntable")?;
    if old.material != terra_core::rails::TURNTABLE {
        return Err("Not a turntable".into());
    }
    let next = Piece::new(
        old.anchor,
        old.material,
        (old.facing + if clockwise { 1 } else { 3 }) % 4,
    )
    .ok_or("Invalid turntable pose")?;
    let mut result = break_layout(chunks, journal, cell)?;
    let old_cells: BTreeSet<_> = cells(old).into_iter().collect();
    if cells(next).into_iter().collect::<BTreeSet<_>>() != old_cells {
        return Err("Turntable platform must be rotation invariant".into());
    }
    let wood = journal.object_state(old.anchor).unwrap().wood;
    for edit in &mut result.edits {
        edit.material = old.material;
    }
    for (position, state) in &mut result.states {
        *state = Some(object_state_with_wood(next, *position, wood));
    }
    Ok(result)
}

pub fn toggle_switch_plan(
    chunks: &ChunkStore,
    journal: &EditJournal,
    cell: Cell,
) -> Result<Layout, String> {
    let old = owner_at(journal, cell).ok_or("Rail module has no canonical owner")?;
    let wood = journal
        .object_state(old.anchor)
        .ok_or("Missing rail wood state")?
        .wood;
    let switched = Piece::from_facing(old.anchor, old.material, old.encoded_facing() ^ 4)
        .ok_or("Only rail switches can change route")?;
    let mut result = break_layout(chunks, journal, cell)?;
    for edit in &mut result.edits {
        edit.material = old.material;
    }
    for (position, state) in &mut result.states {
        *state = Some(object_state_with_wood(switched, *position, wood));
    }
    Ok(result)
}

pub(super) fn proposed_piece(plan: &Layout) -> Option<Piece> {
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
        piece_from_state(*cell, material, state)
    })
}

/// Require a complete top face; lower slabs, furniture and other rails cannot float a module.
pub(super) fn supports_floor(journal: &EditJournal, cell: Cell, material: u16) -> bool {
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
    if terra_core::construction::is_stair(material) {
        return state.is_some_and(|state| (4..8).contains(&state.facing));
    }
    terra_core::beds::supports_bed_floor(material)
}

fn supported(piece: Piece, chunks: &ChunkStore, journal: &EditJournal) -> bool {
    piece.supports().iter().any(|point| {
        let support =
            std::array::from_fn(|axis| piece.anchor[axis] + i64::from(point.support[axis]));
        chunks
            .block_at(support)
            .is_some_and(|material| supports_floor(journal, support, material))
    })
}

/// Check support and real occupied prop space using the same shape as collision.
/// A wheel can share a voxel with its supporting rail without intersecting it.
/// Shrink only the contact tolerance, never reserve the entire wheel voxel.
fn rail_intersects_vehicle(journal: &EditJournal, piece: Piece, cell: Cell) -> bool {
    let offset = std::array::from_fn(|axis| (cell[axis] - piece.anchor[axis]) as i32);
    let boxes = piece.cell_boxes(offset);
    journal.gameplay().trains.vehicles.values().any(|car| {
        let bounds = car.model_bounds();
        boxes.iter().any(|rail| {
            let low = std::array::from_fn(|axis| cell[axis] as f64 + rail.minimum[axis] + 1e-6);
            let high = std::array::from_fn(|axis| cell[axis] as f64 + rail.maximum[axis] - 1e-6);
            if !(0..3).all(|axis| {
                low[axis] < high[axis]
                    && bounds[0][axis] < high[axis]
                    && bounds[1][axis] > low[axis]
            }) {
                return false;
            }
            car.parts.iter().enumerate().any(|(index, _)| {
                car.transformed_shapes(index)
                    .iter()
                    .any(|shape| shape.intersects(low, high))
            })
        })
    })
}

pub fn environment(
    plan: &Layout,
    chunks: &ChunkStore,
    journal: &EditJournal,
) -> Result<(), String> {
    environment_for(plan, chunks, journal, None)
}

fn environment_for(
    plan: &Layout,
    chunks: &ChunkStore,
    journal: &EditJournal,
    replaced: Option<Piece>,
) -> Result<(), String> {
    if replaced.is_none() && journal.rail_module_count() >= terra_core::rails::MAX_PIECES {
        return Err("Rail network has reached its module limit".into());
    }
    let piece = proposed_piece(plan).ok_or("Missing canonical rail placement state")?;
    if !supported(piece, chunks, journal) {
        return Err(
            "At least one rail sleeper needs a loaded, full-height supporting block".into(),
        );
    }
    let positions: BTreeSet<_> = plan.edits.iter().map(|edit| edit.position).collect();
    let mut expected: BTreeSet<_> = cells(piece).into_iter().collect();
    if let Some(old) = replaced {
        expected.extend(cells(old));
    }
    if positions != expected
        || plan.edits.len() != positions.len()
        || plan.states.len() != positions.len()
        || !plan.belts.is_empty()
    {
        return Err("Rail layout is incomplete".into());
    }
    let wood = plan
        .states
        .iter()
        .find(|(cell, _)| *cell == piece.anchor)
        .and_then(|(_, state)| *state)
        .ok_or("Missing rail wood state")?
        .wood;
    for edit in &plan.edits {
        if edit.material != 0 && rail_intersects_vehicle(journal, piece, edit.position) {
            return Err("An assembled vehicle intersects the physical rail".into());
        }
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
        let state = plan
            .states
            .iter()
            .find(|(cell, _)| *cell == edit.position)
            .and_then(|(_, state)| *state);
        let offset = std::array::from_fn(|axis| (edit.position[axis] - piece.anchor[axis]) as i32);
        let occupied = piece.contains_offset(offset);
        let old_cell = replaced.is_some_and(|old| owner_at(journal, edit.position) == Some(old));
        if !old_cell && terra_core::rails::is_rail(edit.expected_material) {
            return Err("Rail curve overlaps an existing module; connect its free end or use a switch for a side branch".into());
        }
        if chunks.block_at(edit.position) != Some(edit.expected_material)
            || (old_cell && replaced.is_none_or(|old| edit.expected_material != old.material))
            || (!old_cell
                && (!matches!(edit.expected_material, 0 | 14 | 27)
                    || journal.object_state(edit.position).is_some()))
            || edit.material != if occupied { piece.material } else { 0 }
            || state != occupied.then(|| object_state_with_wood(piece, edit.position, wood))
        {
            return Err("Rail footprint is occupied or changed".into());
        }
    }
    super::clearance(plan, chunks, journal)?;
    Ok(())
}

/// Replace only a complete, plain four-block straight with its own smooth
/// one-block height transition, optionally continuing at most four blocks
/// forward to its actual raised floor. The opposite physical port stays unchanged.
pub(super) fn ramp_replacement(
    chunks: &ChunkStore,
    journal: &EditJournal,
    old: Piece,
    near: u8,
    end: terra_core::rails::Port,
    wood: terra_core::WoodSpecies,
) -> Result<Layout, String> {
    use terra_core::rails::{CurveSpec, Port};
    if old.material != terra_core::rails::STRAIGHT
        || old.curve.is_some()
        || near > 1
        || !complete(journal, old)
    {
        return Err("Only a complete plain straight can become a terrain ramp".into());
    }
    if journal
        .object_state(old.anchor)
        .is_none_or(|state| state.wood != wood)
    {
        return Err("A terrain ramp must use the same wood as the replaced straight".into());
    }
    let ports = old.ports();
    let near_port = ports[usize::from(near)];
    let far_port = ports[usize::from(1 - near)];
    let displacement: [i64; 3] =
        std::array::from_fn(|axis| end.position.0[axis] - near_port.position.0[axis]);
    let forward = near_port.tangent.map(i64::from);
    let advance = displacement[0] * forward[0] + displacement[2] * forward[2];
    if port_connected(journal, old, near_port)
        || !(0..=4 * 16).contains(&advance)
        || advance % 16 != 0
        || displacement[0] != advance * forward[0]
        || displacement[2] != advance * forward[2]
        || displacement[1].unsigned_abs() != 16
        || end.tangent != near_port.tangent.map(|value| -value)
    {
        return Err(
            "A terrain ramp must reach a raised floor within four blocks ahead of this straight"
                .into(),
        );
    }
    let start = Port {
        tangent: far_port.tangent.map(|value| -value),
        ..far_port
    };
    let point = start.position.position();
    let anchor = std::array::from_fn(|axis| {
        (point[axis]
            + if axis == 1 {
                0.0
            } else {
                f64::from(start.tangent[axis]).signum() * 1e-6
            })
        .floor() as i64
    });
    let spec =
        CurveSpec::between_for_build(start, end, anchor).map_err(|error| error.to_string())?;
    let next = Piece::new_curve(anchor, spec).ok_or("Invalid terrain ramp")?;
    let mut layout = candidate_piece(chunks, next, wood)?;
    let next_cells: BTreeSet<_> = cells(next).into_iter().collect();
    for cell in cells(old) {
        if !next_cells.contains(&cell) {
            layout.edits.push(BlockEditRequest {
                position: cell,
                expected_material: old.material,
                material: 0,
            });
            layout.states.push((cell, None));
        }
    }
    Ok(layout)
}

pub(super) fn replacement_environment(
    plan: &Layout,
    chunks: &ChunkStore,
    journal: &EditJournal,
    old: Piece,
) -> Result<(), String> {
    let next = proposed_piece(plan).ok_or("Missing terrain ramp owner")?;
    let ports = next.ports();
    let end = terra_core::rails::Port {
        tangent: ports[1].tangent.map(|value| -value),
        ..ports[1]
    };
    let old_ports = old.ports();
    let near = old_ports
        .iter()
        .find(|port| port.tangent == end.tangent.map(|value| -value))
        .ok_or("Terrain ramp changed its original endpoints")?
        .index;
    let wood = plan
        .states
        .iter()
        .find(|(cell, _)| *cell == next.anchor)
        .and_then(|(_, state)| *state)
        .ok_or("Missing ramp wood")?
        .wood;
    let canonical = ramp_replacement(chunks, journal, old, near, end, wood)?;
    let edits = |layout: &Layout| {
        layout
            .edits
            .iter()
            .map(|edit| (edit.position, (edit.expected_material, edit.material)))
            .collect::<std::collections::BTreeMap<_, _>>()
    };
    let states = |layout: &Layout| {
        layout
            .states
            .iter()
            .copied()
            .collect::<std::collections::BTreeMap<_, _>>()
    };
    if edits(plan) != edits(&canonical) || states(plan) != states(&canonical) {
        return Err("Terrain ramp is not the canonical complete replacement".into());
    }
    environment_for(plan, chunks, journal, Some(old))
}

/// Existing rails retain ownership when their bedding changes. This compatibility
/// hook does not bypass the caller's ordinary terrain authorization or collisions.
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

/// Support is a placement requirement, not a restriction on later terrain edits.
pub(crate) fn support_state_allowed(
    _journal: &EditJournal,
    _position: Cell,
    _new_material: u16,
    _state: Option<crate::edits::BlockObjectState>,
) -> bool {
    true
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
    let first = plan
        .edits
        .iter()
        .find(|edit| terra_core::rails::is_rail(edit.expected_material))
        .ok_or("Missing replaced rail")?;
    let current = owner_at(journal, first.position).ok_or("Missing rail owner")?;
    if current.material == terra_core::rails::STRAIGHT
        && current.curve.is_none()
        && proposed_piece(plan).is_some_and(|next| next.curve.is_some())
    {
        return replacement_environment(plan, chunks, journal, current);
    }
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
    let wood = journal
        .object_state(current.anchor)
        .ok_or("Missing rail wood state")?
        .wood;
    if next.anchor != current.anchor
        || next.material != current.material
        || next.curve != current.curve
        || if current.material == terra_core::rails::TURNTABLE {
            next.facing != (current.facing + 1) % 4 && next.facing != (current.facing + 3) % 4
        } else {
            next.encoded_facing() != (current.encoded_facing() ^ 4)
        }
        || plan.edits.iter().any(|edit| edit.material != next.material)
        || plan
            .states
            .iter()
            .any(|(cell, state)| *state != Some(object_state_with_wood(next, *cell, wood)))
    {
        return Err("Invalid rail switch transaction".into());
    }
    Ok(())
}
