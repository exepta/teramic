use super::{ControlWorld, Peer};
use crate::protocol::session::*;
use bevy::prelude::*;
use lightyear::prelude::Connected;
use lightyear::prelude::MessageSender;
use std::{collections::HashMap, time::Instant};
use terra_core::{
    GameMode, ItemKind, ToolKind,
    farming::{FarmToolKind, FarmingAction},
};
use terra_voxel::global_to_section;
use terra_world::{
    props::placement,
    streaming::{BlockEditRequest, PropRemoveRequest, commit_block_edit, commit_prop_removal},
    targeting::WorldTarget,
};
#[path = "server_farm_tools/buckets.rs"]
mod buckets;

#[derive(Default)]
pub(super) struct FarmPeer {
    units: u8,
    used_at: Option<Instant>,
}
#[cfg(test)]
impl FarmPeer {
    pub(super) fn reset_cooldown(&mut self) {
        self.used_at = None;
    }
    pub(super) fn water(&self) -> u8 {
        self.units
    }
}

fn held(command: EditCommand, peer: &Peer, world: &ControlWorld) -> Option<ToolKind> {
    let source = command.source?;
    let bag = world.journal.gameplay().inventories.get(&peer.uuid)?;
    if peer.held_slot != Some(source.slot) || bag.revision != source.revision {
        return None;
    }
    let tool = bag.slot(usize::from(source.slot))?.item.tool_kind()?;
    (tool.farm_kind().is_some() || matches!(tool, ToolKind::Bucket | ToolKind::WaterBucket))
        .then_some(tool)
}

pub(super) fn check(command: EditCommand, peer: &Peer, world: &ControlWorld) -> EditOutcome {
    let EditAction::Farm { position, action } = command.action else {
        return EditOutcome::Invalid;
    };
    let permission = match action {
        FarmingAction::Refill | FarmingAction::FillBucket => {
            terra_core::kingdom::KingdomPermission::Inventories
        }
        FarmingAction::Harvest => terra_core::kingdom::KingdomPermission::Break,
        _ => terra_core::kingdom::KingdomPermission::Build,
    };
    if !super::kingdoms::permission_allowed(peer.uuid, permission, position, world)
        || !peer.permissions.build
        || peer.vitals.dead()
        || peer.game_mode(&world.config) == GameMode::Spectator
        || command.look.iter().any(|value| !value.is_finite())
        || command.look[0].abs() > std::f32::consts::TAU + 0.001
        || command.look[1].abs() > 1.501
        || position
            .iter()
            .any(|value| !(-1_000_000..=1_000_000).contains(value))
    {
        return EditOutcome::Invalid;
    }
    let Some(tool) = held(command, peer, world) else {
        return EditOutcome::Conflict;
    };
    if matches!(tool, ToolKind::Bucket | ToolKind::WaterBucket) {
        return buckets::check(command, peer, world, tool);
    }
    let role = tool.farm_kind().unwrap();
    if !matches!(
        (role, action),
        (FarmToolKind::Hoe, FarmingAction::Till)
            | (
                FarmToolKind::WateringCan,
                FarmingAction::Water | FarmingAction::Refill
            )
            | (FarmToolKind::Scythe, FarmingAction::Harvest)
    ) {
        return EditOutcome::Invalid;
    }
    if peer
        .farming
        .used_at
        .is_some_and(|last| last.elapsed().as_secs_f32() < tool.farm_cooldown())
    {
        return EditOutcome::RateLimited;
    }
    let section = global_to_section(position).0;
    let revision = if matches!(action, FarmingAction::Till | FarmingAction::Refill) {
        world.journal.revision(section)
    } else {
        world.journal.prop_epoch(section)
    };
    if revision != command.revision {
        return EditOutcome::Conflict;
    }
    let Ok(body) = world.bodies.get(peer.body) else {
        return EditOutcome::Invalid;
    };
    let origin = (body.translation + Vec3::Y * terra_physics::character::PLAYER_EYE_OFFSET)
        .to_array()
        .map(f64::from);
    let direction = (Quat::from_euler(EulerRot::YXZ, command.look[0], command.look[1], 0.0)
        * Vec3::NEG_Z)
        .to_array()
        .map(f64::from);
    if action == FarmingAction::Refill {
        return if terra_world::farming::water_target(
            &world.chunks,
            &world.journal,
            origin,
            direction,
        ) == Some(position)
        {
            EditOutcome::Applied
        } else {
            EditOutcome::Occluded
        };
    }
    let (block, selected) = terra_world::targeting::pick_world_with_context(
        &world.chunks,
        &world.journal,
        &world.block_properties,
        origin,
        direction,
        terra_world::props::PropPose::WeatherSwept {
            frame: terra_world::props::weather_wind_frame(world.weather.wind()),
        }
        .with_season(
            world
                .journal
                .clock
                .unwrap_or_default()
                .season(world.config.world.seasons),
        ),
    );
    let selected_support = match selected {
        Some(WorldTarget::Prop(hit)) => Some(placement::support(&hit.prop)),
        Some(WorldTarget::Block(hit)) => Some(hit.position),
        None => None,
    };
    if selected_support != Some(position)
        && block.is_none_or(|hit| hit.position != position || hit.normal != [0, 1, 0])
    {
        return EditOutcome::Occluded;
    }
    match action {
        FarmingAction::Till => {
            if !matches!(world.chunks.block_at(position), Some(1 | 2))
                || block.is_none_or(|hit| hit.position != position || hit.normal != [0, 1, 0])
                || world
                    .chunks
                    .block_at([position[0], position[1] + 1, position[2]])
                    != Some(0)
                || world.chunks.loaded.get(&section).is_none_or(|loaded| {
                    loaded
                        .props
                        .iter()
                        .any(|prop| !prop.removed && placement::support(prop) == position)
                })
            {
                return EditOutcome::Invalid;
            }
        }
        FarmingAction::Water => {
            if world
                .journal
                .crop_at(position)
                .is_none_or(|crop| crop.moisture >= 0.999)
                || peer.game_mode(&world.config) != GameMode::Creative
                    && held_water(command, peer, world) == 0
            {
                return EditOutcome::Invalid;
            }
        }
        FarmingAction::Harvest => {
            if harvests(position, tool, world, origin, peer.uuid).is_empty() {
                return EditOutcome::Conflict;
            }
        }
        FarmingAction::Refill => unreachable!(),
        FarmingAction::FillBucket | FarmingAction::EmptyBucket => return EditOutcome::Invalid,
    }
    EditOutcome::Applied
}

fn held_water(command: EditCommand, peer: &Peer, world: &ControlWorld) -> u8 {
    command
        .source
        .and_then(|source| {
            world
                .journal
                .gameplay()
                .inventories
                .get(&peer.uuid)
                .and_then(|bag| bag.slot(usize::from(source.slot)))
        })
        .map_or(0, |stack| stack.item.watering_units())
}

fn harvests(
    center: [i64; 3],
    tool: ToolKind,
    world: &ControlWorld,
    eye: [f64; 3],
    player: terra_core::PlayerUuid,
) -> Vec<([i64; 3], terra_core::CropState)> {
    let mut result = Vec::new();
    for z in -tool.farm_radius()..=tool.farm_radius() {
        for x in -tool.farm_radius()..=tool.farm_radius() {
            let support = [center[0] + x, center[1], center[2] + z];
            if !super::kingdoms::permission_allowed(
                player,
                terra_core::kingdom::KingdomPermission::Break,
                support,
                world,
            ) {
                continue;
            }
            let Some(crop) = world
                .journal
                .crop_at(support)
                .copied()
                .filter(|crop| crop.mature())
            else {
                continue;
            };
            let section = global_to_section(support).0;
            if world.chunks.loaded.get(&section).is_none_or(|loaded| {
                !loaded
                    .props
                    .iter()
                    .any(|prop| !prop.removed && prop.id.0 == crop.prop_id)
            }) {
                continue;
            }
            let target = [
                support[0] as f64 + 0.5,
                support[1] as f64 + 1.3,
                support[2] as f64 + 0.5,
            ];
            let ray = std::array::from_fn(|axis| target[axis] - eye[axis]);
            let distance = ray.iter().map(|v| v * v).sum::<f64>().sqrt();
            if distance > 8.0
                || terra_world::interaction_shapes::raycast_solid(
                    &world.chunks,
                    &world.journal,
                    &world.block_properties,
                    eye,
                    ray,
                    distance,
                )
                .is_some()
            {
                continue;
            }
            result.push((support, crop));
        }
    }
    result
}

pub(super) fn apply(
    command: EditCommand,
    peer: &mut Peer,
    sender: &mut MessageSender<ServerControl>,
    world: &mut ControlWorld,
) -> EditOutcome {
    let outcome = check(command, peer, world);
    if outcome != EditOutcome::Applied {
        return outcome;
    }
    let tool = held(command, peer, world).unwrap();
    if matches!(tool, ToolKind::Bucket | ToolKind::WaterBucket) {
        return buckets::apply(command, peer, sender, world, tool);
    }
    if tool.farm_kind() == Some(FarmToolKind::WateringCan) {
        let source = command.source.unwrap();
        let bag = &world.journal.gameplay().inventories[&peer.uuid];
        if bag.revision == u64::MAX || world.journal.gameplay().revision > u64::MAX - 512 {
            return EditOutcome::Invalid;
        }
        peer.farming.units = bag
            .slot(usize::from(source.slot))
            .unwrap()
            .item
            .watering_units();
    }
    let EditAction::Farm { position, action } = command.action else {
        return EditOutcome::Invalid;
    };
    let can_plan = if tool.watering_capacity() > 0 {
        let units = match action {
            FarmingAction::Refill
                if world.chunks.block_at(position) == Some(terra_core::plumbing::FAUCET) =>
            {
                let available =
                    terra_world::plumbing::visual_data(&world.chunks, &world.journal, position)
                        .map_or(0, |state| state.amount as u8);
                peer.farming.units
                    + tool
                        .watering_capacity()
                        .saturating_sub(peer.farming.units)
                        .min(available)
            }
            FarmingAction::Refill => tool.watering_capacity(),
            FarmingAction::Water if peer.game_mode(&world.config) != GameMode::Creative => {
                peer.farming.units.saturating_sub(1)
            }
            _ => peer.farming.units,
        };
        let source = command.source.unwrap();
        let mut next = world.journal.gameplay().inventories[&peer.uuid].clone();
        let current = next.slot(usize::from(source.slot)).unwrap();
        let replacement = current.item.with_watering_units(units).unwrap();
        if current.item != replacement && !next.transform_one(usize::from(source.slot), replacement)
        {
            return EditOutcome::Invalid;
        }
        Some(next)
    } else {
        None
    };
    let result = match action {
        FarmingAction::Refill => {
            if peer.farming.units >= tool.watering_capacity() {
                return EditOutcome::Conflict;
            }
            if world.chunks.block_at(position) == Some(terra_core::plumbing::FAUCET) {
                let needed = tool.watering_capacity().saturating_sub(peer.farming.units);
                let available =
                    terra_world::plumbing::visual_data(&world.chunks, &world.journal, position)
                        .map_or(0, |state| state.amount as u8);
                let units = needed.min(available);
                match terra_world::plumbing::take_tap(
                    &world.chunks,
                    &mut world.journal,
                    position,
                    units,
                ) {
                    Ok(true) => {
                        peer.farming.units += units;
                        Ok(true)
                    }
                    other => other,
                }
            } else {
                peer.farming.units = tool.watering_capacity();
                Ok(true)
            }
        }
        FarmingAction::FillBucket | FarmingAction::EmptyBucket => {
            unreachable!("bucket transfers handled above")
        }
        FarmingAction::Till => {
            let expected = world.chunks.block_at(position).unwrap();
            commit_block_edit(
                &mut world.chunks,
                &mut world.journal,
                &BlockEditRequest {
                    position,
                    expected_material: expected,
                    material: terra_core::farming::FARMLAND,
                },
                &world.block_properties,
            )
        }
        FarmingAction::Water => {
            let mut crop = *world.journal.crop_at(position).unwrap();
            crop.moisture = 1.0;
            let result = terra_world::farming::commit_crop_state(
                &mut world.chunks,
                &mut world.journal,
                position,
                crop,
            );
            if result.as_ref().is_ok_and(|changed| *changed)
                && peer.game_mode(&world.config) != GameMode::Creative
            {
                peer.farming.units = peer.farming.units.saturating_sub(1);
            }
            result
        }
        FarmingAction::Harvest => {
            let body = world.bodies.get(peer.body).unwrap();
            let eye = (body.translation + Vec3::Y * terra_physics::character::PLAYER_EYE_OFFSET)
                .to_array()
                .map(f64::from);
            let crops = harvests(position, tool, world, eye, peer.uuid);
            let survival = peer.game_mode(&world.config) == GameMode::Survival;
            let drops: Vec<_> = if survival {
                crops
                    .iter()
                    .flat_map(|(support, crop)| {
                        crop.harvest().into_iter().map(move |stack| {
                            (
                                [
                                    support[0] as f64 + 0.5,
                                    support[1] as f64 + 1.2,
                                    support[2] as f64 + 0.5,
                                ],
                                stack,
                            )
                        })
                    })
                    .collect()
            } else {
                Vec::new()
            };
            let mut section_counts = HashMap::new();
            for (support, _) in &crops {
                *section_counts
                    .entry(global_to_section(*support).0)
                    .or_insert(0_u64) += 1;
            }
            if world.journal.gameplay().drops.len() + drops.len() > terra_core::MAX_WORLD_DROPS
                || world
                    .journal
                    .gameplay()
                    .next_drop
                    .checked_add(drops.len() as u64)
                    .is_none()
                || section_counts.into_iter().any(|(section, count)| {
                    world
                        .journal
                        .prop_epoch(section)
                        .checked_add(count)
                        .is_none()
                })
            {
                return EditOutcome::RateLimited;
            }
            for (support, crop) in &crops {
                if let Err(error) = commit_prop_removal(
                    &mut world.chunks,
                    &mut world.journal,
                    &PropRemoveRequest {
                        section: global_to_section(*support).0,
                        id: terra_world::props::PropId(crop.prop_id),
                    },
                ) {
                    tracing::error!(%error,"Preflighted crop harvest failed");
                    return EditOutcome::Invalid;
                }
            }
            let stats = world
                .journal
                .gameplay_mut()
                .player_stats
                .entry(peer.uuid)
                .or_default();
            stats.props_harvested = stats.props_harvested.saturating_add(crops.len() as u64);
            for (position, stack) in drops {
                world
                    .journal
                    .gameplay_mut()
                    .drop_stack(position, stack, [0.0, 1.8, 0.0], 0.35);
            }
            Ok(!crops.is_empty())
        }
    };
    match result {
        Ok(true) => {
            peer.farming.used_at = Some(Instant::now());
            if tool.farm_kind() == Some(FarmToolKind::WateringCan) {
                let bag = can_plan.unwrap();
                if bag.revision != world.journal.gameplay().inventories[&peer.uuid].revision {
                    let game = world.journal.gameplay_mut();
                    game.inventories.insert(peer.uuid, bag.clone());
                    game.revision += 1;
                    sender.send::<ControlChannel>(ServerControl::InventoryResult {
                        outcome: crate::protocol::inventory::InventoryOutcome::Applied,
                        inventory: bag,
                    });
                }
                sender.send::<ControlChannel>(ServerControl::WateringReserve {
                    units: peer.farming.units.min(tool.watering_capacity()),
                    capacity: tool.watering_capacity(),
                });
            }
            EditOutcome::Applied
        }
        Ok(false) => EditOutcome::Conflict,
        Err(error) => {
            tracing::warn!(%error,"Farming action rejected");
            EditOutcome::Invalid
        }
    }
}

pub(super) fn send_progress(
    mut peers: Query<(&Peer, &mut MessageSender<ServerControl>), With<Connected>>,
    bodies: Query<&Transform, With<super::ServerCharacter>>,
    journal: Res<terra_world::edits::EditJournal>,
    mut last: Local<Option<Instant>>,
) {
    if last.is_some_and(|time| time.elapsed().as_secs_f32() < 1.0) {
        return;
    }
    *last = Some(Instant::now());
    for (peer, mut sender) in &mut peers {
        if !peer.synced {
            continue;
        }
        let Ok(body) = bodies.get(peer.body) else {
            continue;
        };
        let center = global_to_section(
            body.translation
                .to_array()
                .map(|value| value.floor() as i64),
        )
        .0;
        let mut records = Vec::new();
        for z in -1..=1 {
            for y in -1..=1 {
                for x in -1..=1 {
                    let section =
                        terra_voxel::SectionPos::new(center.x + x, center.y + y, center.z + z);
                    for support in journal.crops_in_section(section) {
                        let Some(crop) = journal.crop_at(support) else {
                            continue;
                        };
                        let distance = (0..3)
                            .map(|axis| {
                                (support[axis] as f64 - f64::from(body.translation[axis])).powi(2)
                            })
                            .sum::<f64>();
                        if distance <= 24.0 * 24.0 {
                            records.push((
                                distance,
                                CropProgress {
                                    support,
                                    kind: crop.kind,
                                    prop_id: crop.prop_id,
                                    progress: crop.progress(),
                                    moisture: crop.moisture,
                                },
                            ));
                        }
                    }
                }
            }
        }
        records.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.support.cmp(&b.1.support)));
        sender.send::<ControlChannel>(ServerControl::CropProgress(
            records
                .into_iter()
                .take(256)
                .map(|(_, record)| record)
                .collect(),
        ));
    }
}
