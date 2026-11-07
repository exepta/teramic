//! Optional whole-schedule timings without replacing or reordering game systems.
use bevy::{app::MainScheduleOrder, ecs::schedule::ScheduleLabel, prelude::*};
use terra_core::profiling::ScopeTimer;

#[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash)]
enum Boundary {
    Start(u8),
    Stop(u8),
}

#[derive(Resource, Default)]
struct Timers([Option<ScopeTimer>; 6]);

#[derive(Resource, Default)]
struct UiReactivityTimer(Option<ScopeTimer>);

#[derive(Resource, Default)]
struct UiStyleTimer(Option<ScopeTimer>);

#[derive(Resource, Default)]
struct UiLayoutTimers([Option<ScopeTimer>; 2]);

const NAMES: [&str; 6] = [
    "main.first",
    "main.pre_update",
    "main.fixed_loop",
    "main.update",
    "main.post_update",
    "main.last",
];

pub(crate) fn install(app: &mut App) {
    if std::env::var("TERRA_CPU_SCOPES").is_ok_and(|value| value == "1") {
        install_enabled(app);
    }
}

fn start<const N: usize>(mut timers: ResMut<Timers>) {
    timers.0[N] = Some(ScopeTimer::new(NAMES[N]));
}

fn stop<const N: usize>(mut timers: ResMut<Timers>) {
    drop(timers.0[N].take());
}

fn start_ui_reactivity(mut timer: ResMut<UiReactivityTimer>) {
    timer.0 = Some(ScopeTimer::new("ui.reactivity"));
}

fn stop_ui_reactivity(mut timer: ResMut<UiReactivityTimer>) {
    drop(timer.0.take());
}

fn start_ui_style(mut timer: ResMut<UiStyleTimer>) {
    timer.0 = Some(ScopeTimer::new("ui.style"));
}

fn stop_ui_style(mut timer: ResMut<UiStyleTimer>) {
    drop(timer.0.take());
}

fn start_ui_layout<const N: usize>(mut timers: ResMut<UiLayoutTimers>) {
    timers.0[N] = Some(ScopeTimer::new(["ui.content", "ui.layout"][N]));
}

fn stop_ui_layout<const N: usize>(mut timers: ResMut<UiLayoutTimers>) {
    drop(timers.0[N].take());
}

fn install_enabled(app: &mut App) {
    app.init_resource::<Timers>()
        .init_resource::<UiReactivityTimer>()
        .init_resource::<UiStyleTimer>()
        .init_resource::<UiLayoutTimers>()
        .add_systems(
            Update,
            (
                start_ui_reactivity.before(tilt_ui::TiltUiComponentRuntimeSet::Reactivity),
                stop_ui_reactivity.after(tilt_ui::TiltUiComponentRuntimeSet::Reactivity),
            )
                .chain(),
        )
        .add_systems(
            Update,
            (
                // Scheduling bounds can include other ready Update systems;
                // this is an interval, not the private style system's body.
                start_ui_style.before(tilt_ui_runtime::style::TiltUiStyleRuntimeSet::Apply),
                stop_ui_style.after(tilt_ui_runtime::style::TiltUiStyleRuntimeSet::Apply),
            )
                .chain(),
        )
        .add_systems(
            PostUpdate,
            (
                start_ui_layout::<0>
                    .after(bevy::ui::UiSystems::Propagate)
                    .before(bevy::ui::UiSystems::Content),
                stop_ui_layout::<0>
                    .after(bevy::ui::UiSystems::Content)
                    .before(bevy::ui::UiSystems::Layout),
                start_ui_layout::<1>
                    .after(bevy::ui::UiSystems::Content)
                    .before(bevy::ui::UiSystems::Layout),
                stop_ui_layout::<1>
                    .after(bevy::ui::UiSystems::Layout)
                    .before(bevy::ui::UiSystems::PostLayout),
            )
                .chain(),
        );
    macro_rules! bracket {
        ($index:literal, $schedule:expr) => {
            app.add_systems(Boundary::Start($index), start::<$index>)
                .add_systems(Boundary::Stop($index), stop::<$index>);
            let mut order = app.world_mut().resource_mut::<MainScheduleOrder>();
            order.insert_before($schedule, Boundary::Start($index));
            order.insert_after($schedule, Boundary::Stop($index));
        };
    }
    bracket!(0, First);
    bracket!(1, PreUpdate);
    bracket!(2, RunFixedMainLoop);
    bracket!(3, Update);
    bracket!(4, PostUpdate);
    bracket!(5, Last);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Resource, Default)]
    struct Visited(Vec<u8>);

    #[test]
    fn instrumentation_brackets_schedules_without_changing_their_order() {
        let mut app = App::new();
        install_enabled(&mut app);
        app.init_resource::<Visited>()
            .add_systems(First, observe::<0>)
            .add_systems(PreUpdate, observe::<1>)
            .add_systems(RunFixedMainLoop, observe::<2>)
            .add_systems(Update, observe::<3>)
            .add_systems(PostUpdate, observe::<4>)
            .add_systems(Last, observe::<5>);
        app.update();
        assert_eq!(app.world().resource::<Visited>().0, [0, 1, 2, 3, 4, 5]);
        assert!(
            app.world()
                .resource::<Timers>()
                .0
                .iter()
                .all(Option::is_none)
        );
    }

    fn observe<const N: usize>(timers: Res<Timers>, mut visited: ResMut<Visited>) {
        assert!(timers.0[N].is_some());
        assert_eq!(timers.0.iter().filter(|timer| timer.is_some()).count(), 1);
        visited.0.push(N as u8);
    }

    #[test]
    fn ui_interval_encloses_existing_reactivity_and_releases_afterward() {
        let mut app = App::new();
        install_enabled(&mut app);
        app.add_systems(
            Update,
            (|timer: Res<UiReactivityTimer>| assert!(timer.0.is_some()))
                .in_set(tilt_ui::TiltUiComponentRuntimeSet::Reactivity),
        );
        for _ in 0..3 {
            app.update();
            assert!(app.world().resource::<UiReactivityTimer>().0.is_none());
        }
    }

    #[test]
    fn style_interval_encloses_application_and_releases_afterward() {
        let mut app = App::new();
        install_enabled(&mut app);
        app.add_systems(
            Update,
            (|timer: Res<UiStyleTimer>| assert!(timer.0.is_some()))
                .in_set(tilt_ui_runtime::style::TiltUiStyleRuntimeSet::Apply),
        );
        for _ in 0..3 {
            app.update();
            assert!(app.world().resource::<UiStyleTimer>().0.is_none());
        }
    }

    #[test]
    fn text_content_and_layout_intervals_are_separate_and_release_afterward() {
        let mut app = App::new();
        install_enabled(&mut app);
        app.add_systems(
            PostUpdate,
            (
                (|timers: Res<UiLayoutTimers>| {
                    assert!(timers.0[0].is_some());
                    assert!(timers.0[1].is_none());
                })
                .in_set(bevy::ui::UiSystems::Content),
                (|timers: Res<UiLayoutTimers>| {
                    assert!(timers.0[0].is_none());
                    assert!(timers.0[1].is_some());
                })
                .in_set(bevy::ui::UiSystems::Layout),
            ),
        );
        for _ in 0..3 {
            app.update();
            assert!(
                app.world()
                    .resource::<UiLayoutTimers>()
                    .0
                    .iter()
                    .all(Option::is_none)
            );
        }
    }
}
