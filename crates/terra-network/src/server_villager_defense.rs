//! Short-lived retaliation against the actual player who landed an authorized hit.
//! Orders, carried supplies, professions and construction progress remain untouched.
use super::*;
use std::collections::BTreeMap;
use terra_core::{
    PlayerUuid,
    creatures::{Creature, CreatureBehavior, CreatureKind},
    kingdom::{KingdomId, KingdomPermission, KingdomState},
};
use terra_world::creatures::{Habitat, distance};

const MAX_DEFENDERS: usize = 128;
const PURSUIT_RADIUS: f64 = 24.0;
const REACH: f64 = 2.2;

struct Defense {
    attacker: PlayerUuid,
    origin: [f64; 3],
    realm: Option<KingdomId>,
    actor: Option<PlayerUuid>,
    remaining: f32,
    cooldown: f32,
    blocked: f32,
}

#[derive(Resource, Default)]
pub(super) struct Defenses {
    entries: BTreeMap<u64, Defense>,
    navigation: terra_world::creatures::navigation::Navigation,
}

#[derive(Clone, Copy)]
struct Player {
    position: [f64; 3],
    vulnerable: bool,
    protected: bool,
}

fn realm_of(game: &terra_core::GameplayState, npc: &Creature) -> Option<KingdomId> {
    game.creatures
        .owners
        .get(&npc.id)
        .and_then(|owner| game.kingdoms.active_for_member(*owner))
        .map(|realm| realm.id)
        .or_else(|| {
            game.kingdoms
                .region_at(npc.position.map(|v| v.floor() as i64))
                .map(|realm| game.kingdoms.realm_id(realm.id))
        })
}

fn sovereign(
    game: &terra_core::GameplayState,
    realm: Option<KingdomId>,
    player: PlayerUuid,
) -> bool {
    realm
        .and_then(|id| game.kingdoms.get(id))
        .is_some_and(|realm| realm.state == KingdomState::Active && realm.founder == player)
}

impl Defenses {
    pub(super) fn contains(&self, npc: u64) -> bool {
        self.entries.contains_key(&npc)
    }
    pub(super) fn suspended(&self) -> impl Iterator<Item = u64> + '_ {
        self.entries.keys().copied()
    }

    /// Called only after the server's normal range, cooldown, LOS and damage checks.
    pub(super) fn provoke(
        &mut self,
        npc: u64,
        attacker: PlayerUuid,
        game: &terra_core::GameplayState,
    ) {
        let Some(creature) = game
            .creatures
            .animals
            .get(&npc)
            .filter(|npc| npc.kind == CreatureKind::Villager && npc.health > 0.0)
        else {
            self.entries.remove(&npc);
            self.navigation.invalidate(npc);
            self.navigation.begin_tick(npc);
            return;
        };
        let realm = realm_of(game, creature);
        if attacker.is_nil() || sovereign(game, realm, attacker) {
            return;
        }
        if let Some(defense) = self.entries.get_mut(&npc)
            && defense.attacker == attacker
        {
            defense.remaining = 20.0;
            return;
        }
        if self.entries.len() >= MAX_DEFENDERS && !self.entries.contains_key(&npc) {
            return;
        }
        self.navigation.invalidate(npc);
        self.entries.insert(
            npc,
            Defense {
                attacker,
                origin: creature.position,
                realm,
                actor: game.creatures.owners.get(&npc).copied(),
                remaining: 20.0,
                cooldown: 0.45,
                blocked: 0.0,
            },
        );
    }

    fn advance(
        &mut self,
        chunks: &ChunkStore,
        journal: &mut EditJournal,
        terrain: &terra_world::TerraGenerator,
        players: &BTreeMap<PlayerUuid, Player>,
        dt: f32,
    ) -> Vec<(PlayerUuid, f32)> {
        let mut strikes = Vec::new();
        let active = std::mem::take(&mut self.entries);
        for (id, mut defense) in active {
            let Some(mut npc) = journal.gameplay().creatures.animals.get(&id).cloned() else {
                self.navigation.invalidate(id);
                self.navigation.begin_tick(id);
                continue;
            };
            let target = players.get(&defense.attacker).copied();
            defense.remaining -= dt;
            defense.cooldown = (defense.cooldown - dt).max(0.0);
            let target = target.filter(|target| {
                target.vulnerable
                    && npc.health > 0.0
                    && npc.kind == CreatureKind::Villager
                    && defense.remaining > 0.0
                    && defense.blocked < 5.0
                    && distance(target.position, defense.origin) <= PURSUIT_RADIUS.powi(2)
                    && distance(npc.position, defense.origin) <= PURSUIT_RADIUS.powi(2)
                    && !sovereign(journal.gameplay(), defense.realm, defense.attacker)
            });
            let Some(target) = target else {
                self.navigation.invalidate(id);
                self.navigation.begin_tick(id);
                if npc.health > 0.0 {
                    npc.behavior = CreatureBehavior::Rest;
                    npc.velocity[0] = 0.0;
                    npc.velocity[2] = 0.0;
                    journal.gameplay_mut().creatures.animals.insert(id, npc);
                }
                continue;
            };
            let h = Habitat {
                chunks,
                journal,
                terrain,
            };
            let allowed =
                |point: [f64; 3]| {
                    distance(point, defense.origin) <= PURSUIT_RADIUS.powi(2)
                        && [-0.3, 0.3].into_iter().all(|dx| {
                            [-0.3, 0.3].into_iter().all(|dz| {
                                let cell = [
                                    (point[0] + dx).floor() as i64,
                                    point[1].floor() as i64,
                                    (point[2] + dz).floor() as i64,
                                ];
                                if let Some(actor) = defense.actor {
                                    terra_world::kingdom_access::can_use(
                                        &h.journal.gameplay().kingdoms,
                                        h.chunks,
                                        actor,
                                        KingdomPermission::NpcWork,
                                        cell,
                                    )
                                } else {
                                    h.journal.gameplay().kingdoms.region_at(cell).is_none_or(
                                        |record| {
                                            Some(h.journal.gameplay().kingdoms.realm_id(record.id))
                                                == defense.realm
                                        },
                                    )
                                }
                            })
                        })
                };
            let can_hit = |npc: &Creature| {
                let center = [
                    npc.position[0],
                    npc.position[1] + npc.size()[1] * 0.5,
                    npc.position[2],
                ];
                distance(center, target.position) <= REACH.powi(2)
                    && terra_world::creatures::bears::visible(
                        &h,
                        [npc.position[0], npc.position[1] + 1.4, npc.position[2]],
                        target.position,
                    )
            };
            if can_hit(&npc) {
                npc.velocity[0] = 0.0;
                npc.velocity[2] = 0.0;
                terra_world::creatures::work_rest_with_access(&mut npc, &h, dt, allowed);
                if defense.cooldown == 0.0 && !target.protected && can_hit(&npc) {
                    let damage =
                        if terra_core::creatures::soldiers::SoldierLoadout::of(&npc).is_some() {
                            4.0
                        } else {
                            2.0
                        };
                    strikes.push((defense.attacker, damage));
                    defense.cooldown = 0.9;
                }
                defense.blocked = 0.0;
                npc.behavior = CreatureBehavior::Attack;
            } else {
                let before = npc.position;
                let feet = [
                    target.position[0],
                    target.position[1] - f64::from(terra_physics::character::PLAYER_HALF_HEIGHT),
                    target.position[2],
                ];
                let moved = terra_world::creatures::work_step_with_access(
                    &mut npc,
                    &h,
                    feet,
                    dt,
                    &mut self.navigation,
                    allowed,
                );
                defense.blocked = if moved && distance(before, npc.position) > 0.00001 {
                    0.0
                } else {
                    defense.blocked + dt
                };
                npc.behavior = CreatureBehavior::Hunt;
            }
            journal.gameplay_mut().creatures.animals.insert(id, npc);
            self.entries.insert(id, defense);
        }
        strikes
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn tick(
    time: Res<Time<Real>>,
    config: Res<GameConfig>,
    chunks: Res<ChunkStore>,
    terrain: Res<terra_world::streaming::WorldTerrain>,
    mut journal: ResMut<EditJournal>,
    mut defenses: ResMut<Defenses>,
    bodies: Query<&Transform, With<ServerCharacter>>,
    mut peers: Query<&mut Peer, With<Connected>>,
    mut next: Local<f64>,
) {
    let now = time.elapsed_secs_f64();
    if now < *next {
        return;
    }
    let dt = (now - *next + 0.1).clamp(0.001, 0.25) as f32;
    *next = now + 0.1;
    let players = peers
        .iter()
        .filter(|peer| {
            peer.accepted && peer.synced && !peer.vitals.dead() && peer.chat.teleport.is_none()
        })
        .filter_map(|peer| {
            Some((
                peer.uuid,
                Player {
                    position: bodies
                        .get(peer.body)
                        .ok()?
                        .translation
                        .to_array()
                        .map(f64::from),
                    vulnerable: peer.game_mode(&config) == terra_core::GameMode::Survival,
                    protected: peer.survival.protection > 0.0,
                },
            ))
        })
        .take(128)
        .collect();
    for (uuid, damage) in defenses.advance(&chunks, &mut journal, &terrain.0, &players, dt) {
        if let Some(mut peer) = peers.iter_mut().find(|peer| peer.uuid == uuid) {
            let mode = peer.game_mode(&config);
            survival::animal_attack(&mut peer, damage, mode);
        }
    }
}

#[cfg(test)]
#[path = "server_villager_defense_tests.rs"]
mod tests;
