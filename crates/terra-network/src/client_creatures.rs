use super::*;
use std::collections::{BTreeMap, VecDeque};
use terra_core::creatures::Creature;
#[path = "client_creatures/playback.rs"]
mod playback;
pub use playback::CreaturePlayback;

#[derive(Clone)]
pub struct CreatureTrack {
    pub playback: CreaturePlayback,
    pub previous: Creature,
    pub current: Creature,
    pub received: Instant,
    pub interval: f32,
}
impl CreatureTrack {
    pub fn position(&self) -> Vec3 {
        if let Some(position) = self.playback.position_at(Instant::now()) {
            return position;
        }
        Vec3::from_array(self.previous.position.map(|v| v as f32)).lerp(
            Vec3::from_array(self.current.position.map(|v| v as f32)),
            (self.received.elapsed().as_secs_f32() / self.interval).clamp(0.0, 1.0),
        )
    }
}
#[derive(Resource, Default)]
pub struct ReplicatedCreatures {
    pub creatures: BTreeMap<u64, CreatureTrack>,
    pub intentions: BTreeMap<u64, terra_core::creatures::intentions::CreatureIntent>,
    pub bonds: BTreeMap<u64, terra_core::creatures::taming::CompanionView>,
    pub families: BTreeMap<u64, terra_core::creatures::breeding::FamilyView>,
    pub ecology: terra_core::creatures::ecology::Ecology,
    tick: u64,
}
#[derive(Resource, Default)]
pub struct CreatureActions(pub VecDeque<(u64, bool)>);

#[derive(Resource, Default)]
pub struct PetCommands(pub VecDeque<(u64, terra_core::creatures::taming::CompanionMode)>);

#[derive(Resource, Default)]
pub struct BeeNestActions(pub VecDeque<(terra_voxel::SectionPos, terra_world::props::PropId)>);

#[derive(Resource, Default)]
pub struct CreatureInspection(pub Option<u64>);

pub(super) fn install(app: &mut App) {
    app.init_resource::<ReplicatedCreatures>()
        .init_resource::<CreatureActions>()
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
    for (entity, mut receiver) in &mut receivers {
        let mut pending: Option<(CreatureStates, std::collections::HashSet<u64>)> = None;
        for packet in receiver.receive() {
            // Snapshot packets can beat the reliable world-ready message. Always
            // drain them; otherwise Lightyear discards an unhandled queue in Last.
            if !active || session.connection != Some(entity) {
                continue;
            }
            if packet.tick <= creatures.tick
                || pending
                    .as_ref()
                    .is_some_and(|(previous, _)| packet.tick <= previous.tick)
                || packet.creatures.len() > terra_core::creatures::MAX_VISIBLE_CREATURES
                || packet.creatures.iter().any(|a| !a.valid())
                || !packet.ecology.valid()
                || packet.stalker_grip.is_some_and(|grip| {
                    !grip.valid()
                        || !packet.creatures.iter().any(|a| {
                            a.id == grip.creature
                                && a.kind == terra_core::creatures::CreatureKind::Stalker
                                && a.health > 0.0
                        })
                })
                || packet.intentions.len() > 1
                || packet.intentions.iter().any(|intent| {
                    !intent.valid() || !packet.creatures.iter().any(|a| a.id == intent.id)
                })
                || packet.bonds.len() > packet.creatures.len()
                || packet.families.len() > packet.creatures.len()
                || packet.families.iter().any(|family| {
                    !family.valid()
                        || !packet
                            .creatures
                            .iter()
                            .any(|a| a.id == family.id && a.growth() == family.growth)
                })
                || packet.creatures.iter().any(|a| {
                    terra_core::creatures::breeding::is_birth_id(a.id)
                        && (!packet
                            .families
                            .iter()
                            .any(|f| f.id == a.id && f.parents.is_some())
                            || !packet
                                .bonds
                                .iter()
                                .any(|b| b.id == a.id && b.owner.is_some()))
                })
                || packet
                    .bonds
                    .iter()
                    .any(|bond| !bond.valid() || !packet.creatures.iter().any(|a| a.id == bond.id))
            {
                continue;
            }
            let ids: std::collections::HashSet<_> = packet.creatures.iter().map(|a| a.id).collect();
            if ids.len() != packet.creatures.len() {
                continue;
            }
            let bond_ids: std::collections::HashSet<_> =
                packet.bonds.iter().map(|bond| bond.id).collect();
            if bond_ids.len() != packet.bonds.len() {
                continue;
            }
            let family_ids: std::collections::HashSet<_> =
                packet.families.iter().map(|family| family.id).collect();
            if family_ids.len() != packet.families.len() {
                continue;
            }
            pending = Some((packet, ids));
        }
        // Full snapshots supersede one another. Applying a whole delayed burst
        // in one render frame used to restart interpolation several times.
        let Some((packet, ids)) = pending else {
            continue;
        };
        creatures.tick = packet.tick;
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
            let mut previous = track.current.clone();
            previous.position = track.position().to_array().map(f64::from);
            track.interval = now
                .saturating_duration_since(track.received)
                .as_secs_f32()
                .clamp(0.05, 0.4);
            track.playback.push(
                Vec3::from_array(current.position.map(|v| v as f32)),
                Vec3::from_array(current.velocity),
                stamp,
                now,
            );
            track.previous = previous;
            track.current = current;
            track.received = now;
        }
    }
}
#[allow(clippy::too_many_arguments)]
fn send(
    mut queue: ResMut<CreatureActions>,
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
    pets.0.clear();
    nests.0.clear();
}
