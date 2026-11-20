//! Give related menu categories the intrinsic width of their longest caption.
//! Read shaped glyph metrics, never character counts or the previous button
//! width: German translations, DPI changes and shorter replacement labels can
//! all reflow without a feedback loop.
use bevy::{prelude::*, text::TextLayoutInfo};
use tilt_ui::{ElementClasses, ElementKind, TiltElement};

fn category_group(classes: &ElementClasses) -> bool {
    classes.classes.iter().any(|class| {
        matches!(
            class.as_str(),
            "copper-options-tabs"
                | "kingdom-tabs"
                | "character-tabs"
                | "world-create-tabs"
                | "build-mode-categories"
                | "editor-categories"
        )
    })
}

fn has_class(world: &World, entity: Entity, class: &str) -> bool {
    world
        .get::<ElementClasses>(entity)
        .is_some_and(|classes| classes.classes.iter().any(|value| value == class))
}

fn button(world: &World, entity: Entity) -> bool {
    world.get::<TiltElement>(entity).is_some_and(|element| {
        matches!(
            element.kind,
            ElementKind::Button | ElementKind::ToggleButton
        )
    })
}

fn buttons_in(world: &World, root: Entity) -> Vec<Entity> {
    let mut pending = world
        .get::<Children>(root)
        .map_or_else(Vec::new, |children| children.to_vec());
    let mut buttons = Vec::new();
    for _ in 0..512 {
        let Some(entity) = pending.pop() else {
            return buttons;
        };
        if button(world, entity) {
            buttons.push(entity);
        } else if let Some(children) = world.get::<Children>(entity) {
            pending.extend(children.iter());
        }
    }
    // An unexpectedly large subtree should keep its authored layout rather
    // than size a partial group from an incomplete traversal.
    Vec::new()
}

fn text_width(world: &World, entity: Entity) -> Option<f32> {
    // Tilt's styled paragraph/label can own an anonymous text child. Measure
    // the actual shaped descendant, not only entities that also carry classes.
    let mut pending = vec![entity];
    let mut width = 0.0_f32;
    let mut measured = false;
    for _ in 0..128 {
        let Some(entity) = pending.pop() else {
            return measured.then_some(width);
        };
        if let Some(text) = world.get::<Text>(entity) {
            if text.0.trim().is_empty() {
                continue;
            }
            if text.0.contains("{{") {
                return None;
            }
            let info = world.get::<TextLayoutInfo>(entity)?;
            let actual = info.size.x / info.scale_factor.max(0.01);
            if !actual.is_finite() || actual <= 0.0 {
                return None;
            }
            width = width.max(actual);
            measured = true;
        } else if let Some(children) = world.get::<Children>(entity) {
            pending.extend(children.iter());
        }
    }
    None
}

fn horizontal_edges(world: &World, entity: Entity) -> f32 {
    world.get::<ComputedNode>(entity).map_or(0.0, |node| {
        (node.padding.min_inset.x
            + node.padding.max_inset.x
            + node.border.min_inset.x
            + node.border.max_inset.x)
            * node.inverse_scale_factor()
    })
}

fn intrinsic_width(world: &World, entity: Entity) -> Option<f32> {
    let node = world.get::<Node>(entity)?;
    let column = matches!(
        node.flex_direction,
        FlexDirection::Column | FlexDirection::ColumnReverse
    );
    let mut pending = world.get::<Children>(entity)?.to_vec();
    let mut parts = Vec::new();
    for _ in 0..128 {
        let Some(child) = pending.pop() else {
            if parts.is_empty() {
                return None;
            }
            let content = if column {
                parts.into_iter().fold(0.0_f32, f32::max)
            } else {
                let gap = match node.column_gap {
                    Val::Px(px) => px,
                    _ => 0.0,
                };
                parts.iter().sum::<f32>() + gap * (parts.len() - 1) as f32
            };
            return Some(content + horizontal_edges(world, entity));
        };
        if world
            .get::<Node>(child)
            .is_some_and(|node| node.position_type == PositionType::Absolute)
        {
            continue;
        }
        if world.get::<Text>(child).is_some() {
            parts.push(text_width(world, child)?);
        } else if world
            .get::<TiltElement>(child)
            .is_some_and(|element| element.kind == ElementKind::Icon)
        {
            let computed = world.get::<ComputedNode>(child)?;
            let width = computed.size().x * computed.inverse_scale_factor();
            if width <= 0.0 {
                return None;
            }
            parts.push(width);
        } else if let Some(children) = world.get::<Children>(child) {
            pending.extend(children.iter());
        }
    }
    None
}

pub(super) fn equalize_category_widths(world: &mut World) {
    let targets: Vec<_> = world
        .query::<(Entity, &ElementClasses)>()
        .iter(world)
        .filter_map(|(entity, classes)| category_group(classes).then_some(entity))
        .collect();
    for group in targets {
        let buttons = buttons_in(world, group);
        let widths: Option<Vec<_>> = buttons
            .iter()
            .map(|&entity| intrinsic_width(world, entity))
            .collect();
        let Some(widths) = widths else {
            continue;
        };
        let width = widths.into_iter().fold(0.0_f32, f32::max).ceil();
        if width <= 0.0 {
            continue;
        }
        for entity in buttons {
            if let Some(mut node) = world.get_mut::<Node>(entity)
                && (node.width != Val::Px(width)
                    || node.min_width != Val::Px(width)
                    || node.flex_grow != 0.0
                    || node.flex_shrink != 0.0
                    || node.flex_basis != Val::Auto)
            {
                node.width = Val::Px(width);
                node.min_width = Val::Px(width);
                node.flex_grow = 0.0;
                node.flex_shrink = 0.0;
                node.flex_basis = Val::Auto;
            }
        }
    }
    // Reserve the real width of a settings title. The row's normal flex-wrap
    // can move its controls below it instead of splitting a German compound.
    let descriptions: Vec<_> = world
        .query::<(Entity, &ElementClasses)>()
        .iter(world)
        .filter_map(|(entity, classes)| {
            classes
                .classes
                .iter()
                .any(|class| class == "setting-description")
                .then_some(entity)
        })
        .collect();
    for entity in descriptions {
        let width = world
            .get::<Children>(entity)
            .into_iter()
            .flatten()
            .filter(|&&child| has_class(world, child, "setting-title"))
            .filter_map(|&child| text_width(world, child))
            .fold(0.0_f32, f32::max)
            + horizontal_edges(world, entity);
        // Long labels must not force a description wider than its row. Narrow
        // layouts move the category rail above the form and use compact text.
        let available = world.get::<ChildOf>(entity).and_then(|parent| {
            world.get::<ComputedNode>(parent.parent()).map(|node| {
                (node.size().x * node.inverse_scale_factor()
                    - horizontal_edges(world, parent.parent()))
                .max(0.0)
            })
        });
        let width = available
            .filter(|width| *width > 0.0)
            .map_or(width, |available| width.min(available));
        if width > 0.0
            && let Some(mut node) = world.get_mut::<Node>(entity)
            && node.min_width != Val::Px(width.ceil())
        {
            node.min_width = Val::Px(width.ceil());
        }
    }
}

#[cfg(test)]
mod tests;
