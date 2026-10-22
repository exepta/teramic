use super::{ControlWorld, Peer};
use crate::protocol::session::*;
use bevy::prelude::*;
use terra_physics::character::{PLAYER_EYE_OFFSET, PLAYER_HALF_HEIGHT, PLAYER_RADIUS};
use terra_voxel::{MaterialId, global_to_section};
use terra_world::{
    streaming::{BlockEditRequest, PropRemoveRequest, commit_block_edit, commit_prop_removal},
    targeting::{WorldTarget, pick_world_with_context},
};

pub(super) fn check(command: EditCommand, peer: &Peer, world: &ControlWorld) -> EditOutcome {
    if peer.vitals.dead() || peer.game_mode(&world.config) == terra_core::GameMode::Spectator {
        return EditOutcome::Invalid;
    }
    if match command.action {
        EditAction::Surface { .. } => !peer.permissions.environment,
        EditAction::Block { .. }
        | EditAction::Prop { .. }
        | EditAction::PlaceProp { .. }
        | EditAction::ToggleObject { .. } => !peer.permissions.build,
    } {
        return EditOutcome::Invalid;
    }
    if peer.game_mode(&world.config) == terra_core::GameMode::Survival
        && let Some(item) = placement_item(command.action)
        && let Some(source) = command.source
    {
        let Some(bag) = world.journal.gameplay().inventories.get(&peer.uuid) else {
            return EditOutcome::Invalid;
        };
        if bag.revision != source.revision {
            return EditOutcome::Conflict;
        }
        if bag
            .slot(usize::from(source.slot))
            .is_none_or(|stack| !same_placement_item(stack.item, item))
        {
            return EditOutcome::Invalid;
        }
    }
    if command.look.iter().any(|v| !v.is_finite())
        || command.look[0].abs() > std::f32::consts::TAU + 0.001
        || command.look[1].abs() > 1.501
    {
        return EditOutcome::Invalid;
    }
    let Ok(body) = world.bodies.get(peer.body) else {
        return EditOutcome::Invalid;
    };
    let eye = body.translation + Vec3::Y * PLAYER_EYE_OFFSET;
    let direction =
        Quat::from_euler(EulerRot::YXZ, command.look[0], command.look[1], 0.0) * Vec3::NEG_Z;
    let target = match command.action {
        EditAction::Block { position, .. }
        | EditAction::Surface { position, .. }
        | EditAction::ToggleObject { position, .. } => position.map(|v| v as f64 + 0.5),
        EditAction::Prop { section, id } => {
            let Some(prop) = world
                .chunks
                .loaded
                .get(&section)
                .and_then(|s| s.props.iter().find(|p| p.id == id && !p.removed))
            else {
                return EditOutcome::Conflict;
            };
            prop.position
        }
        EditAction::PlaceProp { support, .. } => [
            support[0] as f64 + 0.5,
            support[1] as f64 + 1.0,
            support[2] as f64 + 0.5,
        ],
    };
    let distance = (0..3)
        .map(|i| (target[i] - f64::from(eye[i])).powi(2))
        .sum::<f64>();
    let crystal_placement = matches!(
        command.action,
        EditAction::Block {
            material: 51..=53,
            ..
        }
    );
    // Extension may end beyond reach; the clicked cell must still be hit by
    // the authority's eight-metre ray and resolve to exactly this endpoint.
    if !distance.is_finite() || (distance > 9.5 * 9.5 && !crystal_placement) {
        return EditOutcome::OutOfReach;
    }
    let (block, selected) = pick_world_with_context(
        &world.chunks,
        &world.journal,
        &world.block_properties,
        eye.to_array().map(f64::from),
        direction.to_array().map(f64::from),
        terra_world::props::PropPose::WeatherSwept {
            frame: terra_world::props::weather_wind_frame(world.weather.wind()),
        },
    );
    match command.action {
        EditAction::ToggleObject {
            position,
            expected,
            facing,
        } => {
            if if terra_core::beds::is_bed(expected) {
                facing >= 28
            } else {
                !terra_world::objects::is_opening(expected) || facing >= 8
            } {
                return EditOutcome::Invalid;
            }
            if command.revision != world.journal.revision(global_to_section(position).0)
                || world.chunks.block_at(position) != Some(expected)
                || world
                    .journal
                    .object_state(position)
                    .is_none_or(|s| s.facing != facing)
            {
                return EditOutcome::Conflict;
            }
            if !matches!(selected,Some(WorldTarget::Block(hit)) if hit.position==position) {
                return EditOutcome::Occluded;
            }
        }
        EditAction::Block {
            position,
            expected,
            material,
        } => {
            if world.config.world.bounded_world && !(-128..=356).contains(&position[1]) {
                return EditOutcome::Invalid;
            }
            if material == 0
                && expected == MaterialId::Bedrock as u16
                && peer.game_mode(&world.config) == terra_core::GameMode::Survival
            {
                return EditOutcome::Invalid;
            }
            if material == 0
                && peer.game_mode(&world.config) == terra_core::GameMode::Survival
                && world.block_properties.0.get(&expected).map_or_else(
                    || terra_core::OreKind::from_material(expected).is_some(),
                    |properties| !properties.can_harvest(super::gameplay::held_tool(peer, world)),
                )
            {
                return EditOutcome::Invalid;
            }
            if MaterialId::from_raw(material).is_none()
                || MaterialId::from_raw(expected).is_none()
                || material == expected && !terra_core::construction::is_slab(material)
                || !terra_core::construction::obtainable(material)
                || (material == 0 && expected == MaterialId::FluidWater as u16)
                || (material != 0
                    && expected != 0
                    && expected != MaterialId::FluidWater as u16
                    && !(terra_core::construction::is_slab(material)
                        && terra_core::construction::is_slab(expected)))
            {
                return EditOutcome::Invalid;
            }
            let section = global_to_section(position).0;
            if world.journal.revision(section) != command.revision
                || world.chunks.block_at(position) != Some(expected)
            {
                return EditOutcome::Conflict;
            }
            let visible = if material == 0 {
                matches!(selected,Some(WorldTarget::Block(hit)) if hit.position==position)
            } else {
                block.and_then(|hit| {
                    terra_world::targeting::item_placement_with_state(
                        &world.chunks,
                        &world.journal,
                        hit,
                        material,
                    )
                }) == Some((position, expected))
            };
            if !visible {
                return EditOutcome::Occluded;
            }
            if material == 40
                && block.is_none_or(|hit| {
                    hit.normal == [0, -1, 0]
                        || !world
                            .chunks
                            .block_at(hit.position)
                            .and_then(MaterialId::from_raw)
                            .is_some_and(MaterialId::is_solid)
                        || world
                            .chunks
                            .block_at(hit.position)
                            .is_none_or(|m| !world.block_properties.shape(m).is_empty())
                })
            {
                return EditOutcome::Invalid;
            }
            if terra_world::segments::is_segment(material) {
                let below = [position[0], position[1] - 1, position[2]];
                let above = [position[0], position[1] + 1, position[2]];
                let support = |p| {
                    world.chunks.block_at(p).is_some_and(|m| {
                        m == material
                            || (!terra_world::segments::is_segment(m)
                                && MaterialId::from_raw(m).is_some_and(MaterialId::is_solid))
                    })
                };
                let valid = match material {
                    50 | 56 => support(below),
                    54 | 55 => support(above),
                    _ => support(below) || support(above),
                };
                if !valid
                    || material == 50
                        && world
                            .chunks
                            .fluid_cell_at(&world.terrain.0, &world.journal, position, expected)
                            .amount
                            == 0
                {
                    return EditOutcome::Invalid;
                }
            }
            if MaterialId::from_raw(material).is_some_and(MaterialId::is_solid) {
                let bounds = block.and_then(|hit| {
                    terra_world::targeting::placement_bounds(
                        &world.chunks,
                        &world.journal,
                        hit,
                        material,
                        (
                            eye.to_array().map(f64::from),
                            direction.to_array().map(f64::from),
                        ),
                    )
                });
                let Some(bounds) = bounds else {
                    return EditOutcome::Invalid;
                };
                let half = Vec3::new(PLAYER_RADIUS, PLAYER_HALF_HEIGHT, PLAYER_RADIUS);
                if world.bodies.iter().any(|body| {
                    bounds.intersects(
                        (body.translation - half).to_array().map(f64::from),
                        (body.translation + half).to_array().map(f64::from),
                    )
                }) {
                    return EditOutcome::Invalid;
                }
            }
            if material != 0
                && peer.game_mode(&world.config) == terra_core::GameMode::Survival
                && world
                    .journal
                    .gameplay()
                    .inventories
                    .get(&peer.uuid)
                    .is_none_or(|bag| {
                        !bag.slots
                            .iter()
                            .any(|s| s.count > 0 && s.item.block_material() == Some(material))
                    })
            {
                return EditOutcome::Invalid;
            }
        }
        EditAction::Prop { section, id } => {
            if world.journal.prop_epoch(section) != command.revision {
                return EditOutcome::Conflict;
            }
            if !matches!(selected,Some(WorldTarget::Prop(hit)) if hit.prop.id==id && hit.prop.section==section)
            {
                return EditOutcome::Occluded;
            }
        }
        EditAction::Surface {
            position,
            condition,
        } => {
            if condition.fire > 0.0
                && world.journal.surface_condition(position).fire == 0.0
                && world.journal.active_fires().take(256).count() >= 256
            {
                return EditOutcome::RateLimited;
            }
            // Existing environment tools are creative commands, not unrestricted world writes.
            if condition.validate().is_err()
                || !world
                    .chunks
                    .block_at(position)
                    .and_then(MaterialId::from_raw)
                    .is_some_and(MaterialId::is_solid)
            {
                return EditOutcome::Invalid;
            }
            if world
                .journal
                .appearance_epoch(global_to_section(position).0)
                != command.revision
            {
                return EditOutcome::Conflict;
            }
            if block.is_none_or(|hit| hit.position != position) {
                return EditOutcome::Occluded;
            }
        }
        EditAction::PlaceProp { support, kind } => {
            let section = global_to_section(support).0;
            if world.journal.prop_epoch(section) != command.revision {
                return EditOutcome::Conflict;
            }
            let (_, planting_target) = terra_world::targeting::pick_world_for_planting(
                &world.chunks,
                &world.journal,
                &world.block_properties,
                eye.to_array().map(f64::from),
                direction.to_array().map(f64::from),
                terra_world::props::PropPose::WeatherSwept {
                    frame: terra_world::props::weather_wind_frame(world.weather.wind()),
                },
            );
            if !matches!(planting_target, Some(WorldTarget::Block(hit)) if hit.position == support && hit.normal == [0, 1, 0])
            {
                return EditOutcome::Occluded;
            }
            if !prop_environment_valid(
                &world.chunks,
                &world.terrain.0,
                &world.journal,
                support,
                kind,
            ) {
                return EditOutcome::Invalid;
            }
            if world.chunks.loaded.get(&section).is_none_or(|section| {
                terra_world::props::placement::next_slot(&section.props, support, prop_kind(kind))
                    .is_none()
            }) {
                return EditOutcome::Conflict;
            }
            if peer.game_mode(&world.config) == terra_core::GameMode::Survival
                && world
                    .journal
                    .gameplay()
                    .inventories
                    .get(&peer.uuid)
                    .is_none_or(|bag| bag.count(terra_core::ItemKind::Prop(kind)) == 0)
            {
                return EditOutcome::Invalid;
            }
        }
    }
    EditOutcome::Applied
}

pub(super) fn apply(
    command: EditCommand,
    peer: &Peer,
    world: &mut ControlWorld,
    timed: bool,
) -> EditOutcome {
    let survival = peer.game_mode(&world.config) == terra_core::GameMode::Survival;
    let removal = matches!(
        command.action,
        EditAction::Block { material: 0, .. } | EditAction::Prop { .. }
    );
    if survival && removal && !timed {
        return EditOutcome::Invalid;
    }
    let valid = check(command, peer, world);
    if valid != EditOutcome::Applied {
        return valid;
    }
    if let EditAction::ToggleObject {
        position,
        expected: 71..=73,
        ..
    } = command.action
    {
        return match super::beds::interact(position, command.source, peer, world) {
            Ok(true) => EditOutcome::Applied,
            Ok(false) => EditOutcome::Conflict,
            Err(_) => EditOutcome::Invalid,
        };
    }
    let torch_facing = if matches!(command.action, EditAction::Block { material: 40, .. }) {
        let Ok(body) = world.bodies.get(peer.body) else {
            return EditOutcome::Invalid;
        };
        let eye = body.translation + Vec3::Y * PLAYER_EYE_OFFSET;
        let direction =
            Quat::from_euler(EulerRot::YXZ, command.look[0], command.look[1], 0.0) * Vec3::NEG_Z;
        pick_world_with_context(
            &world.chunks,
            &world.journal,
            &world.block_properties,
            eye.to_array().map(f64::from),
            direction.to_array().map(f64::from),
            terra_world::props::PropPose::WeatherSwept {
                frame: terra_world::props::weather_wind_frame(world.weather.wind()),
            },
        )
        .0
        .map_or(0, |hit| match hit.normal {
            [1, 0, 0] => 4,
            [-1, 0, 0] => 5,
            [0, 0, 1] => 6,
            [0, 0, -1] => 7,
            _ => 0,
        })
    } else {
        0
    };
    let placed = placement_item(command.action).map(|fallback| {
        let bag = world.journal.gameplay().inventories.get(&peer.uuid);
        command
            .source
            .and_then(|source| bag?.slot(usize::from(source.slot)))
            .or_else(|| {
                bag?.slots
                    .iter()
                    .find(|stack| stack.valid() && same_placement_item(stack.item, fallback))
                    .copied()
            })
            .map_or(fallback, |stack| stack.item)
    });
    let construction_facing = if let EditAction::Block {
        position,
        material,
        expected,
    } = command.action
        && (terra_core::construction::is_stair(material)
            || terra_core::construction::is_slab(material))
    {
        if terra_core::construction::is_slab(expected) {
            terra_core::construction::merged_slab(
                expected,
                world
                    .journal
                    .object_state(position)
                    .unwrap_or_default()
                    .facing,
                material,
            )
        } else {
            let Ok(body) = world.bodies.get(peer.body) else {
                return EditOutcome::Invalid;
            };
            let eye = body.translation + Vec3::Y * PLAYER_EYE_OFFSET;
            let direction = Quat::from_euler(EulerRot::YXZ, command.look[0], command.look[1], 0.0)
                * Vec3::NEG_Z;
            let hit = pick_world_with_context(
                &world.chunks,
                &world.journal,
                &world.block_properties,
                eye.to_array().map(f64::from),
                direction.to_array().map(f64::from),
                terra_world::props::PropPose::Rest,
            )
            .0;
            hit.map(|hit| {
                terra_world::targeting::placement_facing(
                    &world.chunks,
                    &world.journal,
                    hit,
                    material,
                    (
                        eye.to_array().map(f64::from),
                        direction.to_array().map(f64::from),
                    ),
                )
            })
        }
    } else {
        None
    };
    let slab_drop = if let EditAction::Block {
        position,
        expected,
        material: 0,
    } = command.action
        && terra_core::construction::is_slab(expected)
        && world
            .journal
            .object_state(position)
            .is_some_and(|s| terra_core::construction::full_slab(s.facing))
    {
        let facing = world
            .journal
            .object_state(position)
            .unwrap_or_default()
            .facing;
        Some(if facing == terra_core::construction::DOUBLE_SLAB {
            expected
        } else {
            terra_core::construction::slab_parts(expected, facing)[1].0
        })
    } else {
        None
    };
    let furniture = match super::furniture::plan(command, world) {
        Ok(plan) => plan,
        Err(_) => return EditOutcome::Invalid,
    };
    let drop = if survival {
        resource_drop(command.action, &world.chunks).map(|(position, item)| {
            let item = if let EditAction::Block {
                position,
                expected,
                material: 0,
            } = command.action
            {
                world.journal.object_state(position).map_or(item, |state| {
                    if terra_core::beds::is_bed(expected) && state.facing >= 4 {
                        terra_core::ItemKind::DyedBed {
                            material: expected,
                            color: terra_core::beds::bed_color(expected, state.facing),
                        }
                    } else {
                        terra_core::ItemKind::for_wood(expected, state.wood)
                    }
                })
            } else {
                item
            };
            (position, item)
        })
    } else {
        None
    };
    if drop.is_some() && !world.journal.gameplay().can_drop() {
        return EditOutcome::RateLimited;
    }
    if drop.is_some()
        && slab_drop.is_some()
        && (world.journal.gameplay().drops.len() + 2 > terra_core::MAX_WORLD_DROPS
            || world.journal.gameplay().next_drop.checked_add(2).is_none())
    {
        return EditOutcome::RateLimited;
    }
    if survival
        && removal
        && drop.is_some_and(|(position, item)| {
            !terra_core::WorldDrop {
                id: 1,
                position,
                stack: terra_core::ItemStack { item, count: 1 },
                velocity: [0.0; 3],
                age_seconds: 0.0,
                rotation: [0.0, 0.0, 0.0, 1.0],
                angular_velocity: [0.0; 3],
                grounded: false,
                pickup_delay_seconds: 0.35,
            }
            .valid()
        })
    {
        return EditOutcome::Invalid;
    }
    let result = match command.action {
        EditAction::ToggleObject {
            position,
            expected,
            facing,
        } => super::furniture::toggle(position, expected, facing, world),
        EditAction::Block {
            position,
            expected,
            material,
        } => {
            if let Some(plan) = furniture {
                let wood = placed
                    .and_then(terra_core::ItemKind::wood_species)
                    .unwrap_or_default();
                super::furniture::commit(
                    plan.with_wood(position, wood)
                        .with_item(placed)
                        .with_construction_facing(construction_facing),
                    world,
                )
            } else {
                let result = commit_block_edit(
                    &mut world.chunks,
                    &mut world.journal,
                    &BlockEditRequest {
                        position,
                        expected_material: expected,
                        material,
                    },
                    &world.block_properties,
                );
                if result.is_ok() && material == 40 {
                    let facing = torch_facing;
                    if let Err(error) = world.journal.set_object_state(
                        position,
                        Some(terra_world::edits::BlockObjectState {
                            facing,
                            ..Default::default()
                        }),
                    ) {
                        return {
                            tracing::error!(%error, "Torch mount commit failed");
                            EditOutcome::Invalid
                        };
                    }
                }
                result
            }
        }
        EditAction::Prop { section, id } => commit_prop_removal(
            &mut world.chunks,
            &mut world.journal,
            &PropRemoveRequest { section, id },
        ),
        EditAction::Surface {
            position,
            condition,
        } => world
            .journal
            .set_surface_condition(position, condition)
            .map_err(|e| e.to_string()),
        EditAction::PlaceProp { support, kind } => {
            let prop = placed_prop(support, kind, command.revision);
            terra_world::streaming::commit_prop_placement(
                &mut world.chunks,
                &mut world.journal,
                prop,
            )
        }
    };
    match result {
        Ok(true) => {
            let stats = world
                .journal
                .gameplay_mut()
                .player_stats
                .entry(peer.uuid)
                .or_default();
            match command.action {
                EditAction::Block { material: 0, .. } => {
                    stats.blocks_broken = stats.blocks_broken.saturating_add(1)
                }
                EditAction::Block { .. } => {
                    stats.blocks_placed = stats.blocks_placed.saturating_add(1)
                }
                EditAction::Prop { .. } => {
                    stats.props_harvested = stats.props_harvested.saturating_add(1)
                }
                _ => {}
            }
            if let Some((position, item)) = drop {
                world.journal.gameplay_mut().drop_stack(
                    position,
                    terra_core::ItemStack {
                        item,
                        count: if slab_drop.is_some() && slab_drop == item.block_material() {
                            2
                        } else {
                            1
                        },
                    },
                    [0.0, 1.8, 0.0],
                    0.35,
                );
                if let Some(other) = slab_drop.filter(|&id| Some(id) != item.block_material()) {
                    world.journal.gameplay_mut().drop_stack(
                        position,
                        terra_core::ItemStack {
                            item: terra_core::ItemKind::Block(other),
                            count: 1,
                        },
                        [0.0, 1.8, 0.0],
                        0.35,
                    );
                }
            }
            if survival && let Some(item) = placed {
                world.journal.gameplay_mut().consume_from(
                    peer.uuid,
                    item,
                    command.source.map(|source| source.slot),
                );
            }
            EditOutcome::Applied
        }
        Ok(false) => EditOutcome::Conflict,
        Err(error) => {
            tracing::warn!(%error,"Authoritative edit rejected");
            EditOutcome::Invalid
        }
    }
}

fn same_placement_item(a: terra_core::ItemKind, b: terra_core::ItemKind) -> bool {
    match (a.block_material(), b.block_material()) {
        (Some(a), Some(b)) => a == b,
        _ => a == b,
    }
}

fn placement_item(action: EditAction) -> Option<terra_core::ItemKind> {
    match action {
        EditAction::Block {
            material: material @ 1..,
            ..
        } => Some(terra_core::ItemKind::Block(material)),
        EditAction::PlaceProp { kind, .. } => Some(terra_core::ItemKind::Prop(kind)),
        _ => None,
    }
}

fn placed_prop(
    support: [i64; 3],
    kind: terra_core::ResourcePropKind,
    epoch: u64,
) -> terra_world::props::WorldProp {
    use terra_world::props::{PropId, WorldProp};
    let mut hash = 0xcbf2_9ce4_8422_2325_u64 ^ epoch.rotate_left(23) ^ kind as u64;
    for value in support {
        hash = (hash ^ value as u64).wrapping_mul(0x100_0000_01b3);
    }
    WorldProp {
        id: PropId(hash),
        kind: prop_kind(kind),
        position: [
            support[0] as f64 + 0.5,
            support[1] as f64 + 1.0,
            support[2] as f64 + 0.5,
        ],
        rotation: (hash % 628) as f32 * 0.01,
        scale: 1.0,
        variant: (hash % 6) as u8,
        section: global_to_section(support).0,
        removed: false,
    }
}

fn prop_kind(kind: terra_core::ResourcePropKind) -> terra_world::props::PropKind {
    kind.into()
}

fn prop_environment_valid(
    chunks: &terra_world::streaming::ChunkStore,
    terrain: &terra_world::TerraGenerator,
    journal: &terra_world::edits::EditJournal,
    support: [i64; 3],
    kind: terra_core::ResourcePropKind,
) -> bool {
    if !chunks
        .block_at(support)
        .and_then(MaterialId::from_raw)
        .is_some_and(|m| {
            if prop_kind(kind).aquatic() {
                m.supports_water_plants()
            } else {
                m.is_natural_ground()
            }
        })
    {
        return false;
    }
    let above = [support[0], support[1] + 1, support[2]];
    let Some(material) = chunks.block_at(above) else {
        return false;
    };
    if !matches!(
        MaterialId::from_raw(material),
        Some(MaterialId::Air | MaterialId::FluidWater)
    ) {
        return false;
    }
    let wet = chunks
        .fluid_cell_at(terrain, journal, above, material)
        .amount
        > 0;
    if kind == terra_core::ResourcePropKind::SmallStones {
        return true;
    }
    if !prop_kind(kind).aquatic() {
        return !wet;
    }
    if !wet {
        return false;
    }
    // A submerged base alone is insufficient for tall corals and seagrass.
    let prop = placed_prop(
        support,
        kind,
        journal.prop_epoch(global_to_section(support).0),
    );
    let [low, high] = prop.bounds();
    let size = std::array::from_fn(|i| high[i] - low[i]);
    terra_world::creatures::Habitat {
        terrain,
        chunks,
        journal,
    }
    .clear(
        [(low[0] + high[0]) * 0.5, low[1], (low[2] + high[2]) * 0.5],
        size,
        true,
    )
}

fn resource_drop(
    action: EditAction,
    chunks: &terra_world::streaming::ChunkStore,
) -> Option<([f64; 3], terra_core::ItemKind)> {
    use terra_core::{ItemKind, ResourcePropKind};
    match action {
        EditAction::Block {
            expected: 30 | 38,
            material: 0,
            ..
        } => None,
        EditAction::Block {
            position,
            expected,
            material: 0,
        } => Some((
            position.map(|v| v as f64 + 0.5),
            if expected == terra_voxel::MaterialId::Grass as u16 {
                ItemKind::Block(terra_voxel::MaterialId::Dirt as u16)
            } else if expected == terra_voxel::MaterialId::SaltCrystal as u16 {
                ItemKind::Mineral(terra_core::OreKind::Salt.resource())
            } else {
                terra_core::OreKind::from_material(expected)
                    .map_or(ItemKind::Block(expected), |ore| {
                        ItemKind::Mineral(ore.resource())
                    })
            },
        )),
        EditAction::Prop { section, id } => {
            let prop = chunks
                .loaded
                .get(&section)?
                .props
                .iter()
                .find(|p| p.id == id && !p.removed)?;
            let kind: ResourcePropKind = prop.kind.into();
            let mut position = prop.position;
            position[1] += 0.25;
            Some((position, ItemKind::Prop(kind)))
        }
        _ => None,
    }
}

#[cfg(test)]
#[path = "server_validation_aquatic_tests.rs"]
mod aquatic_tests;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn grass_harvest_drops_dirt_without_changing_creative_pick() {
        let chunks = terra_world::streaming::ChunkStore::default();
        let (_, item) = resource_drop(
            EditAction::Block {
                position: [-1, 2, -3],
                expected: 1,
                material: 0,
            },
            &chunks,
        )
        .unwrap();
        assert_eq!(item, terra_core::ItemKind::Block(2));
        assert_ne!(item, terra_core::ItemKind::Block(1));
    }
    use bevy::ecs::system::SystemState;
    use terra_core::{
        GameConfig, GameMode, Inventory, ItemKind, PlayerId, PlayerUuid, ToolKind,
        block_properties::{BlockProperties, BlockPropertiesCatalog, ToolClass},
    };
    use terra_world::{
        edits::EditJournal,
        streaming::{ChunkStore, LoadedSection, WorldTerrain},
    };

    #[test]
    fn salt_crystal_harvest_reuses_the_existing_salt_resource_but_host_rock_drops_nothing() {
        for expected in [30, 38] {
            assert!(
                resource_drop(
                    EditAction::Block {
                        position: [-1, -32, 7],
                        expected,
                        material: 0
                    },
                    &ChunkStore::default()
                )
                .is_none()
            );
        }
        let drop = resource_drop(
            EditAction::Block {
                position: [-1, -32, 7],
                expected: MaterialId::SaltCrystal as u16,
                material: 0,
            },
            &ChunkStore::default(),
        )
        .unwrap();
        assert_eq!(drop.0, [-0.5, -31.5, 7.5]);
        assert_eq!(
            drop.1,
            ItemKind::Mineral(terra_core::OreKind::Salt.resource())
        );
    }

    #[test]
    fn missing_ruby_mechanics_fail_closed_without_mining_or_drops_even_with_an_iron_pickaxe() {
        let mut config = GameConfig::default();
        config.server.game_mode = GameMode::Survival;
        let terrain = terra_world::TerraGenerator::new(config.world.clone()).unwrap();
        let mut app = App::new();
        app.insert_resource(terra_core::WorldClock::new(config.world.time))
            .init_resource::<terra_core::WorldWeather>()
            .insert_resource(config)
            .insert_resource(WorldTerrain(terrain))
            .init_resource::<terra_core::crafting::RecipeBook>()
            .init_resource::<BlockPropertiesCatalog>()
            .init_resource::<ChunkStore>()
            .init_resource::<EditJournal>()
            .init_resource::<crate::server::ConnectedProfiles>();
        // A missing single asset leaves a nonempty catalog; startup's empty-
        // catalog check alone cannot enforce this ore's harvest requirements.
        app.world_mut()
            .resource_mut::<BlockPropertiesCatalog>()
            .0
            .insert(3, BlockProperties::default());
        let position = [4, 4, 4];
        let owner = terra_voxel::SectionPos::default();
        let mut data = terra_voxel::BlockSection::empty(owner).unwrap();
        data.set_block([4, 4, 4], 35).unwrap();
        app.world_mut().resource_mut::<ChunkStore>().loaded.insert(
            owner,
            LoadedSection {
                data,
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            },
        );
        let uuid = PlayerUuid::new_v4();
        let mut inventory = Inventory::default();
        inventory.insert(ItemKind::Tool(ToolKind::IronPickaxe), 1);
        {
            let mut journal = app.world_mut().resource_mut::<EditJournal>();
            journal.record(position, 35).unwrap();
            journal.gameplay_mut().inventories.insert(uuid, inventory);
        }
        let body_position = Vec3::new(4.5, 3.0, 8.5);
        let body = app
            .world_mut()
            .spawn((
                Transform::from_translation(body_position),
                crate::server::ServerCharacter {
                    link: Entity::PLACEHOLDER,
                },
            ))
            .id();
        let eye = body_position + Vec3::Y * PLAYER_EYE_OFFSET;
        let direction = (Vec3::new(4.5, 4.5, 4.5) - eye).normalize();
        let look = [(-direction.x).atan2(-direction.z), direction.y.asin()];
        let now = std::time::Instant::now();
        let peer = Peer {
            chest_cursor: None,
            creature_action_at: std::time::Instant::now(),
            container: None,
            held_slot: Some(0),
            mode: Some(GameMode::Survival),
            generation: 0,
            chat: Default::default(),
            uuid,
            gender: terra_core::PlayerGender::default(),
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
            last_movement: Default::default(),
            vitals: Default::default(),
            survival: Default::default(),
            stats_sent: None,
        };
        let command = EditCommand {
            request: 1,
            revision: 1,
            look,
            source: None,
            action: EditAction::Block {
                position,
                expected: 35,
                material: 0,
            },
        };
        {
            let mut system = SystemState::<ControlWorld>::new(app.world_mut());
            let mut world = system.get_mut(app.world_mut()).unwrap();
            assert_eq!(check(command, &peer, &world), EditOutcome::Invalid);
            assert_eq!(
                apply(command, &peer, &mut world, true),
                EditOutcome::Invalid
            );
            assert_eq!(world.chunks.block_at(position), Some(35));
            assert_eq!(world.journal.block(position), Some(35));
            assert!(world.journal.gameplay().drops.is_empty());
        }
        app.world_mut()
            .resource_mut::<BlockPropertiesCatalog>()
            .0
            .insert(
                35,
                BlockProperties {
                    required_tool: Some(ToolClass::Pickaxe),
                    mining_level: 3,
                    ..Default::default()
                },
            );
        let mut system = SystemState::<ControlWorld>::new(app.world_mut());
        let mut world = system.get_mut(app.world_mut()).unwrap();
        assert_eq!(
            apply(command, &peer, &mut world, true),
            EditOutcome::Applied
        );
        assert_eq!(world.chunks.block_at(position), Some(0));
        assert_eq!(world.journal.gameplay().drops.len(), 1);
        assert_eq!(
            world.journal.gameplay().drops[0].stack.item,
            ItemKind::Mineral(terra_core::MineralItem::Ruby)
        );
    }
}
