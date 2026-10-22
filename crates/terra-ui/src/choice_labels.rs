use bevy::{prelude::*, text::LineBreak};
use tilt_ui::{ChoiceBoxParts, OptionData};

// TiltUI 0.1 retains the authored interpolation in OptionData even after the
// child text binding resolves. Keep the ChoiceBox's semantic label in sync.
pub(crate) fn refresh_bound_labels(world: &mut World) {
    let options = world
        .query::<(Entity, &OptionData, &Children)>()
        .iter(world)
        .filter_map(|(entity, data, children)| {
            if !data.label.contains("{{") {
                return world
                    .get::<tilt_ui::ControlChecked>(entity)
                    .is_some_and(|checked| checked.0)
                    .then(|| (entity, data.label.clone()));
            }
            let label = children
                .iter()
                .filter_map(|child| world.get::<Text>(child))
                .map(|text| text.0.trim())
                .collect::<Vec<_>>()
                .join(" ");
            (!label.is_empty() && !label.contains("{{")).then_some((entity, label))
        })
        .collect::<Vec<_>>();
    for (entity, label) in options {
        if let Some(mut data) = world.get_mut::<OptionData>(entity)
            && data.label != label
        {
            data.label.clone_from(&label);
        }
        if !world
            .get::<tilt_ui::ControlChecked>(entity)
            .is_some_and(|checked| checked.0)
        {
            continue;
        }
        let mut parent = world.get::<ChildOf>(entity).map(ChildOf::parent);
        while let Some(id) = parent {
            if let Some(parts) = world.get::<ChoiceBoxParts>(id).copied() {
                if let Some(mut text) = world.get_mut::<Text>(parts.value)
                    && text.0 != label
                {
                    text.0.clone_from(&label);
                }
                break;
            }
            parent = world.get::<ChildOf>(id).map(ChildOf::parent);
        }
    }
}

// Choice rows use one intrinsic line; long labels are clipped by the option.
pub(crate) fn single_line_labels(
    options: Query<&Children, Added<OptionData>>,
    mut labels: Query<&mut TextLayout>,
) {
    for children in &options {
        for child in children {
            if let Ok(mut layout) = labels.get_mut(*child) {
                layout.linebreak = LineBreak::NoWrap;
            }
        }
    }
}

pub(crate) fn refresh_popup_layout(
    choices: Query<&ChoiceBoxParts, Changed<ChoiceBoxParts>>,
    mut children: Query<&mut Children>,
) {
    for choice in &choices {
        if !choice.open {
            continue;
        }
        let mut pending = vec![choice.popup];
        // TiltUI reparents authored options into an initially hidden popup.
        // Resync that subtree on open so Bevy does not retain zero-sized labels.
        while let Some(entity) = pending.pop() {
            if let Ok(mut descendants) = children.get_mut(entity) {
                pending.extend(descendants.to_vec());
                descendants.set_changed();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bound_option_labels_refresh_the_selected_choice_without_literal_interpolation() {
        let mut world = World::new();
        let value = world.spawn(Text::new("{{ choice.label }}")).id();
        let popup = world.spawn_empty().id();
        let choice = world
            .spawn(ChoiceBoxParts {
                value,
                popup,
                open: false,
            })
            .id();
        world.entity_mut(choice).add_child(popup);
        let label = world.spawn(Text::new("3840 x 2160")).id();
        let option = world
            .spawn((
                OptionData {
                    value: "3840x2160".into(),
                    label: "{{ choice.label }}".into(),
                },
                tilt_ui::ControlChecked(true),
                ChildOf(popup),
            ))
            .add_child(label)
            .id();
        refresh_bound_labels(&mut world);
        assert_eq!(
            world.get::<OptionData>(option).unwrap().label,
            "3840 x 2160"
        );
        assert_eq!(world.get::<Text>(value).unwrap().0, "3840 x 2160");
        world.get_mut::<Text>(value).unwrap().0 = "{{ choice.label }}".into();
        refresh_bound_labels(&mut world);
        assert_eq!(world.get::<Text>(value).unwrap().0, "3840 x 2160");
    }
    #[derive(Resource, Default)]
    struct ChangedTrees(usize);
    fn count(changed: Query<(), Changed<Children>>, mut count: ResMut<ChangedTrees>) {
        count.0 = changed.iter().count();
    }
    #[test]
    fn opening_remeasures_the_existing_option_tree_without_duplicate_nodes() {
        let mut app = App::new();
        app.init_resource::<ChangedTrees>().add_systems(
            Update,
            (single_line_labels, refresh_popup_layout, count).chain(),
        );
        let label = app.world_mut().spawn(TextLayout::default()).id();
        let option = app
            .world_mut()
            .spawn(OptionData {
                value: "survival".into(),
                label: "Survival".into(),
            })
            .add_child(label)
            .id();
        let popup = app.world_mut().spawn_empty().add_child(option).id();
        let choice = app
            .world_mut()
            .spawn(ChoiceBoxParts {
                value: label,
                popup,
                open: false,
            })
            .id();
        app.update();
        assert_eq!(
            app.world().get::<TextLayout>(label).unwrap().linebreak,
            LineBreak::NoWrap
        );
        app.update();
        assert_eq!(app.world().resource::<ChangedTrees>().0, 0);
        app.world_mut()
            .get_mut::<ChoiceBoxParts>(choice)
            .unwrap()
            .open = true;
        app.update();
        assert_eq!(app.world().resource::<ChangedTrees>().0, 2);
        assert_eq!(app.world().get::<Children>(option).unwrap().len(), 1);
        app.update();
        assert_eq!(app.world().resource::<ChangedTrees>().0, 0);
    }
}
