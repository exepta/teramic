use super::*;
use terra_core::{Inventory, ItemStack, ToolKind, brewing, campfire};

fn station_snapshot(anchor: [i64; 3], cauldron: bool) -> ContainerSnapshot {
    let mut snapshot = chest_snapshot(anchor, false);
    snapshot.halves[0].inventory = Inventory {
        slots: vec![
            ItemStack::EMPTY;
            if cauldron {
                brewing::SLOTS
            } else {
                campfire::SLOTS
            }
        ],
        ..default()
    };
    if cauldron {
        snapshot.cauldron = Some(brewing::CauldronView {
            recipe: Some(1),
            water: 2,
            progress: 0.4,
            heated: true,
            working: false,
        });
    } else {
        snapshot.campfire = Some(campfire::CampfireView::default());
    }
    snapshot
}

fn compact_station_snapshot(anchor: [i64; 3], fire: bool) -> ContainerSnapshot {
    let mut snapshot = station_snapshot(anchor, !fire);
    snapshot.attached_campfire = true;
    snapshot.view = if fire {
        ContainerView::AttachedCampfire
    } else {
        ContainerView::Primary
    };
    snapshot
}

#[test]
fn compact_cauldron_navigation_opens_both_real_views_without_closing_the_inventory() {
    let anchor = [1, 60, 1];
    let mut app = app(GameMode::Survival);
    app.world_mut().resource_mut::<ClientSession>().container =
        Some(compact_station_snapshot(anchor, false));
    action(&mut app, InventoryAction::Toggle);
    let before = app.world().resource::<ClientSession>().inventory.clone();
    let view = app.world().resource::<InventoryView>();
    assert!(view.hearth_navigation && view.hearth_show_fire && view.hearth_navigation_enabled);
    assert_eq!(view.container_items.len(), brewing::SLOTS);
    action(&mut app, InventoryAction::HearthView(true));
    assert_eq!(
        requests(&mut app)[0].action,
        ServerAction::OpenCampfire { position: anchor }
    );
    assert!(
        !app.world()
            .resource::<InventoryView>()
            .hearth_navigation_enabled
    );
    {
        let mut session = app.world_mut().resource_mut::<ClientSession>();
        session.container = Some(compact_station_snapshot(anchor, true));
        session.inventory_response += 1;
    }
    app.update();
    let view = app.world().resource::<InventoryView>();
    assert!(
        view.open
            && view.hearth_navigation
            && !view.hearth_show_fire
            && view.hearth_navigation_enabled
            && view.campfire.active
            && !view.cauldron.active
    );
    assert_eq!(view.container_items.len(), campfire::SLOTS);
    action(&mut app, InventoryAction::HearthView(false));
    assert_eq!(
        requests(&mut app)[0].action,
        ServerAction::OpenContainer { position: anchor }
    );
    assert_eq!(app.world().resource::<ClientSession>().inventory, before);
}

#[test]
fn compact_hearth_navigation_requires_empty_cursor_and_no_in_flight_transfer() {
    let anchor = [1, 60, 1];
    let mut app = app(GameMode::Survival);
    app.world_mut().resource_mut::<ClientSession>().container =
        Some(compact_station_snapshot(anchor, false));
    action(&mut app, InventoryAction::Toggle);
    app.world_mut()
        .resource_mut::<ClientSession>()
        .inventory
        .cursor = Some(ItemStack {
        item: ItemKind::Stick,
        count: 1,
    });
    action(&mut app, InventoryAction::HearthView(true));
    assert!(requests(&mut app).is_empty());
    assert!(
        !app.world()
            .resource::<InventoryView>()
            .hearth_navigation_enabled
    );
    app.world_mut()
        .resource_mut::<ClientSession>()
        .inventory
        .cursor = None;
    action(&mut app, InventoryAction::BrewingRecipe(6));
    assert_eq!(requests(&mut app).len(), 1);
    action(&mut app, InventoryAction::HearthView(true));
    assert!(requests(&mut app).is_empty());
    assert!(
        !app.world()
            .resource::<InventoryView>()
            .hearth_navigation_enabled
    );
}

#[test]
fn queued_hearth_transfers_keep_their_exact_view_and_never_retarget_the_same_cell() {
    for fire in [false, true] {
        let anchor = [1, 60, 1];
        let filled_snapshot = |fire| {
            let mut snapshot = compact_station_snapshot(anchor, fire);
            snapshot.halves[0].inventory.slots[0] = ItemStack {
                item: if fire {
                    campfire::recipes()[0].input
                } else {
                    brewing::recipes()[0].inputs[0].item
                },
                count: 2,
            };
            assert!(snapshot.valid());
            assert_eq!(snapshot.halves[0].inventory.revision, 0);
            snapshot
        };
        let mut app = queue_app(Some(filled_snapshot(fire)));
        let expected = if fire {
            ContainerView::AttachedCampfire
        } else {
            ContainerView::Primary
        };
        {
            let mut queue = app.world_mut().resource_mut::<pointer::InventoryQueue>();
            queue.owner = Some(anchor);
            queue.owner_view = Some(expected);
            for _ in 0..2 {
                queue.push(ServerAction::Click {
                    slot: terra_core::INVENTORY_SLOTS as u8,
                    secondary: false,
                });
            }
        }
        app.update();
        let commands = container_requests(&mut app);
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].view, expected);
        assert_eq!(commands[0].revisions, [0]);
        {
            let mut session = app.world_mut().resource_mut::<ClientSession>();
            session.container = Some(filled_snapshot(!fire));
            session.inventory_response += 1;
        }
        app.update();
        assert!(container_requests(&mut app).is_empty());
        assert!(requests(&mut app).is_empty());
        assert!(app.world().resource::<pointer::InventoryQueue>().is_empty());
    }
}

#[test]
fn alchemy_catalog_offers_each_finite_potion_once_with_independent_effect_tooltips() {
    let palette = BlockPalette::default();
    let items = available_items(GameMode::Creative, &Inventory::default(), &palette);
    for recipe in brewing::recipes() {
        let item = recipe.output().item;
        assert_eq!(
            items.iter().filter(|&&candidate| candidate == item).count(),
            1
        );
        assert!(catalog_item(item));
        let view = view::item_view(item, 1, &palette);
        assert_eq!(view.name, recipe.name);
        assert_eq!(view.identifier, item.identifier());
        assert_eq!(view.preview_key, preview_key(item));
        for effect in &recipe.effects {
            assert!(view.description.contains(&format!(
                "{} {}: {} s",
                terra_core::effect_definition(effect.kind).name,
                effect.level,
                effect.seconds
            )));
        }
        assert!(view.description.contains("Returns a glass bottle"));
    }
    for (tool, speed) in [
        (ToolKind::WoodSpoon, 1.0),
        (ToolKind::StoneSpoon, 1.2),
        (ToolKind::GoldSpoon, 1.5),
    ] {
        let view = view::item_view(ItemKind::Tool(tool), 1, &palette);
        assert!(
            view.description
                .contains(&format!("{speed:.1}x | Reusable"))
        );
    }
}

#[test]
fn negative_potion_recipes_are_available_in_tmi_without_duplicate_catalog_items() {
    use terra_core::effects::EffectKind;
    for effect in [
        EffectKind::Poison,
        EffectKind::Slow,
        EffectKind::Freeze,
        EffectKind::Bleeding,
        EffectKind::Rotten,
        EffectKind::OnFire,
    ] {
        let recipe = brewing::recipes()
            .iter()
            .find(|recipe| recipe.effects.iter().any(|dose| dose.kind == effect))
            .expect("Every requested debuff has a real brewing recipe");
        let mut app = app(GameMode::Creative);
        action(&mut app, InventoryAction::Toggle);
        action(
            &mut app,
            InventoryAction::Recipe(item_id(recipe.output().item)),
        );
        let view = app.world().resource::<CraftingView>();
        assert!(view.brewing && view.selected && view.recipe_open);
        assert_eq!(view.recipe_output[0].item, Some(recipe.output().item));
        assert_eq!(view.description, alchemy::effects(recipe));
        assert!(
            view.recipe_grid
                .iter()
                .any(|slot| slot.item == Some(ItemKind::WaterBottle))
        );
        assert!(
            !view
                .recipe_grid
                .iter()
                .any(|slot| slot.item == Some(ItemKind::GlassBottle))
        );
    }
}

#[test]
fn cauldron_workspace_has_seven_real_slots_and_shapeless_missing_ghosts() {
    let mut app = app(GameMode::Survival);
    let recipe = brewing::recipes()
        .iter()
        .find(|recipe| recipe.inputs.len() == 3)
        .unwrap();
    let mut snapshot = station_snapshot([1, 60, 1], true);
    snapshot.cauldron.as_mut().unwrap().recipe = Some(recipe.id);
    // A correct ingredient in a different slot must not reappear as a ghost.
    snapshot.halves[0].inventory.slots[2] = recipe.inputs[0];
    snapshot.halves[0].inventory.slots[brewing::BOTTLES] = ItemStack {
        item: ItemKind::GlassBottle,
        count: 2,
    };
    snapshot.halves[0].inventory.slots[brewing::SPOON] = ItemStack {
        item: ItemKind::Tool(ToolKind::GoldSpoon),
        count: 1,
    };
    let before = snapshot.halves[0].inventory.clone();
    app.world_mut().resource_mut::<ClientSession>().container = Some(snapshot);
    action(&mut app, InventoryAction::Toggle);
    let view = app.world().resource::<InventoryView>();
    assert!(!view.container && view.cauldron.active && !view.campfire.active);
    assert_eq!(view.title, "Cauldron");
    assert_eq!(view.container_items.len(), brewing::SLOTS);
    assert_eq!(view.cauldron.inputs[2].item, Some(recipe.inputs[0].item));
    assert_eq!(
        view.cauldron.inputs[0].ghost_preview,
        preview_key(recipe.inputs[1].item)
    );
    assert_eq!(
        view.cauldron.inputs[1].ghost_preview,
        preview_key(recipe.inputs[2].item)
    );
    assert!(
        view.cauldron.inputs[..2]
            .iter()
            .all(|slot| slot.empty && slot.item.is_none() && slot.item_id.is_empty())
    );
    for (slots, index) in [
        (&view.cauldron.water, brewing::WATER),
        (&view.cauldron.bottles, brewing::BOTTLES),
        (&view.cauldron.spoon, brewing::SPOON),
        (&view.cauldron.output, brewing::OUTPUT),
    ] {
        assert_eq!(slots.len(), 1);
        assert_eq!(slots[0].command, "container");
        assert_eq!(slots[0].index, index.to_string());
    }
    assert_eq!(view.cauldron.progress, "width: 40.0%;");
    assert_eq!(view.cauldron.water_amount, "2 / 3");
    assert_eq!(view.cauldron.status, "Missing ingredients");
    assert_eq!(
        app.world()
            .resource::<ClientSession>()
            .container
            .as_ref()
            .unwrap()
            .halves[0]
            .inventory,
        before
    );
}

#[test]
fn cauldron_recipe_selection_is_validated_optimistic_and_never_moves_inputs() {
    let mut app = app(GameMode::Survival);
    let anchor = [1, 60, 1];
    app.world_mut().resource_mut::<ClientSession>().container =
        Some(station_snapshot(anchor, true));
    action(&mut app, InventoryAction::Toggle);
    let before = app.world().resource::<ClientSession>().inventory.clone();
    action(&mut app, InventoryAction::BrewingRecipe(u16::MAX));
    assert!(requests(&mut app).is_empty());
    let recipe = brewing::recipes().last().unwrap();
    action(&mut app, InventoryAction::BrewingRecipe(recipe.id));
    assert_eq!(
        requests(&mut app)[0].action,
        ServerAction::SetBrewingRecipe {
            station: anchor,
            recipe: recipe.id
        }
    );
    assert_eq!(
        app.world().resource::<InventoryView>().cauldron.recipe,
        recipe.id.to_string()
    );
    assert_eq!(app.world().resource::<ClientSession>().inventory, before);
    assert_eq!(
        app.world()
            .resource::<ClientSession>()
            .container
            .as_ref()
            .unwrap()
            .cauldron
            .as_ref()
            .unwrap()
            .recipe,
        Some(1)
    );
}

#[test]
fn campfire_ui_shows_real_slots_and_queues_lighting_without_local_fuel_consumption() {
    let mut app = app(GameMode::Survival);
    let anchor = [1, 60, 1];
    let mut snapshot = station_snapshot(anchor, false);
    snapshot.halves[0].inventory.slots[campfire::FUEL] = ItemStack {
        item: ItemKind::Stick,
        count: 3,
    };
    let before = snapshot.halves[0].inventory.clone();
    app.world_mut().resource_mut::<ClientSession>().container = Some(snapshot);
    action(&mut app, InventoryAction::Toggle);
    let view = app.world().resource::<InventoryView>();
    assert!(
        view.campfire.active && view.campfire.can_toggle && !view.campfire.lit && !view.container
    );
    assert_eq!(view.container_items.len(), campfire::SLOTS);
    assert_eq!(view.campfire.input[0].index, "0");
    assert_eq!(view.campfire.fuel[0].index, "1");
    assert_eq!(view.campfire.output[0].index, "2");
    action(&mut app, InventoryAction::CampfireLit(true));
    assert_eq!(
        requests(&mut app)[0].action,
        ServerAction::SetCampfireLit {
            station: anchor,
            lit: true
        }
    );
    assert!(app.world().resource::<InventoryView>().campfire.lit);
    assert_eq!(
        app.world()
            .resource::<ClientSession>()
            .container
            .as_ref()
            .unwrap()
            .halves[0]
            .inventory,
        before
    );
}

#[test]
fn brewing_tmi_single_and_mixed_recipes_are_real_and_cannot_fill_a_crafting_grid() {
    for recipe in brewing::recipes() {
        let mut app = app(GameMode::Creative);
        action(&mut app, InventoryAction::Toggle);
        action(
            &mut app,
            InventoryAction::Recipe(item_id(recipe.output().item)),
        );
        let view = app.world().resource::<CraftingView>();
        assert!(view.selected && view.brewing && view.recipe_open && !view.can_apply_recipe);
        assert_eq!(view.recipe_output[0].item, Some(recipe.output().item));
        assert_eq!(view.recipe_grid.len(), recipe.inputs.len() + 2);
        assert!(
            view.ingredients
                .iter()
                .any(|ingredient| ingredient.name.contains("reusable"))
        );
        assert_eq!(view.description, alchemy::effects(recipe));
        assert_eq!(view.recipe_station, "Cauldron / lit campfire");
        add_recipe(&mut app);
        assert!(requests(&mut app).is_empty());
    }
    let mut app = app(GameMode::Creative);
    let anchor = [1, 60, 1];
    app.world_mut().resource_mut::<ClientSession>().container =
        Some(station_snapshot(anchor, true));
    action(&mut app, InventoryAction::Toggle);
    let recipe = brewing::recipes().last().unwrap();
    action(
        &mut app,
        InventoryAction::Recipe(item_id(recipe.output().item)),
    );
    assert!(app.world().resource::<CraftingView>().can_apply_recipe);
    add_recipe(&mut app);
    assert_eq!(
        requests(&mut app)[0].action,
        ServerAction::SetBrewingRecipe {
            station: anchor,
            recipe: recipe.id
        }
    );
}

#[test]
fn fresh_brew_tmi_needs_one_filled_bottle_not_an_additional_empty_bottle() {
    let recipe = brewing::recipe(6).unwrap();
    let mut app = app(GameMode::Survival);
    {
        let mut session = app.world_mut().resource_mut::<ClientSession>();
        for required in &recipe.inputs {
            session.inventory.insert(required.item, required.count);
        }
        session.inventory.insert(ItemKind::WaterBottle, 1);
        session
            .inventory
            .insert(ItemKind::Tool(ToolKind::GoldSpoon), 1);
    }
    action(&mut app, InventoryAction::Toggle);
    action(
        &mut app,
        InventoryAction::Recipe(item_id(recipe.output().item)),
    );
    let view = app.world().resource::<CraftingView>();
    assert!(view.brewing && !view.missing);
    assert!(view.ingredients.iter().all(|ingredient| ingredient.enough));
    assert_eq!(
        view.recipe_grid
            .iter()
            .filter(|slot| slot.item == Some(ItemKind::WaterBottle))
            .count(),
        1
    );
    assert!(
        !view
            .recipe_grid
            .iter()
            .any(|slot| slot.item == Some(ItemKind::GlassBottle))
    );
    assert!(
        view.ingredients
            .iter()
            .any(|ingredient| ingredient.name == "Stirring spoon (reusable)")
    );

    let mut inventory = Inventory {
        slots: vec![ItemStack::EMPTY; brewing::SLOTS],
        ..default()
    };
    for (slot, required) in recipe.inputs.iter().enumerate() {
        inventory.slots[slot] = *required;
    }
    inventory.slots[brewing::WATER] = ItemStack {
        item: ItemKind::WaterBottle,
        count: 1,
    };
    inventory.slots[brewing::SPOON] = ItemStack {
        item: ItemKind::Tool(ToolKind::GoldSpoon),
        count: 1,
    };
    let mut state = brewing::CauldronState {
        recipe: Some(recipe.id),
        ..default()
    };
    brewing::advance(&mut state, &mut inventory, false, 1.0);
    assert_eq!(
        inventory.slot(brewing::BOTTLES),
        Some(ItemStack {
            item: ItemKind::GlassBottle,
            count: 1
        })
    );
    for _ in 0..recipe.seconds.ceil() as usize {
        brewing::advance(&mut state, &mut inventory, true, 1.0);
    }
    assert_eq!(inventory.slot(brewing::OUTPUT), Some(recipe.output()));
    assert!(inventory.slot(brewing::BOTTLES).is_none());
    assert_eq!(
        inventory.slot(brewing::SPOON),
        Some(ItemStack {
            item: ItemKind::Tool(ToolKind::GoldSpoon),
            count: 1
        })
    );
}

#[test]
fn campfire_tmi_uses_distinct_station_tabs_and_keeps_the_real_campfire_context_selected() {
    for recipe in campfire::recipes() {
        let mut app = app(GameMode::Creative);
        app.world_mut().resource_mut::<ClientSession>().container =
            Some(station_snapshot([1, 60, 1], false));
        assert!(
            app.world()
                .resource::<ClientSession>()
                .container
                .as_ref()
                .unwrap()
                .campfire
                .is_some()
        );
        action(&mut app, InventoryAction::Toggle);
        action(
            &mut app,
            InventoryAction::Recipe(item_id(recipe.output.item)),
        );
        let inventory = app.world().resource::<ClientSession>().inventory.clone();
        let container = app.world().resource::<ClientSession>().container.clone();
        let view = app.world().resource::<CraftingView>();
        // The actual open Campfire still determines the initial recipe. Page
        // counts now describe alternatives inside the selected station tab.
        assert!(view.campfire && view.selected && !view.smelting);
        assert!(
            view.recipe_categories
                .iter()
                .any(|tab| tab.id == "campfire" && tab.selected)
        );
        assert!(
            view.recipe_categories
                .iter()
                .any(|tab| tab.id == "smelting" && !tab.selected)
        );
        assert_eq!(
            view.recipe_categories
                .iter()
                .filter(|tab| tab.selected)
                .count(),
            1
        );
        assert!(!view.recipe_multiple && !view.recipe_next && !view.recipe_previous);
        assert_eq!(view.recipe_page, "1 / 1");
        assert_eq!(view.recipe_station, "Campfire / fuel");
        assert_eq!(view.recipe_shape, "Heat & fuel");
        assert_eq!(view.recipe_duration, format!("{} s", recipe.seconds));
        assert_eq!(view.recipe_output[0].item, Some(recipe.output.item));
        assert_eq!(view.recipe_output[0].count, recipe.output.count.to_string());
        assert!(!view.can_apply_recipe);
        assert!(requests(&mut app).is_empty());
        for category in ["smelting", "campfire"] {
            app.world_mut()
                .write_message(crafting::CraftingAction::Category(category.into()));
            app.update();
            let view = app.world().resource::<CraftingView>();
            assert_eq!(view.smelting, category == "smelting");
            assert_eq!(view.campfire, category == "campfire");
            assert!(view.selected && view.recipe_open);
            assert!(
                view.recipe_categories
                    .iter()
                    .any(|tab| tab.id == category && tab.selected)
            );
            assert_eq!(
                view.recipe_station,
                if category == "campfire" {
                    "Campfire / fuel"
                } else {
                    "Furnace"
                }
            );
            assert_eq!(view.recipe_output[0].item, Some(recipe.output.item));
            assert!(!view.can_apply_recipe);
            app.world_mut()
                .write_message(crafting::CraftingAction::ApplyRecipe);
            app.update();
            assert!(requests(&mut app).is_empty());
            assert_eq!(app.world().resource::<ClientSession>().inventory, inventory);
            assert_eq!(app.world().resource::<ClientSession>().container, container);
        }
    }
}

#[test]
fn tunnel_random_switch_preserves_filter_and_non_tunnels_ignore_the_action() {
    for random in [false, true] {
        let mut app = app(GameMode::Survival);
        let anchor = [1, 60, 1];
        let mut snapshot = filtered_funnel_snapshot(anchor);
        snapshot.tunnel_random = Some(random);
        snapshot.filter = Some(terra_core::logistics::FilterConfig {
            item: Some(ItemKind::Block(3)),
            allowed: false,
        });
        app.world_mut().resource_mut::<ClientSession>().container = Some(snapshot);
        action(&mut app, InventoryAction::Toggle);
        let view = app.world().resource::<InventoryView>();
        assert!(view.tunnel_options && !view.filter_allowed && view.filter_has_item);
        assert_eq!(view.tunnel_random, random);
        action(&mut app, InventoryAction::TunnelRandom(!random));
        assert_eq!(
            requests(&mut app)[0].action,
            ServerAction::SetTunnelRandom {
                station: anchor,
                random: !random
            }
        );
        let view = app.world().resource::<InventoryView>();
        assert_eq!(view.tunnel_random, !random);
        assert!(!view.filter_allowed && view.filter_has_item);
    }
    let mut app = app(GameMode::Survival);
    app.world_mut().resource_mut::<ClientSession>().container =
        Some(filtered_funnel_snapshot([1, 60, 1]));
    action(&mut app, InventoryAction::Toggle);
    action(&mut app, InventoryAction::TunnelRandom(true));
    assert!(requests(&mut app).is_empty());
    assert!(!app.world().resource::<InventoryView>().tunnel_options);
}

#[test]
fn ordinary_tunnel_has_only_random_controls_and_no_ghost_filter_workspace() {
    let mut app = app(GameMode::Survival);
    let anchor = [1, 60, 1];
    let mut snapshot = filtered_funnel_snapshot(anchor);
    snapshot.filter = None;
    snapshot.tunnel_random = Some(false);
    assert!(snapshot.valid());
    app.world_mut().resource_mut::<ClientSession>().container = Some(snapshot);
    action(&mut app, InventoryAction::Toggle);
    let view = app.world().resource::<InventoryView>();
    assert_eq!(view.title, "Tunnel");
    assert!(view.tunnel_options);
    assert!(!view.filtered_funnel && !view.container);
    assert!(view.funnel_filter.is_empty());
    action(&mut app, InventoryAction::FilterAllowed(false));
    action(&mut app, InventoryAction::FilterClear);
    assert!(requests(&mut app).is_empty());
    action(&mut app, InventoryAction::TunnelRandom(true));
    let queued = requests(&mut app);
    assert_eq!(queued.len(), 1);
    assert_eq!(
        queued[0].action,
        ServerAction::SetTunnelRandom {
            station: anchor,
            random: true
        }
    );
    let view = app.world().resource::<InventoryView>();
    assert!(view.tunnel_random && !view.filtered_funnel && view.funnel_filter.is_empty());
}
