//! Pointer capture for rotating the inventory's shared, live character model.
use bevy::{prelude::*, window::PrimaryWindow};
use tilt_ui::ElementId;

#[derive(Resource, Default, Clone, Copy, PartialEq)]
pub struct InventoryPortraitRotation {
    pub active: bool,
    pub rotation: f32,
}

pub(crate) fn install(app: &mut App) {
    app.init_resource::<InventoryPortraitRotation>()
        .add_systems(Update, drag.after(crate::menu::MenuInputSet))
        .add_systems(PostUpdate, fit.before(bevy::ui::UiSystems::Prepare));
}

#[derive(Default)]
struct Drag {
    previous: Option<f32>,
}
impl Drag {
    fn update(&mut self, pressed: bool, start: bool, hit: bool, x: f32, width: f32) -> f32 {
        if !pressed {
            self.previous = None;
            return 0.0;
        }
        if start {
            self.previous = hit.then_some(x);
            return 0.0;
        }
        let Some(old) = self.previous else {
            return 0.0;
        };
        self.previous = Some(x);
        (x - old) / width.max(1.0) * std::f32::consts::TAU
    }
}

fn drag(
    windows: Query<&Window, With<PrimaryWindow>>,
    mouse: Res<ButtonInput<MouseButton>>,
    view: Res<crate::inventory::InventoryView>,
    crafting: Res<crate::inventory::crafting::CraftingView>,
    nodes: Query<(&ElementId, &ComputedNode, &UiGlobalTransform)>,
    mut rotation: ResMut<InventoryPortraitRotation>,
    mut captured: Local<Drag>,
) {
    let active = view.open && view.player && !crafting.recipe_open;
    if rotation.active != active {
        rotation.active = active;
    }
    if !active {
        captured.previous = None;
        return;
    }
    let window = windows.iter().next();
    let cursor = window.and_then(Window::physical_cursor_position);
    let active = !view.sort_player_open
        && !view.sort_storage_open
        && window.is_some_and(|window| window.focused);
    let node = nodes
        .iter()
        .find(|(id, _, _)| id.0 == "inventory-character-preview");
    let hit = cursor
        .zip(node)
        .is_some_and(|(cursor, (_, node, transform))| {
            transform.try_inverse().is_some_and(|inverse| {
                Rect::from_center_size(Vec2::ZERO, node.size())
                    .contains(inverse.transform_point2(cursor))
            })
        });
    let width = node.map_or(1.0, |(_, node, transform)| {
        (transform.transform_point2(Vec2::X * node.size().x)
            - transform.transform_point2(Vec2::ZERO))
        .length()
    });
    let delta = captured.update(
        active && cursor.is_some() && mouse.pressed(MouseButton::Left),
        mouse.just_pressed(MouseButton::Left),
        hit,
        cursor.map_or(0.0, |cursor| cursor.x),
        width,
    );
    if delta != 0.0 {
        rotation.rotation = (rotation.rotation + delta).rem_euclid(std::f32::consts::TAU);
    }
}

/// Preserve the source aspect ratio when switching from the cached to live portrait.
fn fit(
    images: Res<Assets<Image>>,
    parents: Query<&ComputedNode>,
    mut nodes: Query<(&ElementId, &ChildOf, &ImageNode, &mut Node), Without<tilt_ui::ParkedUi>>,
) {
    for (id, parent, image, mut node) in &mut nodes {
        if id.0 != "inventory-character-preview" {
            continue;
        }
        let (Some(image), Ok(parent)) = (images.get(&image.image), parents.get(parent.parent()))
        else {
            continue;
        };
        let available = parent.size() * parent.inverse_scale_factor();
        let ratio = image.texture_descriptor.size.width as f32
            / image.texture_descriptor.size.height.max(1) as f32;
        let height = available.y.min(available.x / ratio).max(0.0);
        let size = Vec2::new(height * ratio, height);
        let offset = (available - size) * 0.5;
        if node.position_type != PositionType::Absolute
            || node.width != Val::Px(size.x)
            || node.height != Val::Px(size.y)
            || node.left != Val::Px(offset.x)
            || node.top != Val::Px(offset.y)
        {
            node.position_type = PositionType::Absolute;
            node.width = Val::Px(size.x);
            node.height = Val::Px(size.y);
            node.left = Val::Px(offset.x);
            node.top = Val::Px(offset.y);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retained_portrait_only_invalidates_layout_when_its_visible_fit_changes() {
        let mut app = App::new();
        app.init_resource::<Assets<Image>>()
            .add_systems(Update, fit);
        let handle = app
            .world_mut()
            .resource_mut::<Assets<Image>>()
            .add(Image::default());
        let parent = app
            .world_mut()
            .spawn(ComputedNode {
                size: Vec2::new(160.0, 100.0),
                inverse_scale_factor: 1.0,
                ..default()
            })
            .id();
        let portrait = app
            .world_mut()
            .spawn((
                ElementId("inventory-character-preview".into()),
                ImageNode::new(handle),
                ChildOf(parent),
            ))
            .id();
        app.update();
        let node = app.world().entity(portrait).get_ref::<Node>().unwrap();
        assert_eq!(node.width, Val::Px(100.0));
        assert_eq!(node.left, Val::Px(30.0));
        let initial = node.last_changed();

        app.update();
        assert_eq!(
            app.world()
                .entity(portrait)
                .get_ref::<Node>()
                .unwrap()
                .last_changed(),
            initial
        );
        app.world_mut()
            .entity_mut(portrait)
            .insert(tilt_ui::ParkedUi);
        app.world_mut()
            .get_mut::<ComputedNode>(parent)
            .unwrap()
            .size
            .y = 80.0;
        app.update();
        assert_eq!(
            app.world()
                .entity(portrait)
                .get_ref::<Node>()
                .unwrap()
                .last_changed(),
            initial
        );
        app.world_mut()
            .entity_mut(portrait)
            .remove::<tilt_ui::ParkedUi>();
        app.update();
        let node = app.world().entity(portrait).get_ref::<Node>().unwrap();
        assert_ne!(node.last_changed(), initial);
        assert_eq!(node.width, Val::Px(80.0));
        assert_eq!(node.left, Val::Px(40.0));
    }

    #[test]
    fn rotation_requires_press_on_portrait_and_captures_until_release() {
        let mut drag = Drag::default();
        assert_eq!(drag.update(true, true, false, 10.0, 200.0), 0.0);
        assert_eq!(drag.update(true, false, true, 60.0, 200.0), 0.0);
        assert_eq!(drag.update(true, true, true, 10.0, 200.0), 0.0);
        assert!(
            (drag.update(true, false, false, 110.0, 200.0) - std::f32::consts::PI).abs() < 0.001
        );
        assert_eq!(drag.update(false, false, false, 110.0, 200.0), 0.0);
        assert_eq!(drag.update(true, false, true, 190.0, 200.0), 0.0);
    }
}
