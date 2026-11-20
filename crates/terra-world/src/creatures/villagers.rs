//! Settlement schedules use the same loaded-voxel steering and bounded A* as animals.
use super::{Behavior, Creature, Habitat, Kind, distance, navigation, support};
use crate::generation::structures::ResidentSite;
use std::collections::BTreeMap;
use terra_core::creatures::{
    CreatureNeeds, CreaturePopulation, NeedGoal, NeedKind, soldiers::SoldierLoadout, villagers,
};
pub(super) mod patrol;

#[derive(Default)]
pub struct Residents {
    sites: BTreeMap<u64, ResidentSite>,
}

fn home(site: &ResidentSite) -> [i64; 2] {
    [
        site.village_center[0].div_euclid(32),
        site.village_center[2].div_euclid(32),
    ]
}

fn resident_id(site: &ResidentSite) -> Option<u64> {
    villagers::id(home(site), site.slot)
}

impl Residents {
    /// Called at 1 Hz. Existing dead residents remain tombstones and are never rerolled.
    pub fn discover(
        &mut self,
        population: &mut CreaturePopulation,
        h: &Habitat,
        players: &[[f64; 3]],
    ) {
        self.sites.retain(|_, site| {
            players.iter().any(|p| {
                let center = site.village_center.map(|v| v as f64);
                (p[0] - center[0]).hypot(p[2] - center[2]) < 320.0
            })
        });
        for site in h.chunks.village_residents() {
            if self.sites.len() >= 1024 || site.slot >= villagers::MAX_RESIDENTS {
                continue;
            }
            let Some(id) = resident_id(site) else {
                continue;
            };
            if self.sites.contains_key(&id)
                || !players
                    .iter()
                    .any(|p| distance(*p, site.bed.map(|v| v as f64)) < 175.0_f64.powi(2))
            {
                continue;
            }
            self.sites.insert(id, site.clone());
        }
        for (&id, site) in &self.sites {
            if population.animals.contains_key(&id)
                || population.animals.len() >= terra_core::creatures::MAX_CREATURES
            {
                continue;
            }
            if !assigned_bed(site, h) {
                continue;
            }
            // Residents enter at a clear floor near their own doorway, never inside a wall.
            let candidates = [
                [0.0, 1.5],
                [1.5, 0.0],
                [-1.5, 0.0],
                [0.0, -1.5],
                [2.5, 1.5],
                [-2.5, -1.5],
            ];
            let occupied = |p: [f64; 3]| {
                population
                    .animals
                    .values()
                    .any(|a| a.health > 0.0 && distance(a.position, p) < 0.6_f64.powi(2))
            };
            // Large barracks have many beds sharing a door. Prefer each resident's
            // own verified bedside so the initial garrison is not serialized
            // through six occupied doorway positions.
            let bedside =
                exit_bed_where(site, h, site.bed.map(|v| v as f64 + 0.5), |p| !occupied(p));
            let Some(position) = bedside.or_else(|| {
                candidates.into_iter().find_map(|[x, z]| {
                    let p = [
                        site.door[0] as f64 + 0.5 + x,
                        site.door[1] as f64,
                        site.door[2] as f64 + 0.5 + z,
                    ];
                    h.ground(p, Kind::Villager).filter(|p| {
                        !population
                            .animals
                            .values()
                            .any(|a| a.health > 0.0 && distance(a.position, *p) < 0.6_f64.powi(2))
                    })
                })
            }) else {
                continue;
            };
            population.animals.insert(
                id,
                Creature {
                    id,
                    kind: Kind::Villager,
                    home: home(site),
                    position,
                    velocity: [0.0; 3],
                    yaw: f32::from(site.bed_facing) * std::f32::consts::FRAC_PI_2,
                    health: Kind::Villager.health(),
                    behavior: Behavior::Rest,
                    timer: (site.slot % 7) as f32 * 0.3,
                    age: 0.0,
                    variant: site.role.min(villagers::ROLE_COUNT - 1),
                    needs: CreatureNeeds::full(Kind::Villager),
                },
            );
        }
    }

    /// Returns a nearby closed home door to open through the existing object-state path.
    pub fn step(
        &self,
        resident: &mut Creature,
        habitat: &Habitat,
        phase: f64,
        dt: f32,
        search: bool,
        navigation: &mut navigation::Navigation,
    ) -> Option<[i64; 3]> {
        self.step_as(resident, habitat, phase, dt, search, navigation, None)
    }

    /// Owned residents carry their actual actor rights; native residents remain
    /// inside their settlement and may not enter another player's claims.
    #[allow(clippy::too_many_arguments)]
    pub fn step_as(
        &self,
        resident: &mut Creature,
        habitat: &Habitat,
        phase: f64,
        dt: f32,
        search: bool,
        navigation: &mut navigation::Navigation,
        actor: Option<terra_core::PlayerUuid>,
    ) -> Option<[i64; 3]> {
        if resident.health <= 0.0
            || !dt.is_finite()
            || !(0.001..=0.25).contains(&dt)
            || habitat
                .block(resident.position.map(|v| v.floor() as i64))
                .is_none()
        {
            return None;
        }
        resident.age += f64::from(dt);
        resident.needs.search_in = (resident.needs.search_in - dt).max(0.0);
        resident.timer = (resident.timer - dt).max(0.0);
        resident.needs.food = (resident.needs.food - dt * 0.003).max(0.0);
        resident.needs.water = (resident.needs.water - dt * 0.004).max(0.0);
        if let Some(actor) = actor
            && resident.kind == Kind::Villager
        {
            let military = SoldierLoadout::of(resident);
            let sleep = military.map_or_else(
                || villagers::sleeping(resident.id, phase),
                |loadout| loadout.sleeping(resident.id, phase),
            );
            let home_needed = self.sites.get(&resident.id).is_some_and(|site| {
                // Leave a generated home through its real doorway before taking
                // a claim patrol goal. The home schedule also owns the authorized
                // opening request while the resident is still inside.
                inside_home(site, resident.position)
                    || (assigned_bed(site, habitat)
                        && (sleep
                            || resident.behavior == Behavior::Sleep
                            || resident.needs.food < resident.kind.food() * 0.45
                            || resident.needs.water < resident.kind.water() * 0.45))
            });
            if !home_needed {
                if military.is_none() && sleep {
                    // A homeless civilian still rests at night; owned guards
                    // retain their existing night-patrol behavior.
                    resident.needs.goal = None;
                    resident.timer = 0.5;
                    navigation.invalidate(resident.id);
                }
                return patrol::step(resident, habitat, dt, search, navigation, actor);
            }
        }
        let Some(site) = self.sites.get(&resident.id) else {
            resident.velocity[0] = 0.0;
            resident.velocity[2] = 0.0;
            resident.behavior = Behavior::Rest;
            support::settle(resident, habitat, dt);
            return None;
        };
        let access = |point| resident_access(habitat, site, actor, point);
        if !access(resident.position) {
            navigation.invalidate(resident.id);
            resident.velocity = [0.0; 3];
            resident.behavior = Behavior::Rest;
            return None;
        }
        let military = SoldierLoadout::of(resident);
        let sleep = military.map_or_else(
            || villagers::sleeping(resident.id, phase),
            |loadout| loadout.sleeping(resident.id, phase),
        );
        let bed_exists = assigned_bed(site, habitat);
        if resident.behavior == Behavior::Sleep && (!sleep || !bed_exists) {
            // Leave the bed only onto a verified adjacent floor, not by snapping through walls.
            resident.position =
                exit_bed(site, habitat, resident.position).filter(|point| access(*point))?;
            resident.behavior = Behavior::Rest;
            resident.needs.goal = None;
            resident.timer = 1.0;
            navigation.forget(resident.id);
        }
        if sleep && bed_exists && resident.behavior == Behavior::Sleep {
            resident.needs.food = (resident.needs.food + dt * 0.025).min(resident.kind.food());
            resident.needs.water = (resident.needs.water + dt * 0.025).min(resident.kind.water());
            resident.health = (resident.health + dt * 0.01).min(resident.kind.health());
            resident.velocity = [0.0; 3];
            return None;
        }
        let mut door = None;
        if distance(resident.position, site.door.map(|v| v as f64 + 0.5)) < 2.5_f64.powi(2)
            && habitat
                .block(site.door)
                .is_some_and(crate::npc_work::worker_openable_door)
            && resident_door_allowed(habitat, site, actor)
            && !crate::objects::is_open(
                habitat.block(site.door).unwrap_or(0),
                habitat.journal.object_state(site.door),
            )
        {
            door = Some(site.door);
        }
        let hungry = resident.needs.food < resident.kind.food() * 0.45
            || resident.needs.water < resident.kind.water() * 0.45;
        let going_home = (sleep || hungry) && bed_exists;
        let bedside = going_home
            .then(|| exit_bed(site, habitat, resident.position))
            .flatten();
        if going_home {
            let point = site.bed.map(|v| v as f64 + 0.5);
            if inside_home(site, resident.position)
                && bedside.is_some_and(|entry| {
                    distance(resident.position, entry) < 0.65_f64.powi(2)
                        && habitat
                            .walk_step(
                                resident.position,
                                entry,
                                resident.kind,
                                f64::from(resident.yaw),
                            )
                            .is_some()
                })
            {
                resident.velocity = [0.0; 3];
                resident.needs.goal = None;
                navigation.forget(resident.id);
                if sleep {
                    let d = [
                        [0.0, 0.0, 1.0],
                        [1.0, 0.0, 0.0],
                        [0.0, 0.0, -1.0],
                        [-1.0, 0.0, 0.0],
                    ][usize::from(site.bed_facing % 4)];
                    resident.position = [
                        point[0] - d[0] * 0.4,
                        site.bed[1] as f64 + 0.79,
                        point[2] - d[2] * 0.4,
                    ];
                    resident.yaw = f32::from(site.bed_facing % 4) * std::f32::consts::FRAC_PI_2;
                    resident.behavior = Behavior::Sleep;
                } else {
                    resident.behavior = Behavior::Graze;
                    resident.needs.food =
                        (resident.needs.food + dt * 2.5).min(resident.kind.food());
                    resident.needs.water =
                        (resident.needs.water + dt * 2.5).min(resident.kind.water());
                }
                return door;
            }
            let (outside, inside) = door_points(site, habitat);
            let inside_cell = inside.map(|v| v.floor() as i64);
            // Keep the gate phase stable while a local detour crosses the
            // threshold. Re-selecting the outer portal here causes ping-pong.
            let entering = resident
                .needs
                .goal
                .is_some_and(|g| g.position == inside_cell)
                || in_doorway(site, habitat, resident.position);
            let position = if inside_home(site, resident.position) {
                site.bed
            } else if entering {
                inside_cell
            } else {
                outside.map(|v| v.floor() as i64)
            };
            if resident
                .needs
                .goal
                .is_some_and(|old| old.position != position)
            {
                navigation.forget(resident.id);
            }
            resident.needs.goal = Some(NeedGoal {
                kind: NeedKind::Food,
                position,
            });
        } else if resident.timer > 0.0 {
            resident.velocity[0] = 0.0;
            resident.velocity[2] = 0.0;
            resident.behavior = Behavior::Rest;
            support::settle(resident, habitat, dt);
            return door;
        } else if resident.needs.goal.is_none() && search {
            // Short, reproducible walks keep residents in their settlement and respect slopes.
            if inside_home(site, resident.position) {
                let (_, inside) = door_points(site, habitat);
                resident.needs.goal = Some(NeedGoal {
                    kind: NeedKind::Food,
                    position: inside.map(|v| v.floor() as i64),
                });
            } else {
                if military.is_some()
                    && let Some(position) = military_patrol_goal(site, resident, habitat)
                        .filter(|position| access(position.map(|v| v as f64)))
                {
                    resident.needs.goal = Some(NeedGoal {
                        kind: NeedKind::Food,
                        position,
                    });
                }
                if military.is_none()
                    && let Some(position) = profession_goal(site, resident, habitat)
                        .filter(|position| access(position.map(|v| v as f64)))
                {
                    resident.needs.goal = Some(NeedGoal {
                        kind: NeedKind::Food,
                        position,
                    });
                }
                let hash = super::random(resident.id, (resident.age / 7.0) as u64);
                for n in 0..6 {
                    if resident.needs.goal.is_some() {
                        break;
                    }
                    let angle = ((hash.rotate_left(n * 7) & 65535) as f64 / 65535.0)
                        * std::f64::consts::TAU;
                    let radius = if villagers::is_traveller(resident.variant) {
                        5.0 + ((hash >> (n * 5)) & 7) as f64
                    } else {
                        3.0 + ((hash >> (n * 5)) & 7) as f64
                    };
                    let p = [
                        resident.position[0] + angle.sin() * radius,
                        resident.position[1],
                        resident.position[2] + angle.cos() * radius,
                    ];
                    if p[0] < site.village_bounds[0][0] as f64 + 2.0
                        || p[0] > site.village_bounds[1][0] as f64 - 2.0
                        || p[2] < site.village_bounds[0][2] as f64 + 2.0
                        || p[2] > site.village_bounds[1][2] as f64 - 2.0
                        || inside_home(site, p)
                    {
                        continue;
                    }
                    if let Some(p) = habitat
                        .ground(p, Kind::Villager)
                        .filter(|point| access(*point))
                    {
                        resident.needs.goal = Some(NeedGoal {
                            kind: NeedKind::Food,
                            position: p.map(|v| v.floor() as i64),
                        });
                        break;
                    }
                }
                // A narrow ledge can support the body while every strict path
                // probe fails. Without a goal the stalled-route branch never runs.
                if resident.needs.goal.is_none()
                    && habitat
                        .ground_rotated(resident.position, resident.kind, f64::from(resident.yaw))
                        .is_none()
                    && let Some(exit) = local_exit(resident, habitat, dt)
                {
                    navigation.escape_via(resident.id, exit, resident.age);
                    resident.needs.goal = Some(NeedGoal {
                        kind: NeedKind::Food,
                        position: exit.map(|v| v.floor() as i64),
                    });
                }
            }
            resident.needs.search_in = 2.0;
        }
        if let Some(mut goal) = resident.needs.goal {
            let (outside, inside) = door_points(site, habitat);
            let target = if going_home {
                if inside_home(site, resident.position) {
                    bedside.unwrap_or(inside)
                } else if goal.position == inside.map(|v| v.floor() as i64) {
                    inside
                } else {
                    outside
                }
            } else if inside_home(site, resident.position) {
                if goal.position == outside.map(|v| v.floor() as i64)
                    || (resident.position[0] - inside[0]).hypot(resident.position[2] - inside[2])
                        < 0.05
                {
                    let position = outside.map(|v| v.floor() as i64);
                    if goal.position != position {
                        navigation.forget(resident.id);
                    }
                    goal.position = position;
                    resident.needs.goal = Some(goal);
                    outside
                } else {
                    inside
                }
            } else {
                let point = [
                    goal.position[0] as f64 + 0.5,
                    goal.position[1] as f64,
                    goal.position[2] as f64 + 0.5,
                ];
                habitat.ground(point, Kind::Villager).unwrap_or(point)
            };
            let escape = navigation.escape(resident);
            let target = escape.unwrap_or(target);
            if escape.is_none()
                && !going_home
                && !inside_home(site, resident.position)
                && navigation::ground_arrived(resident, habitat, target, 0.5)
            {
                resident.needs.goal = None;
                resident.timer = 1.5 + (resident.id % 35) as f32 * 0.1;
                resident.behavior = Behavior::Rest;
                resident.velocity[0] = 0.0;
                resident.velocity[2] = 0.0;
            } else {
                walk_with_access(
                    resident,
                    habitat,
                    target,
                    dt,
                    search,
                    escape.is_some(),
                    navigation,
                    access,
                );
                if navigation.stalled(resident.id, resident.age) {
                    // An unreachable destination must not keep winning the
                    // same A* retry. Take a verified local exit before exploring.
                    let escape = local_exit(resident, habitat, dt);
                    if !going_home {
                        resident.needs.goal = None;
                    }
                    resident.needs.search_in = 0.3;
                    resident.timer = 0.5 + (resident.id % 7) as f32 * 0.1;
                    navigation.forget(resident.id);
                    if let Some(p) = escape {
                        navigation.escape_via(resident.id, p, resident.age);
                        // Keep a route owner while the exact sub-voxel exit is
                        // active; household doorway goals must not override it.
                        if resident.needs.goal.is_none() {
                            resident.needs.goal = Some(NeedGoal {
                                kind: NeedKind::Food,
                                position: p.map(|v| v.floor() as i64),
                            });
                        }
                    }
                }
            }
        } else {
            resident.behavior = Behavior::Rest;
            resident.velocity[0] = 0.0;
            resident.velocity[2] = 0.0;
        }
        support::settle(resident, habitat, dt);
        door
    }
}

fn resident_access(
    h: &Habitat,
    site: &ResidentSite,
    actor: Option<terra_core::PlayerUuid>,
    point: [f64; 3],
) -> bool {
    let registry = &h.journal.gameplay().kingdoms;
    let home = registry
        .region_at(site.village_center)
        .map(|record| registry.realm_id(record.id));
    [-0.3, 0.3].into_iter().all(|dx| {
        [-0.3, 0.3].into_iter().all(|dz| {
            let cell = [
                (point[0] + dx).floor() as i64,
                point[1].floor() as i64,
                (point[2] + dz).floor() as i64,
            ];
            if let Some(actor) = actor {
                crate::kingdom_access::can_use(
                    registry,
                    h.chunks,
                    actor,
                    terra_core::kingdom::KingdomPermission::NpcWork,
                    cell,
                )
            } else {
                registry
                    .region_at(cell)
                    .is_none_or(|record| Some(registry.realm_id(record.id)) == home)
            }
        })
    })
}

fn resident_door_allowed(
    h: &Habitat,
    site: &ResidentSite,
    actor: Option<terra_core::PlayerUuid>,
) -> bool {
    let Some(material) = h
        .block(site.door)
        .filter(|material| crate::npc_work::worker_openable_door(*material))
    else {
        return false;
    };
    let Some(state) = h.journal.object_state(site.door) else {
        return false;
    };
    let Some(group) = crate::objects::doors::group(site.door, material, state, |cell| {
        Some((h.block(cell)?, h.journal.object_state(cell)?))
    }) else {
        return false;
    };
    group.cells().all(|cell| {
        h.block(cell) == Some(material)
            && resident_access(h, site, actor, cell.map(|v| v as f64 + 0.5))
            && actor.is_none_or(|actor| {
                crate::kingdom_access::can_use(
                    &h.journal.gameplay().kingdoms,
                    h.chunks,
                    actor,
                    terra_core::kingdom::KingdomPermission::Openings,
                    cell,
                )
            })
    })
}

/// Reuses normal loaded-voxel walking to visit nearby workplaces without editing their contents.
fn profession_goal(
    site: &ResidentSite,
    resident: &Creature,
    habitat: &Habitat,
) -> Option<[i64; 3]> {
    if !matches!(
        resident.variant,
        1 | 8 | villagers::ENGINEER | villagers::MINER | villagers::BUTCHER | villagers::SELLER
    ) {
        return None;
    }
    let origin = resident.position.map(|v| v.floor() as i64);
    let first = (resident.id.wrapping_add((resident.age / 7.0) as u64) % 8) as usize;
    let directions = [
        [1, 0],
        [1, 1],
        [0, 1],
        [-1, 1],
        [-1, 0],
        [-1, -1],
        [0, -1],
        [1, -1],
    ];
    // At most 96 marker checks per existing budgeted navigation search.
    for radius in 1..=4 {
        for index in 0..8 {
            let direction = directions[(index + first) % 8];
            for height in -1..=1 {
                let target = [
                    origin[0] + direction[0] * radius,
                    origin[1] + height,
                    origin[2] + direction[1] * radius,
                ];
                if !habitat
                    .block(target)
                    .is_some_and(|material| workplace_matches(resident.variant, material))
                {
                    continue;
                }
                for [x, z] in [[1, 0], [-1, 0], [0, 1], [0, -1]] {
                    let point = [
                        (target[0] + x) as f64 + 0.5,
                        resident.position[1],
                        (target[2] + z) as f64 + 0.5,
                    ];
                    if [0, 2].into_iter().any(|axis| {
                        point[axis] <= site.village_bounds[0][axis] as f64 + 1.0
                            || point[axis] >= site.village_bounds[1][axis] as f64 - 1.0
                    }) || inside_home(site, point)
                    {
                        continue;
                    }
                    if let Some(point) = habitat.ground(point, Kind::Villager) {
                        return Some(point.map(|v| v.floor() as i64));
                    }
                }
            }
        }
    }
    None
}

fn workplace_matches(role: u8, material: u16) -> bool {
    match role {
        1 => material == terra_core::farming::FARMLAND,
        8 => material == 59,
        villagers::ENGINEER => {
            material == terra_core::train_parts::STATION || terra_core::rails::is_rail(material)
        }
        villagers::MINER => terra_core::OreKind::from_material(material).is_some(),
        villagers::BUTCHER => material == terra_core::campfire::MATERIAL,
        villagers::SELLER => material == 29,
        _ => false,
    }
}

fn assigned_bed(site: &ResidentSite, h: &Habitat) -> bool {
    let Some(material) = h.block(site.bed).filter(|&b| terra_core::beds::is_bed(b)) else {
        return false;
    };
    let Some(foot) = h.journal.object_state(site.bed) else {
        return false;
    };
    if foot.secondary
        || foot.facing % 4 != site.bed_facing % 4
        || !foot.valid_for(site.bed, material)
    {
        return false;
    }
    let Some(head) = foot.peer else { return false };
    h.block(head) == Some(material)
        && h.journal.object_state(head).is_some_and(|state| {
            state.secondary
                && state.peer == Some(site.bed)
                && state.facing == foot.facing
                && state.valid_for(head, material)
        })
}

/// Ordered sentry posts around the inhabited bounds. Intermediate goals stay
/// within the existing bounded navigation radius and never jump across walls.
fn military_patrol_goal(site: &ResidentSite, a: &Creature, h: &Habitat) -> Option<[i64; 3]> {
    let [min, max] = site.village_bounds;
    let inset = 5.0;
    let x0 = min[0] as f64 + inset;
    let x1 = max[0] as f64 - inset;
    let z0 = min[2] as f64 + inset;
    let z1 = max[2] as f64 - inset;
    if x0 >= x1 || z0 >= z1 {
        return None;
    }
    let posts = [[x0, z0], [x1, z0], [x1, z1], [x0, z1]];
    let first = ((a.age / 35.0) as usize + a.id as usize) % 4;
    for n in 0..4 {
        let [x, z] = posts[(first + n) % 4];
        let dx = x - a.position[0];
        let dz = z - a.position[2];
        let length = dx.hypot(dz);
        if length < 1.0 {
            continue;
        }
        let step = 12.0_f64.min(length) / length;
        let point = [
            a.position[0] + dx * step,
            a.position[1],
            a.position[2] + dz * step,
        ];
        if !inside_home(site, point)
            && let Some(point) = h.ground(point, Kind::Villager)
        {
            return Some(point.map(|v| v.floor() as i64));
        }
    }
    None
}

fn inside_home(site: &ResidentSite, p: [f64; 3]) -> bool {
    [0, 2].into_iter().all(|a| {
        p[a] > site.home_bounds[0][a] as f64 + 2.0 && p[a] < site.home_bounds[1][a] as f64 - 1.0
    })
}

fn door_points(site: &ResidentSite, h: &Habitat) -> ([f64; 3], [f64; 3]) {
    let facing = h
        .journal
        .object_state(site.door)
        .map_or(site.bed_facing, |s| s.facing);
    let inward = [[0.0, 1.0], [1.0, 0.0], [0.0, -1.0], [-1.0, 0.0]][usize::from(facing % 4)];
    let at = |direction: f64| {
        [
            site.door[0] as f64 + 0.5 + inward[0] * direction,
            site.door[1] as f64,
            site.door[2] as f64 + 0.5 + inward[1] * direction,
        ]
    };
    (at(-1.25), at(1.25))
}

fn in_doorway(site: &ResidentSite, h: &Habitat, p: [f64; 3]) -> bool {
    let (outside, inside) = door_points(site, h);
    let inward = [
        (inside[0] - outside[0]) / 2.5,
        (inside[2] - outside[2]) / 2.5,
    ];
    let offset = [
        p[0] - (outside[0] + inside[0]) * 0.5,
        p[2] - (outside[2] + inside[2]) * 0.5,
    ];
    let along = offset[0] * inward[0] + offset[1] * inward[1];
    let across = offset[0] * inward[1] - offset[1] * inward[0];
    (along + 1.25).abs() < 0.05 && across.abs() < 0.035
}

fn exit_bed(site: &ResidentSite, h: &Habitat, from: [f64; 3]) -> Option<[f64; 3]> {
    exit_bed_where(site, h, from, |_| true)
}

fn exit_bed_where(
    site: &ResidentSite,
    h: &Habitat,
    from: [f64; 3],
    allowed: impl Fn([f64; 3]) -> bool,
) -> Option<[f64; 3]> {
    let d = [[0, 1], [1, 0], [0, -1], [-1, 0]][usize::from(site.bed_facing % 4)];
    [[d[1], -d[0]], [-d[1], d[0]], [-d[0], -d[1]]]
        .into_iter()
        .filter_map(|[x, z]| {
            h.ground(
                [
                    site.bed[0] as f64 + 0.5 + x as f64,
                    site.bed[1] as f64,
                    site.bed[2] as f64 + 0.5 + z as f64,
                ],
                Kind::Villager,
            )
            .filter(|p| inside_home(site, *p) && (p[1] - site.bed[1] as f64).abs() < 0.3)
            .filter(|p| p[1] <= site.bed[1] as f64 + 0.2 && allowed(*p))
        })
        .min_by(|a, b| distance(*a, from).total_cmp(&distance(*b, from)))
}

fn physical_step(h: &Habitat, start: [f64; 3], target: [f64; 3], yaw: f64) -> Option<[f64; 3]> {
    h.travel_step(start, target, Kind::Villager, yaw)
        .or_else(|| {
            // Walking off a low ledge must enter the existing swept gravity path,
            // not wait forever for all feet to reach the lower floor simultaneously.
            let floor = [1.0, 2.0].into_iter().find_map(|drop| {
                h.ground_rotated(
                    [target[0], target[1] - drop, target[2]],
                    Kind::Villager,
                    yaw,
                )
                .filter(|p| p[1] < start[1] && start[1] - p[1] <= 3.01)
            })?;
            let length = (target[0] - start[0]).hypot(target[2] - start[2]);
            if length > 1.5 || !h.clear_swimmer_rotated(start, Kind::Villager.size(), yaw) {
                return None;
            }
            let count = (length / 0.1).ceil().max(1.0) as usize;
            (1..=count)
                .all(|n| {
                    let p = std::array::from_fn(|i| {
                        start[i] + (target[i] - start[i]) * n as f64 / count as f64
                    });
                    h.clear_swimmer_rotated(p, Kind::Villager.size(), yaw)
                })
                .then_some([target[0], start[1].max(floor[1]), target[2]])
        })
}

fn local_exit(a: &Creature, h: &Habitat, dt: f32) -> Option<[f64; 3]> {
    [1.0, 0.65, 1.4].into_iter().find_map(|radius| {
        (0..8).find_map(|n| {
            let angle = f64::from(n) * std::f64::consts::FRAC_PI_4;
            let p = [
                a.position[0] + angle.sin() * radius,
                a.position[1],
                a.position[2] + angle.cos() * radius,
            ];
            // Back out without rotating the body through furniture or a narrow
            // doorframe. Both the first step and the whole exit must be swept.
            let yaw = f64::from(a.yaw);
            let first = [
                a.position[0] + angle.sin() * a.kind.speed() * f64::from(dt),
                a.position[1],
                a.position[2] + angle.cos() * a.kind.speed() * f64::from(dt),
            ];
            physical_step(h, a.position, first, yaw)?;
            physical_step(h, a.position, p, yaw)
                .filter(|end| (end[0] - a.position[0]).hypot(end[2] - a.position[2]) > 0.5)
        })
    })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn walk_with_access(
    a: &mut Creature,
    h: &Habitat,
    target: [f64; 3],
    dt: f32,
    search: bool,
    strafe: bool,
    nav: &mut navigation::Navigation,
    allowed: impl Fn([f64; 3]) -> bool,
) {
    if navigation::ground_arrived(a, h, target, 0.05) {
        a.velocity[0] = 0.0;
        a.velocity[2] = 0.0;
        a.behavior = Behavior::Rest;
        nav.ground_unblocked(a.id);
        return;
    }
    nav.intend(
        a,
        terra_core::creatures::intentions::IntentReason::Explore,
        Some(target),
    );
    let before = a.position;
    let movement = |point: [f64; 3]| {
        let dx = point[0] - before[0];
        let dz = point[2] - before[2];
        let length = dx.hypot(dz);
        if length < 0.00001 {
            return None;
        }
        let amount = (a.kind.speed() * f64::from(dt)).min(length);
        let yaw = if strafe {
            f64::from(a.yaw)
        } else {
            (-dx).atan2(-dz)
        };
        physical_step(
            h,
            before,
            [
                before[0] + dx / length * amount,
                before[1],
                before[2] + dz / length * amount,
            ],
            yaw,
        )
        .filter(|p| (p[0] - before[0]).hypot(p[2] - before[2]) > 0.00001)
        .filter(|p| {
            let steps = ((p[0] - before[0]).hypot(p[2] - before[2]) / 0.18)
                .ceil()
                .clamp(1.0, 64.0) as usize;
            (0..=steps).all(|step| {
                allowed(std::array::from_fn(|axis| {
                    before[axis] + (p[axis] - before[axis]) * step as f64 / steps as f64
                }))
            })
        })
        .map(|p| (p, yaw))
    };
    let detouring = nav.following_ground_route(a.id);
    // The shared graph uses one-meter samples. Finish a clear short segment
    // precisely, otherwise its raster waypoints can orbit a narrow doorway.
    let precise_finish = detouring
        && distance(before, target) < 3.0_f64.powi(2)
        && h.walk_step(
            before,
            target,
            a.kind,
            (before[0] - target[0]).atan2(before[2] - target[2]),
        )
        .is_some_and(|point| (point[1] - target[1]).abs() <= 0.125);
    let direct = (!detouring || precise_finish)
        .then(|| movement(target))
        .flatten();
    let next = direct
        .or_else(|| {
            navigation::ground_waypoint_with_access(a, h, target, search, Some(nav), &allowed)
                .and_then(movement)
        })
        .filter(|(p, _)| (p[0] - before[0]).hypot(p[2] - before[2]) > 0.00001);
    if let Some((p, yaw)) = next {
        let falling = a.velocity[1].min(0.0);
        a.position = p;
        a.yaw = yaw as f32;
        a.velocity = std::array::from_fn(|i| ((p[i] - before[i]) / f64::from(dt)) as f32);
        a.velocity[1] = falling;
        a.behavior = Behavior::Wander;
        if direct.is_some() {
            nav.ground_unblocked(a.id);
        }
    } else {
        a.velocity[0] = 0.0;
        a.velocity[2] = 0.0;
        a.behavior = Behavior::Rest;
        if !detouring {
            a.needs.search_in = 0.0;
        }
    }
}

#[cfg(test)]
mod settlement_tests;
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        TerraGenerator,
        edits::EditJournal,
        streaming::{ChunkStore, LoadedSection},
    };
    use terra_core::WorldConfig;
    use terra_voxel::{BlockSection, SectionPos};

    fn fixture() -> (TerraGenerator, ChunkStore, EditJournal, Residents, Creature) {
        let terrain = TerraGenerator::new(WorldConfig {
            water: false,
            trees: false,
            caves: false,
            rivers: false,
            biomes: false,
            ..Default::default()
        })
        .unwrap();
        let mut data = BlockSection::empty(SectionPos::default()).unwrap();
        for x in 0..16 {
            for z in 0..16 {
                data.set_block([x, 2, z], 3).unwrap();
            }
        }
        data.set_block([8, 3, 8], 71).unwrap();
        data.set_block([8, 3, 9], 71).unwrap();
        let mut chunks = ChunkStore::default();
        chunks.loaded.insert(
            SectionPos::default(),
            LoadedSection {
                data,
                mesh: None,
                water: None,
                mesh_epoch: 0,
                water_epoch: 0,
                props: vec![],
            },
        );
        let home = [0, 0];
        let id = villagers::id(home, 0).unwrap();
        let site = ResidentSite {
            layout: crate::generation::structures::SettlementLayout::Village,
            slot: 0,
            role: 7,
            bed: [8, 3, 8],
            bed_facing: 0,
            door: [8, 3, 5],
            home_bounds: [[0, 2, 0], [15, 12, 15]],
            village_id: 124,
            village_center: [8, 3, 8],
            village_bounds: [[0, 2, 0], [15, 12, 15]],
        };
        let mut residents = Residents::default();
        residents.sites.insert(id, site);
        let a = Creature {
            id,
            kind: Kind::Villager,
            home,
            position: [7.5, 3.0, 8.5],
            velocity: [0.0; 3],
            yaw: 0.0,
            health: Kind::Villager.health(),
            behavior: Behavior::Rest,
            timer: 0.0,
            age: 0.0,
            variant: 7,
            needs: CreatureNeeds::full(Kind::Villager),
        };
        let mut journal = EditJournal::default();
        for (cell, peer, secondary) in [([8, 3, 8], [8, 3, 9], false), ([8, 3, 9], [8, 3, 8], true)]
        {
            journal.record(cell, 71).unwrap();
            journal
                .set_object_state(
                    cell,
                    Some(crate::edits::BlockObjectState {
                        peer: Some(peer),
                        secondary,
                        ..Default::default()
                    }),
                )
                .unwrap();
        }
        (terrain, chunks, journal, residents, a)
    }

    #[test]
    fn f49_height_changes_sweep_the_old_column_before_advancing_past_a_ceiling() {
        let (terrain, mut chunks, journal, _, _) = fixture();
        let data = &mut chunks.loaded.get_mut(&SectionPos::default()).unwrap().data;
        data.set_block([5, 3, 4], 3).unwrap();
        data.set_block([4, 5, 4], 3).unwrap();
        let start = [4.94, 3.0, 4.5];
        let target = [5.3, 3.0, 4.5];
        let size = [0.1, 1.75, 0.1];
        let h = Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        };
        assert!(h.clear(start, size, false));
        assert!(h.clear([target[0], 4.0, target[2]], size, false));
        assert!(
            h.walk_step_sized(start, target, size, 0.0).is_none(),
            "two clear endpoints cannot step vertically through the old ceiling"
        );
        chunks
            .loaded
            .get_mut(&SectionPos::default())
            .unwrap()
            .data
            .set_block([4, 5, 4], 0)
            .unwrap();
        let h = Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        };
        assert_eq!(h.walk_step_sized(start, target, size, 0.0).unwrap()[1], 4.0);
    }

    #[test]
    fn f49_real_worker_walks_a_stair_detour_to_the_same_xz_on_an_upper_floor() {
        let (terrain, mut chunks, mut journal, _, mut a) = fixture();
        for cell in [[8, 3, 8], [8, 3, 9]] {
            journal.record(cell, 0).unwrap();
            journal.set_object_state(cell, None).unwrap();
            chunks
                .loaded
                .get_mut(&SectionPos::default())
                .unwrap()
                .data
                .set_block(cell.map(|v| v as usize), 0)
                .unwrap();
        }
        let data = &mut chunks.loaded.get_mut(&SectionPos::default()).unwrap().data;
        for x in 6..=10 {
            for z in 6..=10 {
                data.set_block([x, 5, z], 3).unwrap();
            }
        }
        for (x, height) in [(4, 1), (5, 2), (6, 3)] {
            for y in 3..=height + 2 {
                data.set_block([x, y, 5], 3).unwrap();
            }
        }
        a.position = [8.5, 3.0, 8.5];
        let target = [8.5, 6.0, 8.5];
        let h = Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        };
        let mut navigation = navigation::Navigation::default();
        let mut detoured = false;
        for _ in 0..1400 {
            if navigation::ground_arrived(&a, &h, target, 0.2) {
                break;
            }
            let previous = a.position;
            crate::creatures::work_step(&mut a, &h, target, 0.05, &mut navigation);
            assert!((previous[0] - a.position[0]).hypot(previous[2] - a.position[2]) <= 0.16);
            assert!(h.clear(a.position, a.size(), false));
            detoured |= a.position[0] < 6.0;
        }
        assert!(detoured, "must physically reach the side stairs");
        assert!(
            navigation::ground_arrived(&a, &h, target, 0.2),
            "walker at {:?}, route {:?}",
            a.position,
            navigation.inspection(&a)
        );
        assert_eq!(a.position[1], 6.0);
    }

    #[test]
    fn f49_owned_resident_rechecks_claim_rights_before_moving_or_requesting_a_door() {
        use terra_core::{PlayerUuid, kingdom::KingdomRole};
        let (terrain, mut chunks, mut journal, residents, mut a) = fixture();
        for (cell, peer, secondary) in [([8, 3, 5], [8, 4, 5], false), ([8, 4, 5], [8, 3, 5], true)]
        {
            chunks
                .loaded
                .get_mut(&SectionPos::default())
                .unwrap()
                .data
                .set_block(cell.map(|v| v as usize), 65)
                .unwrap();
            journal.record(cell, 65).unwrap();
            journal
                .set_object_state(
                    cell,
                    Some(crate::edits::BlockObjectState {
                        peer: Some(peer),
                        secondary,
                        ..Default::default()
                    }),
                )
                .unwrap();
        }
        let founder = PlayerUuid::from_u128(2);
        let actor = PlayerUuid::from_u128(1);
        let realm = journal
            .gameplay_mut()
            .kingdoms
            .found(founder, [8, 3, 8], "Private")
            .unwrap();
        a.position = [8.5, 3.0, 6.5];
        let original = a.position;
        let mut navigation = navigation::Navigation::default();
        assert!(
            residents
                .step_as(
                    &mut a,
                    &Habitat {
                        terrain: &terrain,
                        chunks: &chunks,
                        journal: &journal
                    },
                    0.5,
                    0.1,
                    true,
                    &mut navigation,
                    Some(actor)
                )
                .is_none()
        );
        assert_eq!(a.position, original);
        assert_eq!(a.behavior, Behavior::Rest);
        journal.gameplay_mut().kingdoms.join(actor, realm).unwrap();
        journal
            .gameplay_mut()
            .kingdoms
            .set_role(founder, realm, actor, KingdomRole::Worker)
            .unwrap();
        assert_eq!(
            residents.step_as(
                &mut a,
                &Habitat {
                    terrain: &terrain,
                    chunks: &chunks,
                    journal: &journal
                },
                0.5,
                0.1,
                true,
                &mut navigation,
                Some(actor)
            ),
            Some([8, 3, 5])
        );
        let authorized_position = a.position;
        journal
            .gameplay_mut()
            .kingdoms
            .set_role(founder, realm, actor, KingdomRole::Visitor)
            .unwrap();
        assert!(
            residents
                .step_as(
                    &mut a,
                    &Habitat {
                        terrain: &terrain,
                        chunks: &chunks,
                        journal: &journal,
                    },
                    0.5,
                    0.1,
                    true,
                    &mut navigation,
                    Some(actor),
                )
                .is_none()
        );
        assert_eq!(a.position, authorized_position);
        assert_eq!(a.behavior, Behavior::Rest);
    }

    #[test]
    fn new_professions_visit_loaded_workplaces_without_mining_or_entering_solids() {
        let (terrain, mut chunks, journal, residents, mut resident) = fixture();
        let mut site = residents.sites[&resident.id].clone();
        site.home_bounds = [[0, 0, 0], [3, 6, 3]];
        let marker = [9, 3, 8];
        chunks
            .loaded
            .get_mut(&SectionPos::default())
            .unwrap()
            .data
            .set_block(marker, terra_core::OreKind::Iron.material())
            .unwrap();
        resident.variant = villagers::MINER;
        let habitat = Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        };
        let goal = profession_goal(&site, &resident, &habitat)
            .expect("loaded ore has a clear adjacent floor");
        assert_ne!(goal, marker.map(|value| value as i64));
        assert_eq!(habitat.block(goal), Some(0));
        assert_eq!(
            habitat.block(marker.map(|value| value as i64)),
            Some(terra_core::OreKind::Iron.material())
        );
        resident.variant = 10;
        assert!(
            profession_goal(&site, &resident, &habitat).is_none(),
            "soldiers retain their patrols"
        );
        resident.variant = villagers::MINER;
        let empty = ChunkStore::default();
        let unknown = Habitat {
            terrain: &terrain,
            chunks: &empty,
            journal: &journal,
        };
        assert!(profession_goal(&site, &resident, &unknown).is_none());
        assert!(workplace_matches(
            villagers::ENGINEER,
            terra_core::train_parts::STATION
        ));
        assert!(workplace_matches(
            villagers::ENGINEER,
            terra_core::rails::STRAIGHT
        ));
        assert!(!workplace_matches(
            villagers::MINER,
            terra_core::rails::STRAIGHT
        ));
        for role in villagers::NOMAD_A..=villagers::NOMAD_D {
            assert!(villagers::is_traveller(role));
        }
    }

    #[test]
    fn maximum_initial_kingdom_population_materializes_all_144_beds_without_doorway_overlap() {
        let (terrain, _, _, _, _) = fixture();
        let mut journal = EditJournal::default();
        let mut chunks = ChunkStore::default();
        for z in 0..3 {
            for x in 0..2 {
                let owner = SectionPos::new(x, 0, z);
                let mut data = BlockSection::empty(owner).unwrap();
                for dx in 0..16 {
                    for dz in 0..16 {
                        data.set_block([dx, 2, dz], 3).unwrap();
                    }
                }
                chunks.loaded.insert(
                    owner,
                    LoadedSection {
                        data,
                        mesh: None,
                        water: None,
                        mesh_epoch: 0,
                        water_epoch: 0,
                        props: vec![],
                    },
                );
            }
        }
        let mut residents = Residents::default();
        for index in 0_u16..144 {
            let bed = [
                4 + i64::from(index % 12) * 2,
                3,
                4 + i64::from(index / 12) * 3,
            ];
            let head = [bed[0], 3, bed[2] + 1];
            for (cell, peer, secondary) in [(bed, head, false), (head, bed, true)] {
                journal.record(cell, 71).unwrap();
                journal
                    .set_object_state(
                        cell,
                        Some(crate::edits::BlockObjectState {
                            peer: Some(peer),
                            secondary,
                            ..Default::default()
                        }),
                    )
                    .unwrap();
            }
            let site = ResidentSite {
                layout: crate::generation::structures::SettlementLayout::KingdomTemperate,
                slot: index,
                role: if index < 80 {
                    0
                } else {
                    10 + ((index - 80) % 3) as u8
                },
                bed,
                bed_facing: 0,
                door: [16, 3, 0],
                home_bounds: [[-2, 2, -2], [33, 8, 49]],
                village_id: 197,
                village_center: [16, 3, 24],
                village_bounds: [[-2, 2, -2], [34, 12, 50]],
            };
            residents.sites.insert(resident_id(&site).unwrap(), site);
        }
        let h = Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        };
        let mut population = CreaturePopulation::default();
        residents.discover(&mut population, &h, &[[16.0, 3.0, 24.0]]);
        assert_eq!(
            population.animals.len(),
            144,
            "Initial counts must not silently truncate at128 or a shared barracks doorway"
        );
        assert_eq!(
            population
                .animals
                .values()
                .filter(|a| SoldierLoadout::of(a).is_some())
                .count(),
            64
        );
        assert!(population.valid());
        let actors = population.animals.values().collect::<Vec<_>>();
        for (i, a) in actors.iter().enumerate() {
            for b in &actors[i + 1..] {
                assert!(distance(a.position, b.position) >= 0.6_f64.powi(2));
            }
        }
    }

    #[test]
    fn military_patrol_uses_loaded_ground_and_dead_residents_are_never_respawned() {
        let (terrain, chunks, journal, mut residents, mut a) = fixture();
        a.variant = 11;
        let site = residents.sites.get_mut(&a.id).unwrap();
        site.role = 11;
        site.home_bounds = [[7, 2, 7], [11, 12, 11]];
        a.position = [4.5, 3.0, 4.5];
        let h = Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        };
        let goal = military_patrol_goal(site, &a, &h).expect("loaded ground sentry waypoint");
        assert!((goal[0] as f64 - a.position[0]).hypot(goal[2] as f64 - a.position[2]) <= 13.0);
        assert!((0..=15).contains(&goal[0]) && (0..=15).contains(&goal[2]));
        a.health = 0.0;
        a.behavior = Behavior::Dead;
        let mut population = CreaturePopulation::default();
        population.animals.insert(a.id, a.clone());
        residents.discover(&mut population, &h, &[[8.0, 3.0, 8.0]]);
        assert_eq!(population.animals.get(&a.id), Some(&a));
        assert!(population.valid());
    }

    #[test]
    fn residents_sleep_on_their_bed_wake_on_valid_floor_and_keep_valid_needs() {
        let (terrain, chunks, journal, residents, mut a) = fixture();
        let h = Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        };
        let mut nav = navigation::Navigation::default();
        residents.step(&mut a, &h, 0.93, 0.1, true, &mut nav);
        assert_eq!(a.behavior, Behavior::Sleep);
        assert!(
            (a.position[1] - 3.79).abs() < 0.001,
            "on mattress not under blanket"
        );
        let sleeping = a.position;
        for _ in 0..100 {
            residents.step(&mut a, &h, 0.1, 0.1, true, &mut nav);
        }
        assert_eq!(a.position, sleeping);
        assert!(a.valid());
        residents.step(&mut a, &h, 0.3, 0.1, true, &mut nav);
        assert_ne!(a.behavior, Behavior::Sleep);
        assert_eq!(a.position[1], 3.0);
        assert!(h.clear(a.position, a.size(), false));
    }

    #[test]
    fn a_goal_less_resident_can_leave_a_narrow_supported_ledge_without_teleporting() {
        let (terrain, mut chunks, journal, mut residents, mut a) = fixture();
        let data = &mut chunks.loaded.get_mut(&SectionPos::default()).unwrap().data;
        for y in 3..=4 {
            data.set_block([4, y, 4], 3).unwrap();
        }
        residents.sites.get_mut(&a.id).unwrap().home_bounds = [[8, 2, 8], [15, 12, 15]];
        a.position = [4.99, 5.0, 4.5];
        a.needs.goal = None;
        let start = a.position;
        let h = Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        };
        assert!(h.ground_rotated(start, a.kind, f64::from(a.yaw)).is_none());
        assert!(
            h.supported_sized(start, a.size(), f64::from(a.yaw))
                .is_some()
        );
        assert!(local_exit(&a, &h, 0.1).is_some());
        let mut nav = navigation::Navigation::default();
        let mut moved = 0.0;
        for _ in 0..40 {
            let before = a.position;
            residents.step(&mut a, &h, 0.5, 0.1, true, &mut nav);
            let step = (a.position[0] - before[0]).hypot(a.position[2] - before[2]);
            assert!(step <= 0.25);
            moved += step;
            assert!(h.clear_swimmer_rotated(a.position, a.size(), f64::from(a.yaw)));
            assert!(a.valid());
        }
        assert!(
            moved > 0.5,
            "resident did not leave the ledge: {:?}",
            a.position
        );
        assert!(a.position[1] < 4.0, "must descend onto the real floor");
    }

    #[test]
    fn walkers_respect_loaded_terrain_walls_and_village_bounds_without_teleporting() {
        let (terrain, mut chunks, journal, residents, mut a) = fixture();
        for y in 3..7 {
            for z in 0..16 {
                chunks
                    .loaded
                    .get_mut(&SectionPos::default())
                    .unwrap()
                    .data
                    .set_block([5, y, z], 3)
                    .unwrap();
            }
        }
        a.position = [3.0, 3.0, 8.5];
        a.needs.goal = Some(NeedGoal {
            kind: NeedKind::Food,
            position: [10, 3, 8],
        });
        let h = Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        };
        let mut nav = navigation::Navigation::default();
        for _ in 0..150 {
            residents.step(&mut a, &h, 0.5, 0.1, true, &mut nav);
            let yaw = f64::from(a.yaw);
            let half_x = (a.size()[0] * yaw.cos().abs() + a.size()[2] * yaw.sin().abs()) * 0.5;
            assert!(
                a.position[0] + half_x <= 5.000001,
                "rotated body crossed wall: {:?}, yaw={yaw}",
                a.position
            );
            assert!(h.clear_rotated(a.position, a.size(), false, yaw));
        }
        assert_eq!(a.position[1], 3.0);
        assert!(a.valid());
        assert!(h.clear_rotated(a.position, a.size(), false, f64::from(a.yaw)));
    }

    #[test]
    fn daytime_schedule_really_moves_and_removed_beds_cannot_be_slept_in() {
        let (terrain, mut chunks, mut journal, residents, mut a) = fixture();
        a.position = [3.5, 3.0, 4.5];
        a.needs.goal = Some(NeedGoal {
            kind: NeedKind::Food,
            position: [10, 3, 4],
        });
        let mut nav = navigation::Navigation::default();
        let start = a.position;
        for _ in 0..20 {
            residents.step(
                &mut a,
                &Habitat {
                    terrain: &terrain,
                    chunks: &chunks,
                    journal: &journal,
                },
                0.5,
                0.1,
                true,
                &mut nav,
            );
        }
        assert!(a.position[0] > start[0] + 2.0);
        chunks
            .loaded
            .get_mut(&SectionPos::default())
            .unwrap()
            .data
            .set_block([8, 3, 8], 0)
            .unwrap();
        journal.record([8, 3, 8], 0).unwrap();
        a.position = [7.5, 3.0, 8.5];
        residents.step(
            &mut a,
            &Habitat {
                terrain: &terrain,
                chunks: &chunks,
                journal: &journal,
            },
            0.93,
            0.1,
            true,
            &mut nav,
        );
        assert_ne!(a.behavior, Behavior::Sleep);
    }

    #[test]
    fn rotated_house_returns_use_the_door_before_sleeping_and_leave_again_in_daylight() {
        use crate::edits::BlockObjectState;
        for facing in 0..4 {
            let (terrain, mut chunks, _, mut residents, mut a) = fixture();
            let mut journal = EditJournal::default();
            let rotate = |[x, z]: [i64; 2]| match facing {
                0 => [x, z],
                1 => [z, -x],
                2 => [-x, -z],
                _ => [-z, x],
            };
            let point = |x, y, z| {
                let [x, z] = rotate([x, z]);
                [8 + x, y, 8 + z]
            };
            let data = &mut chunks.loaded.get_mut(&SectionPos::default()).unwrap().data;
            for x in 0..16 {
                for y in 3..8 {
                    for z in 0..16 {
                        data.set_block([x, y, z], 0).unwrap();
                    }
                }
            }
            for y in 3..7 {
                for offset in -4..=4 {
                    for cell in [
                        point(-4, y, offset),
                        point(4, y, offset),
                        point(offset, y, -4),
                        point(offset, y, 4),
                    ] {
                        data.set_block(cell.map(|p| p as usize), 3).unwrap();
                    }
                }
            }
            let bed = point(-2, 3, 2);
            let head = point(-2, 3, 3);
            let door = point(0, 3, -4);
            let upper = [door[0], 4, door[2]];
            for (cell, peer, material, secondary) in [
                (bed, head, 71, false),
                (head, bed, 71, true),
                (door, upper, 65, false),
                (upper, door, 65, true),
            ] {
                data.set_block(cell.map(|p| p as usize), material).unwrap();
                journal.record(cell, material).unwrap();
                journal
                    .set_object_state(
                        cell,
                        Some(BlockObjectState {
                            facing,
                            peer: Some(peer),
                            secondary,
                            ..Default::default()
                        }),
                    )
                    .unwrap();
            }
            let site = residents.sites.get_mut(&a.id).unwrap();
            site.bed = bed;
            site.bed_facing = facing;
            site.door = door;
            site.home_bounds = [[3, 1, 3], [13, 12, 13]];
            let start = point(0, 3, 6);
            a.position = [start[0] as f64 + 0.5, 3.0, start[2] as f64 + 0.5];
            let mut nav = navigation::Navigation::default();
            let mut opened = false;
            for _ in 0..600 {
                let requested = residents.step(
                    &mut a,
                    &Habitat {
                        terrain: &terrain,
                        chunks: &chunks,
                        journal: &journal,
                    },
                    0.93,
                    0.1,
                    true,
                    &mut nav,
                );
                if requested == Some(door) {
                    opened = true;
                    for cell in [door, upper] {
                        let mut state = journal.object_state(cell).unwrap();
                        state.facing = facing + 4;
                        journal.set_object_state(cell, Some(state)).unwrap();
                    }
                    let h = Habitat {
                        terrain: &terrain,
                        chunks: &chunks,
                        journal: &journal,
                    };
                    let (outside, inside) = door_points(residents.sites.get(&a.id).unwrap(), &h);
                    assert!(
                        h.walk_step(
                            outside,
                            inside,
                            a.kind,
                            f64::from(facing) * std::f64::consts::FRAC_PI_2
                        )
                        .is_some(),
                        "an open door header is above the feet, not a walking support"
                    );
                }
                if a.behavior == Behavior::Sleep {
                    break;
                }
            }
            assert!(
                opened,
                "facing={facing}: must approach its front door, position={:?}",
                a.position
            );
            assert_eq!(
                a.behavior,
                Behavior::Sleep,
                "facing={facing}: must reach its actual bed, position={:?}, goal={:?}, navigation={:?}",
                a.position,
                a.needs.goal,
                nav.inspection(&a)
            );
            for _ in 0..600 {
                residents.step(
                    &mut a,
                    &Habitat {
                        terrain: &terrain,
                        chunks: &chunks,
                        journal: &journal,
                    },
                    0.5,
                    0.1,
                    true,
                    &mut nav,
                );
                if !inside_home(residents.sites.get(&a.id).unwrap(), a.position) {
                    break;
                }
            }
            assert!(
                !inside_home(residents.sites.get(&a.id).unwrap(), a.position),
                "facing={facing}: must walk outside in daylight"
            );
            assert!(a.valid());
        }
    }

    #[test]
    fn replaced_rotated_or_incomplete_beds_are_not_slept_in_with_stale_site_orientation() {
        use crate::edits::BlockObjectState;
        let (terrain, chunks, mut journal, residents, mut a) = fixture();
        let site = residents.sites.get(&a.id).unwrap();
        assert!(assigned_bed(
            site,
            &Habitat {
                terrain: &terrain,
                chunks: &chunks,
                journal: &journal
            }
        ));
        journal.record([8, 3, 9], 0).unwrap();
        journal.record([9, 3, 8], 71).unwrap();
        for (cell, peer, secondary) in [([8, 3, 8], [9, 3, 8], false), ([9, 3, 8], [8, 3, 8], true)]
        {
            journal
                .set_object_state(
                    cell,
                    Some(BlockObjectState {
                        facing: 1,
                        peer: Some(peer),
                        secondary,
                        ..Default::default()
                    }),
                )
                .unwrap();
        }
        let mut nav = navigation::Navigation::default();
        let h = Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        };
        assert!(!assigned_bed(site, &h));
        residents.step(&mut a, &h, 0.93, 0.1, true, &mut nav);
        assert_ne!(a.behavior, Behavior::Sleep);
        journal.set_object_state([9, 3, 8], None).unwrap();
        assert!(!assigned_bed(
            site,
            &Habitat {
                terrain: &terrain,
                chunks: &chunks,
                journal: &journal
            }
        ));
    }
}
