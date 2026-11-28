//! Real cactus contacts share ordinary authoritative health, death and drop paths.
use super::*;
use terra_world::hazards;

#[derive(Default)]
pub(super) struct Cooldowns {
    next: f64,
    players: HashMap<terra_core::PlayerUuid, f64>,
    animals: HashMap<u64, f64>,
}

#[derive(bevy::ecs::system::SystemParam)]
pub(super) struct CactusWorld<'w, 's> {
    time: Res<'w, Time>,
    config: Res<'w, GameConfig>,
    runtime: Res<'w, ServerRuntime>,
    terrain: Res<'w, terra_world::streaming::WorldTerrain>,
    chunks: Res<'w, ChunkStore>,
    journal: ResMut<'w, EditJournal>,
    mounts: ResMut<'w, riding::Mounts>,
    peers: Query<'w, 's, &'static mut Peer, With<Connected>>,
    bodies: Query<'w, 's, &'static Transform, With<ServerCharacter>>,
}

pub(super) fn tick(mut world: CactusWorld, mut cooldowns: Local<Cooldowns>) {
    let now = world.time.elapsed_secs_f64();
    let thermal_time = world.runtime.tick as f64 / f64::from(world.config.server.tick_rate.max(1));
    if now < cooldowns.next {
        return;
    }
    cooldowns.next = now + 0.1;
    cooldowns.players.retain(|_, until| *until > now);
    cooldowns.animals.retain(|_, until| *until > now);
    let mut vents = terra_world::volcano::geyser::Vents::default();
    for mut peer in &mut world.peers {
        if !peer.synced
            || peer.vitals.dead()
            || peer.chat.teleport.is_some()
            || peer.game_mode(&world.config) != terra_core::GameMode::Survival
            || cooldowns.players.contains_key(&peer.uuid)
        {
            continue;
        }
        let Ok(body) = world.bodies.get(peer.body) else {
            continue;
        };
        let contact = hazards::cactus_contact(
            body.translation.to_array().map(f64::from),
            f64::from(terra_physics::character::PLAYER_RADIUS),
            f64::from(terra_physics::character::PLAYER_HALF_HEIGHT),
            |p| {
                world
                    .chunks
                    .block_at(p)
                    .map(|m| world.journal.block(p).unwrap_or(m))
            },
        );
        let radius = f64::from(terra_physics::character::PLAYER_RADIUS);
        let half = f64::from(terra_physics::character::PLAYER_HALF_HEIGHT);
        let mut feet = body.translation.to_array().map(f64::from);
        feet[1] -= half;
        let heat = terra_world::lava::thermal_damage(
            &world.chunks,
            &world.journal,
            feet,
            [radius * 2., half * 2., radius * 2.],
            thermal_time,
        )
        .max(vents.exposure(
            &world.chunks,
            &world.journal,
            &world.terrain.0,
            feet,
            half * 2.,
            thermal_time,
        ));
        if contact || heat > 0. {
            let health = peer.vitals.health;
            survival::environmental_damage(
                &mut peer,
                if heat > 0. {
                    heat
                } else {
                    hazards::CACTUS_DAMAGE
                },
                terra_core::GameMode::Survival,
                if heat > 0. {
                    terra_core::DamageCause::Fire
                } else {
                    terra_core::DamageCause::Cactus
                },
            );
            if peer.vitals.health < health {
                cooldowns
                    .players
                    .insert(peer.uuid, now + hazards::CACTUS_INTERVAL);
            }
        }
    }
    let hits: Vec<_> = world
        .journal
        .gameplay()
        .creatures
        .animals
        .values()
        .filter(|a| {
            a.health > 0.0
                && !cooldowns.animals.contains_key(&a.id)
                && world
                    .chunks
                    .block_at(a.position.map(|p| p.floor() as i64))
                    .is_some()
        })
        .filter(|a| {
            a.kind != terra_core::creatures::CreatureKind::SandWorm
                || terra_core::creatures::sandworm::emergence(a) >= 0.3
        })
        .filter_map(|a| {
            let heat = if a.kind == terra_core::creatures::CreatureKind::LavaWorm {
                0.
            } else {
                terra_world::lava::thermal_damage(
                    &world.chunks,
                    &world.journal,
                    a.position,
                    a.size(),
                    thermal_time,
                )
                .max(vents.exposure(
                    &world.chunks,
                    &world.journal,
                    &world.terrain.0,
                    a.position,
                    a.size()[1],
                    thermal_time,
                ))
            };
            if heat > 0. {
                return Some((a.id, heat));
            }
            terra_core::creatures::hitboxes::for_creature(a)
                .iter()
                .any(|part| {
                    hazards::cactus_hitbox_contact(*part, |p| {
                        world
                            .chunks
                            .block_at(p)
                            .map(|m| world.journal.block(p).unwrap_or(m))
                    })
                })
                .then_some((a.id, hazards::CACTUS_DAMAGE))
        })
        .collect();
    for (id, amount) in hits {
        if train_damage::hurt_creature(world.journal.gameplay_mut(), id, amount) {
            cooldowns.animals.insert(id, now + hazards::CACTUS_INTERVAL);
            if let Some(animal) = world.journal.gameplay().creatures.animals.get(&id) {
                for mount in world
                    .mounts
                    .active
                    .values_mut()
                    .filter(|m| m.horse.id == id)
                {
                    mount.horse = animal.clone();
                    mount.leave |= animal.health == 0.0;
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "server_cactus/tests.rs"]
mod tests;
