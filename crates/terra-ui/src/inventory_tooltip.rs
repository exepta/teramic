use bevy::{prelude::*, ui::UiGlobalTransform};
use tilt_ui::{ElementId, ElementKind, TiltElement, WidgetLayoutOverride};

#[derive(Component)]
pub struct TooltipOwner(pub Entity);

#[derive(Component, Default, PartialEq, Eq)]
struct TooltipPresentation {
    owner_hidden: bool,
    open: bool,
}

fn portal(world: &mut World, popup: Entity) {
    if world.get::<TooltipOwner>(popup).is_some() {
        return;
    }
    let Some(owner) = world.get::<ChildOf>(popup).map(ChildOf::parent) else {
        return;
    };
    let mut cursor = Some(owner);
    let mut root = None;
    while let Some(entity) = cursor {
        if world.get::<Node>(entity).is_some() {
            root = Some(entity);
        }
        cursor = world.get::<ChildOf>(entity).map(ChildOf::parent);
    }
    if let Some(root) = root.filter(|root| *root != owner) {
        // Clip overrides alone still enlarge scroll extents. Keep the TiltUI
        // widget, bindings and target, but place it outside scrolling ancestors.
        world.entity_mut(popup).insert((
            TooltipOwner(owner),
            TooltipPresentation::default(),
            Visibility::Hidden,
            ChildOf(root),
        ));
    }
}

pub(super) fn fit_contents(world: &mut World) {
    let detached = world
        .query::<(Entity, &TooltipOwner)>()
        .iter(world)
        .map(|(entity, owner)| (entity, owner.0))
        .collect::<Vec<_>>();
    for (entity, owner) in detached {
        if world.get_entity(owner).is_err() {
            world.entity_mut(entity).despawn();
            continue;
        }
        let mut parent = Some(owner);
        let mut hidden = false;
        while let Some(node) = parent {
            hidden |= world
                .get::<Node>(node)
                .is_some_and(|n| n.display == Display::None);
            parent = world.get::<ChildOf>(node).map(ChildOf::parent);
        }
        let open = !hidden
            && world
                .get::<Node>(entity)
                .is_some_and(|n| n.display != Display::None);
        let was_open = world
            .get::<TooltipPresentation>(entity)
            .is_some_and(|p| p.open);
        let next = TooltipPresentation {
            owner_hidden: hidden,
            open,
        };
        if world.get::<TooltipPresentation>(entity) != Some(&next) {
            world.entity_mut(entity).insert(next);
        }
        if (!open || !was_open) && world.get::<Visibility>(entity) != Some(&Visibility::Hidden) {
            world.entity_mut(entity).insert(Visibility::Hidden);
        }
    }
    let popups = world
        .query::<(Entity, &TiltElement)>()
        .iter(world)
        .filter_map(|(entity, element)| (element.kind == ElementKind::ToolTip).then_some(entity))
        .collect::<Vec<_>>();
    for popup in popups {
        portal(world, popup);
        if world
            .get::<Node>(popup)
            .is_none_or(|n| n.display == Display::None)
        {
            continue;
        }
        let mut descendants = vec![popup];
        let mut index = 0;
        while index < descendants.len() {
            let entity = descendants[index];
            if let Some(children) = world.get::<Children>(entity) {
                descendants.extend(children.iter());
            }
            if let Some(mut node) = world.get_mut::<Node>(entity) {
                if node.overflow != Overflow::visible() {
                    node.overflow = Overflow::visible();
                }
                if node.scrollbar_width != 0.0 {
                    node.scrollbar_width = 0.0;
                }
            }
            if let Some(mut text) = world.get_mut::<TextLayout>(entity)
                && text.linebreak != bevy::text::LineBreak::WordOrCharacter
            {
                // Long registry identifiers must wrap even when they contain no spaces.
                text.linebreak = bevy::text::LineBreak::WordOrCharacter;
            }
            index += 1;
        }
    }
}

pub(super) fn position(world: &mut World) {
    let Some(pointer) = world
        .query_filtered::<&Window, With<bevy::window::PrimaryWindow>>()
        .iter(world)
        .next()
        .and_then(Window::physical_cursor_position)
    else {
        return;
    };
    let open = world
        .query::<(
            Entity,
            &TiltElement,
            &Node,
            &ComputedNode,
            &ComputedUiRenderTargetInfo,
            Option<&TooltipPresentation>,
        )>()
        .iter(world)
        .filter_map(|(entity, element, node, computed, target, presentation)| {
            (element.kind == ElementKind::ToolTip
                && node.display != Display::None
                && !presentation.is_some_and(|p| p.owner_hidden)
                && !computed.is_empty())
            .then_some((
                entity,
                computed.size(),
                computed.inverse_scale_factor,
                target.physical_size().as_vec2(),
            ))
        })
        .collect::<Vec<_>>();
    for (entity, size, inverse_scale, viewport) in open {
        if !inverse_scale.is_finite() || inverse_scale <= 0.0 {
            continue;
        }
        let top_left = placement(pointer, size, viewport, 10.0 / inverse_scale);
        // Conditional TiltUI templates may insert GhostNodes between popup and layout parent.
        let mut parent = world.get::<ChildOf>(entity).map(ChildOf::parent);
        let layout_parent = loop {
            let Some(candidate) = parent else { break None };
            if let (Some(transform), Some(node)) = (
                world.get::<UiGlobalTransform>(candidate),
                world.get::<ComputedNode>(candidate),
            ) && let Some(inverse) = transform.try_inverse()
            {
                break Some((inverse, node.size()));
            }
            parent = world.get::<ChildOf>(candidate).map(ChildOf::parent);
        };
        let Some((inverse, parent_size)) = layout_parent else {
            continue;
        };
        let local = (inverse.transform_point2(top_left) + parent_size * 0.5) * inverse_scale;
        let layout = WidgetLayoutOverride {
            left: Some(Val::Px(local.x)),
            top: Some(Val::Px(local.y)),
            ..default()
        };
        if !world
            .get::<WidgetLayoutOverride>(entity)
            .is_some_and(|old| old.left == layout.left && old.top == layout.top)
        {
            if let Some(mut node) = world.get_mut::<Node>(entity) {
                node.left = Val::Px(local.x);
                node.top = Val::Px(local.y);
            }
            world.entity_mut(entity).insert(layout);
        }
        // Layout has already run. Move this measured subtree now as well as
        // setting next frame's layout, so no frame is drawn at the old origin.
        if let Some(transform) = world.get::<UiGlobalTransform>(entity) {
            let current = transform.affine().transform_point2(-size * 0.5);
            translate_subtree(world, entity, top_left - current);
            if world.get::<Visibility>(entity) != Some(&Visibility::Inherited) {
                world.entity_mut(entity).insert(Visibility::Inherited);
            }
        }
    }
}

fn translate_subtree(world: &mut World, root: Entity, delta: Vec2) {
    if delta.length_squared() < 0.0001 {
        return;
    }
    let mut pending = vec![root];
    while let Some(entity) = pending.pop() {
        if let Some(children) = world.get::<Children>(entity) {
            pending.extend(children.iter());
        }
        if let Some(mut transform) = world.get_mut::<UiGlobalTransform>(entity) {
            let mut affine = transform.affine();
            affine.translation += delta;
            *transform = affine.into();
        }
    }
}

fn placement(pointer: Vec2, size: Vec2, viewport: Vec2, gap: f32) -> Vec2 {
    let mut result = pointer + Vec2::splat(gap);
    for axis in 0..2 {
        if result[axis] + size[axis] > viewport[axis] - gap {
            result[axis] = pointer[axis] - gap - size[axis];
        }
    }
    result.clamp(Vec2::ZERO, (viewport - size).max(Vec2::ZERO))
}

#[derive(Component)]
pub(super) struct IdentifierMeasure {
    full: String,
    full_width: Option<f32>,
    stable_frames: u8,
}

pub(super) fn fit_identifier(world: &mut World) {
    use bevy::text::{LineBreak, TextLayoutInfo};
    let Some(view) = world.get_resource::<super::view::InventoryView>() else {
        return;
    };
    if !view.open {
        return;
    }
    let full = view.detail_identifier.clone();
    let short = view.detail_registry_name.clone();
    let roots = world
        .query::<(Entity, &ElementId)>()
        .iter(world)
        .filter_map(|(entity, id)| (id.0 == "inventory-identifier-text").then_some(entity))
        .collect::<Vec<_>>();
    for root in roots {
        let Some(parent) = world.get::<ChildOf>(root).map(ChildOf::parent) else {
            continue;
        };
        let width = world
            .get::<ComputedNode>(parent)
            .map_or(0.0, |node| node.size().x);
        if width <= 0.0 {
            continue;
        }
        let texts = world
            .query::<(Entity, &Text)>()
            .iter(world)
            .filter_map(|(entity, _)| {
                let mut p = Some(entity);
                while let Some(id) = p {
                    if id == root {
                        return Some(entity);
                    }
                    p = world.get::<ChildOf>(id).map(ChildOf::parent);
                }
                None
            })
            .collect::<Vec<_>>();
        for entity in texts {
            if let Some(mut layout) = world.get_mut::<TextLayout>(entity) {
                layout.linebreak = LineBreak::NoWrap;
            }
            if world
                .get::<IdentifierMeasure>(entity)
                .is_none_or(|m| m.full != full)
            {
                if let Some(mut text) = world.get_mut::<Text>(entity) {
                    text.0.clone_from(&full);
                }
                world.entity_mut(entity).insert(IdentifierMeasure {
                    full: full.clone(),
                    full_width: None,
                    stable_frames: 0,
                });
                continue;
            }
            let measured = world
                .get::<TextLayoutInfo>(entity)
                .map_or(0.0, |info| info.size.x);
            let Some(mut measure) = world.get_mut::<IdentifierMeasure>(entity) else {
                continue;
            };
            measure.stable_frames = measure.stable_frames.saturating_add(1);
            if measure.full_width.is_none() && measure.stable_frames >= 2 && measured > 0.0 {
                measure.full_width = Some(measured);
            }
            let text = if measure.full_width.is_some_and(|full| full > width) {
                &short
            } else {
                &full
            };
            if let Some(mut current) = world.get_mut::<Text>(entity)
                && current.0 != *text
            {
                current.0.clone_from(text);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hidden_cached_tooltips_do_not_dirty_visibility_or_presentation_every_frame() {
        let mut world = World::new();
        let root = world.spawn(Node::default()).id();
        let owner = world.spawn((Node::default(), ChildOf(root))).id();
        let popup = world
            .spawn((
                TiltElement {
                    kind: ElementKind::ToolTip,
                },
                Node {
                    display: Display::None,
                    ..default()
                },
                ChildOf(owner),
            ))
            .id();
        fit_contents(&mut world);
        fit_contents(&mut world);
        world.clear_trackers();
        fit_contents(&mut world);
        assert!(
            !world
                .query::<Ref<Visibility>>()
                .get(&world, popup)
                .unwrap()
                .is_changed()
        );
        assert!(
            !world
                .query::<Ref<TooltipPresentation>>()
                .get(&world, popup)
                .unwrap()
                .is_changed()
        );
    }
    #[test]
    fn tooltip_portal_is_outside_scroll_content_and_dies_with_its_owner() {
        let mut world = World::new();
        let root = world.spawn(Node::default()).id();
        let scroll = world
            .spawn((
                Node {
                    overflow: Overflow::scroll_y(),
                    ..default()
                },
                ChildOf(root),
            ))
            .id();
        let slot = world.spawn((Node::default(), ChildOf(scroll))).id();
        let popup = world
            .spawn((
                TiltElement {
                    kind: ElementKind::ToolTip,
                },
                Node {
                    position_type: PositionType::Absolute,
                    ..default()
                },
                ChildOf(slot),
            ))
            .id();
        fit_contents(&mut world);
        assert_eq!(world.get::<ChildOf>(popup).unwrap().parent(), root);
        assert_eq!(
            world.get::<Node>(scroll).unwrap().overflow,
            Overflow::scroll_y()
        );
        world.entity_mut(slot).despawn();
        fit_contents(&mut world);
        assert!(world.get_entity(popup).is_err());
    }
    #[test]
    fn tooltip_text_wraps_long_identifiers_and_never_becomes_a_scroll_container() {
        let mut world = World::new();
        let parent = world
            .spawn(Node {
                overflow: Overflow::scroll_y(),
                ..default()
            })
            .id();
        let tooltip = world
            .spawn((
                TiltElement {
                    kind: ElementKind::ToolTip,
                },
                Node {
                    overflow: Overflow::scroll_y(),
                    scrollbar_width: 8.0,
                    ..default()
                },
                ChildOf(parent),
            ))
            .id();
        let text = world
            .spawn((
                Text::new("my_mod::a_very_long_unbroken_registry_identifier"),
                TextLayout {
                    linebreak: bevy::text::LineBreak::NoWrap,
                    ..default()
                },
                Node {
                    overflow: Overflow::scroll_x(),
                    ..default()
                },
                ChildOf(tooltip),
            ))
            .id();
        fit_contents(&mut world);
        assert_eq!(
            world.get::<Node>(tooltip).unwrap().overflow,
            Overflow::visible()
        );
        assert_eq!(world.get::<Node>(tooltip).unwrap().scrollbar_width, 0.0);
        assert_eq!(
            world.get::<Node>(text).unwrap().overflow,
            Overflow::visible()
        );
        assert_eq!(
            world.get::<TextLayout>(text).unwrap().linebreak,
            bevy::text::LineBreak::WordOrCharacter
        );
        assert_eq!(
            world.get::<Node>(parent).unwrap().overflow,
            Overflow::scroll_y()
        );
    }

    #[test]
    fn identifier_uses_the_short_name_only_when_the_shaped_full_text_does_not_fit() {
        let mut world = World::new();
        world.insert_resource(super::super::view::InventoryView {
            open: true,
            detail_identifier: "my_mod::purple_mushroom_cap".into(),
            detail_registry_name: "purple_mushroom_cap".into(),
            ..default()
        });
        let parent = world
            .spawn(ComputedNode {
                size: Vec2::new(180.0, 18.0),
                ..default()
            })
            .id();
        let root = world
            .spawn((
                ElementId("inventory-identifier-text".into()),
                ChildOf(parent),
            ))
            .id();
        let label = world
            .spawn((
                Text::new("my_mod::purple_mushroom_cap"),
                TextLayout::default(),
                bevy::text::TextLayoutInfo {
                    size: Vec2::new(220.0, 14.0),
                    ..default()
                },
                ChildOf(root),
            ))
            .id();
        for _ in 0..3 {
            fit_identifier(&mut world);
        }
        assert_eq!(world.get::<Text>(label).unwrap().0, "purple_mushroom_cap");
        assert_eq!(
            world.get::<TextLayout>(label).unwrap().linebreak,
            bevy::text::LineBreak::NoWrap
        );
        for _ in 0..4 {
            fit_identifier(&mut world);
        }
        assert_eq!(world.get::<Text>(label).unwrap().0, "purple_mushroom_cap");
        world.get_mut::<ComputedNode>(parent).unwrap().size.x = 300.0;
        fit_identifier(&mut world);
        assert_eq!(
            world.get::<Text>(label).unwrap().0,
            "my_mod::purple_mushroom_cap"
        );
    }
    #[test]
    fn all_tooltips_follow_the_pointer_at_ten_pixels() {
        assert_eq!(
            placement(
                Vec2::new(100.0, 120.0),
                Vec2::new(240.0, 100.0),
                Vec2::new(1280.0, 720.0),
                10.0
            ),
            Vec2::new(110.0, 130.0)
        );
    }
    #[test]
    fn viewport_edges_flip_without_covering_the_cursor() {
        for pointer in [
            Vec2::ZERO,
            Vec2::new(639.0, 0.0),
            Vec2::new(0.0, 479.0),
            Vec2::new(639.0, 479.0),
        ] {
            let size = Vec2::new(250.0, 120.0);
            let p = placement(pointer, size, Vec2::new(640.0, 480.0), 10.0);
            assert!(p.cmpge(Vec2::ZERO).all());
            assert!((p + size).cmple(Vec2::new(640.0, 480.0)).all());
            assert!(!Rect::from_corners(p, p + size).contains(pointer));
        }
    }
}
