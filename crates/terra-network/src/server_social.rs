use super::*;
use crate::protocol::social::SharedCursor;

#[derive(Default)]
pub(super) struct Schedule {
    next: f64,
    sent: HashMap<PlayerId, (Vec<(PlayerId, String)>, bool)>,
    chests: HashMap<PlayerId, Vec<[i64; 3]>>,
}

pub(super) fn broadcast(
    time: Res<Time>,
    profiles: Res<ConnectedProfiles>,
    journal: Res<EditJournal>,
    chunks: Res<ChunkStore>,
    mut peers: Query<(&Peer, &mut MessageSender<ServerControl>), With<Connected>>,
    mut schedule: Local<Schedule>,
) {
    let now = time.elapsed_secs_f64();
    if now < schedule.next {
        return;
    }
    schedule.next = now + 0.1;
    schedule.sent.retain(|id, _| profiles.0.contains_key(id));
    schedule.chests.retain(|id, _| profiles.0.contains_key(id));
    let open: std::collections::BTreeSet<_> = peers
        .iter()
        .filter_map(|(peer, _)| {
            let container = peer.container.as_ref()?;
            (peer.synced
                && !peer.vitals.dead()
                && container.furnace.is_none()
                && container.crusher.is_none()
                && journal
                    .block(container.anchor)
                    .or_else(|| chunks.block_at(container.anchor))
                    == Some(29))
            .then_some(container.anchor)
        })
        .collect();
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
        let visible_chests: Vec<_> = open
            .iter()
            .filter(|position| {
                chest_in_view(**position, &journal, |section| {
                    peer.known.contains_key(&section)
                }) || peer
                    .container
                    .as_ref()
                    .is_some_and(|container| container.anchor == **position)
            })
            .take(256)
            .copied()
            .collect();
        let previous_chests = schedule.chests.entry(peer.id).or_default();
        if *previous_chests != visible_chests {
            previous_chests.clone_from(&visible_chests);
            sender.send::<ControlChannel>(ServerControl::ChestViewers(visible_chests));
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

fn chest_in_view(
    position: [i64; 3],
    journal: &EditJournal,
    known: impl Fn(terra_voxel::SectionPos) -> bool,
) -> bool {
    known(terra_voxel::global_to_section(position).0)
        || journal
            .object_state(position)
            .and_then(|s| s.peer)
            .is_some_and(|peer| known(terra_voxel::global_to_section(peer).0))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn connected_chest_viewer_state_reaches_an_observer_that_only_knows_the_secondary_section() {
        let owner = [-1, 2, -17];
        let peer = [0, 2, -17];
        let mut journal = EditJournal::default();
        for position in [owner, peer] {
            journal.record(position, 29).unwrap();
        }
        journal
            .set_object_state(
                owner,
                Some(terra_world::edits::BlockObjectState {
                    peer: Some(peer),
                    ..default()
                }),
            )
            .unwrap();
        journal
            .set_object_state(
                peer,
                Some(terra_world::edits::BlockObjectState {
                    peer: Some(owner),
                    secondary: true,
                    ..default()
                }),
            )
            .unwrap();
        let first = terra_voxel::global_to_section(owner).0;
        let second = terra_voxel::global_to_section(peer).0;
        assert_ne!(first, second);
        assert!(chest_in_view(owner, &journal, |p| p == first));
        assert!(chest_in_view(owner, &journal, |p| p == second));
        assert!(!chest_in_view(owner, &journal, |_| false));
    }
}
