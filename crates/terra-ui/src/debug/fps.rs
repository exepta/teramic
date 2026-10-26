use std::time::Instant;

use bevy::prelude::*;

use crate::menu::{MenuState, SessionInfo};

/// Unsmoothed frame-rate extrema for the current world session, independent of F1.
#[derive(Resource, Default)]
pub(crate) struct SessionFps {
    pub minimum: Option<f64>,
    pub maximum: Option<f64>,
    session: Option<Instant>,
    active: bool,
}

impl SessionFps {
    fn sample(&mut self, session: Option<Instant>, active: bool, seconds: f64) {
        if self.session != session || !active {
            *self = Self {
                session,
                ..default()
            };
        }
        // The joining frame belongs to loading, not to the playable session.
        let record = self.active && active;
        self.active = active;
        if record && seconds.is_finite() && seconds > 0.0 {
            let fps = seconds.recip();
            if fps.is_finite() {
                self.minimum = Some(self.minimum.map_or(fps, |old| old.min(fps)));
                self.maximum = Some(self.maximum.map_or(fps, |old| old.max(fps)));
            }
        }
    }
}

pub(super) fn install(app: &mut App) {
    app.init_resource::<SessionFps>()
        .add_systems(First, sample.after(bevy::time::TimeSystems));
}

fn sample(
    time: Res<Time<Real>>,
    menu: Option<Res<MenuState>>,
    session: Option<Res<SessionInfo>>,
    mut fps: ResMut<SessionFps>,
) {
    fps.sample(
        session.and_then(|session| session.started),
        menu.is_none_or(|menu| menu.world_hud_active()),
        // Virtual time clamps long frames and may pause; it cannot measure stalls.
        time.delta_secs_f64(),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn preserves_single_frame_stalls_and_ignores_invalid_samples() {
        let mut fps = SessionFps::default();
        fps.sample(None, true, 3.0);
        assert_eq!(fps.minimum, None);
        for delta in [0.01, 0.5, 0.02, 0.0, f64::NAN, f64::INFINITY, -1.0] {
            fps.sample(None, true, delta);
        }
        assert_eq!(fps.minimum, Some(2.0));
        assert_eq!(fps.maximum, Some(100.0));
    }

    #[test]
    fn samples_real_time_with_debug_hidden_and_keeps_stats_in_inventory_and_pause() {
        let mut app = App::new();
        app.init_resource::<Time<Real>>()
            .init_resource::<terra_core::DebugSettings>()
            .insert_resource(MenuState {
                page: crate::menu::MenuPage::Playing,
                session_active: true,
                ..default()
            });
        install(&mut app);
        for millis in [16, 10, 500, 20] {
            app.world_mut()
                .resource_mut::<Time<Real>>()
                .advance_by(Duration::from_millis(millis));
            app.update();
        }
        app.world_mut().resource_mut::<MenuState>().page = crate::menu::MenuPage::Pause;
        app.update();
        let fps = app.world().resource::<SessionFps>();
        assert_eq!(fps.minimum, Some(2.0));
        assert_eq!(fps.maximum, Some(100.0));
        assert!(!app.world().resource::<terra_core::DebugSettings>().enabled);
    }

    #[test]
    fn resets_on_world_exit_or_a_different_session_not_on_display_refresh() {
        let mut fps = SessionFps::default();
        let first = Some(Instant::now());
        fps.sample(first, true, 0.01);
        fps.sample(first, true, 0.01);
        fps.sample(first, false, 1.0);
        assert_eq!((fps.minimum, fps.maximum), (None, None));
        fps.sample(first, true, 1.0);
        fps.sample(first, true, 0.02);
        assert_eq!(fps.minimum, Some(50.0));
        fps.sample(first.map(|time| time + Duration::from_secs(1)), true, 0.5);
        assert_eq!((fps.minimum, fps.maximum), (None, None));
    }
}
