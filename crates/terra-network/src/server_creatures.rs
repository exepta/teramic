use super::*;
use terra_core::creatures::{CreatureBehavior, CreatureKind, MAX_VISIBLE_CREATURES};
use terra_world::creatures::{ACTIVE_RADIUS, Habitat, distance};

#[derive(Resource, Default)]
pub(super) struct InspectionTargets(pub std::collections::HashMap<terra_core::PlayerUuid, u64>);
#[cfg(test)]
#[path = "population_tests.rs"]
mod population_tests;
#[path = "server_creatures/poultry.rs"]
pub(super) mod poultry;
#[path = "server_creatures/worm_encounters.rs"]
mod worm_encounters;
#[path = "server_creatures/worm_spit.rs"]
mod worm_spit;

/// Only doors opened by residents are closed automatically. Player-operated
/// doors retain their state; paired changes use the normal server transaction.
#[derive(Default)]
pub(super) struct ResidentDoors {
    pending: std::collections::BTreeMap<[i64; 3], ManagedDoor>,
}

struct ManagedDoor {
    material: u16,
    facing: u8,
    cells: Vec<([i64; 3], terra_world::edits::BlockObjectState)>,
    deadline: f32,
}

pub(super) struct ResidentDoorWorld<'a> {
    pub chunks: &'a mut ChunkStore,
    pub journal: &'a mut EditJournal,
    pub properties: &'a terra_core::block_properties::BlockPropertiesCatalog,
    pub population: &'a terra_core::creatures::CreaturePopulation,
    pub players: &'a [[f64; 3]],
}

fn resident_door_group(
    chunks: &ChunkStore,
    journal: &EditJournal,
    position: [i64; 3],
) -> Option<terra_world::objects::doors::Group> {
    let material = chunks
        .block_at(position)
        .or_else(|| journal.block(position))?;
    let state = journal.object_state(position)?;
    terra_world::objects::doors::group(position, material, state, |p| {
        Some((
            chunks.block_at(p).or_else(|| journal.block(p))?,
            journal.object_state(p)?,
        ))
    })
}

impl ResidentDoors {
    pub(super) fn update(
        &mut self,
        now: f32,
        requests: &std::collections::BTreeSet<[i64; 3]>,
        world: ResidentDoorWorld<'_>,
    ) {
        let ResidentDoorWorld {
            chunks,
            journal,
            properties,
            population,
            players,
        } = world;
        let requests: std::collections::BTreeSet<_> = requests
            .iter()
            .map(|&door| {
                resident_door_group(chunks, journal, door).map_or(door, |group| group.anchor)
            })
            .collect();
        for &door in &requests {
            let Some(group) = resident_door_group(chunks, journal, door) else {
                continue;
            };
            // A manually open leaf already provides passage. Do not acquire and
            // later close it merely because its neighbor was still closed.
            if group
                .opened
                .into_iter()
                .take(usize::from(group.width))
                .any(|open| open)
                || self.pending.len() >= 1024 && !self.pending.contains_key(&door)
            {
                continue;
            }
            let Some(state) = journal.object_state(door) else {
                continue;
            };
            if furniture::toggle_with_players(
                door,
                group.material,
                state.facing,
                chunks,
                journal,
                properties,
                players,
            ) != Ok(true)
            {
                continue;
            }
            let cells = group
                .cells()
                .filter_map(|p| journal.object_state(p).map(|state| (p, state)))
                .collect();
            self.pending.insert(
                door,
                ManagedDoor {
                    material: group.material,
                    facing: state.facing ^ 4,
                    cells,
                    deadline: now + 3.0,
                },
            );
        }
        self.pending.retain(|&door, managed| {
            // Retain the complete expected transaction across partial unloads.
            // Any player override, replacement or changed grouping retires it.
            for &(cell, state) in &managed.cells {
                match chunks.block_at(cell) {
                    None => return true,
                    Some(material) if material != managed.material => return false,
                    _ => {}
                }
                if journal.object_state(cell) != Some(state) {
                    return false;
                }
            }
            let Some(group) = resident_door_group(chunks, journal, door) else {
                return false;
            };
            if !group.cells().eq(managed.cells.iter().map(|(p, _)| *p)) {
                return false;
            }
            if requests.contains(&door) {
                managed.deadline = now + 3.0;
                return true;
            }
            if now < managed.deadline {
                return true;
            }
            let occupied = managed
                .cells
                .iter()
                .filter(|(_, state)| !state.secondary)
                .any(|(cell, _)| {
                    let center = [
                        cell[0] as f64 + 0.5,
                        cell[1] as f64 + 1.0,
                        cell[2] as f64 + 0.5,
                    ];
                    players
                        .iter()
                        .any(|&p| distance(p, center) < 2.5_f64.powi(2))
                        || population.animals.values().any(|animal| {
                            if animal.health <= 0.0 {
                                return false;
                            }
                            let size = animal.size();
                            let radius = 2.0 + size[0].max(size[2]) * 0.5;
                            (animal.position[0] - center[0]).hypot(animal.position[2] - center[2])
                                < radius
                                && animal.position[1] < cell[1] as f64 + 3.0
                                && animal.position[1] + size[1] > cell[1] as f64 - 1.0
                        })
                });
            if occupied {
                managed.deadline = now + 3.0;
                return true;
            }
            match furniture::toggle_with_players(
                door,
                managed.material,
                managed.facing,
                chunks,
                journal,
                properties,
                players,
            ) {
                Ok(_) => false,
                Err(_) => {
                    managed.deadline = now + 0.5;
                    true
                }
            }
        });
    }
}

/// Real hits let several players participate in a boss fight without showing it to spectators.
#[derive(Resource, Default)]
pub(super) struct DragonChallengers(
    std::collections::BTreeMap<(u64, terra_core::PlayerUuid), Instant>,
);

#[derive(Default)]
pub(super) struct Schedule {
    elapsed: f32,
    scan: usize,
    search_cursor: Option<u64>,
    last_step: std::collections::HashMap<u64, f32>,
    bee_habitats: std::collections::HashSet<[i64; 2]>,
    spider_habitats: std::collections::HashMap<[i64; 2], f32>,
    ecology_elapsed: f32,
    spirit_aura_elapsed: f32,
    breeding_elapsed: f32,
    breeding_cursor: Option<u64>,
    bee_breeding_cursor: Option<u64>,
    navigation: terra_world::creatures::navigation::Navigation,
    stalkers: terra_world::creatures::stalker::Stalkers,
    spider_cycles: terra_world::creatures::spiders::cycle::Cycles,
    residents: terra_world::creatures::villagers::Residents,
    resident_elapsed: f32,
    resident_doors: ResidentDoors,
    dragons: std::collections::BTreeMap<u64, terra_world::creatures::dragons::Encounter>,
    worm_spits: worm_spit::Projectiles,
    worm_encounters: worm_encounters::Encounters,
}

/// The long worm's visible rings can cross the interest boundary before its root.
fn in_interest(a: &terra_core::creatures::Creature, player: [f64; 3]) -> bool {
    if a.kind.is_dragon() {
        return distance(player, a.position)
            <= (terra_world::creatures::dragons::ROAM_RADIUS + 32.).powi(2);
    }
    let extent = if matches!(a.kind, CreatureKind::SandWorm | CreatureKind::LavaWorm) {
        24.0
    } else {
        a.size()[0].max(a.size()[2]) * 0.5
    };
    distance(player, a.position) <= (ACTIVE_RADIUS + extent).powi(2)
}
fn active_near(a: &terra_core::creatures::Creature, player: [f64; 3]) -> bool {
    in_interest(a, player)
        || a.kind.is_dragon()
            && (player[0] - (a.home[0] * 32 + 16) as f64)
                .hypot(player[2] - (a.home[1] * 32 + 16) as f64)
                < ACTIVE_RADIUS
}
fn needs_distant_tick(a: &terra_core::creatures::Creature, chunks: &ChunkStore) -> bool {
    match a.kind {
        CreatureKind::SandWorm => terra_core::creatures::sandworm::committed(a.behavior),
        // Keep lake patrols alive while their terrain is resident without requesting chunks.
        CreatureKind::LavaWorm => {
            let [x, y, z] = a.position.map(|v| v.floor() as i64);
            chunks.block_at([x, y - 1, z]).is_some()
        }
        _ => false,
    }
}

fn molten_worm_hit(peer: &mut Peer, damage: f32, mode: terra_core::GameMode) {
    let health = peer.vitals.health;
    survival::animal_attack(peer, damage, mode);
    if peer.vitals.health < health && !peer.vitals.dead() {
        peer.vitals
            .effects
            .apply(terra_core::EffectKind::OnFire, 5., 1);
    }
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
                && animal.kind != CreatureKind::Villager
                && !population.owners.contains_key(&animal.id)
                && distance(position, animal.position) <= ACTIVE_RADIUS.powi(2)
        })
        .count()
}

fn snapshot_ecology(
    population: &terra_core::creatures::CreaturePopulation,
    visible: &[terra_core::creatures::Creature],
    position: [f64; 3],
) -> terra_core::creatures::ecology::Ecology {
    let mut ecology = population.ecology.nearby(position);
    let mut bees = std::collections::BTreeMap::new();
    // Visible offspring need their colony identity even beyond the 80 m ecology
    // radius or when lower natural IDs fill the nearby metadata limit.
    for animal in visible.iter().take(MAX_VISIBLE_CREATURES) {
        if animal.kind == CreatureKind::Bee
            && let Some(state) = population.ecology.bees.get(&animal.id)
        {
            bees.insert(animal.id, state.clone());
        }
    }
    for (id, state) in std::mem::take(&mut ecology.bees) {
        if bees.len() == MAX_VISIBLE_CREATURES {
            break;
        }
        bees.entry(id).or_insert(state);
    }
    ecology.bees = bees;
    ecology
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

fn spider_bite(
    peer: &mut Peer,
    bite: terra_world::creatures::spiders::Bite,
    mode: terra_core::GameMode,
) {
    if mode != terra_core::GameMode::Survival
        || peer.vitals.dead()
        || peer.survival.protection > 0.0
    {
        return;
    }
    survival::animal_attack(peer, bite.damage, mode);
    if !peer.vitals.dead() {
        peer.vitals.effects.apply(
            terra_core::EffectKind::Poison,
            bite.poison_seconds,
            bite.poison_level,
        );
    }
}

fn spin_web(
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
    terrain: &terra_world::TerraGenerator,
    web: terra_world::creatures::spiders::WebRequest,
) -> Option<terra_core::creatures::taming::SpiderDen> {
    let prop = web.prop()?;
    let section = chunks.loaded.get(&prop.section)?;
    let live: Vec<_> = section
        .props
        .iter()
        .filter(|p| {
            p.kind == terra_world::props::PropKind::SpiderWeb
                && !p.removed
                && !journal.prop_removed(p.section, p.id)
                && terra_world::props::webs::within_world_bounds(p, terrain.config().bounded_world)
        })
        .collect();
    if let Some(existing) = live
        .iter()
        .find(|p| {
            distance(p.position, prop.position) < 2.0_f64.powi(2)
                && (!terra_world::props::webs::is_construction(&prop)
                    || terra_world::props::webs::is_construction(p))
        })
        .or_else(|| {
            (live.len() >= 4)
                .then(|| {
                    live.iter()
                        .filter(|p| {
                            !terra_world::props::webs::is_construction(&prop)
                                || terra_world::props::webs::is_construction(p)
                        })
                        .min_by(|a, b| {
                            distance(a.position, prop.position)
                                .total_cmp(&distance(b.position, prop.position))
                        })
                })
                .flatten()
        })
    {
        return Some(terra_core::creatures::taming::SpiderDen {
            anchor: terra_world::props::webs::support(existing),
            web_id: existing.id.0,
        });
    }
    // Spinning is persistent but strictly bounded, including previously placed webs.
    if section
        .props
        .iter()
        .filter(|p| {
            p.kind == terra_world::props::PropKind::SpiderWeb
                && !p.removed
                && !journal.prop_removed(p.section, p.id)
        })
        .count()
        >= 4
        || section.props.iter().any(|p| {
            p.kind == terra_world::props::PropKind::SpiderWeb
                && !p.removed
                && distance(p.position, prop.position) < 2.0_f64.powi(2)
                && (!terra_world::props::webs::is_construction(&prop)
                    || terra_world::props::webs::is_construction(p))
        })
        || !terra_world::props::webs::fits_in_world(
            &prop,
            terrain.config().bounded_world,
            |position| {
                let raw = chunks.block_at(position)?;
                let block = journal.block(position).unwrap_or(raw);
                Some(
                    if block == 0
                        && chunks
                            .fluid_cell_at(terrain, journal, position, block)
                            .amount
                            > 0
                    {
                        terra_voxel::MaterialId::FluidWater as u16
                    } else {
                        block
                    },
                )
            },
        )
    {
        return None;
    }
    match journal.place_prop(prop) {
        Ok(true) => {
            if let Some(section) = chunks.loaded.get_mut(&prop.section) {
                section.props.push(prop);
            }
            Some(terra_core::creatures::taming::SpiderDen {
                anchor: web.anchor,
                web_id: web.id,
            })
        }
        Ok(false) => None,
        Err(error) => {
            tracing::warn!(%error, "Spider web could not be committed");
            None
        }
    }
}

fn constructed_den(
    chunks: &ChunkStore,
    journal: &EditJournal,
    den: terra_core::creatures::taming::SpiderDen,
) -> bool {
    let owner = terra_voxel::global_to_section(den.anchor).0;
    !journal.prop_removed(owner, terra_world::props::PropId(den.web_id))
        && chunks.loaded.get(&owner).is_some_and(|section| {
            section.props.iter().any(|p| {
                p.id.0 == den.web_id && !p.removed && terra_world::props::webs::is_construction(p)
            })
        })
}

fn den_rebuild_request(
    mut request: terra_world::creatures::spiders::WebRequest,
    prior: Option<terra_core::creatures::taming::SpiderDen>,
    journal: &EditJournal,
) -> terra_world::creatures::spiders::WebRequest {
    if let Some(prior) = prior
        && (request.separation != 0
            || journal.prop_removed(
                terra_voxel::global_to_section(prior.anchor).0,
                terra_world::props::PropId(prior.web_id),
            ))
    {
        request.id = terra_world::creatures::spiders::replacement_den_id(request.id, prior.web_id);
    }
    request
}

fn search_due(a: &terra_core::creatures::Creature, dt: f32, pending_navigation: bool) -> bool {
    if matches!(
        a.kind,
        CreatureKind::Butterfly
            | CreatureKind::SandWorm
            | CreatureKind::LavaWorm
            | CreatureKind::SpiritGloom
            | CreatureKind::Raven
            | CreatureKind::BloodWisp
    ) {
        // Air steering never performs an expensive ground/food path search.
        return false;
    }
    if a.kind.is_dragon() {
        return false;
    }
    if matches!(a.kind, CreatureKind::StoneGolem | CreatureKind::Stalker)
        || terra_world::creatures::forest::hostile(a.kind)
    {
        return pending_navigation || a.needs.search_in <= dt;
    }
    if a.kind == CreatureKind::Villager {
        return pending_navigation || a.needs.search_in <= dt;
    }
    if a.kind.is_spider() {
        return a.behavior != CreatureBehavior::Descend
            && (pending_navigation
                || !matches!(
                    a.behavior,
                    CreatureBehavior::Hunt | CreatureBehavior::Attack
                ) && a.needs.search_in <= dt);
    }
    if a.kind == CreatureKind::Fish {
        // Swimming itself needs no graph. Only stopped fish with an expired
        // retry timer compete for the shared eight-search grant.
        return a.behavior == CreatureBehavior::Swim
            && a.needs.search_in <= dt
            && a.velocity.iter().all(|speed| speed.abs() < 0.05);
    }
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

fn refresh_spirit_regeneration(vitals: &mut terra_core::SurvivalVitals) {
    // Never downgrade a potion, add levels, or extend a long buff indefinitely.
    // This also bounds duration if a data pack changes the effect stacking policy.
    if vitals
        .effects
        .get(terra_core::EffectKind::Regeneration)
        .is_none_or(|effect| effect.level == 1 && effect.remaining_seconds <= 1.0)
    {
        vitals
            .effects
            .apply(terra_core::EffectKind::Regeneration, 2.0, 1);
    }
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
pub(super) struct SimulationWorld<'w, 's> {
    defenses: Option<Res<'w, super::villager_defense::Defenses>>,
    mounts: Res<'w, riding::Mounts>,
    played: ResMut<'w, crate::world_time::PlayedDays>,
    inspections: ResMut<'w, InspectionTargets>,
    challengers: Option<ResMut<'w, DragonChallengers>>,
    poultry_alerts: Option<ResMut<'w, poultry::Alerts>>,
    chunks: ResMut<'w, ChunkStore>,
    terrain: Res<'w, terra_world::streaming::WorldTerrain>,
    journal: ResMut<'w, EditJournal>,
    properties: Res<'w, terra_core::block_properties::BlockPropertiesCatalog>,
    grips: ResMut<'w, stalkers::Grips>,
    body_impulses: Query<
        'w,
        's,
        (
            &'static mut avian3d::prelude::LinearVelocity,
            &'static mut CharacterGrounded,
        ),
        With<ServerCharacter>,
    >,
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
        defenses,
        mounts,
        mut played,
        mut inspections,
        mut challengers,
        mut poultry_alerts,
        mut chunks,
        terrain,
        mut journal,
        properties,
        mut grips,
        mut body_impulses,
    } = world;
    inspections.0.retain(|uuid, _| {
        peers
            .iter()
            .any(|(peer, _)| peer.uuid == *uuid && peer.synced)
    });
    schedule.elapsed += time.delta_secs().min(0.1);
    if schedule.elapsed < 0.1 && !played.slept {
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
        played.take();
        grips.retain(|_| false);
        schedule.worm_spits.clear();
        schedule.dragons.clear();
        chunks.track_dragon_flight([]);
        if let Some(challengers) = challengers.as_deref_mut() {
            challengers.0.clear();
        }
        if let Some(alerts) = poultry_alerts.as_deref_mut() {
            alerts.clear();
        }
        return;
    }
    schedule.worm_spits.advance(&mut chunks, &mut journal, dt);
    let mut vulnerable: Vec<_> = peers
        .iter()
        .filter(|(p, _)| {
            p.synced
                && !p.vitals.dead()
                && p.chat.teleport.is_none()
                && p.game_mode(&config) == terra_core::GameMode::Survival
        })
        .filter_map(|(p, _)| {
            bodies
                .get(p.body)
                .ok()
                .map(|b| (p.uuid, b.translation.to_array().map(f64::from)))
        })
        .take(128)
        .collect();
    vulnerable.sort_unstable_by_key(|(uuid, _)| *uuid);
    let targets: Vec<_> = vulnerable.iter().map(|(_, p)| *p).collect();
    let stalker_players: Vec<_> = peers
        .iter()
        .filter(|(peer, _)| peer.synced && !peer.vitals.dead() && peer.chat.teleport.is_none())
        .filter_map(|(peer, _)| {
            Some(terra_world::creatures::stalker::Player {
                id: peer.uuid,
                position: bodies
                    .get(peer.body)
                    .ok()?
                    .translation
                    .to_array()
                    .map(f64::from),
                look: peer.look,
                held_light: stalkers::held_light(peer, &journal),
                targetable: peer.game_mode(&config) == terra_core::GameMode::Survival,
            })
        })
        .take(128)
        .collect();
    let owner_positions: std::collections::BTreeMap<_, _> = stalker_players
        .iter()
        .filter(|p| {
            peers.iter().any(|(peer, _)| {
                peer.uuid == p.id && peer.game_mode(&config) != terra_core::GameMode::Spectator
            })
        })
        .map(|p| (p.id, p.position))
        .collect();
    grips.retain(|uuid| {
        stalker_players
            .iter()
            .any(|p| p.id == uuid && p.targetable && !p.held_light)
    });
    let mut population = std::mem::take(&mut journal.gameplay_mut().creatures);
    schedule.resident_elapsed += dt;
    if schedule.resident_elapsed >= 1.0 {
        schedule.resident_elapsed %= 1.0;
        schedule.residents.discover(
            &mut population,
            &Habitat {
                terrain: &terrain.0,
                chunks: &chunks,
                journal: &journal,
            },
            &players,
        );
    }
    let night = journal
        .clock
        .is_some_and(|clock| !(0.25..0.7916666667).contains(&clock.phase));
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
        let lair = !population.explored.contains(&home) && terrain.0.dragon_nest(home).is_some();
        if limit == 0
            || (!lair && natural >= limit)
            || (home[0] as f64 * 32.0 + 16.0 - player[0])
                .hypot(home[1] as f64 * 32.0 + 16.0 - player[2])
                > ACTIVE_RADIUS
        {
            continue;
        }
        let habitat = Habitat {
            terrain: &terrain.0,
            chunks: &chunks,
            journal: &journal,
        };
        if !lair && !natural_habitat_enabled(home, config.world.seed.0) {
            // A guardian district is independent of the herd/bee occupancy roll.
            terra_world::creatures::golem::populate(&mut population, &habitat, home, &players);
            continue;
        }
        terra_world::creatures::spawn_cell_away_from_players(
            &mut population,
            &habitat,
            home,
            &players,
            limit.saturating_sub(natural).max(usize::from(lair)),
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
        natural = natural_near(&population, player);
        let now = time.elapsed_secs();
        if natural < limit
            && schedule
                .spider_habitats
                .get(&home)
                .is_none_or(|last| now - *last >= 30.0)
        {
            if schedule.spider_habitats.len() >= 1024 {
                let oldest = schedule
                    .spider_habitats
                    .iter()
                    .min_by(|a, b| a.1.total_cmp(b.1))
                    .map(|(home, _)| *home);
                if let Some(oldest) = oldest {
                    schedule.spider_habitats.remove(&oldest);
                }
            }
            schedule.spider_habitats.insert(home, now);
            terra_world::creatures::spiders::spawn(
                &mut population,
                &Habitat {
                    terrain: &terrain.0,
                    chunks: &chunks,
                    journal: &journal,
                },
                home,
                night,
                player[1],
                limit.saturating_sub(natural),
            );
            natural = natural_near(&population, player);
            terra_world::creatures::stalker::spawn(
                &mut population,
                &Habitat {
                    terrain: &terrain.0,
                    chunks: &chunks,
                    journal: &journal,
                },
                home,
                player[1],
                limit.saturating_sub(natural),
            );
            natural = natural_near(&population, player);
            terra_world::creatures::forest::graves::spawn(
                &mut population,
                &Habitat {
                    terrain: &terrain.0,
                    chunks: &chunks,
                    journal: &journal,
                },
                home,
                player[1],
                night,
                limit.saturating_sub(natural),
            );
            natural = natural_near(&population, player);
            terra_world::creatures::spirit::caves::spawn(
                &mut population,
                &Habitat {
                    terrain: &terrain.0,
                    chunks: &chunks,
                    journal: &journal,
                },
                home,
                player[1],
                limit.saturating_sub(natural),
            );
        }
    }
    for a in population
        .animals
        .values_mut()
        .filter(|a| a.kind.is_dragon() && a.health <= 0.0 && a.timer > 0.0)
    {
        a.timer = (a.timer.min(terra_core::creatures::dragons::duration(
            CreatureBehavior::Dead,
        )) - dt)
            .max(0.0);
    }
    let ids: Vec<_> = population
        .animals
        .values()
        .filter(|a| {
            a.health > 0.0
                && !mounts.contains(a.id)
                && (players.iter().any(|p| active_near(a, *p)) || needs_distant_tick(a, &chunks))
        })
        .map(|a| a.id)
        .collect();
    chunks.track_dragon_flight(ids.iter().filter_map(|id| {
        let a = population.animals.get(id)?;
        (a.kind.is_dragon() && !a.juvenile() && !population.owners.contains_key(id))
            .then_some(a.position)
    }));
    let observation_players: Vec<_> = stalker_players
        .iter()
        .map(|p| (p.id, p.position, p.targetable))
        .collect();
    let worm_frame =
        schedule
            .worm_encounters
            .frame(time.elapsed_secs(), &observation_players, &population);
    if let Some(challengers) = challengers.as_deref_mut() {
        challengers
            .0
            .retain(|_, at| at.elapsed() < Duration::from_secs(12));
    }
    schedule.dragons.retain(|id, _| {
        population
            .animals
            .get(id)
            .is_some_and(|a| a.health > 0.0 && players.iter().any(|p| active_near(a, *p)))
    });
    use terra_world::creatures::dragons::Target as DragonTarget;
    let mut dragon_targets = Vec::new();
    let mut dragon_headings = std::collections::BTreeMap::new();
    if ids.iter().any(|id| {
        population
            .animals
            .get(id)
            .is_some_and(|a| a.kind.is_dragon() && a.health > 0.0)
    }) {
        dragon_headings.extend(
            stalker_players
                .iter()
                .filter(|p| p.targetable)
                .map(|p| (DragonTarget::Player(p.id), p.look[0])),
        );
        dragon_targets.extend(
            vulnerable
                .iter()
                .map(|&(id, at)| (DragonTarget::Player(id), at)),
        );
        dragon_targets.extend(ids.iter().filter_map(|id| {
            let a = population.animals.get(id)?;
            dragon_headings.insert(DragonTarget::Mob(*id), a.yaw);
            if a.health <= 0.0
                || a.kind.is_dragon()
                || population.owners.contains_key(id)
                || matches!(
                    a.behavior,
                    CreatureBehavior::InNest | CreatureBehavior::Burrowed
                )
                || a.kind == CreatureKind::SandWorm && a.behavior == CreatureBehavior::Tremor
            {
                return None;
            }
            let [x, y, z] = a.position;
            Some((DragonTarget::Mob(*id), [x, y + a.size()[1] * 0.55, z]))
        }));
    }
    let mut dragon_strikes = Vec::new();
    let mut dragon_mob_damage = Vec::new();
    let mut worm_damage = Vec::new();
    let mut worm_launches = Vec::new();
    schedule.spirit_aura_elapsed += dt;
    if schedule.spirit_aura_elapsed >= 1.0 {
        schedule.spirit_aura_elapsed %= 1.0;
        let spirits: Vec<_> = ids
            .iter()
            .filter_map(|id| population.animals.get(id))
            .filter(|a| a.kind == CreatureKind::SpiritGloom)
            .collect();
        for (mut peer, _) in peers.iter_mut().take(128) {
            if !peer.synced
                || peer.vitals.dead()
                || peer.game_mode(&config) != terra_core::GameMode::Survival
            {
                continue;
            }
            let Ok(body) = bodies.get(peer.body) else {
                continue;
            };
            if terra_world::creatures::spirit::regeneration_near(
                body.translation.to_array().map(f64::from),
                spirits.iter().copied(),
            ) {
                refresh_spirit_regeneration(&mut peer.vitals);
            }
        }
    }
    let mut deaths = Vec::new();
    let prey = terra_world::creatures::bears::PreyIndex::new(
        ids.iter().filter_map(|id| population.animals.get(id)),
    );
    let mut bear_effects = Vec::new();
    let mut spider_effects = Vec::new();
    let mut spider_meals = Vec::new();
    let mut stalker_effects = Vec::new();
    let mut resident_doors = std::collections::BTreeSet::new();
    let hunting_spiders = ids
        .iter()
        .filter_map(|id| population.animals.get(id))
        .any(|a| {
            a.kind.is_spider()
                && schedule.spider_cycles.wants_hunt(a)
                && journal.gameplay().spider_dens.contains_key(&a.id)
        });
    let food_index = hunting_spiders.then(|| {
        terra_world::creatures::spider_hunting::FoodIndex::new(
            ids.iter()
                .filter(|id| !population.owners.contains_key(*id))
                .filter_map(|id| population.animals.get(id)),
            journal
                .gameplay()
                .drops
                .iter()
                .filter(|drop| {
                    chunks
                        .block_at(drop.position.map(|v| v.floor() as i64))
                        .is_some()
                        && players
                            .iter()
                            .any(|p| distance(*p, drop.position) <= ACTIVE_RADIUS.powi(2))
                })
                .take(512),
        )
    });
    population.ecology.tick(dt);
    schedule.ecology_elapsed += dt;
    if schedule.ecology_elapsed >= 1.0 {
        schedule.ecology_elapsed %= 1.0;
        schedule.spider_cycles.retain(&population);
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
            let cadence = if distance < 64.0_f64.powi(2)
                || a.kind == CreatureKind::SandWorm
                    && terra_core::creatures::sandworm::committed(a.behavior)
            {
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
            ((search_due(
                animal,
                elapsed.min(0.25),
                schedule.navigation.needs_search(id, animal.age),
            ) || animal.kind.is_spider()
                && schedule
                    .spider_cycles
                    .needs_search(animal, elapsed.min(0.25)))
                && chunks
                    .block_at(animal.position.map(|value| value.floor() as i64))
                    .is_some())
            .then_some(id)
        })
        .collect();
    let searches = grant_searches(eligible, &mut schedule.search_cursor);
    let flocks = poultry_alerts
        .as_deref_mut()
        .map(|alerts| alerts.frame(&population, &ids, &vulnerable));
    let mut rooster_strikes = Vec::new();
    let mut predator_strikes = Vec::new();
    let prey_land: Vec<_> = if ids.iter().any(|id| {
        population
            .animals
            .get(id)
            .is_some_and(|a| a.kind.is_biome_predator())
    }) {
        ids.iter()
            .filter_map(|id| {
                let a = population.animals.get(id)?;
                (a.health > 0.0
                    && !population.owners.contains_key(id)
                    && matches!(
                        a.kind,
                        CreatureKind::Hen
                            | CreatureKind::Pig
                            | CreatureKind::Sheep
                            | CreatureKind::Zebra
                            | CreatureKind::Cow
                            | CreatureKind::Fish
                    ))
                .then_some((a.id, a.kind, a.position, a.size()[1]))
            })
            .collect()
    } else {
        Vec::new()
    };
    for (id, elapsed, _) in due.into_iter().take(512) {
        let dt = elapsed.min(0.25);
        // Keep bounded debt when a busy frame delays a creature. Discarding all
        // elapsed time after the 250 ms physics cap made distant actions stall.
        schedule
            .last_step
            .insert(id, now - (elapsed - dt).clamp(0.0, 1.0));
        if defenses
            .as_ref()
            .is_some_and(|defenses| defenses.contains(id))
        {
            continue;
        }
        if population.animals.get(&id).is_some_and(|animal| {
            animal.kind == CreatureKind::Villager
                && (terra_world::npc_work::controls_movement(journal.gameplay(), id, phase)
                    || journal.gameplay().buildings.is_busy(id))
        }) {
            continue;
        }
        let owned = population.owners.contains_key(&id);
        let mut order = terra_core::creatures::taming::order(
            journal.gameplay(),
            id,
            population.animals[&id].kind,
        );
        if population.owners.get(&id).is_some_and(|actor| {
            !terra_world::kingdom_access::can_use(
                &journal.gameplay().kingdoms,
                &chunks,
                *actor,
                terra_core::kingdom::KingdomPermission::NpcWork,
                population.animals[&id].position.map(|v| v.floor() as i64),
            )
        }) {
            order = terra_core::creatures::taming::CompanionMode::Wait;
        }
        let owner = population
            .owners
            .get(&id)
            .and_then(|id| owner_positions.get(id))
            .copied();
        let companion = owned && order != terra_core::creatures::taming::CompanionMode::None
            || population.animals.get(&id).is_some_and(|animal| {
                journal
                    .gameplay()
                    .animal_bonds
                    .get(&id)
                    .is_some_and(|bond| bond.calmed(animal.age))
            });
        let prior_den = journal.gameplay().spider_dens.get(&id).copied();
        let den = prior_den.filter(|den| {
            !journal.prop_removed(
                terra_voxel::global_to_section(den.anchor).0,
                terra_world::props::PropId(den.web_id),
            )
        });
        let owner_uuid = population.owners.get(&id).copied();
        if let Some(a) = population.animals.get_mut(&id) {
            let previous_age = a.age;
            let previous_position = a.position;
            schedule.navigation.begin_tick(id);
            let search = searches.contains(&id);
            let climatic_rest = terra_world::creatures::ecology::climate_step(
                a,
                &Habitat {
                    terrain: &terrain.0,
                    chunks: &chunks,
                    journal: &journal,
                },
                dt,
            );
            let sleeping = climatic_rest
                || night
                    && (a.kind == CreatureKind::Bee
                        || terra_world::creatures::ecology::sleep_at(a.id, phase));
            let effect = if a.kind.is_dragon() {
                let encounter = schedule.dragons.entry(id).or_insert_with(|| {
                    let nest = if terra_core::creatures::spawning::is_command_id(id) || owned {
                        None
                    } else {
                        terrain
                            .0
                            .dragon_roost(a.home)
                            .filter(|(kind, _, _)| *kind == a.kind)
                            .map(|(_, center, direction)| (center, direction))
                    };
                    terra_world::creatures::dragons::Encounter::new(a, nest)
                });
                encounter.set_prey_heading(
                    encounter
                        .target
                        .and_then(|id| dragon_headings.get(&id).copied()),
                );
                encounter.set_keeper(
                    owner,
                    order == terra_core::creatures::taming::CompanionMode::Follow,
                );
                dragon_strikes.extend(encounter.step(
                    a,
                    &Habitat {
                        terrain: &terrain.0,
                        chunks: &chunks,
                        journal: &journal,
                    },
                    dt,
                    &dragon_targets,
                    night,
                    owned,
                ));
                if let Some(DragonTarget::Mob(target)) = encounter.target
                    && matches!(
                        a.behavior,
                        CreatureBehavior::Hunt
                            | CreatureBehavior::Attack
                            | CreatureBehavior::DragonVolley
                            | CreatureBehavior::DragonSurge
                            | CreatureBehavior::DragonBurrow
                            | CreatureBehavior::Leap
                    )
                    && flocks
                        .as_ref()
                        .is_some_and(|frame| frame.tracks_hen(target))
                    && let Some(alerts) = poultry_alerts.as_deref_mut()
                {
                    alerts.report(target, poultry::Actor::Mob(id));
                }
                terra_world::creatures::Forage::None
            } else if a.kind == CreatureKind::Villager {
                let Schedule {
                    residents,
                    navigation,
                    worm_encounters,
                    ..
                } = &mut *schedule;
                let guard = worm_encounters.guard_step(
                    a,
                    &worm_frame,
                    &Habitat {
                        terrain: &terrain.0,
                        chunks: &chunks,
                        journal: &journal,
                    },
                    owner_uuid,
                    dt,
                    search,
                    navigation,
                );
                if let Some(strike) = guard {
                    if let Some((target, damage)) = strike {
                        worm_damage.push((target, damage, a.position));
                    }
                } else if let Some(door) = residents.step_as(
                    a,
                    &Habitat {
                        terrain: &terrain.0,
                        chunks: &chunks,
                        journal: &journal,
                    },
                    phase,
                    dt,
                    search,
                    navigation,
                    owner_uuid,
                ) && config.world.game_rules.mob_griefing
                {
                    resident_doors.insert(door);
                }
                terra_world::creatures::Forage::None
            } else if companion {
                schedule.spider_cycles.forget(id);
                // Ground navigation stores waypoints at standing height. A
                // spider's surface steering instead aims at the owner's body.
                let destination = owner.map(|mut position| {
                    if !a.kind.is_spider() {
                        position[1] -= f64::from(terra_physics::character::PLAYER_HALF_HEIGHT);
                    }
                    position
                });
                terra_world::creatures::companions::step(
                    a,
                    &Habitat {
                        terrain: &terrain.0,
                        chunks: &chunks,
                        journal: &journal,
                    },
                    destination,
                    order,
                    dt,
                    search,
                    Some(&mut schedule.navigation),
                );
                if owner_uuid.is_some_and(|actor| {
                    !terra_world::kingdom_access::can_use(
                        &journal.gameplay().kingdoms,
                        &chunks,
                        actor,
                        terra_core::kingdom::KingdomPermission::NpcWork,
                        a.position.map(|v| v.floor() as i64),
                    )
                }) {
                    // Keep the authorized side of a border; the persisted order is
                    // rechecked each tick and can resume after an authorized change.
                    a.position = previous_position;
                    a.velocity = [0.0; 3];
                    a.behavior = CreatureBehavior::Rest;
                }
                terra_world::creatures::Forage::None
            } else if a.kind == CreatureKind::Stalker {
                let Schedule {
                    stalkers,
                    navigation,
                    ..
                } = &mut *schedule;
                stalker_effects.push(stalkers.step(
                    a,
                    &Habitat {
                        terrain: &terrain.0,
                        chunks: &chunks,
                        journal: &journal,
                    },
                    &stalker_players,
                    dt,
                    search,
                    navigation,
                ));
                terra_world::creatures::Forage::None
            } else if a.kind.is_spider() {
                let habitat = Habitat {
                    terrain: &terrain.0,
                    chunks: &chunks,
                    journal: &journal,
                };
                let day_refuge = !owned
                    && a.kind == CreatureKind::MediumSpider
                    && !night
                    && !terra_world::creatures::spiders::in_cave(a, &habitat);
                let den = den.filter(|den| !day_refuge && constructed_den(&chunks, &journal, *den));
                let missing_den = den.is_none()
                    && !day_refuge
                    && !habitat.wet(terra_core::creatures::spiders::visual_center(a));
                // A territorial spider can defend itself while waiting at its
                // den or seeking an anchor. Committed shots run to impact even
                // after their original target has moved out of sight.
                let defending = !owned
                    && !day_refuge
                    && (a.needs.goal.is_some_and(|goal| {
                        goal.kind == terra_core::creatures::NeedKind::AttackAim
                    }) || (a.kind != CreatureKind::SmallSpider || a.health < a.kind.health())
                        && terra_world::creatures::spiders::sees_threat(a, &habitat, &targets));
                let effect = if defending {
                    terra_world::creatures::spiders::step_with_navigation(
                        a,
                        &habitat,
                        &targets,
                        dt,
                        search,
                        night,
                        Some(&mut schedule.navigation),
                    )
                } else if missing_den {
                    schedule.spider_cycles.forget(id);
                    let prepare = if owned {
                        terra_world::creatures::spiders::prepare_owned_den
                    } else {
                        terra_world::creatures::spiders::prepare_den
                    };
                    let request = prepare(a, &habitat, dt, search, &mut schedule.navigation)
                        .filter(|request| request.separation >= 8)
                        .map(|request| den_rebuild_request(request, prior_den, &journal));
                    terra_world::creatures::spiders::Outcome {
                        web: request,
                        ..Default::default()
                    }
                } else if let Some(den) = den {
                    use terra_world::creatures::spiders::cycle::Phase;
                    let phase = schedule.spider_cycles.phase(a, den.web_id, den.anchor, dt);
                    let food = (phase == Phase::Hunt)
                        .then(|| {
                            food_index.as_ref().and_then(|index| {
                                terra_world::creatures::spider_hunting::cycle_step(
                                    a,
                                    &habitat,
                                    (index, owned),
                                    den.anchor,
                                    dt,
                                    search,
                                    &mut schedule.navigation,
                                    |target| {
                                        if let terra_world::creatures::spider_hunting::Meal::Creature(target) = target
                                            && flocks.as_ref().is_some_and(|frame| frame.tracks_hen(target))
                                            && let Some(alerts) = poultry_alerts.as_deref_mut()
                                        {
                                            alerts.report(target, poultry::Actor::Mob(id));
                                        }
                                    }
                                )
                            })
                        })
                        .flatten();
                    if let Some(meal) = food {
                        if let Some(meal) = meal {
                            spider_meals.push((id, meal));
                        }
                        terra_world::creatures::spiders::Outcome::default()
                    } else {
                        terra_world::creatures::spiders::cycle::step_den(
                            a,
                            &habitat,
                            den.anchor,
                            phase,
                            dt,
                            search,
                            (&mut schedule.navigation, owned),
                        );
                        terra_world::creatures::spiders::Outcome::default()
                    }
                } else if owned {
                    terra_world::creatures::spiders::step_owned(
                        a,
                        &Habitat {
                            terrain: &terrain.0,
                            chunks: &chunks,
                            journal: &journal,
                        },
                        dt,
                        search,
                        night,
                        Some(&mut schedule.navigation),
                    )
                } else {
                    terra_world::creatures::spiders::step_with_navigation(
                        a,
                        &Habitat {
                            terrain: &terrain.0,
                            chunks: &chunks,
                            journal: &journal,
                        },
                        &targets,
                        dt,
                        search,
                        night,
                        Some(&mut schedule.navigation),
                    )
                };
                spider_effects.push((id, effect));
                terra_world::creatures::Forage::None
            } else if a.kind.is_aquatic() && a.kind != CreatureKind::Fish {
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
            } else if matches!(a.kind, CreatureKind::SandWorm | CreatureKind::LavaWorm) {
                let (worm_targets, target_ids) = worm_frame.targets(a, &targets);
                let attack_phase = a.behavior;
                let habitat = Habitat {
                    terrain: &terrain.0,
                    chunks: &chunks,
                    journal: &journal,
                };
                let effect = if a.kind == CreatureKind::LavaWorm {
                    terra_world::creatures::lava_worm::step(a, &habitat, &worm_targets, dt)
                } else {
                    terra_world::creatures::sandworm::step_sensed(
                        a,
                        &habitat,
                        &worm_targets,
                        worm_frame.disturbances.iter().copied(),
                        dt,
                    )
                };
                match effect {
                    terra_world::creatures::bears::Effect::StrikePlayer { index, damage } => {
                        match target_ids.get(index) {
                            Some(worm_encounters::Target::Creature(target)) => {
                                worm_damage.push((*target, damage, a.position))
                            }
                            Some(worm_encounters::Target::Player(index)) => {
                                let impulse = worm_encounters::strike_impulse(attack_phase, a.yaw);
                                if impulse != Vec3::ZERO {
                                    worm_launches.push((*index, damage, impulse));
                                } else {
                                    bear_effects.push((
                                        id,
                                        terra_world::creatures::bears::Effect::StrikePlayer {
                                            index: *index,
                                            damage,
                                        },
                                    ));
                                }
                            }
                            None => {}
                        }
                    }
                    terra_world::creatures::bears::Effect::None => {}
                    effect => bear_effects.push((id, effect)),
                }
                terra_world::creatures::Forage::None
            } else if a.kind == CreatureKind::StoneGolem {
                terra_world::creatures::golem::step(
                    a,
                    &Habitat {
                        terrain: &terrain.0,
                        chunks: &chunks,
                        journal: &journal,
                    },
                    &targets,
                    dt,
                    search,
                    Some(&mut schedule.navigation),
                    |index, damage| {
                        bear_effects.push((
                            id,
                            terra_world::creatures::bears::Effect::StrikePlayer { index, damage },
                        ))
                    },
                );
                terra_world::creatures::Forage::None
            } else if a.kind.is_biome_predator() {
                let habitat = Habitat {
                    terrain: &terrain.0,
                    chunks: &chunks,
                    journal: &journal,
                };
                let origin = a.position;
                let range = terra_core::creatures::definitions::get(a.kind)
                    .combat
                    .map_or(0.0, |combat| combat.aggro_radius * 1.25);
                let mut visible_prey: Vec<_> = prey_land
                    .iter()
                    .filter(|&&(_, kind, point, _)| {
                        distance(origin, point) < range.powi(2)
                            && (a.kind != CreatureKind::Snake || kind == CreatureKind::Hen)
                            && (kind != CreatureKind::Fish
                                || a.kind == CreatureKind::Crocodile
                                    && (origin[1] - point[1]).abs() < 1.4)
                    })
                    .collect();
                visible_prey.sort_unstable_by(|a, b| {
                    distance(origin, a.2)
                        .total_cmp(&distance(origin, b.2))
                        .then(a.0.cmp(&b.0))
                });
                let candidates = &visible_prey[..visible_prey.len().min(12)];
                use terra_world::creatures::forest::Target;
                let animal_targets = candidates
                    .iter()
                    .map(|&&(_, _, point, height)| Target::creature(point, height));
                let all_targets = targets
                    .iter()
                    .copied()
                    .map(Target::player)
                    .chain(animal_targets);
                let effect = terra_world::creatures::forest::step_observed(
                    a,
                    &habitat,
                    all_targets,
                    dt,
                    search,
                    Some(&mut schedule.navigation),
                    |index| {
                        if let Some(heading) = index.checked_sub(targets.len())
                            && let Some(&&(target, CreatureKind::Hen, ..)) = candidates.get(heading)
                            && let Some(alerts) = poultry_alerts.as_deref_mut()
                        {
                            alerts.report(target, poultry::Actor::Mob(id));
                        }
                    },
                );
                if let terra_world::creatures::bears::Effect::StrikePlayer { index, damage } =
                    effect
                {
                    if let Some(heading) = index.checked_sub(targets.len()) {
                        if let Some(animal) = candidates.get(heading) {
                            predator_strikes.push((id, animal.0, damage));
                        }
                    } else {
                        bear_effects.push((id, effect));
                    }
                }
                terra_world::creatures::Forage::None
            } else if terra_world::creatures::forest::hostile(a.kind) {
                let effect = terra_world::creatures::forest::step(
                    a,
                    &Habitat {
                        terrain: &terrain.0,
                        chunks: &chunks,
                        journal: &journal,
                    },
                    &targets,
                    dt,
                    search,
                    Some(&mut schedule.navigation),
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
            } else if matches!(a.kind, CreatureKind::Hen | CreatureKind::Rooster) {
                let habitat = Habitat {
                    terrain: &terrain.0,
                    chunks: &chunks,
                    journal: &journal,
                };
                let herd = flocks.as_ref();
                let nav = &mut schedule.navigation;
                let scare = a.kind == CreatureKind::Hen
                    && (herd
                        .and_then(|frame| frame.threat(a))
                        .is_some_and(|(_, point)| {
                            terra_world::creatures::poultry::alert_hen(a, &habitat, point)
                        })
                        || sleeping
                            && players.iter().any(|&point| {
                                distance(a.position, point) <= 3.6_f64.powi(2)
                                    && terra_world::creatures::poultry::alert_hen(
                                        a, &habitat, point,
                                    )
                            }));
                let enemy = (a.kind == CreatureKind::Rooster)
                    .then(|| herd.and_then(|frame| frame.enemy(a, &habitat)))
                    .flatten();
                if let Some((enemy, target)) = enemy
                    && let Some(alerts) = poultry_alerts.as_deref_mut()
                {
                    let same = alerts.lock(a.id, enemy);
                    if let Some(damage) = terra_world::creatures::poultry::defend(
                        a, &habitat, target, same, dt, search, nav,
                    ) {
                        rooster_strikes.push((id, enemy, damage));
                    }
                    terra_world::creatures::Forage::None
                } else if !sleeping
                    && !scare
                    && (a.kind == CreatureKind::Rooster
                        || !players
                            .iter()
                            .any(|&p| distance(a.position, p) < 4.0_f64.powi(2)))
                    && herd
                        .and_then(|frame| frame.flockmate(a))
                        .is_some_and(|target| {
                            terra_world::creatures::poultry::follow(
                                a, target, &habitat, dt, search, nav,
                            )
                        })
                {
                    terra_world::creatures::Forage::None
                } else {
                    if a.kind == CreatureKind::Rooster {
                        if let Some(alerts) = poultry_alerts.as_deref_mut() {
                            alerts.forget(a.id);
                        }
                        if matches!(
                            a.behavior,
                            CreatureBehavior::Attack | CreatureBehavior::Hunt
                        ) {
                            a.behavior = CreatureBehavior::Rest;
                            a.timer = 0.0;
                        }
                    }
                    terra_world::creatures::ecology::step_with_navigation(
                        a,
                        &mut population.ecology,
                        &habitat,
                        &players,
                        dt,
                        search,
                        sleeping && !scare,
                        Some(nav),
                    )
                }
            } else {
                terra_world::creatures::ecology::step_with_navigation(
                    a,
                    &mut population.ecology,
                    &Habitat {
                        terrain: &terrain.0,
                        chunks: &chunks,
                        journal: &journal,
                    },
                    if owned { &[] } else { &players },
                    dt,
                    search,
                    sleeping,
                    Some(&mut schedule.navigation),
                )
            };
            // Contact drag slows physical travel, never attack/burrow clocks or
            // the hidden end-of-leap re-anchor between its two apertures.
            if !(a.kind == CreatureKind::SandWorm
                && previous_position != a.position
                && a.behavior == CreatureBehavior::Emerge)
            {
                let drag = terra_world::viscous::body_contact(
                    &chunks,
                    &journal,
                    previous_position,
                    a.size(),
                )
                .movement_multiplier;
                if drag < 1.0 {
                    for axis in [0, 2] {
                        a.position[axis] = previous_position[axis]
                            + (a.position[axis] - previous_position[axis]) * f64::from(drag);
                        a.velocity[axis] *= drag;
                    }
                }
            }
            terra_world::conveyors::carry_creature(
                a,
                &Habitat {
                    terrain: &terrain.0,
                    chunks: &chunks,
                    journal: &journal,
                },
                dt,
            );
            let simulated = terra_world::creatures::breeding::constrain_growth(
                a,
                previous_age,
                &Habitat {
                    terrain: &terrain.0,
                    chunks: &chunks,
                    journal: &journal,
                },
            );
            if terra_core::creatures::breeding::is_birth_id(a.id)
                && let Some(family) = journal.gameplay_mut().animal_families.get_mut(&a.id)
            {
                family.advance_active_age(previous_age, simulated);
            }
            if effect != terra_world::creatures::Forage::None
                && let Err(error) = terra_world::creatures::apply_forage(
                    effect,
                    a,
                    &mut chunks,
                    &mut journal,
                    config.world.game_rules,
                )
            {
                tracing::warn!(%error, "Animal forage could not be committed");
            }
            if a.health <= 0.0 {
                if let Some(bee) = population.ecology.bees.get_mut(&a.id) {
                    bee.inside = false;
                    bee.carried = 0;
                }
                deaths.push((
                    a.position,
                    a.product(),
                    terra_core::creatures::drops::forest_loot(a.kind, a.variant, a.id),
                ));
            }
        }
    }
    for effect in stalker_effects {
        if let Some((uuid, damage)) = effect.strike
            && let Some((mut peer, _)) = peers.iter_mut().find(|(peer, _)| peer.uuid == uuid)
        {
            let mode = peer.game_mode(&config);
            survival::animal_attack(&mut peer, damage, mode);
        }
        if let Some(pull) = effect.pull {
            grips.refresh(pull);
        }
    }
    schedule.stalkers.retain(&population);
    for (id, effect) in spider_effects {
        if let Some(bite) = effect.bite
            && let Some((uuid, _)) = vulnerable.get(bite.player)
            && let Some((mut peer, _)) = peers.iter_mut().find(|(p, _)| p.uuid == *uuid)
        {
            let mode = peer.game_mode(&config);
            spider_bite(&mut peer, bite, mode);
        }
        if let Some((uuid, _)) = effect.web_hit.and_then(|index| vulnerable.get(index))
            && let Some((mut peer, _)) = peers.iter_mut().find(|(peer, _)| peer.uuid == *uuid)
            && peer.survival.protection <= 0.
            && !peer.vitals.dead()
        {
            peer.vitals
                .effects
                .apply(terra_core::EffectKind::Slow, 3.5, 2);
        }
        if config.world.game_rules.mob_griefing
            && let Some(web) = effect.web
            && let Some(den) = spin_web(&mut chunks, &mut journal, &terrain.0, web)
        {
            journal.gameplay_mut().spider_dens.insert(id, den);
        }
    }
    if !spider_meals.is_empty() {
        let mut gameplay = std::mem::take(journal.gameplay_mut());
        for (id, meal) in spider_meals {
            terra_world::creatures::spider_hunting::eat(
                &mut population,
                &mut gameplay,
                &Habitat {
                    terrain: &terrain.0,
                    chunks: &chunks,
                    journal: &journal,
                },
                id,
                meal,
            );
        }
        *journal.gameplay_mut() = gameplay;
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
                    if population
                        .animals
                        .get(&id)
                        .is_some_and(|a| a.kind == CreatureKind::LavaWorm)
                    {
                        molten_worm_hit(&mut peer, damage, mode);
                    } else {
                        let before = peer.vitals.health;
                        survival::animal_attack(&mut peer, damage, mode);
                        if peer.vitals.health < before
                            && !peer.vitals.dead()
                            && let Some(snake) = population.animals.get(&id)
                            && snake.kind.venomous(snake.variant)
                            && let Some(attack) =
                                terra_core::creatures::definitions::get(snake.kind).combat
                        {
                            peer.vitals.effects.apply(
                                terra_core::EffectKind::Poison,
                                attack.poison_seconds,
                                attack.poison_level,
                            );
                        }
                    }
                }
            }
            terra_world::creatures::bears::Effect::Ignite { position } => {
                if config.world.game_rules.fire_spread {
                    terra_world::lava::ignite(&chunks, &mut journal, position);
                }
            }
            terra_world::creatures::bears::Effect::SpitBile { origin, velocity } => {
                schedule.worm_spits.launch(origin, velocity);
            }
            terra_world::creatures::bears::Effect::None => {}
        }
    }
    for strike in dragon_strikes {
        let target = match strike.target {
            DragonTarget::Player(uuid) => uuid,
            DragonTarget::Mob(id) => {
                dragon_mob_damage.push((id, strike.damage));
                continue;
            }
        };
        let Some((mut peer, _)) = peers.iter_mut().find(|(peer, _)| peer.uuid == target) else {
            continue;
        };
        let mode = peer.game_mode(&config);
        let before = peer.vitals.health;
        survival::animal_attack(&mut peer, strike.damage, mode);
        if peer.vitals.health < before && !peer.vitals.dead() {
            if let Some(challengers) = challengers.as_deref_mut() {
                challengers
                    .0
                    .insert((strike.dragon, peer.uuid), Instant::now());
            }
            if let Some(effect) = strike.effect {
                peer.vitals.effects.apply(effect, 4.0, 1);
            }
            if let Ok((mut velocity, mut grounded)) = body_impulses.get_mut(peer.body) {
                velocity.0 += Vec3::from_array(strike.impulse);
                grounded.0 = false;
            }
        }
    }
    for (index, damage, impulse) in worm_launches {
        if let Some((uuid, _)) = vulnerable.get(index)
            && let Some((mut peer, _)) = peers.iter_mut().find(|(p, _)| p.uuid == *uuid)
        {
            let mode = peer.game_mode(&config);
            let before = peer.vitals.health;
            survival::animal_attack(&mut peer, damage, mode);
            // Damage protection and creative immunity also protect the motor.
            // Snapshot reconciliation already carries this real velocity; no
            // camera displacement or teleport is used to fake a launch.
            if peer.vitals.health < before
                && !peer.vitals.dead()
                && let Ok((mut velocity, mut grounded)) = body_impulses.get_mut(peer.body)
            {
                velocity.0.x += impulse.x;
                velocity.0.z += impulse.z;
                velocity.0.y = velocity.0.y.max(impulse.y);
                grounded.0 = false;
            }
        }
    }
    let separation_ids: Vec<_> = ids
        .iter()
        .copied()
        .filter(|id| {
            !defenses
                .as_ref()
                .is_some_and(|defenses| defenses.contains(*id))
        })
        .collect();
    terra_world::creatures::navigation::separate(
        &mut population,
        &Habitat {
            terrain: &terrain.0,
            chunks: &chunks,
            journal: &journal,
        },
        &separation_ids,
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
    schedule.breeding_elapsed += dt;
    let breeding_due = schedule.breeding_elapsed >= 1.;
    if breeding_due || played.slept {
        let days = played.take();
        terra_world::creatures::dragon_nursery::step(
            &mut population,
            &terrain.0,
            &mut chunks,
            &mut journal,
            &properties,
            &ids,
            days,
        );
    }
    if breeding_due {
        schedule.breeding_elapsed %= 1.0;
        terra_world::creatures::dragon_nursery::lay_eggs(
            &population,
            &chunks,
            journal.gameplay_mut(),
            &ids,
        );
        let mut families = std::mem::take(&mut journal.gameplay_mut().animal_families);
        let mut bonds = std::mem::take(&mut journal.gameplay_mut().animal_bonds);
        let mut next_birth = journal.gameplay().next_animal_birth;
        let mut births = terra_world::creatures::breeding::step(
            &mut population,
            &mut families,
            &mut bonds,
            &mut next_birth,
            &Habitat {
                terrain: &terrain.0,
                chunks: &chunks,
                journal: &journal,
            },
            &ids,
            &mut schedule.breeding_cursor,
        );
        let bee_capacity = terra_world::creatures::breeding::BIRTH_BUDGET
            .saturating_sub(births)
            .min(
                limit.saturating_sub(
                    ids.iter()
                        .filter(|id| !population.owners.contains_key(id))
                        .count(),
                ),
            );
        births += terra_world::creatures::bee_lifecycle::step(
            &mut population,
            &mut families,
            &mut next_birth,
            &Habitat {
                terrain: &terrain.0,
                chunks: &chunks,
                journal: &journal,
            },
            &ids,
            &mut schedule.bee_breeding_cursor,
            bee_capacity,
        );
        let state = journal.gameplay_mut();
        state.animal_families = families;
        state.animal_bonds = bonds;
        state.next_animal_birth = next_birth;
        if births > 0 {
            state.revision = state.revision.wrapping_add(1);
        }
    }
    let occupants: Vec<_> =
        if resident_doors.is_empty() && schedule.resident_doors.pending.is_empty() {
            Vec::new()
        } else {
            bodies
                .iter()
                .map(|body| body.translation.to_array().map(f64::from))
                .collect()
        };
    schedule.resident_doors.update(
        now,
        &resident_doors,
        ResidentDoorWorld {
            chunks: &mut chunks,
            journal: &mut journal,
            properties: &properties,
            population: &population,
            players: &occupants,
        },
    );
    journal.gameplay_mut().creatures = population;
    for (id, target, damage) in predator_strikes {
        let animals = &journal.gameplay().creatures.animals;
        let habitat = Habitat {
            terrain: &terrain.0,
            chunks: &chunks,
            journal: &journal,
        };
        if let (Some(predator), Some(prey)) = (animals.get(&id), animals.get(&target))
            && prey.health > 0.0
            && terra_world::creatures::forest::ready_to_strike(
                predator,
                &habitat,
                terra_world::creatures::forest::Target::creature(prey.position, prey.size()[1]),
            )
        {
            train_damage::hurt_creature(journal.gameplay_mut(), target, damage);
        }
    }
    for (id, enemy, damage) in rooster_strikes {
        let population = &journal.gameplay().creatures;
        let habitat = Habitat {
            terrain: &terrain.0,
            chunks: &chunks,
            journal: &journal,
        };
        if !population.animals.get(&id).is_some_and(|bird| {
            enemy.point(population, &vulnerable).is_some_and(|point| {
                terra_world::creatures::poultry::ready_to_strike(bird, point, &habitat)
            })
        }) {
            continue;
        }
        match enemy {
            poultry::Actor::Player(uuid) => {
                if let Some((mut peer, _)) = peers.iter_mut().find(|(peer, _)| peer.uuid == uuid) {
                    let mode = peer.game_mode(&config);
                    survival::animal_attack(&mut peer, damage, mode);
                }
            }
            poultry::Actor::Mob(target) => {
                poultry::hurt_opponent(journal.gameplay_mut(), target, damage);
            }
        }
    }
    for (id, damage) in dragon_mob_damage {
        train_damage::hurt_creature(journal.gameplay_mut(), id, damage);
    }
    for (target, damage, attacker) in worm_damage {
        let original = journal
            .gameplay()
            .creatures
            .animals
            .get(&target)
            .filter(|a| a.kind == CreatureKind::SandWorm)
            .cloned();
        if train_damage::hurt_creature(journal.gameplay_mut(), target, damage)
            && let Some(original) = original
            && let Some(worm) = journal
                .gameplay_mut()
                .creatures
                .animals
                .get_mut(&target)
                .filter(|a| a.health > 0.0)
        {
            worm.behavior = original.behavior;
            worm.timer = original.timer;
            worm.needs.goal = original.needs.goal;
            terra_core::creatures::sandworm::hit_reaction(worm, attacker);
        }
    }
    {
        let state = journal.gameplay_mut();
        let population = &state.creatures;
        state.pet_orders.retain(|id, _| {
            population.owners.contains_key(id)
                && population.animals.get(id).is_some_and(|a| a.health > 0.0)
        });
    }
    for (position, product, loot) in deaths {
        for stack in loot.into_iter().flatten() {
            journal
                .gameplay_mut()
                .drop_stack(position, stack, [0.0, 1.8, 0.0], 0.35);
        }
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
            .filter(|a| {
                (a.health > 0.0 || a.kind.is_dragon() && a.timer > 0.0) && in_interest(a, position)
            })
            .collect();
        let fighting = |a: &terra_core::creatures::Creature| {
            a.kind.is_dragon()
                && a.health > 0.0
                && !peer.vitals.dead()
                && peer.game_mode(&config) == terra_core::GameMode::Survival
                && schedule.dragons.get(&a.id).is_some_and(|enc| {
                    enc.target.is_some()
                        && (enc.target
                            == Some(terra_world::creatures::dragons::Target::Player(peer.uuid))
                            || challengers
                                .as_deref()
                                .is_some_and(|c| c.0.contains_key(&(a.id, peer.uuid))))
                })
        };
        creatures.sort_unstable_by(|a, b| {
            fighting(b)
                .cmp(&fighting(a))
                .then_with(|| {
                    distance(position, a.position).total_cmp(&distance(position, b.position))
                })
                .then(a.id.cmp(&b.id))
        });
        let visible: Vec<_> = creatures
            .into_iter()
            .take(MAX_VISIBLE_CREATURES)
            .cloned()
            .collect();
        let intentions = inspections
            .0
            .get(&peer.uuid)
            .and_then(|id| visible.iter().find(|a| a.id == *id))
            .map(|a| schedule.navigation.inspection(a))
            .filter(|intent| intent.valid())
            .into_iter()
            .collect();
        let bonds = visible
            .iter()
            .filter_map(|a| {
                let state = journal.gameplay();
                let owner = state.creatures.owners.get(&a.id).copied();
                let bond = state
                    .animal_bonds
                    .get(&a.id)
                    .copied()
                    .or_else(|| owner.map(terra_core::creatures::taming::AnimalBond::owned))?;
                Some(terra_core::creatures::taming::CompanionView {
                    id: a.id,
                    owner,
                    player: bond.player,
                    trust: bond.trust,
                    mode: if owner.is_some() {
                        terra_core::creatures::taming::order(journal.gameplay(), a.id, a.kind)
                    } else {
                        terra_core::creatures::taming::CompanionMode::Wait
                    },
                })
            })
            .collect();
        let families = visible
            .iter()
            .filter_map(|animal| {
                journal
                    .gameplay()
                    .animal_families
                    .get(&animal.id)
                    .copied()
                    .map(|family| terra_core::creatures::breeding::FamilyView::new(animal, family))
            })
            .collect();
        sender.send::<CreatureSnapshotChannel>(CreatureStates {
            boss: visible.first().filter(|a| fighting(a)).map(|a| a.id),
            tick: runtime.tick,
            ecology: snapshot_ecology(&journal.gameplay().creatures, &visible, position),
            creatures: visible,
            intentions,
            bonds,
            families,
            stalker_grip: grips.get(peer.uuid),
        });
    }
}

fn companion_feeding_pose(animal: &mut terra_core::creatures::Creature) {
    let suspended = animal.kind.is_spider()
        && matches!(
            animal.behavior,
            CreatureBehavior::Climb | CreatureBehavior::Ceiling | CreatureBehavior::Descend
        );
    if !suspended {
        animal.behavior = CreatureBehavior::Graze;
    }
    animal.timer = 4.0;
}

pub(super) fn pet_command(
    id: u64,
    mode: terra_core::creatures::taming::CompanionMode,
    peer: &mut Peer,
    world: &mut ControlWorld,
) {
    if !peer.synced
        || !peer.permissions.build
        || peer.vitals.dead()
        || peer.game_mode(&world.config) == terra_core::GameMode::Spectator
        || peer.chat.teleport.is_some()
        || peer.creature_action_at.elapsed() < Duration::from_millis(450)
        || world.mounts.contains(id)
        || world.mounts.active.contains_key(&peer.uuid)
        || world.journal.gameplay().creatures.owners.get(&id) != Some(&peer.uuid)
    {
        return;
    }
    let Ok(body) = world.bodies.get(peer.body) else {
        return;
    };
    let Some(animal) = world
        .journal
        .gameplay()
        .creatures
        .animals
        .get(&id)
        .filter(|a| a.health > 0.0 && terra_core::creatures::taming::tameable(a.kind))
    else {
        return;
    };
    if !super::kingdoms::permission_allowed(
        peer.uuid,
        terra_core::kingdom::KingdomPermission::NpcWork,
        animal.position.map(|v| v.floor() as i64),
        world,
    ) {
        return;
    }
    let eye = (body.translation + Vec3::Y * terra_physics::character::PLAYER_EYE_OFFSET)
        .to_array()
        .map(f64::from);
    let center = [
        animal.position[0],
        animal.position[1] + animal.size()[1] * 0.5,
        animal.position[2],
    ];
    let offset: [f64; 3] = std::array::from_fn(|i| center[i] - eye[i]);
    let length = offset.iter().map(|v| v * v).sum::<f64>().sqrt();
    if !length.is_finite() || !(0.001..=6.0).contains(&length) {
        return;
    }
    let direction = offset.map(|v| v / length);
    let Some(hit) = terra_world::creatures::ray_distance(animal, eye, direction, 4.5) else {
        return;
    };
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
        terra_world::props::PropPose::Rest,
    );
    if block.is_some_and(|b| b.distance + 0.01 < hit) {
        return;
    }
    if !terra_core::creatures::taming::command(world.journal.gameplay_mut(), id, peer.uuid, mode) {
        return;
    }
    if let Some(a) = world.journal.gameplay_mut().creatures.animals.get_mut(&id) {
        pet_command_pose(a);
    }
    peer.creature_action_at = Instant::now();
}

fn pet_command_pose(animal: &mut terra_core::creatures::Creature) {
    let airborne_horse =
        animal.kind == CreatureKind::Horse && animal.behavior == CreatureBehavior::Leap;
    if !animal.kind.is_spider() && !airborne_horse {
        animal.behavior = CreatureBehavior::Rest;
        animal.needs.goal = None;
    }
    if !airborne_horse {
        animal.timer = 0.0;
    }
    animal.velocity[0] = 0.0;
    animal.velocity[2] = 0.0;
}

pub(super) fn interact(
    id: u64,
    feed: bool,
    look: [f32; 2],
    peer: &mut Peer,
    sender: &mut MessageSender<ServerControl>,
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
    let permission = if feed && animal.kind == CreatureKind::Villager {
        terra_core::kingdom::KingdomPermission::Inventories
    } else if feed {
        terra_core::kingdom::KingdomPermission::NpcWork
    } else {
        terra_core::kingdom::KingdomPermission::Break
    };
    if !super::kingdoms::permission_allowed(
        peer.uuid,
        permission,
        animal.position.map(|v| v.floor() as i64),
        world,
    ) {
        return;
    }
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
    let forest_loot =
        terra_core::creatures::drops::forest_loot(animal.kind, animal.variant, animal.id);
    let position = animal.position;
    if feed && kind == CreatureKind::Villager {
        let message = terra_core::creatures::villagers::greeting(
            id,
            animal.variant,
            animal.behavior == CreatureBehavior::Sleep,
        );
        if let Some(chat) = world.chat.as_mut() {
            sender.send::<ControlChannel>(ServerControl::Chat(chat.entry(message)));
        }
        if let Some(a) = world.journal.gameplay_mut().creatures.animals.get_mut(&id)
            && a.behavior != CreatureBehavior::Sleep
        {
            a.yaw = ((position[0] - eye[0]).atan2(position[2] - eye[2])) as f32;
            a.behavior = CreatureBehavior::Rest;
            a.velocity[0] = 0.0;
            a.velocity[2] = 0.0;
            a.timer = 4.0;
            a.needs.goal = None;
        }
        peer.creature_action_at = Instant::now();
        return;
    }
    if feed {
        let held = gameplay::held_tool(peer, world);
        if held.is_none_or(|food| terra_core::creatures::taming::food_gain(kind, food).is_none())
            && kind == CreatureKind::Horse
        {
            let horse = animal.clone();
            let habitat = Habitat {
                terrain: &world.terrain.0,
                chunks: &world.chunks,
                journal: &world.journal,
            };
            if !habitat.clear_swimmer(
                terra_world::creatures::riding::seat_feet(&horse),
                terra_world::creatures::riding::RIDER_SIZE,
            ) {
                return;
            }
            if world
                .mounts
                .mount(&horse, peer.uuid, world.journal.gameplay())
            {
                peer.creature_action_at = Instant::now();
                peer.mining = None;
                peer.consumption = None;
                peer.container = None;
            }
            return;
        }
        let Some(food) = held else {
            if animal.needs.dragon.is_some()
                && world.journal.gameplay().creatures.owners.get(&id) == Some(&peer.uuid)
            {
                let state = world.journal.gameplay_mut();
                if let Some(animal) = state.creatures.animals.get_mut(&id)
                    && let Some(life) = &mut animal.needs.dragon
                    && life.pet(animal.age)
                {
                    peer.creature_action_at = Instant::now();
                    state.revision = state.revision.saturating_add(1);
                }
            }
            return;
        };
        let state = world.journal.gameplay();
        let breed = terra_core::creatures::breeding::can_offer(state, animal, peer.uuid, food);
        let age = animal.age;
        let offer_age =
            terra_core::creatures::breeding::feeding_age(animal, state.animal_families.get(&id));
        let mut offered_animal = animal.clone();
        offered_animal.age = offer_age;
        let Some(bond) = terra_core::creatures::taming::offered(
            &offered_animal,
            &state.creatures,
            state.animal_bonds.get(&id).copied(),
            peer.uuid,
            food,
        )
        .or_else(|| {
            breed.then(|| {
                let mut bond =
                    state.animal_bonds.get(&id).copied().unwrap_or_else(|| {
                        terra_core::creatures::taming::AnimalBond::owned(peer.uuid)
                    });
                bond.last_fed_age = Some(offer_age);
                bond
            })
        }) else {
            return;
        };
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
            if let Some(life) = a.needs.dragon.as_mut() {
                life.affection = (life.affection + 1.).min(100.);
            }
            if !a.kind.is_dragon()
                || !matches!(
                    a.behavior,
                    CreatureBehavior::Leap | CreatureBehavior::Land | CreatureBehavior::Fly
                )
            {
                companion_feeding_pose(a);
            }
        }
        let state = world.journal.gameplay_mut();
        state.animal_bonds.insert(id, bond);
        if breed {
            state
                .animal_families
                .entry(id)
                .or_default()
                .willing_until_age = age + terra_core::creatures::breeding::WILLING_SECONDS;
        }
        if bond.trust == 100 {
            state.creatures.owners.insert(id, peer.uuid);
        }
        state.revision = state.revision.wrapping_add(1);
        if let Some(animal) = state.creatures.animals.get(&id)
            && let Some(mount) = world.mounts.active.values_mut().find(|m| m.horse.id == id)
        {
            mount.horse = animal.clone();
        }
    } else {
        let creative = peer.game_mode(&world.config) == terra_core::GameMode::Creative;
        if !creative && peer.vitals.stamina < 2.0 {
            return;
        }
        let damage = gameplay::held_tool(peer, world)
            .and_then(terra_core::ItemKind::tool_kind)
            .map_or(1.0, terra_core::ToolKind::attack_damage)
            * peer.vitals.effects.attack_multiplier();
        let damage = terra_core::creatures::dragons::incoming_damage(animal, damage);
        let dies = animal.health <= damage;
        let needed = forest_loot.iter().flatten().count()
            + if product.is_none() {
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
            if kind == CreatureKind::Hen
                && damage > 0.0
                && let Some(alerts) = world.poultry_alerts.as_deref_mut()
            {
                alerts.report(id, poultry::Actor::Player(peer.uuid));
            }
        }
        if let Some(bond) = world.journal.gameplay_mut().animal_bonds.get_mut(&id)
            && bond.trust < 100
        {
            bond.last_fed_age = None;
        }
        if let Some(a) = world.journal.gameplay_mut().creatures.animals.get_mut(&id) {
            a.health = (a.health - damage).max(0.0);
            if a.kind.is_dragon() {
                if let Some(challengers) = world.dragon_challengers.as_deref_mut() {
                    challengers.0.insert((id, peer.uuid), Instant::now());
                }
                if dies {
                    a.behavior = CreatureBehavior::Dead;
                    a.timer = terra_core::creatures::dragons::duration(a.behavior);
                    a.velocity = [0.; 3];
                    a.needs.goal = None;
                }
            } else if a.kind == CreatureKind::StoneGolem && !dies {
                terra_core::creatures::golem::hit(a);
            } else if a.kind == CreatureKind::LavaWorm && !dies {
                a.behavior = CreatureBehavior::Retreat;
                a.timer = terra_core::creatures::lava_worm::DIVE;
                a.needs.goal = None;
                a.velocity = [0.; 3];
            } else if a.kind == CreatureKind::SandWorm && !dies {
                terra_core::creatures::sandworm::hit_reaction(a, eye);
            } else {
                a.behavior = if dies {
                    CreatureBehavior::Dead
                } else {
                    CreatureBehavior::Flee
                };
                a.timer = 5.0;
                a.yaw = ((eye[0] - position[0]).atan2(eye[2] - position[2])) as f32;
            }
        }
        if kind == CreatureKind::Villager
            && let Some(defenses) = world.defenses.as_deref_mut()
        {
            defenses.provoke(id, peer.uuid, world.journal.gameplay());
        }
        if dies {
            for stack in forest_loot.into_iter().flatten() {
                world
                    .journal
                    .gameplay_mut()
                    .drop_stack(position, stack, [0.0, 1.8, 0.0], 0.35);
            }
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

#[cfg(test)]
mod spirit_aura_tests {
    use super::refresh_spirit_regeneration;
    use terra_core::{EffectKind, SurvivalVitals};

    #[test]
    fn overlapping_glooms_do_not_stack_or_downgrade_potions() {
        let mut vitals = SurvivalVitals::default();
        for _ in 0..100 {
            refresh_spirit_regeneration(&mut vitals);
        }
        let effect = vitals.effects.get(EffectKind::Regeneration).unwrap();
        assert_eq!(effect.level, 1);
        assert_eq!(effect.remaining_seconds, 2.0);
        vitals.effects.apply(EffectKind::Regeneration, 30.0, 2);
        refresh_spirit_regeneration(&mut vitals);
        let effect = vitals.effects.get(EffectKind::Regeneration).unwrap();
        assert_eq!(effect.level, 2);
        assert_eq!(effect.remaining_seconds, 30.0);
    }
}
