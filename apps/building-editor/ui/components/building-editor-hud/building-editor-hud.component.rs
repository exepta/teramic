use bevy::prelude::*;
use tilt_ui::{ElementId,HtmlEvent,UiBindingStore,UiStore,html_fn};
#[derive(Default,serde::Serialize,serde::Deserialize,UiStore)]
#[ui_store(mutable)]
pub struct EditorIntent{pub value:String}
#[html_fn("editor_command")]
fn editor_command(In(event):In<HtmlEvent>,elements:Query<&ElementId>,parents:Query<&ChildOf>,store:Res<UiBindingStore>,mut actions:MessageWriter<crate::editor::EditorAction>){
 let mut target=Some(event.target);
 for _ in 0..32 {let Some(entity)=target else{return;};if let Ok(id)=elements.get(entity)&&let Some(command)=id.0.strip_prefix("editor-"){actions.write(crate::editor::EditorAction::Command{command:command.into(),value:store.get_store::<EditorIntent>().map_or_else(String::new,|intent|intent.value.clone())});return;}target=parents.get(entity).ok().map(ChildOf::parent);}
}
#[html_fn("editor_change")]
fn editor_change(In(event):In<HtmlEvent>,elements:Query<&ElementId>,mut actions:MessageWriter<crate::editor::EditorAction>){
 if let (Ok(id),Some(value))=(elements.get(event.target),event.value)&&let Some(control)=id.0.strip_prefix("editor-"){actions.write(crate::editor::EditorAction::Change{control:control.into(),value});}
}
