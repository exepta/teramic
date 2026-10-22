use bevy::prelude::*;
use terra_network::client::ClientSession;
use tilt_ui::html_shared;

#[html_shared]
#[derive(Resource, Default, serde::Serialize, PartialEq)]
pub struct SleepView {
    pub visible: bool,
    pub opacity: f32,
    pub style: String,
}

pub(crate) fn install(app: &mut App) {
    app.init_resource::<SleepView>().add_systems(Update, update);
}

fn update(
    time: Res<Time<Real>>,
    session: Option<Res<ClientSession>>,
    mut progress: Local<f32>,
    mut view: ResMut<SleepView>,
) {
    let sleeping = session
        .as_deref()
        .is_some_and(|s| s.ready && !s.dead() && s.sleeping().is_some());
    let target = if sleeping { 1.0 } else { 0.0 };
    let duration = if sleeping { 1.8 } else { 0.4 };
    let step = time.delta_secs() / duration;
    *progress += (target - *progress).clamp(-step, step);
    let eased = *progress * *progress * (3.0 - 2.0 * *progress);
    let opacity = (eased * 80.0).round() / 100.0;
    let next = SleepView {
        visible: opacity > 0.0,
        opacity,
        style: format!(
            "background-color: rgba(3, 5, 8, {opacity:.2}); animated-filter: old-film({:.2}, 0.35);",
            eased * 0.65
        ),
    };
    if *view != next {
        *view = next;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_network::protocol::session::SleepingPlayer;

    #[test]
    fn sleep_fades_in_to_eighty_percent_and_cleans_up_on_waking() {
        let mut app = App::new();
        app.init_resource::<Time<Real>>()
            .init_resource::<ClientSession>()
            .init_resource::<SleepView>()
            .add_systems(Update, update);
        {
            let mut session = app.world_mut().resource_mut::<ClientSession>();
            session.ready = true;
            session.player = Some(terra_core::PlayerId(1));
            session.sleepers.push(SleepingPlayer {
                player: terra_core::PlayerId(1),
                anchor: [0; 3],
                facing: 0,
            });
        }
        let mut before = 0.0;
        for _ in 0..20 {
            app.world_mut()
                .resource_mut::<Time<Real>>()
                .advance_by(std::time::Duration::from_millis(100));
            app.update();
            let view = app.world().resource::<SleepView>();
            assert!(view.opacity >= before && view.opacity <= 0.8);
            assert!(view.opacity - before < 0.08);
            before = view.opacity;
        }
        assert_eq!(before, 0.8);
        let css = format!(".sleep {{ {} }}", app.world().resource::<SleepView>().style);
        tilt_ui::tilt_ui_css::parse_stylesheet(&css).unwrap();
        app.world_mut()
            .resource_mut::<ClientSession>()
            .sleepers
            .clear();
        for _ in 0..5 {
            app.world_mut()
                .resource_mut::<Time<Real>>()
                .advance_by(std::time::Duration::from_millis(100));
            app.update();
        }
        assert!(!app.world().resource::<SleepView>().visible);
        assert_eq!(app.world().resource::<SleepView>().opacity, 0.0);
    }
}
