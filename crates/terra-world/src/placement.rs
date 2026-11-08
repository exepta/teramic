//! Shared placement layout and authored collision for prediction and authority.
use crate::{
    edits::{BlockObjectState, EditJournal},
    streaming::{BlockEditRequest, ChunkStore},
};
use bevy_math::{Quat, Vec3};
use terra_core::{block_properties::BlockPropertiesCatalog, block_shapes::ShapeBox};

#[derive(Clone, Debug)]
pub struct Layout {
    pub edits: Vec<BlockEditRequest>,
    pub states: Vec<([i64; 3], Option<BlockObjectState>)>,
    pub belts: Vec<([i64; 3], Option<crate::edits::BeltOwner>)>,
}
pub fn furniture(material: u16) -> bool {
    matches!(material, 28..=29 | 59..=73 | 99..=100 | 131 | 141..=172)
        || crate::objects::is_opening(material)
        || terra_core::construction::is_shape(material)
        || terra_core::castle::is_castle(material)
        || terra_core::textiles::is_carpet(material)
        || terra_core::rails::is_rail(material)
        || material == terra_core::rails::SWITCH_STAND
        || terra_core::train_parts::is_train(material)
}

/// Creative copying retains authoritative appearance metadata, not only the raw block ID.
pub fn picked_item(
    chunks: &ChunkStore,
    journal: &EditJournal,
    position: [i64; 3],
) -> Option<terra_core::ItemKind> {
    use terra_core::{ItemKind, beds::DyeColor};
    let material = chunks.block_at(position)?;
    if material == 0 {
        return None;
    }
    if terra_core::castle::retired_item(material) {
        return Some(ItemKind::Block(terra_core::castle::item_material(material)));
    }
    let state = journal.object_state(position).unwrap_or_default();
    if terra_core::rails::is_rail(material) {
        return Some(terra_core::ItemKind::for_wood(
            if matches!(material, 202 | 203) {
                material
            } else {
                terra_core::rails::STRAIGHT
            },
            state.wood,
        ));
    }
    if material == terra_core::kinetic::CONVEYOR
        && let Some(span) = crate::conveyors::span_at(journal, position)
    {
        return Some(crate::conveyors::item(span, journal));
    }
    let item = ItemKind::for_wood(material, state.wood);
    let color = if terra_core::beds::is_bed(material) {
        let anchor = if state.secondary {
            state.peer.unwrap_or(position)
        } else {
            position
        };
        Some(
            journal
                .tint_at(anchor)
                .or_else(|| journal.tint_at(position))
                .map(DyeColor::Rgb)
                .unwrap_or_else(|| terra_core::beds::bed_color(material, state.facing)),
        )
    } else {
        journal.tint_at(position).map(DyeColor::Rgb).or_else(|| {
            (material == terra_core::kinetic::CONVEYOR)
                .then(|| terra_core::kinetic::belt_color(state.facing))
                .flatten()
        })
    };
    Some(
        color
            .and_then(|color| terra_core::tints::recolored(item, color))
            .unwrap_or(item),
    )
}

pub fn picked_item_for_ray(
    chunks: &ChunkStore,
    journal: &EditJournal,
    position: [i64; 3],
    ray: ([f64; 3], [f64; 3]),
) -> Option<terra_core::ItemKind> {
    if let Some(half) = crate::slabs::targeted_half(chunks, journal, position, ray) {
        let item = terra_core::ItemKind::Block(half.removed);
        return Some(
            journal
                .tint_at(position)
                .and_then(|rgb| {
                    terra_core::tints::recolored(item, terra_core::beds::DyeColor::Rgb(rgb))
                })
                .unwrap_or(item),
        );
    }
    picked_item(chunks, journal, position)
}

#[cfg(test)]
#[path = "placement_pick_tests.rs"]
mod pick_tests;

/// A port can use a real machine's mounted drive as its outward attachment face.
/// This is one validated crank hop, never a search through arbitrary kinetic parts.
pub fn logistics_inventory_at(
    chunks: &ChunkStore,
    journal: &EditJournal,
    port: [i64; 3],
    peer: [i64; 3],
) -> Option<[i64; 3]> {
    let material = chunks.block_at(peer)?;
    if terra_core::logistics::inventory(material) {
        return Some(peer);
    }
    if material != 100 {
        return None;
    }
    let state = journal.object_state(peer)?;
    if port != terra_core::kinetic::offset(peer, terra_core::kinetic::axis(state.facing)) {
        return None;
    }
    let anchor = journal.crank_anchor(peer)?;
    match chunks.block_at(anchor)? {
        99 if crate::crushers::loaded(anchor, chunks, journal) => Some(anchor),
        156 if journal.valid_mixer(anchor) => Some(anchor),
        _ => None,
    }
}
fn offset(p: [i64; 3], d: [i64; 3], sign: i64) -> [i64; 3] {
    std::array::from_fn(|i| p[i] + d[i] * sign)
}

/// One orientation rule for the placement preview and server transaction:
/// clicked mechanical ports, other usable neighbors, support face, then view.
pub fn kinetic_facing(
    chunks: &ChunkStore,
    journal: &EditJournal,
    position: [i64; 3],
    material: u16,
    clicked: Option<terra_voxel::blocks::BlockHit>,
    look: [f32; 2],
) -> u8 {
    use terra_core::kinetic::{self, Node};
    let mut preferred = BlockObjectState::from_yaw(look[0]).facing;
    if !matches!(
        material,
        kinetic::WATER_WHEEL | kinetic::CONVEYOR | terra_core::plumbing::PUMP
    ) && !kinetic::is_gearbox(material)
        && look[1].abs() > 0.75
    {
        preferred = if look[1] > 0.0 { 4 } else { 5 };
    }
    let face = clicked.and_then(|hit| kinetic::facing(hit.normal.map(i64::from)));
    let mut best = (0u8, 0u8, 0u8, 0u8, 0u8);
    let mut chosen = preferred;
    for facing in 0..if matches!(
        material,
        kinetic::WATER_WHEEL | kinetic::CONVEYOR | terra_core::plumbing::PUMP
    ) || kinetic::is_gearbox(material)
    {
        4
    } else {
        6
    } {
        let node = Node {
            position,
            material,
            facing,
            secondary: false,
            source_rpm: 0.0,
            source_capacity: 0,
            belt_load: false,
            links: [None; 2],
        };
        let mut connections = 0u8;
        let mut clicked_port = 0u8;
        for offset in kinetic::NEIGHBORS {
            let cell = kinetic::offset(position, offset);
            let Some(other_material) = chunks.block_at(cell).filter(|m| kinetic::is_part(*m))
            else {
                continue;
            };
            let Some(state) = journal
                .object_state(cell)
                .filter(|s| s.valid_for(cell, other_material))
            else {
                continue;
            };
            let other = Node {
                position: cell,
                material: other_material,
                facing: state.facing,
                secondary: state.secondary,
                source_rpm: 0.0,
                source_capacity: 0,
                belt_load: false,
                links: if other_material == kinetic::CONVEYOR {
                    crate::conveyors::segment_at(journal, cell)
                        .map_or([None; 2], |segment| segment.span.links(cell))
                } else {
                    [None; 2]
                },
            };
            if kinetic::connection(node, other).is_some() {
                connections += 1;
                clicked_port |= u8::from(clicked.is_some_and(|hit| {
                    hit.position == cell
                        || other_material == terra_core::crusher::MATERIAL
                            && journal
                                .crusher_primary(hit.position)
                                .is_some_and(|primary| {
                                    journal.crusher_primary(cell) == Some(primary)
                                })
                }));
            }
        }
        let score = (
            clicked_port,
            u8::from(face == Some(facing)),
            u8::from(face.is_some_and(|f| {
                kinetic::axis(f).map(i64::abs) == kinetic::axis(facing).map(i64::abs)
            })),
            connections,
            u8::from(facing == preferred),
        );
        if score > best {
            best = score;
            chosen = facing;
        }
    }
    chosen
}

pub fn layout(
    chunks: &ChunkStore,
    journal: &EditJournal,
    position: [i64; 3],
    expected: u16,
    material: u16,
    look: [f32; 2],
    crank_owner: Option<[i64; 3]>,
) -> Result<Layout, String> {
    if position.iter().any(|p| p.unsigned_abs() > 990_000) || look.iter().any(|v| !v.is_finite()) {
        return Err("Invalid placement coordinates".into());
    }
    if terra_core::rails::is_rail(material) {
        return crate::rails::plan(
            chunks,
            journal,
            position,
            material,
            BlockObjectState::from_yaw(look[0]).facing,
        );
    }
    let mut result = Layout {
        edits: vec![BlockEditRequest {
            position,
            expected_material: expected,
            material,
        }],
        states: Vec::new(),
        belts: Vec::new(),
    };
    if matches!((expected, material), (161, 162) | (162, 161)) {
        let mut state = journal
            .object_state(position)
            .ok_or("Missing hearth placement state")?;
        if journal.has_attached_campfire(position) || !state.valid_for(position, expected) {
            return Err("Hearth is already attached or invalid".into());
        }
        state.facing = (state.facing % 4) | terra_core::brewing::ATTACHED_FIRE;
        result.edits[0].material = terra_core::brewing::MATERIAL;
        result.states.push((position, Some(state)));
        return Ok(result);
    }
    if material == terra_core::flower_pot::MATERIAL
        || material == terra_core::rails::SWITCH_STAND
        || terra_core::textiles::is_carpet(material)
    {
        let support = chunks
            .block_at([position[0], position[1] - 1, position[2]])
            .ok_or("Decorative support is not loaded")?;
        if !terra_core::beds::supports_bed_floor(support) {
            return Err("A full supporting surface is required".into());
        }
    }
    if !furniture(material) || terra_core::textiles::is_carpet(material) {
        return Ok(result);
    }
    let mut state = BlockObjectState::from_yaw(look[0]);
    if material == terra_core::infusion::HOLD_BLOCK {
        state.facing = crate::conveyors::infusion_facing(journal, position, state.facing).0;
    } else if material == terra_core::infusion::FLUID_INFUSER {
        let below = terra_core::infusion::hold_below(position);
        state.facing =
            crate::conveyors::infusion_geometry(below, journal).map_or(state.facing, |g| g.facing);
    }
    if terra_core::logistics::is_port(material) {
        result.states.push((position, Some(state)));
        return Ok(result);
    }
    if terra_core::kinetic::uses_adaptive_facing(material) {
        state.facing = kinetic_facing(chunks, journal, position, material, None, look);
    }
    if material == terra_core::flower_pot::MATERIAL || material == terra_core::plumbing::BASIN {
        state.facing = 0;
    }
    if material == 100 {
        let rotary = crank_owner
            .filter(|anchor| {
                journal
                    .block(*anchor)
                    .is_some_and(terra_core::kinetic::is_crank_mount)
            })
            .and_then(|anchor| {
                let direction = std::array::from_fn(|i| position[i] - anchor[i]);
                let facing = terra_core::kinetic::facing(direction)?;
                (journal.rotary_crank_mount(anchor, facing) == Some(position)
                    && chunks.block_at(anchor) == journal.block(anchor))
                .then_some((anchor, facing))
            });
        if let Some((anchor, facing)) = rotary {
            result.states.push((
                position,
                Some(BlockObjectState {
                    facing,
                    peer: Some(anchor),
                    ..Default::default()
                }),
            ));
            return Ok(result);
        }
        let anchor = crank_owner
            .or_else(|| {
                (0..4).find_map(|facing| {
                    let state = BlockObjectState {
                        facing,
                        ..Default::default()
                    };
                    [1, 2].into_iter().find_map(|distance| {
                        let anchor = offset(position, state.right(), -distance);
                        (journal.crusher_crank_position(anchor) == Some(position)
                            && journal
                                .object_state(anchor)
                                .is_some_and(|s| s.facing == facing))
                        .then_some(anchor)
                    })
                })
            })
            .ok_or("Crank needs a crusher drive gear")?;
        let machine = journal
            .object_state(anchor)
            .ok_or("Missing crusher drive")?;
        if journal.crusher_crank_position(anchor) != Some(position)
            || chunks.block_at(anchor) != Some(99)
            || machine.peer.is_some_and(|p| chunks.block_at(p) != Some(99))
        {
            return Err("Crank attachment is not loaded".into());
        }
        state = BlockObjectState {
            peer: Some(anchor),
            secondary: false,
            ..machine
        };
    }
    if terra_core::beds::is_bed(material) {
        state.facing = (state.facing + 2) % 4;
    }
    if matches!(material, 28 | 59 | 71..=73) || crate::objects::is_door(material) {
        let direction = if terra_core::beds::is_bed(material) {
            [[0, 0, 1], [1, 0, 0], [0, 0, -1], [-1, 0, 0]][usize::from(state.facing % 4)]
        } else if material == 28 {
            state.right()
        } else {
            [0, 1, 0]
        };
        let peer = offset(position, direction, 1);
        let expected_peer = if chunks.block_at(peer) == Some(98) {
            98
        } else {
            0
        };
        state.peer = Some(peer);
        result.edits.push(BlockEditRequest {
            position: peer,
            expected_material: expected_peer,
            material,
        });
        result.states.push((
            peer,
            Some(BlockObjectState {
                peer: Some(position),
                secondary: true,
                ..state
            }),
        ));
    } else if material == 29 {
        let mut neighbors = Vec::new();
        for direction in [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1]] {
            let peer = offset(position, direction, 1);
            if chunks.block_at(peer) == Some(29) {
                let other = journal.object_state(peer).unwrap_or_default();
                let sign = direction
                    .into_iter()
                    .zip(other.right())
                    .map(|(a, b)| a * b)
                    .sum::<i64>();
                if sign != 0 {
                    if other.peer.is_some() {
                        return Err("A double chest cannot merge again".into());
                    }
                    neighbors.push((peer, sign, other));
                }
            }
        }
        if neighbors.len() > 1 {
            return Err("Chest would create a triple merge".into());
        }
        if let Some((peer, sign, other)) = neighbors.pop() {
            state.facing = other.facing;
            state.peer = Some(peer);
            state.secondary = sign < 0;
            result.states.push((
                peer,
                Some(BlockObjectState {
                    peer: Some(position),
                    secondary: sign > 0,
                    ..other
                }),
            ));
        }
    }
    if material == 60 {
        state.facing = chimney(position, look, chunks, journal, &mut result.states)?;
    }
    result.states.push((position, Some(state)));
    Ok(result)
}

fn chimney(
    position: [i64; 3],
    look: [f32; 2],
    chunks: &ChunkStore,
    journal: &EditJournal,
    states: &mut Vec<([i64; 3], Option<BlockObjectState>)>,
) -> Result<u8, String> {
    use crate::objects::{DIRECTIONS, chimney_ports, chimney_pose};
    let connects = |cell: [i64; 3], direction: [i64; 3]| {
        let other = offset(cell, direction, 1);
        let id = chunks.block_at(other)?;
        if id == 59
            && direction == [0, -1, 0]
            && journal.object_state(other).is_some_and(|s| s.secondary)
        {
            return Some(());
        }
        chimney_ports(id, journal.object_state(other).unwrap_or_default().facing)?
            .contains(&direction.map(|v| -v))
            .then_some(())
    };
    let mut incoming: Vec<_> = DIRECTIONS
        .into_iter()
        .filter(|d| connects(position, *d).is_some())
        .collect();
    if incoming.is_empty() {
        for direction in DIRECTIONS {
            let neighbor = offset(position, direction, 1);
            if chunks.block_at(neighbor) != Some(60) {
                continue;
            }
            let old = journal.object_state(neighbor).unwrap_or_default();
            let Some(ports) = chimney_ports(60, old.facing) else {
                continue;
            };
            let linked: Vec<_> = ports
                .into_iter()
                .filter(|d| connects(neighbor, *d).is_some())
                .collect();
            if let [upstream] = linked.as_slice()
                && let Some(pose) = chimney_pose([*upstream, direction.map(|v| -v)])
            {
                states.push((
                    neighbor,
                    Some(BlockObjectState {
                        facing: pose,
                        ..old
                    }),
                ));
                incoming.push(direction);
                break;
            }
        }
    }
    if incoming.len() > 2 {
        return Err("Chimney cannot form a three-way junction".into());
    }
    let ray = Quat::from_euler(bevy_math::EulerRot::YXZ, look[0], look[1], 0.0) * Vec3::NEG_Z;
    let preferred = if ray.y.abs() > 0.65 {
        [0, if ray.y > 0.0 { 1 } else { -1 }, 0]
    } else if ray.x.abs() > ray.z.abs() {
        [if ray.x > 0.0 { 1 } else { -1 }, 0, 0]
    } else {
        [0, 0, if ray.z > 0.0 { 1 } else { -1 }]
    };
    let ports = match incoming.as_slice() {
        [a, b] => [*a, *b],
        [a] => [
            *a,
            if preferred == *a {
                a.map(|v| -v)
            } else {
                preferred
            },
        ],
        _ => [preferred.map(|v| -v), preferred],
    };
    let pose = chimney_pose(ports).ok_or("Invalid chimney connection")?;
    Ok(pose
        + if (0.25..0.85).contains(&look[1].abs()) && ports[0] != ports[1].map(|v| -v) {
            32
        } else {
            0
        })
}

pub fn environment(
    plan: &Layout,
    chunks: &ChunkStore,
    journal: &EditJournal,
    bounded: bool,
) -> Result<(), String> {
    if plan
        .edits
        .iter()
        .any(|edit| edit.material != 0 && crate::trains::reserved(journal, edit.position))
    {
        return Err("An assembled vehicle occupies this cell".into());
    }
    if plan
        .edits
        .iter()
        .any(|edit| terra_core::rails::is_rail(edit.material))
    {
        if bounded
            && plan
                .edits
                .iter()
                .any(|edit| !(-128..=356).contains(&edit.position[1]))
        {
            return Err("Rail module exceeds bounded world height".into());
        }
        return crate::rails::environment(plan, chunks, journal);
    }
    let belt = crate::conveyors::proposed_span(plan).filter(|span| span.linked);
    if let Some(span) = belt {
        crate::conveyors::clearance(span, chunks, journal)?;
        if bounded
            && crate::conveyors::boxes(span)
                .iter()
                .any(|bounds| bounds.minimum[1] < -128.0 || bounds.maximum[1] > 357.0)
        {
            return Err("Conveyor exceeds world height bounds".into());
        }
    }
    for edit in &plan.edits {
        let state = plan
            .states
            .iter()
            .find(|(cell, _)| *cell == edit.position)
            .map_or_else(|| journal.object_state(edit.position), |(_, state)| *state);
        if !crate::rails::placement::support_state_allowed(
            journal,
            edit.position,
            edit.material,
            state,
        ) {
            return Err("Remove the supported rail module before changing its floor".into());
        }
        if bounded && !(-128..=356).contains(&edit.position[1]) {
            return Err("Object exceeds world height bounds".into());
        }
        if chunks.block_at(edit.position) != Some(edit.expected_material) {
            return Err("Object cell unavailable".into());
        }
        if edit.material == 40 {
            if crate::torches::wet_placement(chunks, journal, edit.position) {
                return Err("Torches cannot be placed in water".into());
            }
            let facing = plan
                .states
                .iter()
                .find(|(position, _)| *position == edit.position)
                .and_then(|(_, state)| *state)
                .unwrap_or_default()
                .facing;
            if !crate::torches::supported(
                chunks,
                &BlockPropertiesCatalog::default(),
                edit.position,
                facing,
            ) {
                return Err("Torch needs a full supporting floor or wall".into());
            }
        }
        if terra_core::lantern::is_part(edit.material) {
            if edit.material == terra_core::lantern::LANTERN
                && crate::torches::wet_placement(chunks, journal, edit.position)
            {
                return Err("Lanterns cannot be placed in water".into());
            }
            let facing = plan
                .states
                .iter()
                .find(|(position, _)| *position == edit.position)
                .and_then(|(_, state)| *state)
                .unwrap_or_default()
                .facing;
            if !crate::lanterns::supported(
                chunks,
                &BlockPropertiesCatalog::default(),
                edit.position,
                edit.material,
                facing,
            ) {
                return Err(
                    "Lantern or chain needs a solid mount or anchored hanging chain".into(),
                );
            }
        }
        if edit.material == terra_core::kinetic::WATER_WHEEL
            && plan
                .states
                .iter()
                .find(|(p, _)| *p == edit.position)
                .and_then(|(_, state)| *state)
                .is_some_and(|state| state.facing >= 4)
        {
            return Err("Water wheels require a horizontal axle".into());
        }
        if edit.material != 0
            && !belt.is_some_and(|span| {
                edit.position == span.start
                    || edit.position == span.end
                    || span.cells().any(|p| p == edit.position)
            })
            && (terra_core::kinetic::is_extended(edit.material) || journal.has_extended_kinetics())
        {
            kinetic_clearance(plan, chunks, journal, edit.position, edit.material, bounded)?;
        }
        if edit.material == 0
            || terra_core::construction::is_shape(edit.material)
            || terra_core::castle::is_castle(edit.material)
        {
            continue;
        }
        if terra_core::logistics::is_port(edit.material) {
            if terra_core::logistics::is_router(edit.material) {
                let below = [edit.position[0], edit.position[1] - 1, edit.position[2]];
                if !crate::conveyors::segment_at(journal, below)
                    .is_some_and(|segment| segment.span.loaded(chunks, journal))
                {
                    return Err("A belt merger must be mounted above a loaded conveyor".into());
                }
                continue;
            }
            let peer = plan
                .states
                .iter()
                .find(|(p, _)| *p == edit.position)
                .and_then(|(_, state)| *state)
                .and_then(|state| state.peer);
            let Some(peer) = peer else {
                continue;
            };
            if logistics_inventory_at(chunks, journal, edit.position, peer).is_none() {
                return Err("Logistics port attachment is no longer valid".into());
            }
            continue;
        }
        if edit.material == 99
            && chunks.block_at([edit.position[0], edit.position[1] + 1, edit.position[2]])
                != Some(0)
        {
            return Err("Crusher needs free space above its hoppers".into());
        }
        let below = [edit.position[0], edit.position[1] - 1, edit.position[2]];
        if edit.material == terra_core::infusion::FLUID_INFUSER {
            let hold = terra_core::infusion::hold_below(edit.position);
            let gap = terra_core::infusion::gap_above(hold);
            let proposed = |p| {
                plan.edits
                    .iter()
                    .find(|e| e.position == p)
                    .map(|e| e.material)
                    .or_else(|| chunks.block_at(p))
            };
            if proposed(hold) != Some(terra_core::infusion::HOLD_BLOCK) || proposed(gap) != Some(0)
            {
                return Err("Fluid Infuser: ein freier Luftblock zwischen Hold Block und Geraet erforderlich".into());
            }
        }
        if terra_core::beds::is_bed(edit.material)
            && !chunks.block_at(below).is_some_and(|m| {
                terra_core::beds::supports_bed_floor(m)
                    || terra_core::construction::is_slab(m)
                        && journal.object_state(below).is_some_and(|s| {
                            s.facing == 4 || terra_core::construction::full_slab(s.facing)
                        })
            })
        {
            return Err("Bed requires a full-height supporting floor".into());
        }
        // Ordinary construction, furniture and vehicle parts can extend from
        // any clicked face and remain suspended. Only objects with an explicit
        // mounting rule above require support; a generic floor check prevents
        // connecting the frames between a train's two bogies.
    }
    Ok(())
}

pub(crate) fn kinetic_clearance(
    plan: &Layout,
    chunks: &ChunkStore,
    journal: &EditJournal,
    position: [i64; 3],
    material: u16,
    bounded: bool,
) -> Result<(), String> {
    use terra_core::kinetic::{self, Node};
    let state = plan
        .states
        .iter()
        .find(|(p, _)| *p == position)
        .and_then(|(_, s)| *s)
        .unwrap_or_default();
    let chest_peer = (material == 29)
        .then_some(state.peer)
        .flatten()
        .and_then(|peer| {
            let other = plan.states.iter().find(|(p, _)| *p == peer)?.1?;
            (chunks.block_at(peer) == Some(29)
                && other.peer == Some(position)
                && other.facing == state.facing
                && other.secondary != state.secondary)
                .then_some((peer, other))
        });
    let (shape_position, shape_state) = if state.secondary {
        let planned_primary = state.peer.and_then(|peer| {
            let other = plan.states.iter().find(|(p, _)| *p == peer)?.1?;
            let planned = plan.edits.iter().find(|edit| edit.position == peer)?;
            (planned.material == material
                && !other.secondary
                && other.peer == Some(position)
                && other.facing == state.facing)
                .then_some((peer, other))
        });
        chest_peer.or(planned_primary).unwrap_or((position, state))
    } else {
        (position, state)
    };
    let node = Node {
        position,
        material,
        facing: state.facing,
        secondary: false,
        source_rpm: 0.0,
        source_capacity: 0,
        belt_load: false,
        links: [None; 2],
    };
    let properties = BlockPropertiesCatalog::default();
    let mut shapes = Vec::new();
    if !crate::interaction_shapes::visit_proposed_boxes(
        chunks,
        journal,
        &properties,
        shape_position,
        material,
        shape_state,
        |b| shapes.push(b),
    ) && terra_voxel::MaterialId::from_raw(material)
        .is_some_and(terra_voxel::MaterialId::is_solid)
    {
        shapes.push(ShapeBox {
            minimum: position.map(|v| v as f64),
            maximum: position.map(|v| v as f64 + 1.0),
            hinged: false,
        });
    }
    let joined_windows = if crate::objects::is_window(material) {
        crate::objects::windows::group(position, material, state.facing, |p| {
            let material = plan
                .edits
                .iter()
                .find(|edit| edit.position == p)
                .map(|edit| edit.material)
                .or_else(|| chunks.block_at(p))?;
            let facing = plan
                .states
                .iter()
                .find(|(cell, _)| *cell == p)
                .and_then(|(_, state)| *state)
                .or_else(|| journal.object_state(p))
                .unwrap_or_default()
                .facing;
            Some((material, facing))
        })
        .cells()
        .collect::<std::collections::BTreeSet<_>>()
    } else {
        std::collections::BTreeSet::new()
    };
    let mut owners = std::collections::BTreeSet::new();
    for shape in &shapes {
        if bounded && (shape.minimum[1] < -128.0 || shape.maximum[1] > 357.0) {
            return Err("Mechanical assembly exceeds world height bounds".into());
        }
        let low = shape.minimum.map(|v| v.floor() as i64);
        let high = shape.maximum.map(|v| (v - 0.000001).floor() as i64);
        for x in low[0]..=high[0] {
            for y in low[1]..=high[1] {
                for z in low[2]..=high[2] {
                    let cell = [x, y, z];
                    if chunks.block_at(cell).is_none() {
                        return Err("Mechanical clearance is not loaded".into());
                    }
                    owners.insert(cell);
                    journal.visit_extended_kinetics_near(cell, |owner| {
                        owners.insert(owner);
                    });
                }
            }
        }
    }
    for owner in owners {
        // A chest merge updates the existing half's pose without replacing its block.
        if plan.edits.iter().any(|edit| edit.position == owner)
            || chest_peer.is_some_and(|(peer, _)| peer == owner)
        {
            continue;
        }
        let Some(other_material) = chunks.block_at(owner) else {
            continue;
        };
        let other = journal.object_state(owner).unwrap_or_default();
        if joined_windows.contains(&owner)
            && other_material == material
            && other.facing == state.facing
        {
            // The prospective group already represents the complete joined window.
            continue;
        }
        if kinetic::is_part(other_material)
            && kinetic::connection(
                node,
                Node {
                    position: owner,
                    material: other_material,
                    facing: other.facing,
                    secondary: other.secondary,
                    source_rpm: 0.0,
                    source_capacity: 0,
                    belt_load: false,
                    links: [None; 2],
                },
            )
            .is_some()
        {
            // Teeth intentionally share contact space at a validated mesh.
            continue;
        }
        let mut blocked = false;
        let overlaps = |bounds: ShapeBox, shape: &ShapeBox| {
            let penetration = bounds.maximum[1] - shape.minimum[1];
            let floor_depth = if kinetic::is_gear(material) {
                f64::from(kinetic::radial_extent(material).unwrap_or(0.5) - 0.5)
            } else if terra_core::logistics::is_port(material) {
                0.375
            } else {
                0.0
            };
            let existing_depth = if kinetic::is_gear(other_material) {
                f64::from(kinetic::radial_extent(other_material).unwrap_or(0.5) - 0.5)
            } else if terra_core::logistics::is_port(other_material) {
                0.375
            } else {
                0.0
            };
            let floor = floor_depth > 0.0
                && owner[1] < position[1]
                && bounds.maximum[1] <= position[1] as f64 + 0.000001
                && penetration > 0.0
                && penetration <= floor_depth + 0.000001
                && (properties.shape(other_material).is_empty()
                    || terra_core::construction::is_slab(other_material));
            let existing_floor = existing_depth > 0.0
                && position[1] < owner[1]
                && shape.maximum[1] <= owner[1] as f64 + 0.000001
                && shape.maximum[1] > bounds.minimum[1]
                && shape.maximum[1] - bounds.minimum[1] <= existing_depth + 0.000001
                && (properties.shape(material).is_empty()
                    || terra_core::construction::is_slab(material));
            // Only a gear rim or the authored port feet may enter its supporting floor.
            bounds.intersects(shape.minimum, shape.maximum) && !floor && !existing_floor
        };
        let modeled =
            crate::interaction_shapes::visit_boxes(chunks, journal, &properties, owner, |bounds| {
                blocked |= shapes.iter().any(|shape| overlaps(bounds, shape));
            });
        if !modeled
            && terra_voxel::MaterialId::from_raw(other_material)
                .is_some_and(terra_voxel::MaterialId::is_solid)
        {
            let bounds = ShapeBox {
                minimum: owner.map(|v| v as f64),
                maximum: owner.map(|v| v as f64 + 1.0),
                hinged: false,
            };
            blocked |= shapes.iter().any(|shape| overlaps(bounds, shape));
        }
        if blocked {
            return Err("Mechanical assembly is obstructed".into());
        }
    }
    Ok(())
}

pub fn boxes(
    plan: &Layout,
    chunks: &ChunkStore,
    journal: &EditJournal,
    properties: &BlockPropertiesCatalog,
) -> Vec<ShapeBox> {
    let mut result = Vec::new();
    let positions: std::collections::BTreeSet<_> = plan
        .edits
        .iter()
        .map(|e| e.position)
        .chain(plan.states.iter().map(|(p, _)| *p))
        .collect();
    for position in positions {
        let material = plan
            .edits
            .iter()
            .find(|e| e.position == position)
            .map(|e| e.material)
            .or_else(|| chunks.block_at(position))
            .unwrap_or(0);
        if material == 0 {
            continue;
        }
        let state = plan
            .states
            .iter()
            .find(|(p, _)| *p == position)
            .and_then(|(_, s)| *s)
            .unwrap_or_default();
        if state.secondary && !terra_core::rails::is_rail(material) {
            continue;
        }
        if crate::objects::is_door(material)
            && let Some(group) = crate::objects::doors::group(position, material, state, |p| {
                Some((
                    plan.edits
                        .iter()
                        .find(|e| e.position == p)
                        .map(|e| e.material)
                        .or_else(|| chunks.block_at(p))?,
                    plan.states
                        .iter()
                        .find(|(cell, _)| *cell == p)
                        .and_then(|(_, state)| *state)
                        .or_else(|| journal.object_state(p))?,
                ))
            })
            && group.width == 2
        {
            result.extend(
                crate::objects::doors::pieces()
                    .into_iter()
                    .filter(|p| p.solid)
                    .map(|p| group.world_box(p)),
            );
            continue;
        }
        if !crate::interaction_shapes::visit_proposed_boxes(
            chunks,
            journal,
            properties,
            position,
            material,
            state,
            |b| result.push(b),
        ) {
            result.push(ShapeBox {
                minimum: position.map(|v| v as f64),
                maximum: position.map(|v| v as f64 + 1.0),
                hinged: false,
            });
        }
    }
    result
}

/// A complete proposed object, before any inventory mutation or world edit.
pub fn propose(
    chunks: &ChunkStore,
    journal: &EditJournal,
    hit: terra_voxel::blocks::BlockHit,
    item: terra_core::ItemKind,
    ray: ([f64; 3], [f64; 3]),
    look: [f32; 2],
) -> Result<Layout, String> {
    let material = item.block_material().ok_or("Item cannot be placed")?;
    if terra_core::rails::is_rail(material) {
        return crate::rails::placement(chunks, journal, hit, material, ray, look);
    }
    let (position, expected) =
        crate::targeting::item_placement_with_state(chunks, journal, hit, material)
            .ok_or("No placement cell")?;
    if expected == terra_core::kinetic::CONVEYOR && matches!(material, 144..=146) {
        return crate::conveyors::insert_shaft_layout(chunks, journal, position, item);
    }
    let crank = (material == 100)
        .then(|| {
            journal.crusher_primary(hit.position).or_else(|| {
                journal
                    .block(hit.position)
                    .filter(|id| terra_core::kinetic::is_crank_mount(*id))
                    .map(|_| hit.position)
            })
        })
        .flatten();
    let mut plan = layout(chunks, journal, position, expected, material, look, crank)?;
    if terra_core::logistics::is_port(material) && hit.normal[1] == 0 {
        for (_, state) in &mut plan.states {
            if let Some(state) = state {
                state.facing = (0..4)
                    .find(|&f| terra_core::kinetic::forward(f) == hit.normal.map(i64::from))
                    .unwrap_or(state.facing);
            }
        }
    }
    if terra_core::logistics::is_port(material)
        && !terra_core::logistics::is_router(material)
        && let Some(inventory) = logistics_inventory_at(chunks, journal, position, hit.position)
        && (0..3)
            .map(|i| (position[i] - hit.position[i]).abs())
            .sum::<i64>()
            == 1
    {
        for (_, state) in &mut plan.states {
            if let Some(state) = state {
                state.peer = Some(hit.position);
                let direction = std::array::from_fn(|i| hit.position[i] - position[i]);
                if direction[1] == 0 {
                    let outward = if terra_core::logistics::extracts(material, 0) {
                        direction.map(|v| -v)
                    } else {
                        direction
                    };
                    state.facing = (0..4)
                        .find(|&f| terra_core::kinetic::forward(f) == outward)
                        .unwrap_or(0);
                }
                state.facing = terra_core::logistics::default_facing(
                    material,
                    state.facing,
                    chunks.block_at(inventory).unwrap_or(0),
                );
            }
        }
    }
    if terra_core::plumbing::is_pipe(material) || material == terra_core::plumbing::VALVE {
        let facing = crate::targeting::placement_facing(chunks, journal, hit, material, ray);
        for (_, state) in &mut plan.states {
            if let Some(state) = state {
                state.facing = facing;
            }
        }
    } else if material == 169 {
        for (_, state) in &mut plan.states {
            if let Some(state) = state {
                state.facing = 0;
            }
        }
    } else if terra_core::kinetic::uses_adaptive_facing(material) {
        let facing = kinetic_facing(chunks, journal, position, material, Some(hit), look);
        for (_, state) in &mut plan.states {
            if let Some(state) = state {
                state.facing = facing;
            }
        }
    } else if terra_core::castle::is_castle(material) {
        let facing = crate::targeting::placement_facing(chunks, journal, hit, material, ray);
        plan.states.retain(|(p, _)| *p != position);
        plan.states.push((
            position,
            Some(BlockObjectState {
                facing,
                ..Default::default()
            }),
        ));
    } else if terra_core::construction::is_shape(material) {
        let facing = if terra_core::construction::is_slab(expected) {
            terra_core::construction::merged_slab(
                expected,
                journal.object_state(position).unwrap_or_default().facing,
                material,
            )
            .ok_or("Slabs cannot merge")?
        } else {
            crate::targeting::placement_facing(chunks, journal, hit, material, ray)
        };
        if terra_core::construction::full_slab(facing)
            && terra_core::construction::is_slab(expected)
        {
            plan.edits[0].material = expected;
        }
        plan.states.retain(|(p, _)| *p != position);
        plan.states.push((
            position,
            Some(BlockObjectState {
                facing,
                ..Default::default()
            }),
        ));
    } else if terra_core::lantern::is_part(material) {
        let facing = terra_core::lantern::facing(material, hit.normal)
            .ok_or("Lantern needs floor, wall or ceiling; chains hang beneath a support")?;
        if terra_core::lantern::support(position, material, facing) != Some(hit.position) {
            return Err("The lamp or chain must attach to the clicked supporting face".into());
        }
        plan.states.retain(|(p, _)| *p != position);
        plan.states.push((
            position,
            Some(BlockObjectState {
                facing,
                ..Default::default()
            }),
        ));
    } else if material == 40 {
        let facing = crate::torches::facing(hit.normal).ok_or("Torch needs floor or wall")?;
        if crate::torches::support(position, facing) != Some(hit.position) {
            return Err("Torch must attach to the clicked supporting face".into());
        }
        plan.states.push((
            position,
            Some(BlockObjectState {
                facing,
                ..Default::default()
            }),
        ));
    }
    for (p, state) in &mut plan.states {
        if let Some(state) = state {
            if let Some(wood) = item.wood_species()
                && (*p == position || material == 28)
            {
                state.wood = wood;
            }
            if let terra_core::ItemKind::DyedBed { color, .. } = item {
                state.facing = terra_core::beds::dyed_facing(state.facing, color);
            }
        }
    }
    Ok(plan)
}

pub fn resolve(
    chunks: &ChunkStore,
    journal: &EditJournal,
    hit: terra_voxel::blocks::BlockHit,
    item: terra_core::ItemKind,
    ray: ([f64; 3], [f64; 3]),
    look: [f32; 2],
    bounded: bool,
) -> Result<Layout, String> {
    let plan = propose(chunks, journal, hit, item, ray, look)?;
    environment(&plan, chunks, journal, bounded)?;
    Ok(plan)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::streaming::LoadedSection;
    use terra_voxel::{BlockSection, SectionPos};

    fn place_kinetic(
        chunks: &mut ChunkStore,
        journal: &mut EditJournal,
        position: [i64; 3],
        material: u16,
        facing: u8,
    ) {
        let (section, local) = terra_voxel::global_to_section(position);
        chunks
            .loaded
            .get_mut(&section)
            .unwrap()
            .data
            .set_block(local, material)
            .unwrap();
        journal.record(position, material).unwrap();
        journal
            .set_object_state(
                position,
                Some(BlockObjectState {
                    facing,
                    ..Default::default()
                }),
            )
            .unwrap();
    }

    fn planned_node(plan: &Layout) -> terra_core::kinetic::Node {
        let edit = plan.edits[0];
        let state = plan
            .states
            .iter()
            .find(|(p, _)| *p == edit.position)
            .unwrap()
            .1
            .unwrap();
        terra_core::kinetic::Node {
            position: edit.position,
            material: edit.material,
            facing: state.facing,
            secondary: state.secondary,
            source_rpm: 0.0,
            source_capacity: 0,
            belt_load: false,
            links: [None; 2],
        }
    }

    #[test]
    fn shaft_end_faces_mount_six_axis_gears_in_the_exact_clicked_cell_without_a_floor() {
        use terra_core::kinetic::{self, Node};
        for facing in 0..6 {
            let (mut chunks, mut journal) = fixture();
            let anchor = [7, 6, 7];
            place_kinetic(&mut chunks, &mut journal, anchor, 146, facing);
            let normal = kinetic::axis(facing).map(|v| v as i32);
            let hit = terra_voxel::blocks::BlockHit {
                position: anchor,
                normal,
                distance: 2.0,
            };
            let plan = resolve(
                &chunks,
                &journal,
                hit,
                terra_core::ItemKind::Block(143),
                ([7.5, 6.5, 11.0], [0.0, 0.0, -1.0]),
                [1.0, 1.3],
                false,
            )
            .unwrap();
            let node = planned_node(&plan);
            assert_eq!(node.position, hit.adjacent());
            assert_ne!(
                chunks.block_at([node.position[0], node.position[1] - 1, node.position[2]]),
                Some(3)
            );
            assert!(
                kinetic::connection(
                    node,
                    Node {
                        position: anchor,
                        material: 146,
                        facing,
                        ..node
                    }
                )
                .is_some(),
                "facing {facing}"
            );
        }
    }

    #[test]
    fn clicked_gears_keep_direct_meshes_and_do_not_guess_a_diagonal_position() {
        use terra_core::kinetic;
        for (anchor_material, placed_material) in [(143, 143), (151, 143), (151, 151)] {
            let (mut chunks, mut journal) = fixture();
            let anchor = [7, 6, 7];
            place_kinetic(&mut chunks, &mut journal, anchor, anchor_material, 0);
            let hit = terra_voxel::blocks::BlockHit {
                position: anchor,
                normal: [0, 1, 0],
                distance: 2.0,
            };
            let plan = propose(
                &chunks,
                &journal,
                hit,
                terra_core::ItemKind::Block(placed_material),
                ([7.5, 10.0, 7.5], [0.0, -1.0, 0.0]),
                [0.0, 1.3],
            )
            .unwrap();
            let node = planned_node(&plan);
            assert_eq!(node.position, hit.adjacent());
            let connection = kinetic::connection(
                node,
                kinetic::Node {
                    position: anchor,
                    material: anchor_material,
                    facing: 0,
                    ..node
                },
            );
            if anchor_material == 143 {
                assert_eq!(connection.unwrap().speed_shift, 0);
                environment(&plan, &chunks, &journal, false).unwrap();
            } else {
                assert!(connection.is_none());
                assert!(environment(&plan, &chunks, &journal, false).is_err());
            }
        }
    }

    #[test]
    fn crusher_attachment_uses_the_actual_outer_port_not_its_occupied_peer_cell() {
        let (mut chunks, mut journal) = fixture();
        let anchor = [7, 5, 7];
        let peer = [8, 5, 7];
        for (position, other, secondary) in [(anchor, peer, false), (peer, anchor, true)] {
            let (section, local) = terra_voxel::global_to_section(position);
            chunks
                .loaded
                .get_mut(&section)
                .unwrap()
                .data
                .set_block(local, 99)
                .unwrap();
            journal.record(position, 99).unwrap();
            journal
                .set_object_state(
                    position,
                    Some(BlockObjectState {
                        peer: Some(other),
                        secondary,
                        ..Default::default()
                    }),
                )
                .unwrap();
        }
        for clicked in [anchor, peer] {
            for (normal, expected) in [([1, 0, 0], [9, 5, 7]), ([-1, 0, 0], [6, 5, 7])] {
                if (normal == [1, 0, 0] && clicked == anchor)
                    || (normal == [-1, 0, 0] && clicked == peer)
                {
                    assert!(
                        crate::targeting::item_placement_with_state(
                            &chunks,
                            &journal,
                            terra_voxel::blocks::BlockHit {
                                position: clicked,
                                normal,
                                distance: 2.0
                            },
                            146
                        )
                        .is_none()
                    );
                    continue;
                }
                let plan = resolve(
                    &chunks,
                    &journal,
                    terra_voxel::blocks::BlockHit {
                        position: clicked,
                        normal,
                        distance: 2.0,
                    },
                    terra_core::ItemKind::Block(146),
                    ([11.0, 5.5, 7.5], [-1.0, 0.0, 0.0]),
                    [0.0, 1.3],
                    false,
                )
                .unwrap();
                let node = planned_node(&plan);
                assert_eq!(node.position, expected);
                assert_eq!(
                    terra_core::kinetic::axis(node.facing).map(i64::abs),
                    [1, 0, 0]
                );
            }
        }
    }

    #[test]
    fn floor_conveyor_accepts_gears_and_shafts_at_real_ports_and_adapts_to_shafts() {
        use terra_core::kinetic::{self, Node};
        for facing in 0..4 {
            let (mut chunks, mut journal) = fixture();
            let anchor = [7, 2, 7];
            place_kinetic(&mut chunks, &mut journal, anchor, 148, facing);
            let hit = terra_voxel::blocks::BlockHit {
                position: anchor,
                normal: kinetic::axis(facing).map(|v| v as i32),
                distance: 2.0,
            };
            for material in [143, 146] {
                let plan = resolve(
                    &chunks,
                    &journal,
                    hit,
                    terra_core::ItemKind::Block(material),
                    ([7.5, 3.0, 11.0], [0.0, 0.0, -1.0]),
                    [0.0, 1.3],
                    false,
                )
                .unwrap();
                let node = planned_node(&plan);
                assert_eq!(
                    chunks.block_at([node.position[0], 1, node.position[2]]),
                    Some(3)
                );
                assert!(
                    kinetic::connection(
                        node,
                        Node {
                            position: anchor,
                            material: 148,
                            facing,
                            ..node
                        }
                    )
                    .is_some()
                );
                if material == 143 {
                    let shapes =
                        boxes(&plan, &chunks, &journal, &BlockPropertiesCatalog::default());
                    assert!(shapes.len() > 1);
                    assert_eq!(
                        shapes
                            .iter()
                            .map(|b| b.minimum[1])
                            .fold(f64::INFINITY, f64::min),
                        1.95
                    );
                }
            }
            assert!(
                resolve(
                    &chunks,
                    &journal,
                    hit,
                    terra_core::ItemKind::Block(151),
                    ([7.5, 3.0, 11.0], [0.0, 0.0, -1.0]),
                    [0.0, 1.3],
                    false,
                )
                .is_ok()
            );
            let mut top = f64::NEG_INFINITY;
            let mut blocked_axis = false;
            let mut bearing = false;
            let center = [7.5, 2.0 + f64::from(kinetic::CONVEYOR_AXIS_HEIGHT), 7.5];
            let bearing_offset =
                Quat::from_rotation_y(f32::from(facing) * std::f32::consts::FRAC_PI_2)
                    * Vec3::new(-0.35, 0.0, -0.118);
            let bearing_point: [f64; 3] =
                std::array::from_fn(|axis| center[axis] + f64::from(bearing_offset[axis]));
            crate::interaction_shapes::visit_boxes(
                &chunks,
                &journal,
                &BlockPropertiesCatalog::default(),
                anchor,
                |bounds| {
                    top = top.max(bounds.maximum[1]);
                    blocked_axis |= bounds.intersects(
                        std::array::from_fn(|i| center[i] - 0.01),
                        std::array::from_fn(|i| center[i] + 0.01),
                    );
                    bearing |= bounds.intersects(
                        bearing_point.map(|value| value - 0.002),
                        bearing_point.map(|value| value + 0.002),
                    );
                },
            );
            assert_eq!(top, 2.0 + f64::from(kinetic::CONVEYOR_TOP));
            assert!(
                !blocked_axis,
                "an empty bearing cannot contain an invisible replacement shaft"
            );
            assert!(
                bearing,
                "the open bearing's actual metal rim keeps its collision"
            );

            place_kinetic(&mut chunks, &mut journal, anchor, 146, facing);
            let mut real_shaft = false;
            crate::interaction_shapes::visit_boxes(
                &chunks,
                &journal,
                &BlockPropertiesCatalog::default(),
                anchor,
                |bounds| {
                    real_shaft |= bounds.intersects(
                        center.map(|value| value - 0.01),
                        center.map(|value| value + 0.01),
                    )
                },
            );
            assert!(
                real_shaft,
                "only the real placed shaft fills the bearing axis"
            );
            let plan = resolve(
                &chunks,
                &journal,
                terra_voxel::blocks::BlockHit {
                    position: anchor,
                    normal: kinetic::axis(facing).map(|v| v as i32),
                    distance: 2.0,
                },
                terra_core::ItemKind::Block(148),
                ([7.5, 5.0, 7.5], [0.0, -1.0, 0.0]),
                [2.0, 1.3],
                false,
            )
            .unwrap();
            assert_eq!(
                kinetic::axis(planned_node(&plan).facing).map(i64::abs),
                kinetic::axis(facing).map(i64::abs)
            );
        }
    }

    #[test]
    fn exact_mechanical_port_placement_crosses_negative_section_borders_but_not_unloaded_ends() {
        let (mut chunks, mut journal) = fixture();
        for x in [-1, 0] {
            for z in [-1, 0] {
                let position = SectionPos { x, y: 0, z };
                chunks
                    .loaded
                    .entry(position)
                    .or_insert_with(|| LoadedSection {
                        data: BlockSection::empty(position).unwrap(),
                        mesh: None,
                        mesh_epoch: 0,
                        water_epoch: 0,
                        water: None,
                        props: Vec::new(),
                    });
            }
        }
        let anchor = [-1, 5, -1];
        place_kinetic(&mut chunks, &mut journal, anchor, 146, 0);
        let hit = terra_voxel::blocks::BlockHit {
            position: anchor,
            normal: [1, 0, 0],
            distance: 2.0,
        };
        let plan = resolve(
            &chunks,
            &journal,
            hit,
            terra_core::ItemKind::Block(143),
            ([-0.5, 5.5, 3.0], [0.0, 0.0, -1.0]),
            [2.0, 1.3],
            false,
        )
        .unwrap();
        assert_eq!(plan.edits[0].position, [0, 5, -1]);
        chunks.loaded.remove(&SectionPos { x: 0, y: 0, z: -1 });
        let (section, local) = terra_voxel::global_to_section([-2, 5, -1]);
        chunks
            .loaded
            .get_mut(&section)
            .unwrap()
            .data
            .set_block(local, 3)
            .unwrap();
        assert!(crate::targeting::item_placement_with_state(&chunks, &journal, hit, 143).is_none());
    }

    #[test]
    fn kinetic_axes_prefer_usable_neighbors_and_clicked_faces_to_view() {
        let (mut chunks, mut journal) = fixture();
        place_kinetic(&mut chunks, &mut journal, [7, 5, 7], 146, 4);
        assert_eq!(
            kinetic_facing(&chunks, &journal, [7, 6, 7], 146, None, [1.5, 0.0]),
            4
        );
        let hit = terra_voxel::blocks::BlockHit {
            position: [6, 5, 10],
            normal: [1, 0, 0],
            distance: 2.0,
        };
        assert_eq!(
            kinetic_facing(&chunks, &journal, [7, 5, 10], 146, Some(hit), [1.5, 1.3]),
            0
        );
        place_kinetic(&mut chunks, &mut journal, [10, 5, 7], 143, 0);
        let face = terra_voxel::blocks::BlockHit {
            position: [10, 5, 8],
            normal: [0, 1, 0],
            distance: 2.0,
        };
        assert_eq!(
            kinetic_facing(&chunks, &journal, [10, 6, 8], 151, Some(face), [0.0, 1.3]),
            4
        );
        place_kinetic(&mut chunks, &mut journal, [7, 8, 7], 151, 0);
        assert_eq!(
            kinetic_facing(&chunks, &journal, [8, 9, 7], 151, None, [0.0, 1.3]),
            4
        );
    }

    #[test]
    fn waterwheel_floor_ceiling_and_neighbor_placement_always_selects_a_horizontal_axle() {
        let (mut chunks, mut journal) = fixture();
        let position = [6, 5, 6];
        for normal in [[0, 1, 0], [0, -1, 0]] {
            let clicked = terra_voxel::blocks::BlockHit {
                position: std::array::from_fn(|i| position[i] - i64::from(normal[i])),
                normal,
                distance: 2.0,
            };
            for pitch in [-1.3, 1.3] {
                let facing = kinetic_facing(
                    &chunks,
                    &journal,
                    position,
                    147,
                    Some(clicked),
                    [std::f32::consts::FRAC_PI_2, pitch],
                );
                assert_eq!(facing, 1);
                let plan = layout(
                    &chunks,
                    &journal,
                    position,
                    0,
                    147,
                    [std::f32::consts::FRAC_PI_2, pitch],
                    None,
                )
                .unwrap();
                environment(&plan, &chunks, &journal, false).unwrap();
            }
        }
        place_kinetic(&mut chunks, &mut journal, [7, 5, 6], 146, 0);
        place_kinetic(&mut chunks, &mut journal, [6, 6, 6], 146, 4);
        let clicked = terra_voxel::blocks::BlockHit {
            position: [6, 6, 6],
            normal: [0, -1, 0],
            distance: 2.0,
        };
        assert_eq!(
            kinetic_facing(
                &chunks,
                &journal,
                position,
                147,
                Some(clicked),
                [std::f32::consts::FRAC_PI_2, 1.3]
            ),
            0
        );
        let mut invalid = layout(&chunks, &journal, position, 0, 147, [0.0; 2], None).unwrap();
        invalid.states[0].1.as_mut().unwrap().facing = 4;
        assert!(environment(&invalid, &chunks, &journal, false).is_err());
        assert!(
            BlockObjectState {
                facing: 4,
                ..Default::default()
            }
            .valid_for(position, 147)
        );
    }

    #[test]
    fn extended_gears_require_free_physical_space_but_allow_real_diagonal_meshes() {
        let (mut chunks, mut journal) = fixture();
        let position = [7, 5, 7];
        let plan = layout(&chunks, &journal, position, 0, 151, [0.0; 2], None).unwrap();
        environment(&plan, &chunks, &journal, false).unwrap();
        chunks
            .loaded
            .get_mut(&SectionPos::default())
            .unwrap()
            .data
            .set_block([7, 5, 8], 3)
            .unwrap();
        assert!(environment(&plan, &chunks, &journal, false).is_err());
        chunks
            .loaded
            .get_mut(&SectionPos::default())
            .unwrap()
            .data
            .set_block([7, 5, 8], 0)
            .unwrap();
        place_kinetic(&mut chunks, &mut journal, [7, 6, 8], 143, 0);
        environment(&plan, &chunks, &journal, false).unwrap();
        place_kinetic(&mut chunks, &mut journal, [15, 5, 7], 151, 4);
        let properties = BlockPropertiesCatalog::default();
        assert!(crate::interaction_shapes::space_intersects(
            &chunks,
            &journal,
            &properties,
            [14, 5, 7],
            [14.49, 5.4, 7.4],
            [14.51, 5.6, 7.6]
        ));
        let fill = layout(&chunks, &journal, [14, 5, 7], 0, 3, [0.0; 2], None).unwrap();
        assert!(environment(&fill, &chunks, &journal, false).is_err());
    }
    #[test]
    fn gear_radial_overhang_may_enter_a_floor_but_not_walls_or_the_axle_cell() {
        for material in [143, 151] {
            let (mut chunks, mut journal) = fixture();
            place_kinetic(&mut chunks, &mut journal, [7, 2, 7], 82, 0);
            let plan = layout(&chunks, &journal, [7, 3, 7], 0, material, [0.0; 2], None).unwrap();
            environment(&plan, &chunks, &journal, false).unwrap();
            chunks
                .loaded
                .get_mut(&SectionPos::default())
                .unwrap()
                .data
                .set_block([7, 2, 7], 3)
                .unwrap();
            environment(&plan, &chunks, &journal, false).unwrap();
            chunks
                .loaded
                .get_mut(&SectionPos::default())
                .unwrap()
                .data
                .set_block([7, 3, 6], 3)
                .unwrap();
            assert!(environment(&plan, &chunks, &journal, false).is_err());
            let data = &mut chunks.loaded.get_mut(&SectionPos::default()).unwrap().data;
            data.set_block([7, 3, 6], 0).unwrap();
            data.set_block([7, 3, 7], 3).unwrap();
            assert!(environment(&plan, &chunks, &journal, false).is_err());
        }
    }
    #[test]
    fn floor_can_be_added_under_existing_gears_in_either_build_order_without_allowing_walls() {
        for gear in [143, 151] {
            let (mut chunks, mut journal) = fixture();
            place_kinetic(&mut chunks, &mut journal, [7, 3, 7], gear, 0);
            for material in [2, 3, 82] {
                let floor =
                    layout(&chunks, &journal, [7, 2, 7], 0, material, [0.0; 2], None).unwrap();
                environment(&floor, &chunks, &journal, false).unwrap();
            }
            let wall = layout(&chunks, &journal, [7, 3, 6], 0, 3, [0.0; 2], None).unwrap();
            assert!(environment(&wall, &chunks, &journal, false).is_err());
        }
    }
    fn fixture() -> (ChunkStore, EditJournal) {
        let mut data = BlockSection::empty(SectionPos::default()).unwrap();
        for x in 0..16 {
            for z in 0..16 {
                data.set_block([x, 1, z], 3).unwrap();
            }
        }
        let mut chunks = ChunkStore::default();
        chunks.loaded.insert(
            SectionPos::default(),
            LoadedSection {
                data,
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            },
        );
        (chunks, EditJournal::default())
    }
    #[test]
    fn construction_and_train_parts_attach_to_every_face_without_a_floor() {
        for material in [
            3, 29, 70, 210, 211, 212, 213, 214, 215, 216, 217, 218, 219, 247, 268, 270, 288,
        ] {
            for normal in crate::objects::DIRECTIONS {
                let (mut chunks, mut journal) = fixture();
                let clicked = [7, 7, 7];
                let position = std::array::from_fn(|axis| clicked[axis] + normal[axis]);
                chunks
                    .loaded
                    .get_mut(&SectionPos::default())
                    .unwrap()
                    .data
                    .set_block([7, 7, 7], 212)
                    .unwrap();
                journal.record(clicked, 212).unwrap();
                journal
                    .set_object_state(clicked, Some(Default::default()))
                    .unwrap();
                let hit = terra_voxel::blocks::BlockHit {
                    position: clicked,
                    normal: normal.map(|v| v as i32),
                    distance: 2.0,
                };
                let plan = resolve(
                    &chunks,
                    &journal,
                    hit,
                    terra_core::ItemKind::Block(material),
                    ([7.5, 7.5, 11.5], [0.0, 0.0, -1.0]),
                    [0.0; 2],
                    true,
                )
                .unwrap_or_else(|error| panic!("{material} on {normal:?}: {error}"));
                assert!(
                    plan.edits
                        .iter()
                        .any(|edit| edit.position == position && edit.material == material)
                );
                assert_eq!(
                    chunks.block_at([position[0], position[1] - 1, position[2]]),
                    Some(if normal == [0, 1, 0] { 212 } else { 0 })
                );
            }
        }
    }

    #[test]
    fn full_train_frames_support_carpet_and_potted_plants_without_hidden_air_gaps() {
        for frame in [210, 212, 270, 276, 282, 288] {
            for decoration in [
                terra_core::textiles::WHITE_CARPET,
                terra_core::flower_pot::MATERIAL,
            ] {
                let (mut chunks, mut journal) = fixture();
                let floor = [7, 5, 7];
                let position = [7, 6, 7];
                place_kinetic(&mut chunks, &mut journal, floor, frame, 0);
                let hit = terra_voxel::blocks::BlockHit {
                    position: floor,
                    normal: [0, 1, 0],
                    distance: 2.0,
                };
                let plan = resolve(
                    &chunks,
                    &journal,
                    hit,
                    terra_core::ItemKind::Block(decoration),
                    ([7.5, 8.5, 7.5], [0.0, -1.0, 0.0]),
                    [0.0; 2],
                    true,
                )
                .unwrap_or_else(|error| panic!("{decoration} on frame {frame}: {error}"));
                assert_eq!(plan.edits[0].position, position);
                assert_eq!(plan.edits[0].material, decoration);
                environment(&plan, &chunks, &journal, true).unwrap();
            }
        }
    }
    #[test]
    fn shared_furniture_layout_keeps_all_four_facings_and_real_multicell_axes() {
        let (chunks, journal) = fixture();
        for material in [28, 29, 59, 65, 66, 67, 68, 69, 70, 71, 72, 73, 99, 247, 248] {
            for facing in 0..4 {
                let yaw = facing as f32 * std::f32::consts::FRAC_PI_2;
                let plan =
                    layout(&chunks, &journal, [6, 2, 6], 0, material, [yaw, 0.0], None).unwrap();
                environment(&plan, &chunks, &journal, true).unwrap();
                let state = plan
                    .states
                    .iter()
                    .find(|(p, _)| *p == [6, 2, 6])
                    .unwrap()
                    .1
                    .unwrap();
                assert_eq!(
                    state.facing,
                    if terra_core::beds::is_bed(material) {
                        (facing + 2) % 4
                    } else {
                        facing
                    }
                );
                if let Some(peer) = state.peer {
                    assert_eq!(plan.edits.len(), 2);
                    assert_eq!(
                        peer[1],
                        if matches!(material, 59 | 65..=67 | 139 | 248) {
                            3
                        } else {
                            2
                        }
                    );
                    let secondary = plan
                        .states
                        .iter()
                        .find(|(p, _)| *p == peer)
                        .unwrap()
                        .1
                        .unwrap();
                    assert!(secondary.secondary && secondary.peer == Some([6, 2, 6]));
                } else {
                    assert_eq!(plan.edits.len(), 1);
                }
            }
        }
    }
    #[test]
    fn invalid_station_keeps_its_exact_proposed_footprint_without_mutating_the_world() {
        let (mut chunks, journal) = fixture();
        chunks
            .loaded
            .get_mut(&SectionPos::default())
            .unwrap()
            .data
            .set_block([7, 2, 6], 3)
            .unwrap();
        let plan = layout(&chunks, &journal, [6, 2, 6], 0, 28, [0.0; 2], None).unwrap();
        assert_eq!(plan.edits[1].position, [7, 2, 6]);
        assert!(environment(&plan, &chunks, &journal, true).is_err());
        assert_eq!(chunks.block_at([6, 2, 6]), Some(0));
    }
    #[test]
    fn combined_chest_preview_includes_the_existing_primary_and_preserves_both_woods() {
        let (mut chunks, mut journal) = fixture();
        chunks
            .loaded
            .get_mut(&SectionPos::default())
            .unwrap()
            .data
            .set_block([6, 2, 6], 29)
            .unwrap();
        journal.record([6, 2, 6], 29).unwrap();
        journal
            .set_object_state(
                [6, 2, 6],
                Some(BlockObjectState {
                    wood: terra_core::WoodSpecies::Birch,
                    ..Default::default()
                }),
            )
            .unwrap();
        let plan = layout(&chunks, &journal, [7, 2, 6], 0, 29, [0.0; 2], None).unwrap();
        assert!(plan.states.iter().any(|(p, s)| *p == [6, 2, 6]
            && s.is_some_and(|s| !s.secondary
                && s.peer == Some([7, 2, 6])
                && s.wood == terra_core::WoodSpecies::Birch)));
        assert!(
            plan.states.iter().any(|(p, s)| *p == [7, 2, 6]
                && s.is_some_and(|s| s.secondary && s.peer == Some([6, 2, 6])))
        );
        assert!(!boxes(&plan, &chunks, &journal, &BlockPropertiesCatalog::default()).is_empty());
    }

    #[test]
    fn side_chests_inherit_the_existing_facing_for_both_sides_without_changing_its_wood() {
        for facing in 0..4 {
            for sign in [-1, 1] {
                let (mut chunks, mut journal) = fixture();
                let anchor = [6, 2, 6];
                let original = BlockObjectState {
                    facing,
                    wood: terra_core::WoodSpecies::Cherry,
                    ..Default::default()
                };
                chunks
                    .loaded
                    .get_mut(&SectionPos::default())
                    .unwrap()
                    .data
                    .set_block([6, 2, 6], 29)
                    .unwrap();
                journal.record(anchor, 29).unwrap();
                journal.set_object_state(anchor, Some(original)).unwrap();
                let position = offset(anchor, original.right(), sign);
                let plan = layout(
                    &chunks,
                    &journal,
                    position,
                    0,
                    29,
                    [
                        f32::from((facing + 1) % 4) * std::f32::consts::FRAC_PI_2,
                        0.0,
                    ],
                    None,
                )
                .unwrap();
                environment(&plan, &chunks, &journal, true).unwrap();
                assert_eq!(plan.edits.len(), 1);
                assert_eq!(plan.states.len(), 2);
                let old = plan
                    .states
                    .iter()
                    .find(|(p, _)| *p == anchor)
                    .unwrap()
                    .1
                    .unwrap();
                let new = plan
                    .states
                    .iter()
                    .find(|(p, _)| *p == position)
                    .unwrap()
                    .1
                    .unwrap();
                assert_eq!((old.facing, new.facing), (facing, facing));
                assert_eq!((old.peer, new.peer), (Some(position), Some(anchor)));
                assert_ne!(old.secondary, new.secondary);
                assert_eq!(old.wood, original.wood);
                assert!(old.valid_for(anchor, 29) && new.valid_for(position, 29));
                assert_eq!(journal.object_state(anchor), Some(original));
            }
        }
    }

    #[test]
    fn chest_merge_clearance_ignores_only_its_own_peer_and_still_rejects_foreign_gear_and_stale_walls()
     {
        for sign in [-1, 1] {
            let (mut chunks, mut journal) = fixture();
            let anchor = [6, 2, 6];
            let position = [6 + sign, 2, 6];
            chunks
                .loaded
                .get_mut(&SectionPos::default())
                .unwrap()
                .data
                .set_block([6, 2, 6], 29)
                .unwrap();
            journal.record(anchor, 29).unwrap();
            journal
                .set_object_state(anchor, Some(BlockObjectState::default()))
                .unwrap();
            place_kinetic(&mut chunks, &mut journal, [12, 3, 12], 141, 0);
            let plan = layout(
                &chunks,
                &journal,
                position,
                0,
                29,
                [std::f32::consts::FRAC_PI_2, 0.0],
                None,
            )
            .unwrap();
            environment(&plan, &chunks, &journal, true)
                .expect("planned chest halves must not obstruct one another");
            let gear = [position[0], 3, position[2]];
            place_kinetic(&mut chunks, &mut journal, gear, 151, 0);
            assert_eq!(
                environment(&plan, &chunks, &journal, true).unwrap_err(),
                "Mechanical assembly is obstructed"
            );
            let (section, local) = terra_voxel::global_to_section(gear);
            chunks
                .loaded
                .get_mut(&section)
                .unwrap()
                .data
                .set_block(local, 0)
                .unwrap();
            journal.record(gear, 0).unwrap();
            let (section, local) = terra_voxel::global_to_section(anchor);
            chunks
                .loaded
                .get_mut(&section)
                .unwrap()
                .data
                .set_block(local, 3)
                .unwrap();
            journal.record(anchor, 3).unwrap();
            assert_eq!(
                environment(&plan, &chunks, &journal, true).unwrap_err(),
                "Mechanical assembly is obstructed"
            );
            assert_eq!(chunks.block_at(position), Some(0));
        }
    }

    #[test]
    fn proposed_collision_matches_committed_objects_for_facings_mounts_and_mixed_slabs() {
        let properties = BlockPropertiesCatalog::default();
        for material in [
            28, 29, 40, 59, 60, 65, 68, 71, 74, 75, 76, 80, 81, 82, 99, 104, 105,
        ]
        .into_iter()
        .chain(terra_core::castle::BLOCKS)
        {
            let facings: Vec<_> = if terra_core::construction::is_slab(material) {
                (0..=terra_core::construction::LAST_MIXED_SLAB)
                    .filter(|&f| terra_core::construction::valid_facing(material, f))
                    .collect()
            } else if terra_core::construction::is_stair(material)
                || terra_core::castle::is_castle(material)
            {
                (0..24).collect()
            } else if material == 60 {
                (0..64)
                    .filter(|&f| crate::objects::chimney_basis(f).is_some())
                    .collect()
            } else if material == 40 {
                vec![0, 4, 5, 6, 7]
            } else {
                (0..4).collect()
            };
            for facing in facings {
                let (mut chunks, mut journal) = fixture();
                let yaw = if terra_core::beds::is_bed(material) {
                    (facing + 2) % 4
                } else {
                    facing % 4
                } as f32
                    * std::f32::consts::FRAC_PI_2;
                let mut plan =
                    layout(&chunks, &journal, [6, 2, 6], 0, material, [yaw, 0.0], None).unwrap();
                if plan.states.is_empty() {
                    plan.states
                        .push(([6, 2, 6], Some(BlockObjectState::default())));
                }
                for (_, state) in &mut plan.states {
                    state.as_mut().unwrap().facing = facing;
                }
                let expected = boxes(&plan, &chunks, &journal, &properties);
                assert!(!expected.is_empty(), "{material}/{facing}");
                for edit in &plan.edits {
                    let (section, local) = terra_voxel::global_to_section(edit.position);
                    chunks
                        .loaded
                        .get_mut(&section)
                        .unwrap()
                        .data
                        .set_block(local, edit.material)
                        .unwrap();
                    journal.record(edit.position, edit.material).unwrap();
                }
                for (position, state) in &plan.states {
                    journal.set_object_state(*position, *state).unwrap();
                }
                let mut actual = Vec::new();
                let positions: std::collections::BTreeSet<_> = plan
                    .edits
                    .iter()
                    .map(|e| e.position)
                    .chain(plan.states.iter().map(|(p, _)| *p))
                    .collect();
                for position in positions {
                    if journal.object_state(position).is_some_and(|s| s.secondary) {
                        continue;
                    }
                    crate::interaction_shapes::visit_boxes(
                        &chunks,
                        &journal,
                        &properties,
                        position,
                        |b| actual.push(b),
                    );
                }
                assert_eq!(expected, actual, "{material}/{facing}");
            }
        }
    }

    #[test]
    fn carpets_and_pots_require_a_real_full_support_and_pots_start_empty_independent_of_yaw() {
        let (chunks, journal) = fixture();
        for material in [106, 118, 131] {
            for yaw in [0.0, 1.5, 3.0, -1.5] {
                let plan =
                    layout(&chunks, &journal, [6, 2, 6], 0, material, [yaw, 0.0], None).unwrap();
                if material == 131 {
                    assert_eq!(plan.states[0].1.unwrap().facing, 0);
                } else {
                    assert!(plan.states.is_empty());
                }
            }
            assert!(layout(&chunks, &journal, [6, 3, 6], 0, material, [0.0; 2], None).is_err());
        }
    }

    #[test]
    fn logistics_ports_mount_on_a_loaded_machine_crank_but_not_unrelated_or_sideways_drives() {
        for material in [99, 156] {
            for facing in 0..4 {
                for port in [152, 153] {
                    let (mut chunks, mut journal) = fixture();
                    let anchor = [6, 5, 6];
                    let direction = terra_core::kinetic::axis(facing);
                    let crank = terra_core::kinetic::offset(anchor, direction);
                    let position = terra_core::kinetic::offset(crank, direction);
                    for (cell, id, peer) in [(anchor, material, None), (crank, 100, Some(anchor))] {
                        let (section, local) = terra_voxel::global_to_section(cell);
                        chunks
                            .loaded
                            .get_mut(&section)
                            .unwrap()
                            .data
                            .set_block(local, id)
                            .unwrap();
                        journal.record(cell, id).unwrap();
                        journal
                            .set_object_state(
                                cell,
                                Some(BlockObjectState {
                                    facing,
                                    peer,
                                    ..Default::default()
                                }),
                            )
                            .unwrap();
                    }
                    let hit = terra_voxel::blocks::BlockHit {
                        position: crank,
                        normal: direction.map(|v| v as i32),
                        distance: 2.0,
                    };
                    let plan = resolve(
                        &chunks,
                        &journal,
                        hit,
                        terra_core::ItemKind::Block(port),
                        (
                            position.map(|v| v as f64 + 0.5),
                            direction.map(|v| -v as f64),
                        ),
                        [0.0; 2],
                        true,
                    )
                    .unwrap();
                    assert_eq!(plan.edits[0].position, position);
                    assert_eq!(plan.states[0].1.unwrap().peer, Some(crank));
                    assert_eq!(
                        logistics_inventory_at(&chunks, &journal, position, crank),
                        Some(anchor)
                    );
                    assert!(terra_core::logistics::extracts(
                        port,
                        plan.states[0].1.unwrap().facing
                    ));
                    environment(&plan, &chunks, &journal, true).unwrap();
                    assert_eq!(
                        logistics_inventory_at(
                            &chunks,
                            &journal,
                            [crank[0], crank[1] + 1, crank[2]],
                            crank
                        ),
                        None
                    );
                    let (_, local) = terra_voxel::global_to_section(anchor);
                    chunks
                        .loaded
                        .get_mut(&SectionPos::default())
                        .unwrap()
                        .data
                        .set_block(local, 3)
                        .unwrap();
                    assert!(environment(&plan, &chunks, &journal, true).is_err());
                }
            }
        }
    }

    #[test]
    fn logistics_ports_mount_on_clicked_inventories_without_a_solid_floor() {
        for (port, target) in [
            (152, 29),
            (152, 59),
            (152, 99),
            (153, 29),
            (153, 59),
            (153, 99),
        ] {
            let (mut chunks, mut journal) = fixture();
            let owner = [6, 5, 6];
            chunks
                .loaded
                .get_mut(&SectionPos::default())
                .unwrap()
                .data
                .set_block([6, 5, 6], target)
                .unwrap();
            journal.record(owner, target).unwrap();
            let hit = terra_voxel::blocks::BlockHit {
                position: owner,
                normal: [0, 0, 1],
                distance: 2.0,
            };
            let plan = resolve(
                &chunks,
                &journal,
                hit,
                terra_core::ItemKind::Block(port),
                ([6.5, 5.5, 9.0], [0.0, 0.0, -1.0]),
                [0.0; 2],
                true,
            )
            .unwrap();
            assert_eq!(plan.states[0].1.unwrap().peer, Some(owner));
            assert_eq!(plan.edits[0].position, [6, 5, 7]);
            assert_eq!(
                terra_core::logistics::extracts(port, plan.states[0].1.unwrap().facing),
                target != 29
            );
            environment(&plan, &chunks, &journal, true).unwrap();
        }
        let (chunks, journal) = fixture();
        for port in [152, 153] {
            let plan = layout(&chunks, &journal, [6, 4, 6], 0, port, [0.0; 2], None).unwrap();
            assert_eq!(plan.states[0].1.unwrap().peer, None);
            environment(&plan, &chunks, &journal, true).unwrap();
        }
    }

    #[test]
    fn standalone_ports_use_the_clicked_surface_and_only_their_feet_may_enter_a_floor() {
        for material in [152, 153] {
            for normal in [
                [1, 0, 0],
                [-1, 0, 0],
                [0, 1, 0],
                [0, -1, 0],
                [0, 0, 1],
                [0, 0, -1],
            ] {
                let (mut chunks, mut journal) = fixture();
                let position = [6, 4, 6];
                let clicked = std::array::from_fn(|i| position[i] - i64::from(normal[i]));
                let (section, local) = terra_voxel::global_to_section(clicked);
                chunks
                    .loaded
                    .get_mut(&section)
                    .unwrap()
                    .data
                    .set_block(local, 3)
                    .unwrap();
                let hit = terra_voxel::blocks::BlockHit {
                    position: clicked,
                    normal,
                    distance: 2.0,
                };
                let plan = resolve(
                    &chunks,
                    &journal,
                    hit,
                    terra_core::ItemKind::Block(material),
                    ([6.5, 7.5, 10.0], [0.0, 0.0, -1.0]),
                    [1.2, 0.0],
                    true,
                )
                .unwrap();
                assert_eq!(plan.edits[0].position, position);
                assert_eq!(plan.states[0].1.unwrap().peer, None);
                if normal[1] == 0 {
                    assert_eq!(
                        terra_core::kinetic::forward(plan.states[0].1.unwrap().facing),
                        normal.map(i64::from)
                    );
                }
                if normal == [0, 1, 0] {
                    chunks
                        .loaded
                        .get_mut(&SectionPos::default())
                        .unwrap()
                        .data
                        .set_block([6, 4, 6], material)
                        .unwrap();
                    journal.record(position, material).unwrap();
                    journal
                        .set_object_state(position, plan.states[0].1)
                        .unwrap();
                    chunks
                        .loaded
                        .get_mut(&SectionPos::default())
                        .unwrap()
                        .data
                        .set_block([6, 3, 6], 0)
                        .unwrap();
                    let floor = layout(&chunks, &journal, clicked, 0, 3, [0.0; 2], None).unwrap();
                    environment(&floor, &chunks, &journal, true).unwrap();
                }
            }
        }
    }
}
