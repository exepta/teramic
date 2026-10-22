use super::*;
use terra_core::creatures::{CreatureBehavior, CreatureKind, MAX_VISIBLE_CREATURES};
use terra_world::creatures::{ACTIVE_RADIUS, Habitat, distance};
#[cfg(test)]
#[path = "population_tests.rs"]
mod population_tests;

#[derive(Default)]
pub(super) struct Schedule {
    elapsed: f32,
    scan: usize,
    search_cursor: Option<u64>,
    last_step: std::collections::HashMap<u64, f32>,
    bee_habitats: std::collections::HashSet<[i64; 2]>,
    ecology_elapsed: f32,
    navigation: terra_world::creatures::navigation::Navigation,
}

fn habitat_offset(index: usize) -> [i64; 2] {
    static OFFSETS: std::sync::LazyLock<Vec<[i64; 2]>> = std::sync::LazyLock::new(|| {
        let mut offsets: Vec<_> = (-5..=5)
            .flat_map(|z| (-5..=5).map(move |x| [x, z]))
            .collect();
        offsets.sort_unstable_by_key(|[x, z]| (x * x + z * z, *z, *x));
        offsets
    });
    OFFSETS[index % OFFSETS.len()]
}

fn natural_habitat_enabled(home: [i64; 2], seed: u64) -> bool {
    let Some(id) = terra_core::creatures::creature_id(home, 0) else {
        return false;
    };
    // Independent occupancy bits preserve species, herd identities and saved animals.
    let mut value = id ^ seed.rotate_left(23) ^ 0xd4e1_2c77_0b69_8a35;
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    (value ^ (value >> 31)) % 5 < 3
}

fn natural_near(
    population: &terra_core::creatures::CreaturePopulation,
    position: [f64; 3],
) -> usize {
    population
        .animals
        .values()
        .filter(|animal| {
            animal.health > 0.0
                && !population.owners.contains_key(&animal.id)
                && distance(position, animal.position) <= ACTIVE_RADIUS.powi(2)
        })
        .count()
}

/// Creature positions are foot-centred; player positions are capsule centres.
fn jelly_contact(animal: &terra_core::creatures::Creature, player: [f64; 3]) -> bool {
    if !animal.kind.is_jelly() || animal.health <= 0.0 {
        return false;
    }
    let size = animal.size();
    let radius = f64::from(terra_physics::character::PLAYER_RADIUS);
    let half_height = f64::from(terra_physics::character::PLAYER_HALF_HEIGHT);
    (animal.position[0] - player[0]).abs() < size[0] * 0.5 + radius
        && (animal.position[2] - player[2]).abs() < size[2] * 0.5 + radius
        && animal.position[1] < player[1] + half_height
        && animal.position[1] + size[1] > player[1] - half_height
}

fn poison_from_jelly(peer: &mut Peer, touching: bool, mode: terra_core::GameMode) {
    if touching
        && mode == terra_core::GameMode::Survival
        && !peer.vitals.dead()
        && peer.survival.protection <= 0.0
    {
        peer.vitals
            .effects
            .apply(terra_core::EffectKind::Poison, 10.0, 1);
    }
}

fn search_due(a: &terra_core::creatures::Creature, dt: f32, pending_navigation: bool) -> bool {
    if a.kind.is_aquatic()
        || matches!(
            a.behavior,
            CreatureBehavior::Flee | CreatureBehavior::Sleep | CreatureBehavior::InNest
        )
        || (a.kind == CreatureKind::Bee && a.timer > dt)
    {
        return false;
    }
    if pending_navigation
        && matches!(
            a.behavior,
            CreatureBehavior::Wander | CreatureBehavior::SeekFood | CreatureBehavior::SeekWater
        )
    {
        return true;
    }
    if a.kind == CreatureKind::Bear {
        if pending_navigation
            && matches!(
                a.behavior,
                CreatureBehavior::Hunt
                    | CreatureBehavior::Attack
                    | CreatureBehavior::SeekWater
                    | CreatureBehavior::Wander
            )
        {
            return true;
        }
        // Combat and climbing do not scan props/water. Reserving a resource
        // search for them every tick would starve the shared eight-search budget.
        if matches!(
            a.behavior,
            CreatureBehavior::Hunt | CreatureBehavior::Attack | CreatureBehavior::Climb
        ) {
            return false;
        }
    }
    a.needs.goal.is_none() && a.needs.search_in <= dt
}

fn grant_searches(mut eligible: Vec<u64>, cursor: &mut Option<u64>) -> Vec<u64> {
    eligible.sort_unstable();
    eligible.dedup();
    let split = cursor.map_or(0, |last| eligible.partition_point(|id| *id <= last));
    let grants: Vec<_> = eligible[split..]
        .iter()
        .chain(&eligible[..split])
        .copied()
        .take(8)
        .collect();
    if let Some(&last) = grants.last() {
        *cursor = Some(last);
    }
    grants
}

#[derive(SystemParam)]
pub(super) struct SimulationWorld<'w> {
    chunks: ResMut<'w, ChunkStore>,
    terrain: Res<'w, terra_world::streaming::WorldTerrain>,
    journal: ResMut<'w, EditJournal>,
}

pub(super) fn tick(
    time: Res<Time>,
    runtime: Res<ServerRuntime>,
    config: Res<terra_core::GameConfig>,
    mut schedule: Local<Schedule>,
    mut peers: Query<(&mut Peer, &mut MessageSender<CreatureStates>), With<Connected>>,
    bodies: Query<&Transform, With<ServerCharacter>>,
    world: SimulationWorld,
) {
    let SimulationWorld {
        mut chunks,
        terrain,
        mut journal,
    } = world;
    schedule.elapsed += time.delta_secs().min(0.1);
    if schedule.elapsed < 0.1 {
        return;
    }
    let dt = schedule.elapsed.min(0.2);
    schedule.elapsed = 0.0;
    let _scope = terra_core::profiling::ScopeTimer::new("creatures.server_tick");
    let players: Vec<_> = peers
        .iter()
        .filter(|(p, _)| p.synced && !p.vitals.dead())
        .filter_map(|(p, _)| {
            bodies
                .get(p.body)
                .ok()
                .map(|b| b.translation.to_array().map(f64::from))
        })
        .take(128)
        .collect();
    if players.is_empty() {
        return;
    }
    let vulnerable: Vec<_> = peers
        .iter()
        .filter(|(p, _)| {
            p.synced && !p.vitals.dead() && p.game_mode(&config) == terra_core::GameMode::Survival
        })
        .filter_map(|(p, _)| {
            bodies
                .get(p.body)
                .ok()
                .map(|b| (p.uuid, b.translation.to_array().map(f64::from)))
        })
        .take(128)
        .collect();
    let targets: Vec<_> = vulnerable.iter().map(|(_, p)| *p).collect();
    let mut population = std::mem::take(&mut journal.gameplay_mut().creatures);
    let limit = usize::from(
        config
            .server
            .natural_mob_limit
            .unwrap_or(config.world.natural_mob_limit),
    );
    // Two deterministic habitat probes per tick, round-robin over all players.
    for _ in 0..2 {
        let index = schedule.scan;
        schedule.scan = schedule.scan.wrapping_add(1);
        let player = players[index % players.len()];
        let mut natural = natural_near(&population, player);
        let offset = habitat_offset(index / players.len());
        let home = [
            (player[0].floor() as i64).div_euclid(32) + offset[0],
            (player[2].floor() as i64).div_euclid(32) + offset[1],
        ];
        if !natural_habitat_enabled(home, config.world.seed.0)
            || natural.saturating_add(4) > limit
            || (home[0] as f64 * 32.0 + 16.0 - player[0])
                .hypot(home[1] as f64 * 32.0 + 16.0 - player[2])
                > ACTIVE_RADIUS
        {
            continue;
        }
        terra_world::creatures::spawn_cell(
            &mut population,
            &Habitat {
                terrain: &terrain.0,
                chunks: &chunks,
                journal: &journal,
            },
            home,
        );
        natural = natural_near(&population, player);
        if schedule.bee_habitats.len() < terra_core::creatures::MAX_SPAWN_CELLS
            && natural.saturating_add(4) <= limit
            && !schedule.bee_habitats.contains(&home)
        {
            match terra_world::creatures::bees::spawn(
                &mut population,
                &terrain.0,
                &mut chunks,
                &mut journal,
                home,
            ) {
                Ok(true) => {
                    schedule.bee_habitats.insert(home);
                }
                Ok(false) => {}
                Err(error) => tracing::warn!(%error, "Bee habitat could not be committed"),
            }
        }
    }
    let ids: Vec<_> = population
        .animals
        .values()
        .filter(|a| {
            a.health > 0.0
                && players
                    .iter()
                    .any(|p| distance(*p, a.position) <= ACTIVE_RADIUS.powi(2))
        })
        .map(|a| a.id)
        .collect();
    let mut deaths = Vec::new();
    let prey = terra_world::creatures::bears::PreyIndex::new(
        ids.iter().filter_map(|id| population.animals.get(id)),
    );
    let mut bear_effects = Vec::new();
    population.ecology.tick(dt);
    schedule.ecology_elapsed += dt;
    if schedule.ecology_elapsed >= 1.0 {
        schedule.ecology_elapsed %= 1.0;
        terra_world::creatures::ecology::discover(
            &mut population.ecology,
            &Habitat {
                terrain: &terrain.0,
                chunks: &chunks,
                journal: &journal,
            },
        );
    }
    let phase = journal.clock.map_or(0.5, |clock| clock.phase);
    let night = journal
        .clock
        .is_some_and(|clock| !(0.25..0.7916666667).contains(&clock.phase));
    let now = time.elapsed_secs();
    schedule
        .last_step
        .retain(|id, _| population.animals.contains_key(id));
    let mut due: Vec<_> = ids
        .iter()
        .filter_map(|&id| {
            let a = population.animals.get(&id)?;
            let distance = players
                .iter()
                .map(|p| distance(*p, a.position))
                .fold(f64::INFINITY, f64::min);
            let cadence = if distance < 64.0_f64.powi(2) {
                0.095
            } else {
                0.195
            };
            let elapsed = now - *schedule.last_step.entry(id).or_insert(now - 0.2);
            (elapsed >= cadence).then_some((id, elapsed, elapsed / cadence))
        })
        .collect();
    // Nearby creatures run at 10 Hz, distant ones at 5 Hz with the full elapsed
    // simulation time. The former rotating 128 cap silently halved movement speed.
    due.sort_unstable_by(|a, b| b.2.total_cmp(&a.2).then(a.0.cmp(&b.0)));
    // Movement keeps its urgency order; only the eight expensive search slots
    // rotate so low IDs with repeatedly failed routes cannot starve the herd.
    let eligible = due
        .iter()
        .take(512)
        .filter_map(|&(id, elapsed, _)| {
            let animal = population.animals.get(&id)?;
            (search_due(
                animal,
                elapsed.min(0.25),
                schedule.navigation.needs_search(id, animal.age),
            ) && chunks
                .block_at(animal.position.map(|value| value.floor() as i64))
                .is_some())
            .then_some(id)
        })
        .collect();
    let searches = grant_searches(eligible, &mut schedule.search_cursor);
    for (id, elapsed, _) in due.into_iter().take(512) {
        let dt = elapsed.min(0.25);
        schedule.last_step.insert(id, now);
        if let Some(a) = population.animals.get_mut(&id) {
            let search = searches.contains(&id);
            let sleeping = night
                && (a.kind == CreatureKind::Bee
                    || terra_world::creatures::ecology::sleep_at(a.id, phase));
            let effect = if a.kind.is_aquatic() && a.kind != CreatureKind::Fish {
                let effect = terra_world::creatures::aquatics::step(
                    a,
                    &Habitat {
                        terrain: &terrain.0,
                        chunks: &chunks,
                        journal: &journal,
                    },
                    &targets,
                    &prey,
                    dt,
                );
                if effect != terra_world::creatures::bears::Effect::None {
                    bear_effects.push((id, effect));
                }
                terra_world::creatures::Forage::None
            } else if a.kind == CreatureKind::Bear {
                let effect = terra_world::creatures::bears::step_with_navigation(
                    a,
                    &Habitat {
                        terrain: &terrain.0,
                        chunks: &chunks,
                        journal: &journal,
                    },
                    &targets,
                    &prey,
                    dt,
                    search,
                    sleeping,
                    Some(&mut schedule.navigation),
                );
                if effect != terra_world::creatures::bears::Effect::None {
                    bear_effects.push((id, effect));
                }
                terra_world::creatures::Forage::None
            } else {
                terra_world::creatures::ecology::step_with_navigation(
                    a,
                    &mut population.ecology,
                    &Habitat {
                        terrain: &terrain.0,
                        chunks: &chunks,
                        journal: &journal,
                    },
                    &players,
                    dt,
                    search,
                    sleeping,
                    Some(&mut schedule.navigation),
                )
            };
            if effect != terra_world::creatures::Forage::None
                && let Err(error) =
                    terra_world::creatures::apply_forage(effect, a, &mut chunks, &mut journal)
            {
                tracing::warn!(%error, "Animal forage could not be committed");
            }
            if a.health <= 0.0 {
                deaths.push((a.position, a.product()));
            }
        }
    }
    for (id, effect) in bear_effects {
        match effect {
            terra_world::creatures::bears::Effect::CatchFish { id: fish } => {
                let habitat = Habitat {
                    terrain: &terrain.0,
                    chunks: &chunks,
                    journal: &journal,
                };
                if population
                    .animals
                    .get(&id)
                    .is_some_and(|a| a.kind.is_shark())
                {
                    terra_world::creatures::aquatics::catch_fish(
                        &mut population,
                        id,
                        fish,
                        &habitat,
                    );
                } else {
                    terra_world::creatures::bears::catch_fish(&mut population, id, fish, &habitat);
                }
            }
            terra_world::creatures::bears::Effect::StrikePlayer { index, damage } => {
                if let Some((uuid, _)) = vulnerable.get(index)
                    && let Some((mut peer, _)) = peers.iter_mut().find(|(p, _)| p.uuid == *uuid)
                {
                    let mode = peer.game_mode(&config);
                    survival::animal_attack(&mut peer, damage, mode);
                }
            }
            terra_world::creatures::bears::Effect::None => {}
        }
    }
    terra_world::creatures::navigation::separate(
        &mut population,
        &Habitat {
            terrain: &terrain.0,
            chunks: &chunks,
            journal: &journal,
        },
        &ids,
        dt,
    );
    let jellies: Vec<_> = ids
        .iter()
        .filter_map(|id| population.animals.get(id))
        .filter(|animal| animal.kind.is_jelly() && animal.health > 0.0)
        .collect();
    if !jellies.is_empty() {
        for (mut peer, _) in &mut peers {
            if !peer.synced || peer.chat.teleport.is_some() {
                continue;
            }
            let Ok(body) = bodies.get(peer.body) else {
                continue;
            };
            let position = body.translation.to_array().map(f64::from);
            let touching = chunks
                .block_at(position.map(|value| value.floor() as i64))
                .is_some()
                && jellies.iter().any(|animal| jelly_contact(animal, position));
            let mode = peer.game_mode(&config);
            poison_from_jelly(&mut peer, touching, mode);
        }
    }
    journal.gameplay_mut().creatures = population;
    for (position, product) in deaths {
        if let Some(product) = product {
            journal
                .gameplay_mut()
                .drop_one(position, terra_core::ItemKind::Animal(product));
        }
    }
    for (peer, mut sender) in &mut peers {
        if !peer.synced {
            continue;
        }
        let Ok(body) = bodies.get(peer.body) else {
            continue;
        };
        let position = body.translation.to_array().map(f64::from);
        let mut creatures: Vec<_> = journal
            .gameplay()
            .creatures
            .animals
            .values()
            .filter(|a| a.health > 0.0 && distance(position, a.position) < ACTIVE_RADIUS.powi(2))
            .collect();
        creatures.sort_unstable_by(|a, b| {
            distance(position, a.position)
                .total_cmp(&distance(position, b.position))
                .then(a.id.cmp(&b.id))
        });
        sender.send::<SnapshotChannel>(CreatureStates {
            tick: runtime.tick,
            ecology: journal.gameplay().creatures.ecology.nearby(position),
            creatures: creatures
                .into_iter()
                .take(MAX_VISIBLE_CREATURES)
                .cloned()
                .collect(),
        });
    }
}

pub(super) fn interact(
    id: u64,
    feed: bool,
    look: [f32; 2],
    peer: &mut Peer,
    world: &mut ControlWorld,
) {
    if peer.vitals.dead()
        || peer.game_mode(&world.config) == terra_core::GameMode::Spectator
        || !peer.permissions.build
        || peer.creature_action_at.elapsed() < Duration::from_millis(450)
        || look.iter().any(|v| !v.is_finite())
        || look[0].abs() > std::f32::consts::TAU + 0.001
        || look[1].abs() > 1.501
    {
        return;
    }
    let Ok(body) = world.bodies.get(peer.body) else {
        return;
    };
    let eye = (body.translation + Vec3::Y * terra_physics::character::PLAYER_EYE_OFFSET)
        .to_array()
        .map(f64::from);
    let direction = (Quat::from_euler(EulerRot::YXZ, look[0], look[1], 0.0) * Vec3::NEG_Z)
        .to_array()
        .map(f64::from);
    let Some(animal) = world.journal.gameplay().creatures.animals.get(&id) else {
        return;
    };
    let Some(hit) = terra_world::creatures::ray_distance(animal, eye, direction, 4.5) else {
        return;
    };
    // Every traversed cell must be loaded; use the existing precise terrain picker.
    for step in 0..=(hit / 0.15).ceil() as usize {
        let p = std::array::from_fn(|i| {
            (eye[i] + direction[i] * (step as f64 * 0.15).min(hit)).floor() as i64
        });
        if world.chunks.block_at(p).is_none() {
            return;
        }
    }
    let (block, _) = terra_world::targeting::pick_world_with_context(
        &world.chunks,
        &world.journal,
        &world.block_properties,
        eye,
        direction,
        terra_world::props::PropPose::WeatherSwept {
            frame: terra_world::props::weather_wind_frame(world.weather.wind()),
        },
    );
    if block.is_some_and(|b| b.distance < hit) {
        return;
    }
    if world
        .journal
        .gameplay()
        .creatures
        .animals
        .values()
        .any(|other| {
            other.id != id
                && terra_world::creatures::ray_distance(other, eye, direction, hit).is_some()
        })
    {
        return;
    }
    let kind = animal.kind;
    let product = animal.product();
    let position = animal.position;
    if feed {
        let food = if kind == CreatureKind::Pig {
            terra_core::ItemKind::Prop(terra_core::ResourcePropKind::Mushroom)
        } else {
            terra_core::ItemKind::Prop(terra_core::ResourcePropKind::Grass)
        };
        if kind.is_aquatic()
            || matches!(kind, CreatureKind::Bee | CreatureKind::Bear)
            || (animal.health >= kind.health() && animal.needs.food >= kind.food())
        {
            return;
        }
        let Some(slot) = peer.held_slot else {
            return;
        };
        if !world
            .journal
            .gameplay_mut()
            .consume_from(peer.uuid, food, Some(slot))
        {
            return;
        }
        if let Some(a) = world.journal.gameplay_mut().creatures.animals.get_mut(&id) {
            a.health = (a.health + 4.0).min(kind.health());
            a.needs.food = (a.needs.food + 6.0).min(kind.food());
            a.behavior = CreatureBehavior::Graze;
            a.timer = 4.0;
        }
    } else {
        let creative = peer.game_mode(&world.config) == terra_core::GameMode::Creative;
        if !creative && peer.vitals.stamina < 2.0 {
            return;
        }
        let damage = match gameplay::held_tool(peer, world) {
            Some(terra_core::ItemKind::Tool(terra_core::ToolKind::WoodSword)) => 3.0,
            Some(terra_core::ItemKind::Tool(_)) => 5.0,
            _ => 1.0,
        } * peer.vitals.effects.attack_multiplier();
        let dies = animal.health <= damage;
        let needed = if kind.product().is_none() {
            0
        } else if matches!(kind, CreatureKind::Sheep | CreatureKind::Cow) {
            2
        } else {
            1
        };
        if dies
            && (world.journal.gameplay().drops.len() + needed > terra_core::MAX_WORLD_DROPS
                || world.journal.gameplay().next_drop > u64::MAX - needed as u64)
        {
            return;
        }
        if !creative {
            peer.vitals.stamina -= 2.0;
        }
        if let Some(a) = world.journal.gameplay_mut().creatures.animals.get_mut(&id) {
            a.health = (a.health - damage).max(0.0);
            a.behavior = if dies {
                CreatureBehavior::Dead
            } else {
                CreatureBehavior::Flee
            };
            a.timer = 5.0;
            a.yaw = ((eye[0] - position[0]).atan2(eye[2] - position[2])) as f32;
        }
        if dies && let Some(product) = product {
            world.journal.gameplay_mut().drop_stack(
                position,
                terra_core::ItemStack {
                    item: terra_core::ItemKind::Animal(product),
                    count: if kind == CreatureKind::Fish { 1 } else { 2 },
                },
                [0.0, 1.8, 0.0],
                0.35,
            );
            if kind == CreatureKind::Sheep {
                world.journal.gameplay_mut().drop_one(
                    position,
                    terra_core::ItemKind::Animal(terra_core::creatures::AnimalProduct::Wool),
                );
            }
            if kind == CreatureKind::Cow {
                world.journal.gameplay_mut().drop_one(
                    position,
                    terra_core::ItemKind::Animal(terra_core::creatures::AnimalProduct::Leather),
                );
            }
        }
    }
    peer.creature_action_at = Instant::now();
}
