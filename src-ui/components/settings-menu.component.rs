use crate::SettingsAction;
use bevy::prelude::*;
use tilt_ui::{ElementId, HtmlEvent, html_fn, html_shared};

#[html_shared]
#[derive(Resource, Default, PartialEq, serde::Serialize)]
pub struct SettingsView {
    pub open: bool,
    pub overlay: bool,
    pub fov: f32,
    pub fov_label: String,
    pub distance: u8,
    pub distance_label: String,
    pub sensitivity: f32,
    pub sensitivity_label: String,
    pub vsync: bool,
    pub debug: bool,
    pub reflections: bool,
    pub waves: f32,
    pub waves_label: String,
    pub quality: String,
    pub weather_quality: String,
    pub mode: String,
    pub status: String,
    pub surface_available: bool,
    pub surface_label: String,
}

#[html_fn("settings_change")]
fn settings_change(
    In(event): In<HtmlEvent>,
    controls: Query<&ElementId>,
    mut actions: MessageWriter<SettingsAction>,
) {
    if let (Ok(id), Some(value)) = (controls.get(event.target), event.value) {
        actions.write(SettingsAction::Change {
            control: id.0.clone(),
            value,
        });
    }
}

#[html_fn("settings_command")]
fn settings_command(
    In(event): In<HtmlEvent>,
    controls: Query<&ElementId>,
    mut actions: MessageWriter<SettingsAction>,
) {
    let Ok(id) = controls.get(event.target) else {
        return;
    };
    let action = match id.0.as_str() {
        "settings-cancel" => SettingsAction::Cancel,
        "settings-resume" => SettingsAction::Resume,
        "settings-apply" => SettingsAction::Apply,
        "settings-save" => SettingsAction::Save,
        "settings-reset" => SettingsAction::Reset,
        "settings-wet" => SettingsAction::SurfaceWet,
        "settings-charred" => SettingsAction::SurfaceCharred,
        "settings-surface-reset" => SettingsAction::SurfaceReset,
        _ => return,
    };
    actions.write(action);
}
