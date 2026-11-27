use super::*;
#[path = "server_kingdom_policy.rs"]
mod policy;
use crate::protocol::kingdom::{KingdomAction, KingdomCommand, KingdomOutcome, KingdomResult};
use terra_core::PlayerUuid;
use terra_core::kingdom::{
    ClaimActor, KingdomError, KingdomPermission, claim_chunk, valid_claim_chunk,
};

#[derive(Resource, Default)]
pub(super) struct KingdomRuntime(HashMap<PlayerId, KingdomPeer>);

struct KingdomPeer {
    request: u64,
    tokens: f64,
    updated: Instant,
    sent: Option<u64>,
}

impl Default for KingdomPeer {
    fn default() -> Self {
        Self {
            request: 0,
            tokens: 16.0,
            updated: Instant::now(),
            sent: None,
        }
    }
}

impl KingdomPeer {
    fn accept(&mut self, request: u64) -> Result<(), KingdomError> {
        if request == 0 || request <= self.request {
            return Err(KingdomError::StaleRequest);
        }
        self.request = request;
        let now = Instant::now();
        self.tokens =
            (self.tokens + now.duration_since(self.updated).as_secs_f64() * 8.0).min(16.0);
        self.updated = now;
        if self.tokens < 1.0 {
            return Err(KingdomError::RateLimited);
        }
        self.tokens -= 1.0;
        Ok(())
    }
}

pub(super) fn control(
    command: KingdomCommand,
    peer: &Peer,
    sender: &mut MessageSender<ServerControl>,
    world: &mut ControlWorld,
    runtime: &mut KingdomRuntime,
) {
    let accepted = runtime
        .0
        .entry(peer.id)
        .or_default()
        .accept(command.request);
    let outcome = accepted
        .and_then(|()| apply(command.action, peer, world))
        .unwrap_or_else(KingdomOutcome::Rejected);
    sender.send::<ControlChannel>(ServerControl::KingdomResult(KingdomResult {
        request: command.request,
        outcome,
    }));
}

fn location(action: &KingdomAction, world: &ControlWorld) -> Result<[i64; 3], KingdomError> {
    match action {
        KingdomAction::Inspect { position }
        | KingdomAction::Found { position, .. }
        | KingdomAction::FoundSettlement { position, .. } => Ok(*position),
        KingdomAction::Rename { id, .. }
        | KingdomAction::Dissolve { id }
        | KingdomAction::Join { id }
        | KingdomAction::Leave { id }
        | KingdomAction::Kick { id, .. }
        | KingdomAction::ClaimChunk { id, .. }
        | KingdomAction::ClaimBatch { id, .. }
        | KingdomAction::ReleaseChunk { id, .. }
        | KingdomAction::ClaimBotChunk { id, .. }
        | KingdomAction::ReleaseBotChunk { id, .. }
        | KingdomAction::Upgrade { id }
        | KingdomAction::ChangeAppearance { id, .. }
        | KingdomAction::SetRole { id, .. }
        | KingdomAction::SetRelation { id, .. } => {
            let kingdom = world
                .journal
                .gameplay()
                .kingdoms
                .get(*id)
                .ok_or(KingdomError::UnknownKingdom)?;
            if kingdom.state != terra_core::kingdom::KingdomState::Active {
                return Err(KingdomError::NotActive);
            }
            Ok(kingdom.location)
        }
    }
}

fn claim_reach(
    peer: &Peer,
    world: &ControlWorld,
    chunk: [i64; 2],
    bot: Option<u64>,
) -> Result<ClaimActor, KingdomError> {
    if !valid_claim_chunk(chunk) {
        return Err(KingdomError::InvalidClaim);
    }
    if let Some(id) = bot {
        let body = world
            .bodies
            .get(peer.body)
            .map_err(|_| KingdomError::OutOfReach)?;
        let gameplay = world.journal.gameplay();
        let creature = gameplay
            .creatures
            .animals
            .get(&id)
            .ok_or(KingdomError::InvalidBot)?;
        if id == 0
            || creature.health <= 0.0
            || gameplay.creatures.owners.get(&id) != Some(&peer.uuid)
        {
            return Err(KingdomError::InvalidBot);
        }
        let bot_chunk = claim_chunk(creature.position.map(|v| v.floor() as i64));
        if bot_chunk != chunk
            || Vec3::from_array(creature.position.map(|v| v as f32))
                .distance_squared(body.translation)
                > 64.0 * 64.0
        {
            return Err(KingdomError::InvalidBot);
        }
        Ok(ClaimActor::Bot {
            id,
            authorized_by: peer.uuid,
        })
    } else {
        Ok(ClaimActor::Player(peer.uuid))
    }
}

pub(super) fn check_keep(
    position: [i64; 3],
    peer: &Peer,
    world: &ControlWorld,
) -> Result<(), KingdomError> {
    if !terra_core::kingdom::valid_location(position) {
        return Err(KingdomError::InvalidLocation);
    }
    if world
        .journal
        .block(position)
        .or_else(|| world.chunks.block_at(position))
        != Some(terra_core::castle::BERGFRIED)
    {
        return Err(KingdomError::NotBergfried);
    }
    let body = world
        .bodies
        .get(peer.body)
        .map_err(|_| KingdomError::OutOfReach)?;
    if world
        .journal
        .gameplay()
        .kingdoms
        .territorial_keep(
            peer.uuid,
            body.translation
                .to_array()
                .map(|v| f64::from(v).floor() as i64),
        )
        .is_some_and(|keep| keep.location == position)
    {
        return Ok(());
    }
    let eye = body.translation + Vec3::Y * terra_physics::character::PLAYER_EYE_OFFSET;
    let center = position.map(|v| v as f64 + 0.5);
    let distance: f64 = (0..3)
        .map(|i| (center[i] - f64::from(eye[i])).powi(2))
        .sum();
    if !distance.is_finite() || distance > 8.0 * 8.0 {
        return Err(KingdomError::OutOfReach);
    }
    let direction = Quat::from_euler(EulerRot::YXZ, peer.look[0], peer.look[1], 0.0) * Vec3::NEG_Z;
    let (_, target) = terra_world::targeting::pick_world_with_context(
        &world.chunks,
        &world.journal,
        &world.block_properties,
        eye.to_array().map(f64::from),
        direction.to_array().map(f64::from),
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
    if !matches!(target, Some(terra_world::targeting::WorldTarget::Block(hit)) if hit.position == position)
    {
        return Err(KingdomError::Occluded);
    }
    Ok(())
}

fn apply(
    action: KingdomAction,
    peer: &Peer,
    world: &mut ControlWorld,
) -> Result<KingdomOutcome, KingdomError> {
    if peer.vitals.dead() || peer.chat.teleport.is_some() {
        return Err(KingdomError::NotAllowed);
    }
    let inspect = matches!(action, KingdomAction::Inspect { .. });
    if !inspect && peer.game_mode(&world.config) == terra_core::GameMode::Spectator {
        return Err(KingdomError::NotAllowed);
    }
    if !inspect && !peer.permissions.build {
        return Err(KingdomError::NotAllowed);
    }
    let position = location(&action, world)?;
    let claim_actor = match &action {
        KingdomAction::ClaimBatch { claims, .. } => {
            if claims.is_empty()
                || claims.len() > terra_core::kingdom::KingdomRegistry::MAX_BATCH_CLAIMS
            {
                return Err(KingdomError::InvalidClaim);
            }
            for (chunk, _) in claims {
                claim_reach(peer, world, *chunk, None)?;
            }
            Some(ClaimActor::Player(peer.uuid))
        }
        KingdomAction::ClaimChunk { chunk, .. } | KingdomAction::ReleaseChunk { chunk, .. } => {
            Some(claim_reach(peer, world, *chunk, None)?)
        }
        KingdomAction::ClaimBotChunk { chunk, bot, .. }
        | KingdomAction::ReleaseBotChunk { chunk, bot, .. } => {
            Some(claim_reach(peer, world, *chunk, Some(*bot))?)
        }
        _ => {
            check_keep(position, peer, world)?;
            None
        }
    };
    if let Some((preview, preview_outcome)) = policy::plan(&action, peer, world, claim_actor, &[])?
    {
        if world.journal.gameplay().revision == u64::MAX {
            return Err(KingdomError::CapacityReached);
        }
        let domains = policy::domains(&action, peer, world, claim_actor)?;
        let (next, outcome) = if domains.is_empty() {
            // Re-use the exact preflighted transaction if no native domain exists.
            (preview, preview_outcome)
        } else {
            policy::plan(&action, peer, world, claim_actor, &domains)?
                .ok_or(KingdomError::InvalidClaim)?
        };
        if let KingdomOutcome::Founded { id } = outcome
            && next
                .get(id)
                .is_some_and(|kingdom| kingdom.kind == terra_core::kingdom::SettlementKind::Kingdom)
        {
            let body = world
                .bodies
                .get(peer.body)
                .map_err(|_| KingdomError::OutOfReach)?;
            let mut feet = body.translation.to_array().map(f64::from);
            feet[1] -= f64::from(terra_physics::character::PLAYER_HALF_HEIGHT);
            let radius = f64::from(terra_physics::character::PLAYER_RADIUS);
            let height = f64::from(terra_physics::character::PLAYER_HALF_HEIGHT);
            let players: Vec<_> = world
                .bodies
                .iter()
                .map(|body| {
                    let p = body.translation.to_array().map(f64::from);
                    [
                        [p[0] - radius, p[1] - height, p[2] - radius],
                        [p[0] + radius, p[1] + height, p[2] + radius],
                    ]
                })
                .collect();
            let positions = terra_world::kingdom_starter::plan(
                &terra_world::creatures::Habitat {
                    terrain: &world.terrain.0,
                    chunks: &world.chunks,
                    journal: &world.journal,
                },
                position,
                peer.uuid,
                feet,
                &players,
            )?;
            world
                .journal
                .gameplay_mut()
                .commit_founding_with_starters(next, id, positions)?;
            return Ok(outcome);
        }
        let game = world.journal.gameplay_mut();
        if next.revision != game.kingdoms.revision {
            game.kingdoms = next;
            game.revision += 1;
        }
        return Ok(outcome);
    }
    let gameplay = world.journal.gameplay_mut();
    if !inspect && gameplay.revision == u64::MAX {
        return Err(KingdomError::CapacityReached);
    }
    let previous = gameplay.kingdoms.revision;
    let previous_gameplay = gameplay.revision;
    let profile_name = world
        .identities
        .0
        .get(&peer.id)
        .map(|profile| profile.name.as_str())
        .unwrap_or("Player");
    let result = match action {
        KingdomAction::ClaimBatch { id, claims } => KingdomOutcome::ClaimsApplied {
            id,
            count: gameplay.kingdoms.claim_batch(
                id,
                &claims,
                claim_actor.ok_or(KingdomError::InvalidIdentity)?,
            )?,
        },
        KingdomAction::Inspect { .. } => KingdomOutcome::Inspected {
            kingdom: gameplay.kingdoms.active_at(position).map(|k| k.id),
        },
        KingdomAction::Found { name, .. } => KingdomOutcome::Founded {
            id: gameplay
                .kingdoms
                .found_with_name(peer.uuid, profile_name, position, &name)?,
        },
        KingdomAction::FoundSettlement {
            name,
            kind,
            heraldry,
            ..
        } => KingdomOutcome::Founded {
            id: gameplay.kingdoms.found_settlement(
                peer.uuid,
                profile_name,
                position,
                &name,
                kind,
                heraldry,
            )?,
        },
        KingdomAction::ClaimChunk { id, chunk, kind }
        | KingdomAction::ClaimBotChunk {
            id, chunk, kind, ..
        } => {
            gameplay.kingdoms.claim(
                id,
                chunk,
                kind,
                claim_actor.ok_or(KingdomError::InvalidIdentity)?,
            )?;
            KingdomOutcome::Claimed { id, chunk, kind }
        }
        KingdomAction::ReleaseChunk { id, chunk }
        | KingdomAction::ReleaseBotChunk { id, chunk, .. } => {
            gameplay.kingdoms.release(peer.uuid, id, chunk)?;
            KingdomOutcome::Released { id, chunk }
        }
        KingdomAction::ChangeAppearance { id, heraldry } => {
            gameplay
                .kingdoms
                .change_appearance(peer.uuid, id, heraldry)?;
            KingdomOutcome::AppearanceChanged { id }
        }
        KingdomAction::Upgrade { id } => {
            let level = gameplay.upgrade_kingdom_with_rules(
                peer.uuid,
                id,
                &world.config.world.kingdom_rules,
            )?;
            KingdomOutcome::Upgraded { id, level }
        }
        KingdomAction::Rename { id, name } => {
            gameplay.kingdoms.rename(peer.uuid, id, &name)?;
            KingdomOutcome::Renamed { id }
        }
        KingdomAction::Dissolve { id } => {
            gameplay.kingdoms.dissolve(peer.uuid, id)?;
            KingdomOutcome::Dissolved { id }
        }
        KingdomAction::Join { id } => {
            gameplay
                .kingdoms
                .join_with_name(peer.uuid, profile_name, id)?;
            KingdomOutcome::Joined { id }
        }
        KingdomAction::Leave { id } => {
            gameplay.kingdoms.leave(peer.uuid, id)?;
            KingdomOutcome::Left { id }
        }
        KingdomAction::SetRole { id, member, role } => {
            gameplay.kingdoms.set_role(peer.uuid, id, member, role)?;
            KingdomOutcome::RoleChanged { id, member, role }
        }
        KingdomAction::SetRelation {
            id,
            target,
            relation,
        } => {
            gameplay
                .kingdoms
                .set_relation(peer.uuid, id, target, relation)?;
            KingdomOutcome::RelationChanged {
                id,
                target,
                relation,
            }
        }
        KingdomAction::Kick { id, member } => {
            gameplay.kingdoms.kick(peer.uuid, id, member)?;
            KingdomOutcome::Kicked { id, member }
        }
    };
    if gameplay.kingdoms.revision != previous && gameplay.revision == previous_gameplay {
        gameplay.revision += 1;
    }
    Ok(result)
}

pub(super) fn broadcast(
    journal: Res<EditJournal>,
    profiles: Res<ConnectedProfiles>,
    mut runtime: ResMut<KingdomRuntime>,
    mut peers: Query<(&Peer, &mut MessageSender<ServerControl>), With<Connected>>,
) {
    runtime.0.retain(|id, _| profiles.0.contains_key(id));
    let kingdoms = &journal.gameplay().kingdoms;
    for (peer, mut sender) in &mut peers {
        if !peer.accepted {
            continue;
        }
        let state = runtime.0.entry(peer.id).or_default();
        if state.sent != Some(kingdoms.revision) {
            sender.send::<ControlChannel>(ServerControl::Kingdoms(kingdoms.clone()));
            state.sent = Some(kingdoms.revision);
        }
    }
}

pub(super) fn bound_keep(action: EditAction, journal: &EditJournal) -> bool {
    matches!(action, EditAction::Block { position, material, .. }
        if material != terra_core::castle::BERGFRIED && journal.gameplay().kingdoms.active_at(position).is_some())
}

/// Capabilities apply to explicit chunk claims and the loaded settlement footprint.
/// The direct chunk claim always wins over a captured/native settlement owner.
pub(super) fn permission_allowed(
    actor: PlayerUuid,
    permission: KingdomPermission,
    position: [i64; 3],
    world: &ControlWorld,
) -> bool {
    permission_at(actor, permission, position, &world.chunks, &world.journal)
}

pub(super) fn permission_at(
    actor: PlayerUuid,
    permission: KingdomPermission,
    position: [i64; 3],
    chunks: &ChunkStore,
    journal: &EditJournal,
) -> bool {
    terra_world::kingdom_access::can_use(
        &journal.gameplay().kingdoms,
        chunks,
        actor,
        permission,
        position,
    )
}

pub(super) fn edit_allowed(action: EditAction, peer: &Peer, world: &ControlWorld) -> bool {
    let permission = match action {
        EditAction::Block { material: 0, .. } | EditAction::Prop { .. } => KingdomPermission::Break,
        EditAction::Farm {
            action: terra_core::farming::FarmingAction::Harvest,
            ..
        } => KingdomPermission::Break,
        EditAction::Farm {
            action:
                terra_core::farming::FarmingAction::Refill
                | terra_core::farming::FarmingAction::FillBucket
                | terra_core::farming::FarmingAction::FillBileBucket
                | terra_core::farming::FarmingAction::FillLavaBucket,
            ..
        } => KingdomPermission::Inventories,
        EditAction::ToggleObject { position, .. }
            if world.chunks.block_at(position).is_some_and(|m| {
                terra_world::objects::is_door(m) || terra_world::objects::is_window(m)
            }) =>
        {
            KingdomPermission::Openings
        }
        EditAction::ToggleObject { .. }
        | EditAction::CollectConveyor { .. }
        | EditAction::HarvestTree { .. } => KingdomPermission::Inventories,
        _ => KingdomPermission::Build,
    };
    let allowed = |position| permission_allowed(peer.uuid, permission, position, world);
    let object_allowed = |position| {
        allowed(position)
            && world
                .journal
                .object_state(position)
                .and_then(|s| s.peer)
                .is_none_or(allowed)
            && terra_world::conveyors::span_at(&world.journal, position)
                .is_none_or(|span| span.cells().all(allowed))
            && world
                .chunks
                .block_at(position)
                .filter(|m| terra_world::objects::is_window(*m))
                .is_none_or(|material| {
                    let facing = world
                        .journal
                        .object_state(position)
                        .unwrap_or_default()
                        .facing;
                    terra_world::objects::windows::group(position, material, facing, |p| {
                        Some((
                            world.chunks.block_at(p)?,
                            world.journal.object_state(p)?.facing,
                        ))
                    })
                    .cells()
                    .all(allowed)
                })
    };
    if let EditAction::Block {
        position,
        expected,
        material,
    } = action
    {
        // Realm Territory permits ordinary terrain changes, while storage stays
        // private. Destroying a container publishes its real stacks as world
        // drops, so removal/replacement needs the same access as opening it.
        if expected != 0
            && material != expected
            && (matches!(expected, 29 | 59 | 99 | 156 | 161 | 162)
                || world.journal.container_at(position).is_some()
                || world.journal.attached_campfire_at(position).is_some())
        {
            let inventory_allowed =
                |cell| permission_allowed(peer.uuid, KingdomPermission::Inventories, cell, world);
            if !inventory_allowed(position)
                || world
                    .journal
                    .object_state(position)
                    .and_then(|state| state.peer)
                    .is_some_and(|cell| !inventory_allowed(cell))
            {
                return false;
            }
        }
        // Replacing an occupied voxel needs both permissions, including its paired half.
        if expected != 0 && material != 0 {
            let can_break = |p| permission_allowed(peer.uuid, KingdomPermission::Break, p, world);
            if !can_break(position)
                || world
                    .journal
                    .object_state(position)
                    .and_then(|s| s.peer)
                    .is_some_and(|p| !can_break(p))
            {
                return false;
            }
        }
        if expected != 0 && expected != material {
            for offset in [
                [-1, 0, 0],
                [1, 0, 0],
                [0, -1, 0],
                [0, 1, 0],
                [0, 0, -1],
                [0, 0, 1],
            ] {
                let neighbor =
                    std::array::from_fn(|axis| position[axis].saturating_add(offset[axis]));
                if world
                    .chunks
                    .block_at(neighbor)
                    .or_else(|| world.journal.block(neighbor))
                    == Some(40)
                    && world.journal.object_state(neighbor).is_some_and(|s| {
                        terra_world::torches::support(neighbor, s.facing) == Some(position)
                    })
                    && !permission_allowed(peer.uuid, KingdomPermission::Break, neighbor, world)
                {
                    return false;
                }
            }
            if terra_world::lanterns::dependents(&world.chunks, &world.journal, position)
                .into_iter()
                .any(|p| !permission_allowed(peer.uuid, KingdomPermission::Break, p, world))
            {
                return false;
            }
        }
    }
    match action {
        EditAction::ToggleObject { position, .. } => {
            object_allowed(position)
                && world.chunks.block_at(position).is_none_or(|material| {
                    let state = world.journal.object_state(position).unwrap_or_default();
                    terra_world::objects::doors::group(position, material, state, |p| {
                        Some((world.chunks.block_at(p)?, world.journal.object_state(p)?))
                    })
                    .is_none_or(|group| group.cells().all(allowed))
                })
        }
        EditAction::Block { position, .. }
        | EditAction::ConnectRail { position, .. }
        | EditAction::BuildRail { position, .. }
        | EditAction::SeedRail { position, .. }
        | EditAction::StretchRail { position, .. }
        | EditAction::Surface { position, .. }
        | EditAction::Farm { position, .. }
        | EditAction::RotateObject { position, .. }
        | EditAction::DyeConveyor { position, .. }
        | EditAction::DyeFabric { position, .. } => object_allowed(position),
        EditAction::PlaceProp { support, .. } => allowed(support),
        EditAction::Prop { section, id } => world
            .chunks
            .loaded
            .get(&section)
            .and_then(|s| s.props.iter().find(|p| p.id == id && !p.removed))
            .is_none_or(|p| allowed(p.position.map(|v| v.floor() as i64))),
        EditAction::ConnectConveyor { start, end, .. } => {
            object_allowed(start) && object_allowed(end)
        }
        EditAction::CollectConveyor { id } => world
            .journal
            .gameplay()
            .drops
            .iter()
            .find(|d| d.id == id)
            .is_none_or(|d| allowed(d.position.map(|v| v.floor() as i64))),
        EditAction::HarvestTree { id, .. } => world
            .journal
            .gameplay()
            .trees
            .get(id)
            .is_some_and(|tree| allowed(tree.root)),
    }
}

pub(super) fn layout_allowed(
    layout: &terra_world::placement::Layout,
    peer: &Peer,
    world: &ControlWorld,
) -> bool {
    layout.edits.iter().all(|edit| {
        edit_allowed(
            EditAction::Block {
                position: edit.position,
                expected: edit.expected_material,
                material: edit.material,
            },
            peer,
            world,
        )
    }) && layout
        .states
        .iter()
        .map(|(p, _)| p)
        .chain(layout.belts.iter().map(|(p, _)| p))
        .all(|position| permission_allowed(peer.uuid, KingdomPermission::Build, *position, world))
}

pub(super) fn inventory_allowed(
    action: crate::protocol::inventory::InventoryAction,
    peer: &Peer,
    world: &ControlWorld,
) -> bool {
    use crate::protocol::inventory::InventoryAction as A;
    let position = match action {
        A::OpenContainer { position }
        | A::OpenCampfire { position }
        | A::FillBottle { position, .. } => Some(position),
        A::CraftGrid { station, .. }
        | A::LoadSmeltingRecipe { station, .. }
        | A::LoadCrushingRecipe { station, .. }
        | A::SetMixerColor { station, .. }
        | A::SetFunnelFilter { station, .. }
        | A::SetDistributorFilter { station, .. }
        | A::SetTunnelRandom { station, .. }
        | A::SetBrewingRecipe { station, .. }
        | A::SetCampfireLit { station, .. } => Some(station),
        A::Recipe { station, .. } | A::CraftReserved { station, .. } => station,
        // Personal bag/cursor drops do not access a world inventory.
        A::DropSlot { .. } | A::DropCursor { .. } => None,
        _ => None,
    };
    position.is_none_or(|p| {
        let allowed =
            |cell| permission_allowed(peer.uuid, KingdomPermission::Inventories, cell, world);
        allowed(p)
            && world
                .journal
                .object_state(p)
                .and_then(|state| state.peer)
                .is_none_or(allowed)
    })
}
