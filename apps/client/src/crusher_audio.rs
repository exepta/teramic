//! One bounded mechanical mix driven only by acknowledged nearby machine states.
mod source;

use bevy::{
    audio::AddAudioSource,
    prelude::*,
    tasks::{AsyncComputeTaskPool, Task, futures_lite::future},
};
use source::{FoleyBank, MachineSource, MixControls};
use std::sync::Arc;
use terra_core::AudioConfig;
use terra_network::{
    client::ClientSession,
    protocol::session::{CrusherVisual, KineticVisual},
};
use terra_player::{PlayerBody, TerraCamera};
use terra_ui::menu::MenuState;
use terra_world::{edits::EditJournal, streaming::WorldAccess};

pub(crate) struct CrusherAudioPlugin;

impl Plugin for CrusherAudioPlugin {
    fn build(&self, app: &mut App) {
        app.add_audio_source::<MachineSource>()
            .init_resource::<Soundscape>()
            .init_resource::<CrusherAudioStatus>()
            .add_systems(Startup, load)
            .add_systems(Update, update.after(terra_player::PlayerSet::Movement));
    }
}

#[derive(Resource, Default)]
struct Soundscape {
    controls: Arc<MixControls>,
    source: Option<Handle<MachineSource>>,
    entity: Option<Entity>,
    bank: Option<FoleyBank>,
    loading: Option<Task<Result<FoleyBank, String>>>,
}

#[derive(Resource, Default, Debug)]
pub(crate) struct CrusherAudioStatus {
    pub nearby: usize,
    pub turning: usize,
    pub grinding: usize,
    pub playback_started: bool,
    pub output_rms: f32,
    pub foley_ready: bool,
    pub foley_voices: usize,
    pub kinetic_parts: usize,
}

fn load(mut sounds: ResMut<Soundscape>) {
    sounds.loading = Some(AsyncComputeTaskPool::get().spawn(async { source::load_foley() }));
}

fn mix(
    visuals: &[CrusherVisual],
    listener: Vec3,
    right: Vec3,
    config: &AudioConfig,
    journal: &EditJournal,
) -> ([[f32; 2]; 2], [usize; 3]) {
    if visuals.is_empty() {
        return ([[0.0; 2]; 2], [0; 3]);
    }
    let mut nearest: Vec<(f32, &CrusherVisual, Vec3)> = Vec::with_capacity(8);
    for machine in visuals.iter().take(64) {
        if !machine.manual_active && !machine.working {
            continue;
        }
        let position = machine_position(machine.position, journal);
        let distance = listener.distance(position);
        if distance >= 24.0 {
            continue;
        }
        let index = nearest.partition_point(|(previous, _, _)| *previous < distance);
        if index < 8 {
            if nearest.len() == 8 {
                nearest.pop();
            }
            nearest.insert(index, (distance, machine, position));
        }
    }
    let mut gains = [[0.0_f32; 2]; 2];
    let mut counts = [nearest.len(), 0, 0];
    let master = if config.muted {
        0.0
    } else {
        config.master_volume * config.ambience_volume
    };
    for (distance, machine, position) in nearest {
        let pan = if distance > 0.2 {
            ((position - listener) / distance)
                .dot(right)
                .clamp(-0.85, 0.85)
        } else {
            0.0
        };
        let fade = (1.0 - distance / 24.0).clamp(0.0, 1.0);
        let gain = fade * fade * master;
        for (kind, active) in [machine.manual_active, machine.working]
            .into_iter()
            .enumerate()
        {
            counts[kind + 1] += usize::from(active);
            if active {
                gains[kind][0] += gain * ((1.0 - pan) * 0.5).sqrt();
                gains[kind][1] += gain * ((1.0 + pan) * 0.5).sqrt();
            }
        }
    }
    for gain in gains.iter_mut().flatten() {
        *gain = (*gain).min(1.0) * 0.42;
    }
    (gains, counts)
}

fn machine_position(anchor: [i64; 3], journal: &EditJournal) -> Vec3 {
    let state = journal.object_state(anchor).unwrap_or_default();
    let legacy_width =
        if journal.block(anchor) == Some(terra_core::crusher::MATERIAL) && state.peer.is_some() {
            0.5
        } else {
            0.0
        };
    Vec3::from_array(anchor.map(|value| value as f32))
        + Vec3::new(0.5, 0.55, 0.5)
        + Vec3::from_array(state.right().map(|value| value as f32)) * legacy_width
}

fn mix_kinetics(
    visuals: &[KineticVisual],
    listener: Vec3,
    right: Vec3,
    config: &AudioConfig,
    journal: &EditJournal,
) -> ([f32; 2], usize, f32) {
    if visuals.is_empty() {
        return ([0.0; 2], 0, 1.0);
    }
    let mut nearest = Vec::with_capacity(8);
    for visual in visuals.iter().take(256) {
        if visual.overloaded
            || !visual.rpm.is_finite()
            || visual.rpm.abs() < 0.01
            || !journal
                .block(visual.position)
                .is_some_and(|m| matches!(m, 141..=151 | 156 | terra_core::plumbing::PUMP))
        {
            continue;
        }
        let delta =
            Vec3::from_array(visual.position.map(|v| v as f32)) + Vec3::splat(0.5) - listener;
        let distance = delta.length();
        if distance >= 24.0 {
            continue;
        }
        let index = nearest.partition_point(|(d, _, _): &(f32, Vec3, f32)| *d < distance);
        if index < 8 {
            if nearest.len() == 8 {
                nearest.pop();
            }
            nearest.insert(index, (distance, delta, visual.rpm.abs()));
        }
    }
    let count = nearest.len();
    let mut gains = [0.0_f32; 2];
    let mut rate = 0.0;
    let master = if config.muted {
        0.0
    } else {
        config.master_volume * config.ambience_volume
    };
    for (distance, delta, rpm) in nearest {
        let pan = if distance > 0.2 {
            (delta / distance).dot(right).clamp(-0.85, 0.85)
        } else {
            0.0
        };
        let gain =
            (1.0 - distance / 24.0).powi(2) * master * 0.14 * (rpm / 30.0).sqrt().clamp(0.4, 1.6);
        gains[0] += gain * ((1.0 - pan) * 0.5).sqrt();
        gains[1] += gain * ((1.0 + pan) * 0.5).sqrt();
        rate += rpm / 30.0;
    }
    (
        gains.map(|v| v.min(0.42)),
        count,
        if count == 0 {
            1.0
        } else {
            (rate / count as f32).clamp(0.25, 4.0)
        },
    )
}

#[derive(bevy::ecs::system::SystemParam)]
struct AudioWorld<'w, 's> {
    access: Res<'w, WorldAccess>,
    journal: Res<'w, EditJournal>,
    menu: Option<Res<'w, MenuState>>,
    preferences: crate::audio_preferences::AudioPreferences<'w>,
    session: Option<Res<'w, ClientSession>>,
    bodies: Query<'w, 's, &'static Transform, With<PlayerBody>>,
    cameras: Query<'w, 's, &'static Transform, With<TerraCamera>>,
    sinks: Query<'w, 's, (), With<bevy::audio::AudioSink>>,
}

fn update(
    world: AudioWorld,
    mut sounds: ResMut<Soundscape>,
    mut assets: ResMut<Assets<MachineSource>>,
    mut status: ResMut<CrusherAudioStatus>,
    mut commands: Commands,
) {
    if let Some(result) = sounds
        .loading
        .as_mut()
        .and_then(|task| future::block_on(future::poll_once(task)))
    {
        sounds.loading = None;
        match result {
            Ok(bank) => sounds.bank = Some(bank),
            Err(error) => error!(%error, "Cannot decode crusher material Foley"),
        }
    }
    let active = world.access.ready()
        && world
            .menu
            .as_ref()
            .is_none_or(|menu| menu.world_audio_active());
    let listener = world
        .bodies
        .iter()
        .next()
        .map(|body| body.translation + Vec3::Y * terra_physics::character::PLAYER_EYE_OFFSET);
    if !active || listener.is_none() {
        sounds.controls.update([[0.0; 2]; 2]);
        if let Some(entity) = sounds.entity.take() {
            commands.entity(entity).try_despawn();
        }
        *status = CrusherAudioStatus::default();
        return;
    }
    let config = world.preferences.current();
    let (mut gains, counts) = mix(
        world
            .session
            .as_ref()
            .map_or(&[], |session| session.crushers.as_slice()),
        listener.unwrap_or_default(),
        world
            .cameras
            .iter()
            .next()
            .map_or(Vec3::X, |camera| *camera.right()),
        config,
        &world.journal,
    );
    let (gears, kinetic_parts, rate) = mix_kinetics(
        world
            .session
            .as_ref()
            .map_or(&[], |s| s.kinetics.as_slice()),
        listener.unwrap_or_default(),
        world.cameras.iter().next().map_or(Vec3::X, |c| *c.right()),
        config,
        &world.journal,
    );
    for (gain, gear) in gains[0].iter_mut().zip(gears) {
        *gain = (*gain + gear).min(0.42);
    }
    sounds.controls.update(gains);
    sounds.controls.update_rate(rate);
    if counts[0] + kinetic_parts > 0
        && sounds.entity.is_none()
        && let Some(bank) = sounds.bank.clone()
    {
        let handle = sounds.source.clone().unwrap_or_else(|| {
            let handle = assets.add(MachineSource {
                controls: sounds.controls.clone(),
                bank,
            });
            sounds.source = Some(handle.clone());
            handle
        });
        sounds.entity = Some(
            commands
                .spawn((
                    Name::new("Crusher mechanical soundscape"),
                    AudioPlayer(handle),
                ))
                .id(),
        );
    }
    *status = CrusherAudioStatus {
        nearby: counts[0],
        turning: counts[1],
        grinding: counts[2],
        playback_started: sounds
            .entity
            .is_some_and(|entity| world.sinks.contains(entity)),
        output_rms: sounds.controls.rms(),
        foley_ready: sounds.bank.is_some(),
        foley_voices: sounds.controls.voices(),
        kinetic_parts,
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn running_kinetic_parts_are_audible_in_both_directions_but_stopped_overloaded_and_crushers_are_not()
     {
        let mut journal = EditJournal::default();
        let position = [2, 1, 0];
        journal.record(position, 141).unwrap();
        let mut visual = KineticVisual {
            position,
            rpm: 30.0,
            overloaded: false,
            ..Default::default()
        };
        let listen = Vec3::new(0.0, 1.5, 0.5);
        let sample = |v: &KineticVisual, c: &AudioConfig, j: &EditJournal| {
            mix_kinetics(std::slice::from_ref(v), listen, Vec3::X, c, j)
        };
        let config = AudioConfig::default();
        let (gain, count, rate) = sample(&visual, &config, &journal);
        assert_eq!(count, 1);
        assert!(gain[1] > gain[0] && gain[0] > 0.0);
        assert_eq!(rate, 1.0);
        visual.rpm = -30.0;
        assert_eq!(sample(&visual, &config, &journal).0, gain);
        visual.rpm = 60.0;
        assert_eq!(sample(&visual, &config, &journal).2, 2.0);
        let muted = AudioConfig {
            muted: true,
            ..config
        };
        assert_eq!(sample(&visual, &muted, &journal).0, [0.0; 2]);
        for rpm in [0.0, f32::NAN] {
            visual.rpm = rpm;
            assert_eq!(sample(&visual, &AudioConfig::default(), &journal).1, 0);
        }
        visual.rpm = 30.0;
        visual.overloaded = true;
        assert_eq!(sample(&visual, &AudioConfig::default(), &journal).1, 0);
        visual.overloaded = false;
        journal.record(position, 156).unwrap();
        assert_eq!(sample(&visual, &AudioConfig::default(), &journal).1, 1);
        visual.rpm = 0.0;
        assert_eq!(sample(&visual, &AudioConfig::default(), &journal).1, 0);
        visual.rpm = 30.0;
        journal
            .record(position, terra_core::plumbing::PUMP)
            .unwrap();
        assert_eq!(sample(&visual, &AudioConfig::default(), &journal).1, 1);
        visual.rpm = -60.0;
        assert_eq!(sample(&visual, &AudioConfig::default(), &journal).2, 2.0);
        visual.rpm = 0.0;
        assert_eq!(sample(&visual, &AudioConfig::default(), &journal).1, 0);
        visual.rpm = 30.0;
        journal.record(position, 99).unwrap();
        assert_eq!(sample(&visual, &AudioConfig::default(), &journal).1, 0);
    }

    #[test]
    fn nearby_manual_turning_is_audible_without_processable_input_and_bounded() {
        let journal = EditJournal::default();
        let mut machine = CrusherVisual {
            position: [0; 3],
            working: false,
            progress: 0.0,
            manual_active: true,
        };
        let (gain, counts) = mix(
            &[machine.clone()],
            Vec3::new(-1.0, 1.0, 0.5),
            Vec3::X,
            &AudioConfig::default(),
            &journal,
        );
        assert_eq!(counts, [1, 1, 0]);
        assert!(gain[0][1] > gain[0][0]);
        let (gain, counts) = mix(
            &vec![machine.clone(); 128],
            Vec3::new(-1.0, 1.0, 0.5),
            Vec3::X,
            &AudioConfig::default(),
            &journal,
        );
        assert_eq!(counts, [8, 8, 0]);
        assert_eq!(gain[1], [0.0; 2]);
        assert!(gain.iter().flatten().all(|v| *v <= 0.42));
        machine.working = true;
        let (gain, _) = mix(
            &[machine.clone()],
            Vec3::ZERO,
            Vec3::X,
            &AudioConfig {
                muted: true,
                ..default()
            },
            &journal,
        );
        assert_eq!(gain, [[0.0; 2]; 2]);
        let (_, counts) = mix(
            &[machine],
            Vec3::splat(100.0),
            Vec3::X,
            &AudioConfig::default(),
            &journal,
        );
        assert_eq!(counts, [0; 3]);
    }

    #[test]
    fn one_cell_machine_audio_stays_at_the_body_center_for_every_yaw() {
        let anchor = [-17, 4, -33];
        let mut journal = EditJournal::default();
        journal
            .record(anchor, terra_core::crusher::MATERIAL)
            .unwrap();
        for facing in 0..4 {
            journal
                .set_object_state(
                    anchor,
                    Some(terra_world::edits::BlockObjectState {
                        facing,
                        ..default()
                    }),
                )
                .unwrap();
            assert_eq!(
                machine_position(anchor, &journal),
                Vec3::new(-16.5, 4.55, -32.5)
            );
        }
    }

    #[test]
    fn source_is_centered_between_both_cells_for_all_facings_at_negative_coordinates() {
        use terra_world::edits::BlockObjectState;
        let anchor = [-17, 4, -33];
        let mut journal = EditJournal::default();
        journal
            .record(anchor, terra_core::crusher::MATERIAL)
            .unwrap();
        let machine = CrusherVisual {
            position: anchor,
            working: true,
            progress: 0.0,
            manual_active: true,
        };
        let mut previous = None;
        for (facing, offset) in [
            Vec3::new(1.0, 0.55, 0.5),
            Vec3::new(0.5, 0.55, 0.0),
            Vec3::new(0.0, 0.55, 0.5),
            Vec3::new(0.5, 0.55, 1.0),
        ]
        .into_iter()
        .enumerate()
        {
            let state = BlockObjectState {
                facing: facing as u8,
                ..default()
            };
            let right = state.right();
            journal
                .set_object_state(
                    anchor,
                    Some(BlockObjectState {
                        peer: Some(std::array::from_fn(|axis| anchor[axis] + right[axis])),
                        ..state
                    }),
                )
                .unwrap();
            let center = Vec3::from_array(anchor.map(|v| v as f32)) + offset;
            assert_eq!(machine_position(anchor, &journal), center);
            let right = Vec3::from_array(right.map(|v| v as f32));
            let (gains, counts) = mix(
                std::slice::from_ref(&machine),
                center + right * 3.0,
                right,
                &AudioConfig::default(),
                &journal,
            );
            assert_eq!(counts, [1, 1, 1]);
            assert!(gains[0][0] > gains[0][1]);
            if let Some(previous) = previous {
                assert_eq!(
                    gains, previous,
                    "rotating the machine and listener must not change attenuation or pan"
                );
            }
            previous = Some(gains);
        }
    }
}
