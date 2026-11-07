//! Session-owned mounts; the persistent animal remains the only world identity.
use super::*;
use terra_core::{
    PlayerUuid,
    creatures::{Creature, CreatureKind},
};

pub(super) struct Mount {
    pub horse: Creature,
    pub stamina: f32,
    pub grounded: bool,
    pub leave: bool,
}

#[derive(Resource, Default)]
pub(super) struct Mounts {
    pub active: HashMap<PlayerUuid, Mount>,
    resting: HashMap<u64, (f32, Instant)>,
}

impl Mounts {
    pub fn contains(&self, id: u64) -> bool {
        self.active.values().any(|mount| mount.horse.id == id)
    }
    pub fn snapshot(&self, uuid: PlayerUuid) -> Option<MountedState> {
        self.active
            .get(&uuid)
            .map(|m| MountedState::new(&m.horse, m.stamina, m.grounded))
            .filter(|state| state.valid())
    }
    pub fn mount(
        &mut self,
        horse: &Creature,
        uuid: PlayerUuid,
        state: &terra_core::GameplayState,
    ) -> bool {
        if horse.kind != CreatureKind::Horse
            || horse.health <= 0.0
            || horse.growth() < 1.0
            || !state.kingdoms.can(
                uuid,
                terra_core::kingdom::KingdomPermission::NpcWork,
                horse.position.map(|v| v.floor() as i64),
            )
            || state.creatures.owners.get(&horse.id) != Some(&uuid)
            || self.active.contains_key(&uuid)
            || self.contains(horse.id)
        {
            return false;
        }
        let capacity = f32::from(horse.horse_traits().unwrap().stamina);
        let stamina = self
            .resting
            .remove(&horse.id)
            .map_or(capacity, |(stamina, at)| {
                (stamina + at.elapsed().as_secs_f32() * 4.0).min(capacity)
            });
        self.active.insert(
            uuid,
            Mount {
                horse: horse.clone(),
                stamina,
                grounded: true,
                leave: false,
            },
        );
        true
    }
    pub fn release(&mut self, uuid: PlayerUuid) {
        if let Some(mount) = self.active.remove(&uuid) {
            self.resting
                .insert(mount.horse.id, (mount.stamina, Instant::now()));
        }
    }
}

pub(super) fn publish(
    mut mounts: ResMut<Mounts>,
    mut journal: ResMut<EditJournal>,
    peers: Query<&Peer, With<Connected>>,
    config: Res<GameConfig>,
    chunks: Res<ChunkStore>,
) {
    // Removed animals and disconnected riders cannot keep runtime handles alive.
    let state = journal.gameplay();
    mounts.resting.retain(|id, _| {
        state.creatures.owners.contains_key(id)
            && state
                .creatures
                .animals
                .get(id)
                .is_some_and(|a| a.health > 0.0)
    });
    let stale: Vec<_> = mounts
        .active
        .iter()
        .filter_map(|(uuid, mount)| {
            (!peers.iter().any(|p| {
                p.uuid == *uuid
                    && p.synced
                    && !p.vitals.dead()
                    && p.chat.teleport.is_none()
                    && p.game_mode(&config) != terra_core::GameMode::Spectator
            }) || !state
                .creatures
                .animals
                .get(&mount.horse.id)
                .is_some_and(|a| a.health > 0.0)
                || state.creatures.owners.get(&mount.horse.id) != Some(uuid)
                || !terra_world::kingdom_access::can_use(
                    &state.kingdoms,
                    &chunks,
                    *uuid,
                    terra_core::kingdom::KingdomPermission::NpcWork,
                    mount.horse.position.map(|v| v.floor() as i64),
                ))
            .then_some(*uuid)
        })
        .collect();
    for uuid in stale {
        mounts.release(uuid);
    }
    for mount in mounts.active.values_mut() {
        if let Some(animal) = journal
            .gameplay_mut()
            .creatures
            .animals
            .get_mut(&mount.horse.id)
        {
            mount.horse.health = mount.horse.health.min(animal.health);
            *animal = mount.horse.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_owned_adult_living_horses_mount_once_and_stamina_is_not_reset() {
        use terra_core::creatures::{CreatureBehavior, CreatureNeeds};
        let mut state = terra_core::GameplayState::default();
        let owner = terra_core::PlayerProfile::default().uuid;
        let stranger = terra_core::PlayerProfile::default().uuid;
        let mut horse = Creature {
            id: (1 << 62) + 301,
            home: [0, 0],
            kind: CreatureKind::Horse,
            position: [0.0, 1.0, 0.0],
            velocity: [0.0; 3],
            yaw: 0.0,
            health: 20.0,
            behavior: CreatureBehavior::Rest,
            timer: 0.0,
            age: 0.0,
            variant: 0,
            needs: CreatureNeeds::full(CreatureKind::Horse),
        };
        let mut mounts = Mounts::default();
        assert!(!mounts.mount(&horse, owner, &state));
        state.creatures.owners.insert(horse.id, owner);
        assert!(!mounts.mount(&horse, stranger, &state));
        assert!(mounts.mount(&horse, owner, &state));
        assert!(!mounts.mount(&horse, owner, &state));
        mounts.active.get_mut(&owner).unwrap().stamina = 1.0;
        mounts.release(owner);
        assert!(mounts.mount(&horse, owner, &state));
        assert!(
            mounts.snapshot(owner).unwrap().stamina
                < f32::from(horse.horse_traits().unwrap().stamina)
        );
        mounts.release(owner);
        horse.id = (1 << 60) + 1;
        state.creatures.owners.insert(horse.id, owner);
        assert!(!mounts.mount(&horse, owner, &state));
        horse.id = (1 << 62) + 301;
        horse.health = 0.0;
        assert!(!mounts.mount(&horse, owner, &state));
    }
}
