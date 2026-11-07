use super::*;
use terra_world::{
    edits::BlockObjectState,
    streaming::{BlockEditRequest, commit_block_edit, commit_object_state},
};

pub(super) struct Plan {
    edits: Vec<BlockEditRequest>,
    states: Vec<([i64; 3], Option<BlockObjectState>)>,
    tint: Option<[u8; 3]>,
    belts: Vec<([i64; 3], Option<terra_world::edits::BeltOwner>)>,
    resource_drops: usize,
}
impl Plan {
    pub(super) fn allowed(&self, peer: &Peer, world: &ControlWorld) -> bool {
        self.edits.iter().all(|edit| {
            super::kingdoms::edit_allowed(
                EditAction::Block {
                    position: edit.position,
                    expected: edit.expected_material,
                    material: edit.material,
                },
                peer,
                world,
            )
        }) && self.states.iter().all(|(position, _)| {
            self.edits.iter().any(|edit| edit.position == *position)
                || super::kingdoms::permission_allowed(
                    peer.uuid,
                    terra_core::kingdom::KingdomPermission::Build,
                    *position,
                    world,
                )
        })
    }
    pub(super) fn with_resource_drops(mut self, count: usize) -> Self {
        self.resource_drops = count;
        self
    }
    pub(super) fn with_slab_half(
        mut self,
        position: [i64; 3],
        half: terra_world::slabs::HalfRemoval,
        previous: BlockObjectState,
    ) -> Self {
        if let Some(edit) = self.edits.iter_mut().find(|edit| edit.position == position) {
            edit.material = half.remaining;
        }
        self.states.retain(|(cell, _)| *cell != position);
        self.states
            .push((position, Some(half.remaining_state(previous))));
        self
    }
    pub(super) fn with_construction_facing(mut self, facing: Option<u8>) -> Self {
        if let Some(facing) = facing.filter(|_| self.belts.is_empty()) {
            if terra_core::construction::full_slab(facing) {
                for edit in &mut self.edits {
                    if terra_core::construction::is_slab(edit.expected_material) {
                        edit.material = edit.expected_material;
                    }
                }
            }
            for (_, state) in &mut self.states {
                if let Some(state) = state {
                    state.facing = facing;
                }
            }
        }
        self
    }
    pub(super) fn with_item(mut self, item: Option<terra_core::ItemKind>) -> Self {
        self.tint = item.and_then(terra_core::tints::color);
        if let Some(terra_core::ItemKind::DyedBed { material, color }) = item
            && terra_core::beds::is_bed(material)
        {
            for (position, state) in &mut self.states {
                if self.edits.iter().any(|edit| {
                    edit.position == *position
                        && edit.material == material
                        && matches!(edit.expected_material, 0 | 27 | 98)
                }) && let Some(state) = state
                {
                    state.facing = terra_core::beds::dyed_facing(state.facing, color);
                }
            }
        }
        self
    }
    pub(super) fn with_wood(mut self, position: [i64; 3], wood: terra_core::WoodSpecies) -> Self {
        let bench = self.edits.first().is_some_and(|edit| edit.material == 28);
        for (cell, state) in &mut self.states {
            if (bench || *cell == position)
                && let Some(state) = state
            {
                state.wood = wood;
            }
        }
        self
    }
}
#[cfg(test)]
fn offset(p: [i64; 3], d: [i64; 3], sign: i64) -> [i64; 3] {
    std::array::from_fn(|i| p[i] + d[i] * sign)
}
pub(super) fn plan(command: EditCommand, world: &ControlWorld) -> Result<Option<Plan>, String> {
    plan_with_crank(command, world, None, None, None)
}
pub(super) fn plan_from_layout(
    command: EditCommand,
    world: &ControlWorld,
    layout: terra_world::placement::Layout,
) -> Result<Option<Plan>, String> {
    plan_with_crank(command, world, None, None, Some(layout))
}
pub(super) fn plan_kinetic(
    command: EditCommand,
    world: &ControlWorld,
    facing: u8,
) -> Result<Option<Plan>, String> {
    plan_with_crank(command, world, None, Some(facing), None)
}
pub(super) fn plan_crank(
    command: EditCommand,
    world: &ControlWorld,
    anchor: [i64; 3],
) -> Result<Option<Plan>, String> {
    plan_with_crank(command, world, Some(anchor), None, None)
}
fn plan_with_crank(
    command: EditCommand,
    world: &ControlWorld,
    crank_owner: Option<[i64; 3]>,
    facing: Option<u8>,
    placement: Option<terra_world::placement::Layout>,
) -> Result<Option<Plan>, String> {
    let EditAction::Block {
        position,
        expected,
        material,
    } = command.action
    else {
        return Ok(None);
    };
    if !(terra_world::placement::furniture(material)
        || material == 0 && terra_world::placement::furniture(expected))
    {
        return Ok(None);
    }
    if position.iter().any(|p| p.unsigned_abs() > 990_000) {
        return Err("Object outside world bounds".into());
    }
    if terra_core::plumbing::is_part(material)
        && (!world.journal.plumbing_slot_available(position)
            || world.journal.gameplay().revision == u64::MAX)
    {
        return Err("Plumbing state capacity exhausted".into());
    }
    if matches!(material, 161 | 162)
        && !(expected == 161 && material == 162)
        && !world.journal.hearth_slot_available(position, material)
    {
        return Err("Hearth metadata capacity exhausted".into());
    }
    if matches!((expected, material), (161, 162) | (162, 161))
        && (!world.journal.attached_campfire_slot_available(position)
            || world.journal.has_attached_campfire(position)
            || expected == 162 && world.journal.campfire_at(position).is_none()
            || expected == 161 && world.journal.cauldron_at(position).is_none())
    {
        return Err("Compound hearth metadata unavailable".into());
    }
    if matches!(material, 59 | 99 | 156 | 161 | 162)
        && world.journal.gameplay().revision > u64::MAX - 16
    {
        return Err("Furnace state revision exhausted".into());
    }
    let mut plan = Plan {
        edits: vec![BlockEditRequest {
            position,
            expected_material: expected,
            material,
        }],
        states: Vec::new(),
        tint: None,
        belts: Vec::new(),
        resource_drops: 1,
    };
    if material == 0 {
        let spans = if expected == 148 {
            terra_world::conveyors::span_at(&world.journal, position)
                .filter(|s| s.linked)
                .into_iter()
                .collect()
        } else if matches!(expected, 144..=146) {
            terra_world::conveyors::connected_at(&world.journal, position)
        } else {
            Vec::new()
        };
        for span in spans {
            let removal = terra_world::conveyors::removal_layout(
                &world.chunks,
                &world.journal,
                span,
                matches!(expected, 144..=146).then_some(position),
            )?;
            for edit in removal.edits {
                if !plan.edits.iter().any(|old| old.position == edit.position) {
                    plan.edits.push(edit);
                }
            }
            plan.states.extend(removal.states);
            plan.belts.extend(removal.belts);
        }
        if terra_core::kinetic::is_crank_mount(expected) {
            for direction in terra_core::kinetic::DIRECTIONS {
                let cell = terra_core::kinetic::offset(position, direction);
                if world.journal.crank_anchor(cell) != Some(position) {
                    continue;
                }
                if world.chunks.block_at(cell) != Some(100) {
                    return Err("Mounted crank is not loaded".into());
                }
                plan.edits.push(BlockEditRequest {
                    position: cell,
                    expected_material: 100,
                    material: 0,
                });
            }
        }
        if matches!(expected, 59 | 99 | 156 | 161 | 162) {
            let state = world
                .journal
                .object_state(position)
                .ok_or("Missing furnace metadata")?;
            let anchor = if state.secondary {
                state.peer.ok_or("Missing furnace anchor")?
            } else {
                position
            };
            let crank = (expected == 99)
                .then(|| world.journal.crusher_crank_position(anchor))
                .flatten()
                .filter(|cell| world.journal.crank_anchor(*cell) == Some(anchor));
            if let Some(cell) = crank {
                if world.chunks.block_at(cell) != Some(100) {
                    return Err("Mounted crank is not loaded".into());
                }
                plan.edits.push(BlockEditRequest {
                    position: cell,
                    expected_material: 100,
                    material: 0,
                });
            }
        }
        if !matches!(expected, 100 | 144..=146 | 148)
            && !terra_core::logistics::is_port(expected)
            && let Some(state) = world.journal.object_state(position)
            && let Some(peer) = state.peer
        {
            let other = world
                .journal
                .object_state(peer)
                .ok_or("Missing object peer")?;
            if other.peer != Some(position) || world.chunks.block_at(peer) != Some(expected) {
                return Err("Invalid object peer".into());
            }
            if matches!(expected, 28 | 59 | 65..=67 | 71..=73 | 99 | 139) {
                plan.edits.push(BlockEditRequest {
                    position: peer,
                    expected_material: expected,
                    material: 0,
                });
            } else {
                plan.states.push((
                    peer,
                    Some(BlockObjectState {
                        peer: None,
                        secondary: false,
                        ..other
                    }),
                ));
            }
        }
    } else {
        let shared = if let Some(layout) = placement {
            layout
        } else {
            terra_world::placement::layout(
                &world.chunks,
                &world.journal,
                position,
                expected,
                material,
                command.look,
                crank_owner,
            )?
        };
        plan.edits = shared.edits;
        plan.states = shared.states;
        plan.belts = shared.belts;
        if let Some(facing) = facing.filter(|_| plan.belts.is_empty()) {
            for (_, state) in &mut plan.states {
                if let Some(state) = state {
                    state.facing = facing;
                }
            }
        }
    }
    let shared = terra_world::placement::Layout {
        edits: plan.edits.clone(),
        states: plan.states.clone(),
        belts: plan.belts.clone(),
    };
    terra_world::placement::environment(
        &shared,
        &world.chunks,
        &world.journal,
        world.config.world.bounded_world,
    )?;
    // Construction orientation is resolved by the clicked face in shared validation.
    if material != 0 && !terra_core::construction::is_shape(material) {
        let boxes = terra_world::placement::boxes(
            &shared,
            &world.chunks,
            &world.journal,
            &world.block_properties,
        );
        if world.bodies.iter().any(|body| {
            let half = Vec3::new(
                terra_physics::character::PLAYER_RADIUS,
                terra_physics::character::PLAYER_HALF_HEIGHT,
                terra_physics::character::PLAYER_RADIUS,
            );
            boxes.iter().any(|bounds| {
                bounds.intersects(
                    (body.translation - half).to_array().map(f64::from),
                    (body.translation + half).to_array().map(f64::from),
                )
            })
        }) {
            return Err("Object would intersect a player".into());
        }
    }
    if !world.journal.can_record_furniture(
        plan.edits
            .iter()
            .filter(|edit| {
                matches!(edit.material, 28..=29) && edit.expected_material != edit.material
            })
            .count(),
    ) {
        return Err("Furniture placement order exhausted".into());
    }
    preflight_object_revisions(&plan, world)?;
    preflight_inventory_drops(&plan, world, 0)?;
    Ok(Some(plan))
}

fn preflight_object_revisions(plan: &Plan, world: &ControlWorld) -> Result<(), String> {
    for position in plan
        .edits
        .iter()
        .map(|e| e.position)
        .chain(plan.states.iter().map(|(p, _)| *p))
    {
        let owner = terra_voxel::global_to_section(position).0;
        if world.journal.revision(owner) > u64::MAX - 32
            || world.journal.fluid_epoch(owner) > u64::MAX - 32
            || world.journal.appearance_epoch(owner) > u64::MAX - 32
            || world.journal.prop_epoch(owner) > u64::MAX - 32
            || world
                .chunks
                .loaded
                .get(&owner)
                .is_none_or(|s| s.data.revision() > u64::MAX - 32)
        {
            return Err("Object transaction cannot advance revision".into());
        }
        for dz in -1..=1 {
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let neighbor = owner.offset([dx, dy, dz]).map_err(|e| e.to_string())?;
                    if world.journal.mesh_epoch(neighbor) > u64::MAX - 64
                        || world.journal.water_mesh_epoch(neighbor) > u64::MAX - 64
                    {
                        return Err("Object mesh epoch exhausted".into());
                    }
                }
            }
        }
    }
    Ok(())
}

fn preflight_inventory_drops(
    plan: &Plan,
    world: &ControlWorld,
    resource_drops: usize,
) -> Result<(), String> {
    let positions: Vec<_> = plan
        .edits
        .iter()
        .filter(|edit| {
            edit.expected_material != edit.material
                && !(edit.expected_material == 162 && edit.material == 161)
                && (matches!(edit.expected_material, 29 | 59 | 99 | 156 | 161 | 162)
                    || world.journal.container_at(edit.position).is_some()
                    || world.journal.attached_campfire_at(edit.position).is_some())
        })
        .map(|edit| edit.position)
        .collect();
    let removing_drive = plan.edits.iter().any(|edit| {
        (edit.expected_material == 99
            || terra_core::kinetic::is_crank_mount(edit.expected_material))
            && edit.material != edit.expected_material
    });
    let cranks = if removing_drive {
        plan.edits
            .iter()
            .filter(|edit| edit.expected_material == 100 && edit.material == 0)
            .count()
    } else {
        0
    };
    if positions.is_empty() && cranks == 0 {
        return Ok(());
    }
    // A staged plan may be committed after its revisions changed. Validate
    // every cell before dropping a crank or removing the first machine half.
    for position in plan
        .edits
        .iter()
        .map(|edit| edit.position)
        .chain(plan.states.iter().map(|(position, _)| *position))
    {
        let owner = terra_voxel::global_to_section(position).0;
        if world.journal.revision(owner) > u64::MAX - 32
            || world.journal.fluid_epoch(owner) > u64::MAX - 32
            || world.journal.appearance_epoch(owner) > u64::MAX - 32
            || world.journal.prop_epoch(owner) > u64::MAX - 32
            || world
                .chunks
                .loaded
                .get(&owner)
                .is_none_or(|section| section.data.revision() > u64::MAX - 32)
        {
            return Err("Container transaction cannot advance revision".into());
        }
        for dz in -1..=1 {
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let neighbor = owner
                        .offset([dx, dy, dz])
                        .map_err(|error| error.to_string())?;
                    if world.journal.mesh_epoch(neighbor) > u64::MAX - 64
                        || world.journal.water_mesh_epoch(neighbor) > u64::MAX - 64
                    {
                        return Err("Container transaction mesh epoch exhausted".into());
                    }
                }
            }
        }
    }
    world
        .journal
        .preflight_inventory_removal(positions, cranks + resource_drops)
}

pub(super) fn toggle(
    position: [i64; 3],
    expected: u16,
    facing: u8,
    world: &mut ControlWorld,
) -> Result<bool, String> {
    let players: Vec<_> = world
        .bodies
        .iter()
        .map(|body| body.translation.to_array().map(f64::from))
        .collect();
    toggle_with_players(
        position,
        expected,
        facing,
        &mut world.chunks,
        &mut world.journal,
        &world.block_properties,
        &players,
    )
}

pub(super) fn toggle_with_players(
    position: [i64; 3],
    expected: u16,
    facing: u8,
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
    properties: &terra_core::block_properties::BlockPropertiesCatalog,
    players: &[[f64; 3]],
) -> Result<bool, String> {
    if matches!(
        expected,
        terra_core::plumbing::VALVE | terra_core::plumbing::FAUCET
    ) {
        return terra_world::plumbing::toggle(chunks, journal, position, expected, facing);
    }
    if !terra_world::objects::is_opening(expected) {
        return Err("Not an opening".into());
    }
    let state = journal
        .object_state(position)
        .ok_or("Missing opening state")?;
    if state.facing != facing {
        return Ok(false);
    }
    let window = terra_world::objects::is_window(expected)
        .then(|| {
            terra_world::objects::windows::group(position, expected, state.facing, |p| {
                let material = chunks.block_at(p).or_else(|| journal.block(p))?;
                Some((material, journal.object_state(p)?.facing))
            })
        })
        .filter(|group| group.width > 1 || group.height > 1);
    let door = terra_world::objects::is_door(expected)
        .then(|| {
            terra_world::objects::doors::group(position, expected, state, |p| {
                Some((
                    chunks.block_at(p).or_else(|| journal.block(p))?,
                    journal.object_state(p)?,
                ))
            })
        })
        .flatten()
        .filter(|group| group.width == 2);
    let mut cells = if let Some(group) = window {
        group
            .cells()
            .map(|p| {
                journal
                    .object_state(p)
                    .map(|state| (p, state))
                    .ok_or_else(|| "Missing connected window state".to_string())
            })
            .collect::<Result<Vec<_>, _>>()?
    } else if let Some(group) = door {
        group
            .cells()
            .map(|p| {
                journal
                    .object_state(p)
                    .map(|state| (p, state))
                    .ok_or_else(|| "Missing connected door state".to_string())
            })
            .collect::<Result<Vec<_>, _>>()?
    } else {
        vec![(position, state)]
    };
    if window.is_none()
        && door.is_none()
        && let Some(peer) = state.peer
    {
        let other = journal
            .object_state(peer)
            .ok_or("Unloaded opening partner")?;
        if chunks.block_at(peer) != Some(expected)
            || other.peer != Some(position)
            || other.facing != facing
            || other.secondary == state.secondary
        {
            return Err("Invalid opening partner".into());
        }
        cells.push((peer, other));
    }
    for (cell, _) in &cells {
        let section = terra_voxel::global_to_section(*cell).0;
        if chunks.block_at(*cell) != Some(expected)
            || journal.revision(section) > u64::MAX - 16
            || journal.appearance_epoch(section) > u64::MAX - 16
            || journal.fluid_epoch(section) > u64::MAX - 16
        {
            return Err("Opening state cannot advance".into());
        }
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    let neighbor = section.offset([dx, dy, dz]).map_err(|e| e.to_string())?;
                    if journal.mesh_epoch(neighbor) > u64::MAX - 32
                        || journal.water_mesh_epoch(neighbor) > u64::MAX - 32
                    {
                        return Err("Opening neighbor revision exhausted".into());
                    }
                }
            }
        }
    }
    let owner = if state.secondary {
        state.peer.ok_or("Missing opening anchor")?
    } else {
        position
    };
    {
        let destination = BlockObjectState {
            facing: facing ^ 4,
            secondary: false,
            ..state
        };
        let boxes: Vec<_> = if let Some(group) = door {
            let destination = group.with_open(facing < 4);
            terra_world::objects::doors::pieces()
                .into_iter()
                .filter(|p| p.solid && p.leaf.is_some())
                .map(|p| destination.world_box(p))
                .collect()
        } else if let Some(group) = window {
            terra_world::objects::windows::pieces(group.width, group.height)
                .into_iter()
                .filter(|p| p.solid && p.leaf.is_some())
                .map(|p| {
                    terra_world::objects::windows::world_box(
                        p,
                        group,
                        f64::from(u8::from(destination.facing >= 4)),
                    )
                })
                .collect()
        } else {
            properties
                .shape(expected)
                .iter()
                .filter(|b| b.hinged)
                .map(|source| terra_world::objects::opening_box(*source, owner, destination))
                .collect()
        };
        for bounds in boxes {
            if players.iter().any(|body| {
                let half = [
                    f64::from(terra_physics::character::PLAYER_RADIUS),
                    f64::from(terra_physics::character::PLAYER_HALF_HEIGHT),
                    f64::from(terra_physics::character::PLAYER_RADIUS),
                ];
                bounds.intersects(
                    std::array::from_fn(|axis| body[axis] - half[axis]),
                    std::array::from_fn(|axis| body[axis] + half[axis]),
                )
            }) {
                return Err("A player blocks this opening".into());
            }
        }
    }
    for (cell, state) in cells {
        commit_object_state(
            chunks,
            journal,
            cell,
            Some(BlockObjectState {
                facing: state.facing % 4 + if facing < 4 { 4 } else { 0 },
                ..state
            }),
        )?;
    }
    Ok(true)
}

pub(super) fn commit(plan: Plan, world: &mut ControlWorld) -> Result<bool, String> {
    if plan.edits.iter().any(|edit| {
        terra_world::objects::is_door(edit.material)
            || terra_world::objects::is_door(edit.expected_material)
    }) {
        preflight_object_revisions(&plan, world)?;
    }
    if plan.edits.len() == 1
        && let Some(edit) = plan.edits.first()
        && matches!(edit.expected_material, 161 | 162)
        && edit.material == 161
        && !world.journal.has_attached_campfire(edit.position)
        && let Some((_, Some(state))) = plan.states.iter().find(|(position, state)| {
            *position == edit.position
                && state.is_some_and(|state| terra_core::brewing::has_fire(state.facing))
        })
    {
        let fire = if edit.expected_material == 162 {
            terra_core::campfire::AttachedCampfire {
                state: world
                    .journal
                    .campfire_at(edit.position)
                    .cloned()
                    .ok_or("Missing standalone fire")?,
                inventory: world
                    .journal
                    .container_at(edit.position)
                    .cloned()
                    .unwrap_or_default(),
            }
        } else {
            terra_core::campfire::AttachedCampfire::default()
        };
        if !fire.valid() {
            return Err("Invalid existing hearth contents".into());
        }
        if edit.expected_material == 162
            && !terra_world::streaming::commit_block_edit_preserving_inventory(
                &mut world.chunks,
                &mut world.journal,
                edit,
                &world.block_properties,
            )?
        {
            return Err("Standalone hearth changed before upgrade".into());
        }
        commit_object_state(
            &mut world.chunks,
            &mut world.journal,
            edit.position,
            Some(*state),
        )?;
        if edit.expected_material == 162 {
            world
                .journal
                .set_cauldron(edit.position, Default::default())?;
        }
        world
            .journal
            .commit_attached_campfire(edit.position, fire)?;
        return Ok(true);
    }
    if plan.tint.is_some() {
        for edit in &plan.edits {
            let section = terra_voxel::global_to_section(edit.position).0;
            if world.journal.revision(section) > u64::MAX - 512
                || world.journal.appearance_epoch(section) > u64::MAX - 512
                || world.journal.mesh_epoch(section) > u64::MAX - 512
            {
                return Err("RGB object revision exhausted".into());
            }
        }
    }
    if !plan.belts.is_empty()
        || plan.edits.iter().any(|edit| {
            edit.expected_material == 148
                && edit.material == 0
                && world
                    .journal
                    .object_state(edit.position)
                    .is_some_and(|s| s.peer.is_some())
        })
    {
        let removing_endpoint = plan.belts.iter().any(|(_, owner)| owner.is_none())
            && plan
                .edits
                .first()
                .is_some_and(|e| e.expected_material != 148);
        let mut drops = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        for edit in &plan.edits {
            if removing_endpoint
                && let Some(span) = terra_world::conveyors::span_at(&world.journal, edit.position)
                && span.linked
                && seen.insert(span.controller)
            {
                drops.push((
                    span.controller,
                    terra_world::conveyors::item(span, &world.journal),
                ));
            }
            if edit.expected_material == 100 && edit.material == 0 {
                drops.push((edit.position, terra_core::ItemKind::Block(100)));
            }
        }
        let game = world.journal.gameplay();
        if game.drops.len() + drops.len() + 1 > terra_core::MAX_WORLD_DROPS
            || game.next_drop.checked_add(drops.len() as u64 + 1).is_none()
            || game.revision > u64::MAX - 64
        {
            return Err("No space for conveyor attachment drops".into());
        }
        let states = plan
            .edits
            .iter()
            .map(|edit| {
                (
                    edit.position,
                    if edit.material == 0 {
                        None
                    } else {
                        plan.states
                            .iter()
                            .find(|(p, _)| *p == edit.position)
                            .and_then(|(_, state)| *state)
                            .or_else(|| world.journal.object_state(edit.position))
                    },
                )
            })
            .collect();
        let shared = terra_world::placement::Layout {
            states,
            edits: plan.edits,
            belts: plan.belts,
        };
        let committed =
            terra_world::conveyors::commit(&mut world.chunks, &mut world.journal, &shared)?;
        if committed {
            for (position, item) in drops {
                world
                    .journal
                    .gameplay_mut()
                    .drop_one(position.map(|v| v as f64 + 0.5), item);
            }
        }
        return Ok(committed);
    }
    for (position, state) in &plan.states {
        if let Some(state) = state {
            let material = plan
                .edits
                .iter()
                .find(|edit| edit.position == *position)
                .map(|edit| edit.material)
                .or_else(|| world.journal.block(*position))
                .ok_or("Planned object state requires a recorded block")?;
            if !state.valid_for(*position, material) {
                return Err("Invalid planned object state".into());
            }
        }
    }
    preflight_inventory_drops(&plan, world, plan.resource_drops)?;
    for edit in &plan.edits {
        if world.chunks.block_at(edit.position) != Some(edit.expected_material) {
            return Err("Object transaction changed before commit".into());
        }
    }
    let removing_drive = plan.edits.iter().any(|edit| {
        (edit.expected_material == 99
            || terra_core::kinetic::is_crank_mount(edit.expected_material))
            && edit.material != edit.expected_material
    });
    for edit in &plan.edits {
        if removing_drive
            && edit.expected_material == 100
            && edit.material == 0
            && !world.journal.gameplay_mut().drop_one(
                edit.position.map(|value| value as f64 + 0.5),
                terra_core::ItemKind::Block(100),
            )
        {
            return Err("Crank drop capacity exhausted".into());
        }
    }
    let tint_cells: Vec<_> = plan
        .edits
        .iter()
        .filter(|edit| {
            edit.material != 0
                && terra_core::tints::cell_dyeable(
                    edit.material,
                    plan.states
                        .iter()
                        .find(|(position, _)| *position == edit.position)
                        .and_then(|(_, state)| *state)
                        .unwrap_or_default()
                        .facing,
                )
        })
        .map(|edit| edit.position)
        .collect();
    let inherited_tints: Vec<_> = tint_cells
        .iter()
        .filter_map(|position| world.journal.tint_at(*position).map(|rgb| (*position, rgb)))
        .collect();
    for edit in plan.edits {
        if terra_core::construction::is_slab(edit.material)
            && edit.expected_material == edit.material
        {
            continue;
        }
        if !commit_block_edit(
            &mut world.chunks,
            &mut world.journal,
            &edit,
            &world.block_properties,
        )? {
            return Err("Object transaction changed during commit".into());
        }
    }
    for (position, state) in plan.states {
        commit_object_state(&mut world.chunks, &mut world.journal, position, state)?;
        if world.journal.block(position) == Some(59) && state.is_some_and(|s| !s.secondary) {
            world.journal.set_furnace(
                position,
                terra_core::furnace::FurnaceState {
                    outlet: [position[0], position[1] + 2, position[2]],
                    ..Default::default()
                },
            )?;
        }
        if world.journal.block(position) == Some(99) && state.is_some_and(|s| !s.secondary) {
            world.journal.set_crusher(position, Default::default())?;
        }
        if world.journal.block(position) == Some(156) && state.is_some_and(|s| !s.secondary) {
            world.journal.set_mixer(position, Default::default())?;
        }
        if world.journal.block(position) == Some(161) && state.is_some_and(|s| !s.secondary) {
            world.journal.set_cauldron(position, Default::default())?;
        }
        if world.journal.block(position) == Some(162) && state.is_some_and(|s| !s.secondary) {
            world.journal.set_campfire(position, Default::default())?;
        }
        if world
            .journal
            .block(position)
            .is_some_and(terra_core::plumbing::is_part)
            && state.is_some()
            && world.journal.plumbing_at(position).is_none()
        {
            world.journal.set_plumbing(position, Default::default())?;
        }
    }
    for position in tint_cells {
        let rgb = plan.tint.or_else(|| {
            inherited_tints
                .iter()
                .find(|(cell, _)| *cell == position)
                .map(|(_, rgb)| *rgb)
        });
        if rgb.is_some() {
            world.journal.set_tint(position, rgb)?;
        }
    }
    Ok(true)
}

#[cfg(test)]
#[path = "server_furniture_domain_tests.rs"]
mod domain_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::SystemState;

    fn legacy_crusher(app: &mut App, anchor: [i64; 3], yaw: f32) {
        edit(app, anchor, 0, 99, yaw).unwrap();
        let state = app
            .world()
            .resource::<EditJournal>()
            .object_state(anchor)
            .unwrap();
        let peer = offset(anchor, state.right(), 1);
        let mut system = SystemState::<ControlWorld>::new(app.world_mut());
        let mut world = system.get_mut(app.world_mut()).unwrap();
        let (section, local) = terra_voxel::global_to_section(peer);
        world
            .chunks
            .loaded
            .get_mut(&section)
            .unwrap()
            .data
            .set_block(local, 99)
            .unwrap();
        world.journal.record(peer, 99).unwrap();
        world
            .journal
            .set_object_state(
                anchor,
                Some(BlockObjectState {
                    peer: Some(peer),
                    ..state
                }),
            )
            .unwrap();
        world
            .journal
            .set_object_state(
                peer,
                Some(BlockObjectState {
                    peer: Some(anchor),
                    secondary: true,
                    ..state
                }),
            )
            .unwrap();
    }

    #[test]
    fn mixer_is_single_cell_and_drops_its_real_pigments_and_output_once() {
        use terra_core::{Inventory, ItemKind, ItemStack, beds::DyeColor, mixer};
        for facing in 0..4 {
            let mut app = app();
            let anchor = [4, 1, 8];
            assert!(
                edit(
                    &mut app,
                    anchor,
                    0,
                    mixer::MATERIAL,
                    facing as f32 * std::f32::consts::FRAC_PI_2
                )
                .unwrap()
            );
            let state = app
                .world()
                .resource::<EditJournal>()
                .object_state(anchor)
                .unwrap();
            assert!(state.peer.is_none() && !state.secondary && state.facing < 4);
            assert!(
                app.world()
                    .resource::<EditJournal>()
                    .mixer_at(anchor)
                    .is_some()
            );
            let crank = mount_mixer_crank(&mut app, anchor);
            let input = ItemStack {
                item: ItemKind::Dye(DyeColor::ALL[0]),
                count: 5,
            };
            let output = ItemStack {
                item: ItemKind::Dye(DyeColor::Rgb([17, 95, 213])),
                count: 7,
            };
            let mut inventory = Inventory::default();
            inventory.slots.resize(mixer::SLOTS, ItemStack::EMPTY);
            inventory.slots[0] = input;
            inventory.slots[mixer::OUTPUT] = output;
            app.world_mut()
                .resource_mut::<EditJournal>()
                .set_container(anchor, inventory)
                .unwrap();
            assert!(edit(&mut app, anchor, mixer::MATERIAL, 0, 0.0).unwrap());
            let journal = app.world().resource::<EditJournal>();
            assert!(journal.mixer_at(anchor).is_none());
            assert!(journal.container_at(anchor).is_none());
            assert_eq!(
                app.world().resource::<ChunkStore>().block_at(crank),
                Some(0)
            );
            assert_eq!(
                journal
                    .gameplay()
                    .drops
                    .iter()
                    .filter(|drop| drop.stack.item == ItemKind::Block(100))
                    .count(),
                1
            );
            assert_eq!(
                journal
                    .gameplay()
                    .drops
                    .iter()
                    .filter(|drop| drop.stack == input)
                    .count(),
                1
            );
            assert_eq!(
                journal
                    .gameplay()
                    .drops
                    .iter()
                    .filter(|drop| drop.stack == output)
                    .count(),
                1
            );
            assert!(journal.gameplay().valid());
        }
    }

    fn mount_mixer_crank(app: &mut App, anchor: [i64; 3]) -> [i64; 3] {
        let mut system = SystemState::<ControlWorld>::new(app.world_mut());
        let mut world = system.get_mut(app.world_mut()).unwrap();
        let state = world.journal.object_state(anchor).unwrap();
        let position = offset(anchor, state.right(), 1);
        let command = EditCommand {
            request: 0,
            revision: 0,
            look: [f32::from(state.facing) * std::f32::consts::FRAC_PI_2, 0.0],
            source: None,
            action: EditAction::Block {
                position,
                expected: 0,
                material: 100,
            },
        };
        let plan = plan_crank(command, &world, anchor).unwrap().unwrap();
        assert!(commit(plan, &mut world).unwrap());
        assert_eq!(world.journal.crank_anchor(position), Some(anchor));
        assert_eq!(
            world.journal.object_state(position).unwrap().facing,
            state.facing
        );
        position
    }

    #[test]
    fn mixer_removal_reserves_contents_block_and_mounted_crank_before_mutation() {
        use terra_core::{Inventory, ItemKind, ItemStack, beds::DyeColor, mixer};
        for facing in 0..4 {
            let mut app = app();
            let anchor = [4, 1, 8];
            edit(
                &mut app,
                anchor,
                0,
                mixer::MATERIAL,
                facing as f32 * std::f32::consts::FRAC_PI_2,
            )
            .unwrap();
            let crank = mount_mixer_crank(&mut app, anchor);
            let inventory = Inventory {
                slots: vec![ItemStack {
                    item: ItemKind::Dye(DyeColor::ALL[0]),
                    count: 5,
                }],
                ..Default::default()
            };
            let before = {
                let mut journal = app.world_mut().resource_mut::<EditJournal>();
                journal.set_container(anchor, inventory.clone()).unwrap();
                for _ in 0..terra_core::MAX_WORLD_DROPS - 2 {
                    assert!(
                        journal
                            .gameplay_mut()
                            .drop_one([0.5, 2.5, 0.5], ItemKind::Block(3))
                    );
                }
                journal.gameplay().clone()
            };
            assert!(edit(&mut app, anchor, mixer::MATERIAL, 0, 0.0).is_err());
            let journal = app.world().resource::<EditJournal>();
            assert_eq!(journal.gameplay(), &before);
            assert_eq!(journal.container_at(anchor), Some(&inventory));
            assert!(journal.mixer_at(anchor).is_some());
            assert_eq!(
                app.world().resource::<ChunkStore>().block_at(crank),
                Some(100)
            );
            app.world_mut()
                .resource_mut::<EditJournal>()
                .gameplay_mut()
                .drops
                .pop();
            assert!(edit(&mut app, anchor, mixer::MATERIAL, 0, 0.0).unwrap());
            let journal = app.world().resource::<EditJournal>();
            assert!(journal.mixer_at(anchor).is_none());
            assert!(journal.container_at(anchor).is_none());
            assert_eq!(
                journal.gameplay().drops.len(),
                terra_core::MAX_WORLD_DROPS - 1
            );
            assert_eq!(
                journal
                    .gameplay()
                    .drops
                    .iter()
                    .filter(|drop| drop.stack == inventory.slots[0])
                    .count(),
                1
            );
            assert_eq!(
                journal
                    .gameplay()
                    .drops
                    .iter()
                    .filter(|drop| drop.stack.item == ItemKind::Block(100))
                    .count(),
                1
            );
            assert_eq!(
                app.world().resource::<ChunkStore>().block_at(crank),
                Some(0)
            );
            assert_eq!(
                app.world().resource::<ChunkStore>().block_at(anchor),
                Some(0)
            );
            assert!(journal.gameplay().valid());
        }
    }

    #[test]
    fn new_crusher_uses_one_cell_and_rotates_its_adjacent_crank_without_losing_inventory() {
        for facing in 0..4 {
            let mut app = app();
            let anchor = [4, 1, 8];
            let yaw = facing as f32 * std::f32::consts::FRAC_PI_2;
            edit(&mut app, anchor, 0, 99, yaw).unwrap();
            let state = app
                .world()
                .resource::<EditJournal>()
                .object_state(anchor)
                .unwrap();
            assert!(state.peer.is_none() && !state.secondary);
            let crank = offset(anchor, state.right(), 1);
            assert_eq!(
                app.world().resource::<ChunkStore>().block_at(crank),
                Some(0)
            );
            edit(&mut app, crank, 0, 100, yaw).unwrap();
            let mut system = SystemState::<ControlWorld>::new(app.world_mut());
            let mut world = system.get_mut(app.world_mut()).unwrap();
            let inventory = terra_core::Inventory {
                slots: vec![terra_core::ItemStack {
                    item: terra_core::crusher::recipes()[0].input,
                    count: 3,
                }],
                ..Default::default()
            };
            world
                .journal
                .set_container(anchor, inventory.clone())
                .unwrap();
            let plan =
                terra_world::wrench::propose(&world.chunks, &world.journal, anchor, false).unwrap();
            terra_world::wrench::commit(&mut world.chunks, &mut world.journal, &plan).unwrap();
            assert_eq!(world.journal.container_at(anchor), Some(&inventory));
            let next = world.journal.crusher_crank_position(anchor).unwrap();
            assert_eq!(world.journal.crank_anchor(next), Some(anchor));
            assert_eq!(world.chunks.block_at(crank), Some(0));
            assert!(super::super::crushers::loaded(
                anchor,
                &world.chunks,
                &world.journal
            ));
        }
    }
    #[test]
    fn conveyor_endpoint_removal_counts_connector_drops_not_every_owned_segment() {
        for free in [1, 2] {
            let mut app = app();
            let start = [5, 3, 2];
            let end = [5, 3, 12];
            edit(&mut app, start, 0, 144, 0.0).unwrap();
            edit(&mut app, end, 0, 144, 0.0).unwrap();
            let mut system = SystemState::<ControlWorld>::new(app.world_mut());
            let mut world = system.get_mut(app.world_mut()).unwrap();
            let belt = terra_world::conveyors::layout(
                &world.chunks,
                &world.journal,
                start,
                end,
                terra_core::ItemKind::Block(148),
            )
            .unwrap();
            let span = terra_world::conveyors::proposed_span(&belt).unwrap();
            terra_world::conveyors::commit(&mut world.chunks, &mut world.journal, &belt).unwrap();
            for _ in 0..terra_core::MAX_WORLD_DROPS - free {
                assert!(
                    world
                        .journal
                        .gameplay_mut()
                        .drop_one([1.5, 3.5, 1.5], terra_core::ItemKind::Block(3))
                );
            }
            let command = EditCommand {
                request: 1,
                revision: 0,
                look: [0.0; 2],
                source: None,
                action: EditAction::Block {
                    position: start,
                    expected: 144,
                    material: 0,
                },
            };
            let removal = plan(command, &world).unwrap().unwrap();
            let result = commit(removal, &mut world);
            if free == 1 {
                assert!(result.is_err());
                assert_eq!(world.chunks.block_at(start), Some(144));
                assert!(span.cells().all(|p| world.chunks.block_at(p) == Some(148)));
                assert_eq!(
                    world.journal.gameplay().drops.len(),
                    terra_core::MAX_WORLD_DROPS - 1
                );
            } else {
                assert_eq!(result, Ok(true));
                assert_eq!(world.chunks.block_at(start), Some(0));
                assert_eq!(world.chunks.block_at(end), Some(144));
                assert!(span.cells().all(|p| world.chunks.block_at(p) == Some(0)));
                assert_eq!(
                    world.journal.gameplay().drops.len(),
                    terra_core::MAX_WORLD_DROPS - 1
                );
                assert_eq!(
                    world.journal.gameplay().drops.last().unwrap().stack.item,
                    terra_core::ItemKind::Block(148)
                );
                assert!(world.journal.gameplay_mut().drop_one(
                    start.map(|v| v as f64 + 0.5),
                    terra_core::ItemKind::Block(144)
                ));
                assert_eq!(
                    world.journal.gameplay().drops.len(),
                    terra_core::MAX_WORLD_DROPS
                );
            }
        }
    }
    #[test]
    fn hand_crank_mounts_without_floor_and_detaches_without_corrupting_the_machine_pair() {
        for facing in 0..4 {
            let mut app = app();
            let anchor = [4, 1, 8];
            let yaw = facing as f32 * std::f32::consts::FRAC_PI_2;
            edit(&mut app, anchor, 0, 99, yaw).unwrap();
            let crank = app
                .world()
                .resource::<EditJournal>()
                .crusher_crank_position(anchor)
                .unwrap();
            let below = [crank[0], 0, crank[2]];
            let (owner, local) = terra_voxel::global_to_section(below);
            app.world_mut()
                .resource_mut::<ChunkStore>()
                .loaded
                .get_mut(&owner)
                .unwrap()
                .data
                .set_block(local, 0)
                .unwrap();
            app.world_mut()
                .resource_mut::<EditJournal>()
                .record(below, 0)
                .unwrap();
            edit(&mut app, crank, 0, 100, yaw).unwrap();
            assert_eq!(
                app.world().resource::<EditJournal>().crank_anchor(crank),
                Some(anchor)
            );
            edit(&mut app, crank, 100, 0, yaw).unwrap();
            assert!(app.world().resource::<EditJournal>().valid_crusher(anchor));
            assert_eq!(
                app.world().resource::<ChunkStore>().block_at(crank),
                Some(0)
            );
            edit(&mut app, crank, 0, 100, yaw).unwrap();
            app.world_mut()
                .resource_mut::<EditJournal>()
                .gameplay_mut()
                .next_drop = u64::MAX;
            assert!(edit(&mut app, anchor, 99, 0, yaw).is_err());
            assert_eq!(
                app.world().resource::<ChunkStore>().block_at(crank),
                Some(100)
            );
            assert!(app.world().resource::<EditJournal>().valid_crusher(anchor));
            app.world_mut()
                .resource_mut::<EditJournal>()
                .gameplay_mut()
                .next_drop = 1;
            edit(&mut app, anchor, 99, 0, yaw).unwrap();
            let journal = app.world().resource::<EditJournal>();
            assert_eq!(
                app.world().resource::<ChunkStore>().block_at(crank),
                Some(0)
            );
            assert_eq!(
                journal
                    .gameplay()
                    .drops
                    .iter()
                    .filter(|drop| drop.stack.item == terra_core::ItemKind::Block(100))
                    .map(|drop| drop.stack.count)
                    .sum::<u16>(),
                1
            );
        }
    }
    #[test]
    fn unloading_a_mounted_crank_prevents_partial_machine_removal_and_content_loss() {
        for anchor in [[14, 1, 8], [-2, 1, 8]] {
            for secondary in [false, true] {
                let mut app = app();
                legacy_crusher(&mut app, anchor, 0.0);
                let journal = app.world().resource::<EditJournal>();
                let peer = journal.object_state(anchor).unwrap().peer.unwrap();
                let crank = journal.crusher_crank_position(anchor).unwrap();
                edit(&mut app, crank, 0, 100, 0.0).unwrap();
                let item = terra_core::ItemKind::Mineral(terra_core::MineralItem::Coal);
                let mut inventory = terra_core::Inventory {
                    slots: vec![terra_core::ItemStack { item, count: 3 }],
                    ..Default::default()
                };
                let mut processing = terra_core::crusher::CrusherState::default();
                assert!(terra_core::crusher::advance(
                    &mut processing,
                    &mut inventory,
                    1.0
                ));
                app.world_mut()
                    .resource_mut::<EditJournal>()
                    .commit_crusher(anchor, processing.clone(), inventory.clone())
                    .unwrap();
                let section = terra_voxel::global_to_section(crank).0;
                assert_ne!(section, terra_voxel::global_to_section(peer).0);
                let unloaded = app
                    .world_mut()
                    .resource_mut::<ChunkStore>()
                    .loaded
                    .remove(&section)
                    .unwrap();
                let journal = app.world().resource::<EditJournal>();
                let revision = journal.gameplay().revision;
                let next_drop = journal.gameplay().next_drop;
                let delta = journal.section_delta(section);
                let selected = if secondary { peer } else { anchor };
                assert!(edit(&mut app, selected, 99, 0, 0.0).is_err());
                let journal = app.world().resource::<EditJournal>();
                assert!(journal.valid_crusher(anchor));
                assert_eq!(journal.crank_anchor(crank), Some(anchor));
                assert_eq!(journal.crusher_at(anchor), Some(&processing));
                assert_eq!(journal.container_at(anchor), Some(&inventory));
                assert_eq!(journal.gameplay().revision, revision);
                assert_eq!(journal.gameplay().next_drop, next_drop);
                assert!(journal.gameplay().drops.is_empty());
                assert_eq!(journal.section_delta(section), delta);
                for position in [anchor, peer] {
                    assert_eq!(
                        app.world().resource::<ChunkStore>().block_at(position),
                        Some(99)
                    );
                }
                app.world_mut()
                    .resource_mut::<ChunkStore>()
                    .loaded
                    .insert(section, unloaded);
                edit(&mut app, selected, 99, 0, 0.0).unwrap();
                let journal = app.world().resource::<EditJournal>();
                assert!(journal.crusher_at(anchor).is_none());
                assert!(journal.container_at(anchor).is_none());
                for position in [anchor, peer, crank] {
                    assert_eq!(
                        app.world().resource::<ChunkStore>().block_at(position),
                        Some(0)
                    );
                }
                for (item, count) in [(item, 3), (terra_core::ItemKind::Block(100), 1)] {
                    assert_eq!(
                        journal
                            .gameplay()
                            .drops
                            .iter()
                            .filter(|drop| drop.stack.item == item)
                            .map(|drop| drop.stack.count)
                            .sum::<u16>(),
                        count
                    );
                }
            }
        }
    }
    #[test]
    fn crusher_pairs_rotate_and_breaking_either_cell_drops_contents_once() {
        for facing in 0..4 {
            for secondary in [false, true] {
                let mut app = app();
                let anchor = [4, 1, 8];
                let yaw = facing as f32 * std::f32::consts::FRAC_PI_2;
                legacy_crusher(&mut app, anchor, yaw);
                let peer = app
                    .world()
                    .resource::<EditJournal>()
                    .object_state(anchor)
                    .unwrap()
                    .peer
                    .unwrap();
                let state = app
                    .world()
                    .resource::<EditJournal>()
                    .object_state(anchor)
                    .unwrap();
                assert_eq!(peer, offset(anchor, state.right(), 1));
                let inventory = terra_core::Inventory {
                    slots: vec![terra_core::ItemStack {
                        item: terra_core::ItemKind::Mineral(terra_core::MineralItem::Coal),
                        count: 3,
                    }],
                    ..Default::default()
                };
                app.world_mut()
                    .resource_mut::<EditJournal>()
                    .set_container(anchor, inventory)
                    .unwrap();
                edit(&mut app, if secondary { peer } else { anchor }, 99, 0, 0.0).unwrap();
                let journal = app.world().resource::<EditJournal>();
                assert!(
                    journal.crusher_at(anchor).is_none() && journal.container_at(anchor).is_none()
                );
                assert_eq!(
                    journal
                        .gameplay()
                        .drops
                        .iter()
                        .map(|drop| drop.stack.count)
                        .sum::<u16>(),
                    3
                );
                for cell in [anchor, peer] {
                    assert_eq!(app.world().resource::<ChunkStore>().block_at(cell), Some(0));
                }
            }
        }
    }
    pub(super) fn app() -> App {
        let mut app = App::new();
        let config = GameConfig::default();
        app.insert_resource(terra_world::streaming::WorldTerrain(
            terra_world::TerraGenerator::new(config.world.clone()).unwrap(),
        ))
        .insert_resource(terra_core::WorldClock::new(config.world.time))
        .init_resource::<terra_core::WorldWeather>()
        .insert_resource(config)
        .init_resource::<terra_core::crafting::RecipeBook>()
        .init_resource::<terra_core::block_properties::BlockPropertiesCatalog>()
        .init_resource::<ChunkStore>()
        .init_resource::<EditJournal>()
        .init_resource::<ConnectedProfiles>()
        .init_resource::<riding::Mounts>();
        for x in -1..=1 {
            let position = terra_voxel::SectionPos::new(x, 0, 0);
            let mut data = terra_voxel::BlockSection::empty(position).unwrap();
            for x in 0..16 {
                for z in 0..16 {
                    data.set_block([x, 0, z], 3).unwrap();
                }
            }
            app.world_mut().resource_mut::<ChunkStore>().loaded.insert(
                position,
                terra_world::streaming::LoadedSection {
                    data,
                    mesh: None,
                    mesh_epoch: 0,
                    water_epoch: 0,
                    water: None,
                    props: Vec::new(),
                },
            );
        }
        app
    }
    pub(super) fn edit(
        app: &mut App,
        position: [i64; 3],
        expected: u16,
        material: u16,
        yaw: f32,
    ) -> Result<bool, String> {
        item_edit(app, position, expected, material, yaw, None)
    }
    fn item_edit(
        app: &mut App,
        position: [i64; 3],
        expected: u16,
        material: u16,
        yaw: f32,
        item: Option<terra_core::ItemKind>,
    ) -> Result<bool, String> {
        let mut system = SystemState::<ControlWorld>::new(app.world_mut());
        let mut world = system.get_mut(app.world_mut()).unwrap();
        let command = EditCommand {
            request: 0,
            revision: 0,
            look: [yaw, 0.0],
            source: None,
            action: EditAction::Block {
                position,
                expected,
                material,
            },
        };
        let plan = plan(command, &world)?.ok_or("Missing furniture plan")?;
        commit(plan.with_item(item), &mut world)
    }
    pub(super) fn wooden_edit(
        app: &mut App,
        position: [i64; 3],
        material: u16,
        wood: terra_core::WoodSpecies,
    ) {
        let mut system = SystemState::<ControlWorld>::new(app.world_mut());
        let mut world = system.get_mut(app.world_mut()).unwrap();
        let command = EditCommand {
            request: 0,
            revision: 0,
            look: [0.0; 2],
            source: None,
            action: EditAction::Block {
                position,
                expected: 0,
                material,
            },
        };
        let plan = plan(command, &world)
            .unwrap()
            .unwrap()
            .with_wood(position, wood);
        commit(plan, &mut world).unwrap();
    }
    fn creative_placement(
        app: &mut App,
        position: [i64; 3],
        material: u16,
        item: terra_core::ItemKind,
    ) -> EditOutcome {
        let uuid = terra_core::PlayerUuid::from_u128(1);
        let mut inventory = terra_core::Inventory::default();
        assert_eq!(inventory.insert(item, 1), 0);
        let source = InventorySource {
            slot: 0,
            revision: inventory.revision,
        };
        app.world_mut()
            .resource_mut::<EditJournal>()
            .gameplay_mut()
            .inventories
            .insert(uuid, inventory);
        let body_position =
            Vec3::from_array(position.map(|value| value as f32)) + Vec3::new(0.5, 1.0, 3.5);
        let body = app
            .world_mut()
            .spawn((
                ServerCharacter {
                    link: Entity::PLACEHOLDER,
                },
                Transform::from_translation(body_position),
            ))
            .id();
        let direction = (Vec3::from_array(position.map(|value| value as f32))
            + Vec3::new(0.5, 0.0, 0.5)
            - body_position
            - Vec3::Y * terra_physics::character::PLAYER_EYE_OFFSET)
            .normalize();
        let look = [(-direction.x).atan2(-direction.z), direction.y.asin()];
        let now = Instant::now();
        let peer = Peer {
            farming: Default::default(),
            crank: Default::default(),
            chest_cursor: None,
            creature_action_at: now,
            container: None,
            held_slot: Some(source.slot),
            mode: Some(terra_core::GameMode::Creative),
            generation: 0,
            chat: Default::default(),
            uuid,
            gender: Default::default(),
            mining: None,
            consumption: None,
            gameplay_sent: None,
            permissions: terra_core::PlayerPermissions {
                build: true,
                game_mode: false,
                flight: false,
                environment: false,
            },
            id: PlayerId(1),
            transport_id: 1,
            body,
            accepted: true,
            synced: true,
            view: 1,
            known: Default::default(),
            inputs: Default::default(),
            received: 0,
            ack: 0,
            look,
            last_input: now,
            joined: now,
            last_request: 0,
            edits_left: 20.0,
            rate_time: now,
            spawned: true,
            restored_spawn: false,
            restore_location: None,
            last_movement: Default::default(),
            vitals: Default::default(),
            equipment: Default::default(),
            survival: Default::default(),
            stats_sent: None,
        };
        let mut system = SystemState::<ControlWorld>::new(app.world_mut());
        let mut world = system.get_mut(app.world_mut()).unwrap();
        let command = EditCommand {
            request: 0,
            revision: world
                .journal
                .revision(terra_voxel::global_to_section(position).0),
            look,
            source: None,
            action: EditAction::Block {
                position,
                expected: 0,
                material,
            },
        };
        super::super::validation::apply(command, &peer, &mut world, false)
    }
    #[test]
    fn dyed_bed_item_only_colors_new_cells_with_the_same_bed_material() {
        let material = 71;
        let edits: Vec<_> = [(0, 71), (27, 71), (0, 72), (0, 59), (0, 74), (71, 0)]
            .into_iter()
            .enumerate()
            .map(|(index, (expected_material, material))| BlockEditRequest {
                position: [index as i64, 1, 8],
                expected_material,
                material,
            })
            .collect();
        let states = edits
            .iter()
            .map(|edit| (edit.position, Some(BlockObjectState::default())))
            .chain([([8, 1, 8], Some(BlockObjectState::default()))])
            .collect();
        let plan = Plan {
            edits,
            states,
            tint: None,
            belts: Vec::new(),
            resource_drops: 1,
        }
        .with_item(Some(terra_core::ItemKind::DyedBed {
            material,
            color: terra_core::beds::DyeColor::Purple,
        }));
        for (index, (_, state)) in plan.states.iter().enumerate() {
            assert_eq!(
                state.unwrap().facing,
                if index < 2 {
                    terra_core::beds::dyed_facing(0, terra_core::beds::DyeColor::Purple)
                } else {
                    0
                }
            );
        }
    }
    #[test]
    fn creative_catalog_placement_ignores_an_unrelated_held_dyed_bed_without_a_source() {
        let item = terra_core::ItemKind::DyedBed {
            material: 71,
            color: terra_core::beds::DyeColor::Purple,
        };
        for material in [59, 74, 72] {
            let mut app = app();
            app.world_mut()
                .resource_mut::<GameConfig>()
                .server
                .game_mode = terra_core::GameMode::Creative;
            let position = [3, 1, 8];
            assert_eq!(
                creative_placement(&mut app, position, material, item),
                EditOutcome::Applied
            );
            let journal = app.world().resource::<EditJournal>();
            assert_eq!(
                journal.gameplay().inventories[&terra_core::PlayerUuid::from_u128(1)].count(item),
                1
            );
            let state = journal.object_state(position).unwrap();
            assert_eq!(
                state.facing,
                if matches!(material, 74 | 72) { 2 } else { 0 }
            );
            assert!(state.valid_for(position, material));
            let cells = std::iter::once(position)
                .chain(state.peer)
                .collect::<Vec<_>>();
            for cell in &cells {
                assert_eq!(
                    app.world().resource::<ChunkStore>().block_at(*cell),
                    Some(material)
                );
                assert_eq!(journal.object_state(*cell).unwrap().facing, state.facing);
                assert!(
                    journal
                        .object_state(*cell)
                        .unwrap()
                        .valid_for(*cell, material)
                );
            }
            assert_eq!(journal.furnace_at(position).is_some(), material == 59);
            assert!(item_edit(&mut app, position, material, 0, 0.0, Some(item)).unwrap());
            for cell in cells {
                assert_eq!(app.world().resource::<ChunkStore>().block_at(cell), Some(0));
                let journal = app.world().resource::<EditJournal>();
                assert!(journal.object_state(cell).is_none());
                assert!(journal.furnace_at(cell).is_none());
            }
        }
    }
    #[test]
    fn invalid_planned_states_do_not_partially_place_furnaces_or_stairs() {
        for material in [59, 74] {
            let mut app = app();
            let position = [3, 1, 8];
            let before = furnace_snapshot(&app, position);
            {
                let mut system = SystemState::<ControlWorld>::new(app.world_mut());
                let mut world = system.get_mut(app.world_mut()).unwrap();
                let command = EditCommand {
                    request: 0,
                    revision: 0,
                    look: [0.0; 2],
                    source: None,
                    action: EditAction::Block {
                        position,
                        expected: 0,
                        material,
                    },
                };
                let mut plan = plan(command, &world).unwrap().unwrap();
                plan.states[0].1.as_mut().unwrap().facing =
                    terra_core::beds::dyed_facing(0, terra_core::beds::DyeColor::Purple);
                assert!(commit(plan, &mut world).is_err());
            }
            assert_eq!(furnace_snapshot(&app, position), before);
            for cell in [position, [3, 2, 8]] {
                assert_eq!(app.world().resource::<ChunkStore>().block_at(cell), Some(0));
                assert!(
                    app.world()
                        .resource::<EditJournal>()
                        .object_state(cell)
                        .is_none()
                );
            }
        }
    }
    #[test]
    fn dyed_bed_placement_keeps_both_halves_colored_and_oriented() {
        for material in 71..=73 {
            for color in terra_core::beds::DyeColor::ALL {
                let mut app = app();
                let position = [3, 1, 8];
                let facing = 3;
                let item = terra_core::ItemKind::DyedBed { material, color };
                assert!(
                    item_edit(
                        &mut app,
                        position,
                        0,
                        material,
                        std::f32::consts::FRAC_PI_2,
                        Some(item),
                    )
                    .unwrap()
                );
                let journal = app.world().resource::<EditJournal>();
                let state = journal.object_state(position).unwrap();
                for cell in [position, state.peer.unwrap()] {
                    let state = journal.object_state(cell).unwrap();
                    assert!(state.valid_for(cell, material));
                    assert_eq!(state.facing % 4, facing);
                    assert_eq!(terra_core::beds::bed_color(material, state.facing), color);
                }
            }
        }
    }
    #[test]
    fn bed_pillows_extend_away_from_the_player_view_in_all_four_directions() {
        for yaw in [-5.8_f32, -3.2, -1.6, -0.2, 0.2, 1.6, 3.2, 5.8] {
            let mut app = app();
            let p = [3, 1, 8];
            edit(&mut app, p, 0, 71, yaw).unwrap();
            let state = app
                .world()
                .resource::<EditJournal>()
                .object_state(p)
                .unwrap();
            let peer = state.peer.unwrap();
            let direction = bevy::math::Vec3::new(-yaw.sin(), 0.0, -yaw.cos());
            let offset =
                bevy::math::Vec3::from_array(std::array::from_fn(|i| (peer[i] - p[i]) as f32));
            assert!(offset.dot(direction) > 0.7, "yaw {yaw}: {p:?} -> {peer:?}");
            assert!(state.valid_for(p, 71));
        }
    }
    #[test]
    fn beds_keep_both_cells_and_blanket_when_broken_from_either_half() {
        for material in 71..=73 {
            for facing in 0..4 {
                for secondary in [false, true] {
                    let mut app = app();
                    let origin = [3, 1, 8];
                    edit(
                        &mut app,
                        origin,
                        0,
                        material,
                        facing as f32 * std::f32::consts::FRAC_PI_2,
                    )
                    .unwrap();
                    let state = app
                        .world()
                        .resource::<EditJournal>()
                        .object_state(origin)
                        .unwrap();
                    let peer = state.peer.unwrap();
                    assert_eq!(origin[1], peer[1]);
                    assert!(state.valid_for(origin, material));
                    for cell in [origin, peer] {
                        let old = app
                            .world()
                            .resource::<EditJournal>()
                            .object_state(cell)
                            .unwrap();
                        app.world_mut()
                            .resource_mut::<EditJournal>()
                            .set_object_state(
                                cell,
                                Some(BlockObjectState {
                                    facing: terra_core::beds::dyed_facing(
                                        old.facing,
                                        terra_core::beds::DyeColor::Purple,
                                    ),
                                    ..old
                                }),
                            )
                            .unwrap();
                    }
                    let cell = if secondary { peer } else { origin };
                    edit(&mut app, cell, material, 0, 0.0).unwrap();
                    for cell in [origin, peer] {
                        assert_eq!(app.world().resource::<ChunkStore>().block_at(cell), Some(0));
                        assert!(
                            app.world()
                                .resource::<EditJournal>()
                                .object_state(cell)
                                .is_none()
                        );
                    }
                }
            }
        }
    }
    #[test]
    fn beds_do_not_float_above_half_slabs_stairs_or_fence_posts() {
        for material in [74, 82, 90] {
            let mut app = app();
            edit(&mut app, [3, 0, 8], 3, material, 0.0).unwrap();
            edit(&mut app, [3, 0, 7], 3, material, 0.0).unwrap();
            assert!(edit(&mut app, [3, 1, 8], 0, 71, 0.0).is_err());
            for position in [[3, 1, 8], [3, 1, 7]] {
                assert_eq!(
                    app.world().resource::<ChunkStore>().block_at(position),
                    Some(0)
                );
                assert!(
                    app.world()
                        .resource::<EditJournal>()
                        .object_state(position)
                        .is_none()
                );
            }
        }
    }

    #[test]
    fn upper_and_merged_slabs_support_beds_but_side_and_lower_halves_do_not() {
        for material in 82..=89 {
            for facing in [0, 4, 5, 6, 7, 8, 9] {
                let mut app = app();
                for position in [[3, 0, 8], [3, 0, 7]] {
                    edit(&mut app, position, 3, material, 0.0).unwrap();
                    app.world_mut()
                        .resource_mut::<EditJournal>()
                        .set_object_state(
                            position,
                            Some(BlockObjectState {
                                facing,
                                ..Default::default()
                            }),
                        )
                        .unwrap();
                }
                assert_eq!(
                    edit(&mut app, [3, 1, 8], 0, 71, 0.0).is_ok(),
                    matches!(facing, 4 | 9),
                    "material={material} facing={facing}"
                );
            }
        }
    }

    #[test]
    fn wool_construction_authority_keeps_every_pose_and_each_ray_selected_mixed_half() {
        fn apply(
            app: &mut App,
            position: [i64; 3],
            expected: u16,
            material: u16,
            facing: Option<u8>,
            half: Option<terra_world::slabs::HalfRemoval>,
        ) {
            let mut system = SystemState::<ControlWorld>::new(app.world_mut());
            let mut world = system.get_mut(app.world_mut()).unwrap();
            let command = EditCommand {
                request: 0,
                revision: 0,
                look: [0.0; 2],
                source: None,
                action: EditAction::Block {
                    position,
                    expected,
                    material,
                },
            };
            let mut proposed = plan(command, &world)
                .unwrap()
                .expect("new wool shapes must use the authoritative furniture transaction")
                .with_construction_facing(facing);
            if let Some(half) = half {
                proposed = proposed.with_slab_half(
                    position,
                    half,
                    world.journal.object_state(position).unwrap(),
                );
            }
            assert!(commit(proposed, &mut world).unwrap());
        }

        let position = [-1, 1, 8];
        for facing in 0..24 {
            let mut app = app();
            apply(&mut app, position, 0, 104, Some(facing), None);
            let journal = app.world().resource::<EditJournal>();
            assert_eq!(journal.block(position), Some(104));
            assert_eq!(journal.object_state(position).unwrap().facing, facing);
            assert_eq!(
                app.world().resource::<ChunkStore>().block_at(position),
                Some(104)
            );
        }
        for (first, second) in [(105, 82), (105, 85), (82, 105), (85, 105)] {
            for facing in [0, 4, 5, 6, 7, 8] {
                for selected in 0..2 {
                    let mut app = app();
                    apply(&mut app, position, 0, first, Some(facing), None);
                    let combined =
                        terra_core::construction::merged_slab(first, facing, second).unwrap();
                    if second == 105 {
                        assert!((58..=63).contains(&combined));
                    }
                    apply(&mut app, position, first, second, Some(combined), None);
                    let halves = terra_core::construction::slab_halves(first, combined).unwrap();
                    let bounds = terra_core::construction::slab_bounds(halves[selected].1);
                    let axis = (0..3)
                        .find(|&i| bounds.maximum[i] - bounds.minimum[i] == 0.5)
                        .unwrap();
                    let sign = if bounds.minimum[axis] == 0.5 {
                        1.0
                    } else {
                        -1.0
                    };
                    let mut origin = position.map(|v| v as f64 + 0.5);
                    origin[axis] += sign * 3.0;
                    let mut direction = [0.0; 3];
                    direction[axis] = -sign;
                    let half = {
                        let chunks = app.world().resource::<ChunkStore>();
                        let journal = app.world().resource::<EditJournal>();
                        assert_eq!(chunks.block_at(position), Some(first));
                        assert_eq!(journal.block(position), Some(first));
                        assert_eq!(journal.object_state(position).unwrap().facing, combined);
                        terra_world::slabs::targeted_half(
                            chunks,
                            journal,
                            position,
                            (origin, direction),
                        )
                        .unwrap()
                    };
                    assert_eq!(half.selected as usize, selected);
                    assert_eq!((half.removed, half.removed_facing), halves[selected]);
                    assert_eq!(
                        (half.remaining, half.remaining_facing),
                        halves[1 - selected]
                    );
                    apply(&mut app, position, first, 0, None, Some(half));
                    let journal = app.world().resource::<EditJournal>();
                    assert_eq!(journal.block(position), Some(half.remaining));
                    let saved = journal.object_state(position).unwrap();
                    assert_eq!(saved.facing, half.remaining_facing);
                    assert!(saved.valid_for(position, half.remaining));
                    assert_eq!(
                        app.world().resource::<ChunkStore>().block_at(position),
                        Some(half.remaining)
                    );
                    let mut actual_boxes = Vec::new();
                    assert!(terra_world::interaction_shapes::visit_boxes(
                        app.world().resource::<ChunkStore>(),
                        journal,
                        &Default::default(),
                        position,
                        |bounds| actual_boxes.push(bounds),
                    ));
                    assert_eq!(
                        actual_boxes,
                        terra_core::construction::placed_boxes(
                            half.remaining,
                            half.remaining_facing,
                            [None; 6],
                        )
                        .into_iter()
                        .map(|mut b| {
                            for (axis, value) in position.iter().enumerate() {
                                b.minimum[axis] += *value as f64;
                                b.maximum[axis] += *value as f64;
                            }
                            b
                        })
                        .collect::<Vec<_>>()
                    );
                }
            }
        }
    }

    #[test]
    fn beds_reject_a_blocked_second_cell_without_partial_placement() {
        let mut app = app();
        let position = [3, 1, 8];
        let other = [3, 1, 7];
        edit(&mut app, other, 0, 29, 0.0).unwrap();
        assert!(edit(&mut app, position, 0, 71, 0.0).is_err());
        assert_eq!(
            app.world().resource::<ChunkStore>().block_at(position),
            Some(0)
        );
        assert!(
            app.world()
                .resource::<EditJournal>()
                .object_state(position)
                .is_none()
        );
    }
    #[test]
    fn bench_halves_keep_crafted_species_and_chest_splitting_keeps_each_item_species() {
        use terra_core::WoodSpecies;
        let mut app = app();
        let p = [-1, 1, 8];
        wooden_edit(&mut app, p, 28, WoodSpecies::Spruce);
        let journal = app.world().resource::<EditJournal>();
        let state = journal.object_state(p).unwrap();
        assert_eq!(state.wood, WoodSpecies::Spruce);
        assert_eq!(
            journal.object_state(state.peer.unwrap()).unwrap().wood,
            WoodSpecies::Spruce
        );
        edit(&mut app, p, 28, 0, 0.0).unwrap();
        wooden_edit(&mut app, p, 29, WoodSpecies::Birch);
        let peer = [0, 1, 8];
        wooden_edit(&mut app, peer, 29, WoodSpecies::Spruce);
        assert_eq!(
            app.world()
                .resource::<EditJournal>()
                .object_state(p)
                .unwrap()
                .wood,
            WoodSpecies::Birch
        );
        assert_eq!(
            app.world()
                .resource::<EditJournal>()
                .object_state(peer)
                .unwrap()
                .wood,
            WoodSpecies::Spruce
        );
        edit(&mut app, p, 29, 0, 0.0).unwrap();
        let state = app
            .world()
            .resource::<EditJournal>()
            .object_state(peer)
            .unwrap();
        assert_eq!(state.wood, WoodSpecies::Spruce);
        assert_eq!(state.peer, None);
        assert!(!state.secondary);
    }
    #[test]
    fn bench_uses_two_horizontal_cells_in_each_direction_and_breaks_as_one_object() {
        for facing in 0..4 {
            let mut app = app();
            let p = [-1, 1, 8];
            let yaw = facing as f32 * std::f32::consts::FRAC_PI_2;
            edit(&mut app, p, 0, 28, yaw).unwrap();
            let state = app
                .world()
                .resource::<EditJournal>()
                .object_state(p)
                .unwrap();
            let peer = state.peer.unwrap();
            assert_eq!(p[1], peer[1]);
            assert_eq!(
                app.world().resource::<ChunkStore>().block_at(peer),
                Some(28)
            );
            assert!(
                app.world()
                    .resource::<EditJournal>()
                    .object_state(peer)
                    .unwrap()
                    .secondary
            );
            edit(&mut app, peer, 28, 0, yaw).unwrap();
            assert_eq!(app.world().resource::<ChunkStore>().block_at(p), Some(0));
            assert_eq!(app.world().resource::<ChunkStore>().block_at(peer), Some(0));
        }
    }
    #[test]
    fn blocked_second_cell_leaves_first_cell_and_journal_unchanged() {
        let mut app = app();
        let p = [-1, 1, 8];
        let (owner, local) = terra_voxel::global_to_section([0, 1, 8]);
        app.world_mut()
            .resource_mut::<ChunkStore>()
            .loaded
            .get_mut(&owner)
            .unwrap()
            .data
            .set_block(local, 3)
            .unwrap();
        assert!(edit(&mut app, p, 0, 28, 0.0).is_err());
        assert_eq!(app.world().resource::<ChunkStore>().block_at(p), Some(0));
        assert_eq!(app.world().resource::<EditJournal>().edited_blocks(), 0);
    }
    #[test]
    fn matching_chests_merge_at_seams_reject_triples_and_split_after_removal() {
        let mut app = app();
        let a = [-1, 1, 8];
        let b = [0, 1, 8];
        edit(&mut app, a, 0, 29, 0.0).unwrap();
        edit(&mut app, b, 0, 29, 0.0).unwrap();
        let journal = app.world().resource::<EditJournal>();
        assert_eq!(journal.object_state(a).unwrap().peer, Some(b));
        assert!(journal.object_state(b).unwrap().secondary);
        assert!(edit(&mut app, [1, 1, 8], 0, 29, 0.0).is_err());
        edit(&mut app, b, 29, 0, 0.0).unwrap();
        assert_eq!(
            app.world()
                .resource::<EditJournal>()
                .object_state(a)
                .unwrap()
                .peer,
            None
        );
        assert_eq!(app.world().resource::<ChunkStore>().block_at(a), Some(29));
    }
    #[test]
    fn front_to_back_chests_remain_single_and_player_blocks_second_bench_cell() {
        let mut app = app();
        edit(&mut app, [-1, 1, 8], 0, 29, 0.0).unwrap();
        edit(&mut app, [-1, 1, 9], 0, 29, 0.0).unwrap();
        assert_eq!(
            app.world()
                .resource::<EditJournal>()
                .object_state([-1, 1, 8])
                .unwrap()
                .peer,
            None
        );
        app.world_mut().spawn((
            ServerCharacter {
                link: Entity::PLACEHOLDER,
            },
            Transform::from_xyz(3.5, 1.9, 8.5),
        ));
        assert!(edit(&mut app, [2, 1, 8], 0, 28, 0.0).is_err());
        assert_eq!(
            app.world().resource::<ChunkStore>().block_at([2, 1, 8]),
            Some(0)
        );
    }

    pub(super) fn furnace_snapshot(
        app: &App,
        position: [i64; 3],
    ) -> (
        terra_voxel::BlockSection,
        terra_world::edits::SectionDelta,
        terra_core::GameplayState,
        Option<terra_core::furnace::FurnaceState>,
        Option<terra_core::Inventory>,
        [u64; 2],
    ) {
        let owner = terra_voxel::global_to_section(position).0;
        let journal = app.world().resource::<EditJournal>();
        (
            app.world().resource::<ChunkStore>().loaded[&owner]
                .data
                .clone(),
            journal.section_delta(owner),
            journal.gameplay().clone(),
            journal.furnace_at(position).cloned(),
            journal.container_at(position).cloned(),
            [journal.mesh_epoch(owner), journal.water_mesh_epoch(owner)],
        )
    }

    fn populated_furnace(app: &mut App, position: [i64; 3]) -> terra_core::Inventory {
        use terra_core::{BellowsKind, Inventory, ItemKind, ItemStack, MineralItem};
        edit(app, position, 0, 59, 0.0).unwrap();
        let inventory = Inventory {
            slots: vec![
                ItemStack {
                    item: ItemKind::Mineral(MineralItem::RawIron),
                    count: 3,
                },
                ItemStack {
                    item: ItemKind::Mineral(MineralItem::Coal),
                    count: 2,
                },
                ItemStack {
                    item: ItemKind::Mineral(MineralItem::IronIngot),
                    count: 4,
                },
                ItemStack {
                    item: ItemKind::Bellows(BellowsKind::Ruby),
                    count: 1,
                },
            ],
            ..Default::default()
        };
        app.world_mut()
            .resource_mut::<EditJournal>()
            .set_container(position, inventory.clone())
            .unwrap();
        inventory
    }

    #[test]
    fn furnace_uses_two_vertical_cells_in_every_facing_and_breaks_from_either_cell() {
        let anchor = [2, 1, 8];
        let upper = [2, 2, 8];
        for facing in 0..4 {
            for target in [anchor, upper] {
                let mut app = app();
                let yaw = facing as f32 * std::f32::consts::FRAC_PI_2;
                assert!(edit(&mut app, anchor, 0, 59, yaw).unwrap());
                let journal = app.world().resource::<EditJournal>();
                let primary = journal.object_state(anchor).unwrap();
                let secondary = journal.object_state(upper).unwrap();
                assert_eq!(primary.facing, facing);
                assert_eq!(secondary.facing, facing);
                assert_eq!(primary.peer, Some(upper));
                assert_eq!(secondary.peer, Some(anchor));
                assert!(!primary.secondary);
                assert!(secondary.secondary);
                assert!(journal.furnace_at(anchor).is_some());
                assert!(journal.furnace_at(upper).is_none());
                for cell in [anchor, upper] {
                    assert_eq!(
                        app.world().resource::<ChunkStore>().block_at(cell),
                        Some(59)
                    );
                }

                assert!(edit(&mut app, target, 59, 0, yaw).unwrap());
                let journal = app.world().resource::<EditJournal>();
                for cell in [anchor, upper] {
                    assert_eq!(app.world().resource::<ChunkStore>().block_at(cell), Some(0));
                    assert!(journal.object_state(cell).is_none());
                    assert!(journal.furnace_at(cell).is_none());
                    assert!(journal.container_at(cell).is_none());
                }
            }
        }
    }

    #[test]
    fn blocked_furnace_upper_cell_leaves_world_and_private_state_unchanged() {
        let mut app = app();
        let anchor = [2, 1, 8];
        let upper = [2, 2, 8];
        let (owner, local) = terra_voxel::global_to_section(upper);
        app.world_mut()
            .resource_mut::<ChunkStore>()
            .loaded
            .get_mut(&owner)
            .unwrap()
            .data
            .set_block(local, 3)
            .unwrap();
        let before = furnace_snapshot(&app, anchor);
        assert!(edit(&mut app, anchor, 0, 59, 0.0).is_err());
        assert_eq!(furnace_snapshot(&app, anchor), before);
        assert_eq!(
            app.world().resource::<ChunkStore>().block_at(anchor),
            Some(0)
        );
        assert_eq!(
            app.world().resource::<ChunkStore>().block_at(upper),
            Some(3)
        );
    }

    #[test]
    fn furnace_checks_both_cells_against_bounded_world_height() {
        for (anchor_y, allowed) in [(355, true), (356, false)] {
            let mut app = app();
            app.world_mut()
                .resource_mut::<GameConfig>()
                .world
                .bounded_world = true;
            let anchor = [2, anchor_y, 8];
            let upper = [2, anchor_y + 1, 8];
            let owner = terra_voxel::global_to_section(anchor).0;
            let mut data = terra_voxel::BlockSection::empty(owner).unwrap();
            let floor = terra_voxel::global_to_section([2, anchor_y - 1, 8]).1;
            data.set_block(floor, 3).unwrap();
            app.world_mut().resource_mut::<ChunkStore>().loaded.insert(
                owner,
                terra_world::streaming::LoadedSection {
                    data,
                    mesh: None,
                    mesh_epoch: 0,
                    water_epoch: 0,
                    water: None,
                    props: Vec::new(),
                },
            );
            let before = furnace_snapshot(&app, anchor);
            let result = edit(&mut app, anchor, 0, 59, 0.0);
            if allowed {
                assert!(result.unwrap());
                assert_eq!(
                    app.world().resource::<ChunkStore>().block_at(upper),
                    Some(59)
                );
            } else {
                assert!(result.is_err());
                assert_eq!(furnace_snapshot(&app, anchor), before);
                assert_eq!(
                    app.world().resource::<ChunkStore>().block_at(upper),
                    Some(0)
                );
            }
        }
    }

    #[test]
    fn exhausted_gameplay_revision_cannot_partially_place_or_remove_a_furnace() {
        let anchor = [2, 1, 8];
        for removing in [false, true] {
            let mut app = app();
            if removing {
                populated_furnace(&mut app, anchor);
            }
            app.world_mut()
                .resource_mut::<EditJournal>()
                .gameplay_mut()
                .revision = u64::MAX;
            let before = furnace_snapshot(&app, anchor);
            let (expected, material) = if removing { (59, 0) } else { (0, 59) };
            assert!(edit(&mut app, anchor, expected, material, 0.0).is_err());
            assert_eq!(furnace_snapshot(&app, anchor), before);
        }
    }

    #[test]
    fn removing_either_furnace_cell_drops_all_contents_once() {
        let anchor = [2, 1, 8];
        let upper = [2, 2, 8];
        for target in [anchor, upper] {
            let mut app = app();
            let inventory = populated_furnace(&mut app, anchor);
            assert!(edit(&mut app, target, 59, 0, 0.0).unwrap());
            let journal = app.world().resource::<EditJournal>();
            assert_eq!(journal.gameplay().drops.len(), inventory.occupied_slots());
            for (drop, stack) in journal.gameplay().drops.iter().zip(&inventory.slots) {
                assert_eq!(drop.stack, *stack);
                assert_eq!(drop.position, anchor.map(|value| value as f64 + 0.5));
                assert!(drop.valid());
            }
            for cell in [anchor, upper] {
                assert_eq!(app.world().resource::<ChunkStore>().block_at(cell), Some(0));
                assert!(journal.object_state(cell).is_none());
                assert!(journal.container_at(cell).is_none());
                assert!(journal.furnace_at(cell).is_none());
            }
        }
    }

    #[test]
    fn furnace_content_drop_limit_reserves_space_for_the_furnace_item_atomically() {
        let anchor = [2, 1, 8];
        for free_slots in [4, 5] {
            let mut app = app();
            let inventory = populated_furnace(&mut app, anchor);
            {
                let mut journal = app.world_mut().resource_mut::<EditJournal>();
                for _ in 0..terra_core::MAX_WORLD_DROPS - free_slots {
                    assert!(
                        journal
                            .gameplay_mut()
                            .drop_one([4.5, 1.5, 4.5], terra_core::ItemKind::Stick)
                    );
                }
            }
            let before = furnace_snapshot(&app, anchor);
            let result = edit(&mut app, [2, 2, 8], 59, 0, 0.0);
            if free_slots == 4 {
                assert!(result.is_err());
                assert_eq!(furnace_snapshot(&app, anchor), before);
            } else {
                assert!(result.unwrap());
                let journal = app.world().resource::<EditJournal>();
                let drops = &journal.gameplay().drops;
                assert_eq!(drops.len(), terra_core::MAX_WORLD_DROPS - 1);
                let contents = &drops[drops.len() - inventory.occupied_slots()..];
                assert_eq!(
                    contents.iter().map(|drop| drop.stack).collect::<Vec<_>>(),
                    inventory.slots
                );
                assert!(journal.furnace_at(anchor).is_none());
                assert!(journal.container_at(anchor).is_none());
            }
        }
    }

    #[test]
    fn exhausted_drop_ids_leave_furnace_and_contents_unchanged() {
        let mut app = app();
        let anchor = [2, 1, 8];
        populated_furnace(&mut app, anchor);
        app.world_mut()
            .resource_mut::<EditJournal>()
            .gameplay_mut()
            .next_drop = u64::MAX - 4;
        let before = furnace_snapshot(&app, anchor);
        assert!(edit(&mut app, [2, 2, 8], 59, 0, 0.0).is_err());
        assert_eq!(furnace_snapshot(&app, anchor), before);
    }

    include!("server_opening_tests.rs");
}
