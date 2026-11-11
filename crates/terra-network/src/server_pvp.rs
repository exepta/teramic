//! Player melee uses the same authority-owned cooldown, held tools and vitals as creature combat.
use super::*;
use terra_core::{DamageCause, GameMode, ItemKind};

#[derive(Clone, Copy)]
pub(super) struct Attack {
    pub attacker: PlayerId,
    pub target: PlayerId,
    pub look: [f32; 2],
}
#[derive(Resource, Default)]
pub(super) struct Attacks(pub VecDeque<Attack>);

struct Environment<'a> {
    config: &'a GameConfig,
    chunks: &'a ChunkStore,
    journal: &'a EditJournal,
    properties: &'a terra_core::block_properties::BlockPropertiesCatalog,
    weather: &'a terra_core::WorldWeather,
    positions: [Vec3; 2],
    other_players: &'a [Vec3],
}

fn strike(attacker: &mut Peer, victim: &mut Peer, look: [f32; 2], world: Environment<'_>) -> bool {
    if !world.config.world.game_rules.pvp
        || attacker.id == victim.id
        || !attacker.accepted
        || !attacker.synced
        || !victim.accepted
        || !victim.synced
        || attacker.vitals.dead()
        || victim.vitals.dead()
        || attacker.game_mode(world.config) == GameMode::Spectator
        || victim.game_mode(world.config) != GameMode::Survival
        || !attacker.permissions.build
        || attacker.creature_action_at.elapsed() < Duration::from_millis(450)
        || victim.survival.protection > 0.0
        || look.iter().any(|v| !v.is_finite())
        || look[0].abs() > std::f32::consts::TAU + 0.001
        || look[1].abs() > 1.501
    {
        return false;
    }
    let creative = attacker.game_mode(world.config) == GameMode::Creative;
    if !creative && attacker.vitals.stamina < 2.0 {
        return false;
    }
    let origin = world.positions[0] + Vec3::Y * terra_physics::character::PLAYER_EYE_OFFSET;
    let direction = Quat::from_euler(EulerRot::YXZ, look[0], look[1], 0.0) * Vec3::NEG_Z;
    let Some(hit) =
        terra_physics::character::melee_ray_distance(world.positions[1], origin, direction, 4.5)
    else {
        return false;
    };
    if world.other_players.iter().any(|body| {
        terra_physics::character::melee_ray_distance(*body, origin, direction, hit).is_some()
    }) {
        return false;
    }
    let eye = origin.to_array().map(f64::from);
    let direction = direction.to_array().map(f64::from);
    // Unknown streamed cells do not authorize an attack through missing terrain.
    for step in 0..=(hit / 0.15).ceil() as usize {
        let p = std::array::from_fn(|axis| {
            (eye[axis] + direction[axis] * (step as f64 * 0.15).min(hit)).floor() as i64
        });
        if world.chunks.block_at(p).is_none() {
            return false;
        }
    }
    let (_, obstruction) = terra_world::targeting::pick_world_with_context(
        world.chunks,
        world.journal,
        world.properties,
        eye,
        direction,
        terra_world::props::PropPose::WeatherSwept {
            frame: terra_world::props::weather_wind_frame(world.weather.wind()),
        },
    );
    if obstruction.is_some_and(|target| match target {
        terra_world::targeting::WorldTarget::Block(block) => block.distance < hit,
        terra_world::targeting::WorldTarget::Prop(prop) => prop.distance < hit,
    }) || world
        .journal
        .gameplay()
        .creatures
        .animals
        .values()
        .any(|animal| terra_world::creatures::ray_distance(animal, eye, direction, hit).is_some())
    {
        return false;
    }
    let held = world
        .journal
        .gameplay()
        .inventories
        .get(&attacker.uuid)
        .and_then(|bag| bag.slot(usize::from(attacker.held_slot?)))
        .map(|stack| stack.item);
    let damage = held
        .and_then(ItemKind::tool_kind)
        .map_or(1.0, terra_core::ToolKind::attack_damage)
        * attacker.vitals.effects.attack_multiplier();
    attacker.creature_action_at = Instant::now();
    attacker.mining = None;
    attacker.consumption = None;
    if !creative {
        attacker.vitals.stamina -= 2.0;
    }
    survival::environmental_damage(victim, damage, GameMode::Survival, DamageCause::Player);
    true
}

#[allow(clippy::too_many_arguments)]
pub(super) fn tick(
    mut queue: ResMut<Attacks>,
    config: Res<GameConfig>,
    chunks: Res<ChunkStore>,
    journal: Res<EditJournal>,
    properties: Res<terra_core::block_properties::BlockPropertiesCatalog>,
    weather: Res<terra_core::WorldWeather>,
    bodies: Query<&Transform, With<ServerCharacter>>,
    mut peers: Query<(Entity, &mut Peer), With<Connected>>,
) {
    for attack in queue.0.drain(..).take(128) {
        let attacker = peers
            .iter()
            .find_map(|(entity, peer)| (peer.id == attack.attacker).then_some(entity));
        let victim = peers
            .iter()
            .find_map(|(entity, peer)| (peer.id == attack.target).then_some(entity));
        let (Some(attacker), Some(victim)) = (attacker, victim) else {
            continue;
        };
        // The target ID does not authorize reaching through a nearer player.
        // Dead players and spectator cameras have no blocking combat body.
        let other_players: Vec<_> = peers
            .iter()
            .filter(|(entity, peer)| {
                *entity != attacker
                    && *entity != victim
                    && peer.synced
                    && !peer.vitals.dead()
                    && peer.game_mode(&config) != GameMode::Spectator
            })
            .filter_map(|(_, peer)| bodies.get(peer.body).ok().map(|body| body.translation))
            .collect();
        let Ok([(_, mut attacker), (_, mut victim)]) = peers.get_many_mut([attacker, victim])
        else {
            continue;
        };
        let (Ok(attacker_body), Ok(victim_body)) =
            (bodies.get(attacker.body), bodies.get(victim.body))
        else {
            continue;
        };
        strike(
            &mut attacker,
            &mut victim,
            attack.look,
            Environment {
                config: &config,
                chunks: &chunks,
                journal: &journal,
                properties: &properties,
                weather: &weather,
                positions: [attacker_body.translation, victim_body.translation],
                other_players: &other_players,
            },
        );
    }
    queue.0.clear();
}

#[cfg(test)]
#[path = "server_pvp_tests.rs"]
mod tests;
