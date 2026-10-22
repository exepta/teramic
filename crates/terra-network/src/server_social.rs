use super::*;
use crate::protocol::social::SharedCursor;

#[derive(Default)]
pub(super) struct Schedule {
    next: f64,
    sent: HashMap<PlayerId, (Vec<(PlayerId, String)>, bool)>,
}

pub(super) fn broadcast(
    time: Res<Time>,
    profiles: Res<ConnectedProfiles>,
    journal: Res<EditJournal>,
    mut peers: Query<(&Peer, &mut MessageSender<ServerControl>), With<Connected>>,
    mut schedule: Local<Schedule>,
) {
    let now = time.elapsed_secs_f64();
    if now < schedule.next {
        return;
    }
    schedule.next = now + 0.1;
    schedule.sent.retain(|id, _| profiles.0.contains_key(id));
    let mut names: Vec<_> = profiles
        .0
        .iter()
        .take(128)
        .map(|(&id, p)| (id, p.name.clone()))
        .collect();
    names.sort_by_key(|(id, _)| id.0);
    let cursors: Vec<_> = peers
        .iter()
        .filter_map(|(peer, _)| {
            let (cursor, at) = peer.chest_cursor?;
            (peer.synced
                && !peer.vitals.dead()
                && at.elapsed().as_secs_f32() < 0.8
                && peer
                    .container
                    .as_ref()
                    .is_some_and(|s| s.anchor == cursor.anchor))
            .then(|| SharedCursor {
                player: peer.id,
                name: profiles
                    .0
                    .get(&peer.id)
                    .map_or_else(String::new, |p| p.name.clone()),
                cursor,
                item: journal
                    .gameplay()
                    .inventories
                    .get(&peer.uuid)
                    .and_then(|i| i.cursor),
            })
        })
        .collect();
    for (peer, mut sender) in &mut peers {
        if !peer.synced {
            continue;
        }
        let visible: Vec<_> = cursors
            .iter()
            .filter(|c| {
                c.player != peer.id
                    && peer
                        .container
                        .as_ref()
                        .is_some_and(|s| s.anchor == c.cursor.anchor)
            })
            .take(32)
            .cloned()
            .collect();
        let previous = schedule.sent.entry(peer.id).or_default();
        let roster = (previous.0 != names).then(|| names.clone());
        if roster.is_none() && visible.is_empty() && !previous.1 {
            continue;
        }
        if let Some(names) = roster.as_ref() {
            previous.0.clone_from(names);
        }
        previous.1 = !visible.is_empty();
        sender.send::<ControlChannel>(ServerControl::Social {
            names: roster,
            cursors: visible,
        });
    }
}
