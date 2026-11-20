//! Acid contacts use actual fluid height and ordinary health/death/drop paths.
use super::*;
use terra_core::bile;

#[derive(Default)]
pub(super) struct Cooldowns {
    next: f64,
    players: HashMap<terra_core::PlayerUuid, f64>,
    animals: HashMap<u64, f64>,
}

#[derive(bevy::ecs::system::SystemParam)]
pub(super) struct BileWorld<'w, 's> {
    time: Res<'w, Time>,
    config: Res<'w, GameConfig>,
    chunks: Res<'w, ChunkStore>,
    journal: ResMut<'w, EditJournal>,
    mounts: ResMut<'w, riding::Mounts>,
    peers: Query<'w, 's, &'static mut Peer, With<Connected>>,
    bodies: Query<'w, 's, &'static Transform, With<ServerCharacter>>,
}

fn near_acid(
    sections: &std::collections::HashSet<terra_voxel::SectionPos>,
    center: [f64; 3],
    radius: f64,
) -> bool {
    if center.iter().any(|p| !p.is_finite() || p.abs() > 990_000.) {
        return false;
    }
    let lo = center.map(|p| ((p - radius) / 16.).floor() as i64);
    let hi = center.map(|p| ((p + radius) / 16.).floor() as i64);
    (lo[2]..=hi[2]).any(|z| {
        (lo[1]..=hi[1]).any(|y| {
            (lo[0]..=hi[0]).any(|x| sections.contains(&terra_voxel::SectionPos::new(x, y, z)))
        })
    })
}

pub(super) fn tick(mut world: BileWorld, mut cooldowns: Local<Cooldowns>) {
    let now = world.time.elapsed_secs_f64();
    if now < cooldowns.next {
        return;
    }
    cooldowns.next = now + 0.1;
    cooldowns.players.retain(|_, until| *until > now);
    cooldowns.animals.retain(|_, until| *until > now);
    let acid_sections: std::collections::HashSet<_> = world
        .chunks
        .loaded
        .iter()
        .filter(|(p, s)| {
            ((s.data.may_contain(bile::FLUID) || s.data.may_contain(bile::BLOCK))
                && s.data
                    .blocks()
                    .iter()
                    .any(|id| matches!(*id, bile::FLUID | bile::BLOCK)))
                || world.journal.section_has_edited_material(**p, bile::FLUID)
                || world.journal.section_has_edited_material(**p, bile::BLOCK)
        })
        .map(|(p, _)| *p)
        .collect();
    if acid_sections.is_empty() {
        return;
    }

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
        if !near_acid(
            &acid_sections,
            body.translation.to_array().map(f64::from),
            2.,
        ) {
            continue;
        }
        let contact = terra_world::bile::hitbox_contact(
            terra_core::creatures::hitboxes::Hitbox {
                center: body.translation.to_array().map(f64::from),
                size: [
                    f64::from(terra_physics::character::PLAYER_RADIUS) * 2.,
                    f64::from(terra_physics::character::PLAYER_HALF_HEIGHT) * 2.,
                    f64::from(terra_physics::character::PLAYER_RADIUS) * 2.,
                ],
                axes: [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]],
            },
            &world.chunks,
            &world.journal,
        );
        if contact {
            let health = peer.vitals.health;
            survival::environmental_damage(
                &mut peer,
                bile::DAMAGE,
                terra_core::GameMode::Survival,
                terra_core::DamageCause::Acid,
            );
            if peer.vitals.health < health {
                cooldowns
                    .players
                    .insert(peer.uuid, now + bile::DAMAGE_INTERVAL);
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
                && near_acid(
                    &acid_sections,
                    a.position,
                    if a.kind == terra_core::creatures::CreatureKind::SandWorm {
                        26.0
                    } else {
                        (a.size().into_iter().sum::<f64>() * 2. + 2.).clamp(2., 48.)
                    },
                )
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
        .filter(|a| {
            terra_core::creatures::hitboxes::for_creature(a)
                .iter()
                .any(|part| terra_world::bile::hitbox_contact(*part, &world.chunks, &world.journal))
        })
        .map(|a| a.id)
        .collect();
    for id in hits {
        if train_damage::hurt_creature(world.journal.gameplay_mut(), id, bile::DAMAGE) {
            cooldowns.animals.insert(id, now + bile::DAMAGE_INTERVAL);
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
#[path = "server_bile/tests.rs"]
mod tests;
