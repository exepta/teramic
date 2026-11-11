use bevy::prelude::*;
use tilt_ui::{ElementId, HtmlEvent, html_fn};

#[html_fn("minimap_action")]
fn minimap_action(In(event): In<HtmlEvent>, elements: Query<&ElementId>, mut commands: MessageWriter<crate::minimap::MapAction>) {
    if let Ok(id) = elements.get(event.target) {
        commands.write(crate::minimap::MapAction(id.0.clone()));
    }
}
