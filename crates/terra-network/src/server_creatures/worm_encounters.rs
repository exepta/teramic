//! Movement wakes buried worms; exposed anatomy gives nearby guards real targets.
use std::collections::BTreeMap;
use terra_core::{
    PlayerUuid,
    creatures::{
        Creature, CreatureBehavior as B, CreatureKind as K, CreaturePopulation,
        soldiers::SoldierLoadout,
    },
};
use terra_world::creatures::{Habitat, distance, navigation::Navigation, sandworm::Footfall};

#[derive(Default)]
pub(super) struct Encounters {
    players: BTreeMap<PlayerUuid, Observation>,
    animals: BTreeMap<u64, Observation>,
    guards: BTreeMap<u64, Guard>,
}
struct Observation {
    position: [f64; 3],
    last_seen: f32,
    moving_until: f32,
}
struct Guard {
    target: u64,
    origin: [f64; 3],
    cooldown: f32,
}
pub(super) struct Frame {
    pub disturbances: Vec<Footfall>,
    prey: Vec<Creature>,
    worms: Vec<Creature>,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Target {
    Player(usize),
    Creature(u64),
}

/// An eruption throws a victim upward using its actual replicated motor state.
/// A charge adds a smaller lift and forward shove; ordinary bites have no launch.
pub(super) fn strike_impulse(phase: B, yaw: f32) -> bevy::prelude::Vec3 {
    use bevy::prelude::Vec3;
    match phase {
        B::Emerge => Vec3::Y * terra_core::creatures::sandworm::KNOCKUP_SPEED,
        B::Hunt => Vec3::new(-yaw.sin() * 7.0, 4.5, -yaw.cos() * 7.0),
        _ => Vec3::ZERO,
    }
}

fn peaceful(a: &Creature) -> bool {
    a.health > 0.0
        && !a.kind.hostile_to_players()
        && !a.kind.is_flying()
        && !a.kind.is_aquatic()
        && a.kind != K::SandWorm
}
fn observe<K: Ord + Copy>(
    memory: &mut BTreeMap<K, Observation>,
    key: K,
    position: [f64; 3],
    now: f32,
) -> Option<[f64; 2]> {
    let entry = memory.entry(key).or_insert(Observation {
        position,
        last_seen: now,
        moving_until: -1.0,
    });
    let moved = distance(position, entry.position);
    let elapsed = f64::from(now - entry.last_seen);
    // Teleports and stale observations must not become predictive footsteps.
    let mut velocity = [0.; 2];
    if moved > 6.0_f64.powi(2) || elapsed > 0.65 {
        entry.moving_until = -1.0;
    } else if moved >= 0.035_f64.powi(2) && elapsed > 0.0 {
        entry.moving_until = now + 0.65;
        velocity = [
            (position[0] - entry.position[0]) / elapsed,
            (position[2] - entry.position[2]) / elapsed,
        ];
    }
    entry.position = position;
    entry.last_seen = now;
    (now < entry.moving_until).then_some(velocity)
}
impl Encounters {
    pub(super) fn frame(
        &mut self,
        now: f32,
        players: &[(PlayerUuid, [f64; 3], bool)],
        population: &CreaturePopulation,
    ) -> Frame {
        self.players
            .retain(|id, _| players.iter().any(|(other, _, _)| other == id));
        self.animals
            .retain(|id, _| population.animals.contains_key(id));
        self.guards
            .retain(|id, _| population.animals.get(id).is_some_and(|a| a.health > 0.0));
        let mut disturbances = Vec::new();
        for &(id, position, prey) in players {
            if let Some(velocity) = observe(&mut self.players, id, position, now) {
                disturbances.push(Footfall {
                    position,
                    velocity,
                    prey,
                });
            }
        }
        let prey: Vec<_> = population
            .animals
            .values()
            .filter(|a| {
                peaceful(a)
                    && players
                        .iter()
                        .any(|(_, p, _)| distance(*p, a.position) <= 174.0_f64.powi(2))
            })
            .cloned()
            .collect();
        for a in &prey {
            if let Some(velocity) = observe(&mut self.animals, a.id, a.position, now) {
                disturbances.push(Footfall {
                    position: a.position,
                    velocity,
                    prey: true,
                });
            }
        }
        let worms = population
            .animals
            .values()
            .filter(|a| {
                a.kind == K::SandWorm
                    && a.health > 0.0
                    && players
                        .iter()
                        .any(|(_, p, _)| distance(*p, a.position) <= 174.0_f64.powi(2))
            })
            .cloned()
            .collect();
        Frame {
            disturbances,
            prey,
            worms,
        }
    }
    /// Returns None to let the resident's normal work/sleep schedule proceed.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn guard_step(
        &mut self,
        npc: &mut Creature,
        frame: &Frame,
        h: &Habitat,
        owner: Option<PlayerUuid>,
        dt: f32,
        search: bool,
        navigation: &mut Navigation,
    ) -> Option<Option<(u64, f32)>> {
        let loadout = SoldierLoadout::of(npc)?;
        let eye = [npc.position[0], npc.position[1] + 1.4, npc.position[2]];
        let target = frame
            .worms
            .iter()
            .filter_map(|worm| {
                if distance(npc.position, worm.position) > 28.0_f64.powi(2) {
                    return None;
                }
                let point = nearest_exposed(worm, eye)?;
                terra_world::creatures::bears::visible(h, eye, point).then_some((worm, point))
            })
            .min_by(|(_, a), (_, b)| distance(*a, eye).total_cmp(&distance(*b, eye)));
        let Some((worm, point)) = target else {
            self.guards.remove(&npc.id);
            return None;
        };
        let state = self.guards.entry(npc.id).or_insert(Guard {
            target: worm.id,
            origin: npc.position,
            cooldown: 0.9,
        });
        if state.target != worm.id {
            state.target = worm.id;
            state.cooldown = 0.9;
        }
        let origin = state.origin;
        let realm = h
            .journal
            .gameplay()
            .kingdoms
            .region_at(origin.map(|v| v.floor() as i64))
            .map(|r| h.journal.gameplay().kingdoms.realm_id(r.id));
        let allowed = |p: [f64; 3]| {
            distance(p, origin) <= 28.0_f64.powi(2)
                && if let Some(actor) = owner {
                    terra_world::kingdom_access::can_use(
                        &h.journal.gameplay().kingdoms,
                        h.chunks,
                        actor,
                        terra_core::kingdom::KingdomPermission::NpcWork,
                        p.map(|v| v.floor() as i64),
                    )
                } else {
                    h.journal
                        .gameplay()
                        .kingdoms
                        .region_at(p.map(|v| v.floor() as i64))
                        .is_none_or(|r| Some(h.journal.gameplay().kingdoms.realm_id(r.id)) == realm)
                }
        };
        let (reach, damage) = match loadout {
            SoldierLoadout::Sword => (2.2, 4.0),
            SoldierLoadout::Spear => (3.0, 5.0),
            SoldierLoadout::Bow => (14.0, 3.0),
        };
        npc.yaw = ((npc.position[0] - point[0]).atan2(npc.position[2] - point[2])) as f32;
        if distance(eye, point) <= reach * reach {
            terra_world::creatures::work_rest_with_access(npc, h, dt, allowed);
            npc.velocity[0] = 0.0;
            npc.velocity[2] = 0.0;
            let before = state.cooldown;
            state.cooldown = (state.cooldown - dt).max(0.0);
            npc.behavior = B::Attack;
            npc.timer = state.cooldown;
            let strike = before > 0.45 && state.cooldown <= 0.45;
            if state.cooldown <= 0.0 {
                state.cooldown = 0.9;
            }
            Some(strike.then_some((worm.id, damage)))
        } else {
            let target = [point[0], worm.position[1], point[2]];
            let cached = matches!(
                navigation.ground_state(npc.id, npc.age),
                terra_world::creatures::navigation::GroundRouteState::Following
                    | terra_world::creatures::navigation::GroundRouteState::Arrived
            );
            if search || cached {
                if !search {
                    npc.needs.search_in = npc.needs.search_in.max(dt + 0.01);
                }
                terra_world::creatures::work_step_with_access(
                    npc, h, target, dt, navigation, allowed,
                );
            } else {
                npc.needs.search_in = (npc.needs.search_in - dt).max(0.0);
                terra_world::creatures::work_rest_with_access(npc, h, dt, allowed);
            }
            npc.behavior = B::Hunt;
            state.cooldown = 0.9;
            Some(None)
        }
    }
}
impl Frame {
    pub(super) fn targets(
        &self,
        worm: &Creature,
        players: &[[f64; 3]],
    ) -> (Vec<[f64; 3]>, Vec<Target>) {
        let mut candidates: Vec<_> = players
            .iter()
            .enumerate()
            .map(|(i, p)| ([p[0], p[1] + 0.6, p[2]], Target::Player(i)))
            .chain(
                self.prey
                    .iter()
                    .filter(|a| distance(a.position, worm.position) <= 40.0_f64.powi(2))
                    .map(|a| {
                        (
                            [
                                a.position[0],
                                a.position[1] + a.size()[1] * 0.5,
                                a.position[2],
                            ],
                            Target::Creature(a.id),
                        )
                    }),
            )
            .collect();
        candidates.sort_unstable_by(|(a, _), (b, _)| {
            distance(*a, worm.position).total_cmp(&distance(*b, worm.position))
        });
        candidates.truncate(128);
        candidates.into_iter().unzip()
    }
}
fn nearest_exposed(worm: &Creature, eye: [f64; 3]) -> Option<[f64; 3]> {
    terra_core::creatures::hitboxes::for_creature(worm)
        .iter()
        .map(|b| {
            let delta = std::array::from_fn::<_, 3, _>(|i| eye[i] - b.center[i]);
            let local = std::array::from_fn::<_, 3, _>(|axis| {
                delta
                    .iter()
                    .zip(b.axes[axis])
                    .map(|(a, b)| a * b)
                    .sum::<f64>()
                    .clamp(-b.size[axis] * 0.5, b.size[axis] * 0.5)
            });
            std::array::from_fn(|i| {
                b.center[i]
                    + (0..3)
                        .map(|axis| local[axis] * b.axes[axis][i])
                        .sum::<f64>()
            })
        })
        .min_by(|a, b| distance(*a, eye).total_cmp(&distance(*b, eye)))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn footfalls_expire_and_stationary_or_teleported_observers_do_not_wake_worms() {
        let mut memory = BTreeMap::new();
        assert!(observe(&mut memory, 1, [0.0, 1.0, 0.0], 0.0).is_none());
        assert!(observe(&mut memory, 1, [0.0, 1.0, 0.0], 1.0).is_none());
        let velocity = observe(&mut memory, 1, [0.15, 1.0, 0.0], 1.1).unwrap();
        assert!((velocity[0] - 1.5).abs() < 0.001 && velocity[1] == 0.0);
        assert_eq!(
            observe(&mut memory, 1, [0.15, 1.0, 0.0], 1.5),
            Some([0.; 2])
        );
        assert!(observe(&mut memory, 1, [0.15, 1.0, 0.0], 2.0).is_none());
        assert!(observe(&mut memory, 1, [100.0, 1.0, 0.0], 3.0).is_none());
    }
}

#[cfg(test)]
mod combat_tests {
    use super::*;
    use terra_core::creatures::CreatureNeeds;
    fn creature(id: u64, kind: K, p: [f64; 3]) -> Creature {
        Creature {
            id,
            kind,
            home: [0, 0],
            position: p,
            velocity: [0.0; 3],
            yaw: 0.0,
            health: kind.health(),
            behavior: B::Rest,
            timer: 0.0,
            age: 0.0,
            variant: 0,
            needs: CreatureNeeds::full(kind),
        }
    }
    #[test]
    fn peaceful_prey_and_player_indices_survive_nearest_target_sorting() {
        let worm = creature(1, K::SandWorm, [8.0, 1.0, 8.0]);
        let cow = creature(2, K::Cow, [9.0, 1.0, 8.0]);
        let mut population = CreaturePopulation::default();
        population.animals.insert(1, worm.clone());
        population.animals.insert(2, cow);
        population
            .animals
            .insert(3, creature(3, K::Skeleton, [8.5, 1.0, 8.0]));
        let frame = Encounters::default().frame(
            0.0,
            &[(PlayerUuid::from_u128(1), [11.0, 1.9, 8.0], true)],
            &population,
        );
        let (_, ids) = frame.targets(&worm, &[[11.0, 1.9, 8.0]]);
        assert_eq!(ids, vec![Target::Creature(2), Target::Player(0)]);
        assert!(
            frame.disturbances.is_empty(),
            "first observation is not motion"
        );
    }
    #[test]
    fn soldier_strikes_exposed_worm_after_windup_and_never_hits_buried_anatomy() {
        let terrain = terra_world::TerraGenerator::new(terra_core::WorldConfig::default()).unwrap();
        let mut chunks = terra_world::streaming::ChunkStore::default();
        let position = terra_voxel::SectionPos::default();
        let mut data = terra_voxel::BlockSection::empty(position).unwrap();
        for x in 0..16 {
            for z in 0..16 {
                data.set_block([x, 0, z], 3).unwrap();
            }
        }
        chunks.loaded.insert(
            position,
            terra_world::streaming::LoadedSection {
                data,
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            },
        );
        let journal = terra_world::edits::EditJournal::default();
        let h = Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        };
        let mut worm = creature(1, K::SandWorm, [8.5, 1.0, 8.5]);
        worm.behavior = B::Burrowed;
        let mut soldier = creature(2, K::Villager, [7.0, 1.0, 8.5]);
        soldier.variant = SoldierLoadout::Sword.role();
        let mut frame = Frame {
            disturbances: vec![],
            prey: vec![],
            worms: vec![worm],
        };
        let mut encounters = Encounters::default();
        let mut navigation = Navigation::default();
        assert!(
            encounters
                .guard_step(&mut soldier, &frame, &h, None, 0.1, true, &mut navigation)
                .is_none()
        );
        frame.worms[0].behavior = B::Attack;
        frame.worms[0].timer = terra_core::creatures::sandworm::EXPOSED_SECONDS;
        // The upright neck is behind the mouth. Put the windup fixture beside
        // its actual lower rings instead of the former squat body's empty root.
        let lower_neck = terra_core::creatures::sandworm::pose(
            &frame.worms[0],
            terra_core::creatures::sandworm::NECK_ROOT_SEGMENT + 3,
        );
        soldier.position[2] = frame.worms[0].position[2] + f64::from(lower_neck.center[2]);
        let eye = [
            soldier.position[0],
            soldier.position[1] + 1.4,
            soldier.position[2],
        ];
        let contact = nearest_exposed(&frame.worms[0], eye).unwrap();
        assert!(
            distance(eye, contact) <= 2.2_f64.powi(2),
            "the windup fixture starts in real melee reach"
        );
        let mut hits = 0;
        for tick in 0..7 {
            let result =
                encounters.guard_step(&mut soldier, &frame, &h, None, 0.1, true, &mut navigation);
            if let Some(Some((target, damage))) = result {
                assert!(tick >= 4);
                assert_eq!((target, damage), (1, 4.0));
                hits += 1;
            }
        }
        assert_eq!(hits, 1);

        // Start beyond the actual exposed anatomy: the vertical shoulder now
        // emerges at the hole, so the former 1.5 m offset is already in reach.
        let mut pursuing = creature(3, K::Villager, [3.0, 1.0, 8.5]);
        pursuing.variant = SoldierLoadout::Sword.role();
        let initial = pursuing.position;
        let initial_eye = [initial[0], initial[1] + 1.4, initial[2]];
        assert!(
            distance(
                initial_eye,
                nearest_exposed(&frame.worms[0], initial_eye).unwrap()
            ) > 2.2_f64.powi(2),
            "the pursuit fixture must start outside real sword reach"
        );
        let mut attack_started = None;
        let mut struck = false;
        for tick in 0..50 {
            let result =
                encounters.guard_step(&mut pursuing, &frame, &h, None, 0.1, true, &mut navigation);
            if pursuing.behavior == B::Attack {
                attack_started.get_or_insert(tick);
            }
            if let Some(Some((target, damage))) = result {
                assert!(
                    tick - attack_started.unwrap() >= 4,
                    "pursuit must not skip the impact windup"
                );
                assert_eq!((target, damage), (1, 4.0));
                struck = true;
                break;
            }
        }
        assert!(
            distance(initial, pursuing.position) > 0.1,
            "the guard must physically approach the exposed anatomy"
        );
        assert!(
            struck,
            "the distant guard must reach the exposed worm and strike"
        );
    }
}
