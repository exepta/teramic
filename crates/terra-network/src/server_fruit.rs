//! Authoritative per-apple harvesting; inventory and the persisted fruit mask commit together.
use super::*;
#[cfg(test)]
use terra_core::TreeStage;
use terra_core::{FruitPhase, ItemKind, TreeSpecies, kingdom::KingdomPermission};
use terra_world::{
    props::{PropKind, PropPose},
    targeting::WorldTarget,
};

struct Prepared {
    tree: terra_core::TreeRecord,
    bag: terra_core::Inventory,
    section: terra_voxel::SectionPos,
    prop: terra_world::props::PropId,
}

pub(super) fn check(command: EditCommand, peer: &Peer, world: &ControlWorld) -> EditOutcome {
    prepare(command, peer, world).map_or_else(|outcome| outcome, |_| EditOutcome::Applied)
}

fn prepare(
    command: EditCommand,
    peer: &Peer,
    world: &ControlWorld,
) -> Result<Prepared, EditOutcome> {
    let EditAction::HarvestTree {
        id,
        fruit_revision,
        slot,
    } = command.action
    else {
        return Err(EditOutcome::Invalid);
    };
    if !peer.synced
        || !peer.permissions.build
        || peer.vitals.dead()
        || peer.game_mode(&world.config) == terra_core::GameMode::Spectator
        || peer.chat.teleport.is_some()
        || peer.container.is_some()
        || command.look.iter().any(|v| !v.is_finite())
        || command.look[0].abs() > std::f32::consts::TAU + 0.001
        || command.look[1].abs() > 1.501
    {
        return Err(EditOutcome::Invalid);
    }
    let Some(tree) = world.journal.gameplay().trees.get(id) else {
        return Err(EditOutcome::Conflict);
    };
    if tree.species != TreeSpecies::Apple
        || !tree.stage.is_mature()
        || slot >= 8
        || tree
            .fruit
            .is_none_or(|fruit| fruit.phase != FruitPhase::Ripe || fruit.revision != fruit_revision)
        || world.chunks.block_at(tree.root) != Some(tree.species.wood())
        || tree.planted_by.is_none() && world.journal.block(tree.root).is_some()
    {
        return Err(EditOutcome::Conflict);
    }
    if !super::kingdoms::permission_allowed(
        peer.uuid,
        KingdomPermission::Inventories,
        tree.root,
        world,
    ) {
        return Err(EditOutcome::Invalid);
    }
    let Ok(body) = world.bodies.get(peer.body) else {
        return Err(EditOutcome::Invalid);
    };
    let eye = (body.translation + Vec3::Y * terra_physics::character::PLAYER_EYE_OFFSET)
        .to_array()
        .map(f64::from);
    let direction = (Quat::from_euler(EulerRot::YXZ, command.look[0], command.look[1], 0.0)
        * Vec3::NEG_Z)
        .to_array()
        .map(f64::from);
    let (_, target) = terra_world::targeting::pick_world_with_context(
        &world.chunks,
        &world.journal,
        &world.block_properties,
        eye,
        direction,
        PropPose::WeatherSwept {
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
    let Some(WorldTarget::Prop(hit)) = target else {
        return Err(EditOutcome::Occluded);
    };
    if hit.prop.kind != PropKind::AppleFruit
        || hit.prop.variant != 0
        || hit.prop.id != terra_world::fruit::prop_id(id, slot)
        || hit.distance > 6.0
    {
        return Err(EditOutcome::Occluded);
    }
    let anchor = terra_world::props::placement::support(&hit.prop);
    if world.chunks.block_at(anchor) != Some(195)
        || world.chunks.block_at([anchor[0], anchor[1] - 1, anchor[2]]) != Some(0)
    {
        return Err(EditOutcome::Conflict);
    }
    if !super::kingdoms::permission_allowed(
        peer.uuid,
        KingdomPermission::Inventories,
        hit.prop.position.map(|v| v.floor() as i64),
        world,
    ) {
        return Err(EditOutcome::Invalid);
    }
    let next_tree = terra_world::fruit::harvest_state(tree, fruit_revision, slot, world.clock.day)
        .map_err(|_| EditOutcome::Conflict)?;
    let game = world.journal.gameplay();
    let Some(bag) = game.inventories.get(&peer.uuid) else {
        return Err(EditOutcome::Invalid);
    };
    if game.revision == u64::MAX || game.trees.revision == u64::MAX || bag.revision > u64::MAX - 2 {
        return Err(EditOutcome::Conflict);
    }
    let mut next = bag.clone();
    if next.insert(ItemKind::Apple, 1) != 0 {
        return Err(EditOutcome::Conflict);
    }
    Ok(Prepared {
        tree: next_tree,
        bag: next,
        section: hit.prop.section,
        prop: hit.prop.id,
    })
}

pub(super) fn harvest(command: EditCommand, peer: &Peer, world: &mut ControlWorld) -> EditOutcome {
    let prepared = match prepare(command, peer, world) {
        Ok(prepared) => prepared,
        Err(outcome) => return outcome,
    };
    let game = world.journal.gameplay_mut();
    game.inventories.insert(peer.uuid, prepared.bag);
    game.trees.trees.insert(prepared.tree.id, prepared.tree);
    game.trees.changed();
    game.revision += 1;
    // This is derived picking geometry, not a persistent prop edit. Retire it
    // immediately so the next ray reaches the next apple before a fruit tick.
    if let Some(section) = world
        .chunks
        .bypass_change_detection()
        .loaded
        .get_mut(&prepared.section)
    {
        section.props.retain(|prop| prop.id != prepared.prop);
    }
    EditOutcome::Applied
}

#[cfg(test)]
#[path = "server_fruit_tests.rs"]
mod tests;
