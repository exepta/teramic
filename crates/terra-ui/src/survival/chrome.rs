//! Render native TiltUI meter values inside the artwork's chamfered channels.
use bevy::{
    prelude::*, reflect::TypePath, render::render_resource::AsBindGroup, shader::ShaderRef,
};

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone, PartialEq)]
struct VitalMaterial {
    #[uniform(0)]
    values: Vec4,
    #[uniform(0)]
    effects: Vec4,
    #[uniform(0)]
    timing: Vec4,
    #[texture(1)]
    #[sampler(2)]
    chrome: Handle<Image>,
}

impl UiMaterial for VitalMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/terra_vital_ui.wgsl".into()
    }
}

#[derive(Resource, Default)]
struct Chrome(Handle<VitalMaterial>);

#[derive(Resource, Default, Debug)]
pub struct VitalRenderStatus {
    pub health: f32,
    pub stamina: f32,
    pub heartbeat_period: Option<f32>,
    pub heartbeat_scale: f32,
    pub bound: bool,
    pub poison: bool,
    pub freeze: bool,
    pub regeneration: bool,
}

struct ChromePlugin;

pub(super) fn install(app: &mut App) {
    app.add_plugins(ChromePlugin);
}

impl Plugin for ChromePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Chrome>()
            .init_resource::<VitalRenderStatus>()
            .add_systems(PostUpdate, bind.before(bevy::ui::UiSystems::Prepare));
    }

    fn finish(&self, app: &mut App) {
        app.add_plugins(UiMaterialPlugin::<VitalMaterial>::default());
    }
}

fn heartbeat_period(health: f32, enabled: bool) -> Option<f32> {
    if !enabled || health <= 0.0 || health >= 60.0 {
        None
    } else if health < 10.0 {
        Some(0.36)
    } else if health < 30.0 {
        Some(0.62)
    } else {
        Some(1.2)
    }
}

fn heartbeat_scale(seconds: f32, period: Option<f32>) -> f32 {
    let Some(period) = period else { return 1.0 };
    let phase = (seconds / period).fract();
    let bump = |at: f32, width: f32| {
        let x = ((phase - at) / width).clamp(-1.0, 1.0);
        (1.0 - x * x).powi(2)
    };
    1.0 + bump(0.15, 0.12) * 0.055 + bump(0.39, 0.09) * 0.027
}

fn animated_fraction(parts: &tilt_ui::NumericParts, nodes: &Query<&Node>) -> Option<f32> {
    match nodes.get(parts.fill).ok()?.width {
        Val::Percent(value) => Some((value / 100.0).clamp(0.0, 1.0)),
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)]
fn bind(
    mut commands: Commands,
    view: Res<super::SurvivalView>,
    gameplay: Option<Res<terra_core::GameplayConfig>>,
    time: Res<Time>,
    session: Option<Res<terra_network::client::ClientSession>>,
    meters: Query<(&tilt_ui::ElementId, &tilt_ui::NumericParts)>,
    nodes: Query<&Node>,
    mut images: Query<(
        Entity,
        &tilt_ui::ElementId,
        &mut ImageNode,
        Option<&MaterialNode<VitalMaterial>>,
    )>,
    mut state: ResMut<Chrome>,
    mut materials: ResMut<Assets<VitalMaterial>>,
    mut status: ResMut<VitalRenderStatus>,
) {
    if !view.visible {
        *status = VitalRenderStatus::default();
        return;
    }
    let fraction = |name: &str, fallback: f32| {
        meters
            .iter()
            .find(|(id, _)| id.0 == name)
            .and_then(|(_, parts)| animated_fraction(parts, &nodes))
            .unwrap_or(fallback / 100.0)
    };
    let period = heartbeat_period(
        view.health,
        !view.editing && gameplay.as_ref().is_none_or(|g| g.heartbeat),
    );
    let values = Vec4::new(
        fraction("survival-health", view.health),
        if view.show_oxygen {
            fraction("survival-oxygen", view.oxygen)
        } else {
            fraction("survival-stamina", view.stamina)
        },
        heartbeat_scale(time.elapsed_secs(), period),
        if view.show_oxygen { 1.0 } else { 0.0 },
    );
    let vitals = session
        .as_ref()
        .map_or_else(terra_core::SurvivalVitals::default, |s| s.stats.survival);
    let active = |kind| !view.editing && !view.dead && vitals.effects.contains(kind);
    let flag = |value| if value { 1.0 } else { 0.0 };
    let effects = Vec4::new(
        flag(active(terra_core::EffectKind::Poison)),
        flag(active(terra_core::EffectKind::Freeze)),
        flag(active(terra_core::EffectKind::OnFire)),
        flag(active(terra_core::EffectKind::Bleeding)),
    );
    let regeneration = !view.editing
        && !view.dead
        && vitals
            .active_effects()
            .any(|e| e.kind == terra_core::EffectKind::Regeneration);
    let timing = Vec4::new(
        if effects.x > 0.0 || effects.z > 0.0 {
            time.elapsed_secs()
        } else {
            0.0
        },
        flag(regeneration),
        flag(active(terra_core::EffectKind::Rotten)),
        0.0,
    );
    for (entity, id, mut image, material) in &mut images {
        if id.0 != "survival-chrome" {
            continue;
        }
        let value = VitalMaterial {
            values,
            effects,
            timing,
            chrome: image.image.clone(),
        };
        if let Some(mut current) = materials.get_mut(&state.0) {
            if *current != value {
                *current = value;
            }
        } else {
            state.0 = materials.add(value);
        }
        if image.color != Color::NONE {
            image.color = Color::NONE;
        }
        if material.is_none_or(|m| m.0 != state.0) {
            commands
                .entity(entity)
                .insert(MaterialNode(state.0.clone()));
        }
        *status = VitalRenderStatus {
            health: values.x,
            stamina: values.y,
            heartbeat_period: period,
            heartbeat_scale: values.z,
            bound: true,
            poison: effects.x > 0.0,
            freeze: effects.y > 0.0,
            regeneration,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heartbeat_thresholds_are_strict_and_can_be_disabled() {
        assert_eq!(heartbeat_period(100.0, true), None);
        assert_eq!(heartbeat_period(60.0, true), None);
        assert_eq!(heartbeat_period(59.99, true), Some(1.2));
        assert_eq!(heartbeat_period(30.0, true), Some(1.2));
        assert_eq!(heartbeat_period(29.99, true), Some(0.62));
        assert_eq!(heartbeat_period(10.0, true), Some(0.62));
        assert_eq!(heartbeat_period(9.99, true), Some(0.36));
        assert_eq!(heartbeat_period(0.0, true), None);
        for health in [59.0, 29.0, 9.0] {
            assert_eq!(heartbeat_period(health, false), None);
        }
    }

    #[test]
    fn double_beat_rests_between_beats_without_shrinking_the_art() {
        assert_eq!(heartbeat_scale(0.0, None), 1.0);
        assert_eq!(heartbeat_scale(0.9, Some(1.0)), 1.0);
        assert!(heartbeat_scale(0.15, Some(1.0)) > heartbeat_scale(0.39, Some(1.0)));
        for step in 0..1000 {
            assert!((1.0..=1.056).contains(&heartbeat_scale(step as f32 / 1000.0, Some(1.0))));
        }
    }
}
