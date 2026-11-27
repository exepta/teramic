//! The atmosphere renders its live cubemap every frame. Its diffuse and specular
//! convolutions can reuse the previous textures between changes in sky lighting.
use super::daylight::Daylight;
use bevy::{
    light::AtmosphereEnvironmentMapLight,
    pbr::generate::{
        GeneratorBindGroups, RenderEnvironmentMap, extract_generated_environment_map_entities,
    },
    prelude::*,
    render::{Extract, ExtractSchedule, RenderApp, sync_world::RenderEntity},
};
use std::time::{Duration, Instant};
use terra_player::TerraCamera;

pub(super) fn install(app: &mut App) {
    if std::env::args().any(|arg| arg.starts_with("--smoke-test"))
        && std::env::var("TERRA_SKY_FILTER_EVERY_FRAME").is_ok_and(|value| value == "1")
    {
        return;
    }
    if let Some(render) = app.get_sub_app_mut(RenderApp) {
        render.add_systems(
            ExtractSchedule,
            reuse_filtered_sky.after(extract_generated_environment_map_entities),
        );
    }
}

#[derive(PartialEq)]
struct Targets(Entity, AssetId<Image>, AssetId<Image>);

#[derive(Clone, Copy)]
struct SkySample {
    position: Vec3,
    sun: Vec3,
}

impl SkySample {
    fn changed(self, previous: Self) -> bool {
        self.position.distance_squared(previous.position) >= 32.0 * 32.0
            || self.sun.distance_squared(previous.sun) >= 0.004_f32.powi(2)
    }
}

struct LastFilter {
    targets: Targets,
    inputs: SkySample,
    time: Instant,
    follow_up: bool,
}

type SkyCameras<'w, 's> = Query<
    'w,
    's,
    (
        RenderEntity,
        &'static GlobalTransform,
        &'static EnvironmentMapLight,
    ),
    (With<TerraCamera>, With<AtmosphereEnvironmentMapLight>),
>;

fn reuse_filtered_sky(
    mut commands: Commands,
    cameras: Extract<SkyCameras>,
    daylight: Extract<Res<Daylight>>,
    prepared: Query<(), With<GeneratorBindGroups>>,
    mut previous: Local<Option<LastFilter>>,
) {
    let Ok((entity, transform, map)) = cameras.single() else {
        *previous = None;
        return;
    };
    if !prepared.contains(entity) {
        *previous = None;
        return;
    }

    let inputs = SkySample {
        position: transform.translation(),
        sun: daylight.sun,
    };
    let targets = Targets(entity, map.diffuse_map.id(), map.specular_map.id());
    let now = Instant::now();
    let changed = previous
        .as_ref()
        .is_none_or(|last| last.targets != targets || inputs.changed(last.inputs));
    if changed {
        // Bevy filters the source written by the preceding sky pass. Keep the
        // next frame too so a teleport or a clock jump receives the new sky.
        *previous = Some(LastFilter {
            targets,
            inputs,
            time: now,
            follow_up: true,
        });
    } else if let Some(last) = previous.as_mut() {
        if last.follow_up || now.duration_since(last.time) >= Duration::from_millis(125) {
            last.inputs = inputs;
            last.time = now;
            last.follow_up = false;
        } else {
            // Only suppress this frame's filtering job. The PBR environment
            // bindings and their GPU images remain owned by EnvironmentMapLight.
            commands.entity(entity).remove::<RenderEnvironmentMap>();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sky_jumps_refresh_before_the_periodic_update() {
        let from = SkySample {
            position: Vec3::ZERO,
            sun: Vec3::Y,
        };
        assert!(
            !SkySample {
                position: Vec3::X,
                ..from
            }
            .changed(from)
        );
        assert!(
            SkySample {
                position: Vec3::Y * 36.0,
                ..from
            }
            .changed(from)
        );
        assert!(
            SkySample {
                sun: Vec3::NEG_Y,
                ..from
            }
            .changed(from)
        );
    }
}
