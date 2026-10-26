//! Settlement schedules use the same loaded-voxel steering and bounded A* as animals.
use super::{Behavior, Creature, Habitat, Kind, distance, navigation, support};
use crate::generation::structures::ResidentSite;
use std::collections::BTreeMap;
use terra_core::creatures::{CreatureNeeds, CreaturePopulation, NeedGoal, NeedKind, villagers};

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
            let Some(position) = candidates.into_iter().find_map(|[x, z]| {
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
        a: &mut Creature,
        h: &Habitat,
        phase: f64,
        dt: f32,
        search: bool,
        nav: &mut navigation::Navigation,
    ) -> Option<[i64; 3]> {
        if a.health <= 0.0
            || !dt.is_finite()
            || !(0.001..=0.25).contains(&dt)
            || h.block(a.position.map(|v| v.floor() as i64)).is_none()
        {
            return None;
        }
        a.age += f64::from(dt);
        a.needs.search_in = (a.needs.search_in - dt).max(0.0);
        a.timer = (a.timer - dt).max(0.0);
        a.needs.food = (a.needs.food - dt * 0.003).max(0.0);
        a.needs.water = (a.needs.water - dt * 0.004).max(0.0);
        let Some(site) = self.sites.get(&a.id) else {
            a.velocity[0] = 0.0;
            a.velocity[2] = 0.0;
            a.behavior = Behavior::Rest;
            support::settle(a, h, dt);
            return None;
        };
        let sleep = villagers::sleeping(a.id, phase);
        let bed_exists = assigned_bed(site, h);
        if a.behavior == Behavior::Sleep && (!sleep || !bed_exists) {
            // Leave the bed only onto a verified adjacent floor, not by snapping through walls.
            if let Some(p) = exit_bed(site, h, a.position) {
                a.position = p;
                a.behavior = Behavior::Rest;
                a.needs.goal = None;
                a.timer = 1.0;
                nav.forget(a.id);
            } else {
                return None;
            }
        }
        if sleep && bed_exists && a.behavior == Behavior::Sleep {
            a.needs.food = (a.needs.food + dt * 0.025).min(a.kind.food());
            a.needs.water = (a.needs.water + dt * 0.025).min(a.kind.water());
            a.health = (a.health + dt * 0.01).min(a.kind.health());
            a.velocity = [0.0; 3];
            return None;
        }
        let mut door = None;
        if distance(a.position, site.door.map(|v| v as f64 + 0.5)) < 2.5_f64.powi(2)
            && h.block(site.door).is_some_and(crate::objects::is_door)
            && !crate::objects::is_open(
                h.block(site.door).unwrap_or(0),
                h.journal.object_state(site.door),
            )
        {
            door = Some(site.door);
        }
        let hungry = a.needs.food < a.kind.food() * 0.45 || a.needs.water < a.kind.water() * 0.45;
        let going_home = (sleep || hungry) && bed_exists;
        let bedside = going_home.then(|| exit_bed(site, h, a.position)).flatten();
        if going_home {
            let point = site.bed.map(|v| v as f64 + 0.5);
            if inside_home(site, a.position)
                && bedside.is_some_and(|entry| {
                    distance(a.position, entry) < 0.65_f64.powi(2)
                        && h.walk_step(a.position, entry, a.kind, f64::from(a.yaw))
                            .is_some()
                })
            {
                a.velocity = [0.0; 3];
                a.needs.goal = None;
                nav.forget(a.id);
                if sleep {
                    let d = [
                        [0.0, 0.0, 1.0],
                        [1.0, 0.0, 0.0],
                        [0.0, 0.0, -1.0],
                        [-1.0, 0.0, 0.0],
                    ][usize::from(site.bed_facing % 4)];
                    a.position = [
                        point[0] - d[0] * 0.4,
                        site.bed[1] as f64 + 0.79,
                        point[2] - d[2] * 0.4,
                    ];
                    a.yaw = f32::from(site.bed_facing % 4) * std::f32::consts::FRAC_PI_2;
                    a.behavior = Behavior::Sleep;
                } else {
                    a.behavior = Behavior::Graze;
                    a.needs.food = (a.needs.food + dt * 2.5).min(a.kind.food());
                    a.needs.water = (a.needs.water + dt * 2.5).min(a.kind.water());
                }
                return door;
            }
            let (outside, inside) = door_points(site, h);
            let inside_cell = inside.map(|v| v.floor() as i64);
            // Keep the gate phase stable while a local detour crosses the
            // threshold. Re-selecting the outer portal here causes ping-pong.
            let entering = a.needs.goal.is_some_and(|g| g.position == inside_cell)
                || in_doorway(site, h, a.position);
            let position = if inside_home(site, a.position) {
                site.bed
            } else if entering {
                inside_cell
            } else {
                outside.map(|v| v.floor() as i64)
            };
            if a.needs.goal.is_some_and(|old| old.position != position) {
                nav.forget(a.id);
            }
            a.needs.goal = Some(NeedGoal {
                kind: NeedKind::Food,
                position,
            });
        } else if a.timer > 0.0 {
            a.velocity[0] = 0.0;
            a.velocity[2] = 0.0;
            a.behavior = Behavior::Rest;
            support::settle(a, h, dt);
            return door;
        } else if a.needs.goal.is_none() && search {
            // Short, reproducible walks keep residents in their settlement and respect slopes.
            if inside_home(site, a.position) {
                let (_, inside) = door_points(site, h);
                a.needs.goal = Some(NeedGoal {
                    kind: NeedKind::Food,
                    position: inside.map(|v| v.floor() as i64),
                });
            } else {
                let hash = super::random(a.id, (a.age / 7.0) as u64);
                for n in 0..6 {
                    let angle = ((hash.rotate_left(n * 7) & 65535) as f64 / 65535.0)
                        * std::f64::consts::TAU;
                    let radius = 3.0 + ((hash >> (n * 5)) & 7) as f64;
                    let p = [
                        a.position[0] + angle.sin() * radius,
                        a.position[1],
                        a.position[2] + angle.cos() * radius,
                    ];
                    if p[0] < site.village_bounds[0][0] as f64 + 2.0
                        || p[0] > site.village_bounds[1][0] as f64 - 2.0
                        || p[2] < site.village_bounds[0][2] as f64 + 2.0
                        || p[2] > site.village_bounds[1][2] as f64 - 2.0
                        || inside_home(site, p)
                    {
                        continue;
                    }
                    if let Some(p) = h.ground(p, Kind::Villager) {
                        a.needs.goal = Some(NeedGoal {
                            kind: NeedKind::Food,
                            position: p.map(|v| v.floor() as i64),
                        });
                        break;
                    }
                }
            }
            a.needs.search_in = 2.0;
        }
        if let Some(mut goal) = a.needs.goal {
            let (outside, inside) = door_points(site, h);
            let target = if going_home {
                if inside_home(site, a.position) {
                    bedside.unwrap_or(inside)
                } else if goal.position == inside.map(|v| v.floor() as i64) {
                    inside
                } else {
                    outside
                }
            } else if inside_home(site, a.position) {
                if goal.position == outside.map(|v| v.floor() as i64)
                    || (a.position[0] - inside[0]).hypot(a.position[2] - inside[2]) < 0.05
                {
                    let position = outside.map(|v| v.floor() as i64);
                    if goal.position != position {
                        nav.forget(a.id);
                    }
                    goal.position = position;
                    a.needs.goal = Some(goal);
                    outside
                } else {
                    inside
                }
            } else {
                goal.position.map(|v| v as f64 + 0.5)
            };
            let escape = nav.escape(a);
            let target = escape.unwrap_or(target);
            if escape.is_none()
                && !going_home
                && !inside_home(site, a.position)
                && (target[0] - a.position[0]).hypot(target[2] - a.position[2]) < 0.5
            {
                a.needs.goal = None;
                a.timer = 1.5 + (a.id % 35) as f32 * 0.1;
                a.behavior = Behavior::Rest;
                a.velocity[0] = 0.0;
                a.velocity[2] = 0.0;
            } else {
                walk(a, h, target, dt, search, escape.is_some(), nav);
                if nav.stalled(a.id, a.age) {
                    // An unreachable destination must not keep winning the
                    // same A* retry. Take a verified local exit before exploring.
                    let escape = [1.0, 0.65, 1.4].into_iter().find_map(|radius| {
                        (0..8).find_map(|n| {
                            let angle = f64::from(n) * std::f64::consts::FRAC_PI_4;
                            let p = [
                                a.position[0] + angle.sin() * radius,
                                a.position[1],
                                a.position[2] + angle.cos() * radius,
                            ];
                            // Back out without rotating the body through furniture
                            // or a narrow doorframe. Turn again once there is room.
                            let yaw = f64::from(a.yaw);
                            let first = [
                                a.position[0] + angle.sin() * a.kind.speed() * f64::from(dt),
                                a.position[1],
                                a.position[2] + angle.cos() * a.kind.speed() * f64::from(dt),
                            ];
                            physical_step(h, a.position, first, yaw)?;
                            physical_step(h, a.position, p, yaw).filter(|end| {
                                (end[0] - a.position[0]).hypot(end[2] - a.position[2]) > 0.5
                            })
                        })
                    });
                    if !going_home {
                        a.needs.goal = None;
                    }
                    a.needs.search_in = 0.3;
                    a.timer = 0.5 + (a.id % 7) as f32 * 0.1;
                    nav.forget(a.id);
                    if let Some(p) = escape {
                        nav.escape_via(a.id, p, a.age);
                        // Keep a route owner while the exact sub-voxel exit is
                        // active; household doorway goals must not override it.
                        if a.needs.goal.is_none() {
                            a.needs.goal = Some(NeedGoal {
                                kind: NeedKind::Food,
                                position: p.map(|v| v.floor() as i64),
                            });
                        }
                    }
                }
            }
        } else {
            a.behavior = Behavior::Rest;
            a.velocity[0] = 0.0;
            a.velocity[2] = 0.0;
        }
        support::settle(a, h, dt);
        door
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
            .filter(|p| p[1] <= site.bed[1] as f64 + 0.2)
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

fn walk(
    a: &mut Creature,
    h: &Habitat,
    target: [f64; 3],
    dt: f32,
    search: bool,
    strafe: bool,
    nav: &mut navigation::Navigation,
) {
    nav.intend(
        a,
        terra_core::creatures::intentions::IntentReason::Explore,
        Some(target),
    );
    let before = a.position;
    let movement = |point: [f64; 3]| {
        let dx = point[0] - before[0];
        let dz = point[2] - before[2];
        let length = dx.hypot(dz).max(0.001);
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
        .is_some();
    let direct = (!detouring || precise_finish)
        .then(|| movement(target))
        .flatten();
    let next = direct
        .or_else(|| navigation::ground_waypoint(a, h, target, search, Some(nav)).and_then(movement))
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
        }
        assert!(
            a.position[0] < 4.73,
            "does not cross a wall: {:?}",
            a.position
        );
        assert_eq!(a.position[1], 3.0);
        assert!(a.valid());
        assert!(h.clear(a.position, a.size(), false));
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
