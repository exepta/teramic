use super::*;
use terra_core::{GameMode, ItemKind, ToolKind};
use terra_world::placement::Layout;

fn plan(command: EditCommand, peer: &Peer, world: &ControlWorld) -> Result<Layout, EditOutcome> {
    let EditAction::RotateObject {
        position,
        expected,
        facing,
        reverse,
    } = command.action
    else {
        return Err(EditOutcome::Invalid);
    };
    if !peer.permissions.build
        || peer.vitals.dead()
        || peer.game_mode(&world.config) == GameMode::Spectator
        || command.look.iter().any(|value| !value.is_finite())
        || command.look[0].abs() > std::f32::consts::TAU + 0.001
        || command.look[1].abs() > 1.501
        || position.iter().any(|value| value.unsigned_abs() > 990_000)
        || !terra_world::wrench::supported(expected)
    {
        return Err(EditOutcome::Invalid);
    }
    let source = command.source.ok_or(EditOutcome::Invalid)?;
    let bag = world
        .journal
        .gameplay()
        .inventories
        .get(&peer.uuid)
        .ok_or(EditOutcome::Invalid)?;
    if peer.held_slot != Some(source.slot)
        || bag.revision != source.revision
        || bag
            .slot(usize::from(source.slot))
            .is_none_or(|stack| stack.item != ItemKind::Tool(ToolKind::Wrench))
    {
        return Err(EditOutcome::Conflict);
    }
    if world.chunks.block_at(position) != Some(expected)
        || world.journal.revision(global_to_section(position).0) != command.revision
        || world
            .journal
            .object_state(position)
            .unwrap_or_default()
            .facing
            != facing
    {
        return Err(EditOutcome::Conflict);
    }
    let body = world
        .bodies
        .get(peer.body)
        .map_err(|_| EditOutcome::Invalid)?;
    let eye = body.translation + Vec3::Y * PLAYER_EYE_OFFSET;
    let direction =
        Quat::from_euler(EulerRot::YXZ, command.look[0], command.look[1], 0.0) * Vec3::NEG_Z;
    let (_, selected) = pick_world_with_context(
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
    if !matches!(selected, Some(WorldTarget::Block(hit)) if hit.position == position) {
        return Err(EditOutcome::Occluded);
    }
    let plan = terra_world::wrench::propose(&world.chunks, &world.journal, position, reverse)
        .map_err(|_| EditOutcome::Invalid)?;
    if !crate::server::kingdoms::layout_allowed(&plan, peer, world) {
        return Err(EditOutcome::Invalid);
    }
    if world.sleep.as_deref().is_some_and(|sleep| {
        plan.edits.iter().any(|edit| {
            terra_core::beds::is_bed(edit.expected_material) && sleep.occupies_bed(edit.position)
        })
    }) {
        return Err(EditOutcome::Invalid);
    }
    terra_world::placement::environment(
        &plan,
        &world.chunks,
        &world.journal,
        world.config.world.bounded_world,
    )
    .map_err(|_| EditOutcome::Invalid)?;
    let boxes = terra_world::placement::boxes(
        &plan,
        &world.chunks,
        &world.journal,
        &world.block_properties,
    );
    if world.bodies.iter().any(|body| {
        let half = Vec3::new(PLAYER_RADIUS, PLAYER_HALF_HEIGHT, PLAYER_RADIUS);
        boxes.iter().any(|bounds| {
            bounds.intersects(
                (body.translation - half).to_array().map(f64::from),
                (body.translation + half).to_array().map(f64::from),
            )
        })
    }) {
        return Err(EditOutcome::Invalid);
    }
    Ok(plan)
}

pub(super) fn check(command: EditCommand, peer: &Peer, world: &ControlWorld) -> EditOutcome {
    plan(command, peer, world).map_or_else(|outcome| outcome, |_| EditOutcome::Applied)
}

pub(super) fn apply(command: EditCommand, peer: &Peer, world: &mut ControlWorld) -> EditOutcome {
    let plan = match plan(command, peer, world) {
        Ok(plan) => plan,
        Err(outcome) => return outcome,
    };
    match terra_world::wrench::commit(&mut world.chunks, &mut world.journal, &plan) {
        Ok(true) => EditOutcome::Applied,
        Ok(false) => EditOutcome::Conflict,
        Err(error) => {
            tracing::warn!(%error, "Wrench rotation rejected");
            EditOutcome::Invalid
        }
    }
}
