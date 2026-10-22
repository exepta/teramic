use bevy::{
    prelude::*,
    sprite::{BorderRect, TextureSlicer},
    ui::widget::{ImageNode, NodeImageMode},
};
use tilt_ui::ElementClasses;

pub(crate) fn install(app: &mut App) {
    app.add_systems(PostUpdate, nine_slice.before(bevy::ui::UiSystems::Prepare));
}

fn nine_slice(mut images: Query<(&ElementClasses, &ComputedNode, &mut ImageNode)>) {
    for (classes, node, mut image) in &mut images {
        if !classes
            .classes
            .iter()
            .any(|class| class == "ornate-frame-art")
        {
            continue;
        }
        // Compact browsers have only 6-10 px of inset. Keep the ornament
        // inside that space without changing inventory slot dimensions.
        let corner_scale = if node.size().x > 0.0 {
            (node.size().x / 4000.0).clamp(0.05, 0.10)
        } else {
            0.10
        };
        let mode = NodeImageMode::Sliced(TextureSlicer {
            border: BorderRect::all(196.0),
            max_corner_scale: corner_scale,
            ..default()
        });
        if image.image_mode != mode {
            image.image_mode = mode;
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn frame_styles_use_supported_tilt_ui_properties() {
        tilt_ui::tilt_ui_css::parse_stylesheet(include_str!(
            "../../../src-ui/components/ornate-frame.component.css"
        ))
        .unwrap();
    }
}
