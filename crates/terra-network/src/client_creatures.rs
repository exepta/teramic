use super::*;
use std::collections::{BTreeMap, VecDeque};
use terra_core::creatures::Creature;

#[derive(Clone)]
pub struct CreatureTrack {
    pub previous: Creature,
    pub current: Creature,
    pub received: Instant,
    pub interval: f32,
}
impl CreatureTrack {
    pub fn position(&self) -> Vec3 {
        Vec3::from_array(self.previous.position.map(|v| v as f32)).lerp(
            Vec3::from_array(self.current.position.map(|v| v as f32)),
            (self.received.elapsed().as_secs_f32() / self.interval).clamp(0.0, 1.0),
        )
    }
}
#[derive(Resource, Default)]
pub struct ReplicatedCreatures {
    pub creatures: BTreeMap<u64, CreatureTrack>,
    pub ecology: terra_core::creatures::ecology::Ecology,
    tick: u64,
}
#[derive(Resource, Default)]
pub struct CreatureActions(pub VecDeque<(u64, bool)>);

pub(super) fn install(app: &mut App) {
    app.init_resource::<ReplicatedCreatures>()
        .init_resource::<CreatureActions>()
        .add_systems(Update, receive)
        .add_systems(PostUpdate, send.before(MessageSystems::Send));
}
fn receive(
    session: Res<ClientSession>,
    mut creatures: ResMut<ReplicatedCreatures>,
    mut receivers: Query<&mut MessageReceiver<CreatureStates>, With<Client>>,
) {
    if !session.ready || session.error.is_some() {
        *creatures = Default::default();
        return;
    }
    for mut receiver in &mut receivers {
        for packet in receiver.receive() {
            if packet.tick <= creatures.tick
                || packet.creatures.len() > terra_core::creatures::MAX_VISIBLE_CREATURES
                || packet.creatures.iter().any(|a| !a.valid())
                || !packet.ecology.valid()
            {
                continue;
            }
            let ids: std::collections::HashSet<_> = packet.creatures.iter().map(|a| a.id).collect();
            if ids.len() != packet.creatures.len() {
                continue;
            }
            creatures.tick = packet.tick;
            creatures.ecology = packet.ecology;
            creatures.creatures.retain(|id, _| ids.contains(id));
            for current in packet.creatures {
                let track = creatures.creatures.get(&current.id);
                if track.is_some_and(|t| t.current == current) {
                    continue;
                }
                let interval =
                    track.map_or(0.1, |t| t.received.elapsed().as_secs_f32().clamp(0.05, 0.2));
                let mut previous = track.map_or_else(|| current.clone(), |t| t.current.clone());
                if let Some(track) = track {
                    // A late snapshot starts at the displayed pose, never at an
                    // obsolete packet endpoint (which snapped moving animals backwards).
                    previous.position = track.position().to_array().map(f64::from);
                    let delta: f64 = previous
                        .position
                        .iter()
                        .zip(current.position)
                        .map(|(a, b)| (a - b).powi(2))
                        .sum();
                    if delta > 64.0 {
                        previous.position = current.position;
                    }
                }
                creatures.creatures.insert(
                    current.id,
                    CreatureTrack {
                        previous,
                        current,
                        received: Instant::now(),
                        interval,
                    },
                );
            }
        }
    }
}
fn send(
    mut queue: ResMut<CreatureActions>,
    session: Res<ClientSession>,
    mut senders: Query<&mut MessageSender<ClientControl>, With<Client>>,
) {
    if let Some(mut sender) = senders.iter_mut().next()
        && session.ready
        && session.error.is_none()
    {
        for (id, feed) in queue.0.drain(..).take(2) {
            sender.send::<ControlChannel>(ClientControl::Creature {
                id,
                feed,
                look: session.look,
            });
        }
    }
    queue.0.clear();
}
