//! One bounded, server-resolved lightning event for all clients. Never generate
//! terrain for a strike: unknown columns remain cloud-to-cloud flashes.
use super::*;
use bevy::math::DVec3;
use terra_core::{DamageCause, LightningEvent};

fn exposed_surface(chunks: &ChunkStore, x: i64, z: i64) -> Option<[i64; 3]> {
    let column = chunks.prepared_column([x.div_euclid(16), z.div_euclid(16)])?;
    let generated_top = (column.surface_at(x, z)?.height - 0.5).floor() as i64;
    let mut sections: Vec<_> = chunks
        .loaded
        .keys()
        .filter(|section| section.x == x.div_euclid(16) && section.z == z.div_euclid(16))
        .copied()
        .collect();
    sections.sort_unstable_by_key(|section| std::cmp::Reverse(section.y));
    for section in sections {
        for local_y in (0..16).rev() {
            let y = section.y * 16 + local_y;
            if y < generated_top {
                return None;
            }
            let material = chunks.block_at([x, y, z])?;
            if terra_voxel::MaterialId::from_raw(material)
                .is_some_and(terra_voxel::MaterialId::is_solid)
            {
                return Some([x, y, z]);
            }
        }
    }
    None
}

#[derive(bevy::ecs::system::SystemParam)]
pub(super) struct LightningWorld<'w, 's> {
    weather: ResMut<'w, terra_core::WorldWeather>,
    config: Res<'w, GameConfig>,
    clock: Res<'w, terra_core::WorldClock>,
    chunks: Res<'w, ChunkStore>,
    journal: ResMut<'w, EditJournal>,
    peers: Query<'w, 's, &'static mut Peer, With<Connected>>,
    bodies: Query<'w, 's, &'static Transform, With<ServerCharacter>>,
    surfaces: MessageWriter<'w, terra_world::environment::SurfaceConditionRequest>,
}

pub(super) fn tick(mut world: LightningWorld, mut previous: Local<Option<u64>>) {
    if world.clock.paused {
        return;
    }
    let Some((id, ground, offset)) = world.weather.lightning_slot(world.config.world.seed.0) else {
        return;
    };
    if *previous == Some(id)
        || world
            .weather
            .latest_lightning
            .is_some_and(|event| event.id == id)
    {
        return;
    }
    *previous = Some(id);
    let mut players: Vec<_> = world
        .peers
        .iter()
        .filter(|peer| peer.synced)
        .filter_map(|peer| {
            world
                .bodies
                .get(peer.body)
                .ok()
                .map(|body| (peer.id.0, body.translation))
        })
        .collect();
    players.sort_unstable_by_key(|(id, _)| *id);
    if players.is_empty() {
        return;
    }
    let center = players[id as usize % players.len()].1.as_dvec3();
    let x = (center.x + offset[0]).floor() as i64;
    let z = (center.z + offset[1]).floor() as i64;
    let hit = if ground {
        exposed_surface(&world.chunks, x, z)
    } else {
        None
    };
    let position = hit.map_or(
        [
            center.x + offset[0],
            center.y.max(36.0) + 90.0,
            center.z + offset[1],
        ],
        |block| {
            [
                block[0] as f64 + 0.5,
                block[1] as f64 + 1.02,
                block[2] as f64 + 0.5,
            ]
        },
    );
    let event = LightningEvent {
        id,
        at: world.weather.elapsed_total,
        position,
        ground_strike: hit.is_some(),
    };
    world.weather.latest_lightning = Some(event);
    world.journal.bypass_change_detection().weather = Some(*world.weather);
    let Some(block) = hit else {
        return;
    };
    for mut peer in &mut world.peers {
        if !peer.synced {
            continue;
        }
        let Ok(body) = world.bodies.get(peer.body) else {
            continue;
        };
        let distance = body
            .translation
            .as_dvec3()
            .distance(DVec3::from_array(position));
        if distance < 3.5
            && exposed_surface(
                &world.chunks,
                body.translation.x.floor() as i64,
                body.translation.z.floor() as i64,
            )
            .is_some_and(|roof| roof[1] as f32 <= body.translation.y + 0.5)
        {
            let mode = peer.game_mode(&world.config);
            survival::environmental_damage(
                &mut peer,
                (1.0 - distance as f32 / 3.5) * 65.0,
                mode,
                DamageCause::Lightning,
            );
        }
    }
    // Mutate creature state only for an actual nearby hit, not every flash.
    let nearby: Vec<_> = world
        .journal
        .gameplay()
        .creatures
        .animals
        .values()
        .filter(|animal| {
            animal.health > 0.0
                && DVec3::from_array(animal.position).distance_squared(DVec3::from_array(position))
                    < 12.25
        })
        .map(|animal| (animal.id, animal.position))
        .collect();
    for (id, at) in nearby {
        let mut drop = None;
        let mut forest_loot = [None, None];
        if exposed_surface(&world.chunks, at[0].floor() as i64, at[2].floor() as i64)
            .is_some_and(|roof| roof[1] as f64 <= at[1] + 0.5)
            && let Some(animal) = world.journal.gameplay_mut().creatures.animals.get_mut(&id)
        {
            animal.health = (animal.health - 6.0).max(0.0);
            animal.behavior = if animal.health == 0.0 {
                drop = animal.product();
                forest_loot = terra_core::creatures::drops::forest_loot(
                    animal.kind,
                    animal.variant,
                    animal.id,
                );
                terra_core::creatures::CreatureBehavior::Dead
            } else {
                terra_core::creatures::CreatureBehavior::Flee
            };
            animal.timer = 5.0;
        }
        for stack in forest_loot.into_iter().flatten() {
            world
                .journal
                .gameplay_mut()
                .drop_stack(at, stack, [0.0, 1.8, 0.0], 0.35);
        }
        if let Some(product) = drop {
            world
                .journal
                .gameplay_mut()
                .drop_one(at, terra_core::ItemKind::Animal(product));
        }
    }
    // Existing fire limits, authority checks, and propagation apply unchanged.
    if world
        .chunks
        .block_at(block)
        .and_then(terra_voxel::MaterialId::from_raw)
        .is_some_and(|material| {
            matches!(
                material,
                terra_voxel::MaterialId::Wood
                    | terra_voxel::MaterialId::WeatheredWood
                    | terra_voxel::MaterialId::SpruceLog
                    | terra_voxel::MaterialId::Leaves
                    | terra_voxel::MaterialId::AutumnLeaves
                    | terra_voxel::MaterialId::SilverLeaves
                    | terra_voxel::MaterialId::CherryLeaves
                    | terra_voxel::MaterialId::WhiteCherryLeaves
            )
        })
    {
        let mut condition = world.journal.surface_condition(block);
        condition.fire = 0.75;
        world
            .surfaces
            .write(terra_world::environment::SurfaceConditionRequest {
                position: block,
                condition,
            });
    }
}
