use super::*;
use std::collections::{BTreeMap, VecDeque};
use terra_core::creatures::Creature;
#[path = "client_creatures/playback.rs"]
mod playback;
pub use playback::CreaturePlayback;

#[cfg(test)]
#[path = "client_creatures/tests.rs"]
mod tests;

#[derive(Clone)]
pub struct CreatureTrack {
    pub playback: CreaturePlayback,
    pub previous: Creature,
    pub current: Creature,
    pub received: Instant,
    pub interval: f32,
}
impl CreatureTrack {
    fn apply(&mut self, current: Creature, stamp: f64, now: Instant) {
        // The server broadcasts full snapshots at 10 Hz, including distant
        // actors whose AI runs at 5 Hz. Stamping the unchanged pose as a new
        // sample inserts a stop between every two real strides.
        if self.playback.has_samples()
            && current.age == self.current.age
            && current.position == self.current.position
            && current.velocity == self.current.velocity
            && current.yaw == self.current.yaw
            && current.behavior == self.current.behavior
            && current.timer == self.current.timer
        {
            self.current = current;
            return;
        }
        // The tail is already fully underground before authority re-anchors
        // the worm at the second hole. Interpolating that hidden move would
        // drag its next upright breach sideways through fourteen metres.
        let reanchor = current.kind == terra_core::creatures::CreatureKind::SandWorm
            && matches!(
                self.current.behavior,
                terra_core::creatures::CreatureBehavior::Leap
                    | terra_core::creatures::CreatureBehavior::Hunt
            )
            && current.behavior != self.current.behavior;
        let mut previous = self.current.clone();
        if reanchor {
            self.playback = Default::default();
            previous.position = current.position;
        } else {
            previous.position = self.position_at(now).to_array().map(f64::from);
        }
        self.interval = now
            .saturating_duration_since(self.received)
            .as_secs_f32()
            .clamp(0.05, 0.4);
        self.playback.push(
            Vec3::from_array(current.position.map(|v| v as f32)),
            Vec3::from_array(current.velocity),
            stamp,
            now,
            current.kind.is_dragon()
                && matches!(
                    current.behavior,
                    terra_core::creatures::CreatureBehavior::Hunt
                        | terra_core::creatures::CreatureBehavior::Wander
                ),
        );
        self.previous = previous;
        self.current = current;
        self.received = now;
    }
    pub fn position(&self) -> Vec3 {
        self.position_at(Instant::now())
    }

    fn position_at(&self, now: Instant) -> Vec3 {
        if let Some(position) = self.playback.position_at(now) {
            return position;
        }
        Vec3::from_array(self.previous.position.map(|v| v as f32)).lerp(
            Vec3::from_array(self.current.position.map(|v| v as f32)),
            (now.saturating_duration_since(self.received).as_secs_f32() / self.interval)
                .clamp(0.0, 1.0),
        )
    }
}
#[derive(Resource, Default)]
pub struct ReplicatedCreatures {
    pub boss: Option<u64>,
    pub creatures: BTreeMap<u64, CreatureTrack>,
    pub intentions: BTreeMap<u64, terra_core::creatures::intentions::CreatureIntent>,
    pub bonds: BTreeMap<u64, terra_core::creatures::taming::CompanionView>,
    pub families: BTreeMap<u64, terra_core::creatures::breeding::FamilyView>,
    pub ecology: terra_core::creatures::ecology::Ecology,
    pub rejected_snapshots: u64,
    pub last_snapshot_error: Option<&'static str>,
    pub last_rejected_tick: u64,
    tick: u64,
    warning_at: Option<Instant>,
}
impl ReplicatedCreatures {
    fn reject_snapshot(&mut self, tick: u64, reason: &'static str, now: Instant) {
        self.rejected_snapshots = self.rejected_snapshots.saturating_add(1);
        self.last_snapshot_error = Some(reason);
        self.last_rejected_tick = tick;
        if self
            .warning_at
            .is_none_or(|at| now.saturating_duration_since(at) >= Duration::from_secs(5))
        {
            tracing::warn!(
                tick,
                last_accepted_tick = self.tick,
                rejected = self.rejected_snapshots,
                reason,
                "Creature snapshot rejected; retaining the last valid poses"
            );
            self.warning_at = Some(now);
        }
    }
}

fn snapshot_ids(packet: &CreatureStates) -> Result<std::collections::HashSet<u64>, &'static str> {
    if packet.creatures.len() > terra_core::creatures::MAX_VISIBLE_CREATURES {
        return Err("creature count exceeded limit");
    }
    if packet.creatures.iter().any(|animal| !animal.valid()) {
        return Err("invalid creature state");
    }
    if packet.boss.is_some_and(|id| {
        !packet
            .creatures
            .iter()
            .any(|a| a.id == id && a.kind.is_dragon() && a.health > 0.0)
    }) {
        return Err("invalid boss encounter");
    }
    if !packet.ecology.valid() {
        return Err("invalid ecology state");
    }
    if packet.stalker_grip.is_some_and(|grip| {
        !grip.valid()
            || !packet.creatures.iter().any(|animal| {
                animal.id == grip.creature
                    && animal.kind == terra_core::creatures::CreatureKind::Stalker
                    && animal.health > 0.0
            })
    }) {
        return Err("invalid stalker grip");
    }
    if packet.intentions.len() > 1
        || packet.intentions.iter().any(|intent| {
            !intent.valid() || !packet.creatures.iter().any(|animal| animal.id == intent.id)
        })
    {
        return Err("invalid inspected creature intent");
    }
    if packet.bonds.len() > packet.creatures.len()
        || packet.bonds.iter().any(|bond| {
            !bond.valid() || !packet.creatures.iter().any(|animal| animal.id == bond.id)
        })
    {
        return Err("invalid creature bond");
    }
    if packet.families.len() > packet.creatures.len()
        || packet.families.iter().any(|family| {
            !family.valid()
                || !packet
                    .creatures
                    .iter()
                    .any(|animal| animal.id == family.id && animal.growth() == family.growth)
        })
    {
        return Err("invalid creature family");
    }
    if packet.creatures.iter().any(|animal| {
        terra_core::creatures::breeding::is_birth_id(animal.id)
            && (!packet.families.iter().any(|family| {
                family.id == animal.id && (family.parents.is_some() || animal.kind.is_dragon())
            }) || !(packet
                .bonds
                .iter()
                .any(|bond| bond.id == animal.id && bond.owner.is_some())
                || animal.kind == terra_core::creatures::CreatureKind::Bee
                    && packet.ecology.bees.contains_key(&animal.id)))
    }) {
        return Err("offspring is missing its family, owner or wild bee ecology");
    }
    let ids: std::collections::HashSet<_> =
        packet.creatures.iter().map(|animal| animal.id).collect();
    if ids.len() != packet.creatures.len() {
        return Err("duplicate creature identity");
    }
    let bond_ids: std::collections::HashSet<_> = packet.bonds.iter().map(|bond| bond.id).collect();
    if bond_ids.len() != packet.bonds.len() {
        return Err("duplicate creature bond");
    }
    let family_ids: std::collections::HashSet<_> =
        packet.families.iter().map(|family| family.id).collect();
    if family_ids.len() != packet.families.len() {
        return Err("duplicate creature family");
    }
    Ok(ids)
}
#[derive(Resource, Default)]
pub struct CreatureActions(pub VecDeque<(u64, bool)>);

#[derive(Resource, Default)]
pub struct PlayerAttacks(pub VecDeque<terra_core::PlayerId>);

#[derive(Resource, Default)]
pub struct PetCommands(pub VecDeque<(u64, terra_core::creatures::taming::CompanionMode)>);

#[derive(Resource, Default)]
pub struct BeeNestActions(pub VecDeque<(terra_voxel::SectionPos, terra_world::props::PropId)>);

#[derive(Resource, Default)]
pub struct CreatureInspection(pub Option<u64>);

pub(super) fn install(app: &mut App) {
    app.init_resource::<ReplicatedCreatures>()
        .init_resource::<CreatureActions>()
        .init_resource::<PlayerAttacks>()
        .init_resource::<PetCommands>()
        .init_resource::<BeeNestActions>()
        .init_resource::<CreatureInspection>()
        .add_systems(Update, receive.after(super::receive_controls))
        .add_systems(PostUpdate, send.before(MessageSystems::Send));
}
fn receive(
    config: Res<terra_core::GameConfig>,
    mut session: ResMut<ClientSession>,
    mut creatures: ResMut<ReplicatedCreatures>,
    mut receivers: Query<(Entity, &mut MessageReceiver<CreatureStates>), With<Client>>,
) {
    let active = session.ready && session.error.is_none();
    if !active {
        *creatures = Default::default();
        session.stalker_grip = None;
        session.stalker_grip_at = None;
    }
    // If the last living player dies, the server suspends creature snapshots.
    // Vitals still arrive, and an older creature packet must not restart that fight.
    let can_fight =
        active && !session.dead() && session.game_mode == terra_core::GameMode::Survival;
    if !can_fight && creatures.boss.is_some() {
        creatures.boss = None;
    }
    for (entity, mut receiver) in &mut receivers {
        let mut pending: Option<(CreatureStates, std::collections::HashSet<u64>)> = None;
        for mut packet in receiver.receive() {
            // Snapshot packets can beat the reliable world-ready message. Always
            // drain them; otherwise Lightyear discards an unhandled queue in Last.
            if !active || session.connection != Some(entity) {
                continue;
            }
            if packet.tick <= creatures.tick
                || pending
                    .as_ref()
                    .is_some_and(|(previous, _)| packet.tick <= previous.tick)
            {
                continue;
            }
            if packet.families.len() <= terra_core::creatures::MAX_VISIBLE_CREATURES
                && packet.creatures.len() <= terra_core::creatures::MAX_VISIBLE_CREATURES
            {
                // Shared region creatures retain their legacy binary layout; the network
                // family owns the age record for reared dragons.
                for family in packet
                    .families
                    .iter()
                    .filter(|family| family.dragon.is_some())
                {
                    if let Some(animal) = packet
                        .creatures
                        .iter_mut()
                        .find(|animal| animal.id == family.id)
                    {
                        animal.needs.dragon = family.dragon;
                    }
                }
            }
            let ids = match snapshot_ids(&packet) {
                Ok(ids) => ids,
                Err(reason) => {
                    creatures.reject_snapshot(packet.tick, reason, Instant::now());
                    continue;
                }
            };
            pending = Some((packet, ids));
        }
        // Full snapshots supersede one another. Applying a whole delayed burst
        // in one render frame used to restart interpolation several times.
        let Some((packet, ids)) = pending else {
            continue;
        };
        creatures.tick = packet.tick;
        creatures.boss = packet.boss.filter(|_| can_fight);
        session.stalker_grip = packet.stalker_grip;
        session.stalker_grip_at = Some(Instant::now());
        creatures.bonds = packet
            .bonds
            .into_iter()
            .map(|bond| (bond.id, bond))
            .collect();
        creatures.families = packet
            .families
            .into_iter()
            .map(|family| (family.id, family))
            .collect();
        creatures.ecology = packet.ecology;
        creatures.intentions = packet
            .intentions
            .into_iter()
            .map(|intent| (intent.id, intent))
            .collect();
        creatures.creatures.retain(|id, _| ids.contains(id));
        let stamp = packet.tick as f64 / f64::from(config.server.tick_rate.max(1));
        let now = Instant::now();
        for current in packet.creatures {
            let track = creatures
                .creatures
                .entry(current.id)
                .or_insert_with(|| CreatureTrack {
                    playback: Default::default(),
                    previous: current.clone(),
                    current: current.clone(),
                    received: now,
                    interval: 0.1,
                });
            track.apply(current, stamp, now);
        }
    }
}
#[allow(clippy::too_many_arguments)]
fn send(
    mut queue: ResMut<CreatureActions>,
    mut player_attacks: ResMut<PlayerAttacks>,
    mut pets: ResMut<PetCommands>,
    mut nests: ResMut<BeeNestActions>,
    mut session: ResMut<ClientSession>,
    mut senders: Query<&mut MessageSender<ClientControl>, With<Client>>,
    inspection: Res<CreatureInspection>,
    mut sent_inspection: Local<Option<Option<u64>>>,
    time: Res<Time>,
    mut inspection_due: Local<f64>,
) {
    if !session.ready || session.error.is_some() {
        *sent_inspection = None;
    }
    if let Some(mut sender) = senders.iter_mut().next()
        && session.ready
        && session.error.is_none()
    {
        if session.dismount_requested {
            sender.send::<ControlChannel>(ClientControl::Dismount);
            session.dismount_requested = false;
        }
        if *sent_inspection != Some(inspection.0) && time.elapsed_secs_f64() >= *inspection_due {
            sender.send::<ControlChannel>(ClientControl::InspectCreature(inspection.0));
            *sent_inspection = Some(inspection.0);
            *inspection_due = time.elapsed_secs_f64() + 0.1;
        }
        for player in player_attacks.0.drain(..).take(2) {
            sender.send::<ControlChannel>(ClientControl::AttackPlayer {
                player,
                look: session.look,
            });
        }
        for (id, feed) in queue.0.drain(..).take(2) {
            sender.send::<ControlChannel>(ClientControl::Creature {
                id,
                feed,
                look: session.look,
            });
        }
        for (id, mode) in pets.0.drain(..).take(2) {
            sender.send::<ControlChannel>(ClientControl::PetCommand { id, mode });
        }
        for (section, id) in nests.0.drain(..).take(2) {
            sender.send::<ControlChannel>(ClientControl::HarvestHoney {
                section,
                id,
                look: session.look,
            });
        }
    }
    queue.0.clear();
    player_attacks.0.clear();
    pets.0.clear();
    nests.0.clear();
}

#[cfg(test)]
mod spirit_wire_tests {
    use super::*;
    use terra_core::creatures::{
        Creature, CreatureBehavior, CreatureKind, CreatureNeeds, creature_id,
    };

    #[test]
    fn spirit_and_forest_variants_roundtrip_and_old_kind_discriminants_stay_unchanged() {
        for (ordinal, kind) in CreatureKind::ALL.into_iter().enumerate() {
            assert_eq!(postcard::to_allocvec(&kind).unwrap(), vec![ordinal as u8]);
        }
        assert_eq!(CreatureKind::Villager as u8, 20);
        assert_eq!(CreatureKind::Butterfly as u8, 21);
        assert_eq!(CreatureKind::SpiritGloom as u8, 22);
        assert_eq!(CreatureKind::SpiritFox as u8, 23);
        assert_eq!(CreatureKind::ThornBrute as u8, 29);
        assert_eq!(CreatureKind::SandWorm as u8, 30);
        let mut creatures = Vec::new();
        for kind in [
            CreatureKind::Butterfly,
            CreatureKind::SpiritGloom,
            CreatureKind::SpiritFox,
            CreatureKind::Skeleton,
            CreatureKind::Raven,
            CreatureKind::BloodSlime,
            CreatureKind::BloodStalker,
            CreatureKind::BloodWisp,
            CreatureKind::ThornBrute,
            CreatureKind::SandWorm,
        ] {
            for variant in 0..kind.variant_count() {
                let home = [creatures.len() as i64, 0];
                let animal = Creature {
                    id: creature_id(home, 0).unwrap(),
                    kind,
                    home,
                    position: [0.5, 4.0, 0.5],
                    velocity: [0.0; 3],
                    yaw: 0.0,
                    health: kind.health(),
                    behavior: if kind == CreatureKind::SandWorm {
                        CreatureBehavior::Tremor
                    } else {
                        CreatureBehavior::Fly
                    },
                    timer: 1.0,
                    age: 0.0,
                    variant,
                    needs: CreatureNeeds::full(kind),
                };
                assert!(animal.valid());
                let json = serde_json::to_string(&animal).unwrap();
                assert_eq!(serde_json::from_str::<Creature>(&json).unwrap(), animal);
                let mut invalid = animal.clone();
                invalid.variant = kind.variant_count();
                assert!(!invalid.valid());
                creatures.push(animal);
            }
        }
        let snapshot = CreatureStates {
            tick: 1,
            creatures,
            ecology: Default::default(),
            intentions: Vec::new(),
            bonds: Vec::new(),
            families: Vec::new(),
            boss: None,
            stalker_grip: None,
        };
        let restored: CreatureStates =
            postcard::from_bytes(&postcard::to_allocvec(&snapshot).unwrap()).unwrap();
        assert_eq!(restored.creatures, snapshot.creatures);
        let gloom_variants: Vec<_> = restored
            .creatures
            .iter()
            .filter(|animal| animal.kind == CreatureKind::SpiritGloom)
            .map(|animal| animal.variant)
            .collect();
        assert_eq!(gloom_variants, [0, 1, 2, 3, 4, 5, 6]);
        let expected_ids = snapshot.creatures.iter().map(|animal| animal.id).collect();
        assert_eq!(snapshot_ids(&restored).unwrap(), expected_ids);
        let worm = restored.creatures.last().unwrap();
        assert_eq!(worm.kind, CreatureKind::SandWorm);
        assert_eq!(worm.behavior, CreatureBehavior::Tremor);
    }
}
