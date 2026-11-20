use std::time::{Duration, Instant};

use bevy::prelude::*;

use crate::menu::{MenuState, SessionInfo};

const SESSION_WARMUP: Duration = Duration::from_secs(10);

/// Unsmoothed frame-rate extrema for the current world session, independent of F1.
#[derive(Resource, Default)]
pub(crate) struct SessionFps {
    /// Frames divided by elapsed real time over the last half-second window.
    pub current: Option<f64>,
    pub minimum: Option<f64>,
    pub maximum: Option<f64>,
    session: Option<Instant>,
    joined: Option<Instant>,
    active: bool,
    current_seconds: f64,
    current_frames: u32,
}

impl SessionFps {
    fn sample(&mut self, session: Option<Instant>, active: bool, now: Instant, seconds: f64) {
        if self.session != session || !active {
            *self = Self {
                session,
                ..default()
            };
        }
        if !self.active && active {
            self.joined = Some(session.unwrap_or(now));
        }
        // SessionInfo starts after the loading reveal. Use real time so pause,
        // time scaling and long frames cannot shorten or extend the warmup.
        // A crossing frame still contains warmup work, so gate its start.
        let frame_started = Duration::try_from_secs_f64(seconds)
            .ok()
            .and_then(|delta| now.checked_sub(delta));
        let record = self.active
            && active
            && self
                .joined
                .zip(frame_started)
                .is_some_and(|(joined, frame_started)| {
                    frame_started.saturating_duration_since(joined) >= SESSION_WARMUP
                });
        if self.active && active && seconds.is_finite() && seconds > 0.0 {
            self.current_seconds += seconds;
            self.current_frames = self.current_frames.saturating_add(1);
            if self.current_seconds >= 0.5 {
                self.current = Some(f64::from(self.current_frames) / self.current_seconds);
                self.current_seconds = 0.0;
                self.current_frames = 0;
            }
        }
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
        time.last_update().unwrap_or_else(Instant::now),
        // Virtual time clamps long frames and may pause; it cannot measure stalls.
        time.delta_secs_f64(),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_rate_uses_rendered_frames_before_the_extrema_warmup() {
        let mut fps = SessionFps::default();
        let joined = Instant::now();
        fps.sample(Some(joined), true, joined, 5.0);
        for frame in 1..=25 {
            fps.sample(
                Some(joined),
                true,
                joined + Duration::from_millis(frame * 20),
                0.02,
            );
        }
        assert!((fps.current.unwrap() - 50.0).abs() < 1e-9);
        assert_eq!((fps.minimum, fps.maximum), (None, None));
    }

    #[test]
    fn current_rate_accounts_for_long_real_frames_and_resets_between_worlds() {
        let mut fps = SessionFps::default();
        let joined = Instant::now();
        fps.sample(Some(joined), true, joined, 5.0);
        for frame in 1..=10 {
            fps.sample(
                Some(joined),
                true,
                joined + Duration::from_millis(frame * 10),
                0.01,
            );
        }
        fps.sample(Some(joined), true, joined + Duration::from_millis(500), 0.4);
        // Ten quick frames and one stall are eleven frames over half a second,
        // rather than the misleading mean of their reciprocal frame times.
        assert!((fps.current.unwrap() - 22.0).abs() < 1e-9);
        let next_world = joined + Duration::from_secs(30);
        fps.sample(Some(next_world), true, next_world, 0.01);
        assert_eq!(fps.current, None);
        fps.sample(None, false, next_world, 0.01);
        assert_eq!(fps.current, None);
    }

    #[test]
    fn waits_ten_real_seconds_after_joining_before_recording() {
        let mut fps = SessionFps::default();
        let joined = Instant::now();
        fps.sample(Some(joined), true, joined, 5.0);
        fps.sample(
            Some(joined),
            true,
            joined + Duration::from_millis(9_900),
            0.5,
        );
        assert_eq!((fps.minimum, fps.maximum), (None, None));
        fps.sample(Some(joined), true, joined + SESSION_WARMUP, 0.02);
        assert_eq!((fps.minimum, fps.maximum), (None, None));
        fps.sample(
            Some(joined),
            true,
            joined + SESSION_WARMUP + Duration::from_millis(20),
            0.02,
        );
        assert_eq!((fps.minimum, fps.maximum), (Some(50.0), Some(50.0)));
    }

    #[test]
    fn excludes_a_stalled_frame_crossing_the_end_of_warmup() {
        let mut fps = SessionFps::default();
        let joined = Instant::now();
        fps.sample(Some(joined), true, joined, 5.0);
        fps.sample(
            Some(joined),
            true,
            joined + Duration::from_millis(9_800),
            0.01,
        );
        fps.sample(Some(joined), true, joined + Duration::from_secs(12), 2.2);
        assert_eq!((fps.minimum, fps.maximum), (None, None));
        fps.sample(
            Some(joined),
            true,
            joined + Duration::from_millis(12_020),
            0.02,
        );
        assert_eq!((fps.minimum, fps.maximum), (Some(50.0), Some(50.0)));
    }

    #[test]
    fn preserves_single_frame_stalls_and_ignores_invalid_samples() {
        let mut fps = SessionFps::default();
        let joined = Instant::now();
        fps.sample(None, true, joined, 3.0);
        assert_eq!(fps.minimum, None);
        for delta in [0.01, 0.5, 0.02, 0.0, f64::NAN, f64::INFINITY, -1.0] {
            fps.sample(None, true, joined + Duration::from_secs(20), delta);
        }
        assert_eq!(fps.minimum, Some(2.0));
        assert_eq!(fps.maximum, Some(100.0));
    }

    #[test]
    fn samples_real_time_with_debug_hidden_and_keeps_stats_in_inventory_and_pause() {
        let mut app = App::new();
        let joined = Instant::now();
        let mut clock = Time::<Real>::new(joined);
        clock.update_with_instant(joined);
        app.insert_resource(clock)
            .init_resource::<Time<Virtual>>()
            .init_resource::<terra_core::DebugSettings>()
            .init_resource::<crate::inventory::InventoryState>()
            .insert_resource(SessionInfo {
                started: Some(joined),
                ..default()
            })
            .insert_resource(MenuState {
                page: crate::menu::MenuPage::Playing,
                session_active: true,
                ..default()
            });
        install(&mut app);
        app.update();
        for millis in [9_990, 10_000, 10_010, 10_510, 10_530] {
            app.world_mut()
                .resource_mut::<Time<Real>>()
                .update_with_instant(joined + Duration::from_millis(millis));
            app.update();
        }
        app.world_mut().resource_mut::<Time<Virtual>>().pause();
        app.world_mut()
            .resource_mut::<crate::inventory::InventoryState>()
            .open = true;
        for page in [
            crate::menu::MenuPage::Playing,
            crate::menu::MenuPage::Pause,
            crate::menu::MenuPage::Options,
        ] {
            app.world_mut().resource_mut::<MenuState>().page = page;
            app.update();
        }
        let fps = app.world().resource::<SessionFps>();
        assert_eq!(fps.minimum, Some(2.0));
        assert_eq!(fps.maximum, Some(100.0));
        assert!(!app.world().resource::<terra_core::DebugSettings>().enabled);
    }

    #[test]
    fn resets_on_world_exit_or_a_different_session_not_on_display_refresh() {
        let mut fps = SessionFps::default();
        let first = Instant::now();
        fps.sample(Some(first), true, first, 0.01);
        fps.sample(
            Some(first),
            true,
            first + SESSION_WARMUP + Duration::from_millis(10),
            0.01,
        );
        assert_eq!((fps.minimum, fps.maximum), (Some(100.0), Some(100.0)));
        fps.sample(None, false, first + Duration::from_secs(11), 1.0);
        assert_eq!((fps.minimum, fps.maximum), (None, None));
        let second = first + Duration::from_secs(20);
        fps.sample(Some(second), true, second, 1.0);
        fps.sample(Some(second), true, second + Duration::from_secs(9), 0.5);
        assert_eq!((fps.minimum, fps.maximum), (None, None));
        fps.sample(
            Some(second),
            true,
            second + SESSION_WARMUP + Duration::from_millis(20),
            0.02,
        );
        assert_eq!(fps.minimum, Some(50.0));
        let third = second + Duration::from_secs(20);
        fps.sample(Some(third), true, third, 0.5);
        assert_eq!((fps.minimum, fps.maximum), (None, None));
    }

    #[test]
    fn loading_and_main_menu_never_contribute_samples() {
        let mut fps = SessionFps::default();
        let joined = Instant::now();
        for page in [crate::menu::MenuPage::Loading, crate::menu::MenuPage::Main] {
            let menu = MenuState {
                page,
                session_active: true,
                ..default()
            };
            fps.sample(
                Some(joined),
                menu.world_hud_active(),
                joined + Duration::from_secs(30),
                2.0,
            );
            assert_eq!((fps.minimum, fps.maximum), (None, None));
        }
    }
}
