//! Confirmed opening poses, burning furnaces and body-based smoke exposure.
use super::{
    clips,
    scene::Soundscape,
    source::{Category, Cue},
};
use bevy::prelude::*;
use std::{collections::HashMap, sync::atomic::Ordering};
use terra_network::client::ClientSession;
use terra_voxel::{SectionPos, global_to_section};
use terra_world::{
    edits::EditJournal,
    streaming::{ChunkStore, WorldAccess},
};

const OPENING_RANGE: f32 = 20.0;

fn listener_sections(listener: Vec3) -> impl Iterator<Item = SectionPos> {
    let bounds = listener.is_finite().then(|| {
        let low = listener
            .to_array()
            .map(|v| (f64::from(v) - f64::from(OPENING_RANGE)).floor() as i64);
        let high = listener
            .to_array()
            .map(|v| (f64::from(v) + f64::from(OPENING_RANGE)).floor() as i64);
        (global_to_section(low).0, global_to_section(high).0)
    });
    bounds.into_iter().flat_map(|(low, high)| {
        (low.z..=high.z).flat_map(move |z| {
            (low.y..=high.y)
                .flat_map(move |y| (low.x..=high.x).map(move |x| SectionPos::new(x, y, z)))
        })
    })
}

fn nearby_openings<'a>(
    journal: &'a EditJournal,
    chunks: &'a ChunkStore,
    listener: Vec3,
) -> impl Iterator<Item = ([i64; 3], u16, terra_world::edits::BlockObjectState)> + 'a {
    listener_sections(listener)
        .filter(move |section| chunks.loaded.contains_key(section))
        .flat_map(move |section| journal.object_states_in_section(section))
        .filter(move |(cell, material, state)| {
            terra_world::objects::is_opening(*material)
                && !state.secondary
                && chunks.block_at(*cell) == Some(*material)
                && (Vec3::from_array(cell.map(|v| v as f32)) + Vec3::splat(0.5))
                    .distance_squared(listener)
                    <= OPENING_RANGE.powi(2)
        })
}

#[derive(Default)]
pub(super) struct Timing {
    next_scan: f32,
    cough_at: Option<f32>,
    last_cough: Option<usize>,
    openings: HashMap<[i64; 3], (u16, u8)>,
    chests: super::chests::Transitions,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn update(
    time: Res<Time>,
    access: Res<WorldAccess>,
    session: Option<Res<ClientSession>>,
    journal: Res<EditJournal>,
    chunks: Res<ChunkStore>,
    sounds: Res<Soundscape>,
    menu: Option<Res<terra_ui::menu::MenuState>>,
    bodies: Query<&Transform, With<terra_player::PlayerBody>>,
    cameras: Query<&Transform, With<terra_player::TerraCamera>>,
    mut timing: Local<Timing>,
    mut chest_status: ResMut<super::ChestAudioStatus>,
) {
    let active = access.ready()
        && menu.as_ref().is_none_or(|menu| menu.world_audio_active())
        && sounds.entity.is_some();
    let Some(body) = bodies.iter().next().filter(|_| active) else {
        *timing = Timing::default();
        *chest_status = super::ChestAudioStatus::default();
        for gain in &sounds.controls.furnace {
            gain.store(0, Ordering::Relaxed);
        }
        return;
    };
    let now = time.elapsed_secs();
    if now < timing.next_scan {
        return;
    }
    timing.next_scan = now + 0.1;
    let listener = body.translation + Vec3::Y * terra_physics::character::PLAYER_EYE_OFFSET;
    let right = cameras
        .iter()
        .next()
        .map_or(Vec3::X, |camera| *camera.right());
    let emit = |sound, position: Vec3, gain, pitch| {
        let delta = position - listener;
        let pan = delta.normalize_or_zero().dot(right).clamp(-1.0, 1.0);
        let level = gain * (1.0 - delta.length() / 20.0).clamp(0.0, 1.0).powi(2);
        if level > 0.001 {
            sounds.controls.enqueue(Cue {
                category: Category::Environment,
                sound,
                gains: [level * (0.7 - pan * 0.3), level * (0.7 + pan * 0.3)],
                pitch,
                reverb: sounds.room.cave * 0.45,
            });
        }
    };
    let mut current = HashMap::new();
    let mut sounded = std::collections::HashSet::new();
    for (cell, material, state) in nearby_openings(&journal, &chunks, listener) {
        let position = Vec3::from_array(cell.map(|v| v as f32)) + Vec3::splat(0.5);
        if let Some(&(old_material, old_facing)) = timing.openings.get(&cell)
            && old_material == material
            && (old_facing >= 4) != (state.facing >= 4)
        {
            let window = terra_world::objects::is_window(material);
            let anchor = if window {
                terra_world::objects::windows::group(cell, material, state.facing, |p| {
                    Some((
                        chunks.block_at(p).or_else(|| journal.block(p))?,
                        journal.object_state(p)?.facing,
                    ))
                })
                .anchor
            } else if terra_world::objects::is_door(material) {
                terra_world::objects::doors::group(cell, material, state, |p| {
                    Some((
                        chunks.block_at(p).or_else(|| journal.block(p))?,
                        journal.object_state(p)?,
                    ))
                })
                .map_or(cell, |group| group.anchor)
            } else {
                cell
            };
            if !sounded.insert(anchor) {
                current.insert(cell, (material, state.facing));
                continue;
            }
            emit(
                if state.facing >= 4 {
                    clips::DOOR_OPEN_CLIP
                } else {
                    clips::DOOR_CLOSE_CLIP
                },
                position,
                if window { 0.35 } else { 0.50 },
                if window { 1.12 } else { 1.0 },
            );
        }
        current.insert(cell, (material, state.facing));
    }
    timing.openings = current;
    for (anchor, open) in timing
        .chests
        .update(&chunks, &journal, session.as_deref(), listener)
    {
        emit(
            if open {
                clips::CHEST_OPEN_CLIP
            } else {
                clips::CHEST_CLOSE_CLIP
            },
            Vec3::from_array(anchor.map(|v| v as f32)) + Vec3::splat(0.5),
            0.55,
            1.0,
        );
        if open {
            chest_status.opened = chest_status.opened.saturating_add(1);
        } else {
            chest_status.closed = chest_status.closed.saturating_add(1);
        }
    }
    chest_status.tracked = timing.chests.len();
    let mut energy = [0.0_f32; 2];
    if let Some(session) = session.as_ref() {
        let mut fires: Vec<_> = session
            .furnaces
            .iter()
            .filter(|f| f.lit)
            .map(|f| (f.position, 59, 0.8))
            .chain(session.campfires.iter().filter(|f| f.lit).map(|f| {
                if journal
                    .object_state(f.position)
                    .is_some_and(|state| terra_core::brewing::has_fire(state.facing))
                    && chunks.block_at(f.position) == Some(161)
                {
                    (f.position, 161, 0.27)
                } else {
                    (f.position, 162, 0.4)
                }
            }))
            .filter(|(cell, material, _)| {
                chunks
                    .block_at(*cell)
                    .is_some_and(|raw| journal.block(*cell).unwrap_or(raw) == *material)
            })
            .map(|(cell, _, height)| {
                Vec3::from_array(cell.map(|v| v as f32)) + Vec3::new(0.5, height, 0.5)
            })
            .filter(|position| position.distance_squared(listener) < 16.0_f32.powi(2))
            .collect();
        fires.sort_by(|a, b| {
            a.distance_squared(listener)
                .total_cmp(&b.distance_squared(listener))
        });
        for position in fires.into_iter().take(32) {
            let delta = position - listener;
            if delta.length_squared() > 16.0_f32.powi(2) {
                continue;
            }
            let pan = delta.normalize_or_zero().dot(right).clamp(-1.0, 1.0);
            let gain = 0.50
                * (1.0 - delta.length() / 16.0).powi(2)
                * crate::water_audio::scene::acoustic_transmission(listener, position, |p| {
                    chunks.block_at(p)
                });
            for (sum, value) in energy
                .iter_mut()
                .zip([gain * (0.7 - pan * 0.3), gain * (0.7 + pan * 0.3)])
            {
                *sum += value * value;
            }
        }
        let exposed = session.game_mode != terra_core::GameMode::Spectator
            && session.furnaces.iter().any(|f| {
                f.smoke_cells
                    .iter()
                    .any(|cell| body_in_smoke(body.translation, *cell))
            });
        if exposed {
            let due = *timing.cough_at.get_or_insert(now + 1.4);
            if now >= due {
                let choice = cough_variant(timing.last_cough, now.to_bits());
                emit(clips::COUGH_CLIPS[choice], listener, 0.30, 1.0);
                timing.last_cough = Some(choice);
                timing.cough_at = Some(now + 8.5 + (now * 1.7).sin().abs() * 4.0);
            }
        } else {
            timing.cough_at = None;
        }
    }
    for (target, value) in sounds.controls.furnace.iter().zip(energy) {
        target.store(value.sqrt().min(0.55).to_bits(), Ordering::Relaxed);
    }
}

fn cough_variant(previous: Option<usize>, variation: u32) -> usize {
    previous.map_or((variation % 3) as usize, |previous| {
        (previous + 1 + (variation.rotate_right(7) % 2) as usize) % clips::COUGH_CLIPS.len()
    })
}

fn body_in_smoke(body: Vec3, cell: [i64; 3]) -> bool {
    let low = Vec3::from_array(cell.map(|v| v as f32));
    let half = Vec3::new(0.30, 0.87, 0.30);
    (body + half).cmpgt(low).all() && (body - half).cmplt(low + Vec3::ONE).all()
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_voxel::BlockSection;
    use terra_world::{edits::BlockObjectState, streaming::LoadedSection};

    fn load(chunks: &mut ChunkStore, position: [i64; 3], material: u16) {
        let (section, local) = global_to_section(position);
        chunks
            .loaded
            .entry(section)
            .or_insert_with(|| LoadedSection {
                data: BlockSection::empty(section).unwrap(),
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            })
            .data
            .set_block(local, material)
            .unwrap();
    }

    fn opening(
        journal: &mut EditJournal,
        position: [i64; 3],
        material: u16,
        state: BlockObjectState,
    ) {
        journal.record(position, material).unwrap();
        journal.set_object_state(position, Some(state)).unwrap();
    }

    #[test]
    fn listener_window_is_bounded_and_covers_negative_twenty_meter_section_edges() {
        for coordinate in -33..=33 {
            let listener = Vec3::splat(coordinate as f32 + 0.375);
            let sections: Vec<_> = listener_sections(listener).collect();
            assert!(sections.len() <= 64);
            for delta in [-20.0, 0.0, 20.0] {
                let cell = (listener + Vec3::splat(delta))
                    .floor()
                    .as_i64vec3()
                    .to_array();
                assert!(sections.contains(&global_to_section(cell).0));
            }
        }
        for listener in [Vec3::NAN, Vec3::splat(f32::INFINITY)] {
            assert_eq!(listener_sections(listener).count(), 0);
        }
    }

    #[test]
    fn opening_audio_tracks_near_loaded_poses_but_ignores_far_unloaded_and_stale_history() {
        let listener = Vec3::new(-0.5, 2.5, -0.5);
        let near = [-1, 2, -1];
        let edge = [19, 2, -1];
        let outside = [20, 2, -1];
        let far = [1024, 2, -1];
        let unloaded = [-1, 2, 1];
        let stale = [-2, 2, -1];
        let mut journal = EditJournal::default();
        let mut chunks = ChunkStore::default();
        for position in [near, edge, outside, far, unloaded, stale] {
            opening(&mut journal, position, 68, BlockObjectState::default());
            if ![unloaded, stale].contains(&position) {
                load(&mut chunks, position, 68);
            }
        }
        let mut before: Vec<_> = nearby_openings(&journal, &chunks, listener).collect();
        before.sort_by_key(|entry| entry.0);
        assert_eq!(
            before.iter().map(|entry| entry.0).collect::<Vec<_>>(),
            vec![near, edge]
        );
        assert!(before.iter().all(|entry| entry.2.facing == 0));
        journal
            .set_object_state(
                near,
                Some(BlockObjectState {
                    facing: 4,
                    ..Default::default()
                }),
            )
            .unwrap();
        assert!(
            nearby_openings(&journal, &chunks, listener).any(|(position, material, state)| {
                position == near && material == 68 && state.facing == 4
            }),
            "confirmed nearby open/close transitions retain their original audio inputs"
        );
    }

    #[test]
    fn opening_audio_retains_primary_only_loaded_contract_for_straddling_doors() {
        let primary = [-1, 15, -1];
        let peer = [-1, 16, -1];
        let mut journal = EditJournal::default();
        opening(
            &mut journal,
            primary,
            65,
            BlockObjectState {
                peer: Some(peer),
                ..Default::default()
            },
        );
        opening(
            &mut journal,
            peer,
            65,
            BlockObjectState {
                peer: Some(primary),
                secondary: true,
                ..Default::default()
            },
        );
        let mut chunks = ChunkStore::default();
        load(&mut chunks, peer, 65);
        let listener = Vec3::new(-0.5, 16.5, -0.5);
        assert!(
            nearby_openings(&journal, &chunks, listener)
                .next()
                .is_none()
        );
        load(&mut chunks, primary, 65);
        let entries: Vec<_> = nearby_openings(&journal, &chunks, listener).collect();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].0, primary);
    }

    #[test]
    fn recorded_coughs_use_three_distinct_takes_without_consecutive_repeats() {
        let mut previous = None;
        let mut seen = std::collections::BTreeSet::new();
        for variation in 0..30 {
            let choice = cough_variant(previous, variation);
            assert_ne!(previous, Some(choice));
            seen.insert(clips::COUGH_CLIPS[choice]);
            previous = Some(choice);
        }
        assert_eq!(seen.len(), 3);
    }
    #[test]
    fn cough_exposure_follows_the_body_not_a_nearby_cell_or_camera() {
        assert!(body_in_smoke(Vec3::new(-0.5, 1.9, 0.5), [-1, 2, 0]));
        assert!(!body_in_smoke(Vec3::new(-0.5, 1.9, 0.5), [0, 2, 0]));
        assert!(!body_in_smoke(Vec3::new(-0.5, 1.9, 0.5), [-1, 3, 0]));
    }
}
