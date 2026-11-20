use super::*;

fn browser_app() -> App {
    let mut app = App::new();
    app.init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<ButtonInput<MouseButton>>()
        .init_resource::<Time>()
        .init_resource::<InventoryState>()
        .init_resource::<CraftingState>()
        .init_resource::<CraftingView>()
        .init_resource::<BlockPalette>()
        .init_resource::<pointer::InventoryQueue>()
        .init_resource::<ClientSession>()
        .add_message::<InventoryAction>()
        .add_message::<CraftingAction>()
        .add_message::<MouseWheel>()
        .insert_resource(RecipeBook::builtins().clone())
        .add_systems(Update, (actions, update).chain());
    app.world_mut().resource_mut::<InventoryState>().open = true;
    app.world_mut().resource_mut::<ClientSession>().ready = true;
    app.update();
    app
}

fn command(app: &mut App, action: CraftingAction) {
    app.world_mut().write_message(action);
    app.update();
}

#[test]
fn road_tmi_exposes_real_machine_binder_aggregate_and_all_construction_forms() {
    let book = RecipeBook::builtins();
    for (item, kind) in [
        (ItemKind::Bitumen, RecipeKind::Smelting),
        (ItemKind::RoadAggregate, RecipeKind::Crushing),
    ] {
        let choices = alternatives(book, Some(item));
        assert!(choices.iter().any(|choice| choice.selection().1 == kind));
    }
    for material in terra_core::roads::MATERIALS {
        let choices = alternatives(book, Some(ItemKind::Block(material)));
        assert!(!choices.is_empty());
        assert!(
            choices
                .iter()
                .all(|choice| choice.selection().1 == RecipeKind::Craft)
        );
    }
}

#[test]
fn tar_bucket_tmi_exposes_the_real_workbench_recipe() {
    let choices = alternatives(
        RecipeBook::builtins(),
        Some(ItemKind::Tool(terra_core::ToolKind::TarBucket)),
    );
    assert!(choices.iter().any(|choice| {
        let (key, kind) = choice.selection();
        key == terra_core::crafting::recipe_key("teramic::tar_bucket") && kind == RecipeKind::Craft
    }));
}

#[test]
fn recipe_search_accepts_localized_output_metadata_while_preserving_keys_and_input() {
    let mut app = browser_app();
    let axe = ItemKind::Tool(terra_core::ToolKind::IronAxe);
    app.world_mut()
        .resource_mut::<BlockPalette>()
        .item_metadata
        .insert(
            axe,
            ("Iron Axe".into(), "A iron tool for harvesting wood.".into()),
        );
    crate::localization::install_test_language(app.world_mut(), "de-DE");
    let key = app
        .world()
        .resource::<RecipeBook>()
        .iter()
        .find(|recipe| recipe.id == "teramic::iron_axe")
        .unwrap()
        .key();
    let inventory = app.world().resource::<ClientSession>().inventory.clone();
    for (locale, queries) in [
        (
            "de-DE",
            &["Eisenaxt", "HOLZFÄLLEN", "Iron Axe", "teramic::iron_axe"][..],
        ),
        (
            "en-US",
            &["Iron Axe", "harvesting wood", "teramic::iron_axe"][..],
        ),
    ] {
        app.world_mut()
            .resource_mut::<tilt_ui::UiLocalization>()
            .set_locale(locale)
            .unwrap();
        for query in queries {
            command(&mut app, CraftingAction::Search((*query).into()));
            let state = app.world().resource::<CraftingState>();
            assert_eq!(
                state.selected,
                Some(key),
                "No axe recipe for {locale} / {query}"
            );
            assert_eq!(state.recipe_item, Some(axe));
            assert_eq!(state.search, *query);
            assert!(
                app.world()
                    .resource::<RecipeBook>()
                    .get(key)
                    .is_some_and(
                        |recipe| recipe.id == "teramic::iron_axe" && recipe.name == "Iron Axe"
                    )
            );
        }
    }
    assert_eq!(app.world().resource::<ClientSession>().inventory, inventory);
    assert!(app.world().resource::<pointer::InventoryQueue>().is_empty());
}

#[test]
fn hidden_prewarm_projects_crafting_slots_without_enabling_actions_or_closed_work() {
    let mut app = browser_app();
    app.world_mut().resource_mut::<InventoryState>().open = false;
    app.update();
    assert!(*app.world().resource::<CraftingView>() == CraftingView::default());
    app.insert_resource(InventoryView {
        visible: true,
        prewarming: true,
        ..default()
    });
    app.world_mut()
        .resource_mut::<ClientSession>()
        .inventory
        .insert(ItemKind::Block(6), 4);
    let inventory = app.world().resource::<ClientSession>().inventory.clone();
    for action in [
        CraftingAction::OpenBook,
        CraftingAction::ApplyRecipe,
        CraftingAction::Craft {
            maximum: true,
            cursor: true,
        },
    ] {
        app.world_mut().write_message(action);
    }
    app.update();
    let view = app.world().resource::<CraftingView>();
    assert_eq!(view.grid.len(), 4);
    assert_eq!(view.output.len(), 1);
    assert!(view.grid.iter().all(|slot| slot.empty));
    assert!(!view.recipe_open);
    assert!(!app.world().resource::<InventoryState>().open);
    assert_eq!(app.world().resource::<ClientSession>().inventory, inventory);
    assert!(app.world().resource::<pointer::InventoryQueue>().is_empty());
    let warm_tick = app
        .world()
        .get_resource_ref::<CraftingView>()
        .unwrap()
        .last_changed();
    for _ in 0..4 {
        app.update();
        assert_eq!(
            app.world()
                .get_resource_ref::<CraftingView>()
                .unwrap()
                .last_changed(),
            warm_tick
        );
    }
    app.world_mut().resource_mut::<InventoryView>().prewarming = false;
    app.update();
    assert!(*app.world().resource::<CraftingView>() == CraftingView::default());
    let closed_tick = app
        .world()
        .get_resource_ref::<CraftingView>()
        .unwrap()
        .last_changed();
    for _ in 0..4 {
        app.update();
        assert_eq!(
            app.world()
                .get_resource_ref::<CraftingView>()
                .unwrap()
                .last_changed(),
            closed_tick
        );
    }
    // Resizing can request another prewarm without changing inventory or recipes.
    app.world_mut().resource_mut::<InventoryView>().prewarming = true;
    app.update();
    assert_eq!(app.world().resource::<CraftingView>().grid.len(), 4);
    app.world_mut().resource_mut::<InventoryView>().visible = false;
    app.update();
    assert!(*app.world().resource::<CraftingView>() == CraftingView::default());
    app.world_mut().resource_mut::<InventoryView>().visible = true;
    app.world_mut().resource_mut::<ClientSession>().ready = false;
    app.update();
    assert!(*app.world().resource::<CraftingView>() == CraftingView::default());
    app.world_mut().resource_mut::<ClientSession>().ready = true;
    app.update();
    assert_eq!(app.world().resource::<CraftingView>().output.len(), 1);
    app.world_mut().remove_resource::<ClientSession>();
    app.update();
    assert!(*app.world().resource::<CraftingView>() == CraftingView::default());
}

#[test]
fn items_without_recipes_do_not_open_or_replace_recipe_popups() {
    let mut app = browser_app();
    let item = ItemKind::Block(7);
    assert!(alternatives(app.world().resource::<RecipeBook>(), Some(item)).is_empty());
    app.world_mut()
        .resource_mut::<ClientSession>()
        .inventory
        .insert(item, 1);
    for action in [
        InventoryAction::Recipe(item_id(item)),
        InventoryAction::Recipe("missing-item".into()),
    ] {
        app.world_mut().write_message(action);
        app.update();
        assert!(!app.world().resource::<CraftingView>().recipe_open);
        assert!(
            app.world()
                .resource::<CraftingState>()
                .recipe_item
                .is_none()
        );
    }
    command(&mut app, CraftingAction::OpenBook);
    let before = {
        let state = app.world().resource::<CraftingState>();
        (
            state.recipe_item,
            state.selected,
            state.error.clone(),
            state.cells,
        )
    };
    let inventory = app.world().resource::<ClientSession>().inventory.clone();
    // A survival catalog click follows the same recipe path as R/Recipe.
    app.world_mut().resource_mut::<ClientSession>().game_mode = GameMode::Survival;
    app.world_mut().write_message(InventoryAction::Pointer {
        command: "catalog".into(),
        index: None,
        item: item_id(item),
        secondary: false,
    });
    app.update();
    let state = app.world().resource::<CraftingState>();
    assert!(state.recipe_open);
    assert_eq!(
        (
            state.recipe_item,
            state.selected,
            state.error.clone(),
            state.cells
        ),
        before
    );
    assert_eq!(app.world().resource::<ClientSession>().inventory, inventory);
    assert!(app.world().resource::<pointer::InventoryQueue>().is_empty());
}

#[test]
fn categories_and_alternatives_preserve_inventory_and_reserved_grid_until_explicit_transfer() {
    let mut app = browser_app();
    {
        let mut book = app.world_mut().resource_mut::<RecipeBook>();
        let base = book
            .get(terra_core::crafting::recipe_key("teramic::oak_planks"))
            .unwrap()
            .clone();
        let mut first = base.clone();
        first.id = "test::workbench_oak_a".into();
        first.name = "A Workbench Oak Planks".into();
        first.station = CraftStation::Workbench;
        let mut second = first.clone();
        second.id = "test::workbench_oak_b".into();
        second.name = "B Workbench Oak Planks".into();
        book.extend(vec![first, second]).unwrap();
    }
    app.world_mut()
        .resource_mut::<ClientSession>()
        .inventory
        .insert(ItemKind::Block(6), 5);
    command(
        &mut app,
        CraftingAction::Search("teramic::oak_planks".into()),
    );
    command(&mut app, CraftingAction::ApplyRecipe);
    let reserved = app.world().resource::<CraftingState>().cells;
    assert!(reserved.iter().any(Option::is_some));
    let inventory = app.world().resource::<ClientSession>().inventory.clone();
    command(
        &mut app,
        CraftingAction::Search("teramic::oak_planks".into()),
    );
    let view = app.world().resource::<CraftingView>();
    assert_eq!(view.recipe_categories.len(), 2);
    assert_eq!(
        view.recipe_categories.iter().filter(|r| r.selected).count(),
        1
    );
    assert!(
        view.recipe_categories
            .iter()
            .any(|r| r.id == "workbench" && r.count == "2")
    );
    assert!(
        view.recipe_categories
            .iter()
            .all(|r| !r.name.is_empty() && !r.preview_key.is_empty())
    );
    assert_eq!(view.recipe_page, "1 / 1");
    assert!(!view.recipe_multiple);
    command(&mut app, CraftingAction::Category("workbench".into()));
    let view = app.world().resource::<CraftingView>();
    assert_eq!(view.recipe_category, "Workbench");
    assert_eq!(view.recipe_page, "1 / 2");
    assert!(!view.can_apply_recipe);
    assert_eq!(view.recipe_alternatives.len(), 2);
    assert!(view.recipe_alternatives[0].selected);
    let first = view.recipe_alternatives[0].index;
    command(&mut app, CraftingAction::Alternative(1));
    assert_eq!(app.world().resource::<CraftingView>().recipe_page, "2 / 2");
    assert!(app.world().resource::<CraftingView>().recipe_alternatives[1].selected);
    command(&mut app, CraftingAction::SelectAlternative(first));
    assert_eq!(app.world().resource::<CraftingView>().recipe_page, "1 / 2");
    command(&mut app, CraftingAction::Alternative(-1));
    assert_eq!(app.world().resource::<CraftingView>().recipe_page, "1 / 2");
    assert!(!app.world().resource::<CraftingView>().recipe_previous);
    command(&mut app, CraftingAction::Category("nonexistent".into()));
    command(&mut app, CraftingAction::SelectAlternative(usize::MAX));
    assert_eq!(app.world().resource::<CraftingView>().recipe_page, "1 / 2");
    assert_eq!(app.world().resource::<CraftingState>().cells, reserved);
    assert_eq!(app.world().resource::<ClientSession>().inventory, inventory);
    assert!(app.world().resource::<pointer::InventoryQueue>().is_empty());
    command(&mut app, CraftingAction::CloseRecipe);
    assert!(!app.world().resource::<CraftingView>().recipe_open);
    assert!(app.world().resource::<InventoryState>().open);
}

#[test]
fn pending_inventory_transactions_block_new_category_and_direct_alternative_navigation() {
    let mut app = browser_app();
    app.world_mut()
        .resource_mut::<ClientSession>()
        .inventory
        .insert(ItemKind::Block(6), 5);
    command(
        &mut app,
        CraftingAction::Search("teramic::oak_planks".into()),
    );
    let selection = app.world().resource::<CraftingState>().selected;
    let cells = app.world().resource::<CraftingState>().cells;
    let response = app.world().resource::<ClientSession>().inventory_response;
    app.world_mut().resource_mut::<CraftingState>().pending = Some(PendingGrid {
        response,
        cells,
        retries: 0,
    });
    for action in [
        CraftingAction::Category("smelting".into()),
        CraftingAction::SelectAlternative(1),
    ] {
        command(&mut app, action);
        assert_eq!(app.world().resource::<CraftingState>().selected, selection);
        assert_eq!(app.world().resource::<CraftingState>().cells, cells);
        assert_eq!(
            app.world().resource::<CraftingView>().message,
            "Waiting for the inventory update."
        );
    }
    assert!(app.world().resource::<pointer::InventoryQueue>().is_empty());
}

#[test]
fn real_machine_recipes_use_their_station_tabs_and_actual_input_output_counts() {
    let cases = [
        RecipeChoice::Smelt(&terra_core::furnace::recipes()[0]),
        RecipeChoice::Crush(&terra_core::crusher::recipes()[0]),
        RecipeChoice::Brew(&terra_core::brewing::recipes()[0]),
        RecipeChoice::Campfire(&terra_core::campfire::recipes()[0]),
    ];
    for choice in cases {
        let (key, kind) = choice.selection();
        let output = match choice {
            RecipeChoice::Smelt(r) | RecipeChoice::Campfire(r) => r.output,
            RecipeChoice::Crush(r) => r.output,
            RecipeChoice::Brew(r) => r.output(),
            RecipeChoice::Craft(_) => unreachable!(),
        };
        let mut app = browser_app();
        {
            let mut state = app.world_mut().resource_mut::<CraftingState>();
            state.selected = Some(key);
            state.selected_kind = kind;
            state.recipe_item = Some(output.item);
            state.recipe_open = true;
        }
        app.update();
        let view = app.world().resource::<CraftingView>();
        assert!(view.selected && view.recipe_open);
        assert!(
            view.recipe_categories
                .iter()
                .any(|r| r.id == choice.category() && r.selected)
        );
        assert_eq!(view.recipe_output[0].item, Some(output.item));
        assert_eq!(view.recipe_output[0].count, output.count.to_string());
        assert!(!view.recipe_grid.is_empty());
        assert!(!view.recipe_duration.is_empty());
        assert_eq!(
            view.recipe_alternatives
                .iter()
                .filter(|r| r.selected)
                .count(),
            1
        );
        assert_eq!(
            view.recipe_layout,
            if matches!(choice, RecipeChoice::Brew(_)) {
                "recipe-loose-inputs"
            } else {
                "recipe-single-input"
            }
        );
        assert!(!view.can_apply_recipe);
        assert!(app.world().resource::<pointer::InventoryQueue>().is_empty());
    }
}

#[test]
fn generated_mixing_and_coloring_recipes_keep_a_real_single_category_and_non_granting_action() {
    for item in [
        ItemKind::Dye(terra_core::beds::DyeColor::Rgb([18, 123, 211])),
        ItemKind::TintedBlock {
            material: 103,
            wood: terra_core::WoodSpecies::Oak,
            color: [18, 123, 211],
        },
    ] {
        let mut app = browser_app();
        app.world_mut()
            .resource_mut::<ClientSession>()
            .inventory
            .insert(item, 1);
        app.world_mut()
            .write_message(InventoryAction::Recipe(item_id(item)));
        app.update();
        let before = app.world().resource::<ClientSession>().inventory.clone();
        let view = app.world().resource::<CraftingView>();
        assert!(view.selected);
        assert_eq!(view.recipe_categories.len(), 1);
        assert!(view.recipe_categories[0].selected);
        assert_eq!(
            view.recipe_categories[0].id,
            if matches!(item, ItemKind::Dye(_)) {
                "mixing"
            } else {
                "coloring"
            }
        );
        assert_eq!(view.recipe_page, "1 / 1");
        assert_eq!(view.recipe_layout, "recipe-loose-inputs");
        assert!(!view.recipe_multiple && !view.can_apply_recipe);
        assert_eq!(view.recipe_output[0].item, Some(item));
        command(&mut app, CraftingAction::Category("workbench".into()));
        command(&mut app, CraftingAction::SelectAlternative(0));
        command(&mut app, CraftingAction::ApplyRecipe);
        assert_eq!(app.world().resource::<ClientSession>().inventory, before);
        assert!(app.world().resource::<pointer::InventoryQueue>().is_empty());
    }
}

#[test]
fn recipe_chrome_and_category_styles_use_supported_native_properties() {
    for css in [
        include_str!("../../../src-ui/components/recipe-inspector/recipe-inspector.component.css"),
        include_str!("../../../src-ui/components/recipe-category/recipe-category.component.css"),
    ] {
        tilt_ui::tilt_ui_css::parse_stylesheet(css).unwrap();
    }
}

#[test]
fn r_follows_preview_ingredients_in_both_modes_and_respects_search_focus() {
    for mode in [GameMode::Survival, GameMode::Creative] {
        for typing in [false, true] {
            let mut app = browser_app();
            app.init_resource::<terra_core::ControlsConfig>()
                .init_resource::<crate::SettingsView>()
                .init_resource::<InputFocus>()
                .init_resource::<pointer::InventoryPointer>()
                .add_message::<terra_network::client::ObjectInteractionRequest>()
                .add_systems(Update, super::super::keyboard.before(actions));
            app.world_mut().resource_mut::<ClientSession>().game_mode = mode;
            app.world_mut().resource_mut::<BlockPalette>().choices.push(
                terra_player::interaction::BlockChoice {
                    id: 16,
                    name: "Oak Planks".into(),
                    identifier: "teramic::oak_planks".into(),
                    description: "A board cut from oak.".into(),
                },
            );
            command(
                &mut app,
                CraftingAction::Search("teramic::gearbox_horizontal".into()),
            );
            command(&mut app, CraftingAction::Category("workbench".into()));
            assert!(
                app.world()
                    .resource::<CraftingView>()
                    .recipe_grid
                    .iter()
                    .any(|s| s.item == Some(ItemKind::Block(16)))
            );
            let before = app.world().resource::<ClientSession>().inventory.clone();
            app.world_mut()
                .resource_mut::<pointer::InventoryPointer>()
                .hover("recipe-preview", Some(0), "block-16", false);
            if typing {
                let field = app
                    .world_mut()
                    .spawn(ElementId("inventory-search".into()))
                    .id();
                app.world_mut()
                    .resource_mut::<InputFocus>()
                    .set(field, bevy::input_focus::FocusCause::Navigated);
            }
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(KeyCode::KeyR);
            app.update();
            let view = app.world().resource::<CraftingView>();
            assert!(view.recipe_open);
            assert_eq!(
                view.recipe_output[0].item,
                Some(ItemKind::Block(if typing { 154 } else { 16 }))
            );
            assert_eq!(app.world().resource::<ClientSession>().inventory, before);
            assert!(app.world().resource::<pointer::InventoryQueue>().is_empty());
        }
    }
}

#[test]
fn chest_previews_describe_the_real_wood_wildcard_for_every_plank_species() {
    let book = RecipeBook::builtins();
    let recipe = book
        .iter()
        .find(|recipe| recipe.output.item == ItemKind::Block(29))
        .unwrap();
    for plank in [16, 17, 18, 135] {
        assert!(
            recipe
                .ingredients()
                .iter()
                .all(|ingredient| ingredient.wood && ingredient.matches(ItemKind::Block(plank)))
        );
        let mut app = browser_app();
        app.world_mut()
            .resource_mut::<ClientSession>()
            .inventory
            .insert(ItemKind::Block(plank), 8);
        command(&mut app, CraftingAction::Search("teramic::chest".into()));
        let view = app.world().resource::<CraftingView>();
        let cells: Vec<_> = view.recipe_grid.iter().filter(|slot| !slot.empty).collect();
        assert_eq!(cells.len(), 8);
        assert!(cells.iter().all(|slot| slot.name == "Any wood planks"
            && slot.description.contains("Cherry")
            && !slot.missing));
        assert_eq!(view.recipe_alternatives.len(), 4);
        for (row, (name, material)) in view.recipe_alternatives.iter().zip(WOOD_PREVIEWS) {
            assert!(row.name.contains(name));
            assert!(row.summary.contains(name));
            assert!(!row.summary.contains("Any wood"));
            assert!(
                row.grid
                    .iter()
                    .filter(|slot| !slot.empty)
                    .all(|slot| slot.item == Some(ItemKind::Block(material)))
            );
        }
    }
}

#[test]
fn furnace_fuel_tabs_show_registered_times_and_never_change_inventory_or_recipe_reservations() {
    let mut app = browser_app();
    let recipe = terra_core::furnace::recipes()
        .iter()
        .find(|recipe| recipe.input == ItemKind::Mineral(terra_core::MineralItem::RawIron))
        .unwrap();
    command(&mut app, CraftingAction::Select(recipe.id));
    let baseline = app.world().resource::<ClientSession>().inventory.clone();
    let reserved = app.world().resource::<CraftingState>().cells;
    let fuels = terra_core::furnace::fuels();
    command(&mut app, CraftingAction::FurnaceTab(true));
    for (index, fuel) in fuels.iter().enumerate() {
        command(&mut app, CraftingAction::SelectFuel(index));
        let view = app.world().resource::<CraftingView>();
        assert!(view.smelting && view.furnace_fuels);
        assert_eq!(view.recipe_heading, "Furnace Recipe");
        assert_eq!(view.furnace_fuel_options.len(), fuels.len());
        assert_eq!(view.furnace_fuel[0].item, Some(fuel.item));
        assert_eq!(
            view.furnace_fuel_options[index].burn_time,
            format!("{:.1} s", fuel.seconds)
        );
        assert_eq!(view.recipe_grid[0].item, Some(recipe.input));
        assert_eq!(view.recipe_output[0].item, Some(recipe.output.item));
        assert!(
            view.recipe_alternatives
                .iter()
                .all(|alternative| !alternative.input.is_empty()
                    && !alternative.output.is_empty()
                    && !alternative.duration.is_empty())
        );
        assert_eq!(app.world().resource::<ClientSession>().inventory, baseline);
        assert_eq!(app.world().resource::<CraftingState>().cells, reserved);
        assert!(app.world().resource::<pointer::InventoryQueue>().is_empty());
    }
    command(&mut app, CraftingAction::SelectFuel(usize::MAX));
    command(&mut app, CraftingAction::FurnaceTab(false));
    assert!(!app.world().resource::<CraftingView>().furnace_fuels);
}

#[test]
fn four_wood_previews_change_only_the_preview_and_preserve_wildcard_availability() {
    let mut app = browser_app();
    app.world_mut()
        .resource_mut::<ClientSession>()
        .inventory
        .insert(ItemKind::Block(18), 8);
    command(&mut app, CraftingAction::Search("teramic::chest".into()));
    let before = app.world().resource::<ClientSession>().inventory.clone();
    let reserved = app.world().resource::<CraftingState>().cells;
    for (index, (name, material)) in WOOD_PREVIEWS.iter().enumerate() {
        command(&mut app, CraftingAction::PreviewWood(index));
        let view = app.world().resource::<CraftingView>();
        assert!(view.any_wood);
        assert_eq!(view.wood_options.len(), 4);
        assert_eq!(view.wood_options[index].name, *name);
        assert!(view.wood_options[index].selected);
        assert!(
            view.recipe_grid
                .iter()
                .filter(|slot| !slot.empty)
                .all(|slot| slot.item == Some(ItemKind::Block(*material)) && !slot.missing)
        );
        assert_eq!(app.world().resource::<ClientSession>().inventory, before);
        assert_eq!(app.world().resource::<CraftingState>().cells, reserved);
        assert!(app.world().resource::<pointer::InventoryQueue>().is_empty());
    }
    command(&mut app, CraftingAction::PreviewWood(usize::MAX));
    assert_eq!(
        app.world()
            .resource::<CraftingView>()
            .wood_options
            .iter()
            .filter(|wood| wood.selected)
            .count(),
        1
    );
}

#[test]
fn cauldron_layout_separates_reusable_spoon_from_consumed_inputs_and_has_no_fake_progress() {
    let mut app = browser_app();
    let recipe = &terra_core::brewing::recipes()[0];
    app.world_mut()
        .write_message(InventoryAction::Recipe(item_id(recipe.output().item)));
    app.update();
    let view = app.world().resource::<CraftingView>();
    assert!(view.brewing);
    assert_eq!(view.recipe_heading, "Cauldron Recipe");
    assert_eq!(view.cauldron_inputs.len(), recipe.inputs.len() + 1);
    assert!(
        view.cauldron_inputs
            .iter()
            .any(|slot| slot.item == Some(ItemKind::WaterBottle))
    );
    assert_eq!(view.cauldron_tools.len(), 1);
    assert!(
        view.cauldron_tools[0]
            .item
            .is_some_and(|item| terra_core::brewing::spoon_speed(item).is_some())
    );
    assert_eq!(view.cauldron_progress, "width: 0%;");
    assert!(
        view.recipe_alternatives
            .iter()
            .all(
                |alternative| alternative.input.len() == recipe.inputs.len() + 1
                    && !alternative.output.is_empty()
                    && !alternative.duration.is_empty()
            )
    );
    assert!(app.world().resource::<pointer::InventoryQueue>().is_empty());
}

#[test]
fn cauldron_preview_tracks_authoritative_progress_without_an_inventory_revision_change() {
    use terra_network::protocol::inventory::{ContainerHalf, ContainerSnapshot, ContainerView};
    let mut app = browser_app();
    let recipe = &terra_core::brewing::recipes()[0];
    app.world_mut().resource_mut::<ClientSession>().container = Some(ContainerSnapshot {
        anchor: [0, 0, 0],
        view: ContainerView::Primary,
        attached_campfire: true,
        workbench: None,
        furnace: None,
        crusher: None,
        mixer: None,
        filter: None,
        distributor: None,
        tunnel_random: None,
        campfire: None,
        cauldron: Some(terra_core::brewing::CauldronView {
            recipe: Some(recipe.id),
            water: 1,
            progress: 0.25,
            heated: true,
            working: true,
        }),
        halves: vec![ContainerHalf {
            position: [0, 0, 0],
            inventory: terra_core::Inventory {
                slots: vec![terra_core::ItemStack::EMPTY; terra_core::brewing::SLOTS],
                ..default()
            },
        }],
    });
    app.world_mut()
        .write_message(InventoryAction::Recipe(item_id(recipe.output().item)));
    app.update();
    assert_eq!(
        app.world().resource::<CraftingView>().cauldron_progress,
        "width: 25%;"
    );
    let before = app.world().resource::<ClientSession>().inventory.revision;
    app.world_mut()
        .resource_mut::<ClientSession>()
        .container
        .as_mut()
        .unwrap()
        .cauldron
        .as_mut()
        .unwrap()
        .progress = 0.75;
    app.update();
    assert_eq!(
        app.world().resource::<CraftingView>().cauldron_progress,
        "width: 75%;"
    );
    assert_eq!(
        app.world().resource::<ClientSession>().inventory.revision,
        before
    );
}

#[test]
fn wood_alternatives_are_distinct_interactive_previews_with_bounded_navigation() {
    let mut app = browser_app();
    app.world_mut()
        .resource_mut::<ClientSession>()
        .inventory
        .insert(ItemKind::Block(16), 8);
    command(&mut app, CraftingAction::Search("teramic::chest".into()));
    let before = app.world().resource::<ClientSession>().inventory.clone();
    let reserved = app.world().resource::<CraftingState>().cells;
    let view = app.world().resource::<CraftingView>();
    let row = view
        .recipe_alternatives
        .iter()
        .find(|row| row.wood == Some(2))
        .unwrap()
        .clone();
    assert_eq!(
        view.recipe_alternatives
            .iter()
            .map(|row| &row.id)
            .collect::<std::collections::HashSet<_>>()
            .len(),
        4
    );
    command(
        &mut app,
        CraftingAction::PreviewAlternative {
            index: row.index,
            wood: row.wood,
        },
    );
    assert_eq!(app.world().resource::<CraftingView>().recipe_page, "3 / 4");
    assert!(app.world().resource::<CraftingView>().wood_options[2].selected);
    command(&mut app, CraftingAction::Alternative(1));
    command(&mut app, CraftingAction::Alternative(1));
    assert_eq!(app.world().resource::<CraftingView>().recipe_page, "4 / 4");
    assert!(!app.world().resource::<CraftingView>().recipe_next);
    command(&mut app, CraftingAction::Alternative(-1));
    assert_eq!(app.world().resource::<CraftingView>().recipe_page, "3 / 4");
    assert_eq!(app.world().resource::<ClientSession>().inventory, before);
    assert_eq!(app.world().resource::<CraftingState>().cells, reserved);
    assert!(app.world().resource::<pointer::InventoryQueue>().is_empty());
}

#[test]
fn cape_result_preview_uses_owner_heraldry_and_refreshes_after_realm_appearance_changes() {
    use terra_core::{
        CapeItem, PlayerUuid,
        crafting::{Ingredient, RecipePattern},
        kingdom::{KingdomHeraldry, KingdomIcon, SettlementKind},
    };
    let mut app = browser_app();
    let owner = PlayerUuid::from_u128(196);
    let cape = ItemKind::Cape(CapeItem::ALL_WHITE[0]);
    let initial = KingdomHeraldry {
        color: [27, 109, 181],
        icon: KingdomIcon::Anchor,
    };
    let id = {
        let mut session = app.world_mut().resource_mut::<ClientSession>();
        session.uuid = Some(owner);
        session.inventory.insert(ItemKind::Stick, 1);
        session
            .kingdoms
            .found_settlement(
                owner,
                "Alice",
                [0; 3],
                "Cape makers",
                SettlementKind::Kingdom,
                initial,
            )
            .unwrap()
    };
    app.world_mut()
        .resource_mut::<RecipeBook>()
        .extend(vec![Recipe {
            id: "test::preview_cape".into(),
            name: "Cape".into(),
            station: CraftStation::Hand,
            pattern: RecipePattern::Shapeless {
                ingredients: vec![Ingredient {
                    item: ItemKind::Stick,
                    count: 1,
                    wood: false,
                }],
            },
            output: terra_core::ItemStack {
                item: cape,
                count: 1,
            },
        }])
        .unwrap();
    app.world_mut().resource_mut::<CraftingState>().cells[0] =
        Some(CraftGridCell { slot: 0, count: 1 });
    app.update();
    assert_eq!(
        app.world().resource::<CraftingView>().output[0].item,
        Some(cape.with_default_heraldry(Some(initial)))
    );
    assert!(
        !alternatives(
            RecipeBook::builtins(),
            Some(cape.with_default_heraldry(Some(initial)))
        )
        .is_empty()
    );
    let changed = KingdomHeraldry {
        color: [202, 66, 31],
        icon: KingdomIcon::Tree,
    };
    app.world_mut()
        .resource_mut::<ClientSession>()
        .kingdoms
        .change_appearance(owner, id, changed)
        .unwrap();
    app.update();
    assert_eq!(
        app.world().resource::<CraftingView>().output[0].item,
        Some(cape.with_default_heraldry(Some(changed)))
    );
}
