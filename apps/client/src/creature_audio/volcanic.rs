//! An exposed pool has a low rolling bed, with individual gas releases and cooling fractures.
use super::{
    scene::Soundscape,
    source::{Category, Cue},
};
use bevy::prelude::*;
use std::sync::atomic::Ordering;
use terra_network::client::ClientSession;
use terra_render::VolcanicSound;

pub(super) const FIRST: usize = super::lava_worm::FIRST + super::lava_worm::COUNT;
pub(super) const RUMBLE: usize = FIRST + 4;
pub(super) const FLOW: usize = FIRST + 5;
const FLOW_RANGE: f32 = 18.;
const FLOW_GAIN: f32 = 0.11;
const BUBBLE_RANGE: f32 = 13.;
pub(super) const CLIPS: [&str; 6] = [
    "environment/lava-bubble.wav",
    "environment/lava-crust.wav",
    "environment/fumarole-hiss.wav",
    "environment/eruption-blast.wav",
    "environment/eruption-rumble.wav",
    "environment/lava-flow.wav",
];

fn stereo(source: Vec3, ear: Vec3, right: Vec3, range: f32, gain: f32) -> [f32; 2] {
    let d = source - ear;
    let volume = gain * (1. - d.length() / range).clamp(0., 1.);
    let pan = right.dot(d.normalize_or_zero()).clamp(-0.85, 0.85);
    [
        volume * ((1. - pan) * 0.5).sqrt(),
        volume * ((1. + pan) * 0.5).sqrt(),
    ]
}

fn nearby_stereo(source: Vec3, ear: Vec3, right: Vec3, range: f32, gain: f32) -> [f32; 2] {
    let falloff = (1. - source.distance(ear) / range).clamp(0., 1.);
    stereo(source, ear, right, range, gain * falloff)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn update(
    mut events: MessageReader<VolcanicSound>,
    session: Option<Res<ClientSession>>,
    sounds: Res<Soundscape>,
    surfaces: Option<Res<terra_render::VolcanicSources>>,
    time: Res<Time>,
    mut next_breach: Local<f32>,
    cameras: Query<&Transform, With<terra_player::TerraCamera>>,
    bodies: Query<&Transform, With<terra_player::PlayerBody>>,
    menu: Option<Res<terra_ui::menu::MenuState>>,
    mut last: Local<Option<(u32, u32)>>,
) {
    let live = sounds.entity.is_some() && menu.as_ref().is_none_or(|m| m.world_audio_active());
    let listener = bodies
        .iter()
        .next()
        .zip(cameras.iter().next())
        .filter(|_| live);
    let mut rumble = [0.; 2];
    let mut flow = [0.0_f32; 2];
    if let Some((body, camera)) = listener {
        let ear = body.translation + Vec3::Y * terra_physics::character::PLAYER_EYE_OFFSET;
        let right = *camera.right();
        if let Some(source) = surfaces.as_deref().and_then(|sources| {
            sources
                .surfaces
                .iter()
                .map(|p| Vec3::from_array(p.map(|v| v as f32)) + Vec3::new(0.5, 1., 0.5))
                .min_by(|a, b| a.distance_squared(ear).total_cmp(&b.distance_squared(ear)))
        }) {
            flow = nearby_stereo(source, ear, right, FLOW_RANGE, FLOW_GAIN);
        }
        let play = |sound, gains: [f32; 2], pitch| {
            if gains[0].max(gains[1]) > 0.002 {
                sounds.controls.enqueue(Cue {
                    category: Category::Environment,
                    sound,
                    gains,
                    pitch,
                    reverb: 0.12 + sounds.room.cave * 0.32,
                });
            }
        };
        if let Some(event) = session.as_deref().and_then(ClientSession::current_eruption) {
            let key = (event.vent.seed, event.cycle);
            let vent = Vec3::from_array(event.vent.origin.map(|v| v as f32));
            let envelope = event.pressure();
            rumble = stereo(vent, ear, right, 100., envelope * 0.97);
            if *last != Some(key) && event.age < 1.4 {
                play(FIRST + 3, stereo(vent, ear, right, 100., 1.3), 1.);
            }
            *last = Some(key);
        } else {
            *last = None;
        }
        for event in events.read() {
            let (point, sample, range, gain) = match *event {
                VolcanicSound::Bubble(p) => {
                    if time.elapsed_secs() < *next_breach
                        || p.distance_squared(ear) >= BUBBLE_RANGE.powi(2)
                    {
                        continue;
                    }
                    *next_breach = time.elapsed_secs() + 0.68;
                    // A rock fragment splitting on a bubble is shorter and more brittle.
                    let crust = (p.x.floor() as i64 ^ p.z.floor() as i64).rem_euclid(3) == 0;
                    (
                        p,
                        usize::from(crust),
                        BUBBLE_RANGE,
                        if crust { 0.073 } else { 0.102 },
                    )
                }
                VolcanicSound::Steam(p) => (p, 2, 21., 0.26),
            };
            let variation = (point.x.floor() as i64 ^ point.z.floor() as i64).rem_euclid(15);
            play(
                FIRST + sample,
                nearby_stereo(point, ear, right, range, gain),
                0.9 + variation as f32 * 0.014,
            );
        }
    } else {
        events.clear();
        if session.as_ref().is_none_or(|s| !s.ready) {
            *last = None;
        }
    }
    for (out, gain) in sounds.controls.lava.iter().zip(flow) {
        out.store(gain.to_bits(), Ordering::Relaxed);
    }
    for (out, gain) in sounds.controls.eruption.iter().zip(rumble) {
        out.store(gain.to_bits(), Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lava_bed_is_quiet_and_falls_off_before_the_next_pool() {
        let power = |distance: f32| {
            nearby_stereo(
                Vec3::Z * distance,
                Vec3::ZERO,
                Vec3::X,
                FLOW_RANGE,
                FLOW_GAIN,
            )
            .into_iter()
            .map(|gain| gain * gain)
            .sum::<f32>()
        };
        assert!(power(0.) < 0.35_f32.powi(2) / 9.);
        assert!((power(FLOW_RANGE / 2.) / power(0.) - 0.0625).abs() < 0.0001);
        assert_eq!(power(FLOW_RANGE), 0.);
    }
}
