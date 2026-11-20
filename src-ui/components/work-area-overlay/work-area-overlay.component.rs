use bevy::prelude::*;
use tilt_ui::{HtmlEvent, html_fn};

#[html_fn("hide_work_areas")]
fn hide_work_areas(In(_event): In<HtmlEvent>, mut actions: MessageWriter<crate::WorkAreaUiAction>) {
    actions.write(crate::WorkAreaUiAction::Hide);
}
