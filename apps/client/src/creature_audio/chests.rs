//! Shared chest hinges/latches, driven by authoritative viewer transitions.
use bevy::prelude::*;
use std::collections::BTreeMap;
use terra_network::client::ClientSession;
use terra_world::{edits::EditJournal, streaming::ChunkStore};

const RANGE: f32 = 20.0;
const MAX_TRACKED: usize = 256;

#[derive(Resource, Default, Debug)]
pub(crate) struct ChestAudioStatus {
    pub opened: u64,
    pub closed: u64,
    pub tracked: usize,
}

#[derive(Default)]
pub(super) struct Transitions {
    known: BTreeMap<[i64; 3], bool>,
}

impl Transitions {
    pub(super) fn update(
        &mut self,
        chunks: &ChunkStore,
        journal: &EditJournal,
        session: Option<&ClientSession>,
        listener: Vec3,
    ) -> Vec<([i64; 3], bool)> {
        let Some(session) = session.filter(|session| session.ready) else {
            self.known.clear();
            return Vec::new();
        };
        let center = terra_voxel::global_to_section(listener.floor().as_i64vec3().to_array()).0;
        let mut candidates = BTreeMap::new();
        // Only the nearby loaded spatial index, never the world's historical journal.
        for x in -2..=2 {
            for y in -2..=2 {
                for z in -2..=2 {
                    let Ok(section) = center.offset([x, y, z]) else {
                        continue;
                    };
                    if !chunks.loaded.contains_key(&section) {
                        continue;
                    }
                    for (cell, material, object) in journal.object_states_in_section(section) {
                        if material != terra_voxel::MaterialId::Chest as u16 {
                            continue;
                        }
                        let anchor = if object.secondary {
                            object.peer.unwrap_or(cell)
                        } else {
                            cell
                        };
                        let state = journal.object_state(anchor).unwrap_or_default();
                        let position =
                            Vec3::from_array(anchor.map(|v| v as f32)) + Vec3::splat(0.5);
                        if !state.secondary
                            && journal.block(anchor) == Some(material)
                            && (chunks.block_at(anchor) == Some(material)
                                || state
                                    .peer
                                    .is_some_and(|peer| chunks.block_at(peer) == Some(material)))
                            && listener.distance_squared(position) < RANGE * RANGE
                        {
                            candidates.insert(anchor, listener.distance_squared(position));
                        }
                    }
                }
            }
        }
        let mut nearest: Vec<_> = candidates.into_iter().collect();
        nearest.sort_unstable_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)));
        self.observe(
            nearest
                .into_iter()
                .take(MAX_TRACKED)
                .map(|(anchor, _)| (anchor, session.open_chests.contains(&anchor)))
                .collect(),
        )
    }

    fn observe(&mut self, current: BTreeMap<[i64; 3], bool>) -> Vec<([i64; 3], bool)> {
        let changed = current
            .iter()
            .filter_map(|(&anchor, &open)| {
                self.known
                    .get(&anchor)
                    .filter(|&&old| old != open)
                    .map(|_| (anchor, open))
            })
            .collect();
        self.known = current;
        changed
    }

    pub(super) fn len(&self) -> usize {
        self.known.len()
    }
}

pub(super) fn foley(hinge: &[f32], wood: &[f32], opening: bool) -> Vec<f32> {
    let count = if opening { 11_040 } else { 12_000 };
    let mut low = 0.0;
    let mut bass = 0.0;
    (0..count)
        .map(|i| {
            let cursor = i * hinge.len() / count;
            let mut sample = hinge.get(cursor).copied().unwrap_or(0.0) * 0.85;
            if !opening && i >= 8_160 {
                sample += wood.get(i - 8_160).copied().unwrap_or(0.0) * 0.42;
            }
            low += (sample - low) * 0.60;
            bass += (low - bass) * 0.023;
            let fade = (i as f32 / 192.0).min(1.0) * ((count - 1 - i) as f32 / 960.0).min(1.0);
            ((low - bass) * fade).clamp(-0.60, 0.60)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authoritative_chest_edges_are_once_only_and_streaming_is_silent() {
        let a = [-1, 15, -17];
        let b = [1, 15, -17];
        let mut state = Transitions::default();
        assert!(state.observe([(a, false), (b, true)].into()).is_empty());
        assert_eq!(
            state.observe([(a, true), (b, true)].into()),
            vec![(a, true)]
        );
        assert!(state.observe([(a, true), (b, true)].into()).is_empty());
        assert_eq!(
            state.observe([(a, false), (b, true)].into()),
            vec![(a, false)]
        );
        assert!(state.observe(BTreeMap::new()).is_empty());
        assert_eq!(state.len(), 0);
        assert!(state.observe([(a, true)].into()).is_empty());
    }

    #[test]
    fn double_chest_has_one_sound_when_only_the_other_section_is_loaded() {
        use terra_world::edits::BlockObjectState;
        let owner = [-1, 2, -17];
        let peer = [0, 2, -17];
        let mut journal = EditJournal::default();
        for (cell, other, secondary) in [(owner, peer, false), (peer, owner, true)] {
            journal.record(cell, 29).unwrap();
            journal
                .set_object_state(
                    cell,
                    Some(BlockObjectState {
                        peer: Some(other),
                        secondary,
                        ..default()
                    }),
                )
                .unwrap();
        }
        let mut chunks = ChunkStore::default();
        let (section, local) = terra_voxel::global_to_section(peer);
        let mut data = terra_voxel::BlockSection::empty(section).unwrap();
        data.set_block(local, 29).unwrap();
        chunks.loaded.insert(
            section,
            terra_world::streaming::LoadedSection {
                data,
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            },
        );
        let mut session = ClientSession::default();
        session.ready = true;
        let mut state = Transitions::default();
        let listener = Vec3::new(0.5, 3.0, -18.0);
        assert!(
            state
                .update(&chunks, &journal, Some(&session), listener)
                .is_empty()
        );
        assert_eq!(state.len(), 1);
        session.open_chests.insert(owner);
        assert_eq!(
            state.update(&chunks, &journal, Some(&session), listener),
            vec![(owner, true)]
        );
        assert!(
            state
                .update(&chunks, &journal, Some(&session), listener)
                .is_empty()
        );
        session.open_chests.clear();
        assert_eq!(
            state.update(&chunks, &journal, Some(&session), listener),
            vec![(owner, false)]
        );
        chunks.loaded.clear();
        assert!(
            state
                .update(&chunks, &journal, Some(&session), listener)
                .is_empty()
        );
        assert_eq!(state.len(), 0);
    }
}
