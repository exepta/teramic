//! Native fixture verifies playback, body-based immersion, filter response and cleanup.
use super::{scene::WaterSoundStatus, source::WaterAudioSource};
use bevy::{app::AppExit, audio::AudioPlayer, prelude::*};
use std::time::{Duration, Instant};
use terra_core::AudioConfig;
use terra_player::{ControllerMode, PlayerBody, TerraCamera};
use terra_ui::menu::{MenuPage, MenuState};
use terra_world::streaming::{ChunkFocus, WorldTerrain};

#[derive(Resource)]
struct Fixture {
    point: Vec3,
    phase: u8,
    started: Instant,
    since: Instant,
    original_audio: Option<AudioConfig>,
    baseline_gains: [[f32; 2]; 3],
}

pub(super) fn install(app: &mut App) {
    if !std::env::args().any(|argument| argument == "--smoke-test-water-audio") {
        return;
    }
    app.insert_resource(ControllerMode::Walk)
        .insert_resource(MenuState {
            page: MenuPage::Playing,
            session_active: true,
            ..default()
        })
        .add_systems(PostStartup, locate)
        .add_systems(
            Update,
            pin_listener
                .after(terra_player::PlayerSet::Movement)
                .before(super::scene::update),
        )
        .add_systems(Update, verify.after(super::scene::update));
}

fn locate(world: &mut World) {
    let generator = &world.resource::<WorldTerrain>().0;
    let point = crate::hydrology_smoke::find_waterfall(generator).and_then(|(fall, _)| {
        let z = fall.z + 2.0;
        let pool = generator.water_column(f64::from(fall.x), f64::from(z))?;
        Some(Vec3::new(fall.x, pool.level as f32, z))
    });
    let Some(point) = point else {
        tracing::error!("Water audio fixture could not locate a generated waterfall");
        world.write_message(AppExit::error());
        return;
    };
    // A transform-only player fixture is not integrated by the character physics query.
    let original_audio = world.get_resource::<AudioConfig>().cloned();
    world.insert_resource(AudioConfig::default());
    world.spawn((PlayerBody, Transform::from_translation(point + Vec3::Y)));
    world.resource_mut::<ChunkFocus>().0 = point.to_array().map(f64::from);
    world.insert_resource(Fixture {
        point,
        phase: 0,
        started: Instant::now(),
        since: Instant::now(),
        original_audio,
        baseline_gains: [[0.0; 2]; 3],
    });
    tracing::info!(?point, "Water audio fixture located a generated waterfall");
}

fn pin_listener(world: &mut World) {
    let Some(fixture) = world.get_resource::<Fixture>() else {
        return;
    };
    let phase = fixture.phase;
    let point = fixture.point;
    if phase == 255 {
        return;
    }
    let underwater = phase == 1;
    let body = point + Vec3::Y * if underwater { -1.4 } else { 1.0 };
    for mut transform in world
        .query_filtered::<&mut Transform, With<PlayerBody>>()
        .iter_mut(world)
    {
        transform.translation = body;
    }
    // Force the camera into the opposite medium, after the normal follow-camera system.
    for mut camera in world
        .query_filtered::<&mut Transform, With<TerraCamera>>()
        .iter_mut(world)
    {
        camera.translation = point + Vec3::new(0.0, if underwater { 16.0 } else { -0.6 }, 0.0);
    }
    world.resource_mut::<ChunkFocus>().0 = point.to_array().map(f64::from);
}

fn verify(world: &mut World) {
    if !world.contains_resource::<Fixture>() {
        return;
    }
    world.resource_scope(|world, mut fixture: Mut<Fixture>| {
        if fixture.phase == 255 { return; }
        if fixture.started.elapsed() > Duration::from_secs(75) {
            tracing::error!(phase=fixture.phase, status=?world.resource::<WaterSoundStatus>(), "Water audio fixture timed out");
            restore_audio(world, &fixture.original_audio);
            world.write_message(AppExit::error());
            fixture.phase = 255;
            return;
        }
        if fixture.since.elapsed() < Duration::from_secs(2) { return; }
        let status = world.resource::<WaterSoundStatus>();
        match fixture.phase {
            0 => {
                if !status.playback_started || status.gains[2].iter().sum::<f32>() < 0.01 { return; }
                if status.submerged || status.filter_blend > 0.05 { return; }
                let expected = fixture.point + Vec3::Y * (1.0 + terra_physics::character::PLAYER_EYE_OFFSET);
                if status.listener.is_none_or(|listener| listener.distance(expected) > 0.01) { return; }
                tracing::info!(?status, "Water audio dry listener passed with submerged camera");
                fixture.phase = 1;
            }
            1 => {
                if !status.submerged || status.filter_blend < 0.95 { return; }
                tracing::info!(?status, "Water audio underwater low-pass passed with dry remote camera");
                fixture.phase = 2;
            }
            2 => {
                if status.submerged || status.filter_blend > 0.05 { return; }
                world.resource_mut::<MenuState>().page = MenuPage::Pause;
                fixture.phase = 3;
            }
            3 => {
                if !status.playback_started || status.gains[2].iter().sum::<f32>() < 0.01 { return; }
                fixture.baseline_gains = status.gains;
                world.resource_mut::<AudioConfig>().master_volume = 0.0;
                fixture.phase = 4;
            }
            4 => {
                if status.gains != [[0.0; 2]; 3] || status.output_rms > 0.00001 { return; }
                world.insert_resource(AudioConfig { ambience_volume: 0.0, ..default() });
                fixture.phase = 5;
            }
            5 => {
                if status.gains != [[0.0; 2]; 3] || status.output_rms > 0.00001 { return; }
                world.insert_resource(AudioConfig { waterfall_volume: 0.0, ..default() });
                fixture.phase = 6;
            }
            6 => {
                if status.gains[2] != [0.0; 2] || status.gains[1].iter().sum::<f32>() <= 0.001 || status.output_rms <= 0.00001 { return; }
                world.insert_resource(AudioConfig { muted: true, ..default() });
                fixture.phase = 7;
            }
            7 => {
                if status.gains != [[0.0; 2]; 3] || status.output_rms > 0.00001 { return; }
                world.insert_resource(AudioConfig {
                    master_volume: 0.5, ambience_volume: 0.5, water_volume: 0.4,
                    river_volume: 0.3, waterfall_volume: 0.2, muted: false,
                    ..default()
                });
                fixture.phase = 8;
            }
            8 => {
                for (kind, category) in [0.4, 0.3, 0.2].into_iter().enumerate() {
                    for channel in 0..2 {
                        let expected = fixture.baseline_gains[kind][channel] * 0.25 * category;
                        if (status.gains[kind][channel] - expected).abs() > 0.0001 { return; }
                    }
                }
                if status.output_rms <= 0.00001 { return; }
                tracing::info!(?status, "Water audio settings passed: master, ambience, categories, mute and actual decoder silence");
                restore_audio(world, &fixture.original_audio);
                let mut menu = world.resource_mut::<MenuState>();
                menu.page = MenuPage::Main;
                menu.session_active = false;
                fixture.phase = 9;
            }
            _ => {
                if status.listener.is_some() || status.playback_started { return; }
                if world.query::<&AudioPlayer<WaterAudioSource>>().iter(world).next().is_some() { return; }
                tracing::info!("Water audio fixture passed: real audio sink, player listener, river/fall mix, underwater DSP, audio settings, restoration, pause and session cleanup");
                world.write_message(AppExit::Success);
                fixture.phase = 255;
            }
        }
        fixture.since = Instant::now();
    });
}

fn restore_audio(world: &mut World, original: &Option<AudioConfig>) {
    if let Some(config) = original {
        world.insert_resource(config.clone());
    } else {
        world.remove_resource::<AudioConfig>();
    }
}
