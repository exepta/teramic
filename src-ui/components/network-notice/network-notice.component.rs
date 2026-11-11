use bevy::prelude::*;
use terra_network::{
    client::{ClientSession, ReconnectRequest},
    protocol::session::EditOutcome,
};
use tilt_ui::{ElementId, HtmlEvent, ToastKind, html_fn, html_shared};

#[html_shared]
#[derive(Resource, Default, PartialEq, serde::Serialize)]
pub struct NetworkNotice {
    pub visible: bool,
    pub retry: bool,
    pub message: String,
    pub warning: String,
    pub error: String,
}

#[derive(Resource, Default)]
pub struct PendingNotices {
    warning: bool,
    error: bool,
}

pub fn update_notice(
    session: Option<Res<ClientSession>>,
    mut notice: ResMut<NetworkNotice>,
    mut last_edit: Local<u64>,
    mut pending: ResMut<PendingNotices>,
) {
    let Some(session) = session else {
        *notice = NetworkNotice::default();
        *last_edit = 0;
        *pending = PendingNotices::default();
        return;
    };
    if let Some((id, outcome)) = session.last_edit
        && id != *last_edit
    {
        *last_edit = id;
        let reason = match outcome {
            EditOutcome::Applied => None,
            EditOutcome::Conflict => Some("Weltzustand hat sich bereits geaendert."),
            EditOutcome::OutOfReach => Some("Ziel ist ausser Reichweite."),
            EditOutcome::Occluded => Some("Ziel ist verdeckt."),
            EditOutcome::Invalid => Some("Diese Aktion ist nicht erlaubt."),
            EditOutcome::RateLimited => Some("Zu viele Aktionen. Bitte kurz warten."),
            EditOutcome::Planting(reason) => Some(reason.message()),
        };
        if let Some(reason) = reason {
            if matches!(outcome, EditOutcome::Conflict | EditOutcome::RateLimited) {
                notice.warning = reason.into();
                pending.warning = true;
            } else {
                notice.error = reason.into();
                pending.error = true;
            }
        }
    }
    let (visible, message) = if let Some(error) = &session.error {
        (true, format!("Verbindung unterbrochen: {error}"))
    } else {
        (false, String::new())
    };
    if notice.visible != visible || notice.retry != visible || notice.message != message {
        notice.visible = visible;
        notice.retry = visible;
        notice.message = message;
    }
}

pub fn present_toasts(world: &mut World) {
    let pending = world.resource::<PendingNotices>();
    let (warning, error) = (pending.warning, pending.error);
    if !warning && !error {
        return;
    }
    let nodes = world
        .query::<(
            Entity,
            &ElementId,
            &tilt_ui::ToastState,
            &tilt_ui::CascadedStyle,
        )>()
        .iter(world)
        .filter_map(|(entity, id, state, style)| {
            let wanted = match id.0.as_str() {
                "network-warning-toast" => warning,
                "network-error-toast" => error,
                _ => false,
            };
            (wanted && style.0.background_color.is_some()).then_some((entity, state.kind))
        })
        .collect::<Vec<_>>();
    for (entity, kind) in nodes {
        // Authored widgets are styled before TiltUI portals them. Reuse them
        // instead of dynamically creating parts that are materialized twice.
        tilt_ui::close_toast(world, entity);
        if tilt_ui::show_toast(world, entity) {
            let mut pending = world.resource_mut::<PendingNotices>();
            if kind == ToastKind::Warning {
                pending.warning = false;
            } else {
                pending.error = false;
            }
        }
    }
}

#[html_fn("network_retry")]
fn network_retry(In(_event): In<HtmlEvent>, mut retry: MessageWriter<ReconnectRequest>) {
    retry.write(ReconnectRequest);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn successful_join_and_streaming_do_not_overlay_the_game_but_errors_do() {
        let mut app = App::new();
        app.init_resource::<ClientSession>()
            .init_resource::<NetworkNotice>()
            .init_resource::<PendingNotices>()
            .add_systems(Update, update_notice);
        app.update();
        assert!(!app.world().resource::<NetworkNotice>().visible);
        app.world_mut().resource_mut::<ClientSession>().ready = true;
        app.update();
        assert!(!app.world().resource::<NetworkNotice>().visible);
        app.world_mut().resource_mut::<ClientSession>().error = Some("Connection lost".into());
        app.update();
        assert!(app.world().resource::<NetworkNotice>().retry);
        app.world_mut().remove_resource::<ClientSession>();
        app.update();
        assert!(!app.world().resource::<NetworkNotice>().visible);
    }

    #[test]
    fn rejected_edits_use_semantic_toasts_once_instead_of_the_center_notice() {
        let mut app = App::new();
        app.init_resource::<ClientSession>()
            .init_resource::<NetworkNotice>()
            .init_resource::<PendingNotices>()
            .add_systems(Update, update_notice);
        for (id, outcome, kind) in [
            (1, EditOutcome::Conflict, tilt_ui::ToastKind::Warning),
            (2, EditOutcome::Invalid, tilt_ui::ToastKind::Error),
        ] {
            app.world_mut().resource_mut::<ClientSession>().last_edit = Some((id, outcome));
            app.update();
            assert!(!app.world().resource::<NetworkNotice>().visible);
            let pending = app.world().resource::<PendingNotices>();
            assert_eq!(pending.warning, kind == ToastKind::Warning);
            assert_eq!(pending.error, kind == ToastKind::Error);
            *app.world_mut().resource_mut::<PendingNotices>() = PendingNotices::default();
            app.update();
            let pending = app.world().resource::<PendingNotices>();
            assert!(!pending.warning && !pending.error);
        }
    }

    #[test]
    fn sapling_rejections_show_the_actual_reason_instead_of_a_generic_permission_error() {
        let mut app = App::new();
        app.init_resource::<ClientSession>()
            .init_resource::<NetworkNotice>()
            .init_resource::<PendingNotices>()
            .add_systems(Update, update_notice);
        for (index, reason) in [
            terra_core::trees::SaplingPlacementError::Soil,
            terra_core::trees::SaplingPlacementError::Water,
            terra_core::trees::SaplingPlacementError::Occupied,
            terra_core::trees::SaplingPlacementError::Protected,
            terra_core::trees::SaplingPlacementError::Unloaded,
        ]
        .into_iter()
        .enumerate()
        {
            app.world_mut().resource_mut::<ClientSession>().last_edit =
                Some((index as u64 + 1, EditOutcome::Planting(reason)));
            app.update();
            assert_eq!(
                app.world().resource::<NetworkNotice>().error,
                reason.message()
            );
            assert!(app.world().resource::<PendingNotices>().error);
        }
    }
}
