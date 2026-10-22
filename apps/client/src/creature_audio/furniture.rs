//! Confirmed opening poses, burning furnaces and body-based smoke exposure.
use super::{
    clips,
    scene::Soundscape,
    source::{Category, Cue},
};
use bevy::prelude::*;
use std::{collections::HashMap, sync::atomic::Ordering};
use terra_network::client::ClientSession;
use terra_world::{
    edits::EditJournal,
    streaming::{ChunkStore, WorldAccess},
};

#[derive(Default)]
pub(super) struct Timing {
    next_scan: f32,
    cough_at: Option<f32>,
    last_cough: Option<usize>,
    openings: HashMap<[i64; 3], (u16, u8)>,
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
) {
    let active = access.ready()
        && menu.as_ref().is_none_or(|menu| menu.world_audio_active())
        && sounds.entity.is_some();
    let Some(body) = bodies.iter().next().filter(|_| active) else {
        *timing = Timing::default();
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
    for (cell, material, state) in journal.object_states().filter(|(_, material, state)| {
        terra_world::objects::is_opening(*material) && !state.secondary
    }) {
        let position = Vec3::from_array(cell.map(|v| v as f32)) + Vec3::splat(0.5);
        if position.distance_squared(listener) > 20.0_f32.powi(2)
            || chunks.block_at(cell) != Some(material)
        {
            continue;
        }
        if let Some(&(old_material, old_facing)) = timing.openings.get(&cell)
            && old_material == material
            && (old_facing >= 4) != (state.facing >= 4)
        {
            let window = terra_world::objects::is_window(material);
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
    let mut energy = [0.0_f32; 2];
    if let Some(session) = session.as_ref() {
        for furnace in session.furnaces.iter().filter(|f| f.lit).take(32) {
            let position =
                Vec3::from_array(furnace.position.map(|v| v as f32)) + Vec3::new(0.5, 0.8, 0.5);
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
