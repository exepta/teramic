//! Limit retained UI presentation work, never the world's render or simulation rate.
use bevy::prelude::*;
use tilt_ui::UiFrameRate;

#[derive(Resource)]
pub(crate) struct Cadence {
    next: f64,
    due: bool,
    fps: u32,
}

pub(crate) fn install(app: &mut App) {
    // TiltUI's with_ui_fps installs a blocking sleep for the entire application.
    // Keep its public rate resource, but schedule presentation updates independently.
    app.insert_resource(UiFrameRate::Fps60)
        .insert_resource(Cadence {
            next: 0.0,
            due: true,
            fps: 60,
        })
        .add_systems(First, tick);
}

fn tick(
    time: Res<Time<Real>>,
    graphics: Option<Res<terra_core::GraphicsConfig>>,
    mut rate: ResMut<UiFrameRate>,
    mut cadence: ResMut<Cadence>,
) {
    let fps = u32::from(graphics.map_or(60, |g| g.ui_fps)).clamp(24, 240);
    let now = time.elapsed_secs_f64();
    let changed = fps != cadence.fps;
    cadence.due = now >= cadence.next || changed;
    if cadence.due {
        let interval = 1.0 / f64::from(fps);
        cadence.next = if changed {
            now + interval
        } else {
            let elapsed_intervals = ((now - cadence.next) / interval).floor().max(0.0) + 1.0;
            cadence.next + elapsed_intervals * interval
        };
        cadence.fps = fps;
    }
    if rate.fps() != fps
        && let Some(next) = UiFrameRate::custom(fps)
    {
        *rate = next;
    }
}

pub(crate) fn ready(cadence: Option<Res<Cadence>>) -> bool {
    cadence.is_none_or(|c| c.due)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Resource, Default)]
    struct Counts(u32, u32);
    #[test]
    fn ui_rate_limits_presentation_without_limiting_game_updates() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
                std::time::Duration::from_millis(5),
            ))
            .init_resource::<terra_core::GraphicsConfig>()
            .init_resource::<Counts>();
        install(&mut app);
        app.add_systems(Update, (|mut c: ResMut<Counts>| c.0 += 1).run_if(ready));
        app.add_systems(Update, |mut c: ResMut<Counts>| c.1 += 1);
        for _ in 0..200 {
            app.update();
        }
        let counts = app.world().resource::<Counts>();
        assert_eq!(counts.1, 200);
        assert!((58..=62).contains(&counts.0));
        let before = counts.0;
        app.world_mut()
            .resource_mut::<terra_core::GraphicsConfig>()
            .ui_fps = 30;
        for _ in 0..200 {
            app.update();
        }
        assert_eq!(app.world().resource::<UiFrameRate>().fps(), 30);
        assert_eq!(app.world().resource::<Counts>().1, 400);
        assert!((29..=31).contains(&(app.world().resource::<Counts>().0 - before)));
    }
}
