//! Authoritative use of a real nest; never substitutes the prop's decorative item drop.
use super::*;

pub(super) fn harvest(
    section: terra_voxel::SectionPos,
    id: terra_world::props::PropId,
    look: [f32; 2],
    peer: &mut Peer,
    sender: &mut MessageSender<ServerControl>,
    world: &mut ControlWorld,
) {
    if !peer.synced
        || !peer.permissions.build
        || peer.vitals.dead()
        || peer.chat.teleport.is_some()
        || peer.game_mode(&world.config) == terra_core::GameMode::Spectator
        || world.mounts.active.contains_key(&peer.uuid)
        || peer.creature_action_at.elapsed() < Duration::from_millis(450)
        || look.iter().any(|v| !v.is_finite())
        || look[0].abs() > std::f32::consts::TAU + 0.001
        || look[1].abs() > 1.501
    {
        return;
    }
    peer.creature_action_at = Instant::now();
    let Ok(body) = world.bodies.get(peer.body) else {
        return;
    };
    let eye = (body.translation + Vec3::Y * terra_physics::character::PLAYER_EYE_OFFSET)
        .to_array()
        .map(f64::from);
    let direction = (Quat::from_euler(EulerRot::YXZ, look[0], look[1], 0.0) * Vec3::NEG_Z)
        .to_array()
        .map(f64::from);
    let (_, target) = terra_world::targeting::pick_world_with_context(
        &world.chunks,
        &world.journal,
        &world.block_properties,
        eye,
        direction,
        terra_world::props::PropPose::Rest,
    );
    let Some(terra_world::targeting::WorldTarget::Prop(hit)) = target else {
        return;
    };
    if hit.distance > 4.5 {
        return;
    }
    if !super::kingdoms::permission_allowed(
        peer.uuid,
        terra_core::kingdom::KingdomPermission::Inventories,
        hit.prop.position.map(|v| v.floor() as i64),
        world,
    ) {
        return;
    }
    if hit.prop.section != section
        || hit.prop.id != id
        || hit.prop.kind != terra_world::props::PropKind::BeeNest
        || hit.prop.removed
        || world.journal.prop_removed(section, id)
    {
        return;
    }
    for step in 0..=(hit.distance / 0.15).ceil() as usize {
        let p = std::array::from_fn(|i| {
            (eye[i] + direction[i] * (step as f64 * 0.15).min(hit.distance)).floor() as i64
        });
        if world.chunks.block_at(p).is_none() {
            return;
        }
    }
    // A loaded hand-placed nest may not yet have reached the one-second discovery pass.
    let state = world.journal.gameplay_mut();
    let honey = terra_core::ItemKind::Animal(terra_core::creatures::AnimalProduct::Honey);
    let full = state
        .inventories
        .get(&peer.uuid)
        .is_some_and(|bag| !bag.has_capacity(honey));
    let collected =
        terra_core::creatures::ecology::collect_honey(state, peer.uuid, id.0, hit.prop.position);
    sender.send::<ControlChannel>(ServerControl::InventoryResult {
        outcome: if collected {
            crate::protocol::inventory::InventoryOutcome::Applied
        } else if full {
            crate::protocol::inventory::InventoryOutcome::NoSpace
        } else {
            crate::protocol::inventory::InventoryOutcome::MissingIngredients
        },
        inventory: state
            .inventories
            .get(&peer.uuid)
            .cloned()
            .unwrap_or_default(),
    });
}
