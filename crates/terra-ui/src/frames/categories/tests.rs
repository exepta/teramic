use super::*;
use bevy::{sprite::BorderRect, text::LineBreak};

fn layout(world: &mut World) {
    world.init_resource::<crate::layout_classes::LayoutClasses>();
    world
        .run_system_cached(crate::layout_classes::track)
        .unwrap();
    super::equalize_category_widths(world);
}

fn group(world: &mut World, class: &str) -> Entity {
    world
        .spawn((
            Node::default(),
            ElementClasses {
                classes: vec![class.into()],
            },
        ))
        .id()
}
fn text(world: &mut World, caption: &str, physical_width: f32, scale: f32) -> Entity {
    world
        .spawn((
            Text::new(caption),
            TextLayout::linebreak(LineBreak::NoWrap),
            TextLayoutInfo {
                size: Vec2::new(physical_width, 20.0 * scale),
                scale_factor: scale,
                ..default()
            },
            Node::default(),
        ))
        .id()
}
fn caption_button(
    world: &mut World,
    group: Entity,
    caption: &str,
    measured_width: f32,
    scale: f32,
) -> (Entity, Entity) {
    let icon = world
        .spawn((
            TiltElement {
                kind: ElementKind::Icon,
            },
            ComputedNode {
                size: Vec2::splat(24.0 * scale),
                inverse_scale_factor: 1.0 / scale,
                ..default()
            },
        ))
        .id();
    let text = text(world, caption, measured_width * scale, scale);
    let button = world
        .spawn((
            TiltElement {
                kind: ElementKind::Button,
            },
            Node {
                column_gap: Val::Px(8.0),
                ..default()
            },
            ComputedNode {
                padding: BorderRect::all(10.0 * scale),
                border: BorderRect::all(2.0 * scale),
                inverse_scale_factor: 1.0 / scale,
                ..default()
            },
            ChildOf(group),
        ))
        .add_children(&[icon, text])
        .id();
    (button, text)
}

#[test]
fn longest_shaped_label_sets_one_width_independent_of_character_count_or_dpi() {
    for scale in [1.0, 1.5, 2.0] {
        let mut world = World::new();
        let group = group(&mut world, "copper-options-tabs");
        let (narrow, _) = caption_button(&mut world, group, "IIII", 15.0, scale);
        let (wide, _) = caption_button(&mut world, group, "WWWW", 100.0, scale);
        layout(&mut world);
        for button in [narrow, wide] {
            assert_eq!(world.get::<Node>(button).unwrap().width, Val::Px(156.0));
        }
    }
}

#[test]
fn changing_translations_can_grow_and_shrink_without_previous_button_width_feedback() {
    let mut world = World::new();
    let group = group(&mut world, "kingdom-tabs");
    let (first, _) = caption_button(&mut world, group, "Armee", 42.0, 1.0);
    let (second, label) = caption_button(&mut world, group, "Arbeitskräfte", 123.0, 1.0);
    layout(&mut world);
    assert_eq!(world.get::<Node>(first).unwrap().width, Val::Px(179.0));
    world.get_mut::<Text>(label).unwrap().0 = "Work".into();
    world.get_mut::<TextLayoutInfo>(label).unwrap().size.x = 35.0;
    // A stretched parent from the previous frame must not become a new minimum.
    world.get_mut::<ComputedNode>(second).unwrap().size.x = 179.0;
    layout(&mut world);
    assert_eq!(world.get::<Node>(first).unwrap().width, Val::Px(98.0));
    assert_eq!(world.get::<Node>(second).unwrap().width, Val::Px(98.0));
}

#[test]
fn group_waits_for_all_real_labels_and_does_not_resize_unrelated_buttons() {
    let mut world = World::new();
    let group = group(&mut world, "editor-categories");
    let (first, _) = caption_button(&mut world, group, "Steine", 60.0, 1.0);
    let (second, label) = caption_button(&mut world, group, "{{ category.name }}", 0.0, 1.0);
    layout(&mut world);
    assert_eq!(world.get::<Node>(first).unwrap().width, Val::Auto);
    world.get_mut::<Text>(label).unwrap().0 = "Baumaterialien".into();
    world.get_mut::<TextLayoutInfo>(label).unwrap().size.x = 140.0;
    layout(&mut world);
    for entity in [first, second] {
        assert_eq!(world.get::<Node>(entity).unwrap().width, Val::Px(196.0));
    }
    let other_group = self::group(&mut world, "copper-dialog-actions");
    let (other, _) = caption_button(&mut world, other_group, "Zurück", 70.0, 1.0);
    layout(&mut world);
    assert_eq!(world.get::<Node>(other).unwrap().width, Val::Auto);
}

#[test]
fn translated_setting_title_reserves_intrinsic_space_without_resizing_its_text() {
    let mut world = World::new();
    let desc = group(&mut world, "setting-description");
    let title = text(&mut world, "Mehrspielereinstellungen", 284.0, 1.0);
    world.entity_mut(title).insert(ElementClasses {
        classes: vec!["setting-title".into()],
    });
    world.entity_mut(desc).add_child(title);
    layout(&mut world);
    assert_eq!(world.get::<Node>(desc).unwrap().min_width, Val::Px(284.0));
    assert_eq!(world.get::<Node>(title).unwrap().width, Val::Auto);
}

#[test]
fn asymmetric_insets_use_the_horizontal_edges_only() {
    let mut world = World::new();
    let group = group(&mut world, "copper-options-tabs");
    let (button, _) = caption_button(&mut world, group, "Steuerung", 80.0, 1.0);
    let mut geometry = world.get_mut::<ComputedNode>(button).unwrap();
    geometry.padding = BorderRect::from([3.0, 11.0, 27.0, 37.0]);
    geometry.border = BorderRect::from([1.0, 5.0, 17.0, 19.0]);
    layout(&mut world);
    assert_eq!(world.get::<Node>(button).unwrap().width, Val::Px(132.0));
}

#[test]
fn setting_description_never_forces_a_narrow_row_beyond_its_available_width() {
    let mut world = World::new();
    let row = world
        .spawn(ComputedNode {
            size: Vec2::new(240.0, 80.0),
            padding: BorderRect::all(6.0),
            inverse_scale_factor: 1.0,
            ..default()
        })
        .id();
    let desc = group(&mut world, "setting-description");
    world.entity_mut(row).add_child(desc);
    let title = text(&mut world, "Sichtbarkeitsaktualisierungsrate", 280.0, 1.0);
    world.entity_mut(title).insert(ElementClasses {
        classes: vec!["setting-title".into()],
    });
    world.entity_mut(desc).add_child(title);
    layout(&mut world);
    assert_eq!(world.get::<Node>(desc).unwrap().min_width, Val::Px(228.0));
}

#[test]
fn anonymous_text_inside_a_styled_title_wrapper_reserves_the_shaped_width() {
    let mut world = World::new();
    let desc = group(&mut world, "setting-description");
    let wrapper = group(&mut world, "setting-title");
    let glyphs = text(&mut world, "Sichtbarkeitsaktualisierungsrate", 237.0, 1.5);
    world.entity_mut(wrapper).add_child(glyphs);
    world.entity_mut(desc).add_child(wrapper);
    layout(&mut world);
    assert_eq!(world.get::<Node>(desc).unwrap().min_width, Val::Px(158.0));
}
