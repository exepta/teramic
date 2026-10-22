//! Explicit contact-message fixture for recorded material playback, not real walking input.
use super::{scene::Soundscape, source::RecordedSoundscape};
use bevy::{audio::AudioSink, prelude::*};
use std::{
    sync::atomic::Ordering,
    time::{Duration, Instant},
};
use terra_core::footsteps::{Footstep, FootstepSet, Walker};

#[derive(Resource)]
struct Fixture {
    started: Instant,
    next: Instant,
    emitted: usize,
    point: Vec3,
    baseline: Option<u32>,
}

pub(super) fn install(app: &mut App) {
    if !std::env::args().any(|arg| arg == "--smoke-test-steps") {
        return;
    }
    app.insert_resource(terra_player::ControllerMode::Walk)
        .insert_resource(terra_core::AudioConfig::default())
        .insert_resource(terra_ui::menu::MenuState {
            page: terra_ui::menu::MenuPage::Playing,
            session_active: true,
            ..default()
        })
        .add_systems(PostStartup, setup)
        .add_systems(
            Update,
            run.after(terra_player::PlayerSet::Movement)
                .before(FootstepSet),
        );
}

fn setup(world: &mut World) {
    let height = world
        .resource::<terra_world::streaming::WorldTerrain>()
        .0
        .surface_column(8.0, 8.0)
        .height;
    let point = Vec3::new(8.0, height as f32 + 20.0, 8.0);
    world.spawn((terra_player::PlayerBody, Transform::from_translation(point)));
    world.insert_resource(Fixture {
        started: Instant::now(),
        next: Instant::now(),
        emitted: 0,
        point,
        baseline: None,
    });
}

fn run(world: &mut World) {
    world.resource_scope(|world, mut fixture: Mut<Fixture>| {
        if fixture.started.elapsed() > Duration::from_secs(60) {
            error!(emitted=fixture.emitted, "Recorded material contact fixture timed out");
            world.write_message(AppExit::error());
            return;
        }
        for mut body in world.query_filtered::<&mut Transform, With<terra_player::PlayerBody>>().iter_mut(world) {
            body.translation = fixture.point;
        }
        world.resource_mut::<terra_world::streaming::ChunkFocus>().0 = fixture.point.to_array().map(f64::from);
        let soundscape = world.resource::<Soundscape>();
        let Some(entity) = soundscape.entity else { return; };
        if world.get::<AudioSink>(entity).is_none()
            || world.get::<AudioPlayer<RecordedSoundscape>>(entity).is_none() { return; }
        let played = soundscape.controls.played.load(Ordering::Relaxed);
        let baseline = *fixture.baseline.get_or_insert(played);
        if Instant::now() < fixture.next { return; }
        let materials = [1_u16, 4, 5, 3, 16, 2, 42, 15, 7];
        if fixture.emitted == materials.len() * 3 {
            if played.wrapping_sub(baseline) < fixture.emitted as u32 { return; }
            info!(contacts=fixture.emitted, played=played.wrapping_sub(baseline),
                "Recorded steps passed: grass, sand, snow, stone, wood, dirt, gravel, glass, leaves; real sink consumed injected contacts");
            world.write_message(AppExit::Success);
            return;
        }
        let material = materials[fixture.emitted / 3];
        world.write_message(Footstep {
            position: fixture.point - Vec3::Y * terra_physics::character::PLAYER_HALF_HEIGHT,
            forward: Vec2::Y, walker: Walker::Player, material,
            alternate: fixture.emitted.is_multiple_of(2),
        });
        info!(material, contact=fixture.emitted + 1, "Recorded material contact fixture");
        fixture.emitted += 1;
        fixture.next = Instant::now() + Duration::from_millis(550);
    });
}
