use bevy::prelude::*;
use tilt_ui::{ElementId,HtmlEvent,UiBindingStore,UiStore,html_fn};

#[derive(Default,serde::Serialize,serde::Deserialize,UiStore)]
#[ui_store(mutable)]
pub struct BuildModeIntent { pub template: String, pub category: String }

#[html_fn("build_mode_action")]
fn build_mode_action(In(event):In<HtmlEvent>,ids:Query<&ElementId>,parents:Query<&ChildOf>,store:Res<UiBindingStore>,mut actions:MessageWriter<crate::BuildModeUiAction>) {
    let mut current = Some(event.target);
    for _ in 0..20 {
        let Some(entity) = current else {return;};
        if let Ok(id)=ids.get(entity) {
            let intent=store.get_store::<BuildModeIntent>();
            let action=match id.0.as_str() {
                "build-mode-exit"=>Some(crate::BuildModeUiAction::Exit),
                "build-mode-catalog"=>Some(crate::BuildModeUiAction::Catalog),
                "build-mode-rotate"=>Some(crate::BuildModeUiAction::Rotate),
                "build-mode-place"=>Some(crate::BuildModeUiAction::Confirm),
                "build-mode-refresh"=>Some(crate::BuildModeUiAction::Reload),
                "build-mode-template"=>intent.as_ref().map(|i|crate::BuildModeUiAction::Template(i.template.clone())),
                "build-mode-from-keep"=>intent.as_ref().map(|i|crate::BuildModeUiAction::FromKeep(i.template.clone())),
                "build-mode-category"=>intent.as_ref().map(|i|crate::BuildModeUiAction::Category(i.category.clone())),
                _=>None,
            };
            if let Some(action)=action {actions.write(action);return;}
        }
        current=parents.get(entity).ok().map(ChildOf::parent);
    }
}
