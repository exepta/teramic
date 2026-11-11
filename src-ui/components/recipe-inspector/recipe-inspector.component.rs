use bevy::prelude::*;
use tilt_ui::{ElementId, HtmlEvent, html_fn};

#[html_fn("recipe_category")]
fn recipe_category(
    In(event): In<HtmlEvent>,
    view: Res<crate::CraftingView>,
    elements: Query<&ElementId>,
    parents: Query<&ChildOf>,
    children: Query<&Children>,
    mut actions: MessageWriter<crate::CraftingAction>,
) {
    if !view.recipe_open {
        return;
    }
    if let Some(index) = materialized_row(
        event.target,
        "recipe-category",
        "recipe-category-tabs",
        &elements,
        &parents,
        &children,
    ) && let Some(category) = view.recipe_categories.get(index)
    {
        actions.write(crate::CraftingAction::Category(category.id.clone()));
    }
}

#[html_fn("recipe_alternative")]
fn recipe_alternative(
    In(event): In<HtmlEvent>,
    view: Res<crate::CraftingView>,
    elements: Query<&ElementId>,
    parents: Query<&ChildOf>,
    children: Query<&Children>,
    mut actions: MessageWriter<crate::CraftingAction>,
) {
    if !view.recipe_open {
        return;
    }
    if let Some(index) = materialized_row(
        event.target,
        "recipe-alternative",
        "recipe-alternative-list",
        &elements,
        &parents,
        &children,
    ) && let Some(alternative) = view.recipe_alternatives.get(index)
    {
        actions.write(crate::CraftingAction::PreviewAlternative { index: alternative.index, wood: alternative.wood });
    }
}

#[html_fn("recipe_fuel")]
fn recipe_fuel(
    In(event): In<HtmlEvent>,
    view: Res<crate::CraftingView>,
    elements: Query<&ElementId>,
    parents: Query<&ChildOf>,
    children: Query<&Children>,
    mut actions: MessageWriter<crate::CraftingAction>,
) {
    if !view.recipe_open || !view.smelting || !view.furnace_fuels {
        return;
    }
    if let Some(index) = materialized_row(
        event.target,
        "recipe-fuel",
        "recipe-fuel-list",
        &elements,
        &parents,
        &children,
    ) && let Some(fuel) = view.furnace_fuel_options.get(index)
    {
        actions.write(crate::CraftingAction::SelectFuel(fuel.index));
    }
}

#[html_fn("recipe_wood")]
fn recipe_wood(
    In(event): In<HtmlEvent>,
    view: Res<crate::CraftingView>,
    elements: Query<&ElementId>,
    parents: Query<&ChildOf>,
    children: Query<&Children>,
    mut actions: MessageWriter<crate::CraftingAction>,
) {
    if !view.recipe_open {
        return;
    }
    if let Some(index) = materialized_row(
        event.target,
        "recipe-wood",
        "recipe-wood-options",
        &elements,
        &parents,
        &children,
    ) && let Some(wood) = view.wood_options.get(index)
    {
        actions.write(crate::CraftingAction::PreviewWood(wood.index));
    }
}

fn materialized_row(
    target: Entity,
    row_id: &str,
    root_id: &str,
    elements: &Query<&ElementId>,
    parents: &Query<&ChildOf>,
    children: &Query<&Children>,
) -> Option<usize> {
    if !elements.get(target).is_ok_and(|id| id.0 == row_id) {
        return None;
    }
    let mut root = target;
    loop {
        root = parents.get(root).ok()?.parent();
        if elements.get(root).is_ok_and(|id| id.0 == root_id) {
            break;
        }
    }
    children
        .iter_descendants(root)
        .filter(|entity| elements.get(*entity).is_ok_and(|id| id.0 == row_id))
        .position(|entity| entity == target)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    fn click(target: Entity, handler: &str) -> HtmlEvent {
        HtmlEvent {
            target,
            kind: "click",
            value: None,
            submitter: None,
            data: default(),
            form_data: None,
            handler: handler.into(),
        }
    }

    #[test]
    fn recipe_tabs_and_alternatives_use_the_visible_rows_and_ignore_foreign_buttons() {
        let mut world = World::new();
        world.init_resource::<Messages<crate::CraftingAction>>();
        world.insert_resource(crate::CraftingView {
            recipe_open: true,
            recipe_categories: vec![
                crate::inventory::crafting::RecipeCategoryView {
                    id: "crafting".into(),
                    ..default()
                },
                crate::inventory::crafting::RecipeCategoryView {
                    id: "smelting".into(),
                    ..default()
                },
            ],
            recipe_alternatives: vec![
                crate::inventory::crafting::RecipeAlternativeView {
                    index: 3,
                    ..default()
                },
                crate::inventory::crafting::RecipeAlternativeView {
                    index: 7,
                    wood: Some(2),
                    ..default()
                },
            ],
            ..default()
        });
        let categories = world.spawn(ElementId("recipe-category-tabs".into())).id();
        let mut category_buttons = Vec::new();
        for _ in 0..2 {
            let wrapper = world.spawn(ChildOf(categories)).id();
            category_buttons.push(
                world
                    .spawn((ElementId("recipe-category".into()), ChildOf(wrapper)))
                    .id(),
            );
        }
        let alternatives = world
            .spawn(ElementId("recipe-alternative-list".into()))
            .id();
        let mut recipe_buttons = Vec::new();
        for _ in 0..2 {
            recipe_buttons.push(
                world
                    .spawn((
                        ElementId("recipe-alternative".into()),
                        ChildOf(alternatives),
                    ))
                    .id(),
            );
        }
        world
            .run_system_once_with(
                recipe_category,
                click(category_buttons[1], "recipe_category"),
            )
            .unwrap();
        world
            .run_system_once_with(
                recipe_alternative,
                click(recipe_buttons[1], "recipe_alternative"),
            )
            .unwrap();
        let foreign = world.spawn(ElementId("recipe-category".into())).id();
        world
            .run_system_once_with(recipe_category, click(foreign, "recipe_category"))
            .unwrap();
        let actions: Vec<_> = world
            .resource_mut::<Messages<crate::CraftingAction>>()
            .drain()
            .collect();
        assert_eq!(actions.len(), 2);
        assert!(matches!(&actions[0], crate::CraftingAction::Category(id) if id == "smelting"));
        assert!(matches!(
            actions[1],
            crate::CraftingAction::PreviewAlternative { index: 7, wood: Some(2) }
        ));
        world.resource_mut::<crate::CraftingView>().recipe_open = false;
        world
            .run_system_once_with(
                recipe_category,
                click(category_buttons[0], "recipe_category"),
            )
            .unwrap();
        assert!(
            world
                .resource_mut::<Messages<crate::CraftingAction>>()
                .drain()
                .next()
                .is_none()
        );
    }
}
