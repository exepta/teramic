//! A continuous nearby faucet layer in the existing recorded device soundscape.
use super::scene::Soundscape;
use bevy::prelude::*;
use std::sync::atomic::Ordering;
use terra_core::plumbing::{self, FAUCET};
use terra_network::{client::ClientSession, protocol::session::PlumbingVisual};
use terra_world::{
    edits::EditJournal,
    streaming::{ChunkStore, WorldAccess},
};

const RANGE: f32 = 20.0;
const MAX_TAPS: usize = 8;

#[derive(Resource, Default, Debug)]
pub(crate) struct HydraulicAudioStatus {
    pub taps: usize,
    pub gains: [f32; 2],
    pub output_rms: f32,
}

fn mix_faucets(
    visuals: &[PlumbingVisual],
    chunks: &ChunkStore,
    journal: &EditJournal,
    listener: Vec3,
    right: Vec3,
) -> ([f32; 2], usize) {
    let mut nearby = Vec::with_capacity(MAX_TAPS);
    for visual in visuals.iter().take(256) {
        if chunks.block_at(visual.position) != Some(FAUCET)
            || !visual.open
            || visual.amount == 0
            || !(visual.flowing || visual.supplied)
        {
            continue;
        }
        let Some(state) = journal.object_state(visual.position) else {
            continue;
        };
        let level = plumbing::faucet_level(state.facing);
        if level == 0 {
            continue;
        }
        let point = Vec3::from_array(visual.position.map(|v| v as f32)) + Vec3::new(0.5, 0.25, 0.5);
        let distance = point.distance(listener);
        if distance >= RANGE {
            continue;
        }
        let index =
            nearby.partition_point(|(previous, _, _): &(f32, Vec3, u8)| *previous < distance);
        if index < MAX_TAPS {
            if nearby.len() == MAX_TAPS {
                nearby.pop();
            }
            nearby.insert(index, (distance, point, level));
        }
    }
    let count = nearby.len();
    let mut energy = [0.0_f32; 2];
    for (distance, position, level) in nearby {
        let delta = position - listener;
        let pan = delta.normalize_or_zero().dot(right).clamp(-0.85, 0.85);
        let transmission =
            crate::water_audio::scene::acoustic_transmission(listener, position, |p| {
                chunks.block_at(p)
            });
        let gain =
            (0.15 + f32::from(level) * 0.075) * (1.0 - distance / RANGE).powi(2) * transmission;
        for (sum, value) in energy
            .iter_mut()
            .zip([gain * (0.7 - pan * 0.3), gain * (0.7 + pan * 0.3)])
        {
            *sum += value * value;
        }
    }
    (energy.map(|value| value.sqrt().min(0.55)), count)
}

#[derive(bevy::ecs::system::SystemParam)]
pub(super) struct World<'w, 's> {
    access: Res<'w, WorldAccess>,
    session: Option<Res<'w, ClientSession>>,
    journal: Res<'w, EditJournal>,
    chunks: Res<'w, ChunkStore>,
    menu: Option<Res<'w, terra_ui::menu::MenuState>>,
    bodies: Query<'w, 's, &'static Transform, With<terra_player::PlayerBody>>,
    cameras: Query<'w, 's, &'static Transform, With<terra_player::TerraCamera>>,
}

pub(super) fn update(
    world: World,
    sounds: Res<Soundscape>,
    mut status: ResMut<HydraulicAudioStatus>,
) {
    let active = world.access.ready()
        && world
            .menu
            .as_ref()
            .is_none_or(|menu| menu.world_audio_active())
        && sounds.entity.is_some();
    let (gains, taps) =
        world
            .bodies
            .iter()
            .next()
            .filter(|_| active)
            .map_or(([0.0; 2], 0), |body| {
                mix_faucets(
                    world
                        .session
                        .as_ref()
                        .map_or(&[], |session| session.plumbing.as_slice()),
                    &world.chunks,
                    &world.journal,
                    body.translation + Vec3::Y * terra_physics::character::PLAYER_EYE_OFFSET,
                    world
                        .cameras
                        .iter()
                        .next()
                        .map_or(Vec3::X, |camera| *camera.right()),
                )
            });
    for (target, gain) in sounds.controls.faucet.iter().zip(gains) {
        target.store(gain.to_bits(), Ordering::Relaxed);
    }
    status.taps = taps;
    status.gains = gains;
    status.output_rms = f32::from_bits(sounds.controls.faucet_rms.load(Ordering::Relaxed));
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_voxel::{BlockSection, global_to_section};
    use terra_world::{edits::BlockObjectState, streaming::LoadedSection};

    fn fixture() -> (ChunkStore, EditJournal, PlumbingVisual) {
        let cell = [2, 1, 0];
        let mut chunks = ChunkStore::default();
        let (section, local) = global_to_section(cell);
        let mut data = BlockSection::empty(section).unwrap();
        data.set_block(local, FAUCET).unwrap();
        chunks.loaded.insert(
            section,
            LoadedSection {
                data,
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            },
        );
        let mut journal = EditJournal::default();
        journal.record(cell, FAUCET).unwrap();
        journal
            .set_object_state(cell, Some(BlockObjectState::default()))
            .unwrap();
        (
            chunks,
            journal,
            PlumbingVisual {
                intake: None,
                position: cell,
                amount: 8,
                capacity: 8,
                open: true,
                flowing: true,
                supplied: true,
                pump_rpm: 0.0,
                basin: None,
            },
        )
    }

    #[test]
    fn taps_pan_scale_with_stage_and_stop_when_closed_dry_distant_removed_or_unloaded() {
        let (mut chunks, mut journal, mut tap) = fixture();
        let sample = |tap: PlumbingVisual, chunks: &ChunkStore, journal: &EditJournal, listener| {
            mix_faucets(&[tap], chunks, journal, listener, Vec3::X)
        };
        let listener = Vec3::new(0.5, 1.25, 0.5);
        let (quiet, count) = sample(tap, &chunks, &journal, listener);
        assert_eq!(count, 1);
        assert!(quiet[1] > quiet[0] && quiet[0] > 0.0);
        journal
            .set_object_state(
                tap.position,
                Some(BlockObjectState {
                    facing: plumbing::faucet_facing(0, 3),
                    ..Default::default()
                }),
            )
            .unwrap();
        assert!(sample(tap, &chunks, &journal, listener).0[1] > quiet[1]);
        assert_eq!(
            sample(tap, &chunks, &journal, listener + Vec3::Z * 21.0),
            ([0.0; 2], 0)
        );
        tap.open = false;
        assert_eq!(sample(tap, &chunks, &journal, listener), ([0.0; 2], 0));
        tap.open = true;
        tap.amount = 0;
        assert_eq!(sample(tap, &chunks, &journal, listener), ([0.0; 2], 0));
        tap.amount = 8;
        tap.flowing = false;
        tap.supplied = false;
        assert_eq!(sample(tap, &chunks, &journal, listener), ([0.0; 2], 0));
        tap.supplied = true;
        let (gains, count) = mix_faucets(&vec![tap; 300], &chunks, &journal, listener, Vec3::X);
        assert_eq!(count, MAX_TAPS);
        assert!(gains.into_iter().all(|gain| gain <= 0.55));
        let (section, local) = global_to_section(tap.position);
        chunks
            .loaded
            .get_mut(&section)
            .unwrap()
            .data
            .set_block(local, 0)
            .unwrap();
        assert_eq!(sample(tap, &chunks, &journal, listener), ([0.0; 2], 0));
        chunks
            .loaded
            .get_mut(&section)
            .unwrap()
            .data
            .set_block(local, FAUCET)
            .unwrap();
        chunks.loaded.clear();
        assert_eq!(sample(tap, &chunks, &journal, listener), ([0.0; 2], 0));
    }
}
