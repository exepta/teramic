//! Eruptions are simulated once per occupied volcanic region. Every observer
//! receives the same launch sequence and authoritative impact mask.
use super::*;
use bevy::math::DVec3;
use std::collections::BTreeMap;
use terra_core::creatures::hitboxes::Hitbox;
use terra_world::{
    streaming::WorldTerrain,
    volcano::{EJECTA_COUNT, Eruption},
};

#[derive(Default)]
pub(super) struct Volcanism {
    next: f64,
    publish: f64,
    previous: BTreeMap<[i64; 2], Eruption>,
}

#[derive(SystemParam)]
pub(super) struct EruptionWorld<'w, 's> {
    time: Res<'w, Time>,
    terrain: Res<'w, WorldTerrain>,
    chunks: ResMut<'w, ChunkStore>,
    properties: Res<'w, terra_core::block_properties::BlockPropertiesCatalog>,
    config: Res<'w, GameConfig>,
    journal: ResMut<'w, EditJournal>,
    mounts: ResMut<'w, riding::Mounts>,
    peers: Query<
        'w,
        's,
        (
            Entity,
            &'static mut Peer,
            &'static mut MessageSender<ServerControl>,
        ),
        With<Connected>,
    >,
    bodies: Query<'w, 's, &'static Transform, With<ServerCharacter>>,
}

#[derive(Clone, Copy)]
enum Body {
    Player(Entity),
    Creature(u64),
}

fn sweep(world: &mut EruptionWorld, event: &mut Eruption, since: f32, bodies: &[(Hitbox, Body)]) {
    for i in 0..EJECTA_COUNT {
        let bit = 1_u64 << i;
        if event.landed & bit != 0 {
            continue;
        }
        let stone = event.fragment(i);
        let mut age = since.max(stone.delay);
        while age < event.age {
            let end = (age + 0.07).min(event.age);
            let (Some(start), Some(finish)) = (stone.position(age), stone.position(end)) else {
                break;
            };
            age = end;
            let delta = DVec3::from_array(finish) - DVec3::from_array(start);
            let length = delta.length();
            if length < 1e-6 {
                continue;
            }
            let direction = (delta / length).to_array();
            let ground = terra_voxel::blocks::raycast_blocks(start, direction, length, |p| {
                let raw = world.journal.block(p).or_else(|| world.chunks.block_at(p));
                let solid = match raw {
                    Some(raw) => terra_voxel::MaterialId::from_raw(raw)
                        .is_some_and(|m| m.is_solid() || m == terra_voxel::MaterialId::Lava),
                    // High arcing fragments pass above streamed sections. Use
                    // prepared ground only; never generate a chunk for a stone.
                    None => world
                        .chunks
                        .surface_column_at(p[0], p[2])
                        .is_some_and(|c| (p[1] as f64) + 0.5 < c.height),
                };
                Some(u16::from(solid))
            });
            let reach = ground.map_or(length, |hit| hit.distance);
            let target = bodies
                .iter()
                .filter_map(|(bounds, target)| {
                    bounds
                        .ray_distance(start, direction, reach)
                        .map(|dist| (dist, *target))
                })
                .min_by(|a, b| a.0.total_cmp(&b.0));
            if let Some((_, body)) = target {
                let damage = if stone.molten { 23. } else { 30. };
                match body {
                    Body::Player(peer) => {
                        if let Ok((_, mut peer, _)) = world.peers.get_mut(peer) {
                            let mode = peer.game_mode(&world.config);
                            survival::environmental_damage(
                                &mut peer,
                                damage,
                                mode,
                                if stone.molten {
                                    terra_core::DamageCause::Fire
                                } else {
                                    terra_core::DamageCause::FallingMineral
                                },
                            );
                        }
                    }
                    Body::Creature(id) => {
                        if train_damage::hurt_creature(world.journal.gameplay_mut(), id, damage)
                            && let Some(animal) =
                                world.journal.gameplay().creatures.animals.get(&id)
                        {
                            for mount in world
                                .mounts
                                .active
                                .values_mut()
                                .filter(|mount| mount.horse.id == id)
                            {
                                mount.horse = animal.clone();
                                mount.leave |= animal.health == 0.;
                            }
                        }
                    }
                }
            } else if let Some(hit) = ground {
                if hit.normal[1] == 1 && (stone.molten || i % 4 == 3) {
                    terra_world::lava::deposit_ejecta(
                        &mut world.chunks,
                        &mut world.journal,
                        &world.properties,
                        hit.position,
                        stone.molten,
                    );
                }
                if stone.molten && world.config.world.game_rules.fire_spread {
                    for off in [
                        [0, 0, 0],
                        [-1, 0, 0],
                        [1, 0, 0],
                        [0, 1, 0],
                        [0, 0, -1],
                        [0, 0, 1],
                    ] {
                        let cell = std::array::from_fn(|i| hit.position[i] + off[i]);
                        terra_world::lava::ignite(&world.chunks, &mut world.journal, cell);
                    }
                }
            } else {
                continue;
            }
            event.landed |= bit;
            break;
        }
    }
}

pub(super) fn tick(mut world: EruptionWorld, mut state: Local<Volcanism>) {
    let now = world.time.elapsed_secs_f64();
    if now < state.next {
        return;
    }
    state.next = now + 0.08;
    let mut events = BTreeMap::new();
    let mut observers = Vec::new();
    let mut bodies = Vec::new();
    for (entity, peer, _) in &world.peers {
        if !peer.synced || peer.chat.teleport.is_some() {
            continue;
        }
        let Ok(body) = world.bodies.get(peer.body) else {
            continue;
        };
        let p = body.translation.as_dvec3();
        if let Some(vent) = world.terrain.0.eruptive_vent(p.x, p.z)
            && let Some(event) = vent.eruption(now)
        {
            let key = [vent.origin[0].floor() as i64, vent.origin[2].floor() as i64];
            events.insert(key, event);
            observers.push((entity, key));
        }
        if !peer.vitals.dead() && peer.game_mode(&world.config) == terra_core::GameMode::Survival {
            bodies.push((
                Hitbox {
                    center: p.to_array(),
                    size: [0.82, 1.92, 0.82],
                    axes: [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]],
                },
                Body::Player(entity),
            ));
        }
    }
    if !events.is_empty() {
        for animal in world
            .journal
            .gameplay()
            .creatures
            .animals
            .values()
            .filter(|a| {
                a.health > 0.
                    && events.values().any(|e| {
                        (a.position[0] - e.vent.origin[0]).hypot(a.position[2] - e.vent.origin[2])
                            < 120.
                    })
            })
        {
            bodies.extend(
                terra_core::creatures::hitboxes::for_creature(animal)
                    .iter()
                    .map(|b| (*b, Body::Creature(animal.id))),
            );
        }
    }
    for (key, event) in &mut events {
        let since = if let Some(old) = state
            .previous
            .get(key)
            .filter(|old| old.cycle == event.cycle)
        {
            event.landed = old.landed;
            old.age
        } else {
            // On entering an ongoing eruption, only newly launched fragments
            // can hurt anything. Avoid replaying old impacts into a loaded save.
            for i in 0..EJECTA_COUNT {
                if event.fragment(i).delay < event.age - 0.10 {
                    event.landed |= 1 << i;
                }
            }
            event.age - 0.1
        };
        sweep(&mut world, event, since, &bodies);
    }
    if now >= state.publish {
        state.publish = now + 0.30;
        for (who, key) in observers {
            if let Ok((_, _, mut sender)) = world.peers.get_mut(who) {
                sender.send::<ControlChannel>(ServerControl::Eruption(events[&key]));
            }
        }
    }
    state.previous = events;
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::SystemState;
    use terra_core::creatures::{Creature, CreatureBehavior, CreatureKind};
    use terra_world::volcano::Vent;

    #[test]
    fn a_molten_fragment_hits_the_first_body_or_ignites_the_first_voxel_only_once() {
        let mut event = Eruption {
            vent: Vent {
                origin: [8., 8., 8.],
                seed: 367,
            },
            cycle: 0,
            age: 0.,
            landed: 0,
        };
        let index = (0..EJECTA_COUNT)
            .find(|i| event.fragment(*i).molten)
            .unwrap();
        let stone = event.fragment(index);
        let since = stone.delay + 1.0;
        event.age = since + 0.06;
        let pos = stone.position(event.age - 0.01).unwrap();
        let cell = pos.map(|n| n.floor() as i64);
        let (section, local) = terra_voxel::global_to_section(cell);
        let mut data = terra_voxel::BlockSection::empty(section).unwrap();
        data.set_block(local, terra_voxel::MaterialId::Wood as u16)
            .unwrap();
        let mut chunks = ChunkStore::default();
        chunks.loaded.insert(
            section,
            terra_world::streaming::LoadedSection {
                data,
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            },
        );
        let mut world = World::new();
        world.insert_resource(Time::<()>::default());
        world.insert_resource(WorldTerrain(
            terra_world::TerraGenerator::new(Default::default()).unwrap(),
        ));
        world.init_resource::<GameConfig>();
        world.init_resource::<terra_core::block_properties::BlockPropertiesCatalog>();
        world.init_resource::<riding::Mounts>();
        world.init_resource::<EditJournal>();
        world.insert_resource(chunks);
        let kind = CreatureKind::Pig;
        let mut pig = Creature {
            id: 42,
            kind,
            home: [0, 0],
            position: pos,
            velocity: [0.; 3],
            yaw: 0.,
            health: kind.health(),
            behavior: CreatureBehavior::Rest,
            timer: 0.,
            age: 0.,
            variant: 0,
            needs: Default::default(),
        };
        pig.health = 100.;
        world
            .resource_mut::<EditJournal>()
            .gameplay_mut()
            .creatures
            .animals
            .insert(pig.id, pig);
        let mut state = SystemState::<EruptionWorld>::new(&mut world);
        let mut sys = state.get_mut(&mut world).unwrap();
        let front = Hitbox {
            center: stone.position(since).unwrap(),
            size: [0.85; 3],
            axes: [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]],
        };
        event.landed = !(1 << index);
        sweep(&mut sys, &mut event, since, &[(front, Body::Creature(42))]);
        assert_eq!(event.landed, u64::MAX);
        assert!(sys.journal.surface_condition(cell).fire == 0.);
        let health = sys.journal.gameplay().creatures.animals[&42].health;
        assert!(health < 100.);
        sweep(&mut sys, &mut event, since, &[(front, Body::Creature(42))]);
        assert_eq!(sys.journal.gameplay().creatures.animals[&42].health, health);
        event.landed = !(1 << index);
        sweep(&mut sys, &mut event, since, &[]);
        assert!(sys.journal.surface_condition(cell).fire > 0.);
        assert_eq!(event.landed, u64::MAX);
    }
}
